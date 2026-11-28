#![cfg_attr(target_arch = "powerpc", no_std)]
#![cfg_attr(target_arch = "powerpc", no_main)]

#[cfg(not(target_arch = "powerpc"))]
fn main() {}

#[cfg(target_arch = "powerpc")]
#[unsafe(no_mangle)]
/// # Safety
///
/// libogc calls this entry point with its live argument array.
pub unsafe extern "C" fn main(argc: i32, argv: *mut *mut u8) -> i32 {
    // SAFETY: libogc provides the live loader argument array.
    unsafe { ogc_system_probe::probe_support::set_agent_arguments(argc, argv) };
    ogc_system_probe::run_bt_dispatch_probe_bin()
}

#[cfg(target_arch = "powerpc")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
