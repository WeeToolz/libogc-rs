use core::sync::atomic::{AtomicU32, Ordering};

use ogc_system::{self as system, SysResetInfo};

use crate::probe_support::ProbeLogger;

static POWER_COUNT: AtomicU32 = AtomicU32::new(0);
static RESET_COUNT: AtomicU32 = AtomicU32::new(0);
static REPORT_BYTES: AtomicU32 = AtomicU32::new(0);
static REPORT_LEN: AtomicU32 = AtomicU32::new(0);

pub fn run(logger: &mut ProbeLogger) {
    logger.section("CALLBACK PROBE");

    logger.record("reset callback roundtrip", test_reset_callback());
    logger.record(
        "reset callback replacement preserves previous pointer",
        test_reset_callback_replacement(),
    );
    logger.record("power callback roundtrip", test_power_callback());
    let power_dispatch = test_power_dispatch(logger);
    logger.record("power callback dispatch", power_dispatch);
    logger.record("reset registration bookkeeping", test_reset_registration());
    logger.record("reset function gate", test_reset_function_gate());
    logger.record("power cooldown math", test_power_cooldown_math());
    logger.record(
        "report callback receives formatted bytes",
        test_report_callback(),
    );
    logger.record(
        "stdio report installs UART stream writers",
        test_stdio_report_writers(),
    );
}

fn test_stdio_report_writers() -> bool {
    // SAFETY: This single-threaded probe saves and restores the stream table.
    unsafe {
        let table = core::ptr::addr_of_mut!(ogc_sysbase::devoptab_list)
            .cast::<*const ogc_sysbase::Devoptab>();
        let previous_stdout = table.add(1).read();
        let previous_stderr = table.add(2).read();
        table.add(1).write(core::ptr::null());
        table.add(2).write(core::ptr::null());
        system::stdio_report(false);
        let stderr = table.add(2).read();
        let stderr_only = table.add(1).read().is_null()
            && !stderr.is_null()
            && (*stderr).write_r.is_some()
            && system::sys::sys_stdio_write(1, core::ptr::null(), 0).is_none();
        let stderr_bytes = b"STDIO_STDERR_PROBE\n";
        let stderr_written = if stderr.is_null() {
            false
        } else if let Some(write) = (*stderr).write_r {
            write(
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                stderr_bytes.as_ptr().cast(),
                stderr_bytes.len(),
            ) == stderr_bytes.len() as isize
        } else {
            false
        };
        system::stdio_report(true);
        let stdout = table.add(1).read();
        let both =
            !stdout.is_null() && stdout == table.add(2).read() && (*stdout).write_r.is_some();
        let stdout_bytes = b"STDIO_STDOUT_PROBE\n";
        let stdout_written =
            system::sys::sys_stdio_write(1, stdout_bytes.as_ptr(), stdout_bytes.len())
                == Some(stdout_bytes.len() as isize);
        let invalid = system::sys::sys_stdio_write(2, core::ptr::null(), 1)
            == Some(-(ogc_sysbase::EINVAL as isize));
        system::stdio_report(false);
        let retained = table.add(1).read() == stdout && system::stdio_report_uses_stdout();
        table.add(1).write(previous_stdout);
        table.add(2).write(previous_stderr);
        stderr_only && stderr_written && both && stdout_written && invalid && retained
    }
}

fn test_report_callback() -> bool {
    // SAFETY: The probe runs this callback check on one thread and restores
    // the callback before it returns to the logger.
    let original = unsafe { core::ptr::read(core::ptr::addr_of!(system::sys::DIET_PRINT_FN)) };
    REPORT_BYTES.store(0, Ordering::Release);
    REPORT_LEN.store(0, Ordering::Release);
    system::diet_print_set_func(Some(report_callback));
    let installed =
        system::snapshot_callbacks().diet_print_fn == report_callback as *const () as usize;
    let result = system::try_report(format_args!("Hi!\n"));
    let first = REPORT_LEN.load(Ordering::Acquire) == 4
        && REPORT_BYTES.load(Ordering::Acquire) == 0x4869_210a;
    REPORT_BYTES.store(0, Ordering::Release);
    REPORT_LEN.store(0, Ordering::Release);
    system::report(format_args!("OK"));
    let second = system::last_report_succeeded()
        && REPORT_LEN.load(Ordering::Acquire) == 2
        && REPORT_BYTES.load(Ordering::Acquire) == 0x4f4b;
    system::diet_print_set_func(original);
    installed
        && result.is_ok()
        && first
        && second
        && system::snapshot_callbacks().diet_print_fn == original.map_or(0, |cb| cb as usize)
}

unsafe extern "C" fn report_callback(buffer: *const core::ffi::c_char, size: usize) {
    // SAFETY: The report sink supplies a readable buffer for this call.
    let bytes = unsafe { core::slice::from_raw_parts(buffer.cast::<u8>(), size) };
    for byte in bytes {
        let previous = REPORT_BYTES.load(Ordering::Acquire);
        REPORT_BYTES.store((previous << 8) | u32::from(*byte), Ordering::Release);
        REPORT_LEN.fetch_add(1, Ordering::AcqRel);
    }
}

fn test_reset_callback_replacement() -> bool {
    let original = system::set_reset_callback(Some(reset_callback));
    let previous = system::set_reset_callback(Some(other_reset_callback));
    let snapshot = system::snapshot_callbacks();
    let cleared = system::set_reset_callback(None);
    let empty = system::snapshot_callbacks();
    let replaced = system::set_reset_callback(original);
    previous.map(|cb| cb as usize) == Some(reset_callback as *const () as usize)
        && snapshot.reset_callback == other_reset_callback as *const () as usize
        && cleared.map(|cb| cb as usize) == Some(other_reset_callback as *const () as usize)
        && empty.reset_callback == 0
        && replaced.is_none()
}

fn test_reset_callback() -> bool {
    let expected = reset_callback as *const () as usize;
    let original = system::set_reset_callback(Some(reset_callback));
    let current = system::snapshot_callbacks().reset_callback;
    let restore = system::set_reset_callback(original);
    let restored = system::snapshot_callbacks().reset_callback;
    current == expected
        && restore.map(|cb| cb as *const () as usize) == Some(expected)
        && restored == original.map_or(0, |cb| cb as usize)
}

fn test_power_callback() -> bool {
    let expected = power_callback as *const () as usize;
    let original = system::set_power_callback(Some(power_callback));
    let current = system::snapshot_callbacks().power_callback;
    let restore = system::set_power_callback(original);
    let restored = system::snapshot_callbacks().power_callback;
    current == expected
        && restore.map(|cb| cb as *const () as usize) == Some(expected)
        && restored == original.map_or(0, |cb| cb as usize)
}

fn test_power_dispatch(logger: &mut ProbeLogger) -> bool {
    POWER_COUNT.store(0, Ordering::Release);
    let original = system::set_power_callback(Some(power_callback));
    system::do_power_cb();
    let dispatch = system::snapshot_power_dispatch();
    let count = POWER_COUNT.load(Ordering::Acquire);
    system::do_power_cb();
    let suppressed = system::snapshot_power_dispatch();
    let after_repeat = POWER_COUNT.load(Ordering::Acquire);
    let _ = system::set_power_callback(original);
    logger.note_hex32(
        "power dispatch timebase low",
        ogc_core::powerpc::read_timebase() as u32,
    );
    logger.note_u32("power dispatch called", u32::from(dispatch.called));
    logger.note_u32("power dispatch result", u32::from(dispatch.result));
    logger.note_u32("power dispatch callback count", count);
    dispatch.called
        && dispatch.result
        && count == 1
        && suppressed.called
        && !suppressed.result
        && after_repeat == 1
}

fn test_reset_registration() -> bool {
    let mut info = SysResetInfo {
        node: ogc_system::LwpNode {
            next: core::ptr::null_mut(),
            prev: core::ptr::null_mut(),
        },
        func: Some(reset_function),
        prio: 7,
    };
    // SAFETY: `info` remains live and registered only until the immediately
    // following unregister call.
    unsafe {
        system::register_reset_func(&mut info);
        system::unregister_reset_func(&mut info);
    }
    let snap = system::snapshot_reset_registration();
    snap.register_called
        && snap.unregister_called
        && snap.register_ptr == (&mut info as *mut SysResetInfo as usize)
        && snap.unregister_ptr == (&mut info as *mut SysResetInfo as usize)
}

fn test_reset_function_gate() -> bool {
    RESET_COUNT.store(0, Ordering::Release);
    let callbacks: [ogc_system::ResetFunction; 1] = [Some(reset_function)];
    system::call_reset_functions(&callbacks, 1, true)
        && RESET_COUNT.load(Ordering::Acquire) == 1
        && !system::call_reset_functions(&callbacks, 0, false)
}

fn test_power_cooldown_math() -> bool {
    !system::power_callback_due(99, 100)
        && system::power_callback_due(100, 100)
        && system::next_power_enable_time(500, 50) == 550
}

unsafe extern "C" fn reset_callback(_irq: u32, _ctx: *mut core::ffi::c_void) {}

unsafe extern "C" fn other_reset_callback(_irq: u32, _ctx: *mut core::ffi::c_void) {}

unsafe extern "C" fn power_callback() {
    POWER_COUNT.fetch_add(1, Ordering::AcqRel);
}

unsafe extern "C" fn reset_function(final_: i32) -> i32 {
    RESET_COUNT.fetch_add(1, Ordering::AcqRel);
    if final_ == 1 { 1 } else { 0 }
}
