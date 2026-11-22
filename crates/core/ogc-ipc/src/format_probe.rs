#![cfg(feature = "format-probe-hooks")]

use core::sync::atomic::{AtomicBool, AtomicU8, AtomicUsize, Ordering};

use super::{FormatCall, FormatState, Ioctlv};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocation_ledger_requires_one_matching_free_per_allocation() {
        let one = AllocationEvent {
            kind: ALLOCATION_EVENT_ALLOC,
            pointer: 0x1000,
            size: 64,
            role: 0,
        };
        let free = AllocationEvent {
            kind: ALLOCATION_EVENT_FREE,
            ..one
        };
        assert!(allocation_ledger_balanced(&[one, free]));
        assert!(!allocation_ledger_balanced(&[one, one, free]));
        assert!(!allocation_ledger_balanced(&[one, free, free]));
        assert!(!allocation_ledger_balanced(&[one]));
        assert!(!allocation_ledger_balanced(&[free]));
        assert!(allocation_ledger_balanced(&[]));
    }
}

pub const ALLOCATION_EVENT_ALLOC: u8 = 1;
pub const ALLOCATION_EVENT_FREE: u8 = 2;
pub const FORMAT_PROBE_LEDGER_CAPACITY: usize = 256;

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct AllocationEvent {
    pub kind: u8,
    pub pointer: usize,
    pub size: usize,
    pub role: u8,
}

struct AllocationEventSlot {
    kind: AtomicU8,
    pointer: AtomicUsize,
    size: AtomicUsize,
    role: AtomicU8,
}

impl AllocationEventSlot {
    const fn new() -> Self {
        Self {
            kind: AtomicU8::new(0),
            pointer: AtomicUsize::new(0),
            size: AtomicUsize::new(0),
            role: AtomicU8::new(0),
        }
    }
}

static FORMAT_HEAP_EVENT_COUNT: AtomicUsize = AtomicUsize::new(0);
static FORMAT_HEAP_EVENTS: [AllocationEventSlot; FORMAT_PROBE_LEDGER_CAPACITY] =
    [const { AllocationEventSlot::new() }; FORMAT_PROBE_LEDGER_CAPACITY];
static REQUEST_EVENT_COUNT: AtomicUsize = AtomicUsize::new(0);
static REQUEST_EVENTS: [AllocationEventSlot; FORMAT_PROBE_LEDGER_CAPACITY] =
    [const { AllocationEventSlot::new() }; FORMAT_PROBE_LEDGER_CAPACITY];
static HELD_REQUESTS: [AtomicUsize; FORMAT_PROBE_LEDGER_CAPACITY] =
    [const { AtomicUsize::new(0) }; FORMAT_PROBE_LEDGER_CAPACITY];
static PARSE_BARRIER_ENABLED: AtomicBool = AtomicBool::new(false);
static PARSE_BARRIER_ARRIVALS: AtomicUsize = AtomicUsize::new(0);

pub fn arm_two_thread_parse_barrier() {
    PARSE_BARRIER_ARRIVALS.store(0, Ordering::Release);
    PARSE_BARRIER_ENABLED.store(true, Ordering::Release);
}

pub fn parse_barrier_arrivals() -> usize {
    PARSE_BARRIER_ARRIVALS.load(Ordering::Acquire)
}

fn wait_at_parse_barrier() {
    if !PARSE_BARRIER_ENABLED.load(Ordering::Acquire) {
        return;
    }
    let arrivals = PARSE_BARRIER_ARRIVALS.fetch_add(1, Ordering::AcqRel) + 1;
    if arrivals >= 2 {
        PARSE_BARRIER_ENABLED.store(false, Ordering::Release);
    } else {
        while PARSE_BARRIER_ENABLED.load(Ordering::Acquire) {
            super::lwp_sys::yield_thread();
        }
    }
}

fn record_event(
    count: &AtomicUsize,
    events: &[AllocationEventSlot; FORMAT_PROBE_LEDGER_CAPACITY],
    kind: u8,
    pointer: usize,
    size: usize,
    role: u8,
) {
    let index = count.fetch_add(1, Ordering::AcqRel);
    let Some(slot) = events.get(index) else {
        return;
    };
    slot.pointer.store(pointer, Ordering::Relaxed);
    slot.size.store(size, Ordering::Relaxed);
    slot.role.store(role, Ordering::Relaxed);
    slot.kind.store(kind, Ordering::Release);
}

fn read_events(
    count: &AtomicUsize,
    events: &[AllocationEventSlot; FORMAT_PROBE_LEDGER_CAPACITY],
    output: &mut [AllocationEvent],
) -> usize {
    let used = count.load(Ordering::Acquire);
    let copied = used.min(output.len());
    for (index, event) in output.iter_mut().take(copied).enumerate() {
        let slot = &events[index];
        *event = AllocationEvent {
            kind: slot.kind.load(Ordering::Acquire),
            pointer: slot.pointer.load(Ordering::Relaxed),
            size: slot.size.load(Ordering::Relaxed),
            role: slot.role.load(Ordering::Relaxed),
        };
    }
    used
}

fn reset_events(count: &AtomicUsize, events: &[AllocationEventSlot; FORMAT_PROBE_LEDGER_CAPACITY]) {
    for slot in events {
        slot.kind.store(0, Ordering::Relaxed);
        slot.pointer.store(0, Ordering::Relaxed);
        slot.size.store(0, Ordering::Relaxed);
        slot.role.store(0, Ordering::Relaxed);
    }
    count.store(0, Ordering::Release);
}

fn record_format_heap(kind: u8, pointer: usize, size: usize, role: u8) {
    record_event(
        &FORMAT_HEAP_EVENT_COUNT,
        &FORMAT_HEAP_EVENTS,
        kind,
        pointer,
        size,
        role,
    );
}

pub fn reset_format_heap_ledger() {
    reset_events(&FORMAT_HEAP_EVENT_COUNT, &FORMAT_HEAP_EVENTS);
}

pub fn read_format_heap_ledger(output: &mut [AllocationEvent]) -> usize {
    read_events(&FORMAT_HEAP_EVENT_COUNT, &FORMAT_HEAP_EVENTS, output)
}

pub fn reset_request_ledger() {
    reset_events(&REQUEST_EVENT_COUNT, &REQUEST_EVENTS);
}

pub fn read_request_ledger(output: &mut [AllocationEvent]) -> usize {
    read_events(&REQUEST_EVENT_COUNT, &REQUEST_EVENTS, output)
}

pub(super) fn request_allocated(pointer: usize, size: usize) {
    record_event(
        &REQUEST_EVENT_COUNT,
        &REQUEST_EVENTS,
        ALLOCATION_EVENT_ALLOC,
        pointer,
        size,
        0,
    );
}

pub(super) fn request_freed(pointer: usize, size: usize) {
    record_event(
        &REQUEST_EVENT_COUNT,
        &REQUEST_EVENTS,
        ALLOCATION_EVENT_FREE,
        pointer,
        size,
        0,
    );
}

pub fn allocation_ledger_balanced(events: &[AllocationEvent]) -> bool {
    events.iter().all(|event| {
        (event.kind == ALLOCATION_EVENT_ALLOC || event.kind == ALLOCATION_EVENT_FREE)
            && events
                .iter()
                .filter(|other| {
                    other.kind == ALLOCATION_EVENT_ALLOC
                        && other.pointer == event.pointer
                        && other.size == event.size
                })
                .count()
                == 1
            && events
                .iter()
                .filter(|other| {
                    other.kind == ALLOCATION_EVENT_FREE
                        && other.pointer == event.pointer
                        && other.size == event.size
                })
                .count()
                == 1
    })
}

/// Hold real request-heap records until allocation fails. Returns held count
/// and the allocator error. Call `release_held_request_records` afterward.
pub fn hold_request_records_until_exhausted() -> (usize, i32) {
    let mut held = 0;
    while held < HELD_REQUESTS.len() {
        match super::alloc_request() {
            Ok(pointer) => {
                HELD_REQUESTS[held].store(pointer.as_ptr() as usize, Ordering::Release);
                held += 1;
            }
            Err(error) => return (held, error),
        }
    }
    (held, super::IPC_OK)
}

pub fn release_held_request_records() -> usize {
    let mut released = 0;
    for slot in &HELD_REQUESTS {
        let pointer = slot.swap(0, Ordering::AcqRel);
        if pointer != 0 {
            if let Some(pointer) = core::ptr::NonNull::new(pointer as *mut super::IpcRequest) {
                unsafe { super::free_request(pointer) };
            }
            released += 1;
        }
    }
    released
}

/// One bounded formatter vector snapshot for Dolphin probes.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FormatProbeVector {
    pub data_address: usize,
    pub length: u32,
    /// Workspace bytes reserved for this vector, or zero for direct buffers.
    pub allocation_length: usize,
    pub sample_length: u8,
    pub sample: [u8; 8],
    /// FNV-1a hash of all logical vector bytes, for bounded full-buffer checks.
    pub content_hash: u64,
    /// True when any owned staging bytes after the logical vector are zero.
    pub padding_is_zero: bool,
}

/// One allocation or free observed at the LWP workspace allocator boundary.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FormatProbeWorkspaceEvent {
    pub kind: u8,
    pub pointer: usize,
    pub size: usize,
}

// A maximum-length format can allocate one state plus one workspace record
// for each of its 31 parameters. Each record produces an allocation and a
// free event, so keep room for all 64 events.
pub const FORMAT_PROBE_MAX_WORKSPACE_EVENTS: usize = 64;
pub const FORMAT_PROBE_WORKSPACE_ALLOC: u8 = super::lwp_sys::WORKSPACE_PROBE_ALLOC;
pub const FORMAT_PROBE_WORKSPACE_FREE: u8 = super::lwp_sys::WORKSPACE_PROBE_FREE;

/// A fixed-size snapshot returned by the one-shot formatter probe hook.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FormatProbeSnapshot {
    pub input_count: u32,
    pub output_count: u32,
    pub asynchronous: u32,
    pub vector_count: u32,
    /// Fill output vectors with this byte before formatter cleanup.
    pub output_pattern: u8,
    pub workspace_event_count: u32,
    pub workspace_events: [FormatProbeWorkspaceEvent; FORMAT_PROBE_MAX_WORKSPACE_EVENTS],
    pub vectors: [FormatProbeVector; super::IOS_MAXFMT_PARAMS],
}

impl Default for FormatProbeSnapshot {
    fn default() -> Self {
        Self {
            input_count: 0,
            output_count: 0,
            asynchronous: 0,
            vector_count: 0,
            output_pattern: 0,
            workspace_event_count: 0,
            workspace_events: [FormatProbeWorkspaceEvent::default();
                FORMAT_PROBE_MAX_WORKSPACE_EVENTS],
            vectors: [FormatProbeVector::default(); super::IOS_MAXFMT_PARAMS],
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct FormatProbeSubmission {
    pub input_address: usize,
    pub input_length: u32,
    pub input_hash: u64,
    pub tail_hash: u64,
}

/// Set to a live submission record to capture the next actual IOS request.
pub static FORMAT_PROBE_SUBMISSION: AtomicUsize = AtomicUsize::new(0);

pub(super) fn capture_submission(argv: *mut Ioctlv, input_count: usize) {
    let output = FORMAT_PROBE_SUBMISSION.swap(0, Ordering::AcqRel) as *mut FormatProbeSubmission;
    if output.is_null() || argv.is_null() || input_count == 0 {
        return;
    }
    let first = unsafe { &*argv };
    let bytes = if first.data.is_null() {
        &[][..]
    } else {
        unsafe { core::slice::from_raw_parts(first.data.cast::<u8>(), first.len as usize) }
    };
    let hash = |bytes: &[u8]| {
        bytes.iter().fold(0xcbf2_9ce4_8422_2325u64, |hash, byte| {
            (hash ^ *byte as u64).wrapping_mul(0x0000_0100_0000_01b3)
        })
    };
    unsafe {
        output.write(FormatProbeSubmission {
            input_address: first.data as usize,
            input_length: first.len,
            input_hash: hash(bytes),
            tail_hash: hash(bytes.get(32..).unwrap_or_default()),
        });
    }
}

/// Set this to a live [`FormatProbeSnapshot`] pointer to intercept the next
/// formatted request. The bridge clears it before it parses the call. This
/// symbol exists only when the Dolphin probe feature is enabled.
pub static FORMAT_PROBE_SNAPSHOT: AtomicUsize = AtomicUsize::new(0);

pub const FORMAT_PROBE_ROLE_STATE: usize = 1;
pub const FORMAT_PROBE_ROLE_ARGV: usize = 2;
pub const FORMAT_PROBE_ROLE_SCALAR: usize = 3;
pub const FORMAT_PROBE_ROLE_STAGING_BUFFER: usize = 4;
pub const FORMAT_PROBE_ROLE_ASYNC_REQUEST: usize = 5;
/// Fail the selected occurrence of one allocation role. Zero disables faults.
pub static FORMAT_PROBE_FAIL_ALLOCATION_ROLE: AtomicUsize = AtomicUsize::new(0);
pub static FORMAT_PROBE_FAIL_ALLOCATION_OCCURRENCE: AtomicUsize = AtomicUsize::new(0);
/// Count occurrences only for the selected role.
pub static FORMAT_PROBE_ALLOCATION_ROLE_ATTEMPTS: AtomicUsize = AtomicUsize::new(0);
/// Count live formatter allocations for cleanup checks.
pub static FORMAT_PROBE_LIVE_ALLOCATIONS: AtomicUsize = AtomicUsize::new(0);
/// Fail the next async submission before the IOS request becomes pending.
pub static FORMAT_PROBE_FAIL_ASYNC_SUBMISSION: AtomicBool = AtomicBool::new(false);

/// Sentinel returned after the one-shot probe captured a formatted request.
pub const FORMAT_PROBE_COMPLETE: i32 = i32::MIN + 101;

pub(super) fn allow_allocation(role: u8) -> bool {
    if FORMAT_PROBE_FAIL_ALLOCATION_ROLE.load(Ordering::Acquire) != role as usize {
        return true;
    }
    let occurrence = FORMAT_PROBE_ALLOCATION_ROLE_ATTEMPTS.fetch_add(1, Ordering::AcqRel) + 1;
    if occurrence == FORMAT_PROBE_FAIL_ALLOCATION_OCCURRENCE.load(Ordering::Acquire) {
        FORMAT_PROBE_FAIL_ALLOCATION_ROLE.store(0, Ordering::Release);
        false
    } else {
        true
    }
}

pub(super) fn allocation_created() {
    FORMAT_PROBE_LIVE_ALLOCATIONS.fetch_add(1, Ordering::AcqRel);
}

pub(super) fn allocation_released() {
    FORMAT_PROBE_LIVE_ALLOCATIONS.fetch_sub(1, Ordering::AcqRel);
}

pub(super) fn fail_async_submission() -> bool {
    FORMAT_PROBE_FAIL_ASYNC_SUBMISSION.swap(false, Ordering::AcqRel)
}

pub(super) fn format_heap_allocated(pointer: usize, size: usize, role: u8) {
    record_format_heap(ALLOCATION_EVENT_ALLOC, pointer, size, role);
}

pub(super) fn format_heap_freed(pointer: usize, size: usize) {
    record_format_heap(ALLOCATION_EVENT_FREE, pointer, size, 0);
}

pub(super) unsafe fn parse_or_capture(
    call: &FormatCall,
) -> Result<(*mut FormatState, usize, usize, *mut Ioctlv), i32> {
    wait_at_parse_barrier();
    let snapshot = FORMAT_PROBE_SNAPSHOT.swap(0, Ordering::AcqRel) as *mut FormatProbeSnapshot;
    if snapshot.is_null() {
        return unsafe { super::parse_format_call(call) };
    }
    super::lwp_sys::WORKSPACE_PROBE_EVENT_COUNT.store(0, Ordering::Release);
    let result = unsafe { capture_format_probe(call, snapshot) };
    Err(result)
}

unsafe fn capture_format_probe(call: &FormatCall, snapshot: *mut FormatProbeSnapshot) -> i32 {
    if snapshot.is_null() {
        return super::IPC_EINVAL;
    }
    let (state, cnt_in, cnt_io, argv) = match unsafe { super::parse_format_call(call) } {
        Ok(parsed) => parsed,
        Err(error) => return error,
    };
    let total = cnt_in + cnt_io;
    let state_ref = unsafe { &*state };
    let snapshot_ref = unsafe { &mut *snapshot };
    snapshot_ref.input_count = cnt_in as u32;
    snapshot_ref.output_count = cnt_io as u32;
    snapshot_ref.asynchronous = u32::from(call.asynchronous != 0);
    snapshot_ref.vector_count = total as u32;
    for index in 0..total {
        let entry = unsafe { &*argv.add(index) };
        let sample_len = (entry.len as usize).min(8);
        let record = &mut snapshot_ref.vectors[index];
        record.data_address = entry.data as usize;
        record.length = entry.len;
        record.allocation_length = state_ref.owned[..state_ref.num_owned]
            .iter()
            .find(|owned| owned.ipc_buf == entry.data)
            .map_or(0, |owned| owned.allocation_len);
        record.sample_length = sample_len as u8;
        if sample_len > 0 && !entry.data.is_null() {
            unsafe {
                core::ptr::copy_nonoverlapping(
                    entry.data.cast::<u8>(),
                    record.sample.as_mut_ptr(),
                    sample_len,
                );
            }
        }
        let mut hash = 0xcbf2_9ce4_8422_2325u64;
        if !entry.data.is_null() {
            for byte in
                unsafe { core::slice::from_raw_parts(entry.data.cast::<u8>(), entry.len as usize) }
            {
                hash = (hash ^ *byte as u64).wrapping_mul(0x0000_0100_0000_01b3);
            }
        }
        record.content_hash = hash;
        record.padding_is_zero = record.allocation_length <= entry.len as usize
            || entry.data.is_null()
            || unsafe {
                core::slice::from_raw_parts(
                    entry.data.cast::<u8>().add(entry.len as usize),
                    record.allocation_length - entry.len as usize,
                )
            }
            .iter()
            .all(|byte| *byte == 0);
    }
    let output_pattern = snapshot_ref.output_pattern;
    if output_pattern != 0 {
        for index in cnt_in..total {
            let entry = unsafe { &*argv.add(index) };
            if !entry.data.is_null() && entry.len != 0 {
                unsafe {
                    core::ptr::write_bytes(
                        entry.data.cast::<u8>(),
                        output_pattern,
                        entry.len as usize,
                    )
                };
            }
        }
    }
    unsafe { super::format_release(super::IPC_OK, state, output_pattern != 0, false) };
    let event_count = super::lwp_sys::WORKSPACE_PROBE_EVENT_COUNT
        .load(Ordering::Acquire)
        .min(FORMAT_PROBE_MAX_WORKSPACE_EVENTS);
    snapshot_ref.workspace_event_count = event_count as u32;
    for index in 0..event_count {
        let slot = &super::lwp_sys::WORKSPACE_PROBE_EVENTS[index];
        let kind = slot.kind.load(Ordering::Acquire);
        if kind != 0 {
            snapshot_ref.workspace_events[index] = FormatProbeWorkspaceEvent {
                kind,
                pointer: slot.pointer.load(Ordering::Relaxed),
                size: slot.size.load(Ordering::Relaxed),
            };
        }
    }
    FORMAT_PROBE_COMPLETE
}
