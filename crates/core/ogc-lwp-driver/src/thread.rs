//! Stack and name rules used by the std thread provider.

pub const STD_MIN_STACK_SIZE: usize = 64 * 1024;
pub const THREAD_NAME_CAPACITY: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ThreadError {
    ExceptionContext,
    InterruptsDisabled,
    InvalidThread,
    InvalidStack,
    InvalidTimeout,
    MissingEntry,
    ResourceExhausted,
    InvalidName,
    BufferTooSmall,
    UnknownStack,
    CleanupComplete,
    CleanupCallbackConflict,
    TlsCleanup,
    Cancelled,
}

/// Reserve room for the driver's alignment slack and reject pointer extents
/// larger than isize::MAX before the scheduler allocates or publishes a thread.
pub(crate) fn stack_size(requested: usize) -> Result<u32, ThreadError> {
    // Keep the bottom guard and initial ABI frame outside the requested size.
    let size = requested
        .max(STD_MIN_STACK_SIZE)
        .checked_add(64)
        .and_then(|size| size.checked_add(31))
        .ok_or(ThreadError::InvalidStack)?
        & !31;
    if size
        .checked_add(31)
        .is_none_or(|size| size > isize::MAX as usize)
    {
        return Err(ThreadError::InvalidStack);
    }
    u32::try_from(size).map_err(|_| ThreadError::InvalidStack)
}

#[derive(Clone, Copy)]
pub(crate) struct ThreadName {
    bytes: [u8; THREAD_NAME_CAPACITY],
    len: usize,
}

impl ThreadName {
    pub(crate) const fn new() -> Self {
        Self {
            bytes: [0; THREAD_NAME_CAPACITY],
            len: 0,
        }
    }

    pub(crate) fn set(&mut self, name: &str) -> Result<(), ThreadError> {
        if name.as_bytes().contains(&0) {
            return Err(ThreadError::InvalidName);
        }
        let mut len = name.len().min(THREAD_NAME_CAPACITY - 1);
        while !name.is_char_boundary(len) {
            len -= 1;
        }
        self.bytes.fill(0);
        self.bytes[..len].copy_from_slice(&name.as_bytes()[..len]);
        self.len = len;
        Ok(())
    }

    pub(crate) fn copy_to(&self, output: &mut [u8]) -> Result<usize, ThreadError> {
        if output.len() < self.len {
            return Err(ThreadError::BufferTooSmall);
        }
        output[..self.len].copy_from_slice(&self.bytes[..self.len]);
        Ok(self.len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_sizes_are_bounded_and_aligned() {
        assert_eq!(stack_size(0), Ok(65_600));
        assert_eq!(stack_size(65_537), Ok(65_632));
        assert_eq!(stack_size(131_072), Ok(131_136));
        assert_eq!(stack_size(usize::MAX), Err(ThreadError::InvalidStack));
        assert_eq!(
            stack_size(isize::MAX as usize),
            Err(ThreadError::InvalidStack)
        );
        assert_eq!(
            stack_size(u32::MAX as usize),
            Err(ThreadError::InvalidStack)
        );
    }

    #[test]
    fn invalid_names_preserve_existing_metadata() {
        let mut name = ThreadName::new();
        name.set("worker").unwrap();
        assert_eq!(name.set("bad\0name"), Err(ThreadError::InvalidName));
        let mut output = [0xa5; 16];
        assert_eq!(name.copy_to(&mut output), Ok(6));
        assert_eq!(&output[..6], b"worker");
        assert_eq!(output[6], 0xa5);
        let mut short = [0xa5; 2];
        assert_eq!(name.copy_to(&mut short), Err(ThreadError::BufferTooSmall));
        assert_eq!(short, [0xa5; 2]);
    }

    #[test]
    fn truncation_keeps_utf8_and_replacing_a_name_clears_old_bytes() {
        let mut name = ThreadName::new();
        name.set("abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghijé")
            .unwrap();
        let mut output = [0; 64];
        assert_eq!(name.copy_to(&mut output), Ok(62));
        assert_eq!(
            &output[..62],
            b"abcdefghijklmnopqrstuvwxyzabcdefghijklmnopqrstuvwxyzabcdefghij"
        );
        assert!(core::str::from_utf8(&output[..62]).is_ok());
        name.set("x").unwrap();
        assert_eq!(name.bytes[1..], [0; 63]);
    }
}
