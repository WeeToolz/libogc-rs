//! Ownership for asynchronous adapter transfers. A timeout does not release IOS memory.
use crate::{Error, Result};
use alloc::sync::Arc;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicBool, AtomicI32, Ordering};

pub(crate) const TRANSFER_BYTES: usize = 1024;

#[repr(align(32))]
pub(crate) struct TransferBuffer(pub UnsafeCell<[u8; TRANSFER_BYTES]>);

pub(crate) struct TransferState {
    pub buffer: TransferBuffer,
    result: AtomicI32,
    complete: AtomicBool,
}

// The sole IOS request writes the buffer before complete(Release). Readers only
// access it after complete(Acquire). The buffer is never reused by another request.
unsafe impl Send for TransferState {}
unsafe impl Sync for TransferState {}

impl TransferState {
    #[cfg(any(target_arch = "powerpc", test))]
    pub fn new() -> Self {
        Self {
            buffer: TransferBuffer(UnsafeCell::new([0; TRANSFER_BYTES])),
            result: AtomicI32::new(0),
            complete: AtomicBool::new(false),
        }
    }
    #[cfg(any(target_arch = "powerpc", test))]
    pub fn complete(&self, result: i32) {
        self.result.store(result, Ordering::Relaxed);
        self.complete.store(true, Ordering::Release);
    }
    pub fn result(&self) -> Option<i32> {
        self.complete
            .load(Ordering::Acquire)
            .then(|| self.result.load(Ordering::Relaxed))
    }
}

/// Owns a result while IOS independently owns the transfer memory.
/// Dropping this handle does not cancel a pending request or release its memory.
#[derive(Clone)]
pub struct TransferRequest {
    pub(crate) state: Arc<TransferState>,
}
impl TransferRequest {
    pub fn raw_result(&self) -> Option<i32> {
        self.state.result()
    }
    /// Copies a completed input transfer. Pending transfers return None.
    /// Oversized IOS results are errors, not silently truncated packets.
    pub fn copy_to(&self, output: &mut [u8]) -> Option<Result<usize>> {
        let result = self.raw_result()?;
        if result < 0 {
            return Some(Err(Error::Ipc(result)));
        }
        let used = result as usize;
        if used > TRANSFER_BYTES || used > output.len() {
            return Some(Err(Error::BufferTooSmall));
        }
        // SAFETY: IOS completed and published all writes; no further writer exists.
        output[..used].copy_from_slice(unsafe { &(&*self.state.buffer.0.get())[..used] });
        Some(Ok(used))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn pending_and_immediate_completion_preserve_owned_data() {
        let state = Arc::new(TransferState::new());
        let request = TransferRequest {
            state: state.clone(),
        };
        let mut output = [0; 8];
        assert_eq!(request.copy_to(&mut output), None);
        unsafe {
            (&mut *state.buffer.0.get())[..3].copy_from_slice(&[1, 2, 3]);
        }
        state.complete(3);
        assert_eq!(request.copy_to(&mut output), Some(Ok(3)));
        assert_eq!(&output[..3], &[1, 2, 3]);
        assert_eq!(request.clone().copy_to(&mut output), Some(Ok(3)));
    }
    #[test]
    fn detached_timeout_keeps_ios_owner_alive() {
        let state = Arc::new(TransferState::new());
        let request = TransferRequest {
            state: state.clone(),
        };
        drop(request);
        assert_eq!(Arc::strong_count(&state), 1);
        state.complete(-7);
        assert_eq!(state.result(), Some(-7));
    }
    #[test]
    fn errors_and_oversized_results_are_not_reports() {
        for result in [-4, 2048] {
            let state = Arc::new(TransferState::new());
            state.complete(result);
            let request = TransferRequest { state };
            assert!(request.copy_to(&mut [0; 1024]).unwrap().is_err());
        }
    }
}
