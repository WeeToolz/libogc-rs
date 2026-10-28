#![no_std]

//! Tuxedo ABI compatibility over the authoritative LWP scheduler.
//!
//! This crate intentionally owns no scheduler, current-thread variable, timer
//! interrupt, or synthetic runtime model.

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::ffi::c_void;
use core::ptr;

use ogc_lwp_driver as lwp;

pub const KTHR_MAX_PRIO: u16 = 0;
pub const KTHR_MAIN_PRIO: u16 = 0x3f;
pub const KTHR_MIN_PRIO: u16 = 0x7f;

pub const KTHR_STATE_UNINIT: u16 = 0;
pub const KTHR_STATE_FINISHED: u16 = 1;
pub const KTHR_STATE_RUNNING: u16 = 2;
pub const KTHR_STATE_WAITING: u16 = 3;
pub const KTHR_STATE_WAITING_MUTEX: u16 = 4;

pub type KThreadFn = Option<unsafe extern "C" fn(arg: *mut c_void) -> isize>;
pub type KTickTaskFn = Option<unsafe extern "C" fn(task: *mut KTickTask)>;

/// Register state saved by the Tuxedo PowerPC exception entry code.
#[repr(C)]
#[derive(Copy, Clone)]
pub struct PpcContext {
    pub pc: u32,
    pub msr: u32,
    pub cr: u32,
    pub lr: u32,
    pub ctr: u32,
    pub xer: u32,
    pub gpr: [u32; 32],
    pub fpscr: u64,
    pub fpr: [u64; 32],
    pub gqr: [u32; 8],
    pub ps: [u64; 32],
}

impl PpcContext {
    const fn zeroed() -> Self {
        Self {
            pc: 0,
            msr: 0,
            cr: 0,
            lr: 0,
            ctr: 0,
            xer: 0,
            gpr: [0; 32],
            fpscr: 0,
            fpr: [0; 32],
            gqr: [0; 8],
            ps: [0; 32],
        }
    }
}

const _: () = {
    assert!(core::mem::offset_of!(PpcContext, gpr) == 0x018);
    assert!(core::mem::offset_of!(PpcContext, fpscr) == 0x098);
    assert!(core::mem::offset_of!(PpcContext, fpr) == 0x0a0);
    assert!(core::mem::offset_of!(PpcContext, gqr) == 0x1a0);
    assert!(core::mem::offset_of!(PpcContext, ps) == 0x1c0);
    assert!(core::mem::size_of::<PpcContext>() == 0x2c0);
};

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct KThrQueue {
    pub next: *mut KThread,
    pub prev: *mut KThread,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct KThreadWait {
    pub link: KThrQueue,
    pub queue: *mut KThrQueue,
    pub token: usize,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct KThreadFinish {
    pub rc: isize,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union KThreadWaitFinish {
    pub wait: KThreadWait,
    pub finish: KThreadFinish,
}

impl KThreadWaitFinish {
    const fn zeroed() -> Self {
        Self {
            wait: KThreadWait {
                link: KThrQueue {
                    next: ptr::null_mut(),
                    prev: ptr::null_mut(),
                },
                queue: ptr::null_mut(),
                token: 0,
            },
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct KThread {
    pub ctx: PpcContext,
    pub next: *mut KThread,
    pub state: u16,
    pub suspend: u16,
    pub prio: u16,
    pub baseprio: u16,
    pub waiters: KThrQueue,
    pub data: KThreadWaitFinish,
    pub reent: [u8; 512],
}

impl KThread {
    pub const fn empty() -> Self {
        Self {
            ctx: PpcContext::zeroed(),
            next: ptr::null_mut(),
            state: KTHR_STATE_UNINIT,
            suspend: 0,
            prio: 0,
            baseprio: 0,
            waiters: KThrQueue {
                next: ptr::null_mut(),
                prev: ptr::null_mut(),
            },
            data: KThreadWaitFinish::zeroed(),
            reent: [0; 512],
        }
    }

    const fn main_thread() -> Self {
        let mut thread = Self::empty();
        thread.state = KTHR_STATE_RUNNING;
        thread.prio = KTHR_MAIN_PRIO;
        thread.baseprio = KTHR_MAIN_PRIO;
        thread
    }
}

impl Default for KThread {
    fn default() -> Self {
        Self::empty()
    }
}

// C's Tuxedo runtime owns one statically allocated main-thread object and a
// priority-sorted intrusive thread list rooted at `s_firstThread`. The Rust
// compatibility layer bridges that object to the authoritative LWP scheduler
// with one cached handle; no fixed-capacity mapping table is involved.
static mut MAIN_THREAD_OBJECT: KThread = KThread::main_thread();
static mut MAIN_LWP_HANDLE: lwp::LwpThread = lwp::LWP_THREAD_NULL;
static mut FIRST_THREAD: *mut KThread = ptr::null_mut();

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct KMutex {
    pub owner: *mut KThread,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct KrMutex {
    pub mutex: KMutex,
    pub counter: u32,
}

/// C-header spelling retained alongside the historical Rust `KrMutex` name.
pub type KRMutex = KrMutex;

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct KCondVar {
    pub dummy: u8,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct KTickTask {
    pub next: *mut KTickTask,
    pub fn_: KTickTaskFn,
    pub target: u64,
    pub period: u64,
}

const _: () = {
    assert!(core::mem::offset_of!(KThread, ctx) == 0x000);
    assert!(core::mem::offset_of!(KThread, next) == 0x2c0);
    assert!(core::mem::offset_of!(KThread, state) == 0x2c4);
    assert!(core::mem::offset_of!(KThread, suspend) == 0x2c6);
    assert!(core::mem::offset_of!(KThread, prio) == 0x2c8);
    assert!(core::mem::offset_of!(KThread, baseprio) == 0x2ca);
    assert!(core::mem::offset_of!(KThread, waiters) == 0x2cc);
    assert!(core::mem::offset_of!(KThread, data) == 0x2d4);
    assert!(core::mem::offset_of!(KThread, reent) == 0x2e4);
    assert!(core::mem::size_of::<KThread>() == 0x4e8);
};

#[inline]
fn to_lwp_priority(priority: u16) -> u8 {
    (KTHR_MIN_PRIO.saturating_sub(priority.min(KTHR_MIN_PRIO))) as u8
}

#[inline]
fn state_after_external_unblock(state: u16, suspend: u16) -> u16 {
    if suspend == 0 {
        KTHR_STATE_RUNNING
    } else {
        state
    }
}

unsafe fn wait_data(thread: *mut KThread) -> *mut KThreadWait {
    unsafe { core::ptr::addr_of_mut!((*thread).data.wait) }
}

unsafe fn finish_result(thread: *mut KThread) -> isize {
    unsafe { (*thread).data.finish.rc }
}

unsafe fn set_finish_result(thread: *mut KThread, result: isize) {
    unsafe { (*thread).data.finish.rc = result };
}

unsafe fn lwp_handle_for_object(object: *mut KThread) -> Option<lwp::LwpThread> {
    if object.is_null() {
        return None;
    }
    if object == ptr::addr_of_mut!(MAIN_THREAD_OBJECT) {
        let handle = unsafe { MAIN_LWP_HANDLE };
        return (handle != lwp::LWP_THREAD_NULL).then_some(handle);
    }
    lwp::thread_handle_by_argument(object.cast())
}

unsafe fn thread_list_contains(object: *mut KThread) -> bool {
    let mut current = unsafe { FIRST_THREAD };
    while !current.is_null() {
        if current == object {
            return true;
        }
        current = unsafe { (*current).next };
    }
    false
}

unsafe fn thread_list_insert(object: *mut KThread) {
    if object.is_null() || unsafe { thread_list_contains(object) } {
        return;
    }
    let mut previous: *mut KThread = ptr::null_mut();
    let mut current = unsafe { FIRST_THREAD };
    while !current.is_null() && unsafe { (*current).prio <= (*object).prio } {
        previous = current;
        current = unsafe { (*current).next };
    }
    unsafe { (*object).next = current };
    if previous.is_null() {
        unsafe { FIRST_THREAD = object };
    } else {
        unsafe { (*previous).next = object };
    }
}

unsafe fn thread_list_remove(object: *mut KThread) {
    if object.is_null() {
        return;
    }
    let mut previous: *mut KThread = ptr::null_mut();
    let mut current = unsafe { FIRST_THREAD };
    while !current.is_null() {
        if current == object {
            let next = unsafe { (*current).next };
            if previous.is_null() {
                unsafe { FIRST_THREAD = next };
            } else {
                unsafe { (*previous).next = next };
            }
            unsafe { (*current).next = ptr::null_mut() };
            return;
        }
        previous = current;
        current = unsafe { (*current).next };
    }
}

unsafe fn ensure_main_thread_registered() {
    let main = ptr::addr_of_mut!(MAIN_THREAD_OBJECT);
    if unsafe { MAIN_LWP_HANDLE == lwp::LWP_THREAD_NULL } {
        unsafe { MAIN_LWP_HANDLE = lwp::current_thread() };
    }
    if !unsafe { thread_list_contains(main) } {
        let _ = unsafe { lwp::initialize_external_reent((*main).reent.as_mut_ptr().cast()) };
        let _ = unsafe {
            lwp::set_thread_context_mirror(MAIN_LWP_HANDLE, ptr::addr_of_mut!((*main).ctx).cast())
        };
        let _ = unsafe {
            lwp::set_thread_reent_storage(MAIN_LWP_HANDLE, (*main).reent.as_mut_ptr().cast())
        };
        unsafe { thread_list_insert(main) };
    }
}

unsafe fn current_object() -> *mut KThread {
    unsafe { ensure_main_thread_registered() };
    let current = lwp::current_thread();
    if let Some(argument) = lwp::thread_argument(current) {
        let object = argument.cast::<KThread>();
        if !object.is_null() && unsafe { thread_list_contains(object) } {
            return object;
        }
    }
    ptr::addr_of_mut!(MAIN_THREAD_OBJECT)
}

unsafe fn queue_insert(queue: *mut KThrQueue, thread: *mut KThread) {
    let mut position: *mut KThread = ptr::null_mut();
    let mut current = unsafe { (*queue).next };
    while !current.is_null() && unsafe { (*current).prio <= (*thread).prio } {
        position = current;
        current = unsafe { (*wait_data(current)).link.next };
    }

    let wait = unsafe { &mut *wait_data(thread) };
    if position.is_null() {
        wait.link.next = unsafe { (*queue).next };
        wait.link.prev = ptr::null_mut();
        unsafe { (*queue).next = thread };
    } else {
        let previous_wait = unsafe { &mut *wait_data(position) };
        wait.link.next = previous_wait.link.next;
        wait.link.prev = position;
        previous_wait.link.next = thread;
    }

    if !wait.link.next.is_null() {
        unsafe { (*wait_data(wait.link.next)).link.prev = thread };
    } else {
        unsafe { (*queue).prev = thread };
    }
    wait.queue = queue;
}

unsafe fn queue_remove(queue: *mut KThrQueue, thread: *mut KThread) {
    let wait = unsafe { &mut *wait_data(thread) };
    if wait.queue != queue {
        return;
    }
    let previous = wait.link.prev;
    let next = wait.link.next;
    wait.queue = ptr::null_mut();
    if previous.is_null() {
        unsafe { (*queue).next = next };
    } else {
        unsafe { (*wait_data(previous)).link.next = next };
    }
    if next.is_null() {
        unsafe { (*queue).prev = previous };
    } else {
        unsafe { (*wait_data(next)).link.prev = previous };
    }
    wait.link.next = ptr::null_mut();
    wait.link.prev = ptr::null_mut();
}

unsafe fn update_dynamic_priority(mut thread: *mut KThread) {
    while !thread.is_null() {
        let mut priority = unsafe { (*thread).baseprio };
        let first_waiter = unsafe { (*thread).waiters.next };
        if !first_waiter.is_null() {
            priority = priority.min(unsafe { (*first_waiter).prio });
        }
        if unsafe { (*thread).prio == priority } {
            break;
        }

        unsafe {
            thread_list_remove(thread);
            (*thread).prio = priority;
            thread_list_insert(thread);
        }
        if let Some(handle) = unsafe { lwp_handle_for_object(thread) } {
            lwp::set_thread_priority(handle, to_lwp_priority(priority) as u32);
        }
        if unsafe { (*thread).state == KTHR_STATE_RUNNING } {
            break;
        }

        let queue = unsafe { (*wait_data(thread)).queue };
        if queue.is_null() {
            break;
        }
        unsafe {
            queue_remove(queue, thread);
            queue_insert(queue, thread);
        }
        if unsafe { (*thread).state != KTHR_STATE_WAITING_MUTEX } {
            break;
        }
        let mutex = unsafe { (*wait_data(thread)).token as *mut KMutex };
        if mutex.is_null() {
            break;
        }
        thread = unsafe { (*mutex).owner };
    }
}

unsafe fn remove_mutex_waiters(owner: *mut KThread, mutex: *mut KMutex) -> *mut KThread {
    let mut next_owner: *mut KThread = ptr::null_mut();
    let mut current = unsafe { (*owner).waiters.next };
    while !current.is_null() {
        let next = unsafe { (*wait_data(current)).link.next };
        if unsafe { (*wait_data(current)).token == mutex as usize } {
            unsafe { queue_remove(core::ptr::addr_of_mut!((*owner).waiters), current) };
            if next_owner.is_null() {
                next_owner = current;
                unsafe {
                    (*current).state =
                        state_after_external_unblock(KTHR_STATE_WAITING_MUTEX, (*current).suspend);
                }
            } else {
                unsafe {
                    queue_insert(core::ptr::addr_of_mut!((*next_owner).waiters), current);
                }
            }
        }
        current = next;
    }
    if !next_owner.is_null() {
        unsafe {
            update_dynamic_priority(owner);
            update_dynamic_priority(next_owner);
        }
    }
    next_owner
}

static mut COND_WAIT_QUEUE: KThrQueue = KThrQueue {
    next: ptr::null_mut(),
    prev: ptr::null_mut(),
};

#[repr(C)]
struct KCondVarTimeout {
    task: KTickTask,
    thread: *mut KThread,
}

unsafe extern "C" fn cond_var_timeout(task: *mut KTickTask) {
    let timeout = task.cast::<KCondVarTimeout>();
    if timeout.is_null() {
        return;
    }
    unsafe {
        k_thr_queue_cancel(ptr::addr_of_mut!(COND_WAIT_QUEUE), (*timeout).thread);
    }
}

#[inline]
fn normalize_priority(priority: u16) -> u16 {
    priority & KTHR_MIN_PRIO
}

#[inline]
fn mul_div_u64(value: u64, numerator: u64, denominator: u64) -> u64 {
    value.wrapping_mul(numerator) / denominator
}

#[inline]
fn ns_to_ticks(ns: u64) -> u64 {
    // 60_750_000 / 1_000_000_000 reduces by 250_000.
    mul_div_u64(ns, 243, 4_000)
}

#[inline]
fn us_to_ticks(us: u64) -> u64 {
    // 60_750_000 / 1_000_000 reduces by 250_000.
    mul_div_u64(us, 243, 4)
}

#[inline]
fn ms_to_ticks(ms: u64) -> u64 {
    // 60_750_000 / 1_000 reduces by 250.
    mul_div_u64(ms, 243_000, 4)
}

unsafe extern "C" fn thread_adapter(argument: *mut c_void) -> *mut c_void {
    let object = argument.cast::<KThread>();
    if object.is_null() {
        return usize::MAX as *mut c_void;
    }
    let pc = unsafe { (*object).ctx.pc };
    if pc == 0 {
        return usize::MAX as *mut c_void;
    }
    let entry: unsafe extern "C" fn(*mut c_void) -> isize =
        unsafe { core::mem::transmute(pc as usize) };
    let arg = unsafe { (*object).ctx.gpr[3] as usize as *mut c_void };
    unsafe {
        (*object).state = KTHR_STATE_RUNNING;
        (*object).suspend = 0;
    }
    let result = unsafe { entry(arg) };
    unsafe {
        thread_list_remove(object);
        (*object).state = KTHR_STATE_FINISHED;
        set_finish_result(object, result);
    }
    result as usize as *mut c_void
}

/// # Safety
/// All non-null pointers must remain valid for the created thread's lifetime;
/// `stack_top` must delimit uniquely owned writable stack storage.
pub unsafe fn k_thread_prepare(
    t: *mut KThread,
    entrypoint: KThreadFn,
    arg: *mut c_void,
    stack_top: *mut c_void,
    prio: u16,
) {
    if t.is_null() || entrypoint.is_none() || stack_top.is_null() {
        return;
    }
    unsafe {
        ensure_main_thread_registered();
        *t = KThread::empty();
        let mut handle = lwp::LWP_THREAD_NULL;
        let rc = lwp::create_thread_at_top(
            &mut handle,
            Some(thread_adapter),
            t.cast(),
            stack_top,
            to_lwp_priority(prio),
            true,
        );
        if rc != lwp::LWP_SUCCESSFUL {
            return;
        }
        if let Some(context) = lwp::thread_context_snapshot(handle) {
            (*t).ctx = PpcContext {
                pc: entrypoint.unwrap() as usize as u32,
                msr: context.msr,
                cr: context.cr,
                lr: k_thread_exit as *const () as usize as u32,
                ctr: context.ctr,
                xer: context.xer,
                gpr: context.gpr,
                fpscr: context.fpscr,
                fpr: context.fpr,
                gqr: context.gqr,
                ps: context.ps,
            };
            (*t).ctx.gpr[3] = arg as usize as u32;
        }
        (*t).state = KTHR_STATE_WAITING;
        (*t).suspend = 1;
        (*t).prio = normalize_priority(prio);
        (*t).baseprio = (*t).prio;
        (*t).data = KThreadWaitFinish::zeroed();
        let _ = lwp::set_thread_context_mirror(handle, ptr::addr_of_mut!((*t).ctx).cast());
        let _ = lwp::initialize_external_reent((*t).reent.as_mut_ptr().cast());
        let _ = lwp::set_thread_reent_storage(handle, (*t).reent.as_mut_ptr().cast());
        thread_list_insert(t);
    }
}

/// Return a copy of a thread's last saved register context.
///
/// The running thread's current register state is live in the exception frame
/// and is not available from the scheduler snapshot.
///
/// # Safety
/// `thread` must be null or point to a live thread object managed by this crate.
pub unsafe fn k_thread_context_snapshot(thread: *mut KThread) -> Option<PpcContext> {
    if thread.is_null() {
        return None;
    }
    let Some(handle) = (unsafe { lwp_handle_for_object(thread) }) else {
        return Some(unsafe { (*thread).ctx });
    };
    lwp::thread_context_snapshot(handle).map(|context| PpcContext {
        pc: context.pc,
        msr: context.msr,
        cr: context.cr,
        lr: context.lr,
        ctr: context.ctr,
        xer: context.xer,
        gpr: context.gpr,
        fpscr: context.fpscr,
        fpr: context.fpr,
        gqr: context.gqr,
        ps: context.ps,
    })
}

/// # Safety
/// `t` must point to a live thread object previously prepared by this module.
pub unsafe fn k_thread_resume(t: *mut KThread) {
    if t.is_null() {
        return;
    }
    unsafe {
        if (*t).suspend == 0 {
            return;
        }
        let Some(handle) = lwp_handle_for_object(t) else {
            return;
        };
        if lwp::resume_thread(handle) == lwp::LWP_SUCCESSFUL {
            (*t).suspend -= 1;
            if (*t).suspend == 0
                && (*t).state == KTHR_STATE_WAITING
                && (*wait_data(t)).queue.is_null()
            {
                (*t).state = KTHR_STATE_RUNNING;
            }
        }
    }
}

/// # Safety
/// `t` must point to a live thread object previously prepared by this module.
pub unsafe fn k_thread_suspend(t: *mut KThread) {
    if t.is_null() {
        return;
    }
    unsafe {
        let Some(handle) = lwp_handle_for_object(t) else {
            return;
        };
        let previous_suspend = (*t).suspend;
        let previous_state = (*t).state;
        let rc = lwp::suspend_thread(handle);
        if rc == lwp::LWP_SUCCESSFUL || rc == lwp::LWP_ALREADY_SUSPENDED {
            (*t).suspend = (*t).suspend.saturating_add(1);
            if previous_suspend == 0 && previous_state == KTHR_STATE_RUNNING {
                (*t).state = KTHR_STATE_WAITING;
            }
        }
    }
}

/// # Safety
/// `t` must point to a live thread object previously prepared by this module.
pub unsafe fn k_thread_set_prio(t: *mut KThread, prio: u16) {
    if t.is_null() {
        return;
    }
    unsafe {
        let Some(handle) = lwp_handle_for_object(t) else {
            return;
        };
        let normalized = normalize_priority(prio);
        let queue = (*wait_data(t)).queue;
        if !queue.is_null() {
            queue_remove(queue, t);
        }
        thread_list_remove(t);
        (*t).prio = normalized;
        (*t).baseprio = normalized;
        thread_list_insert(t);
        if !queue.is_null() {
            queue_insert(queue, t);
        }
        lwp::set_thread_priority(handle, to_lwp_priority(normalized) as u32);
    }
}

#[inline]
fn join_completed_result(state: u16, rc: isize) -> Option<isize> {
    (state < KTHR_STATE_RUNNING).then_some(rc)
}

/// # Safety
/// `t` must point to a live, joinable thread object.
pub unsafe fn k_thread_join(t: *mut KThread) -> isize {
    if t.is_null() {
        return -1;
    }
    unsafe {
        if let Some(result) = join_completed_result((*t).state, finish_result(t)) {
            return result;
        }
        let Some(handle) = lwp_handle_for_object(t) else {
            return -1;
        };
        let mut result = ptr::null_mut();
        if lwp::join_thread(handle, &mut result) != lwp::LWP_SUCCESSFUL {
            return -1;
        }
        (*t).state = KTHR_STATE_FINISHED;
        set_finish_result(t, result as isize);
        result as isize
    }
}

pub fn k_thread_yield() {
    lwp::yield_thread();
}
pub fn k_thread_sleep_ticks(ticks: u64) {
    lwp::sleep_ticks(ticks);
}

/// Returns the currently running Tuxedo thread object.
pub fn k_thread_get_self() -> *mut KThread {
    unsafe { current_object() }
}

/// Returns the first live Tuxedo thread in the authoritative intrusive list.
///
/// This keeps debugger/thread-enumeration clients from recreating scheduler
/// lookup machinery or dereferencing arbitrary caller-provided thread IDs.
pub fn k_thread_first() -> *mut KThread {
    unsafe {
        ensure_main_thread_registered();
        FIRST_THREAD
    }
}

/// Returns the next live Tuxedo thread after `thread`, or null if `thread` is
/// not in the authoritative list or is the final entry.
///
/// # Safety
/// `thread` may be null. Non-null pointers are compared against the managed
/// list before they are dereferenced.
pub unsafe fn k_thread_next(thread: *mut KThread) -> *mut KThread {
    if thread.is_null() {
        return k_thread_first();
    }
    unsafe {
        ensure_main_thread_registered();
        let mut current = FIRST_THREAD;
        while !current.is_null() {
            if current == thread {
                return (*current).next;
            }
            current = (*current).next;
        }
    }
    ptr::null_mut()
}

/// Finds a live thread by the 30-bit physical-address identifier used by the
/// libogc GDB stub. Unknown IDs return null instead of reproducing C's
/// arbitrary-pointer dereference behavior.
pub fn k_thread_find_physical(id: u32) -> *mut KThread {
    if id == 0 {
        return ptr::null_mut();
    }
    unsafe {
        ensure_main_thread_registered();
        let mut current = FIRST_THREAD;
        while !current.is_null() {
            if (current as usize as u32 & 0x3fff_ffff) == id {
                return current;
            }
            current = (*current).next;
        }
    }
    ptr::null_mut()
}

pub fn k_thread_sleep_us(us: u64) {
    k_thread_sleep_ticks(us_to_ticks(us));
}

pub fn k_thread_sleep_ms(ms: u64) {
    k_thread_sleep_ticks(ms_to_ticks(ms));
}
pub extern "C" fn k_thread_exit(rc: isize) {
    unsafe {
        let current = current_object();
        if !current.is_null() {
            thread_list_remove(current);
            (*current).state = KTHR_STATE_FINISHED;
            set_finish_result(current, rc);
        }
    }
    lwp::thread_exit(rc as usize as *mut c_void);
}

/// # Safety
/// `q` must point to a live queue object for the entire blocking operation.
pub unsafe fn k_thr_queue_block(q: *mut KThrQueue, token: u32) -> u32 {
    if q.is_null() {
        return 0;
    }
    let irq = lwp::critical_enter();
    unsafe {
        let object = current_object();
        if object.is_null() {
            lwp::critical_exit(irq);
            return 0;
        }
        let wait = &mut *wait_data(object);
        (*object).state = KTHR_STATE_WAITING;
        wait.token = token as usize;
        queue_insert(q, object);
        let result = lwp::block_current_external(q as usize, token);
        wait.token = result as usize;
        (*object).state = KTHR_STATE_RUNNING;
        lwp::critical_exit(irq);
        result
    }
}

unsafe fn unblock_queue(q: *mut KThrQueue, reference: u32, mask: bool, all: bool) {
    if q.is_null() {
        return;
    }
    let irq = lwp::critical_enter();
    let mut current = unsafe { (*q).next };
    while !current.is_null() {
        let next = unsafe { (*wait_data(current)).link.next };
        let token = unsafe { (*wait_data(current)).token as u32 };
        let matched = if mask {
            (token & reference) != 0
        } else {
            token == reference
        };
        if matched {
            unsafe { queue_remove(q, current) };
            let delivered = if mask { token & reference } else { 1 };
            unsafe {
                (*wait_data(current)).token = delivered as usize;
                (*current).state =
                    state_after_external_unblock((*current).state, (*current).suspend);
            }
            if let Some(handle) = unsafe { lwp_handle_for_object(current) } {
                let _ = lwp::wake_thread_with_token(handle, delivered);
            }
            if !all {
                break;
            }
        }
        current = next;
    }
    lwp::critical_exit(irq);
}

/// # Safety
/// `q` must point to a live queue object.
pub unsafe fn k_thr_queue_unblock_one_by_value(q: *mut KThrQueue, reference: u32) {
    unsafe { unblock_queue(q, reference, false, false) };
}
/// # Safety
/// `q` must point to a live queue object.
pub unsafe fn k_thr_queue_unblock_one_by_mask(q: *mut KThrQueue, reference: u32) {
    unsafe { unblock_queue(q, reference, true, false) };
}
/// # Safety
/// `q` must point to a live queue object.
pub unsafe fn k_thr_queue_unblock_all_by_value(q: *mut KThrQueue, reference: u32) {
    unsafe { unblock_queue(q, reference, false, true) };
}
/// # Safety
/// `q` must point to a live queue object.
pub unsafe fn k_thr_queue_unblock_all_by_mask(q: *mut KThrQueue, reference: u32) {
    unsafe { unblock_queue(q, reference, true, true) };
}

/// # Safety
/// `q` and `t` must point to live queue and thread objects owned by this module.
pub unsafe fn k_thr_queue_cancel(q: *mut KThrQueue, t: *mut KThread) {
    if q.is_null() || t.is_null() {
        return;
    }
    let irq = lwp::critical_enter();
    unsafe {
        if (*t).state == KTHR_STATE_WAITING && (*wait_data(t)).queue == q {
            queue_remove(q, t);
            (*wait_data(t)).token = 0;
            (*t).state = state_after_external_unblock((*t).state, (*t).suspend);
            if let Some(handle) = lwp_handle_for_object(t) {
                lwp::cancel_external_wait(q as usize, handle);
            }
        }
    }
    lwp::critical_exit(irq);
}

/// # Safety
/// `m` must point to a live mutex object that remains valid while registered.
pub unsafe fn k_mutex_try_lock(m: *mut KMutex) -> bool {
    if m.is_null() {
        return false;
    }
    let irq = lwp::critical_enter();
    let current = unsafe { current_object() };
    let acquired = unsafe {
        if (*m).owner.is_null() && !current.is_null() {
            (*m).owner = current;
            true
        } else {
            false
        }
    };
    lwp::critical_exit(irq);
    acquired
}

/// # Safety
/// `m` must point to a live mutex object that remains valid while registered.
pub unsafe fn k_mutex_lock(m: *mut KMutex) {
    if m.is_null() {
        return;
    }
    loop {
        let irq = lwp::critical_enter();
        let current = unsafe { current_object() };
        if current.is_null() {
            lwp::critical_exit(irq);
            return;
        }
        let owner = unsafe { (*m).owner };
        if owner.is_null() {
            unsafe { (*m).owner = current };
            lwp::critical_exit(irq);
            return;
        }

        unsafe {
            (*current).state = KTHR_STATE_WAITING_MUTEX;
            (*wait_data(current)).token = m as usize;
            queue_insert(core::ptr::addr_of_mut!((*owner).waiters), current);
            if (*current).prio < (*owner).prio {
                update_dynamic_priority(owner);
            }
        }
        lwp::critical_exit(irq);
        let _ = lwp::block_current_external(m as usize, m as usize as u32);
        if unsafe { (*m).owner == current } {
            return;
        }
    }
}

/// # Safety
/// `m` must point to the live mutex object held by the current thread.
pub unsafe fn k_mutex_unlock(m: *mut KMutex) {
    if m.is_null() {
        return;
    }
    let irq = lwp::critical_enter();
    let current = unsafe { current_object() };
    if current.is_null() || unsafe { (*m).owner != current } {
        // Safety divergence: C spins forever on invalid ownership. Returning
        // avoids turning a misuse into an unrecoverable scheduler deadlock.
        lwp::critical_exit(irq);
        return;
    }

    let next_owner = unsafe { remove_mutex_waiters(current, m) };
    unsafe { (*m).owner = next_owner };
    if !next_owner.is_null()
        && let Some(handle) = unsafe { lwp_handle_for_object(next_owner) }
    {
        let _ = lwp::wake_thread_with_token(handle, 1);
    }
    lwp::critical_exit(irq);
}

/// Returns whether `m` is owned by the current Tuxedo thread.
///
/// # Safety
/// `m` must be null or point to a live mutex object.
pub unsafe fn k_mutex_is_locked_by_current_thread(m: *mut KMutex) -> bool {
    if m.is_null() {
        return false;
    }
    let current = unsafe { current_object() };
    !current.is_null() && unsafe { (*m).owner == current }
}

/// # Safety
/// `m` must point to a live recursive mutex object.
pub unsafe fn k_rmutex_lock(m: *mut KRMutex) {
    if m.is_null() {
        return;
    }
    if unsafe { k_mutex_is_locked_by_current_thread(ptr::addr_of_mut!((*m).mutex)) } {
        unsafe { (*m).counter = (*m).counter.wrapping_add(1) };
    } else {
        unsafe { k_mutex_lock(ptr::addr_of_mut!((*m).mutex)) };
        unsafe { (*m).counter = 1 };
    }
}

/// # Safety
/// `m` must point to a live recursive mutex object.
pub unsafe fn k_rmutex_try_lock(m: *mut KRMutex) -> bool {
    if m.is_null() {
        return false;
    }
    if unsafe { k_mutex_is_locked_by_current_thread(ptr::addr_of_mut!((*m).mutex)) } {
        unsafe { (*m).counter = (*m).counter.wrapping_add(1) };
        true
    } else if unsafe { k_mutex_try_lock(ptr::addr_of_mut!((*m).mutex)) } {
        unsafe { (*m).counter = 1 };
        true
    } else {
        false
    }
}

/// # Safety
/// `m` must point to a live recursive mutex currently held by this thread.
pub unsafe fn k_rmutex_unlock(m: *mut KRMutex) {
    if m.is_null() {
        return;
    }
    let counter = unsafe { (*m).counter };
    if counter == 0 {
        // Safety divergence: C underflows and leaves the mutex in an invalid
        // recursive state. Ignore the invalid unlock instead.
        return;
    }
    let next = counter - 1;
    unsafe { (*m).counter = next };
    if next == 0 {
        unsafe { k_mutex_unlock(ptr::addr_of_mut!((*m).mutex)) };
    }
}

/// # Safety
/// `cv` must point to a live condition-variable object.
pub unsafe fn k_cond_var_signal(cv: *mut KCondVar) {
    if cv.is_null() {
        return;
    }
    unsafe {
        k_thr_queue_unblock_one_by_value(ptr::addr_of_mut!(COND_WAIT_QUEUE), cv as usize as u32);
    }
}

/// # Safety
/// `cv` must point to a live condition-variable object.
pub unsafe fn k_cond_var_broadcast(cv: *mut KCondVar) {
    if cv.is_null() {
        return;
    }
    unsafe {
        k_thr_queue_unblock_all_by_value(ptr::addr_of_mut!(COND_WAIT_QUEUE), cv as usize as u32);
    }
}

/// # Safety
/// `cv` and `m` must point to live objects, and the current thread must hold `m`.
pub unsafe fn k_cond_var_wait(cv: *mut KCondVar, m: *mut KMutex) {
    if cv.is_null() || m.is_null() {
        return;
    }
    let current = unsafe { current_object() };
    if current.is_null() || unsafe { (*m).owner != current } {
        return;
    }
    unsafe {
        k_mutex_unlock(m);
        let _ = k_thr_queue_block(ptr::addr_of_mut!(COND_WAIT_QUEUE), cv as usize as u32);
        k_mutex_lock(m);
    }
}

/// # Safety
/// `cv` and `m` must point to live objects, and the current thread must hold `m`.
pub unsafe fn k_cond_var_wait_timeout_ticks(
    cv: *mut KCondVar,
    m: *mut KMutex,
    timeout_ticks: u64,
) -> bool {
    if cv.is_null() || m.is_null() || timeout_ticks == 0 {
        return false;
    }
    let current = unsafe { current_object() };
    if current.is_null() || unsafe { (*m).owner != current } {
        return false;
    }
    let mut timeout = KCondVarTimeout {
        task: KTickTask::default(),
        thread: current,
    };
    unsafe {
        k_mutex_unlock(m);
        k_tick_task_start(&mut timeout.task, Some(cond_var_timeout), timeout_ticks, 0);
        let signaled =
            k_thr_queue_block(ptr::addr_of_mut!(COND_WAIT_QUEUE), cv as usize as u32) != 0;
        k_tick_task_stop(&mut timeout.task);
        k_mutex_lock(m);
        signaled
    }
}

/// # Safety
/// Same contract as [`k_cond_var_wait_timeout_ticks`].
pub unsafe fn k_cond_var_wait_ns(cv: *mut KCondVar, m: *mut KMutex, timeout_ns: u64) -> bool {
    unsafe { k_cond_var_wait_timeout_ticks(cv, m, ns_to_ticks(timeout_ns)) }
}

/// # Safety
/// Same contract as [`k_cond_var_wait_timeout_ticks`].
pub unsafe fn k_cond_var_wait_us(cv: *mut KCondVar, m: *mut KMutex, timeout_us: u64) -> bool {
    unsafe { k_cond_var_wait_timeout_ticks(cv, m, us_to_ticks(timeout_us)) }
}

/// # Safety
/// Same contract as [`k_cond_var_wait_timeout_ticks`].
pub unsafe fn k_cond_var_wait_ms(cv: *mut KCondVar, m: *mut KMutex, timeout_ms: u64) -> bool {
    unsafe { k_cond_var_wait_timeout_ticks(cv, m, ms_to_ticks(timeout_ms)) }
}

static mut FIRST_TICK_TASK: *mut KTickTask = ptr::null_mut();

#[inline]
fn tick_is_sequential(lhs: u64, rhs: u64) -> bool {
    (rhs.wrapping_sub(lhs) as i64) > 0
}

unsafe fn tick_task_enqueue(first: *mut *mut KTickTask, task: *mut KTickTask) {
    let mut position = ptr::null_mut();
    let mut current = unsafe { *first };
    while !current.is_null()
        && tick_is_sequential(unsafe { (*current).target }, unsafe { (*task).target })
    {
        position = current;
        current = unsafe { (*current).next };
    }

    if position.is_null() {
        unsafe {
            (*task).next = *first;
            *first = task;
        }
    } else {
        unsafe {
            (*task).next = (*position).next;
            (*position).next = task;
        }
    }
}

unsafe fn tick_task_dequeue(first: *mut *mut KTickTask, task: *mut KTickTask) {
    if unsafe { *first == task } {
        unsafe { *first = (*task).next };
        return;
    }

    let mut current = unsafe { *first };
    while !current.is_null() {
        if unsafe { (*current).next == task } {
            unsafe { (*current).next = (*task).next };
            return;
        }
        current = unsafe { (*current).next };
    }
}

unsafe fn prepare_tick_callback(task: *mut KTickTask) -> KTickTaskFn {
    let callback = unsafe { (*task).fn_ };
    callback?;

    let first = ptr::addr_of_mut!(FIRST_TICK_TASK);
    unsafe { tick_task_dequeue(first, task) };
    if unsafe { (*task).period == 0 } {
        // C clears one-shot tasks before calling them, so callbacks observe the
        // same public task state as KTickTaskRun.
        unsafe { (*task).fn_ = None };
    } else {
        // The timer engine has already advanced its deadline by one period.
        // Keep the public task deadline and intrusive link on that same schedule.
        unsafe { (*task).target = (*task).target.wrapping_add((*task).period) };
        unsafe { tick_task_enqueue(first, task) };
    }
    callback
}

unsafe fn tick_task_adapter(argument: *mut c_void) {
    let task = argument.cast::<KTickTask>();
    if task.is_null() {
        return;
    }
    let callback = unsafe { prepare_tick_callback(task) };
    if let Some(callback) = callback {
        unsafe { callback(task) };
    }
}

/// # Safety
/// `t` must remain valid until the task is stopped and any running callback completes.
pub unsafe fn k_tick_task_start(
    t: *mut KTickTask,
    fn_: KTickTaskFn,
    delay_ticks: u64,
    period_ticks: u64,
) {
    if t.is_null() || fn_.is_none() {
        return;
    }
    let irq = lwp::critical_enter();
    unsafe {
        let first = ptr::addr_of_mut!(FIRST_TICK_TASK);
        if (*t).fn_.is_some() {
            tick_task_dequeue(first, t);
        }
        (*t).fn_ = fn_;
        (*t).target = lwp::now_ticks().wrapping_add(delay_ticks);
        (*t).period = period_ticks;
        let result = lwp::timer_start(
            t as usize,
            Some(tick_task_adapter),
            t.cast(),
            delay_ticks,
            period_ticks,
        );
        if result == lwp::LWP_SUCCESSFUL {
            tick_task_enqueue(first, t);
        } else {
            (*t).fn_ = None;
            (*t).next = ptr::null_mut();
        }
    }
    lwp::critical_exit(irq);
}

/// # Safety
/// `t` must identify a live task previously started by this module.
pub unsafe fn k_tick_task_stop(t: *mut KTickTask) {
    if t.is_null() {
        return;
    }
    let irq = lwp::critical_enter();
    unsafe {
        if (*t).fn_.is_none() {
            lwp::critical_exit(irq);
            return;
        }
        lwp::timer_cancel(t as usize);
        tick_task_dequeue(ptr::addr_of_mut!(FIRST_TICK_TASK), t);
        // C only clears fn. target, period, and next keep their last values.
        (*t).fn_ = None;
    }
    lwp::critical_exit(irq);
}

/// # Safety
/// Same contract as [`k_thread_prepare`].
pub unsafe fn thread_prepare(
    t: *mut KThread,
    entrypoint: KThreadFn,
    arg: *mut c_void,
    stack_top: *mut c_void,
    prio: u16,
) {
    unsafe { k_thread_prepare(t, entrypoint, arg, stack_top, prio) };
}
/// # Safety
/// Same contract as [`k_thread_resume`].
pub unsafe fn thread_resume(t: *mut KThread) {
    unsafe { k_thread_resume(t) };
}
/// # Safety
/// Same contract as [`k_mutex_try_lock`].
pub unsafe fn mutex_try_lock(m: *mut KMutex) -> bool {
    unsafe { k_mutex_try_lock(m) }
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::{
        KTHR_STATE_FINISHED, KTHR_STATE_RUNNING, KTHR_STATE_UNINIT, KTHR_STATE_WAITING, KThrQueue,
        KThread, KTickTask, join_completed_result, ms_to_ticks, normalize_priority, ns_to_ticks,
        prepare_tick_callback, queue_insert, queue_remove, state_after_external_unblock,
        tick_task_dequeue, tick_task_enqueue, us_to_ticks, wait_data,
    };

    #[test]
    fn join_returns_stored_result_without_wait_for_unstarted_or_finished_threads() {
        assert_eq!(join_completed_result(KTHR_STATE_UNINIT, 17), Some(17));
        assert_eq!(join_completed_result(KTHR_STATE_FINISHED, -9), Some(-9));
        assert_eq!(join_completed_result(KTHR_STATE_RUNNING, 5), None);
        assert_eq!(join_completed_result(KTHR_STATE_WAITING, 5), None);
    }

    #[test]
    fn external_unblock_preserves_suspended_public_state() {
        assert_eq!(
            state_after_external_unblock(KTHR_STATE_WAITING, 1),
            KTHR_STATE_WAITING
        );
        assert_eq!(
            state_after_external_unblock(KTHR_STATE_WAITING, 0),
            KTHR_STATE_RUNNING
        );
    }
    #[test]
    fn tick_conversions_match_tuxedo_clock_helpers() {
        assert_eq!(ns_to_ticks(1_000_000_000), 60_750_000);
        assert_eq!(us_to_ticks(1_000_000), 60_750_000);
        assert_eq!(ms_to_ticks(1_000), 60_750_000);
        assert_eq!(us_to_ticks(1), 60);
        assert_eq!(ms_to_ticks(1), 60_750);
    }

    #[test]
    fn priorities_match_c_seven_bit_mask() {
        assert_eq!(normalize_priority(0x80), 0);
        assert_eq!(normalize_priority(0xFF), 0x7F);
        assert_eq!(normalize_priority(0x180), 0);
        assert_eq!(normalize_priority(0x1FF), 0x7F);
        assert_eq!(normalize_priority(u16::MAX), 0x7F);
    }

    #[test]
    fn intrusive_queue_keeps_priority_and_fifo_order() {
        let mut queue = KThrQueue::default();
        let mut first = KThread::empty();
        let mut equal = KThread::empty();
        let mut higher = KThread::empty();
        first.prio = 20;
        equal.prio = 20;
        higher.prio = 10;

        unsafe {
            queue_insert(&mut queue, &mut first);
            queue_insert(&mut queue, &mut equal);
            queue_insert(&mut queue, &mut higher);
        }
        assert_eq!(queue.next, &mut higher as *mut KThread);
        assert_eq!(
            unsafe { (*wait_data(&mut higher)).link.next },
            &mut first as *mut KThread
        );
        assert_eq!(
            unsafe { (*wait_data(&mut first)).link.next },
            &mut equal as *mut KThread
        );
        assert_eq!(queue.prev, &mut equal as *mut KThread);
    }

    #[test]
    fn priority_change_requeue_moves_after_existing_equal_priority_waiter() {
        let mut queue = KThrQueue::default();
        let mut changed = KThread::empty();
        let mut existing = KThread::empty();
        changed.prio = 10;
        existing.prio = 20;
        unsafe {
            queue_insert(&mut queue, &mut changed);
            queue_insert(&mut queue, &mut existing);
            queue_remove(&mut queue, &mut changed);
            changed.prio = 20;
            queue_insert(&mut queue, &mut changed);
        }
        assert_eq!(queue.next, &mut existing as *mut KThread);
        assert_eq!(queue.prev, &mut changed as *mut KThread);
    }

    #[test]
    fn tick_callback_state_matches_c_before_callback_runs() {
        let mut periodic = KTickTask {
            fn_: Some(test_tick_callback),
            target: u64::MAX - 2,
            period: 5,
            ..KTickTask::default()
        };
        let mut first = &mut periodic as *mut KTickTask;
        unsafe { super::FIRST_TICK_TASK = first };
        assert!(unsafe { prepare_tick_callback(&mut periodic) }.is_some());
        assert_eq!(periodic.target, 2);
        assert_eq!(periodic.period, 5);

        let mut one_shot = KTickTask {
            fn_: Some(test_tick_callback),
            target: 10,
            period: 0,
            ..KTickTask::default()
        };
        first = &mut one_shot as *mut KTickTask;
        unsafe { super::FIRST_TICK_TASK = first };
        assert!(unsafe { prepare_tick_callback(&mut one_shot) }.is_some());
        assert!(one_shot.fn_.is_none());
    }

    #[test]
    fn tick_task_links_match_c_intrusive_queue_order() {
        let mut first = core::ptr::null_mut();
        let mut late = KTickTask {
            target: 20,
            ..KTickTask::default()
        };
        let mut equal_old = KTickTask {
            target: 10,
            ..KTickTask::default()
        };
        let mut equal_new = KTickTask {
            target: 10,
            ..KTickTask::default()
        };

        unsafe {
            tick_task_enqueue(&mut first, &mut late);
            tick_task_enqueue(&mut first, &mut equal_old);
            tick_task_enqueue(&mut first, &mut equal_new);
        }
        assert_eq!(first, &mut equal_new as *mut KTickTask);
        assert_eq!(equal_new.next, &mut equal_old as *mut KTickTask);
        assert_eq!(equal_old.next, &mut late as *mut KTickTask);
        assert!(late.next.is_null());

        unsafe { tick_task_dequeue(&mut first, &mut equal_old) };
        assert_eq!(first, &mut equal_new as *mut KTickTask);
        assert_eq!(equal_new.next, &mut late as *mut KTickTask);
        // C does not clear the removed task's next link.
        assert_eq!(equal_old.next, &mut late as *mut KTickTask);
    }

    unsafe extern "C" fn test_tick_callback(_: *mut KTickTask) {}
}
