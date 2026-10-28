//! Preserve stream bytes when the IOS receive transfer needs complete cache lines.
extern crate alloc;
use alloc::vec::Vec;

#[repr(align(32))]
#[derive(Default)]
pub(crate) struct ReceiveBuffer {
    bytes: [u8; 32],
    start: usize,
    end: usize,
}
impl ReceiveBuffer {
    pub(crate) fn receive<E>(
        &mut self,
        output: &mut [u8],
        datagram: bool,
        peek: bool,
        mut read: impl FnMut(&mut [u8]) -> Result<usize, E>,
        error: impl Fn(i32) -> E,
    ) -> Result<usize, E> {
        let length = output.len().min(65535);
        if !datagram && length == 0 {
            return Ok(0);
        }
        if !datagram && self.start != self.end {
            let count = length.min(self.end - self.start);
            output[..count].copy_from_slice(&self.bytes[self.start..self.start + count]);
            if !peek {
                self.start += count;
            }
            return Ok(count);
        }
        if length >= 32 && length.is_multiple_of(32) {
            let count = read(&mut output[..length])?;
            return if count <= length {
                Ok(count)
            } else {
                Err(error(77))
            };
        }
        if !datagram && length >= 32 {
            let capacity = length & !31;
            let count = read(&mut output[..capacity])?;
            return if count <= capacity {
                Ok(count)
            } else {
                Err(error(77))
            };
        }
        if datagram && length > 32 {
            let capacity = (length + 31) & !31;
            let mut bytes = Vec::new();
            bytes.try_reserve_exact(capacity).map_err(|_| error(12))?;
            bytes.resize(capacity, 0);
            let count = read(&mut bytes)?;
            if count > capacity {
                return Err(error(77));
            }
            let copied = length.min(count);
            output[..copied].copy_from_slice(&bytes[..copied]);
            return Ok(copied);
        }
        let count = read(&mut self.bytes)?;
        if count > self.bytes.len() {
            return Err(error(77));
        }
        let copied = length.min(count);
        output[..copied].copy_from_slice(&self.bytes[..copied]);
        if !datagram && !peek {
            self.start = copied;
            self.end = count;
        }
        Ok(copied)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn five_byte_read_preserves_every_extra_stream_byte_for_the_next_reader() {
        let mut state = ReceiveBuffer::default();
        let mut prefix = [0; 5];
        let count = state
            .receive(
                &mut prefix,
                false,
                false,
                |wire| {
                    assert_eq!(wire.len(), 32);
                    wire[..12].copy_from_slice(b"HTTP/1.0 200");
                    Ok(12)
                },
                |code| code,
            )
            .unwrap();
        assert_eq!(count, 5);
        assert_eq!(&prefix, b"HTTP/");
        let mut rest = [0; 16];
        let count = state
            .receive(
                &mut rest,
                false,
                false,
                |_| panic!("retained bytes"),
                |code| code,
            )
            .unwrap();
        assert_eq!(count, 7);
        assert_eq!(&rest[..7], b"1.0 200");
    }
    #[test]
    fn stream_peek_does_not_consume_retained_or_ios_bytes() {
        let mut state = ReceiveBuffer::default();
        let mut one = [0; 1];
        for peek in [true, false] {
            assert_eq!(
                state.receive(
                    &mut one,
                    false,
                    peek,
                    |wire| {
                        wire[..4].copy_from_slice(b"abcd");
                        Ok(4)
                    },
                    |code| code
                ),
                Ok(1)
            );
            assert_eq!(&one, b"a");
        }
        assert_eq!(
            state.receive(&mut one, false, true, |_| panic!("retained"), |code| code),
            Ok(1)
        );
        assert_eq!(&one, b"b");
        assert_eq!(
            state.receive(&mut one, false, false, |_| panic!("retained"), |code| code),
            Ok(1)
        );
        assert_eq!(&one, b"b");
    }
    #[test]
    fn datagram_truncation_never_leaks_the_previous_packet() {
        let mut state = ReceiveBuffer::default();
        let mut output = [0; 4];
        assert_eq!(
            state.receive(
                &mut output,
                true,
                false,
                |wire| {
                    wire[..9].copy_from_slice(b"51a7-data");
                    Ok(9)
                },
                |code| code
            ),
            Ok(4)
        );
        assert_eq!(&output, b"51a7");
        assert_eq!(
            state.receive(
                &mut output,
                true,
                false,
                |wire| {
                    wire[..4].copy_from_slice(b"next");
                    Ok(4)
                },
                |code| code
            ),
            Ok(4)
        );
        assert_eq!(&output, b"next");
    }
    #[test]
    fn large_datagram_keeps_the_callers_full_capacity() {
        let mut state = ReceiveBuffer::default();
        let mut output = [0; 33];
        assert_eq!(
            state.receive(
                &mut output,
                true,
                false,
                |wire| {
                    assert_eq!(wire.len(), 64);
                    wire[..33].fill(0xa5);
                    Ok(33)
                },
                |code| code
            ),
            Ok(33)
        );
        assert_eq!(output, [0xa5; 33]);
    }
    #[test]
    fn empty_stream_does_not_read_but_empty_datagram_consumes_one_packet() {
        let mut state = ReceiveBuffer::default();
        assert_eq!(
            state.receive(
                &mut [],
                false,
                false,
                |_| panic!("empty stream"),
                |code| code
            ),
            Ok(0)
        );
        let mut calls = 0;
        assert_eq!(
            state.receive(
                &mut [],
                true,
                false,
                |_| {
                    calls += 1;
                    Ok(4)
                },
                |code| code
            ),
            Ok(0)
        );
        assert_eq!(calls, 1);
    }
    #[test]
    fn real_errors_and_invalid_transfer_counts_are_returned() {
        let mut state = ReceiveBuffer::default();
        assert_eq!(
            state.receive(&mut [0; 5], false, false, |_| Err(-6), |code| code),
            Err(-6)
        );
        assert_eq!(
            state.receive(&mut [0; 5], false, false, |_| Ok(33), |code| code),
            Err(77)
        );
    }
}
