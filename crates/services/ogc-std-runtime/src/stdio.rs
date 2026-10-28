//! Allocation-free console transport for standard output and panic bytes.
use core::sync::atomic::{AtomicI32, Ordering};

static LAST_ERROR: AtomicI32 = AtomicI32::new(0);

/// Latest failed diagnostic operation, including failures during panic abort.
pub fn diagnostic_error() -> Option<i32> {
    let error = LAST_ERROR.load(Ordering::Acquire);
    (error != 0).then_some(error)
}

/// # Safety
/// Nonempty input must point to `length` readable bytes for this call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wii_std_output(bytes: *const u8, length: usize, flush: bool) -> isize {
    let result = if length > isize::MAX as usize || (length != 0 && bytes.is_null()) {
        Err(ogc_sysbase::EINVAL)
    } else {
        // The console uses shared parser and cursor state. Mask interrupts on
        // this single CPU so stdout, stderr, and panic writes cannot interleave.
        // This backend does not allocate, wait, or call a user callback.
        let irq = ogc_lwp_driver::critical_enter();
        let console = ogc_video::console_get_default();
        let result = if console.is_null() || !unsafe { (*console).console_initialised } {
            Err(ogc_sysbase::ENODEV)
        } else if flush {
            ogc_video::flush();
            Ok(0)
        } else if length == 0 {
            Ok(0)
        } else {
            let input = unsafe { core::slice::from_raw_parts(bytes, length) };
            ogc_video::console_write(input).map_err(|_| ogc_sysbase::EIO)
        };
        ogc_lwp_driver::critical_exit(irq);
        result
    };
    match result {
        Ok(count) => count as isize,
        Err(error) => {
            LAST_ERROR.store(error, Ordering::Release);
            -(error as isize)
        }
    }
}
