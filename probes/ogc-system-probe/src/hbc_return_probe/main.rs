#![no_std]
#![no_main]

use ogc_system as system;

const _: &ogc_system_probe::alloc_support::BumpAllocator =
    &ogc_system_probe::alloc_support::ALLOCATOR;

#[unsafe(no_mangle)]
pub extern "C" fn main(_argc: i32, _argv: *mut *mut u8) -> i32 {
    system::report(format_args!("HBC_RETURN_PROBE: entered\n"));
    system::init();
    system::report(format_args!("HBC_RETURN_PROBE: system init complete\n"));
    system::pre_main();
    system::report(format_args!("HBC_RETURN_PROBE: requesting HBC return\n"));

    // HBC loads this DOL and installs its reload stub below the DOL load range.
    // Exercise libogc's normal application exit path without requesting an IOS
    // reboot or a return to the System Menu.
    system::sys::sys_exit_to_loader(0)
}

#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
