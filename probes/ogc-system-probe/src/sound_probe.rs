use core::sync::atomic::{AtomicU32, Ordering};

use asnd_sys as snd;
use ogc_audio as audio;

use crate::probe_support::ProbeLogger;

static GLOBAL_CALLBACK_COUNT: AtomicU32 = AtomicU32::new(0);
static VOICE_CALLBACK_COUNT: AtomicU32 = AtomicU32::new(0);

pub fn run(logger: &mut ProbeLogger) {
    logger.section("SOUND PROBE");

    let mut stack = [0u8; 1024];
    audio::init(Some(&mut stack));
    let snap = audio::snapshot();
    logger.record("audio initialized", snap.initialized);
    logger.record(
        "audio callback stack tracked",
        snap.callback_stack == stack.as_mut_ptr(),
    );

    audio::set_stream_vol_left(19);
    audio::set_stream_vol_right(23);
    audio::set_stream_sample_rate(audio::AI_SAMPLERATE_48KHZ);
    audio::set_stream_trigger(7);
    audio::set_stream_play_state(audio::AI_STREAM_START);
    audio::set_dsp_sample_rate(1);
    audio::init_dma(0x8123_4000, 0x1000);
    audio::start_dma();

    logger.record(
        "audio register roundtrips",
        audio::get_stream_vol_left() == 19
            && audio::get_stream_vol_right() == 23
            && audio::get_stream_sample_rate() == audio::AI_SAMPLERATE_48KHZ
            && audio::get_stream_play_state() == audio::AI_STREAM_START
            && audio::get_dsp_sample_rate() == 1
            && audio::get_dma_start_addr() == 0x8123_4000
            && audio::get_dma_length() == 0x1000
            && audio::get_dma_enable_flag() != 0,
    );
    logger.record(
        "audio dma bytes left in range",
        audio::get_dma_bytes_left() <= audio::get_dma_length(),
    );

    audio::stop_dma();
    logger.record("audio dma stop", audio::get_dma_enable_flag() == 0);

    let before = audio::snapshot();
    let prev_stream = audio::register_stream_callback(Some(stream_callback));
    let prev_dma = audio::register_dma_callback(Some(dma_callback));
    let after = audio::snapshot();
    logger.record(
        "stream callback registered",
        after.stream_callback_registered,
    );
    logger.record("dma callback registered", after.dma_callback_registered);
    logger.record("stream callback previous preserved", prev_stream.is_none());
    logger.record("dma callback previous preserved", prev_dma.is_none());
    logger.record(
        "audio snapshot stable",
        before.initialized && after.initialized,
    );

    audio::reset_stream_sample_cnt();
    logger.record(
        "stream sample counter reset",
        audio::snapshot().stream_sample_count == 0,
    );
    audio::set_stream_play_state(audio::AI_STREAM_STOP);
    logger.record(
        "audio timing probe",
        audio::snapshot().stream_sample_count == 0
            && audio::snapshot().stream_play_state == audio::AI_STREAM_STOP,
    );

    logger.section("ASND PROBE");
    GLOBAL_CALLBACK_COUNT.store(0, Ordering::Release);
    VOICE_CALLBACK_COUNT.store(0, Ordering::Release);
    snd::init();
    logger.record(
        "asnd initialized",
        snd::is_paused() == 1
            && snd::get_audio_rate() == 48_000
            && snd::get_samples_per_tick() == 1024,
    );

    snd::set_callback(Some(global_callback));
    let voice0 = snd::get_first_unused_voice();
    let voice1 = voice0 + 1;
    let mut right_bytes = [0u8; 64];
    for chunk in right_bytes.as_chunks_mut::<4>().0 {
        *chunk = [0xff, 0x7f, 0xff, 0x7f];
    }

    let mut mono = snd::OwnedVoiceBuffer::copy_from_slice(&[0xff; 64])
        .ok()
        .and_then(|buffer| {
            snd::set_voice_owned(
                snd::OwnedVoiceConfig {
                    voice: voice0,
                    format: snd::VOICE_MONO_8BIT_U,
                    pitch: 48_000,
                    delay: 0,
                    volume_l: 255,
                    volume_r: 0,
                    callback: Some(voice_callback),
                },
                buffer,
            )
            .ok()
        });
    let stereo = snd::OwnedVoiceBuffer::copy_from_slice(&right_bytes)
        .ok()
        .and_then(|buffer| {
            snd::set_infinite_voice_owned(
                snd::OwnedInfiniteVoiceConfig {
                    voice: voice1,
                    format: snd::VOICE_STEREO_16BIT_LE,
                    pitch: 48_000,
                    delay: 0,
                    volume_l: 0,
                    volume_r: 255,
                },
                buffer,
            )
            .ok()
        });
    logger.record(
        "asnd voice setup",
        mono.is_some() && stereo.is_some() && snd::test_voice_buffer_ready(voice0) == 1,
    );

    let mut second = snd::OwnedVoiceBuffer::copy_from_slice(&[0x7f; 64]).ok();
    let add_rejected = match (mono.as_mut(), second.take()) {
        (Some(voice), Some(buffer)) => match voice.add_buffer(buffer) {
            Ok(()) => false,
            Err(error) => {
                let rejected = error.status() == snd::SND_INVALID;
                second = Some(error.into_buffer());
                rejected
            }
        },
        _ => false,
    };
    logger.record("asnd add rejected while busy", add_rejected);

    snd::pause(0);
    let mut progressed = false;
    for _ in 0..250_000 {
        if snd::get_sample_counter() > 0
            && GLOBAL_CALLBACK_COUNT.load(Ordering::Acquire) > 0
            && VOICE_CALLBACK_COUNT.load(Ordering::Acquire) > 0
        {
            progressed = true;
            break;
        }
        core::hint::spin_loop();
    }
    let add_ok = match (mono.as_mut(), second.take()) {
        (Some(voice), Some(buffer)) => match voice.add_buffer(buffer) {
            Ok(()) => true,
            Err(_error) => false,
        },
        _ => false,
    };
    logger.record(
        "asnd double buffer queued",
        add_ok && snd::test_voice_buffer_ready(voice0) == 0,
    );
    logger.record("asnd playback progressed", progressed);
    logger.record(
        "asnd voices active",
        snd::status_voice(voice0) == snd::SND_WORKING
            && snd::status_voice(voice1) == snd::SND_WORKING,
    );
    logger.record(
        "asnd pause/resume",
        snd::pause_voice(voice0, 1) == snd::SND_OK
            && snd::status_voice(voice0) == snd::SND_WAITING
            && snd::pause_voice(voice0, 0) == snd::SND_OK
            && snd::status_voice(voice0) == snd::SND_WORKING,
    );
    logger.record(
        "asnd metrics recorded",
        snd::get_tick_counter_voice(voice0) > 0
            && snd::get_tick_counter_voice(voice1) > 0
            && snd::get_dsp_process_time() < 10_000_000,
    );

    drop(mono);
    drop(stereo);
    snd::end();
    logger.record(
        "asnd cleanup and reinit",
        snd::status_voice(voice0) == snd::SND_UNUSED
            && snd::status_voice(voice1) == snd::SND_UNUSED
            && {
                snd::init();
                let ok = snd::is_paused() == 1;
                snd::end();
                ok
            },
    );
}

unsafe extern "C" fn stream_callback(_smp_cnt: u32) {}

unsafe extern "C" fn dma_callback() {}

unsafe extern "C" fn global_callback() {
    GLOBAL_CALLBACK_COUNT.fetch_add(1, Ordering::AcqRel);
}

fn voice_callback(_voice: i32) {
    VOICE_CALLBACK_COUNT.fetch_add(1, Ordering::AcqRel);
}
