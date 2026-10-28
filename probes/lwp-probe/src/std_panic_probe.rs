//! Terminal probe. Acceptance requires the default panic text on the screen.
use core::sync::atomic::{AtomicU32, Ordering};
#[unsafe(no_mangle)]
#[used]
pub static WII_STD_PANIC_DROPS: AtomicU32 = AtomicU32::new(0);
struct Guard;
impl Drop for Guard {
    fn drop(&mut self) {
        WII_STD_PANIC_DROPS.fetch_add(1, Ordering::Release);
    }
}
pub fn run(logger: &mut ogc_system_probe::probe_support::ProbeLogger) -> ! {
    logger.section("STD PANIC ABORT DIAGNOSTICS");
    logger.record(
        "std runtime is running",
        ogc_std_runtime::runtime_snapshot().phase == ogc_std_runtime::RuntimePhase::Running,
    );
    logger.note("Expect default panic text: WII_STD_PANIC_MARKER_51A7");
    logger.note("This terminal probe has no PASS summary. It must stop here.");
    // Keep the default std hook. Replacing it would hide a broken std sink.
    let _guard = Guard;
    // Keep the drop counter observable in the DOL so a debugger can verify
    // that panic=abort does not run stack destructors.
    let drops = unsafe { core::ptr::read_volatile(core::ptr::addr_of!(WII_STD_PANIC_DROPS)) };
    logger.note_u32("drop count before panic", drops.load(Ordering::Acquire));
    panic!("WII_STD_PANIC_MARKER_51A7");
}
