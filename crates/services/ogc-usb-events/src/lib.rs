#![no_std]

//! Fixed-capacity lifecycle state for Wii USB v5 host events.

use core::ffi::c_void;

pub const MAX_DEVICES: usize = 32;
pub type EventCallback = unsafe extern "C" fn(result: i32, userdata: *mut c_void) -> i32;

#[derive(Clone, Copy)]
pub struct PendingCallback {
    pub callback: EventCallback,
    pub userdata: *mut c_void,
}

#[derive(Clone, Copy)]
struct RemovalRegistration {
    device_id: i32,
    pending: PendingCallback,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RegistrationError {
    MissingCallback,
    HostUnavailable(i32),
    UnknownDevice,
    DuplicateDevice,
    Full,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EventError {
    StaleGeneration,
    TooManyDevices,
}

pub struct CallbackBatch {
    entries: [Option<PendingCallback>; MAX_DEVICES],
    len: usize,
}

impl CallbackBatch {
    pub const fn new() -> Self {
        Self {
            entries: [None; MAX_DEVICES],
            len: 0,
        }
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = PendingCallback> + '_ {
        self.entries[..self.len].iter().flatten().copied()
    }

    fn clear(&mut self) {
        self.entries = [None; MAX_DEVICES];
        self.len = 0;
    }

    fn push(&mut self, callback: PendingCallback) {
        debug_assert!(self.len < MAX_DEVICES);
        self.entries[self.len] = Some(callback);
        self.len += 1;
    }
}

impl Default for CallbackBatch {
    fn default() -> Self {
        Self::new()
    }
}

pub struct EventState {
    generation: u32,
    status: i32,
    active: bool,
    snapshot_ready: bool,
    device_ids: [i32; MAX_DEVICES],
    device_count: usize,
    removals: [Option<RemovalRegistration>; MAX_DEVICES],
    changes: [Option<PendingCallback>; MAX_DEVICES],
}

impl EventState {
    pub const fn new() -> Self {
        Self {
            generation: 0,
            status: 0,
            active: false,
            snapshot_ready: false,
            device_ids: [0; MAX_DEVICES],
            device_count: 0,
            removals: [None; MAX_DEVICES],
            changes: [None; MAX_DEVICES],
        }
    }

    pub fn start(&mut self, generation: u32) {
        self.generation = generation;
        self.status = 0;
        self.active = true;
        self.snapshot_ready = false;
        self.device_ids = [0; MAX_DEVICES];
        self.device_count = 0;
        self.removals = [None; MAX_DEVICES];
        self.changes = [None; MAX_DEVICES];
    }

    pub fn generation(&self) -> u32 {
        self.generation
    }

    pub fn is_ready(&self) -> bool {
        self.active && self.snapshot_ready
    }

    pub fn unavailable_status(&self) -> Option<i32> {
        (!self.is_ready()).then_some(self.status)
    }

    pub fn contains(&self, device_id: i32) -> bool {
        self.is_ready() && self.device_ids[..self.device_count].contains(&device_id)
    }

    pub fn snapshot(&self) -> &[i32] {
        &self.device_ids[..self.device_count]
    }

    pub fn register_removal(
        &mut self,
        device_id: i32,
        callback: Option<EventCallback>,
        userdata: *mut c_void,
    ) -> Result<(), RegistrationError> {
        let callback = callback.ok_or(RegistrationError::MissingCallback)?;
        self.require_ready()?;
        if !self.device_ids[..self.device_count].contains(&device_id) {
            return Err(RegistrationError::UnknownDevice);
        }
        if self
            .removals
            .iter()
            .flatten()
            .any(|entry| entry.device_id == device_id)
        {
            return Err(RegistrationError::DuplicateDevice);
        }
        let slot = self
            .removals
            .iter_mut()
            .find(|entry| entry.is_none())
            .ok_or(RegistrationError::Full)?;
        *slot = Some(RemovalRegistration {
            device_id,
            pending: PendingCallback { callback, userdata },
        });
        Ok(())
    }

    pub fn register_change(
        &mut self,
        callback: Option<EventCallback>,
        userdata: *mut c_void,
    ) -> Result<(), RegistrationError> {
        let callback = callback.ok_or(RegistrationError::MissingCallback)?;
        self.require_ready()?;
        let slot = self
            .changes
            .iter_mut()
            .find(|entry| entry.is_none())
            .ok_or(RegistrationError::Full)?;
        *slot = Some(PendingCallback { callback, userdata });
        Ok(())
    }

    pub fn apply_snapshot(
        &mut self,
        generation: u32,
        device_ids: &[i32],
        removed: &mut CallbackBatch,
    ) -> Result<(), EventError> {
        removed.clear();
        self.require_generation(generation)?;
        if device_ids.len() > MAX_DEVICES {
            return Err(EventError::TooManyDevices);
        }

        for slot in &mut self.removals {
            let Some(registration) = *slot else {
                continue;
            };
            if !device_ids.contains(&registration.device_id) {
                removed.push(registration.pending);
                *slot = None;
            }
        }

        self.device_ids = [0; MAX_DEVICES];
        self.device_ids[..device_ids.len()].copy_from_slice(device_ids);
        self.device_count = device_ids.len();
        self.snapshot_ready = true;
        Ok(())
    }

    pub fn finish_change(
        &mut self,
        generation: u32,
        callbacks: &mut CallbackBatch,
    ) -> Result<(), EventError> {
        callbacks.clear();
        self.require_generation(generation)?;
        drain_reverse(&mut self.changes, callbacks);
        Ok(())
    }

    pub fn close_device(&mut self, device_id: i32, callbacks: &mut CallbackBatch) {
        callbacks.clear();
        for slot in &mut self.removals {
            if slot.is_some_and(|entry| entry.device_id == device_id) {
                callbacks.push(slot.take().unwrap().pending);
                break;
            }
        }
    }

    pub fn fail(
        &mut self,
        generation: u32,
        error: i32,
        removals: &mut CallbackBatch,
        changes: &mut CallbackBatch,
    ) -> Result<(), EventError> {
        self.require_generation(generation)?;
        self.drain_all(error, removals, changes);
        Ok(())
    }

    pub fn stop(&mut self, error: i32, removals: &mut CallbackBatch, changes: &mut CallbackBatch) {
        self.drain_all(error, removals, changes);
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn stop_silent(&mut self, error: i32) {
        self.active = false;
        self.snapshot_ready = false;
        self.status = error;
        self.device_ids = [0; MAX_DEVICES];
        self.device_count = 0;
        self.removals = [None; MAX_DEVICES];
        self.changes = [None; MAX_DEVICES];
        self.generation = self.generation.wrapping_add(1);
    }

    fn drain_all(&mut self, error: i32, removals: &mut CallbackBatch, changes: &mut CallbackBatch) {
        self.active = false;
        self.snapshot_ready = false;
        self.status = error;
        self.device_ids = [0; MAX_DEVICES];
        self.device_count = 0;
        removals.clear();
        for slot in &mut self.removals {
            if let Some(registration) = slot.take() {
                removals.push(registration.pending);
            }
        }
        changes.clear();
        drain(&mut self.changes, changes);
    }

    fn require_ready(&self) -> Result<(), RegistrationError> {
        if self.is_ready() {
            Ok(())
        } else {
            Err(RegistrationError::HostUnavailable(self.status))
        }
    }

    fn require_generation(&self, generation: u32) -> Result<(), EventError> {
        if self.active && generation == self.generation {
            Ok(())
        } else {
            Err(EventError::StaleGeneration)
        }
    }
}

impl Default for EventState {
    fn default() -> Self {
        Self::new()
    }
}

fn drain_reverse(
    registrations: &mut [Option<PendingCallback>; MAX_DEVICES],
    callbacks: &mut CallbackBatch,
) {
    for registration in registrations.iter_mut().rev() {
        if let Some(callback) = registration.take() {
            callbacks.push(callback);
        }
    }
}

fn drain(
    registrations: &mut [Option<PendingCallback>; MAX_DEVICES],
    callbacks: &mut CallbackBatch,
) {
    for registration in registrations {
        if let Some(callback) = registration.take() {
            callbacks.push(callback);
        }
    }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use core::sync::atomic::{AtomicI32, AtomicUsize, Ordering};

    static RESULT: AtomicI32 = AtomicI32::new(i32::MIN);
    static USERDATA: AtomicUsize = AtomicUsize::new(0);
    static CALLS: AtomicUsize = AtomicUsize::new(0);

    unsafe extern "C" fn record(result: i32, userdata: *mut c_void) -> i32 {
        RESULT.store(result, Ordering::Release);
        USERDATA.store(userdata as usize, Ordering::Release);
        CALLS.fetch_add(1, Ordering::AcqRel);
        0
    }

    fn ready_state() -> EventState {
        let mut state = EventState::new();
        state.start(7);
        state
            .apply_snapshot(7, &[10, 20], &mut CallbackBatch::new())
            .unwrap();
        state
    }

    fn invoke(batch: &CallbackBatch, result: i32) {
        for pending in batch.iter() {
            unsafe {
                (pending.callback)(result, pending.userdata);
            }
        }
    }

    #[test]
    fn removal_registration_keeps_callback_and_userdata_until_device_leaves() {
        let mut state = ready_state();
        let userdata = 0x1234usize as *mut c_void;
        CALLS.store(0, Ordering::Release);
        state.register_removal(10, Some(record), userdata).unwrap();
        assert_eq!(CALLS.load(Ordering::Acquire), 0);

        let mut callbacks = CallbackBatch::new();
        state.apply_snapshot(7, &[10, 30], &mut callbacks).unwrap();
        assert!(callbacks.is_empty());
        state.apply_snapshot(7, &[30], &mut callbacks).unwrap();
        assert_eq!(callbacks.len(), 1);
        invoke(&callbacks, 0);
        assert_eq!(CALLS.load(Ordering::Acquire), 1);
        assert_eq!(RESULT.load(Ordering::Acquire), 0);
        assert_eq!(USERDATA.load(Ordering::Acquire), userdata as usize);

        state.apply_snapshot(7, &[], &mut callbacks).unwrap();
        assert!(callbacks.is_empty());
    }

    #[test]
    fn invalid_removal_registrations_fail() {
        let mut state = ready_state();
        assert_eq!(
            state.register_removal(10, None, core::ptr::null_mut()),
            Err(RegistrationError::MissingCallback)
        );
        assert_eq!(
            state.register_removal(99, Some(record), core::ptr::null_mut()),
            Err(RegistrationError::UnknownDevice)
        );
        state
            .register_removal(10, Some(record), core::ptr::null_mut())
            .unwrap();
        assert_eq!(
            state.register_removal(10, Some(record), core::ptr::null_mut()),
            Err(RegistrationError::DuplicateDevice)
        );
    }

    #[test]
    fn full_registries_report_capacity_errors() {
        let mut state = EventState::new();
        state.start(1);
        let ids = core::array::from_fn::<_, MAX_DEVICES, _>(|index| index as i32 + 1);
        state
            .apply_snapshot(1, &ids, &mut CallbackBatch::new())
            .unwrap();
        for id in ids {
            state
                .register_removal(id, Some(record), core::ptr::null_mut())
                .unwrap();
            state
                .register_change(Some(record), core::ptr::null_mut())
                .unwrap();
        }
        assert_eq!(
            state.register_change(Some(record), core::ptr::null_mut()),
            Err(RegistrationError::Full)
        );
    }

    #[test]
    fn change_callbacks_are_one_shot_and_can_register_again() {
        let mut state = ready_state();
        state
            .register_change(Some(record), 1usize as *mut c_void)
            .unwrap();
        let mut callbacks = CallbackBatch::new();
        state.finish_change(7, &mut callbacks).unwrap();
        assert_eq!(callbacks.len(), 1);
        state
            .register_change(Some(record), 2usize as *mut c_void)
            .unwrap();
        state.finish_change(7, &mut callbacks).unwrap();
        assert_eq!(callbacks.len(), 1);
        assert_eq!(callbacks.iter().next().unwrap().userdata as usize, 2);
    }

    #[test]
    fn close_failure_and_stop_complete_each_registration_once() {
        let mut state = ready_state();
        state
            .register_removal(10, Some(record), 10usize as *mut c_void)
            .unwrap();
        state
            .register_removal(20, Some(record), 20usize as *mut c_void)
            .unwrap();
        state
            .register_change(Some(record), 30usize as *mut c_void)
            .unwrap();

        let mut removals = CallbackBatch::new();
        state.close_device(10, &mut removals);
        assert_eq!(removals.len(), 1);
        state.close_device(10, &mut removals);
        assert!(removals.is_empty());

        let mut changes = CallbackBatch::new();
        state.fail(7, -55, &mut removals, &mut changes).unwrap();
        assert_eq!(removals.len(), 1);
        assert_eq!(changes.len(), 1);
        assert_eq!(
            state.register_removal(20, Some(record), core::ptr::null_mut()),
            Err(RegistrationError::HostUnavailable(-55))
        );

        state.start(8);
        state
            .apply_snapshot(8, &[40], &mut CallbackBatch::new())
            .unwrap();
        state
            .register_removal(40, Some(record), core::ptr::null_mut())
            .unwrap();
        state.stop(-6, &mut removals, &mut changes);
        assert_eq!(removals.len(), 1);
        assert!(!state.is_ready());
    }

    #[test]
    fn change_callbacks_run_in_reverse_registration_order() {
        let mut state = ready_state();
        state
            .register_change(Some(record), 1usize as *mut c_void)
            .unwrap();
        state
            .register_change(Some(record), 2usize as *mut c_void)
            .unwrap();
        let mut callbacks = CallbackBatch::new();
        state.finish_change(7, &mut callbacks).unwrap();
        let order: std::vec::Vec<_> = callbacks
            .iter()
            .map(|pending| pending.userdata as usize)
            .collect();
        assert_eq!(order, std::vec![2, 1]);
    }

    #[test]
    fn silent_stop_discards_registered_callbacks() {
        let mut state = ready_state();
        state
            .register_removal(10, Some(record), 1usize as *mut c_void)
            .unwrap();
        state
            .register_change(Some(record), 2usize as *mut c_void)
            .unwrap();
        let generation = state.generation();
        state.stop_silent(-6);
        assert_eq!(state.generation(), generation.wrapping_add(1));
        assert!(!state.is_ready());
        let mut callbacks = CallbackBatch::new();
        assert_eq!(
            state.finish_change(generation, &mut callbacks),
            Err(EventError::StaleGeneration)
        );
        assert!(callbacks.is_empty());
        assert_eq!(state.unavailable_status(), Some(-6));
        assert!(state.snapshot().is_empty());
        state.start(generation.wrapping_add(1));
        state
            .apply_snapshot(generation.wrapping_add(1), &[], &mut callbacks)
            .unwrap();
        assert!(callbacks.is_empty());
        let mut changes = CallbackBatch::new();
        state
            .finish_change(generation.wrapping_add(1), &mut changes)
            .unwrap();
        assert!(changes.is_empty());
    }

    #[test]
    fn stale_and_oversized_events_do_not_change_current_state() {
        let mut state = ready_state();
        let mut callbacks = CallbackBatch::new();
        assert_eq!(
            state.apply_snapshot(6, &[99], &mut callbacks),
            Err(EventError::StaleGeneration)
        );
        assert!(state.contains(10));
        let oversized = [1; MAX_DEVICES + 1];
        assert_eq!(
            state.apply_snapshot(7, &oversized, &mut callbacks),
            Err(EventError::TooManyDevices)
        );
        assert!(state.contains(10));
    }

    #[test]
    fn stale_events_preserve_callbacks_for_current_generation() {
        let mut state = ready_state();
        state
            .register_removal(10, Some(record), 10usize as *mut c_void)
            .unwrap();
        state
            .register_change(Some(record), 20usize as *mut c_void)
            .unwrap();
        let mut removals = CallbackBatch::new();
        let mut changes = CallbackBatch::new();

        assert_eq!(
            state.apply_snapshot(6, &[20], &mut removals),
            Err(EventError::StaleGeneration)
        );
        assert_eq!(
            state.apply_snapshot(7, &[99; MAX_DEVICES + 1], &mut removals),
            Err(EventError::TooManyDevices)
        );
        assert_eq!(
            state.finish_change(6, &mut changes),
            Err(EventError::StaleGeneration)
        );
        assert_eq!(
            state.fail(6, -55, &mut removals, &mut changes),
            Err(EventError::StaleGeneration)
        );
        assert_eq!(state.snapshot(), &[10, 20]);
        assert!(state.is_ready());
        assert!(removals.is_empty());
        assert!(changes.is_empty());

        state.apply_snapshot(7, &[20], &mut removals).unwrap();
        assert_eq!(removals.len(), 1);
        assert_eq!(removals.iter().next().unwrap().userdata as usize, 10);
        state.finish_change(7, &mut changes).unwrap();
        assert_eq!(changes.len(), 1);
        assert_eq!(changes.iter().next().unwrap().userdata as usize, 20);
        state.apply_snapshot(7, &[], &mut removals).unwrap();
        state.finish_change(7, &mut changes).unwrap();
        assert!(removals.is_empty());
        assert!(changes.is_empty());
    }

    #[test]
    fn state_accessors_report_current_lifecycle_and_devices() {
        let mut state = EventState::new();
        assert_eq!(state.generation(), 0);
        assert_eq!(state.unavailable_status(), Some(0));

        state.start(11);
        assert_eq!(state.generation(), 11);
        assert!(!state.is_ready());

        state
            .apply_snapshot(11, &[3, 5], &mut CallbackBatch::new())
            .unwrap();
        assert!(state.is_ready());
        assert_eq!(state.unavailable_status(), None);
        assert_eq!(state.snapshot(), &[3, 5]);
        assert!(state.contains(3));
        assert!(!state.contains(4));
    }
}
