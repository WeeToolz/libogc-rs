#![cfg_attr(not(feature = "std"), no_std)]
#![cfg_attr(feature = "std", feature(restricted_std))]

//! Authoritative Wii PowerPC lightweight-thread runtime.
//!
//! The driver owns thread contexts, scheduling, synchronization, deadlines,
//! deferred callbacks, and interrupt-safe wakeups. Compatibility crates are
//! thin handle/ABI adapters over this implementation.

#[cfg(all(test, not(feature = "std")))]
extern crate std;

#[cfg(not(target_arch = "powerpc"))]
compile_error!("ogc-lwp-driver targets Wii PowerPC only");

mod arch;
mod contracts;
mod deferred_slots;
mod futex;
mod lifecycle;
mod runtime;
mod thread;
mod tls;

pub use futex::{FUTEX_MAX_TIMEOUT_TICKS, FutexError, FutexWait};
pub use runtime::{
    StackBounds, ThreadCleanup, cleanup_current_thread, current_thread_checked,
    detach_thread_checked, join_thread_checked, register_thread_cleanup, set_current_thread_name,
    sleep_for, spawn_thread, thread_name, thread_stack_bounds,
};
pub use thread::{STD_MIN_STACK_SIZE, THREAD_NAME_CAPACITY, ThreadError};
pub use tls::{TLS_DESTRUCTOR_ITERATIONS, TLS_KEY_CAPACITY, TlsDestructor, TlsError, TlsKey};

pub use arch::LwpContext;
pub use contracts::{LwpClock, LwpDeferredExecutor, LwpInterruptWake, LwpTimerService};
pub use runtime::{
    CondHandle, DebugCounters, DebugEvent, DebugHook, DeferredCallback, KeyedTimerCallback,
    LWP_ALREADY_SUSPENDED, LWP_CLOSED, LWP_COND_NULL, LWP_MUTEX_NULL, LWP_NOT_SUSPENDED,
    LWP_PRIO_HIGHEST, LWP_PRIO_IDLE, LWP_SEM_NULL, LWP_SUCCESSFUL, LWP_THREAD_NULL,
    LWP_TQUEUE_NULL, LwpThread, MQ_BOX_NULL, MQ_ERROR_SUCCESSFUL, MQ_ERROR_TOOMANY, MQ_MSG_BLOCK,
    MQ_MSG_NOBLOCK, Message, MessageQueueHandle, MutexHandle, QueueHandle, SemaphoreHandle,
    TICKS_PER_SECOND, ThreadCleanupCounters, ThreadEntry, TimerCallback, WaitResult,
    WorkspaceAlloc, WorkspaceFree, allocate_workspace, block_current_external, broadcast_external,
    cancel_external_wait, close_cond, close_message_queue, close_mutex, close_queue,
    close_semaphore, cond_broadcast, cond_signal, cond_wait, create_thread, create_thread_at_top,
    critical_enter, critical_exit, current_reent, current_thread, debug_counters, debug_drain,
    deferred_cancel_interrupt_slot, deferred_enqueue, deferred_enqueue_from_interrupt,
    deferred_enqueue_reserved_from_interrupt, deferred_reserve_interrupt_slot, detach_thread,
    futex_wait, futex_wait_for, futex_wake, init_cond, init_message_queue, init_mutex, init_queue,
    init_semaphore, initialize, initialize_external_reent, join_thread, message_jam,
    message_receive, message_send, mutex_lock, mutex_try_lock, mutex_unlock, note_ipc_wake,
    now_ticks, queue_broadcast, queue_signal, queue_sleep, release_workspace, reschedule,
    resume_thread, semaphore_get_value, semaphore_post, semaphore_wait, set_debug_hook,
    set_thread_context_mirror, set_thread_priority, set_thread_reent_storage,
    set_workspace_allocator, sleep_ticks, suspend_thread, thread_argument, thread_cleanup_counters,
    thread_context_snapshot, thread_exit, thread_handle_by_argument, thread_is_suspended,
    timer_cancel, timer_start, timer_start_with_key, tls_create, tls_destroy, tls_get, tls_set,
    wake_external_by_mask, wake_external_by_value, wake_thread_with_token, yield_thread,
};
