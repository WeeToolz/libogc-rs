#![cfg_attr(target_arch = "powerpc", no_std)]
#![cfg_attr(target_arch = "powerpc", no_main)]

#[cfg(not(target_arch = "powerpc"))]
fn main() {}

#[cfg(target_arch = "powerpc")]
#[unsafe(no_mangle)]
/// # Safety
///
/// The caller must provide the argument array promised by the libogc loader.
pub unsafe extern "C" fn main(_argc: i32, _argv: *mut *mut u8) -> i32 {
    ogc_system_probe::run_gamecube_card_probe_bin()
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
