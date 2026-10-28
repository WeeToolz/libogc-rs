#![no_std]

//! ABI-oriented facade over the authoritative Wii LWP driver.

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::ffi::c_void;

pub use ogc_lwp_driver::{
    ThreadCleanupCounters, TlsDestructor, TlsError, TlsKey, thread_cleanup_counters, tls_create,
    tls_destroy, tls_get, tls_set,
};

pub use ogc_lwp_driver::{
    DebugCounters, DebugEvent, DebugHook, DeferredCallback, LWP_ALREADY_SUSPENDED, LWP_CLOSED,
    LWP_COND_NULL, LWP_MUTEX_NULL, LWP_NOT_SUSPENDED, LWP_PRIO_HIGHEST, LWP_PRIO_IDLE,
    LWP_SEM_NULL, LWP_SUCCESSFUL, LWP_THREAD_NULL, LWP_TQUEUE_NULL, MQ_BOX_NULL,
    MQ_ERROR_SUCCESSFUL, MQ_ERROR_TOOMANY, MQ_MSG_BLOCK, MQ_MSG_NOBLOCK, TICKS_PER_SECOND,
};
pub use ogc_sys::Bool;

/// Initializes the Rust-owned scheduler, timer, and deferred-callback state.
pub fn initialize() {
    ogc_lwp_driver::initialize();
}

/// Registers the workspace allocator used by dynamic LWP compatibility state.
pub fn set_workspace_allocator(
    alloc: ogc_lwp_driver::WorkspaceAlloc,
    free: ogc_lwp_driver::WorkspaceFree,
) {
    ogc_lwp_driver::set_workspace_allocator(alloc, free);
}

/// Allocates raw dynamic storage from the shared LWP workspace.
///
/// # Safety
/// The allocation must be released with [`release_workspace`] using the same
/// `size`.
pub unsafe fn allocate_workspace(size: usize) -> *mut u8 {
    unsafe { ogc_lwp_driver::allocate_workspace(size) }
}

/// Releases raw storage obtained from [`allocate_workspace`].
///
/// # Safety
/// `allocation` and `size` must describe a live allocation returned by
/// [`allocate_workspace`].
pub unsafe fn release_workspace(allocation: *mut u8, size: usize) {
    unsafe { ogc_lwp_driver::release_workspace(allocation, size) };
}

pub type LwpThread = ogc_lwp_driver::LwpThread;
pub type LwpQueue = ogc_lwp_driver::QueueHandle;
pub type Mutex = ogc_lwp_driver::MutexHandle;
pub type CondVar = ogc_lwp_driver::CondHandle;
pub type Semaphore = ogc_lwp_driver::SemaphoreHandle;
pub type MessageQueue = ogc_lwp_driver::MessageQueueHandle;
pub type Message = *mut c_void;
pub type ThreadEntry = ogc_lwp_driver::ThreadEntry;
pub type KeyedTimerCallback = ogc_lwp_driver::KeyedTimerCallback;

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct TimeSpec {
    pub tv_sec: i64,
    pub tv_nsec: i32,
}

unsafe fn relative_ticks(reltime: *const TimeSpec) -> Result<Option<u64>, i32> {
    if reltime.is_null() {
        return Ok(None);
    }
    let value = unsafe { *reltime };
    // C performs this conversion without validating timespec ranges. Signed
    // fields therefore follow the platform's unsigned conversion/wrap rules.
    let seconds = (value.tv_sec as u64).wrapping_mul(TICKS_PER_SECOND);
    let nanos = (value.tv_nsec as u64).wrapping_mul(TICKS_PER_SECOND) / 1_000_000_000;
    Ok(Some(seconds.wrapping_add(nanos)))
}

/// Creates a thread using optional caller-owned stack storage.
///
/// # Safety
/// `thread` must be writable. A non-null `stackbase` must reference an aligned,
/// uniquely owned `stack_size`-byte region that outlives the thread. `arg` must
/// remain valid for however the entry function uses it.
pub unsafe fn create_thread(
    thread: *mut LwpThread,
    entry: ThreadEntry,
    arg: *mut c_void,
    stackbase: *mut c_void,
    stack_size: u32,
    prio: u8,
) -> i32 {
    unsafe { ogc_lwp_driver::create_thread(thread, entry, arg, stackbase, stack_size, prio) }
}

pub fn suspend_thread(thread: LwpThread) -> i32 {
    ogc_lwp_driver::suspend_thread(thread)
}

pub fn resume_thread(thread: LwpThread) -> i32 {
    ogc_lwp_driver::resume_thread(thread)
}

pub fn detach_thread(thread: LwpThread) -> i32 {
    ogc_lwp_driver::detach_thread(thread)
}

/// Joins a thread and optionally returns its result pointer.
///
/// # Safety
/// A non-null `value_ptr` must be writable for one pointer.
pub unsafe fn join_thread(thread: LwpThread, value_ptr: *mut *mut c_void) -> i32 {
    unsafe { ogc_lwp_driver::join_thread(thread, value_ptr) }
}

pub fn thread_is_suspended(thread: LwpThread) -> Bool {
    ogc_lwp_driver::thread_is_suspended(thread) as Bool
}

pub fn get_self() -> LwpThread {
    ogc_lwp_driver::current_thread()
}

pub fn set_thread_priority(thread: LwpThread, prio: u32) {
    ogc_lwp_driver::set_thread_priority(thread, prio);
}

pub fn yield_thread() {
    ogc_lwp_driver::yield_thread();
}

/// Reads the monotonic PowerPC timebase used by LWP timeout operations.
pub fn now_ticks() -> u64 {
    ogc_lwp_driver::now_ticks()
}

/// Register a deferred timer callback that receives its stable timer key.
///
/// # Safety
/// `arg` must remain valid until the timer expires, is cancelled, and any
/// callback already queued for execution completes.
pub unsafe fn timer_start_with_key(
    key: usize,
    callback: KeyedTimerCallback,
    arg: *mut c_void,
    delay: u64,
    period: u64,
) -> i32 {
    unsafe { ogc_lwp_driver::timer_start_with_key(key, callback, arg, delay, period) }
}

pub fn timer_cancel(key: usize) {
    ogc_lwp_driver::timer_cancel(key);
}

pub fn reschedule(prio: u32) {
    ogc_lwp_driver::reschedule(prio);
}

pub fn sleep_ticks(ticks: u64) {
    ogc_lwp_driver::sleep_ticks(ticks);
}

/// # Safety
/// `mutex` must be writable for one mutex handle.
pub unsafe fn mutex_init(mutex: *mut Mutex, use_recursive: Bool) -> i32 {
    unsafe { ogc_lwp_driver::init_mutex(mutex, use_recursive != 0) }
}

pub fn mutex_destroy(mutex: Mutex) -> i32 {
    ogc_lwp_driver::close_mutex(mutex)
}

pub fn mutex_try_lock(mutex: Mutex) -> i32 {
    ogc_lwp_driver::mutex_try_lock(mutex)
}

pub fn mutex_lock(mutex: Mutex) -> i32 {
    ogc_lwp_driver::mutex_lock(mutex)
}

pub fn mutex_unlock(mutex: Mutex) -> i32 {
    ogc_lwp_driver::mutex_unlock(mutex)
}

/// # Safety
/// `cond` must be writable for one condition-variable handle.
pub unsafe fn cond_init(cond: *mut CondVar) -> i32 {
    unsafe { ogc_lwp_driver::init_cond(cond) }
}

pub fn cond_destroy(cond: CondVar) -> i32 {
    ogc_lwp_driver::close_cond(cond)
}

pub fn cond_wait(cond: CondVar, mutex: Mutex) -> i32 {
    ogc_lwp_driver::cond_wait(cond, mutex, None)
}

/// # Safety
/// A non-null `reltime` must point to a valid [`TimeSpec`] for this call.
pub unsafe fn cond_timed_wait(cond: CondVar, mutex: Mutex, reltime: *const TimeSpec) -> i32 {
    match unsafe { relative_ticks(reltime) } {
        Ok(timeout) => map_cond_timed_wait_result(ogc_lwp_driver::cond_wait(cond, mutex, timeout)),
        Err(error) => error,
    }
}

fn map_cond_timed_wait_result(result: i32) -> i32 {
    // LWP_CondTimedWait returns newlib's positive ETIMEDOUT value, although
    // the Rust driver uses a negative timeout status internally.
    if result == -116 { 116 } else { result }
}

pub fn cond_signal(cond: CondVar) -> i32 {
    ogc_lwp_driver::cond_signal(cond)
}

pub fn cond_broadcast(cond: CondVar) -> i32 {
    ogc_lwp_driver::cond_broadcast(cond)
}

#[cfg(all(test, target_arch = "powerpc"))]
mod timed_wait_tests {
    use super::map_cond_timed_wait_result;

    #[test]
    fn timed_condition_wait_returns_newlib_positive_etimedout() {
        assert_eq!(map_cond_timed_wait_result(-116), 116);
        assert_eq!(map_cond_timed_wait_result(0), 0);
        assert_eq!(map_cond_timed_wait_result(-1), -1);
    }
}

/// # Safety
/// `sem` must be writable for one semaphore handle.
pub unsafe fn sem_init(sem: *mut Semaphore, start: u32, max: u32) -> i32 {
    unsafe { ogc_lwp_driver::init_semaphore(sem, start, max) }
}

pub fn sem_destroy(sem: Semaphore) -> i32 {
    ogc_lwp_driver::close_semaphore(sem)
}

pub fn sem_wait(sem: Semaphore) -> i32 {
    ogc_lwp_driver::semaphore_wait(sem)
}

/// # Safety
/// `value` must be writable for one `u32`.
pub unsafe fn sem_get_value(sem: Semaphore, value: *mut u32) -> i32 {
    unsafe { ogc_lwp_driver::semaphore_get_value(sem, value) }
}

pub fn sem_post(sem: Semaphore) -> i32 {
    ogc_lwp_driver::semaphore_post(sem)
}

/// # Safety
/// `mqbox` must be writable for one message-queue handle.
pub unsafe fn mq_init(mqbox: *mut MessageQueue, count: u32) -> i32 {
    unsafe { ogc_lwp_driver::init_message_queue(mqbox, count) }
}

pub fn mq_close(mqbox: MessageQueue) {
    ogc_lwp_driver::close_message_queue(mqbox);
}

pub fn mq_send(mqbox: MessageQueue, msg: Message, flags: u32) -> Bool {
    ogc_lwp_driver::message_send(mqbox, msg, flags) as Bool
}

pub fn mq_jam(mqbox: MessageQueue, msg: Message, flags: u32) -> Bool {
    ogc_lwp_driver::message_jam(mqbox, msg, flags) as Bool
}

/// # Safety
/// A non-null `msg` must be writable for one message pointer.
pub unsafe fn mq_receive(mqbox: MessageQueue, msg: *mut Message, flags: u32) -> Bool {
    unsafe { ogc_lwp_driver::message_receive(mqbox, msg, flags) as Bool }
}

/// # Safety
///
/// `queue` must be writable for one `LwpQueue` value.
pub unsafe fn init_queue(queue: *mut LwpQueue) -> i32 {
    unsafe { ogc_lwp_driver::init_queue(queue) }
}

pub fn close_queue(queue: LwpQueue) {
    ogc_lwp_driver::close_queue(queue);
}

pub fn thread_sleep(queue: LwpQueue) -> i32 {
    ogc_lwp_driver::queue_sleep(queue)
}

pub fn thread_signal(queue: LwpQueue) {
    ogc_lwp_driver::queue_signal(queue);
}

pub fn thread_broadcast(queue: LwpQueue) {
    ogc_lwp_driver::queue_broadcast(queue);
}

/// # Safety
/// `arg` must satisfy the callback's contract and remain valid until it runs.
pub unsafe fn deferred_call(callback: DeferredCallback, arg: *mut c_void) -> i32 {
    unsafe { ogc_lwp_driver::deferred_enqueue(callback, arg) }
}

/// Enqueues a callback from an IOS/hardware interrupt handler. LWP must already
/// be initialized by the submitting thread.
///
/// # Safety
/// `arg` must satisfy the callback's contract and remain valid until it runs.
pub unsafe fn deferred_call_from_interrupt(callback: DeferredCallback, arg: *mut c_void) -> i32 {
    unsafe { ogc_lwp_driver::deferred_enqueue_from_interrupt(callback, arg) }
}

pub fn deferred_reserve_interrupt_slot() -> Option<u32> {
    ogc_lwp_driver::deferred_reserve_interrupt_slot()
}

pub fn deferred_cancel_interrupt_slot(reservation: u32) -> i32 {
    ogc_lwp_driver::deferred_cancel_interrupt_slot(reservation)
}

/// # Safety
/// The caller must own one reservation. `arg` must remain valid until callback.
pub unsafe fn deferred_call_reserved_from_interrupt(
    reservation: u32,
    callback: DeferredCallback,
    arg: *mut c_void,
) -> i32 {
    unsafe { ogc_lwp_driver::deferred_enqueue_reserved_from_interrupt(reservation, callback, arg) }
}

pub fn set_debug_hook(hook: DebugHook) {
    ogc_lwp_driver::set_debug_hook(hook);
}

pub fn debug_counters() -> DebugCounters {
    ogc_lwp_driver::debug_counters()
}

pub fn debug_drain(output: &mut [DebugEvent]) -> usize {
    ogc_lwp_driver::debug_drain(output)
}

/// Records that an IOS completion interrupt woke an LWP queue. This hook is
/// intentionally separate from `thread_signal` so probes can distinguish IRQ
/// wakeups from application-level queue signals.
pub fn note_ipc_wake(queue: LwpQueue) {
    ogc_lwp_driver::note_ipc_wake(queue);
}

// Legacy libogc C ABI -------------------------------------------------------
//
// Keep these wrappers intentionally thin: the snake-case functions above are
// the single implementation of LWP behavior. These symbols only restore the
// public names/calling convention from the libogc headers.

#[unsafe(export_name = "LWP_CreateThread")]
/// Create a thread through the legacy libogc ABI.
///
/// # Safety
///
/// `thread` must be writable. A non-null `stackbase` must reference an aligned,
/// uniquely owned `stack_size`-byte region that outlives the thread. `arg` must
/// remain valid for as long as the entry function uses it.
pub unsafe extern "C" fn lwp_create_thread_abi(
    thread: *mut LwpThread,
    entry: ThreadEntry,
    arg: *mut c_void,
    stackbase: *mut c_void,
    stack_size: u32,
    prio: u8,
) -> i32 {
    unsafe { create_thread(thread, entry, arg, stackbase, stack_size, prio) }
}

#[unsafe(export_name = "LWP_SuspendThread")]
pub extern "C" fn lwp_suspend_thread_abi(thread: LwpThread) -> i32 {
    suspend_thread(thread)
}
#[unsafe(export_name = "LWP_ResumeThread")]
pub extern "C" fn lwp_resume_thread_abi(thread: LwpThread) -> i32 {
    resume_thread(thread)
}
#[unsafe(export_name = "LWP_ThreadIsSuspended")]
pub extern "C" fn lwp_thread_is_suspended_abi(thread: LwpThread) -> Bool {
    thread_is_suspended(thread)
}
#[unsafe(export_name = "LWP_GetSelf")]
pub extern "C" fn lwp_get_self_abi() -> LwpThread {
    get_self()
}
#[unsafe(export_name = "LWP_SetThreadPriority")]
pub extern "C" fn lwp_set_thread_priority_abi(thread: LwpThread, prio: u32) {
    set_thread_priority(thread, prio);
}
#[unsafe(export_name = "LWP_YieldThread")]
pub extern "C" fn lwp_yield_thread_abi() {
    yield_thread();
}
#[unsafe(export_name = "LWP_Reschedule")]
pub extern "C" fn lwp_reschedule_abi(prio: u32) {
    reschedule(prio);
}
#[unsafe(export_name = "LWP_JoinThread")]
/// Join a thread through the legacy libogc ABI.
///
/// # Safety
///
/// A non-null `value_ptr` must be writable for one pointer.
pub unsafe extern "C" fn lwp_join_thread_abi(
    thread: LwpThread,
    value_ptr: *mut *mut c_void,
) -> i32 {
    unsafe { join_thread(thread, value_ptr) }
}
#[unsafe(export_name = "LWP_InitQueue")]
/// Initialize a queue through the legacy libogc ABI.
///
/// # Safety
///
/// `queue` must be writable for one queue handle.
pub unsafe extern "C" fn lwp_init_queue_abi(queue: *mut LwpQueue) -> i32 {
    unsafe { init_queue(queue) }
}
#[unsafe(export_name = "LWP_CloseQueue")]
pub extern "C" fn lwp_close_queue_abi(queue: LwpQueue) {
    close_queue(queue);
}
#[unsafe(export_name = "LWP_ThreadSleep")]
pub extern "C" fn lwp_thread_sleep_abi(queue: LwpQueue) -> i32 {
    thread_sleep(queue)
}
#[unsafe(export_name = "LWP_ThreadSignal")]
pub extern "C" fn lwp_thread_signal_abi(queue: LwpQueue) {
    thread_signal(queue);
}
#[unsafe(export_name = "LWP_ThreadBroadcast")]
pub extern "C" fn lwp_thread_broadcast_abi(queue: LwpQueue) {
    thread_broadcast(queue);
}

#[unsafe(export_name = "LWP_MutexInit")]
/// Initialize a mutex through the legacy libogc ABI.
///
/// # Safety
///
/// `mutex` must be writable for one mutex handle.
pub unsafe extern "C" fn lwp_mutex_init_abi(mutex: *mut Mutex, use_recursive: u8) -> i32 {
    unsafe { mutex_init(mutex, (use_recursive != 0) as Bool) }
}
#[unsafe(export_name = "LWP_MutexDestroy")]
pub extern "C" fn lwp_mutex_destroy_abi(mutex: Mutex) -> i32 {
    mutex_destroy(mutex)
}
#[unsafe(export_name = "LWP_MutexLock")]
pub extern "C" fn lwp_mutex_lock_abi(mutex: Mutex) -> i32 {
    mutex_lock(mutex)
}
#[unsafe(export_name = "LWP_MutexTryLock")]
pub extern "C" fn lwp_mutex_try_lock_abi(mutex: Mutex) -> i32 {
    mutex_try_lock(mutex)
}
#[unsafe(export_name = "LWP_MutexUnlock")]
pub extern "C" fn lwp_mutex_unlock_abi(mutex: Mutex) -> i32 {
    mutex_unlock(mutex)
}

#[unsafe(export_name = "LWP_CondInit")]
/// Initialize a condition variable through the legacy libogc ABI.
///
/// # Safety
///
/// `cond` must be writable for one condition-variable handle.
pub unsafe extern "C" fn lwp_cond_init_abi(cond: *mut CondVar) -> i32 {
    unsafe { cond_init(cond) }
}
#[unsafe(export_name = "LWP_CondWait")]
pub extern "C" fn lwp_cond_wait_abi(cond: CondVar, mutex: Mutex) -> i32 {
    cond_wait(cond, mutex)
}
#[unsafe(export_name = "LWP_CondSignal")]
pub extern "C" fn lwp_cond_signal_abi(cond: CondVar) -> i32 {
    cond_signal(cond)
}
#[unsafe(export_name = "LWP_CondBroadcast")]
pub extern "C" fn lwp_cond_broadcast_abi(cond: CondVar) -> i32 {
    cond_broadcast(cond)
}
#[unsafe(export_name = "LWP_CondTimedWait")]
/// Wait on a condition variable with a relative timeout through the legacy ABI.
///
/// # Safety
///
/// A non-null `reltime` must point to a valid `TimeSpec` for this call.
pub unsafe extern "C" fn lwp_cond_timed_wait_abi(
    cond: CondVar,
    mutex: Mutex,
    reltime: *const TimeSpec,
) -> i32 {
    unsafe { cond_timed_wait(cond, mutex, reltime) }
}
#[unsafe(export_name = "LWP_CondDestroy")]
pub extern "C" fn lwp_cond_destroy_abi(cond: CondVar) -> i32 {
    cond_destroy(cond)
}

#[unsafe(export_name = "LWP_SemInit")]
/// Initialize a semaphore through the legacy libogc ABI.
///
/// # Safety
///
/// `sem` must be writable for one semaphore handle.
pub unsafe extern "C" fn lwp_sem_init_abi(sem: *mut Semaphore, start: u32, max: u32) -> i32 {
    unsafe { sem_init(sem, start, max) }
}
#[unsafe(export_name = "LWP_SemDestroy")]
pub extern "C" fn lwp_sem_destroy_abi(sem: Semaphore) -> i32 {
    sem_destroy(sem)
}
#[unsafe(export_name = "LWP_SemWait")]
pub extern "C" fn lwp_sem_wait_abi(sem: Semaphore) -> i32 {
    sem_wait(sem)
}
#[unsafe(export_name = "LWP_SemGetValue")]
/// Read a semaphore value through the legacy libogc ABI.
///
/// # Safety
///
/// `value` must be writable for one `u32`.
pub unsafe extern "C" fn lwp_sem_get_value_abi(sem: Semaphore, value: *mut u32) -> i32 {
    unsafe { sem_get_value(sem, value) }
}
#[unsafe(export_name = "LWP_SemPost")]
pub extern "C" fn lwp_sem_post_abi(sem: Semaphore) -> i32 {
    sem_post(sem)
}

#[unsafe(export_name = "MQ_Init")]
/// Initialize a message queue through the legacy libogc ABI.
///
/// # Safety
///
/// `mqbox` must be writable for one message-queue handle.
pub unsafe extern "C" fn mq_init_abi(mqbox: *mut MessageQueue, count: u32) -> i32 {
    unsafe { mq_init(mqbox, count) }
}
#[unsafe(export_name = "MQ_Close")]
pub extern "C" fn mq_close_abi(mqbox: MessageQueue) {
    mq_close(mqbox);
}
#[unsafe(export_name = "MQ_Send")]
pub extern "C" fn mq_send_abi(mqbox: MessageQueue, msg: Message, flags: u32) -> Bool {
    mq_send(mqbox, msg, flags)
}
#[unsafe(export_name = "MQ_Jam")]
pub extern "C" fn mq_jam_abi(mqbox: MessageQueue, msg: Message, flags: u32) -> Bool {
    mq_jam(mqbox, msg, flags)
}
#[unsafe(export_name = "MQ_Receive")]
/// Receive a message through the legacy libogc ABI.
///
/// # Safety
///
/// A non-null `msg` must be writable for one message pointer.
pub unsafe extern "C" fn mq_receive_abi(
    mqbox: MessageQueue,
    msg: *mut Message,
    flags: u32,
) -> Bool {
    unsafe { mq_receive(mqbox, msg, flags) }
}
