//! Native AESND probe: validate DSP-produced RAM samples and callback lifecycle.
use crate::probe_support::{self, ProbeLogger};
use aesnd_sys as snd;
use core::ffi::c_void;
use core::sync::atomic::{AtomicU32, Ordering};

#[repr(align(32))]
struct Input([u8; 4608]);
const fn input() -> [u8; 4608] {
    let mut data = [0; 4608];
    let mut index = 0;
    while index < data.len() {
        data[index] = 0x10;
        index += 2;
    }
    data
}
static INPUT: Input = Input(input());
static AUDIO_CALLS: AtomicU32 = AtomicU32::new(0);
static MATCHED_BUFFERS: AtomicU32 = AtomicU32::new(0);
static RUNNING_CALLS: AtomicU32 = AtomicU32::new(0);
static INVALID_BUFFER: AtomicU32 = AtomicU32::new(0);
static OUTPUT_EVIDENCE: crate::aesnd_output_evidence::OutputEvidence =
    crate::aesnd_output_evidence::OutputEvidence::new();

unsafe extern "C" fn audio(ptr: *mut c_void, len: u32, _arg: *mut c_void) {
    AUDIO_CALLS.fetch_add(1, Ordering::Relaxed);
    if ptr.is_null() || len != 384 {
        INVALID_BUFFER.fetch_add(1, Ordering::Relaxed);
        return;
    }
    // The callback receives the actual AI/DSP output buffer. No mixer model or
    // replacement hardware path supplies these bytes.
    let bytes = unsafe { core::slice::from_raw_parts(ptr.cast::<u8>(), len as usize) };
    OUTPUT_EVIDENCE.observe(bytes);
    if crate::aesnd_output_evidence::matches_pcm(
        bytes,
        &crate::aesnd_output_evidence::INITIAL_FRAME,
    ) {
        MATCHED_BUFFERS.fetch_add(1, Ordering::Relaxed);
    }
}
unsafe extern "C" fn voice(pb: *mut snd::Aesndpb, event: u32, _arg: *mut c_void) {
    if event == snd::VOICE_STATE_RUNNING {
        RUNNING_CALLS.fetch_add(1, Ordering::Relaxed);
        // C callbacks receive the command pointer. These setters must accept it.
        snd::set_voice_volume(pb, 0x80, 0x40);
    }
}

fn frames(count: u32) {
    for _ in 0..count {
        probe_support::wait_vsync();
    }
}

pub fn run(logger: &mut ProbeLogger) {
    logger.section("NATIVE AESND DSP PROBE");
    for pass in 0..2 {
        OUTPUT_EVIDENCE.reset();
        AUDIO_CALLS.store(0, Ordering::Release);
        MATCHED_BUFFERS.store(0, Ordering::Release);
        RUNNING_CALLS.store(0, Ordering::Release);
        INVALID_BUFFER.store(0, Ordering::Release);
        logger.report_step(&alloc::format!("AESND native pass {pass}: initialize DSP"));
        snd::init();
        let previous =
            unsafe { snd::register_audio_callback_with_arg(Some(audio), core::ptr::null_mut()) };
        logger.record("fresh native AESND audio callback slot", previous.is_none());
        let pb = unsafe { snd::allocate_voice_with_arg(Some(voice), core::ptr::null_mut()) };
        logger.record("voice allocation", !pb.is_null());
        if pb.is_null() {
            snd::reset();
            return;
        }
        logger.report_step("AESND native: start loop playback and wait 60 frames");
        unsafe {
            snd::play_voice(
                pb,
                snd::VOICE_MONO16,
                INPUT.0.as_ptr().cast(),
                INPUT.0.len() as u32,
                snd::VOICE_FREQ48KHZ as f32,
                0,
                1,
            );
        }
        frames(60);
        let matched = MATCHED_BUFFERS.load(Ordering::Acquire);
        let running = RUNNING_CALLS.load(Ordering::Acquire);
        logger.note(&OUTPUT_EVIDENCE.detail());
        logger.record_with_detail(
            "DSP RAM samples match left 2048 and right 1024",
            matched >= 10,
            &alloc::format!("pass={pass} matched_buffers={matched}; required>=10"),
        );
        logger.record_with_detail(
            "running and audio callbacks complete",
            running >= 10 && AUDIO_CALLS.load(Ordering::Acquire) >= 10,
            &alloc::format!(
                "running={running} audio={}; each required>=10",
                AUDIO_CALLS.load(Ordering::Acquire)
            ),
        );
        logger.record_with_detail(
            "callback buffer contract",
            INVALID_BUFFER.load(Ordering::Acquire) == 0,
            &alloc::format!(
                "invalid_buffers={}; required=0",
                INVALID_BUFFER.load(Ordering::Acquire)
            ),
        );
        logger.report_step("AESND native: pause and check callback completion");
        snd::pause(1);
        frames(4); // Allow the already-dispatched DSP pass to complete.
        let time = snd::get_dsp_process_time();
        let before = RUNNING_CALLS.load(Ordering::Acquire);
        frames(4);
        logger.record(
            "pause stops new voice dispatch",
            RUNNING_CALLS.load(Ordering::Acquire) == before,
        );
        logger.record(
            "pause preserves completed DSP metric",
            time > 0 && snd::get_dsp_process_time() == time,
        );
        logger.record(
            "DSP usage reports the completed interval",
            (snd::get_dsp_process_usage() - time as f32 / 20.0).abs() < 0.001,
        );
        logger.report_step("AESND native: stop, free voice, and reset DSP");
        snd::set_voice_stop(pb, 1);
        snd::free_voice(pb);
        snd::reset();
        let callbacks = AUDIO_CALLS.load(Ordering::Acquire);
        frames(4);
        let reset_complete = AUDIO_CALLS.load(Ordering::Acquire) == callbacks
            && ogc_audio::get_dma_enable_flag() == 0;
        logger.record("reset stops DMA callbacks", reset_complete);
        if !reset_complete {
            logger.note("UNTESTED remaining AESND cases: reset did not stop DMA callbacks");
            return;
        }
        logger.note(&alloc::format!(
            "pass {pass}: buffers={matched} running={running} DSP_us={time}"
        ));
        logger.report_step("AESND native pass complete");
    }
    crate::aesnd_stream_probe::run(logger);
}
