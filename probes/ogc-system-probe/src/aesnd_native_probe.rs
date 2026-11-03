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
static REPLACED_BUFFERS: AtomicU32 = AtomicU32::new(0);
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
    if crate::aesnd_output_evidence::matches_pcm(
        bytes,
        &crate::aesnd_output_evidence::REPLACEMENT_FRAME,
    ) {
        REPLACED_BUFFERS.fetch_add(1, Ordering::Relaxed);
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
    probe_wrapper_lifecycle(logger);
    probe_owned_playback(logger);
}

fn probe_owned_playback(logger: &mut ProbeLogger) {
    logger.section("AESND OWNED PLAYBACK");
    MATCHED_BUFFERS.store(0, Ordering::Release);
    REPLACED_BUFFERS.store(0, Ordering::Release);
    aesnd::init();
    aesnd::register_audio_callback(Some(wrapper_audio));
    let Some(voice) = aesnd::allocate_voice(None) else {
        logger.record("owned playback voice allocation", false);
        aesnd::reset();
        return;
    };
    let raw = voice.as_ptr();
    let mut negative = INPUT.0;
    for byte in negative.iter_mut().step_by(2) {
        *byte = 0xf0;
    }
    let Ok(initial) = aesnd::OwnedAudioBuffer::copy_from_slice(&INPUT.0) else {
        logger.record("owned playback initial buffer", false);
        voice.free();
        aesnd::reset();
        return;
    };
    let Ok(replacement) = aesnd::OwnedAudioBuffer::copy_from_slice(&negative) else {
        logger.record("owned playback replacement buffer", false);
        voice.free();
        aesnd::reset();
        return;
    };
    voice.set_volume(0x80, 0x40);
    let mut playback = voice.play_owned(snd::VOICE_MONO16, initial, 48_000.0, 0, true);
    frames(40);
    let initial_ok = MATCHED_BUFFERS.load(Ordering::Acquire) >= 10
        && playback.voice().as_ptr() == raw
        && playback.buffer().as_bytes() == INPUT.0;
    let old = playback.replace_buffer(replacement);
    frames(40);
    let replacement_ok = old.as_bytes() == INPUT.0
        && playback.buffer().as_bytes() == negative
        && REPLACED_BUFFERS.load(Ordering::Acquire) >= 10;
    let (voice, final_buffer) = playback.into_parts();
    logger.record(
        "owned playback buffer replacement and recovery",
        initial_ok
            && replacement_ok
            && voice.as_ptr() == raw
            && final_buffer.as_bytes() == negative,
    );
    voice.free();
    aesnd::reset();
    logger.record(
        "owned playback reset stops DMA",
        ogc_audio::get_dma_enable_flag() == 0,
    );
}

unsafe extern "C" fn wrapper_audio(ptr: *mut c_void, len: u32) {
    unsafe { audio(ptr, len, core::ptr::null_mut()) };
}

unsafe extern "C" fn wrapper_voice(_pb: *mut snd::Aesndpb, event: u32) {
    if event == snd::VOICE_STATE_RUNNING {
        RUNNING_CALLS.fetch_add(1, Ordering::Relaxed);
    }
}

fn probe_wrapper_lifecycle(logger: &mut ProbeLogger) {
    logger.section("AESND WRAPPER LIFECYCLE");
    MATCHED_BUFFERS.store(0, Ordering::Release);
    RUNNING_CALLS.store(0, Ordering::Release);
    AUDIO_CALLS.store(0, Ordering::Release);
    INVALID_BUFFER.store(0, Ordering::Release);
    aesnd::init();
    let previous_audio = aesnd::register_audio_callback(Some(wrapper_audio));
    let Some(handle) = aesnd::allocate_voice(None) else {
        logger.record("wrapper voice allocation", false);
        aesnd::reset();
        return;
    };
    let raw = handle.into_raw();
    // SAFETY: The allocated voice remains owned by this probe.
    let handle = unsafe { aesnd::VoiceHandle::from_raw(raw) };
    let first = handle.register_voice_callback(Some(wrapper_voice));
    let second = aesnd::register_voice_callback(&handle, Some(wrapper_voice));
    let backend_voice = snd::register_voice_callback(raw, Some(wrapper_voice));
    handle.set_volume(0x80, 0x40);
    // SAFETY: INPUT is aligned static storage and remains live through reset.
    unsafe {
        handle.play_voice(
            snd::VOICE_MONO16,
            INPUT.0.as_ptr().cast(),
            INPUT.0.len() as u32,
            48_000.0,
            0,
            true,
        )
    };
    frames(60);
    logger.record(
        "wrapper callbacks and DSP samples",
        previous_audio.is_none()
            && first.is_none()
            && second.is_some()
            && backend_voice.is_some()
            && MATCHED_BUFFERS.load(Ordering::Acquire) >= 10
            && RUNNING_CALLS.load(Ordering::Acquire) >= 10
            && INVALID_BUFFER.load(Ordering::Acquire) == 0,
    );
    aesnd::pause(true);
    frames(4);
    let time = aesnd::get_dsp_process_time();
    logger.record(
        "wrapper DSP usage",
        time > 0 && (aesnd::get_dsp_process_usage() - time as f32 / 20.0).abs() < 0.001,
    );
    let backend_audio = snd::register_audio_callback(Some(wrapper_audio));
    aesnd::pause(false);
    frames(6);
    handle.set_stop(true);
    frames(6);
    let stopped = RUNNING_CALLS.load(Ordering::Acquire);
    let audio_before = AUDIO_CALLS.load(Ordering::Acquire);
    frames(6);
    logger.record(
        "wrapper stop ends voice dispatch",
        backend_audio.is_some()
            && stopped >= 10
            && RUNNING_CALLS.load(Ordering::Acquire) == stopped
            && AUDIO_CALLS.load(Ordering::Acquire) > audio_before,
    );
    handle.free();
    let replacement = aesnd::allocate_voice(None);
    logger.record(
        "wrapper free returns voice slot",
        replacement
            .as_ref()
            .is_some_and(|voice| voice.as_ptr() == raw),
    );
    if let Some(voice) = replacement {
        aesnd::free_voice(voice);
    }
    let third = aesnd::allocate_voice(None);
    logger.record(
        "wrapper free function returns voice slot",
        third.as_ref().is_some_and(|voice| voice.as_ptr() == raw),
    );
    if let Some(voice) = third {
        voice.free();
    }
    let direct = snd::allocate_voice(None);
    logger.record("backend free slot reused", direct == raw);
    snd::free_voice(direct);
    aesnd::reset();
    logger.record(
        "wrapper reset stops DMA",
        ogc_audio::get_dma_enable_flag() == 0,
    );
}
