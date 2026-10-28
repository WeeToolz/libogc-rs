//! Checked partial writes for the real report socket transport.

/// WIR2 bit 0 marks truncation; bits 1 and 2 mark a streaming checkpoint.
pub fn flags(checkpoint: bool, truncated: bool) -> u16 {
    u16::from(truncated) | if checkpoint { 6 } else { 0 }
}

#[derive(Debug, PartialEq, Eq)]
pub enum WriteError {
    Deadline,
    NoProgress,
    InvalidCount,
    Operation(i32),
}

/// Retry transient IOS network-startup timeouts without hiding other errors.
/// `-110` is IOS/newlib ETIMEDOUT. The operation must clean up each failed
/// attempt before it returns so the next attempt can use a fresh socket.
pub fn retry_ios_timeout<T>(
    mut operation: impl FnMut() -> Result<T, i32>,
    mut wait: impl FnMut(),
) -> Result<T, i32> {
    const MAX_ATTEMPTS: usize = 3;
    const IOS_ETIMEDOUT: i32 = -110;

    let mut attempt = 1;
    loop {
        match operation() {
            Ok(value) => return Ok(value),
            Err(error) if error == IOS_ETIMEDOUT && attempt < MAX_ATTEMPTS => {
                attempt += 1;
                wait();
            }
            result => return result,
        }
    }
}

/// `None` means the nonblocking operation must be retried. A completed write
/// must report its actual byte count. The caller supplies one packet deadline.
pub fn write_all(
    mut bytes: &[u8],
    mut expired: impl FnMut() -> bool,
    mut write: impl FnMut(&[u8]) -> Result<Option<usize>, i32>,
) -> Result<(), WriteError> {
    while !bytes.is_empty() {
        if expired() {
            return Err(WriteError::Deadline);
        }
        let chunk = &bytes[..bytes.len().min(16 * 1024)];
        match write(chunk).map_err(WriteError::Operation)? {
            None => continue,
            Some(0) => return Err(WriteError::NoProgress),
            Some(count) if count > chunk.len() => return Err(WriteError::InvalidCount),
            Some(count) => bytes = &bytes[count..],
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wire_flags_distinguish_streaming_pages_from_final_reports() {
        assert_eq!(flags(true, false).to_be_bytes(), [0x00, 0x06]);
        assert_eq!(flags(true, true).to_be_bytes(), [0x00, 0x07]);
        assert_eq!(flags(false, false).to_be_bytes(), [0x00, 0x00]);
        assert_eq!(flags(false, true).to_be_bytes(), [0x00, 0x01]);
    }

    #[test]
    fn partial_writes_and_retries_preserve_exact_bytes() {
        let mut received = Vec::new();
        let mut calls = 0;
        let result = write_all(
            b"WIR2\x00\x02payload",
            || false,
            |bytes| {
                calls += 1;
                if calls == 2 {
                    return Ok(None);
                }
                let count = bytes.len().min(3);
                received.extend_from_slice(&bytes[..count]);
                Ok(Some(count))
            },
        );
        assert_eq!(result, Ok(()));
        assert_eq!(received, b"WIR2\x00\x02payload");
    }

    #[test]
    fn operation_failure_after_partial_write_is_returned() {
        let mut received = Vec::new();
        let result = write_all(
            b"abcdef",
            || false,
            |bytes| {
                if !received.is_empty() {
                    return Err(-41);
                }
                received.extend_from_slice(&bytes[..2]);
                Ok(Some(2))
            },
        );
        assert_eq!(result, Err(WriteError::Operation(-41)));
        assert_eq!(received, b"ab");
    }

    #[test]
    fn retry_does_not_restart_the_original_deadline() {
        let mut checks = 0;
        let mut writes = 0;
        let result = write_all(
            b"data",
            || {
                checks += 1;
                checks == 4
            },
            |_| {
                writes += 1;
                Ok(None)
            },
        );
        assert_eq!(result, Err(WriteError::Deadline));
        assert_eq!(writes, 3);
    }

    #[test]
    fn zero_and_oversized_completion_counts_are_errors() {
        assert_eq!(
            write_all(b"data", || false, |_| Ok(Some(0))),
            Err(WriteError::NoProgress)
        );
        assert_eq!(
            write_all(b"data", || false, |_| Ok(Some(5))),
            Err(WriteError::InvalidCount)
        );
    }

    #[test]
    fn empty_payload_does_not_touch_the_socket() {
        assert_eq!(
            write_all(
                b"",
                || panic!("deadline checked"),
                |_| panic!("socket used")
            ),
            Ok(())
        );
    }

    #[test]
    fn retries_ios_timeout_and_returns_the_first_success() {
        let mut attempts = 0;
        let mut waits = 0;
        let result: Result<i32, i32> = retry_ios_timeout(
            || {
                attempts += 1;
                if attempts < 3 { Err(-110) } else { Ok(42) }
            },
            || waits += 1,
        );
        assert_eq!(result, Ok(42));
        assert_eq!(attempts, 3);
        assert_eq!(waits, 2);
    }

    #[test]
    fn stops_after_three_ios_timeouts_and_preserves_the_error() {
        let mut attempts = 0;
        let mut waits = 0;
        let result: Result<(), i32> = retry_ios_timeout(
            || {
                attempts += 1;
                Err(-110)
            },
            || waits += 1,
        );
        assert_eq!(result, Err(-110));
        assert_eq!(attempts, 3);
        assert_eq!(waits, 2);
    }

    #[test]
    fn does_not_retry_other_network_errors() {
        let mut attempts = 0;
        let mut waits = 0;
        let result: Result<(), i32> = retry_ios_timeout(
            || {
                attempts += 1;
                Err(-63)
            },
            || waits += 1,
        );
        assert_eq!(result, Err(-63));
        assert_eq!(attempts, 1);
        assert_eq!(waits, 0);
    }
}
