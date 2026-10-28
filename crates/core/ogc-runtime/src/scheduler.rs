//! Scheduler-facing interfaces required by the Milestone 1 LWP runtime.

use core::num::NonZeroU32;

use crate::WiiError;

/// Stable identifier assigned by the authoritative LWP scheduler.
#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct ThreadId(NonZeroU32);

impl ThreadId {
    /// Creates an identifier when `raw` is not libogc's null-thread sentinel.
    ///
    /// `NonZeroU32` already excludes zero. C also reserves `u32::MAX` as
    /// `LWP_THREAD_NULL`, so that value cannot identify a live thread.
    pub const fn new(raw: NonZeroU32) -> Option<Self> {
        if raw.get() == u32::MAX {
            None
        } else {
            Some(Self(raw))
        }
    }

    pub const fn get(self) -> u32 {
        self.0.get()
    }
}

/// Monotonic absolute Wii timebase deadline.
#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct Deadline(pub u64);

/// Result of a scheduler-integrated wait.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum WaitOutcome {
    Signalled,
    TimedOut,
    Cancelled,
}

/// Opaque wake token safe to enqueue from an interrupt or IOS completion path.
#[derive(Copy, Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct WakeToken(pub u32);

/// Context in which a public callback is delivered.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum CallbackContext {
    Interrupt,
    IosCompletion,
    DeferredWorker,
    Application,
}

/// Monotonic time source shared by scheduler and device timeouts.
pub trait MonotonicClock {
    fn now(&self) -> u64;
    fn ticks_per_second(&self) -> u64;
}

/// Shared deadline service. Implementations must not busy-wait.
pub trait TimerService {
    fn register(&mut self, deadline: Deadline, token: WakeToken) -> Result<(), WiiError>;
    fn cancel(&mut self, token: WakeToken) -> Result<(), WiiError>;
}

/// Type-erased deferred callback accepted by the scheduler worker.
pub type DeferredCallback = fn(usize);

/// Executor used to leave interrupt and IOS completion contexts safely.
pub trait DeferredExecutor {
    fn enqueue(&self, callback: DeferredCallback, argument: usize) -> Result<(), WiiError>;
    fn wake(&self, token: WakeToken) -> Result<(), WiiError>;
}

/// Minimal wake surface callable from a hardware interrupt or IOS completion
/// path. Implementations may update scheduler state but must defer callback
/// execution and must not allocate or block.
pub trait InterruptWake {
    fn wake_from_interrupt(&self, token: WakeToken) -> Result<(), WiiError>;
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::ThreadId;
    use core::num::NonZeroU32;

    #[test]
    fn thread_id_rejects_the_c_null_sentinel() {
        assert_eq!(ThreadId::new(NonZeroU32::new(u32::MAX).unwrap()), None);
        assert_eq!(ThreadId::new(NonZeroU32::new(1).unwrap()).unwrap().get(), 1);
        assert!(NonZeroU32::new(0).is_none());
    }
}
