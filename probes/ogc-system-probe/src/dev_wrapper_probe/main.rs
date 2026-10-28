#![no_std]
#![no_main]

extern crate alloc;
#[cfg(ogc_dev)]
mod screen_scale;
#[cfg(ogc_dev)]
mod stub_entry;

use core::sync::atomic::{AtomicBool, Ordering};
#[cfg(ogc_dev)]
use core::{fmt::Write, sync::atomic::AtomicI32};
const _: &ogc_system_probe::alloc_support::BumpAllocator =
    &ogc_system_probe::alloc_support::ALLOCATOR;

// This probe tests the development wrapper's real process entry, return result,
// and independent deadline. It does not claim to test application hardware.
#[unsafe(no_mangle)]
pub static OGC_DEV_PROBE_ENTERED: AtomicBool = AtomicBool::new(false);

#[cfg(ogc_dev)]
unsafe extern "C" {
    static OGC_DEV_STATE: core::sync::atomic::AtomicU32;
    static OGC_DEV_REPORT_RESULT: AtomicI32;
    static OGC_DEV_ENTRY_STAGE: core::sync::atomic::AtomicU32;
    static OGC_DEV_ENTRY_RESULT: AtomicI32;
    static OGC_DEV_RESET_SEQUENCE: core::sync::atomic::AtomicU32;
    static OGC_DEV_RESET_STAGE: core::sync::atomic::AtomicU32;
    static OGC_DEV_RESET_PRIORITY: core::sync::atomic::AtomicU32;
    static OGC_DEV_RESET_RESULT: AtomicI32;
    static OGC_DEV_RESET_DSP_CONTROL: core::sync::atomic::AtomicU32;
    static OGC_DEV_RESET_DSP_MAILBOX: core::sync::atomic::AtomicU32;
    static OGC_DEV_STUB_COMPARE_RESULT: AtomicI32;
    static OGC_DEV_STUB_ENTRY_WORD: core::sync::atomic::AtomicU32;
    static OGC_DEV_STUB_CONFIG_HIGH: core::sync::atomic::AtomicU32;
    static OGC_DEV_STUB_CONFIG_LOW: core::sync::atomic::AtomicU32;
    static OGC_DEV_STUB_SHA1: [core::sync::atomic::AtomicU32; 5];
    static OGC_DEV_STUB_HASH_RESULT: AtomicI32;
    static OGC_DEV_STUB_REAL_WORDS: [core::sync::atomic::AtomicU32; 2];
    static OGC_DEV_STUB_REAL_TARGET: core::sync::atomic::AtomicU32;
    static OGC_DEV_STUB_REAL_RESULT: AtomicI32;
}

/// Zero means the last synchronous reset screen update wrote all bytes.
#[cfg(ogc_dev)]
#[unsafe(no_mangle)]
pub static OGC_DEV_PROBE_RESET_SCREEN_RESULT: AtomicI32 = AtomicI32::new(i32::MIN);

#[cfg(ogc_dev)]
static TRACE_STUB_ENTRY: AtomicBool = AtomicBool::new(false);
#[cfg(ogc_dev)]
static STUB_ORIGINAL_ENTRY: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
#[cfg(ogc_dev)]
#[unsafe(no_mangle)]
pub static OGC_DEV_PROBE_STUB_PATCH_RESULT: AtomicI32 = AtomicI32::new(i32::MIN);
#[cfg(ogc_dev)]
#[unsafe(no_mangle)]
pub static OGC_DEV_PROBE_STUB_ENTRY_SEEN: AtomicBool = AtomicBool::new(false);
#[cfg(ogc_dev)]
#[unsafe(no_mangle)]
pub static OGC_DEV_PROBE_STUB_RESTORE_RESULT: AtomicI32 = AtomicI32::new(i32::MIN);

// This observer runs only with the explicit probe option. It restores the
// original instruction before re-entering the real caller stub at 0x80001800.
// Preserve the incoming stack and LR around the ordinary C ABI observer call.
#[cfg(ogc_dev)]
core::arch::global_asm!(
    ".section .text.__ogc_dev_probe_stub_entry,\"ax\",@progbits",
    ".balign 4",
    ".global __ogc_dev_probe_stub_entry",
    "__ogc_dev_probe_stub_entry:",
    "stwu 1, -32(1)",
    "mflr 0",
    "stw 0, 36(1)",
    "bl __ogc_dev_probe_stub_observed",
    "mtctr 3",
    "lwz 0, 36(1)",
    "mtlr 0",
    "addi 1, 1, 32",
    "bctr",
);

#[cfg(ogc_dev)]
fn publish_stub_entry(word: u32) -> Result<(), i32> {
    // SAFETY: the probe owns this aligned entry word during its final handoff.
    unsafe {
        core::ptr::write_volatile(0x80001800 as *mut u32, word);
        ogc_cache::dc_flush_range((0x80001800 as *const u32).cast(), 4);
        ogc_cache::ic_invalidate_range((0x80001800 as *const u32).cast(), 4);
        ogc_cache::sync();
        ogc_cache::ic_sync();
        if core::ptr::read_volatile(0xc0001800 as *const u32) != word {
            return Err(-84);
        }
    }
    Ok(())
}

#[cfg(ogc_dev)]
fn prepare_stub_entry_trace() -> Result<(), i32> {
    unsafe extern "C" {
        fn __ogc_dev_probe_stub_entry();
    }
    // SAFETY: the runtime has published these before calling this observer.
    let original = unsafe {
        if OGC_DEV_STUB_COMPARE_RESULT.load(Ordering::Acquire) != 0 {
            return Err(-84);
        }
        OGC_DEV_STUB_ENTRY_WORD.load(Ordering::Acquire)
    };
    // Accept only a relative, non-linking branch into the saved stub image.
    // Do not relocate or interpret an unsupported entry instruction.
    let observer = __ogc_dev_probe_stub_entry as *const () as usize;
    let branch = stub_entry::observer_branch(original, observer)?;
    STUB_ORIGINAL_ENTRY.store(original, Ordering::Release);
    if let Err(error) = publish_stub_entry(branch) {
        let rollback = publish_stub_entry(original);
        OGC_DEV_PROBE_STUB_RESTORE_RESULT.store(rollback.err().unwrap_or(0), Ordering::Release);
        return Err(error);
    }
    Ok(())
}

#[cfg(ogc_dev)]
#[unsafe(no_mangle)]
extern "C" fn __ogc_dev_probe_stub_observed() -> u32 {
    OGC_DEV_PROBE_STUB_ENTRY_SEEN.store(true, Ordering::Release);
    let restored = publish_stub_entry(STUB_ORIGINAL_ENTRY.load(Ordering::Acquire));
    OGC_DEV_PROBE_STUB_RESTORE_RESULT.store(restored.err().unwrap_or(0), Ordering::Release);
    // SAFETY: use the existing synchronous renderer with the actual last event.
    unsafe {
        reset_progress(
            OGC_DEV_RESET_STAGE.load(Ordering::Acquire),
            OGC_DEV_RESET_PRIORITY.load(Ordering::Acquire),
            OGC_DEV_RESET_RESULT.load(Ordering::Acquire),
        )
    };
    if OGC_DEV_PROBE_STUB_RESTORE_RESULT.load(Ordering::Acquire) != 0 {
        // Preserve the failed image and visible evidence. Do not recurse into
        // an entry instruction whose restoration did not complete.
        loop {
            core::hint::spin_loop();
        }
    }
    0x80001800
}

#[cfg(ogc_dev)]
struct ResetConsole {
    error: i32,
}

#[cfg(ogc_dev)]
impl Write for ResetConsole {
    fn write_str(&mut self, text: &str) -> core::fmt::Result {
        self.error = match ogc_video::console_write(text.as_bytes()) {
            Ok(count) if count == text.len() => return Ok(()),
            Ok(_) => -5,
            Err(error) => error,
        };
        Err(core::fmt::Error)
    }
}

#[cfg(ogc_dev)]
struct StubDestination;
#[cfg(ogc_dev)]
impl core::fmt::Display for StubDestination {
    fn fmt(&self, output: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let result = unsafe { OGC_DEV_STUB_REAL_RESULT.load(Ordering::Acquire) };
        if result == 0 {
            write!(output, "Real target={:#010x}", unsafe {
                OGC_DEV_STUB_REAL_TARGET.load(Ordering::Acquire)
            })
        } else {
            write!(output, "Real target unknown err={}", EvidenceResult(result))
        }
    }
}
#[cfg(ogc_dev)]
struct EvidenceResult(i32);
#[cfg(ogc_dev)]
impl core::fmt::Display for EvidenceResult {
    fn fmt(&self, output: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        if self.0 == i32::MIN {
            output.write_str("pending")
        } else {
            write!(output, "{}", self.0)
        }
    }
}

// Scale the direct uncached console while the reset observer holds IRQs off.
#[cfg(ogc_dev)]
unsafe fn enlarge_reset_screen() -> Result<(), i32> {
    let console = unsafe { &*ogc_video::console_get_default() };
    if !console.console_initialised
        || console.destbuffer.is_null()
        || console.target_x < 0
        || console.target_y < 0
        || console.con_xres < 576
        || console.con_yres < 416
        || console.con_stride != console.tgt_stride
        || console.con_stride < 1152
    {
        return Err(-22);
    }
    let stride = console.con_stride as usize / 4;
    let offset = console.target_y as usize * stride + console.target_x as usize / 2;
    let pixels = unsafe {
        core::slice::from_raw_parts_mut(
            console.destbuffer.cast::<u32>().add(offset),
            415 * stride + 288,
        )
    };
    screen_scale::double_text(pixels, stride, 288, 416)
}

#[cfg(ogc_dev)]
#[inline(never)]
unsafe fn reset_progress(stage: u32, priority: u32, result: i32) {
    // Use the initialized uncached console directly. This works when the main
    // loop cannot run and after IRQ shutdown. No heap, network, log lock, or
    // vertical-sync wait is used. Keep each event in the same screen rows.
    let interrupt_state = ogc_irq::irq_disable();
    if stage == ogc_system::sys::SYS_RESET_TRACE_RELOAD_STUB
        && TRACE_STUB_ENTRY.load(Ordering::Acquire)
        && !OGC_DEV_PROBE_STUB_ENTRY_SEEN.load(Ordering::Acquire)
    {
        let prepared = prepare_stub_entry_trace();
        OGC_DEV_PROBE_STUB_PATCH_RESULT.store(prepared.err().unwrap_or(0), Ordering::Release);
    }
    ogc_video::console_clear();
    let mut console = ResetConsole { error: 0 };
    // SAFETY: --dev supplies these process-lifetime evidence statics. IRQs are
    // disabled while this single-core snapshot and screen write take place.
    let formatted = unsafe {
        write!(
            console,
            "\x1b[1;1H\x1b[2KRecovery probe (2x text)\n\
             \x1b[2KRecovery={} report={}\n\
             \x1b[2KEntry stage={} result={}\n\
             \x1b[2KReset #{} event={}\n\
             \x1b[2KStep={} result={}\n\
             \x1b[2KDSP={:08x} mail={:08x}\n\
             \x1b[2KStub={} pc={:#010x}\n\
             \x1b[2KConfig={:08x} {:08x}\n\
             \x1b[2KLoad={:08x} {:08x}\n\
             \x1b[2K{}\n\
             \x1b[2KP={} seen={} R={}\n\
             \x1b[2KSHA1={:08x}{:08x}\n\
             \x1b[2K{:08x}{:08x}{:08x} err={}\n",
            OGC_DEV_STATE.load(Ordering::Acquire),
            OGC_DEV_REPORT_RESULT.load(Ordering::Acquire),
            OGC_DEV_ENTRY_STAGE.load(Ordering::Acquire),
            OGC_DEV_ENTRY_RESULT.load(Ordering::Acquire),
            OGC_DEV_RESET_SEQUENCE.load(Ordering::Acquire),
            stage,
            priority,
            result,
            OGC_DEV_RESET_DSP_CONTROL.load(Ordering::Acquire),
            OGC_DEV_RESET_DSP_MAILBOX.load(Ordering::Acquire),
            OGC_DEV_STUB_COMPARE_RESULT.load(Ordering::Acquire),
            OGC_DEV_STUB_ENTRY_WORD.load(Ordering::Acquire),
            OGC_DEV_STUB_CONFIG_HIGH.load(Ordering::Acquire),
            OGC_DEV_STUB_CONFIG_LOW.load(Ordering::Acquire),
            OGC_DEV_STUB_REAL_WORDS[0].load(Ordering::Acquire),
            OGC_DEV_STUB_REAL_WORDS[1].load(Ordering::Acquire),
            StubDestination,
            EvidenceResult(OGC_DEV_PROBE_STUB_PATCH_RESULT.load(Ordering::Acquire)),
            OGC_DEV_PROBE_STUB_ENTRY_SEEN.load(Ordering::Acquire),
            EvidenceResult(OGC_DEV_PROBE_STUB_RESTORE_RESULT.load(Ordering::Acquire)),
            OGC_DEV_STUB_SHA1[0].load(Ordering::Acquire),
            OGC_DEV_STUB_SHA1[1].load(Ordering::Acquire),
            OGC_DEV_STUB_SHA1[2].load(Ordering::Acquire),
            OGC_DEV_STUB_SHA1[3].load(Ordering::Acquire),
            OGC_DEV_STUB_SHA1[4].load(Ordering::Acquire),
            EvidenceResult(OGC_DEV_STUB_HASH_RESULT.load(Ordering::Acquire)),
        )
    };
    let screen_result = if formatted.is_ok() {
        unsafe { enlarge_reset_screen() }.err().unwrap_or(0)
    } else {
        console.error
    };
    OGC_DEV_PROBE_RESET_SCREEN_RESULT.store(screen_result, Ordering::Release);
    let restore_result = OGC_DEV_PROBE_STUB_RESTORE_RESULT.load(Ordering::Acquire);
    if restore_result < 0 && restore_result != i32::MIN {
        loop {
            core::hint::spin_loop();
        }
    }
    ogc_irq::irq_restore(interrupt_state);
}

#[unsafe(no_mangle)]
pub extern "C" fn main(argc: i32, argv: *mut *mut u8) -> i32 {
    OGC_DEV_PROBE_ENTERED.store(true, Ordering::Release);
    ogc_system::report(format_args!(
        "INFO development wrapper probe entered main\n"
    ));
    let mut stall = false;
    let mut fail = false;
    let mut screen = false;
    let mut overflow = false;
    let mut trace_stub = false;
    if argc > 0 && !argv.is_null() {
        for index in 0..argc as usize {
            // SAFETY: the runtime passes its sheltered argc/argv to this probe.
            let pointer = unsafe { *argv.add(index) };
            if pointer.is_null() {
                return -22;
            }
            let argument = unsafe { core::ffi::CStr::from_ptr(pointer.cast()) }.to_bytes();
            if argument == b"--dev-probe-network-init" {
                let result = __ogc_dev_probe_prepare_network();
                if result < 0 {
                    return result;
                }
            }
            stall |= argument == b"--dev-probe-stall";
            fail |= argument == b"--dev-probe-fail";
            screen |= argument == b"--dev-probe-screen";
            overflow |= argument == b"--dev-probe-log-overflow";
            trace_stub |= argument == b"--dev-probe-trace-stub-entry";
        }
    }
    if trace_stub {
        if !screen || !stall {
            return -22;
        }
        #[cfg(not(ogc_dev))]
        return -95;
        #[cfg(ogc_dev)]
        TRACE_STUB_ENTRY.store(true, Ordering::Release);
    }
    if overflow {
        for _ in 0..2048 {
            ogc_system::report(format_args!(
                "development log overflow acceptance payload\n"
            ));
        }
    }
    if screen {
        let display = ogc_system_probe::probe_support::ProbeDisplay::init();
        if !display.console_ready() {
            return -5;
        }
        if ogc_video::console_write(b"Development wrapper probe entered main\n").is_err() {
            return -5;
        }
        #[cfg(ogc_dev)]
        {
            // Initialize the evidence display before installing the observer.
            unsafe {
                reset_progress(
                    OGC_DEV_RESET_STAGE.load(Ordering::Acquire),
                    OGC_DEV_RESET_PRIORITY.load(Ordering::Acquire),
                    OGC_DEV_RESET_RESULT.load(Ordering::Acquire),
                )
            };
            if OGC_DEV_PROBE_RESET_SCREEN_RESULT.load(Ordering::Acquire) != 0 {
                return -5;
            }
            if ogc_system::sys::set_reset_debug_hook(Some(reset_progress)).is_some() {
                ogc_system::report(format_args!("INFO probe replaced reset observer\n"));
            }
        }
    }
    if stall {
        loop {
            core::hint::spin_loop();
        }
    }
    if fail { -7 } else { 0 }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}

/// Emulator setup entry. The host calls this real initialization path before
/// development entry when a report-delivery test needs an active interface.
#[unsafe(no_mangle)]
#[inline(never)]
pub extern "C" fn __ogc_dev_probe_prepare_network() -> i32 {
    ogc_system::report(format_args!("INFO acceptance network before system init\n"));
    ogc_system::init();
    ogc_system::report(format_args!("INFO acceptance network before async init\n"));
    let started = unsafe { ogc_network::init_async(None, core::ptr::null_mut()) };
    ogc_system::report(format_args!(
        "INFO acceptance network submission={started}\n"
    ));
    if started < 0 {
        return started;
    }
    let before = ogc_system::time();
    loop {
        let status = ogc_network::get_status();
        if status != -16 {
            ogc_system::report(format_args!("INFO acceptance network final={status}\n"));
            return if status < 0 {
                status
            } else {
                ogc_network::soc_init()
            };
        }
        if ogc_system::time().wrapping_sub(before) > 20 * ogc_lwp::sys::TICKS_PER_SECOND {
            return -110;
        }
        ogc_lwp::sys::sleep_ticks(ogc_lwp::sys::TICKS_PER_SECOND / 50);
    }
}
