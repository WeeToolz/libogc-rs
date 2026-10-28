#![cfg_attr(target_arch = "powerpc", no_std)]
#![cfg_attr(target_arch = "powerpc", no_main)]

#[cfg(not(target_arch = "powerpc"))]
fn main() {}

#[cfg(target_arch = "powerpc")]
#[unsafe(no_mangle)]
pub extern "C" fn main(argc: i32, argv: *mut *mut u8) -> i32 {
    // SAFETY: The loader supplied these live startup arguments.
    unsafe { ogc_system_probe::probe_support::set_agent_arguments(argc, argv) };
    ogc_system_probe::run_usbstorage_transport_probe_bin()
}

#[cfg(target_arch = "powerpc")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
