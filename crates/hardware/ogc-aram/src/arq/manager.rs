//! Stack-style ARAM storage management from libogc's `arqmgr.c`.

use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::{AtomicBool, Ordering};

use ogc_lwp_driver as lwp;

use super::{ARQ_MRAMTOARAM, ARQ_PRIO_HI, ArqRequest};

const STACK_ENTRIES: usize = 16;
const ZERO_BYTES: usize = 256;

struct RequestInfo {
    request: ArqRequest,
    polled: AtomicBool,
}

impl RequestInfo {
    const fn new() -> Self {
        Self {
            request: ArqRequest::new(),
            polled: AtomicBool::new(false),
        }
    }
}

#[repr(align(32))]
struct ZeroBuffer([u8; ZERO_BYTES]);

struct ManagerState {
    stack_location: u32,
    free_bytes: u32,
    stack_pointer: [u32; STACK_ENTRIES],
    requests: [RequestInfo; STACK_ENTRIES],
    zero_buffer: ZeroBuffer,
}

impl ManagerState {
    const fn new() -> Self {
        Self {
            stack_location: 0,
            free_bytes: 0,
            stack_pointer: [0; STACK_ENTRIES],
            requests: [const { RequestInfo::new() }; STACK_ENTRIES],
            zero_buffer: ZeroBuffer([0; ZERO_BYTES]),
        }
    }
}

static mut STATE: ManagerState = ManagerState::new();

#[inline]
fn state_ptr() -> *mut ManagerState {
    ptr::addr_of_mut!(STATE)
}

unsafe extern "C" fn poll_callback(request: *mut ArqRequest) {
    unsafe {
        let requests = ptr::addr_of_mut!((*state_ptr()).requests).cast::<RequestInfo>();
        for index in 0..STACK_ENTRIES {
            let info = requests.add(index);
            if ptr::addr_of_mut!((*info).request) == request {
                (*info).polled.store(true, Ordering::Release);
                break;
            }
        }
    }
}

/// Initializes the ARAM allocation stack and writes its zero buffer to ARAM.
///
/// The ARQ subsystem must be initialized before this function is called.
pub fn init(aram_base: u32, len: i32) {
    if len <= 0 {
        return;
    }

    unsafe {
        let state = state_ptr();
        (*state).stack_location = 0;
        (*state).stack_pointer[0] = aram_base;
        (*state).free_bytes = len as u32;
        ptr::write_bytes(
            ptr::addr_of_mut!((*state).zero_buffer.0).cast::<u8>(),
            0,
            ZERO_BYTES,
        );
        let zero_buffer = ptr::addr_of_mut!((*state).zero_buffer.0).cast::<c_void>();
        let _ = push_data(zero_buffer, ZERO_BYTES as i32);
    }
}

/// Pushes an aligned data buffer to ARAM and waits for its DMA to finish.
///
/// Returns the ARAM address, or zero when the input or available stack space is
/// invalid.
///
/// # Safety
/// `buffer` must point to `len` readable bytes and remain valid until the DMA
/// callback completes. The buffer address must be 32-byte aligned.
pub unsafe fn push_data(buffer: *mut c_void, len: i32) -> u32 {
    if (buffer as usize) & 31 != 0 || len <= 0 {
        return 0;
    }

    let rounded_len = (len as u32).wrapping_add(31) & !31;
    let level = lwp::critical_enter();
    let state = state_ptr();
    unsafe {
        if (*state).free_bytes < rounded_len
            || (*state).stack_location >= (STACK_ENTRIES - 1) as u32
        {
            lwp::critical_exit(level);
            return 0;
        }

        let index = (*state).stack_location as usize;
        let info = ptr::addr_of_mut!((*state).requests[index]);
        (*info).polled.store(false, Ordering::Relaxed);
        let aram_start = (*state).stack_pointer[index];
        (*state).stack_location += 1;
        (*state).stack_pointer[index + 1] = aram_start.wrapping_add(rounded_len);
        (*state).free_bytes -= rounded_len;

        super::post_request_async(
            ptr::addr_of_mut!((*info).request),
            super::ArqTransfer {
                owner: index as u32,
                dir: ARQ_MRAMTOARAM,
                prio: ARQ_PRIO_HI,
                aram_addr: aram_start,
                mram_addr: buffer as u32,
                len: rounded_len,
            },
            Some(poll_callback),
        );

        lwp::critical_exit(level);

        while !(*info).polled.load(Ordering::Acquire) {
            core::hint::spin_loop();
        }
        aram_start
    }
}

pub fn pop() {
    let level = lwp::critical_enter();
    unsafe {
        let state = state_ptr();
        if (*state).stack_location > 1 {
            let index = (*state).stack_location as usize;
            (*state).free_bytes = (*state)
                .free_bytes
                .wrapping_add((*state).stack_pointer[index] - (*state).stack_pointer[index - 1]);
            (*state).stack_location -= 1;
        }
    }
    lwp::critical_exit(level);
}

pub fn zero_buffer() -> u32 {
    unsafe { (*state_ptr()).stack_pointer[0] }
}

pub fn stack_pointer() -> u32 {
    let level = lwp::critical_enter();
    let result = unsafe {
        let state = state_ptr();
        (*state).stack_pointer[(*state).stack_location as usize]
    };
    lwp::critical_exit(level);
    result
}

pub fn free_size() -> u32 {
    let level = lwp::critical_enter();
    let result = unsafe { (*state_ptr()).free_bytes };
    lwp::critical_exit(level);
    result
}
