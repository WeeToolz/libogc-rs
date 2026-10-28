use core::ffi::c_void;

pub(crate) type UserCallback = unsafe extern "C" fn(i32, *mut c_void) -> i32;
pub(crate) type DeferredCallback = unsafe fn(*mut c_void);

/// Queues one completed IPC request by consuming its submission-time capacity
/// reservation.
///
/// # Safety
/// `argument` must remain valid until `enqueue` runs `callback`.
pub(crate) unsafe fn enqueue_reply(
    reservation: u32,
    callback: DeferredCallback,
    argument: *mut c_void,
    enqueue: impl FnOnce(u32, DeferredCallback, *mut c_void) -> i32,
    success: i32,
) -> Result<(), i32> {
    let result = enqueue(reservation, callback, argument);
    if result == success {
        Ok(())
    } else {
        Err(result)
    }
}

/// Runs an accepted request's user callback, then releases its request state.
///
/// The caller must provide a release action that frees the callback context.
/// The callback runs before release so it can safely inspect its user data.
///
/// # Safety
/// `user_data` must satisfy the callback's contract. `release` must release the
/// context owned by this completion and must not use it after returning.
pub(crate) unsafe fn deliver_reply(
    result: i32,
    callback: Option<UserCallback>,
    user_data: *mut c_void,
    release: impl FnOnce(),
) {
    if let Some(callback) = callback {
        unsafe { callback(result, user_data) };
    }
    release();
}

#[cfg(test)]
#[path = "../../ogc-lwp-driver/src/deferred_slots.rs"]
mod deferred_slots;

#[cfg(test)]
mod tests {
    use super::deferred_slots::DeferredSlots;
    use super::{DeferredCallback, UserCallback, deliver_reply, enqueue_reply};
    use core::ffi::c_void;

    const QUEUE_CAPACITY: usize = 32;
    const LWP_SUCCESSFUL: i32 = 0;
    const LWP_CLOSED: i32 = -1;

    #[derive(Copy, Clone)]
    struct QueuedCall {
        callback: DeferredCallback,
        argument: *mut c_void,
    }

    struct TestQueue {
        slots: DeferredSlots<QUEUE_CAPACITY>,
        entries: [Option<QueuedCall>; QUEUE_CAPACITY],
        head: usize,
        len: usize,
    }

    impl TestQueue {
        fn new() -> Self {
            Self {
                slots: DeferredSlots::new(),
                entries: [None; QUEUE_CAPACITY],
                head: 0,
                len: 0,
            }
        }

        fn reserve(&mut self) -> Option<u32> {
            self.slots.reserve()
        }

        fn enqueue_unreserved(&mut self, call: QueuedCall) -> bool {
            if !self.slots.enqueue_unreserved() {
                return false;
            }
            self.enqueue(call);
            true
        }

        fn enqueue_reserved(&mut self, reservation: u32, call: QueuedCall) -> i32 {
            if !self.slots.enqueue_reserved(reservation) {
                return LWP_CLOSED;
            }
            self.enqueue(call);
            LWP_SUCCESSFUL
        }

        fn enqueue(&mut self, call: QueuedCall) {
            let tail = (self.head + self.len) % QUEUE_CAPACITY;
            self.entries[tail] = Some(call);
            self.len += 1;
        }

        fn dequeue(&mut self) -> Option<QueuedCall> {
            let call = self.entries[self.head].take()?;
            self.head = (self.head + 1) % QUEUE_CAPACITY;
            self.len -= 1;
            assert!(self.slots.dequeue());
            Some(call)
        }
    }

    struct RequestState {
        reply_result: i32,
        callback: Option<UserCallback>,
        callback_result: Option<i32>,
        callback_saw_live_request: bool,
        released: bool,
    }

    unsafe extern "C" fn record_user_result(result: i32, user_data: *mut c_void) -> i32 {
        let state = unsafe { &mut *user_data.cast::<RequestState>() };
        state.callback_result = Some(result);
        state.callback_saw_live_request = !state.released;
        0
    }

    unsafe fn finish_test_request(argument: *mut c_void) {
        let state = unsafe { &mut *argument.cast::<RequestState>() };
        let result = state.reply_result;
        let callback = state.callback;
        unsafe {
            deliver_reply(result, callback, argument, || {
                (*argument.cast::<RequestState>()).released = true;
            });
        }
    }

    unsafe fn ignore_call(_argument: *mut c_void) {}

    #[test]
    fn accepted_reply_reaches_callback_after_unreserved_queue_pressure() {
        let mut queue = TestQueue::new();
        let reservation = queue.reserve().expect("IPC reserves before submission");
        let mut state = RequestState {
            reply_result: -6,
            callback: Some(record_user_result),
            callback_result: None,
            callback_saw_live_request: false,
            released: false,
        };

        for _ in 0..QUEUE_CAPACITY - 1 {
            assert!(queue.enqueue_unreserved(QueuedCall {
                callback: ignore_call,
                argument: core::ptr::null_mut(),
            }));
        }
        assert!(!queue.enqueue_unreserved(QueuedCall {
            callback: ignore_call,
            argument: core::ptr::null_mut(),
        }));

        let queued = unsafe {
            enqueue_reply(
                reservation,
                finish_test_request,
                (&mut state as *mut RequestState).cast(),
                |reservation, callback, argument| {
                    queue.enqueue_reserved(reservation, QueuedCall { callback, argument })
                },
                LWP_SUCCESSFUL,
            )
        };
        assert_eq!(queued, Ok(()));
        assert_eq!(queue.len, QUEUE_CAPACITY);

        for _ in 0..QUEUE_CAPACITY - 1 {
            let call = queue.dequeue().expect("queued producer callback");
            unsafe { (call.callback)(call.argument) };
        }
        let completion = queue.dequeue().expect("reserved IPC completion");
        assert!(!state.released);
        unsafe { (completion.callback)(completion.argument) };
        assert_eq!(state.callback_result, Some(-6));
        assert!(state.callback_saw_live_request);
        assert!(state.released);
        assert_eq!(queue.len, 0);
    }
}
