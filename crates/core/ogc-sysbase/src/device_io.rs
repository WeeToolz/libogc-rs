//! Checked devoptab results. Callback failures use errno, not IOS result codes.
use crate::error::{EINVAL, EIO};
use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};

static CLOSE_FAILURES: AtomicU32 = AtomicU32::new(0);
static LAST_CLOSE_ERRNO: AtomicI32 = AtomicI32::new(0);

pub fn device_close_failures() -> u32 {
    CLOSE_FAILURES.load(Ordering::Acquire)
}

/// Positive Newlib errno from the latest failed device close.
pub fn last_device_close_errno() -> i32 {
    LAST_CLOSE_ERRNO.load(Ordering::Acquire)
}

pub(crate) fn record_close_failure(error: i32) {
    LAST_CLOSE_ERRNO.store(error.checked_neg().unwrap_or(EIO), Ordering::Release);
    CLOSE_FAILURES.fetch_add(1, Ordering::AcqRel);
}

pub(crate) fn callback_failure(errno: impl FnOnce() -> i32) -> i32 {
    let code = errno();
    -if code > 0 { code } else { EIO }
}

pub(crate) fn count(
    requested: usize,
    operation: impl FnOnce() -> isize,
    errno: impl FnOnce() -> i32,
) -> Result<usize, i32> {
    if requested > isize::MAX as usize {
        return Err(-EINVAL);
    }
    let result = operation();
    if result < 0 {
        Err(callback_failure(errno))
    } else if result as usize > requested {
        Err(-EIO)
    } else {
        Ok(result as usize)
    }
}

pub(crate) fn status(
    operation: impl FnOnce() -> i32,
    errno: impl FnOnce() -> i32,
) -> Result<(), i32> {
    match operation() {
        0 => Ok(()),
        value if value < 0 => Err(callback_failure(errno)),
        _ => Err(-EIO),
    }
}

pub(crate) fn seek(
    operation: impl FnOnce() -> i64,
    errno: impl FnOnce() -> i32,
) -> Result<i64, i32> {
    let result = operation();
    if result < 0 {
        Err(callback_failure(errno))
    } else {
        Ok(result)
    }
}

pub(crate) fn write_all(
    mut bytes: &[u8],
    mut write: impl FnMut(&[u8]) -> Result<usize, i32>,
) -> Result<(), i32> {
    while !bytes.is_empty() {
        let count = write(bytes)?;
        if count == 0 || count > bytes.len() {
            return Err(-EIO);
        }
        bytes = &bytes[count..];
    }
    Ok(())
}

/// Release local storage after one close attempt, including a failed attempt.
/// Backend state can be uncertain on failure; do not retry an invalidated handle.
pub(crate) fn close(
    operation: impl FnOnce() -> Result<(), i32>,
    release: impl FnOnce(),
) -> Result<(), i32> {
    let result = operation();
    release();
    result
}
