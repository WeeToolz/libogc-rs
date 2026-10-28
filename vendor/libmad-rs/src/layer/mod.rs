//! Shared bitstream operations for the MPEG layer decoders.

use crate::{
    bit::{self, BitPtr},
    frame::{FLAG_PROTECTION, Frame, OPTION_IGNORECRC},
    stream::{Error, Stream},
};

pub mod decode;
pub mod huffdecode;
pub mod huffman;
pub mod imdct;
pub mod layer12;
pub mod layer2;
pub mod layer3;
pub mod overlap;
pub mod requantize;
pub mod scalefactor;
pub mod spectral;
pub mod stereo;
pub mod transform;

pub(super) fn bits_remaining(ptr: BitPtr, end: usize) -> isize {
    BitPtr::bit_length(ptr, BitPtr::new(end))
}

pub(super) fn read_bits(
    stream: &mut Stream,
    ptr: &mut BitPtr,
    bits: u32,
    end: usize,
) -> Result<u32, Error> {
    if bits_remaining(*ptr, end) < bits as isize {
        stream.error = Error::LostSync;
        stream.sync = false;
        return Err(Error::LostSync);
    }
    match ptr.read(&stream.buffer, bits) {
        Ok(value) => {
            // C decoders advance `stream->ptr` directly during each read,
            // including when a later read in the same frame fails.
            stream.ptr = *ptr;
            Ok(value)
        }
        Err(_) => {
            stream.error = Error::LostSync;
            stream.sync = false;
            Err(Error::LostSync)
        }
    }
}

pub(super) fn requantize(raw: u32, nbits: u32) -> i32 {
    let sign = 1_u32 << (nbits - 1);
    let centered = raw ^ sign;
    (centered | (centered & sign).wrapping_neg()).wrapping_shl(28 - (nbits - 1)) as i32
}

pub(super) fn check_crc(
    frame: &mut Frame,
    stream: &mut Stream,
    ptr: BitPtr,
    bits: u32,
) -> Result<(), Error> {
    frame.header.crc_check =
        bit::crc(ptr, &stream.buffer, bits, frame.header.crc_check).map_err(|_| Error::BadCrc)?;
    if frame.header.flags & FLAG_PROTECTION != 0
        && frame.header.crc_check != frame.header.crc_target
        && frame.options & OPTION_IGNORECRC == 0
    {
        stream.error = Error::BadCrc;
        return Err(Error::BadCrc);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bounded_reads_accept_exact_end_and_reject_one_extra_bit_at_every_offset() {
        for consumed in 0..8 {
            let mut stream = Stream::new();
            stream.buffer(&[0xff, 0xff]);
            let mut start = BitPtr::new(0);
            start.read(&stream.buffer, consumed).unwrap();
            let remaining = 8 - consumed;

            let mut exact = start;
            assert_eq!(
                read_bits(&mut stream, &mut exact, remaining, 1),
                Ok((1 << remaining) - 1),
                "consumed={consumed}",
            );
            assert_eq!(
                exact,
                BitPtr {
                    byte: 1,
                    cache: 0xff,
                    left: 8
                }
            );
            assert_eq!(stream.ptr, exact);

            stream.sync = true;
            let mut overflow = start;
            assert_eq!(
                read_bits(&mut stream, &mut overflow, remaining + 1, 1),
                Err(Error::LostSync),
                "consumed={consumed}",
            );
            assert_eq!(overflow, start);
            assert_eq!(stream.ptr, exact);
            assert_eq!(stream.error, Error::LostSync);
            assert!(!stream.sync);
        }
    }

    #[test]
    fn bounded_read_rejects_bits_past_partial_byte_end() {
        let mut stream = Stream::new();
        stream.buffer(&[0xff, 0xff]);
        let mut ptr = BitPtr::new(0);
        ptr.read(&stream.buffer, 3).unwrap();
        assert_eq!(bits_remaining(ptr, 1), 5);
        let before = ptr;
        assert_eq!(read_bits(&mut stream, &mut ptr, 6, 1), Err(Error::LostSync));
        assert_eq!(ptr, before);
        assert!(!stream.sync);
    }
}
