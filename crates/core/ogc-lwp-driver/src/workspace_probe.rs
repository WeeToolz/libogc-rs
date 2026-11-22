use core::sync::atomic::{AtomicU8, AtomicUsize, Ordering};

pub const WORKSPACE_PROBE_ALLOC: u8 = 1;
pub const WORKSPACE_PROBE_FREE: u8 = 2;
pub const WORKSPACE_PROBE_CAPACITY: usize = 64;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct WorkspaceProbeEvent {
    pub kind: u8,
    pub pointer: usize,
    pub size: usize,
}

pub struct EventSlot {
    pub kind: AtomicU8,
    pub pointer: AtomicUsize,
    pub size: AtomicUsize,
}

impl EventSlot {
    const fn new() -> Self {
        Self {
            kind: AtomicU8::new(0),
            pointer: AtomicUsize::new(0),
            size: AtomicUsize::new(0),
        }
    }
}

pub static WORKSPACE_PROBE_EVENT_COUNT: AtomicUsize = AtomicUsize::new(0);
pub static WORKSPACE_PROBE_EVENTS: [EventSlot; WORKSPACE_PROBE_CAPACITY] =
    [const { EventSlot::new() }; WORKSPACE_PROBE_CAPACITY];

pub(super) fn record(kind: u8, pointer: *mut u8, size: usize) {
    let index = WORKSPACE_PROBE_EVENT_COUNT.fetch_add(1, Ordering::AcqRel);
    let Some(slot) = WORKSPACE_PROBE_EVENTS.get(index) else {
        return;
    };
    slot.pointer.store(pointer as usize, Ordering::Relaxed);
    slot.size.store(size, Ordering::Relaxed);
    slot.kind.store(kind, Ordering::Release);
}
