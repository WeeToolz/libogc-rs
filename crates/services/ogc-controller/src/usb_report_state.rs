//! Ownership handoff for a USB report buffer retained until IOS completion.
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};

const IDLE: u32 = 0;
const PENDING: u32 = 1;
const READY: u32 = 2;
const READING: u32 = 3;
const COMPLETING: u32 = 4;
const RELEASING: u32 = u32::MAX;

pub(crate) struct UsbReportState {
    phase: AtomicU32,
    result: AtomicI32,
}

impl UsbReportState {
    pub(crate) const fn new() -> Self {
        Self {
            phase: AtomicU32::new(IDLE),
            result: AtomicI32::new(0),
        }
    }

    pub(crate) fn is_idle(&self) -> bool {
        self.phase.load(Ordering::Acquire) == IDLE
    }

    /// Only an idle buffer can be submitted to IOS.
    pub(crate) fn start(&self) -> bool {
        self.phase
            .compare_exchange(IDLE, PENDING, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    }

    /// Publishes the result after IOS has finished writing the buffer.
    pub(crate) fn complete(&self, result: i32) {
        if self
            .phase
            .compare_exchange(PENDING, COMPLETING, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return;
        }
        self.result.store(result, Ordering::Relaxed);
        self.phase.store(READY, Ordering::Release);
    }

    /// Claims a completed buffer for copying. Call release after the copy.
    pub(crate) fn take_result(&self) -> Option<i32> {
        if self
            .phase
            .compare_exchange(READY, READING, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return None;
        }
        let result = self.result.load(Ordering::Relaxed);
        Some(result)
    }

    pub(crate) fn release(&self) {
        self.phase.store(IDLE, Ordering::Release);
    }
}

/// Coordinates the single IOS USB request owner across controller managers.
///
/// A manager keeps its token until every request and descriptor is retired.
pub(crate) struct UsbBusOwnership {
    owner: AtomicU32,
    retiring: AtomicU32,
    retirement_ready: AtomicU32,
}

impl UsbBusOwnership {
    pub(crate) const fn new() -> Self {
        Self {
            owner: AtomicU32::new(0),
            retiring: AtomicU32::new(0),
            retirement_ready: AtomicU32::new(0),
        }
    }

    pub(crate) fn try_acquire(&self, owner: u32) -> bool {
        owner != 0
            && owner != RELEASING
            && self
                .owner
                .compare_exchange(0, owner, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
    }

    pub(crate) fn owner(&self) -> u32 {
        self.owner.load(Ordering::Acquire)
    }

    pub(crate) fn begin_retirement(&self, owner: u32) -> bool {
        owner != 0
            && owner != RELEASING
            && self.owner() == owner
            && match self
                .retiring
                .compare_exchange(0, owner, Ordering::AcqRel, Ordering::Acquire)
            {
                Ok(_) => true,
                Err(current) => current == owner,
            }
    }

    pub(crate) fn is_retiring(&self) -> bool {
        self.retiring.load(Ordering::Acquire) != 0
    }

    /// Publishes that all descriptors have moved into static retirement slots.
    pub(crate) fn finish_retirement(&self, owner: u32) -> bool {
        owner != 0
            && owner != RELEASING
            && self.owner() == owner
            && self.retiring.load(Ordering::Acquire) == owner
            && self
                .retirement_ready
                .compare_exchange(0, owner, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
    }

    pub(crate) fn retirement_ready(&self) -> bool {
        let owner = self.retiring.load(Ordering::Acquire);
        owner != 0 && self.retirement_ready.load(Ordering::Acquire) == owner
    }

    pub(crate) fn release(&self, owner: u32) -> bool {
        if !self.begin_release(owner) {
            return false;
        }
        self.finish_release();
        true
    }

    /// Reserves the bus while the retirement markers are cleared.
    /// Acquisition stays blocked until `finish_release` publishes owner zero.
    fn begin_release(&self, owner: u32) -> bool {
        owner != 0
            && owner != RELEASING
            && self
                .owner
                .compare_exchange(owner, RELEASING, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
    }

    fn finish_release(&self) {
        self.retiring.store(0, Ordering::Release);
        self.retirement_ready.store(0, Ordering::Release);
        self.owner.store(0, Ordering::Release);
    }

    /// Releases ownership only after requests and descriptors have retired.
    pub(crate) fn release_when_drained(
        &self,
        owner: u32,
        requests_idle: bool,
        descriptors_closed: bool,
    ) -> bool {
        requests_idle && descriptors_closed && self.release(owner)
    }
}

/// Serializes main-thread workers that drain orphaned requests and descriptors.
pub(crate) struct UsbRetirementWorker(AtomicBool);

impl UsbRetirementWorker {
    pub(crate) const fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    pub(crate) fn try_lock(&self) -> Option<UsbRetirementWorkerGuard<'_>> {
        self.0
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .ok()
            .map(|_| UsbRetirementWorkerGuard(self))
    }
}

pub(crate) struct UsbRetirementWorkerGuard<'a>(&'a UsbRetirementWorker);

impl Drop for UsbRetirementWorkerGuard<'_> {
    fn drop(&mut self) {
        self.0.0.store(false, Ordering::Release);
    }
}

pub(crate) const fn usb_device_id_is_owned(fd: i32, tracked_id: i32, candidate_id: i32) -> bool {
    fd >= 0 && tracked_id == candidate_id
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delayed_report_does_not_block_or_allow_buffer_reuse() {
        let state = UsbReportState::new();
        assert!(state.is_idle());
        assert!(state.start());
        for _ in 0..120 {
            assert_eq!(state.take_result(), None);
            assert!(!state.start());
        }
        state.complete(8);
        assert!(!state.start());
        assert_eq!(state.take_result(), Some(8));
        assert_eq!(state.take_result(), None);
        assert!(!state.start());
        state.release();
        assert!(state.start());
    }

    #[test]
    fn immediate_completion_and_errors_are_consumed_once() {
        let state = UsbReportState::new();
        for result in [8, -6, 0] {
            assert!(state.start());
            state.complete(result);
            assert_eq!(state.take_result(), Some(result));
            assert_eq!(state.take_result(), None);
            state.release();
        }
    }

    #[test]
    fn completion_cannot_overwrite_idle_or_a_ready_result() {
        let state = UsbReportState::new();
        state.complete(-5);
        assert_eq!(state.take_result(), None);
        assert!(state.start());
        state.complete(8);
        state.complete(-5);
        assert_eq!(state.take_result(), Some(8));
        state.release();
        assert_eq!(state.take_result(), None);
    }

    #[test]
    fn bus_owner_is_exclusive_until_retirement_is_released() {
        let ownership = UsbBusOwnership::new();
        assert!(!ownership.try_acquire(0));
        assert!(ownership.try_acquire(11));
        assert!(!ownership.try_acquire(12));
        assert!(!ownership.begin_retirement(12));
        assert!(ownership.begin_retirement(11));
        assert!(ownership.is_retiring());
        assert!(!ownership.retirement_ready());
        assert!(!ownership.try_acquire(12));
        assert!(ownership.finish_retirement(11));
        assert!(ownership.retirement_ready());
        assert!(!ownership.release(12));
        assert!(!ownership.release_when_drained(11, false, true));
        assert!(!ownership.release_when_drained(11, true, false));
        assert_eq!(ownership.owner(), 11);
        assert!(ownership.release_when_drained(11, true, true));
        assert_eq!(ownership.owner(), 0);
        assert!(!ownership.is_retiring());
        assert!(ownership.try_acquire(12));
    }

    #[test]
    fn ownership_stays_reserved_while_retirement_markers_are_cleared() {
        let ownership = UsbBusOwnership::new();
        assert!(ownership.try_acquire(21));
        assert!(ownership.begin_retirement(21));
        assert!(ownership.finish_retirement(21));

        assert!(ownership.begin_release(21));
        assert_eq!(ownership.owner(), RELEASING);
        assert!(!ownership.try_acquire(22));
        assert!(ownership.is_retiring());

        ownership.finish_release();
        assert_eq!(ownership.owner(), 0);
        assert!(!ownership.is_retiring());
        assert!(!ownership.retirement_ready());
        assert!(ownership.try_acquire(22));
    }

    #[test]
    fn only_one_retirement_worker_can_progress_at_a_time() {
        let worker = UsbRetirementWorker::new();
        let first = worker.try_lock().expect("first worker owns guard");
        assert!(worker.try_lock().is_none());
        drop(first);
        assert!(worker.try_lock().is_some());
    }

    #[test]
    fn open_descriptor_keeps_device_id_owned_after_disconnect() {
        assert!(usb_device_id_is_owned(4, 77, 77));
        assert!(!usb_device_id_is_owned(-1, 77, 77));
        assert!(!usb_device_id_is_owned(4, 77, 78));
    }
}
