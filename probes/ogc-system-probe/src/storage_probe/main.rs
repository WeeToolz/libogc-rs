#![cfg_attr(target_arch = "powerpc", no_std)]
#![cfg_attr(target_arch = "powerpc", no_main)]

#[cfg(not(target_arch = "powerpc"))]
fn main() {}

#[cfg(target_arch = "powerpc")]
#[unsafe(no_mangle)]
pub extern "C" fn main(argc: i32, argv: *mut *mut u8) -> i32 {
    // SAFETY: libogc calls this exported entrypoint with its live argc/argv.
    unsafe { ogc_system_probe::probe_support::set_agent_arguments(argc, argv) };
    const CONFIRMATION: &[u8] = b"--destructive-card-format=ERASE-SLOT-A";
    let mut destructive_card_format = false;
    if argc > 0 && !argv.is_null() {
        for index in 0..argc as usize {
            // SAFETY: the loader supplies `argc` live pointers to NUL-terminated arguments.
            let argument = unsafe { *argv.add(index) };
            if !argument.is_null()
                // SAFETY: each non-null loader argument is NUL-terminated.
                && unsafe { core::ffi::CStr::from_ptr(argument.cast()) }.to_bytes()
                    == CONFIRMATION
            {
                destructive_card_format = true;
            }
        }
    }
    ogc_system_probe::run_storage_probe_bin(destructive_card_format)
}

#[cfg(target_arch = "powerpc")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    use core::sync::atomic::Ordering;

    ogc_system_probe::storage_probe::OGC_STORAGE_PROBE_STAGE.store(u32::MAX, Ordering::Release);
    ogc_system_probe::storage_probe::OGC_STORAGE_PROBE_LAST_RESULT
        .store(i32::MIN, Ordering::Release);
    loop {
        core::hint::spin_loop();
    }
}
