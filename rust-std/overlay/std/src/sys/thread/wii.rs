use crate::ffi::{CStr, c_void};
use crate::io;
use crate::mem::ManuallyDrop;
use crate::num::NonZero;
use crate::ptr;
use crate::sys::wii;
use crate::thread::ThreadInit;
use crate::time::Duration;

pub const DEFAULT_MIN_STACK_SIZE: usize = 128 * 1024;

pub struct Thread {
    id: u32,
}

impl Thread {
    // SAFETY: The caller supplies the closure lifetime contract documented by
    // thread::Builder::spawn_unchecked, including borrowed scoped captures.
    pub unsafe fn new(stack: usize, init: Box<ThreadInit>) -> io::Result<Self> {
        let data = Box::into_raw(init);
        let mut id = u32::MAX;
        let result = unsafe { wii::thread_create(Some(thread_start), data.cast(), stack, &mut id) };
        if result != 0 {
            // Failure never transfers the closure to a worker.
            unsafe { drop(Box::from_raw(data)) };
            return Err(io::Error::from_raw_os_error(result));
        }
        assert!(
            id != 0 && id != u32::MAX,
            "Wii created an invalid thread handle"
        );
        Ok(Self { id })
    }

    pub fn join(self) {
        // A consumed join handle must not run the detach destructor.
        let thread = ManuallyDrop::new(self);
        let result = unsafe { wii::thread_join(thread.id) };
        assert_eq!(result, 0, "Wii thread join failed with OS error {result}");
    }
}

unsafe extern "C" fn thread_start(data: *mut c_void) -> *mut c_void {
    // ThreadInit initializes std's current-thread handle before freeing its
    // allocation. LWP runs key destructors and then std runtime cleanup after
    // this entry returns, before publishing completion or releasing its stack.
    let init = unsafe { Box::from_raw(data.cast::<ThreadInit>()) };
    let rust_start = init.init();
    rust_start();
    ptr::null_mut()
}

impl Drop for Thread {
    fn drop(&mut self) {
        let result = unsafe { wii::thread_detach(self.id) };
        assert_eq!(result, 0, "Wii thread detach failed with OS error {result}");
    }
}

pub fn current_os_id() -> Option<u64> {
    let mut id = 0;
    let result = unsafe { wii::thread_current(&mut id) };
    assert_eq!(
        result, 0,
        "Wii current thread failed with OS error {result}"
    );
    Some(u64::from(id))
}

pub fn set_name(name: &CStr) {
    let name = name.to_bytes();
    let result = unsafe { wii::thread_set_name(name.as_ptr(), name.len()) };
    assert_eq!(result, 0, "Wii thread name failed with OS error {result}");
}

pub fn yield_now() {
    let result = unsafe { wii::thread_yield() };
    assert_eq!(result, 0, "Wii thread yield failed with OS error {result}");
}

pub fn sleep(duration: Duration) {
    let result = unsafe { wii::thread_sleep(duration.as_secs(), duration.subsec_nanos()) };
    assert_eq!(result, 0, "Wii thread sleep failed with OS error {result}");
}

pub fn available_parallelism() -> io::Result<NonZero<usize>> {
    let mut count = 0;
    let result = unsafe { wii::thread_parallelism(&mut count) };
    if result != 0 {
        return Err(io::Error::from_raw_os_error(result));
    }
    NonZero::new(count)
        .ok_or_else(|| io::const_error!(io::ErrorKind::InvalidData, "Wii CPU count is zero"))
}
