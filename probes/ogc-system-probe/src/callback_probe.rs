use core::sync::atomic::{AtomicU32, Ordering};

use ogc_system::{self as system, SysResetInfo};

use crate::probe_support::ProbeLogger;

static POWER_COUNT: AtomicU32 = AtomicU32::new(0);
static RESET_COUNT: AtomicU32 = AtomicU32::new(0);

pub fn run(logger: &mut ProbeLogger) {
    logger.section("CALLBACK PROBE");

    logger.record("reset callback roundtrip", test_reset_callback());
    logger.record("power callback roundtrip", test_power_callback());
    logger.record("power callback dispatch", test_power_dispatch());
    logger.record("reset registration bookkeeping", test_reset_registration());
    logger.record("reset function gate", test_reset_function_gate());
    logger.record("power cooldown math", test_power_cooldown_math());
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

fn test_power_dispatch() -> bool {
    POWER_COUNT.store(0, Ordering::Release);
    let original = system::set_power_callback(Some(power_callback));
    system::do_power_cb();
    let dispatch = system::snapshot_power_dispatch();
    let count = POWER_COUNT.load(Ordering::Acquire);
    let _ = system::set_power_callback(original);
    dispatch.called && dispatch.result && count == 1
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

unsafe extern "C" fn power_callback() {
    POWER_COUNT.fetch_add(1, Ordering::AcqRel);
}

unsafe extern "C" fn reset_function(final_: i32) -> i32 {
    RESET_COUNT.fetch_add(1, Ordering::AcqRel);
    if final_ == 1 { 1 } else { 0 }
}
