use crate::io;

// SAFETY: The compiler calls this once on the initial thread before user main.
// Rust's sigpipe mode applies only to Unix, so it has no Wii operation.
pub unsafe fn init(argc: isize, argv: *const *const u8, _sigpipe: u8) {
    let result = unsafe { crate::sys::wii::runtime_enter(argc, argv) };
    if result != 0 {
        rtabort!("Wii runtime entry failed: {result}");
    }
    crate::sys::thread::set_name(c"main");
}

// SAFETY: std::rt serializes this after main and its stream cleanup.
pub unsafe fn cleanup() {
    let result = unsafe { crate::sys::wii::runtime_cleanup() };
    if result != 0 {
        rtabort!("Wii initial thread cleanup failed: {result}");
    }
}

pub fn unsupported<T>() -> io::Result<T> {
    Err(unsupported_err())
}
pub fn unsupported_err() -> io::Error {
    io::Error::UNSUPPORTED_PLATFORM
}
pub fn abort_internal() -> ! {
    core::intrinsics::abort_immediate()
}
