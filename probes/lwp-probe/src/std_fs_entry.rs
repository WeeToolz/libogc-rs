#![feature(restricted_std)]
//! Wii std FAT file acceptance entry point.
mod std_fs_probe;
mod std_probe_config;
use core::sync::atomic::{AtomicI32, Ordering};
use ogc_system_probe::probe_support;
use std::io::Write;
use std::process::ExitCode;

unsafe extern "C" {
    fn __wii_std_output_stream(stream: u32, bytes: *const u8, length: usize, flush: bool) -> isize;
}

#[unsafe(no_mangle)]
pub static WII_STD_FS_PROBE_RESULT: AtomicI32 = AtomicI32::new(i32::MIN);

extern "C" fn after_main(status: i32) -> ! {
    if status != WII_STD_FS_PROBE_RESULT.load(Ordering::Acquire) {
        WII_STD_FS_PROBE_RESULT.store(-2, Ordering::Release);
    }
    loop {
        probe_support::wait_vsync();
    }
}
fn main() -> ExitCode {
    let mut logger = ogc_system_probe::init_named_probe_logger("wii-std-fs-probe");
    ogc_sys::set_exit_callback(Some(after_main));
    probe_stdio_streams(&mut logger);
    std_fs_probe::run(&mut logger);
    logger.summary();
    let status = if logger.checks_passed() { 0 } else { 1 };
    WII_STD_FS_PROBE_RESULT.store(status, Ordering::Release);
    ExitCode::from(status as u8)
}

fn probe_stdio_streams(logger: &mut probe_support::ProbeLogger) {
    // SAFETY: This startup check owns the device table while the probe is single-threaded.
    let table =
        core::ptr::addr_of_mut!(ogc_sysbase::devoptab_list).cast::<*const ogc_sysbase::Devoptab>();
    let previous_stdout = unsafe { table.add(1).read() };
    let previous_stderr = unsafe { table.add(2).read() };
    unsafe {
        table.add(1).write(core::ptr::null());
        table.add(2).write(core::ptr::null());
    }
    ogc_system::stdio_report(false);
    let stderr = std::io::stderr()
        .write_all(b"STD_STREAM_ERR_ONLY\n")
        .is_ok();
    let stdout_console = std::io::stdout()
        .write_all(b"STD_STREAM_CONSOLE_ONLY\n")
        .is_ok();
    ogc_system::stdio_report(true);
    let stdout_uart = std::io::stdout()
        .write_all(b"STD_STREAM_OUT_UART\n")
        .is_ok();
    let stderr_uart = std::io::stderr()
        .write_all(b"STD_STREAM_ERR_UART\n")
        .is_ok();
    let direct_bytes = b"STD_STREAM_DIRECT\n";
    // SAFETY: The direct output buffer is readable for its stated length.
    let direct = unsafe {
        __wii_std_output_stream(2, direct_bytes.as_ptr(), direct_bytes.len(), false)
            == direct_bytes.len() as isize
            && __wii_std_output_stream(3, core::ptr::null(), 0, false)
                == -(ogc_sysbase::EINVAL as isize)
            && __wii_std_output_stream(2, core::ptr::null(), 1, false)
                == -(ogc_sysbase::EINVAL as isize)
            && __wii_std_output_stream(2, core::ptr::null(), 0, true) == 0
    };
    unsafe {
        table.add(1).write(previous_stdout);
        table.add(2).write(previous_stderr);
    }
    logger.record(
        "Wii std routes stdout and stderr by stream",
        stderr && stdout_console && stdout_uart && stderr_uart,
    );
    logger.record("Wii std stream ABI writes and validates inputs", direct);
}
