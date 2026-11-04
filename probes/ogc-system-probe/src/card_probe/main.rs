#![cfg_attr(target_arch = "powerpc", no_std)]
#![cfg_attr(target_arch = "powerpc", no_main)]

#[cfg(not(target_arch = "powerpc"))]
fn main() {}

#[cfg(target_arch = "powerpc")]
#[unsafe(no_mangle)]
/// # Safety
///
/// The caller must provide the argument array promised by the libogc loader.
pub unsafe extern "C" fn main(argc: i32, argv: *mut *mut u8) -> i32 {
    // SAFETY: libogc calls this entrypoint with its live argc/argv.
    unsafe { ogc_system_probe::probe_support::set_agent_arguments(argc, argv) };
    // SAFETY: libogc supplies the live argument vector to this entrypoint.
    let destructive_card_format = unsafe {
        ogc_system_probe::probe_support::startup_arguments_contain(
            argc,
            argv,
            ogc_system_probe::probe_support::CARD_FORMAT_CONFIRMATION,
        )
    };
    ogc_system_probe::run_gamecube_card_probe_bin(destructive_card_format)
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
