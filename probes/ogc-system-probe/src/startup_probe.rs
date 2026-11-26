use ogc_system::{self as system, SystemInitStep, SystemPreMainStep};

use crate::probe_support::ProbeLogger;

#[cfg(feature = "dolphin-automation")]
use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};

#[cfg(feature = "dolphin-automation")]
static SI_COMPLETIONS: AtomicU32 = AtomicU32::new(0);
#[cfg(feature = "dolphin-automation")]
static SI_CHANNEL: AtomicI32 = AtomicI32::new(-1);
#[cfg(feature = "dolphin-automation")]
static SI_RESULT: AtomicU32 = AtomicU32::new(u32::MAX);

#[cfg(feature = "dolphin-automation")]
unsafe extern "C" fn si_completion(channel: i32, result: u32) {
    SI_CHANNEL.store(channel, Ordering::Release);
    SI_RESULT.store(result, Ordering::Release);
    SI_COMPLETIONS.fetch_add(1, Ordering::Release);
}

pub fn run(logger: &mut ProbeLogger) {
    logger.section("STARTUP PROBE");

    let before = system::time();
    let runtime = system::snapshot_runtime_calls();
    let pre_main = system::snapshot_pre_main();
    let startup = system::snapshot_startup();
    let expected_init = system::system_init_sequence(true, false, false);
    let expected_premain = system::system_premain_sequence(true);
    let after = system::time();

    logger.note_u64("startup time before", before);
    logger.note_u64("startup time after", after);
    logger.note_u32("expected init steps", expected_init.len() as u32);
    logger.note_u32("expected premain steps", expected_premain.len() as u32);

    logger.record("startup time monotonic", after >= before);
    logger.record("runtime init called", runtime.init_called);
    logger.record("runtime pre-main called", runtime.pre_main_called);
    logger.record("runtime pre-main completed", pre_main.completed);
    logger.note_u32("runtime pre-main failed stage", pre_main.failed_stage);
    logger.note_i32("runtime pre-main result", pre_main.result);
    logger.record("low memory init called", startup.low_mem_init_called);
    #[cfg(feature = "dolphin-automation")]
    probe_si_startup(
        logger,
        startup.init_completed && startup.init_completed_mask & (1 << 6) != 0,
    );
    logger.record(
        "wii init sequence includes ipc client",
        contains_init(expected_init, SystemInitStep::IpcClientInit),
    );
    logger.record(
        "wii pre-main sequence includes ios init",
        contains_premain(expected_premain, SystemPreMainStep::InitializeIosSubsystems),
    );
}

#[cfg(feature = "dolphin-automation")]
fn probe_si_startup(logger: &mut ProbeLogger, si_init_completed: bool) {
    use ogc_si as si;

    // The startup path has already called SI initialization before main.
    // Allow its initial type requests to complete through the SI interrupt.
    let mut settled = false;
    for _ in 0..120 {
        if si::busy() == 0 && si::is_chan_busy(0) == 0 && si::is_chan_busy(3) == 0 {
            settled = true;
            break;
        }
        if !ogc_video::wait_vsync_timeout_us(100_000) {
            break;
        }
    }
    SI_COMPLETIONS.store(0, Ordering::Release);
    SI_CHANNEL.store(-1, Ordering::Release);
    SI_RESULT.store(u32::MAX, Ordering::Release);
    let initial_type = si::get_type_async(0, Some(si_completion));
    for _ in 0..120 {
        if SI_COMPLETIONS.load(Ordering::Acquire) != 0 {
            break;
        }
        if !ogc_video::wait_vsync_timeout_us(100_000) {
            break;
        }
    }
    let controller_type = SI_RESULT.load(Ordering::Acquire);
    logger.note_u32("startup SI initial type result", initial_type);
    logger.note_u32("startup SI controller type", controller_type);
    logger.record(
        "SI startup discovers the attached controller",
        si_init_completed
            && settled
            && SI_COMPLETIONS.load(Ordering::Acquire) == 1
            && SI_CHANNEL.load(Ordering::Acquire) == 0
            && controller_type == si::SI_GC_CONTROLLER,
    );
    if SI_COMPLETIONS.load(Ordering::Acquire) != 1 {
        logger.record("SI startup delivers a transfer interrupt", false);
        return;
    }

    // Wait for any other startup transfers before starting the next request.
    let mut idle = false;
    for _ in 0..120 {
        if si::busy() == 0 && si::is_chan_busy(3) == 0 {
            idle = true;
            break;
        }
        if !ogc_video::wait_vsync_timeout_us(100_000) {
            break;
        }
    }
    if !idle {
        logger.record("SI startup delivers a transfer interrupt", false);
        return;
    }
    SI_COMPLETIONS.store(0, Ordering::Release);
    SI_CHANNEL.store(-1, Ordering::Release);
    SI_RESULT.store(u32::MAX, Ordering::Release);
    let output = 0u32;
    let mut input = 0xa5a5_a5a5u32;
    // SAFETY: The aligned buffers stay live until the callback completes.
    let submitted = unsafe {
        si::transfer(
            3,
            (&output as *const u32).cast(),
            1,
            (&mut input as *mut u32).cast(),
            3,
            Some(si_completion),
            0,
        )
    };
    for _ in 0..120 {
        if SI_COMPLETIONS.load(Ordering::Acquire) != 0 {
            break;
        }
        if !ogc_video::wait_vsync_timeout_us(100_000) {
            break;
        }
    }
    let completed = SI_COMPLETIONS.load(Ordering::Acquire) == 1;
    logger.record(
        "SI startup delivers a transfer interrupt",
        submitted == 1
            && completed
            && SI_CHANNEL.load(Ordering::Acquire) == 3
            && SI_RESULT.load(Ordering::Acquire) == si::SI_ERROR_NO_RESPONSE
            && si::busy() == 0
            && input & 0xff == 0xa5,
    );
    if submitted == 1 && !completed {
        // Keep the stack buffers live until the outstanding transfer is drained.
        let _ = si::sync();
    }
}

fn contains_init(steps: &[SystemInitStep], wanted: SystemInitStep) -> bool {
    steps.iter().copied().any(|step| step == wanted)
}

fn contains_premain(steps: &[SystemPreMainStep], wanted: SystemPreMainStep) -> bool {
    steps.iter().copied().any(|step| step == wanted)
}
