#![no_std]
#![no_main]

#[unsafe(no_mangle)]
pub extern "C" fn main(_argc: i32, _argv: *mut *mut u8) -> i32 {
    ogc_system_probe::run_pad_state_probe_bin()
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    ogc_system_probe::probe_support::panic_screen()
}
