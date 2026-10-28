//! Implementations of the shared Milestone 0 scheduler contracts.

use core::ffi::c_void;

use ogc_runtime::{
    Deadline, DeferredExecutor, InterruptWake, MonotonicClock, TimerService, WakeToken, WiiError,
};

use crate::runtime;

/// Wii timebase clock used by LWP and device-service deadlines.
#[derive(Copy, Clone, Debug, Default)]
pub struct LwpClock;

impl MonotonicClock for LwpClock {
    fn now(&self) -> u64 {
        runtime::now_ticks()
    }

    fn ticks_per_second(&self) -> u64 {
        runtime::TICKS_PER_SECOND
    }
}

/// Adapter from the shared deferred-executor contract to the LWP worker.
#[derive(Copy, Clone, Debug, Default)]
pub struct LwpDeferredExecutor;

impl DeferredExecutor for LwpDeferredExecutor {
    fn enqueue(
        &self,
        callback: ogc_runtime::DeferredCallback,
        argument: usize,
    ) -> Result<(), WiiError> {
        // Both functions use the Rust ABI and one pointer-sized argument. The
        // driver stores the erased callback until its dedicated worker runs.
        let callback = unsafe {
            core::mem::transmute::<ogc_runtime::DeferredCallback, unsafe fn(*mut c_void)>(callback)
        };
        // SAFETY: the shared contract requires `argument` to remain valid for
        // the callback invocation.
        match unsafe { runtime::deferred_enqueue(Some(callback), argument as *mut c_void) } {
            runtime::LWP_SUCCESSFUL => Ok(()),
            _ => Err(WiiError::ResourceExhausted),
        }
    }

    fn wake(&self, token: WakeToken) -> Result<(), WiiError> {
        if token.0 == 0 || token.0 == runtime::LWP_TQUEUE_NULL {
            return Err(WiiError::InvalidArgument);
        }
        runtime::queue_signal(token.0);
        Ok(())
    }
}

/// Interrupt/IOS-completion wake adapter. Queue signalling only mutates bounded
/// scheduler state; the selected thread runs at the next safe scheduling point.
#[derive(Copy, Clone, Debug, Default)]
pub struct LwpInterruptWake;

impl InterruptWake for LwpInterruptWake {
    fn wake_from_interrupt(&self, token: WakeToken) -> Result<(), WiiError> {
        if token.0 == 0 || token.0 == runtime::LWP_TQUEUE_NULL {
            return Err(WiiError::InvalidArgument);
        }
        runtime::queue_signal(token.0);
        Ok(())
    }
}

/// Deadline adapter that wakes an LWP queue represented by `WakeToken`.
#[derive(Copy, Clone, Debug, Default)]
pub struct LwpTimerService;

unsafe fn timer_wake(argument: *mut c_void) {
    runtime::queue_signal(argument as usize as u32);
}

impl TimerService for LwpTimerService {
    fn register(&mut self, deadline: Deadline, token: WakeToken) -> Result<(), WiiError> {
        if token.0 == 0 || token.0 == runtime::LWP_TQUEUE_NULL {
            return Err(WiiError::InvalidArgument);
        }
        let delay = deadline.0.saturating_sub(runtime::now_ticks());
        // SAFETY: `timer_wake` interprets the argument only as the copied queue
        // handle value, not as a dereferenceable pointer.
        match unsafe {
            runtime::timer_start(
                token.0 as usize,
                Some(timer_wake),
                token.0 as usize as *mut c_void,
                delay,
                0,
            )
        } {
            runtime::LWP_SUCCESSFUL => Ok(()),
            _ => Err(WiiError::ResourceExhausted),
        }
    }

    fn cancel(&mut self, token: WakeToken) -> Result<(), WiiError> {
        runtime::timer_cancel(token.0 as usize);
        Ok(())
    }
}
