//! ARAM DMA request queues.
//!
//! This module ports libogc's `arqueue.c` request scheduling. Requests remain
//! caller-owned and must stay at a stable address while they are queued or
//! active.

use core::ptr;

use ogc_lwp_driver as lwp;

use crate::{register_callback as ar_register_callback, start_dma};

pub mod manager;

pub const ARQ_MRAMTOARAM: u32 = crate::AR_MRAMTOARAM;
pub const ARQ_ARAMTOMRAM: u32 = crate::AR_ARAMTOMRAM;
pub const ARQ_DEF_CHUNK_SIZE: u32 = 4096;
pub const ARQ_PRIO_LO: u32 = 0;
pub const ARQ_PRIO_HI: u32 = 1;

pub const ARQ_TASK_READY: u32 = 0;
pub const ARQ_TASK_RUNNING: u32 = 1;
pub const ARQ_TASK_FINISHED: u32 = 2;

pub type ArqCallback = Option<unsafe extern "C" fn(*mut ArqRequest)>;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ArqNode {
    pub next: *mut ArqNode,
    pub prev: *mut ArqNode,
}

#[repr(C)]
pub struct ArqRequest {
    pub node: ArqNode,
    pub owner: u32,
    pub dir: u32,
    pub prio: u32,
    pub state: u32,
    pub aram_addr: u32,
    pub mram_addr: u32,
    pub len: u32,
    pub callback: ArqCallback,
}

/// Transfer settings shared by synchronous and asynchronous ARAM requests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ArqTransfer {
    /// Application identifier copied into the completed request.
    pub owner: u32,
    /// `ARQ_MRAMTOARAM` or `ARQ_ARAMTOMRAM`.
    pub dir: u32,
    /// `ARQ_PRIO_LO` or `ARQ_PRIO_HI`.
    pub prio: u32,
    /// Byte address in ARAM.
    pub aram_addr: u32,
    /// Physical byte address in main RAM.
    pub mram_addr: u32,
    /// Transfer length in bytes.
    pub len: u32,
}

const _: () = assert!(core::mem::size_of::<ArqRequest>() == 40);

impl ArqRequest {
    pub const fn new() -> Self {
        Self {
            node: ArqNode {
                next: ptr::null_mut(),
                prev: ptr::null_mut(),
            },
            owner: 0,
            dir: 0,
            prio: 0,
            state: ARQ_TASK_READY,
            aram_addr: 0,
            mram_addr: 0,
            len: 0,
            callback: None,
        }
    }
}

impl Default for ArqRequest {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Copy, Clone)]
struct Queue {
    first: *mut ArqRequest,
    last: *mut ArqRequest,
}

impl Queue {
    const fn new() -> Self {
        Self {
            first: ptr::null_mut(),
            last: ptr::null_mut(),
        }
    }

    unsafe fn push_back(&mut self, request: *mut ArqRequest) {
        unsafe {
            (*request).node.next = ptr::null_mut();
            (*request).node.prev = self.last.cast();
            if self.last.is_null() {
                self.first = request;
            } else {
                (*self.last).node.next = request.cast();
            }
        }
        self.last = request;
    }

    unsafe fn pop_front(&mut self) -> *mut ArqRequest {
        let request = self.first;
        if request.is_null() {
            return request;
        }
        unsafe {
            self.first = (*request).node.next.cast();
            if self.first.is_null() {
                self.last = ptr::null_mut();
            } else {
                (*self.first).node.prev = ptr::null_mut();
            }
        }
        request
    }

    unsafe fn remove(&mut self, target: *mut ArqRequest) -> bool {
        let mut request = self.first;
        while !request.is_null() && request != target {
            request = unsafe { (*request).node.next.cast() };
        }
        if request.is_null() {
            return false;
        }

        unsafe {
            let prev = (*request).node.prev.cast::<ArqRequest>();
            let next = (*request).node.next.cast::<ArqRequest>();
            if prev.is_null() {
                self.first = next;
            } else {
                (*prev).node.next = next.cast();
            }
            if next.is_null() {
                self.last = prev;
            } else {
                (*next).node.prev = prev.cast();
            }
        }
        true
    }
}

struct State {
    initialized: bool,
    chunk_size: u32,
    low: Queue,
    high: Queue,
    pending_low: *mut ArqRequest,
    pending_high: *mut ArqRequest,
    callback_low: ArqCallback,
    callback_high: ArqCallback,
    sync_queue: lwp::QueueHandle,
}

impl State {
    const fn new() -> Self {
        Self {
            initialized: false,
            chunk_size: ARQ_DEF_CHUNK_SIZE,
            low: Queue::new(),
            high: Queue::new(),
            pending_low: ptr::null_mut(),
            pending_high: ptr::null_mut(),
            callback_low: None,
            callback_high: None,
            sync_queue: 0,
        }
    }
}

static mut STATE: State = State::new();

#[inline]
fn state_ptr() -> *mut State {
    ptr::addr_of_mut!(STATE)
}

unsafe extern "C" fn callback_dummy(_request: *mut ArqRequest) {}

unsafe extern "C" fn callback_sync(_request: *mut ArqRequest) {
    unsafe {
        lwp::queue_broadcast((*state_ptr()).sync_queue);
    }
}

unsafe fn service_low(state: *mut State) {
    unsafe {
        if (*state).pending_low.is_null() {
            (*state).pending_low = (*state).low.pop_front();
        }
        let request = (*state).pending_low;
        if request.is_null() {
            return;
        }

        (*request).state = ARQ_TASK_RUNNING;
        if (*request).len <= (*state).chunk_size {
            start_dma(
                (*request).dir,
                (*request).mram_addr,
                (*request).aram_addr,
                (*request).len,
            );
            (*state).callback_low = (*request).callback;
        } else {
            let chunk = (*state).chunk_size;
            start_dma(
                (*request).dir,
                (*request).mram_addr,
                (*request).aram_addr,
                chunk,
            );
            (*request).len -= chunk;
            (*request).aram_addr = (*request).aram_addr.wrapping_add(chunk);
            (*request).mram_addr = (*request).mram_addr.wrapping_add(chunk);
        }
    }
}

unsafe fn pop_high(state: *mut State) {
    unsafe {
        let request = (*state).high.pop_front();
        if request.is_null() {
            return;
        }
        (*request).state = ARQ_TASK_RUNNING;
        start_dma(
            (*request).dir,
            (*request).mram_addr,
            (*request).aram_addr,
            (*request).len,
        );
        (*state).callback_high = (*request).callback;
        (*state).pending_high = request;
    }
}

unsafe extern "C" fn interrupt_service() {
    let state = state_ptr();
    let old_interrupt_state = lwp::critical_enter();

    unsafe {
        let (request, callback, completed_high) = if (*state).callback_high.is_some() {
            let request = (*state).pending_high;
            let callback = (*state).callback_high;
            if !request.is_null() {
                (*request).state = ARQ_TASK_FINISHED;
            }
            (request, callback, true)
        } else if (*state).callback_low.is_some() {
            let request = (*state).pending_low;
            let callback = (*state).callback_low;
            if !request.is_null() {
                (*request).state = ARQ_TASK_FINISHED;
            }
            (request, callback, false)
        } else {
            (ptr::null_mut(), None, false)
        };

        lwp::critical_exit(old_interrupt_state);
        if let Some(callback) = callback {
            callback(request);
        }

        let level = lwp::critical_enter();
        if completed_high {
            (*state).pending_high = ptr::null_mut();
            (*state).callback_high = None;
        } else if !request.is_null() && callback.is_some() {
            (*state).pending_low = ptr::null_mut();
            (*state).callback_low = None;
        }
        pop_high(state);
        if (*state).pending_high.is_null() {
            service_low(state);
        }
        lwp::critical_exit(level);
    }
}

/// Initializes the request queues and installs the ARAM completion callback.
pub fn init() {
    let level = lwp::critical_enter();
    unsafe {
        let state = state_ptr();
        if (*state).initialized {
            lwp::critical_exit(level);
            return;
        }
        (*state).pending_low = ptr::null_mut();
        (*state).pending_high = ptr::null_mut();
        (*state).callback_low = None;
        (*state).callback_high = None;
        (*state).chunk_size = ARQ_DEF_CHUNK_SIZE;
        (*state).low = Queue::new();
        (*state).high = Queue::new();
        if lwp::init_queue(&mut (*state).sync_queue) != lwp::LWP_SUCCESSFUL {
            // BUG FIX: Do not report ARQ as initialized without a valid
            // synchronization queue. Synchronous requests would otherwise
            // sleep on a null queue and never complete.
            lwp::critical_exit(level);
            return;
        }
        ar_register_callback(Some(interrupt_service));
        (*state).initialized = true;
    }
    lwp::critical_exit(level);
}

/// Resets only the initialized flag, matching libogc's `ARQ_Reset`.
pub fn reset() {
    let level = lwp::critical_enter();
    unsafe {
        (*state_ptr()).initialized = false;
    }
    lwp::critical_exit(level);
}

/// Sets the low priority DMA chunk size, rounded up to 32 bytes.
pub fn set_chunk_size(size: u32) {
    let level = lwp::critical_enter();
    unsafe {
        (*state_ptr()).chunk_size = size.wrapping_add(31) & !31;
    }
    lwp::critical_exit(level);
}

pub fn chunk_size() -> u32 {
    unsafe { (*state_ptr()).chunk_size }
}

/// Posts an asynchronous request.
///
/// # Safety
/// `request` must point to a writable, stable request object that remains alive
/// until its callback runs or the request is removed.
pub unsafe fn post_request_async(
    request: *mut ArqRequest,
    transfer: ArqTransfer,
    callback: ArqCallback,
) {
    unsafe {
        (*request).state = ARQ_TASK_READY;
        (*request).dir = transfer.dir;
        (*request).owner = transfer.owner;
        (*request).aram_addr = transfer.aram_addr;
        (*request).mram_addr = transfer.mram_addr;
        (*request).len = transfer.len;
        (*request).prio = transfer.prio;
        (*request).callback = callback.or(Some(callback_dummy));

        let level = lwp::critical_enter();
        let state = state_ptr();
        if transfer.prio == ARQ_PRIO_LO {
            (*state).low.push_back(request);
        } else {
            (*state).high.push_back(request);
        }
        if (*state).pending_low.is_null() && (*state).pending_high.is_null() {
            pop_high(state);
            if (*state).pending_high.is_null() {
                service_low(state);
            }
        }
        lwp::critical_exit(level);
    }
}

/// Posts a request and sleeps until its DMA callback marks it complete.
///
/// # Safety
/// `request` must satisfy the same requirements as [`post_request_async`].
pub unsafe fn post_request(request: *mut ArqRequest, transfer: ArqTransfer) {
    unsafe {
        post_request_async(request, transfer, Some(callback_sync));

        let state = state_ptr();
        let level = lwp::critical_enter();
        // The DMA callback changes this field while the caller sleeps.
        while ptr::read_volatile(ptr::addr_of!((*request).state)) != ARQ_TASK_FINISHED {
            let _ = lwp::queue_sleep((*state).sync_queue);
        }
        lwp::critical_exit(level);
    }
}

/// Removes a queued request. Active transfers cannot be cancelled.
///
/// # Safety
/// `request` must point to a live request object which is queued or pending.
pub unsafe fn remove_request(request: *mut ArqRequest) {
    let level = lwp::critical_enter();
    unsafe {
        let state = state_ptr();
        if !(*state).high.remove(request) {
            (*state).low.remove(request);
        }
        if (*state).pending_low == request && (*state).callback_low.is_none() {
            (*state).pending_low = ptr::null_mut();
        }
    }
    lwp::critical_exit(level);
}

pub fn flush_queue() {
    let level = lwp::critical_enter();
    unsafe {
        let state = state_ptr();
        (*state).high = Queue::new();
        (*state).low = Queue::new();
        if (*state).callback_low.is_none() {
            (*state).pending_low = ptr::null_mut();
        }
    }
    lwp::critical_exit(level);
}

pub fn remove_owner_request(owner: u32) -> u32 {
    let level = lwp::critical_enter();
    let state = state_ptr();
    let mut removed = 0;
    unsafe {
        for queue in [&mut (*state).high, &mut (*state).low] {
            let mut request = queue.first;
            while !request.is_null() {
                let next = (*request).node.next.cast::<ArqRequest>();
                if (*request).owner == owner {
                    queue.remove(request);
                    removed += 1;
                }
                request = next;
            }
        }
    }
    lwp::critical_exit(level);
    removed
}
