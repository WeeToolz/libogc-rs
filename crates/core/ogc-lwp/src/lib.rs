#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::ffi::c_void;

pub use ogc_lwp_sys as sys;
pub use sys::{
    Bool, CondVar as RawCondVar, LwpQueue as RawQueue, LwpThread as RawThread, Message,
    MessageQueue as RawMessageQueue, Mutex as RawMutex, Semaphore as RawSemaphore, TimeSpec,
};

pub use sys::{
    ThreadCleanupCounters, TlsDestructor, TlsError, TlsKey, thread_cleanup_counters, tls_create,
    tls_destroy, tls_get, tls_set,
};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ThreadHandle {
    raw: sys::LwpThread,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct QueueHandle {
    raw: sys::LwpQueue,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MutexHandle {
    raw: sys::Mutex,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct CondVarHandle {
    raw: sys::CondVar,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct SemaphoreHandle {
    raw: sys::Semaphore,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct MessageQueueHandle {
    raw: sys::MessageQueue,
}

impl ThreadHandle {
    pub const fn new(raw: sys::LwpThread) -> Self {
        Self { raw }
    }

    pub const fn raw(self) -> sys::LwpThread {
        self.raw
    }

    pub const fn is_null(self) -> bool {
        self.raw == sys::LWP_THREAD_NULL
    }

    pub const fn into_raw(self) -> sys::LwpThread {
        self.raw
    }

    /// # Safety
    ///
    /// `arg` and `stackbase` must satisfy the ABI and lifetime expectations of
    /// the underlying LWP thread entrypoint and stack management.
    pub unsafe fn create(
        thread: &mut sys::LwpThread,
        entry: sys::ThreadEntry,
        arg: *mut c_void,
        stackbase: *mut c_void,
        stack_size: u32,
        prio: u8,
    ) -> Result<Self, i32> {
        unsafe {
            let rc = sys::create_thread(thread, entry, arg, stackbase, stack_size, prio);
            if rc == 0 {
                Ok(Self::new(*thread))
            } else {
                Err(rc)
            }
        }
    }

    pub fn suspend(self) -> Result<(), i32> {
        let rc = sys::suspend_thread(self.raw);
        if rc == 0 { Ok(()) } else { Err(rc) }
    }

    pub fn resume(self) -> Result<(), i32> {
        let rc = sys::resume_thread(self.raw);
        if rc == 0 { Ok(()) } else { Err(rc) }
    }

    pub fn is_suspended(self) -> bool {
        sys::thread_is_suspended(self.raw) != 0
    }

    pub fn set_priority(self, prio: u32) {
        sys::set_thread_priority(self.raw, prio);
    }

    pub fn join(self) -> Result<*mut c_void, i32> {
        let mut value = core::ptr::null_mut();
        // SAFETY: `value` is a valid unique output for the synchronous join.
        let rc = unsafe { sys::join_thread(self.raw, &mut value) };
        if rc == 0 { Ok(value) } else { Err(rc) }
    }

    /// Allow this thread to finish without a joiner. Cleanup runs after exit.
    pub fn detach(self) -> Result<(), i32> {
        match sys::detach_thread(self.raw) {
            0 => Ok(()),
            error => Err(error),
        }
    }
}

impl QueueHandle {
    pub const fn new(raw: sys::LwpQueue) -> Self {
        Self { raw }
    }

    pub const fn raw(self) -> sys::LwpQueue {
        self.raw
    }

    pub const fn into_raw(self) -> sys::LwpQueue {
        self.raw
    }

    pub fn init(queue: &mut sys::LwpQueue) -> Result<Self, i32> {
        let rc = unsafe { sys::init_queue(queue) };
        if rc == 0 {
            Ok(Self::new(*queue))
        } else {
            Err(rc)
        }
    }

    pub fn close(self) {
        sys::close_queue(self.raw);
    }

    pub fn sleep(self) -> Result<(), i32> {
        let rc = sys::thread_sleep(self.raw);
        if rc == 0 { Ok(()) } else { Err(rc) }
    }

    pub fn signal(self) {
        sys::thread_signal(self.raw);
    }

    pub fn broadcast(self) {
        sys::thread_broadcast(self.raw);
    }
}

impl MutexHandle {
    pub const fn new(raw: sys::Mutex) -> Self {
        Self { raw }
    }

    pub const fn raw(self) -> sys::Mutex {
        self.raw
    }

    pub fn init(mutex: &mut sys::Mutex, use_recursive: bool) -> Result<Self, i32> {
        // SAFETY: `mutex` is a valid unique output reference.
        let rc = unsafe { sys::mutex_init(mutex, use_recursive as Bool) };
        if rc == 0 {
            Ok(Self::new(*mutex))
        } else {
            Err(rc)
        }
    }

    pub fn destroy(self) -> Result<(), i32> {
        let rc = sys::mutex_destroy(self.raw);
        if rc == 0 { Ok(()) } else { Err(rc) }
    }

    pub fn lock(self) -> Result<(), i32> {
        let rc = sys::mutex_lock(self.raw);
        if rc == 0 { Ok(()) } else { Err(rc) }
    }

    pub fn try_lock(self) -> Result<bool, i32> {
        let rc = sys::mutex_try_lock(self.raw);
        if rc == 0 {
            Ok(true)
        } else if rc == 1 {
            Ok(false)
        } else {
            Err(rc)
        }
    }

    pub fn unlock(self) -> Result<(), i32> {
        let rc = sys::mutex_unlock(self.raw);
        if rc == 0 { Ok(()) } else { Err(rc) }
    }
}

impl CondVarHandle {
    pub const fn new(raw: sys::CondVar) -> Self {
        Self { raw }
    }

    pub const fn raw(self) -> sys::CondVar {
        self.raw
    }

    pub fn init(cond: &mut sys::CondVar) -> Result<Self, i32> {
        // SAFETY: `cond` is a valid unique output reference.
        let rc = unsafe { sys::cond_init(cond) };
        if rc == 0 {
            Ok(Self::new(*cond))
        } else {
            Err(rc)
        }
    }

    pub fn wait(self, mutex: MutexHandle) -> Result<(), i32> {
        let rc = sys::cond_wait(self.raw, mutex.raw);
        if rc == 0 { Ok(()) } else { Err(rc) }
    }

    pub fn timed_wait(self, mutex: MutexHandle, reltime: &TimeSpec) -> Result<(), i32> {
        // SAFETY: `reltime` remains valid for this synchronous wait setup.
        let rc = unsafe { sys::cond_timed_wait(self.raw, mutex.raw, reltime) };
        if rc == 0 { Ok(()) } else { Err(rc) }
    }

    pub fn signal(self) -> Result<(), i32> {
        let rc = sys::cond_signal(self.raw);
        if rc == 0 { Ok(()) } else { Err(rc) }
    }

    pub fn broadcast(self) -> Result<(), i32> {
        let rc = sys::cond_broadcast(self.raw);
        if rc == 0 { Ok(()) } else { Err(rc) }
    }

    pub fn destroy(self) -> Result<(), i32> {
        let rc = sys::cond_destroy(self.raw);
        if rc == 0 { Ok(()) } else { Err(rc) }
    }
}

impl SemaphoreHandle {
    pub const fn new(raw: sys::Semaphore) -> Self {
        Self { raw }
    }

    pub const fn raw(self) -> sys::Semaphore {
        self.raw
    }

    pub fn init(sem: &mut sys::Semaphore, start: u32, max: u32) -> Result<Self, i32> {
        // SAFETY: `sem` is a valid unique output reference.
        let rc = unsafe { sys::sem_init(sem, start, max) };
        if rc == 0 {
            Ok(Self::new(*sem))
        } else {
            Err(rc)
        }
    }

    pub fn destroy(self) -> Result<(), i32> {
        let rc = sys::sem_destroy(self.raw);
        if rc == 0 { Ok(()) } else { Err(rc) }
    }

    pub fn wait(self) -> Result<(), i32> {
        let rc = sys::sem_wait(self.raw);
        if rc == 0 { Ok(()) } else { Err(rc) }
    }

    pub fn get_value(self) -> Result<u32, i32> {
        let mut value = 0;
        // SAFETY: `value` is a valid unique output reference.
        let rc = unsafe { sys::sem_get_value(self.raw, &mut value) };
        if rc == 0 { Ok(value) } else { Err(rc) }
    }

    pub fn post(self) -> Result<(), i32> {
        let rc = sys::sem_post(self.raw);
        if rc == 0 { Ok(()) } else { Err(rc) }
    }
}

impl MessageQueueHandle {
    pub const fn new(raw: sys::MessageQueue) -> Self {
        Self { raw }
    }

    pub const fn raw(self) -> sys::MessageQueue {
        self.raw
    }

    pub fn init(mqbox: &mut sys::MessageQueue, count: u32) -> Result<Self, i32> {
        // SAFETY: `mqbox` is a valid unique output reference.
        let rc = unsafe { sys::mq_init(mqbox, count) };
        if rc == 0 {
            Ok(Self::new(*mqbox))
        } else {
            Err(rc)
        }
    }

    pub fn close(self) {
        sys::mq_close(self.raw);
    }

    /// # Safety
    ///
    /// `msg` must be a valid message pointer for the duration of the queueing
    /// operation and obey the ownership rules of the caller and receiver.
    pub unsafe fn send(self, msg: Message, block: bool) -> bool {
        sys::mq_send(
            self.raw,
            msg,
            if block {
                sys::MQ_MSG_BLOCK
            } else {
                sys::MQ_MSG_NOBLOCK
            },
        ) != 0
    }

    /// # Safety
    ///
    /// `msg` must be a valid message pointer for the duration of the queueing
    /// operation and obey the ownership rules of the caller and receiver.
    pub unsafe fn jam(self, msg: Message, block: bool) -> bool {
        sys::mq_jam(
            self.raw,
            msg,
            if block {
                sys::MQ_MSG_BLOCK
            } else {
                sys::MQ_MSG_NOBLOCK
            },
        ) != 0
    }

    pub fn receive(self, block: bool) -> Option<Message> {
        let mut msg = core::ptr::null_mut();
        // SAFETY: `msg` is a valid unique output reference.
        let ok = unsafe {
            sys::mq_receive(
                self.raw,
                &mut msg,
                if block {
                    sys::MQ_MSG_BLOCK
                } else {
                    sys::MQ_MSG_NOBLOCK
                },
            )
        };
        if ok != 0 { Some(msg) } else { None }
    }
}

pub fn current_thread() -> ThreadHandle {
    ThreadHandle::new(sys::get_self())
}

pub fn yield_now() {
    sys::yield_thread()
}

pub fn reschedule(prio: u32) {
    sys::reschedule(prio)
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    #[test]
    fn handle_layouts_are_raw_u32() {
        assert_eq!(core::mem::size_of::<ThreadHandle>(), 4);
        assert_eq!(core::mem::size_of::<QueueHandle>(), 4);
        assert_eq!(core::mem::size_of::<MutexHandle>(), 4);
        assert_eq!(core::mem::size_of::<CondVarHandle>(), 4);
        assert_eq!(core::mem::size_of::<SemaphoreHandle>(), 4);
        assert_eq!(core::mem::size_of::<MessageQueueHandle>(), 4);
    }

    #[test]
    fn helper_functions_exist_without_backend() {
        let _ = current_thread as fn() -> ThreadHandle;
        let _ = yield_now as fn();
        let _ = reschedule as fn(u32);
    }
}
