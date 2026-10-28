//! Cleanup diagnostics for network initialization's IOS device handles.
use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};
static FAILURES: AtomicU32 = AtomicU32::new(0);
static LAST_ERROR: AtomicI32 = AtomicI32::new(0);

pub fn initialization_close_failures() -> u32 {
    FAILURES.load(Ordering::Acquire)
}
/// Original negative IPC code from the latest failed initialization close.
pub fn last_initialization_close_error() -> i32 {
    LAST_ERROR.load(Ordering::Acquire)
}
fn record(error: i32) {
    LAST_ERROR.store(error, Ordering::Release);
    FAILURES.fetch_add(1, Ordering::AcqRel);
}

/// Keep the first operation failure and record any additional close failure.
pub(crate) fn finish(operation: i32, close: Result<(), i32>) -> i32 {
    match close {
        Ok(()) => operation,
        Err(error) => {
            record(error);
            if operation < 0 { operation } else { error }
        }
    }
}

/// Used only on an already-failed or canceled initialization path.
pub(crate) fn after_failure(close: Result<(), i32>) {
    if let Err(error) = close {
        record(error);
    }
}
