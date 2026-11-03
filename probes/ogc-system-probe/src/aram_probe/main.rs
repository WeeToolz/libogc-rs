#![cfg_attr(target_arch = "powerpc", no_std)]
#![cfg_attr(target_arch = "powerpc", no_main)]

#[cfg(not(target_arch = "powerpc"))]
fn main() {}

#[cfg(target_arch = "powerpc")]
#[unsafe(no_mangle)]
/// Run the ARAM probe from the libogc process entry point.
///
/// # Safety
///
/// `argv` must be the live argument vector supplied by libogc.
pub unsafe extern "C" fn main(argc: i32, argv: *mut *mut u8) -> i32 {
    // SAFETY: libogc calls this exported entry point with its live argc/argv.
    unsafe { ogc_system_probe::probe_support::set_agent_arguments(argc, argv) };
    ogc_system_probe::run_aram_probe_bin()
}

#[cfg(target_arch = "powerpc")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
