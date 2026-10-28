//! Exercise stream callbacks through the real DSP and inspect its output RAM.
use crate::probe_support::{self, ProbeLogger};
use aesnd_sys as snd;
use core::ffi::c_void;
use core::sync::atomic::{AtomicPtr, AtomicU32, Ordering};

#[repr(align(32))]
struct Samples([u8; 16_384]);

const fn samples(high: u8) -> Samples {
    let mut bytes = [0; 16_384];
    let mut index = 0;
    while index < bytes.len() {
        bytes[index] = high;
        index += 2;
    }
    Samples(bytes)
}

// Big-endian signed mono PCM: +4096 followed by a separate -4096 source.
// Both allocations remain valid through every callback, stop, and DSP reset.
static INITIAL: Samples = samples(0x10);
static REPLACEMENT: Samples = samples(0xf0);
static AUDIO: AtomicU32 = AtomicU32::new(0);
static INITIAL_OUTPUT: AtomicU32 = AtomicU32::new(0);
static REPLACEMENT_OUTPUT: AtomicU32 = AtomicU32::new(0);
static SILENCE: AtomicU32 = AtomicU32::new(0);
static RUNNING: AtomicU32 = AtomicU32::new(0);
static STREAM: AtomicU32 = AtomicU32::new(0);
static INVALID: AtomicU32 = AtomicU32::new(0);
static OUTPUT_EVIDENCE: crate::aesnd_output_evidence::OutputEvidence =
    crate::aesnd_output_evidence::OutputEvidence::new();
static ORIGINAL_VOICE: AtomicPtr<snd::Aesndpb> = AtomicPtr::new(core::ptr::null_mut());
static REPLACEMENT_VOICE: AtomicPtr<snd::Aesndpb> = AtomicPtr::new(core::ptr::null_mut());

#[derive(Clone, Copy)]
enum Case {
    Once = 1,
    Loop = 2,
    RestartAsLoop = 3,
    Reallocate = 4,
}

unsafe extern "C" fn output(buffer: *mut c_void, len: u32, arg: *mut c_void) {
    AUDIO.fetch_add(1, Ordering::Relaxed);
    if buffer.is_null() || len != 384 || !(1..=4).contains(&(arg as usize)) {
        INVALID.fetch_add(1, Ordering::Relaxed);
        return;
    }
    // SAFETY: AESND supplies the current 384-byte, CPU-readable output buffer.
    let bytes = unsafe { core::slice::from_raw_parts(buffer.cast::<u8>(), 384) };
    OUTPUT_EVIDENCE.observe(bytes);
    if crate::aesnd_output_evidence::matches_pcm(
        bytes,
        &crate::aesnd_output_evidence::INITIAL_FRAME,
    ) {
        INITIAL_OUTPUT.fetch_add(1, Ordering::Relaxed);
    } else if crate::aesnd_output_evidence::matches_pcm(
        bytes,
        &crate::aesnd_output_evidence::REPLACEMENT_FRAME,
    ) {
        REPLACEMENT_OUTPUT.fetch_add(1, Ordering::Relaxed);
    } else if bytes.iter().all(|byte| *byte == 0) {
        SILENCE.fetch_add(1, Ordering::Relaxed);
    }
}

unsafe extern "C" fn voice(pb: *mut snd::Aesndpb, event: u32, arg: *mut c_void) {
    let mode = arg as usize;
    if pb.is_null() || !(1..=4).contains(&mode) {
        INVALID.fetch_add(1, Ordering::Relaxed);
        return;
    }
    if event == snd::VOICE_STATE_RUNNING {
        RUNNING.fetch_add(1, Ordering::Relaxed);
    } else if event == snd::VOICE_STATE_STREAM {
        let previous = STREAM.fetch_add(1, Ordering::Relaxed);
        if previous != 0 {
            INVALID.fetch_add(1, Ordering::Relaxed);
            return;
        }
        if mode == Case::Reallocate as usize {
            snd::free_voice(pb);
            let replacement = unsafe { snd::allocate_voice_with_arg(Some(voice), arg) };
            REPLACEMENT_VOICE.store(replacement, Ordering::Release);
            if replacement.is_null() || replacement != ORIGINAL_VOICE.load(Ordering::Acquire) {
                INVALID.fetch_add(1, Ordering::Relaxed);
                return;
            }
            snd::set_voice_volume(replacement, 0x80, 0x40);
            unsafe {
                snd::play_voice(
                    replacement,
                    snd::VOICE_MONO16,
                    REPLACEMENT.0.as_ptr().cast(),
                    REPLACEMENT.0.len() as u32,
                    48_000.0,
                    0,
                    1,
                );
            }
            return;
        }
        // Change the command through production setters. Do not edit private
        // state, use artificial addresses, or call the request engine directly.
        snd::set_voice_stream(pb, 0);
        if mode == Case::RestartAsLoop as usize {
            // This clears ONCE and sets LOOP through the public playback API.
            unsafe {
                snd::play_voice(
                    pb,
                    snd::VOICE_MONO16,
                    REPLACEMENT.0.as_ptr().cast(),
                    REPLACEMENT.0.len() as u32,
                    48_000.0,
                    0,
                    1,
                );
            }
        } else {
            unsafe {
                snd::set_voice_buffer(
                    pb,
                    REPLACEMENT.0.as_ptr().cast(),
                    REPLACEMENT.0.len() as u32,
                );
            }
        }
    }
}

fn frames(count: u32) {
    for _ in 0..count {
        // The shared VI wait has a 100 ms deadline and records timeouts.
        probe_support::wait_vsync();
    }
}

fn run_case(logger: &mut ProbeLogger, case: Case, label: &str) -> bool {
    logger.section(label);
    logger.report_step(label);
    OUTPUT_EVIDENCE.reset();
    for counter in [
        &AUDIO,
        &INITIAL_OUTPUT,
        &REPLACEMENT_OUTPUT,
        &SILENCE,
        &RUNNING,
        &STREAM,
        &INVALID,
    ] {
        counter.store(0, Ordering::Release);
    }
    ORIGINAL_VOICE.store(core::ptr::null_mut(), Ordering::Release);
    REPLACEMENT_VOICE.store(core::ptr::null_mut(), Ordering::Release);
    logger.report_step("AESND stream: initialize DSP");
    snd::init();
    let arg = case as usize as *mut c_void;
    // The context is an opaque case ID. Callbacks validate it and never
    // dereference it. Audio samples have permanent aligned storage.
    let previous = unsafe { snd::register_audio_callback_with_arg(Some(output), arg) };
    logger.record("fresh AESND audio callback slot", previous.is_none());
    let pb = unsafe { snd::allocate_voice_with_arg(Some(voice), arg) };
    logger.record("AESND stream voice allocated", !pb.is_null());
    if pb.is_null() {
        snd::reset();
        logger.record(
            "AESND DMA stopped after allocation failure",
            ogc_audio::get_dma_enable_flag() == 0,
        );
        return false;
    }
    ORIGINAL_VOICE.store(pb, Ordering::Release);
    snd::set_voice_volume(pb, 0x80, 0x40);
    snd::set_voice_stream(pb, 1);
    logger.report_step("AESND stream: start source and wait 120 frames");
    unsafe {
        snd::play_voice(
            pb,
            snd::VOICE_MONO16,
            INITIAL.0.as_ptr().cast(),
            INITIAL.0.len() as u32,
            48_000.0,
            0,
            i32::from(matches!(case, Case::Loop)) as snd::Bool,
        );
    }
    frames(120);
    logger.note(&OUTPUT_EVIDENCE.detail());
    logger.record_with_detail(
        "initial source produced expected DSP samples",
        INITIAL_OUTPUT.load(Ordering::Acquire) > 0,
        &alloc::format!(
            "case={label}; initial={} replacement={} running={} stream={}",
            INITIAL_OUTPUT.load(Ordering::Acquire),
            REPLACEMENT_OUTPUT.load(Ordering::Acquire),
            RUNNING.load(Ordering::Acquire),
            STREAM.load(Ordering::Acquire)
        ),
    );
    logger.record(
        "stream callback ran exactly once",
        STREAM.load(Ordering::Acquire) == 1,
    );
    logger.record(
        "callback buffer and context valid",
        INVALID.load(Ordering::Acquire) == 0,
    );
    let ready = INITIAL_OUTPUT.load(Ordering::Acquire) > 0
        && STREAM.load(Ordering::Acquire) == 1
        && INVALID.load(Ordering::Acquire) == 0;
    let running = RUNNING.load(Ordering::Acquire);
    let replacement = REPLACEMENT_OUTPUT.load(Ordering::Acquire);
    let silence = SILENCE.load(Ordering::Acquire);
    let audio = AUDIO.load(Ordering::Acquire);
    frames(12);
    logger.record(
        "AI DMA continued during observation",
        AUDIO.load(Ordering::Acquire) > audio,
    );
    if !ready {
        logger.note("UNTESTED stream outcome: initial DSP output or callback prerequisite failed");
    } else if matches!(case, Case::Once) {
        logger.record(
            "ONCE replacement does not play",
            REPLACEMENT_OUTPUT.load(Ordering::Acquire) == 0,
        );
        logger.record(
            "ONCE stops voice dispatch after callback",
            RUNNING.load(Ordering::Acquire) == running,
        );
        logger.record(
            "ONCE produces silence after stop",
            SILENCE.load(Ordering::Acquire) > silence,
        );
    } else {
        logger.record(
            "replacement source produced expected DSP samples",
            replacement >= 10,
        );
        logger.record(
            "replacement loop continues voice dispatch",
            RUNNING.load(Ordering::Acquire) > running,
        );
        logger.record(
            "replacement loop continues correct PCM",
            REPLACEMENT_OUTPUT.load(Ordering::Acquire) > replacement,
        );
    }
    logger.note(&alloc::format!(
        "initial={} replacement={} running={} stream={}",
        INITIAL_OUTPUT.load(Ordering::Acquire),
        REPLACEMENT_OUTPUT.load(Ordering::Acquire),
        RUNNING.load(Ordering::Acquire),
        STREAM.load(Ordering::Acquire)
    ));

    let active = if matches!(case, Case::Reallocate) {
        let replacement = REPLACEMENT_VOICE.load(Ordering::Acquire);
        logger.record(
            "callback reallocated the same voice slot",
            replacement == pb,
        );
        replacement
    } else {
        pb
    };
    logger.report_step("AESND stream: stop voice and check silence");
    snd::set_voice_stop(active, 1);
    frames(8); // Drain an already submitted DSP pass before the stop check.
    let stopped = RUNNING.load(Ordering::Acquire);
    let silence = SILENCE.load(Ordering::Acquire);
    frames(8);
    if ready {
        logger.record(
            "explicit stop prevents new voice callbacks",
            RUNNING.load(Ordering::Acquire) == stopped,
        );
        logger.record(
            "explicit stop produces silence",
            SILENCE.load(Ordering::Acquire) > silence,
        );
    } else {
        logger.note("UNTESTED explicit stop playback outcome: initial DSP output or callback prerequisite failed");
    }
    logger.report_step("AESND stream: free voice and reset DSP");
    snd::free_voice(active);
    snd::reset();
    let audio = AUDIO.load(Ordering::Acquire);
    let running = RUNNING.load(Ordering::Acquire);
    frames(4);
    let reset = ogc_audio::get_dma_enable_flag() == 0
        && AUDIO.load(Ordering::Acquire) == audio
        && RUNNING.load(Ordering::Acquire) == running;
    logger.record("reset disables DMA and all probe callbacks", reset);
    logger.report_step("AESND stream case complete");
    reset
}

pub fn run(logger: &mut ProbeLogger) {
    for (case, label) in [
        (Case::Once, "AESND STREAM REPLACEMENT RETAINS ONCE"),
        (Case::Loop, "AESND STREAM REPLACEMENT RETAINS LOOP"),
        (
            Case::RestartAsLoop,
            "AESND STREAM CALLBACK RESTARTS AS LOOP",
        ),
        (
            Case::Reallocate,
            "AESND STREAM CALLBACK REALLOCATES THE SAME SLOT",
        ),
    ] {
        if !run_case(logger, case, label) {
            logger.note("UNTESTED remaining AESND cases: allocation or reset failed");
            return;
        }
    }
}
