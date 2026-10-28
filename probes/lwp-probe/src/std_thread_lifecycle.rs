use crate::std_thread_probe;

use core::cell::UnsafeCell;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicPtr, AtomicU32, Ordering};
use ogc_std_runtime::{self as runtime, RuntimePhase};
use ogc_system_probe::probe_support::{self, ProbeLogger};
use std::cell::Cell;
use std::process::ExitCode;

struct ExitLogger(UnsafeCell<MaybeUninit<ProbeLogger>>);
// SAFETY: Only main initializes the logger; only its exit callback reads it.
unsafe impl Sync for ExitLogger {}
static EXIT_LOGGER: ExitLogger = ExitLogger(UnsafeCell::new(MaybeUninit::uninit()));
static LOGGER_READY: AtomicBool = AtomicBool::new(false);
static RETURN_TO_MENU_AFTER_SECONDS: AtomicU32 = AtomicU32::new(0);
static MAIN_DROPS: AtomicU32 = AtomicU32::new(0);
static LATE_DROPS: AtomicU32 = AtomicU32::new(0);
static DTOR_ERRORS: AtomicU32 = AtomicU32::new(0);

static RESULT: AtomicPtr<AtomicI32> = AtomicPtr::new(core::ptr::null_mut());
fn result() -> &'static AtomicI32 {
    let pointer = RESULT.load(Ordering::Acquire);
    assert!(!pointer.is_null(), "probe result destination missing");
    // SAFETY: finish publishes a reference to a static AtomicI32.
    unsafe { &*pointer }
}

struct MainTls(Cell<u32>);
struct LateTls;
std::thread_local! {
    static MAIN_TLS: MainTls = const { MainTls(Cell::new(0)) };
    static LATE_TLS: LateTls = const { LateTls };
}

impl Drop for MainTls {
    fn drop(&mut self) {
        if self.0.get() != 0x51a7
            || MAIN_TLS.try_with(|_| ()).is_ok()
            || std::thread::current().name() != Some("main")
        {
            DTOR_ERRORS.fetch_add(1, Ordering::AcqRel);
        }
        // This first access must also be destroyed before final runtime cleanup.
        LATE_TLS.with(|_| ());
        MAIN_DROPS.fetch_add(1, Ordering::AcqRel);
    }
}

impl Drop for LateTls {
    fn drop(&mut self) {
        if std::thread::current().name() != Some("main") {
            DTOR_ERRORS.fetch_add(1, Ordering::AcqRel);
        }
        LATE_DROPS.fetch_add(1, Ordering::AcqRel);
    }
}

fn check_entry_and_arguments(logger: &mut ProbeLogger) {
    logger.section("NORMAL RUST MAIN ENTRY");
    let snapshot = runtime::runtime_snapshot();
    logger.record(
        "early system and heap startup completed",
        ogc_sys::std_startup_result() == Some(0) && runtime::heap_capacity() > 0,
    );
    logger.record(
        "compiler-generated entry selected Wii PAL",
        snapshot.phase == RuntimePhase::Running,
    );
    logger.record(
        "Rust main is the initial LWP thread",
        snapshot.initial_thread == ogc_lwp_driver::current_thread(),
    );
    logger.record(
        "Rust main has its standard name",
        std::thread::current().name() == Some("main"),
    );
    let mut native_name = [0; ogc_lwp_driver::THREAD_NAME_CAPACITY];
    logger.record(
        "main name is stored in production scheduler",
        ogc_lwp_driver::thread_name(snapshot.initial_thread, &mut native_name) == Ok(4)
            && &native_name[..4] == b"main",
    );

    let mut count = usize::MAX;
    let mut vector = core::ptr::null();
    let result = unsafe { runtime::__wii_std_arguments(&mut count, &mut vector) };
    let loaded = ogc_sys::startup_arguments();
    logger.record(
        "std argument provider uses the sheltered vector",
        result == 0
            && loaded.is_some_and(|(argc, argv)| {
                argc as usize == count && argv as usize == vector as usize
            }),
    );
    let arguments: Vec<_> = std::env::args_os().collect();
    let independently_read = if result == 0 {
        // Read the actual loader representation independently of std's iterator.
        (0..count)
            .map(|index| unsafe {
                core::ffi::CStr::from_ptr((*vector.add(index)).cast())
                    .to_bytes()
                    .to_vec()
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    logger.record(
        "args_os preserves actual loader argument bytes",
        result == 0
            && arguments.len() == count
            && arguments
                .iter()
                .zip(&independently_read)
                .all(|(argument, expected)| argument.as_encoded_bytes() == expected),
    );
    let reversed: Vec<_> = std::env::args_os().rev().collect();
    logger.record(
        "argument iterators own independent double-ended snapshots",
        reversed.iter().eq(arguments.iter().rev()),
    );
    logger.record(
        "malformed argument output is rejected",
        unsafe { runtime::__wii_std_arguments(core::ptr::null_mut(), &mut vector) }
            == ogc_sysbase::EINVAL,
    );
    logger.record(
        "repeated compiler entry is rejected",
        runtime::__wii_std_runtime_enter(snapshot.argc as isize, snapshot.argv as *const *const u8)
            == ogc_sysbase::EBUSY,
    );
    logger.record(
        "rejected entry did not change runtime state",
        runtime::runtime_snapshot() == snapshot,
    );
}

extern "C" fn after_main(status: i32) -> ! {
    if !LOGGER_READY.load(Ordering::Acquire) {
        result().store(-2, Ordering::Release);
        loop {
            core::hint::spin_loop();
        }
    }
    // SAFETY: main published the initialized logger before it returned. This
    // callback is its sole remaining owner and runs on the initial stack.
    let logger = unsafe { (&mut *EXIT_LOGGER.0.get()).assume_init_mut() };
    logger.section("AFTER COMPILER-GENERATED MAIN RETURN");
    let snapshot = runtime::runtime_snapshot();
    logger.record(
        "main return status reached the real exit callback",
        status == 42 && snapshot.return_status == Some(42),
    );
    logger.record(
        "runtime cleanup finished before the exit callback",
        snapshot.phase == RuntimePhase::Returned
            && snapshot.cleanup_result == Some(0)
            && ogc_sys::std_return_result() == Some(0),
    );
    logger.record(
        "initial-thread and late TLS destructors completed",
        MAIN_DROPS.load(Ordering::Acquire) == 1
            && LATE_DROPS.load(Ordering::Acquire) == 1
            && DTOR_ERRORS.load(Ordering::Acquire) == 0,
    );
    logger.record(
        "cleanup after return is rejected",
        unsafe { runtime::__wii_std_runtime_cleanup() } == ogc_sysbase::EBUSY,
    );
    logger.record(
        "second return does not overwrite committed status",
        unsafe { runtime::__wii_std_runtime_return(-99) } == ogc_sysbase::EBUSY
            && runtime::runtime_snapshot() == snapshot,
    );
    logger.record(
        "no TLS cleanup failure was hidden",
        ogc_lwp_driver::thread_cleanup_counters().tls_failures
            == std_thread_probe::expected_tls_failures(),
    );
    let return_after = RETURN_TO_MENU_AFTER_SECONDS.load(Ordering::Acquire);
    if return_after != 0 {
        logger.note("Returning to System Menu after the summary delay.");
    }
    logger.summary();
    result().store(
        if logger.checks_passed() { 0 } else { -1 },
        Ordering::Release,
    );
    // Keep the visible result on screen unless the caller selected an
    // automatic return. Report collection completes before this callback.
    let mut page = 0;
    let mut pressed = false;
    let started = ogc_lwp_driver::now_ticks();
    if !logger.checks_passed() {
        logger.show_failure_page(page);
    }
    loop {
        if return_after != 0
            && ogc_lwp_driver::now_ticks().wrapping_sub(started)
                >= u64::from(return_after) * ogc_lwp_driver::TICKS_PER_SECOND
        {
            // SAFETY: This is the explicit probe option selected by its entry
            // point after the report and summary have been committed.
            unsafe {
                ogc_system::reset_system(ogc_system::SYS_RETURNTOMENU as i32, 0, 0);
            }
            loop {
                core::hint::spin_loop();
            }
        }
        let down = ogc_system::reset_button_down() != 0;
        if down && !pressed && !logger.checks_passed() {
            page = (page + 1) % logger.failure_pages();
            logger.show_failure_page(page);
        }
        pressed = down;
        probe_support::wait_vsync();
    }
}

pub fn run(logger: &mut ProbeLogger) {
    check_entry_and_arguments(logger);
    MAIN_TLS.with(|value| value.0.set(0x51a7));
    std_thread_probe::run(logger);
}

pub fn finish(logger: ProbeLogger, output: &'static AtomicI32) -> ExitCode {
    finish_with_menu_return(logger, output, 0)
}

pub fn finish_with_menu_return(
    logger: ProbeLogger,
    output: &'static AtomicI32,
    seconds: u32,
) -> ExitCode {
    RETURN_TO_MENU_AFTER_SECONDS.store(seconds, Ordering::Release);
    RESULT.store(core::ptr::from_ref(output).cast_mut(), Ordering::Release);
    // Keep the logger alive across main return and real initial-thread cleanup.
    unsafe { (*EXIT_LOGGER.0.get()).write(logger) };
    LOGGER_READY.store(true, Ordering::Release);
    ogc_sys::set_exit_callback(Some(after_main));
    ExitCode::from(42)
}
