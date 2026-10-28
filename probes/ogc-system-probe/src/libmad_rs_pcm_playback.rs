//! Finite PCM playback helper for the pure Rust libmad decode probe.
//!
//! Include this file from a probe binary with `mod libmad_rs_pcm_playback;`.
//! The caller must initialize the console video/diagnostic display before
//! calling this function. A successful result proves that ASND accepted the
//! buffer, advanced its DMA callback, and completed the voice. It cannot prove
//! that the connected display/audio setup made sound audible to a person.

extern crate alloc;

use alloc::vec::Vec;
use asnd_sys::{
    self, MAX_VOLUME, OwnedVoice, OwnedVoiceBuffer, OwnedVoiceConfig, SND_OK, SND_UNUSED,
    VOICE_STEREO_16BIT_LE,
};
use core::sync::atomic::{AtomicU32, Ordering};

static DMA_CALLBACKS: AtomicU32 = AtomicU32::new(0);
static VOICE_CALLBACKS: AtomicU32 = AtomicU32::new(0);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PlaybackResult {
    /// ASND's return code when it accepted or rejected the voice.
    pub submit_status: i32,
    /// Number of audio DMA callbacks observed during this call.
    pub dma_callbacks: u32,
    /// Number of finite-voice completion callbacks observed during this call.
    pub voice_callbacks: u32,
    /// Final ASND voice status. `SND_UNUSED` means playback completed.
    pub final_voice_status: i32,
    /// Number of stereo sample frames submitted.
    pub sample_frames: usize,
    /// True when the timeout expired before the completion callback.
    pub timed_out: bool,
}

impl PlaybackResult {
    /// Reports backend completion. This is not a human audibility check.
    pub const fn passed(self) -> bool {
        self.submit_status == SND_OK
            && self.voice_callbacks > 0
            && self.dma_callbacks > 0
            && self.final_voice_status == SND_UNUSED
            && !self.timed_out
    }
}

unsafe extern "C" fn on_dma() {
    DMA_CALLBACKS.fetch_add(1, Ordering::Relaxed);
}

fn on_voice(_voice: i32) {
    VOICE_CALLBACKS.fetch_add(1, Ordering::Release);
}

/// Play owned, interleaved stereo signed 16-bit PCM through ASND.
///
/// `samples` must contain complete left/right pairs. `sample_rate` is the
/// source rate in hertz and is also used as the ASND pitch. `timeout_ticks`
/// uses the Wii LWP timer tick unit. The caller should pass a finite, practical
/// deadline (for example, a few seconds beyond the expected clip duration).
///
/// The function initializes and then ends ASND. It therefore owns the ASND
/// lifecycle for this probe call and should not run while another component
/// uses ASND.
pub fn play_pcm_stereo_i16(
    mut samples: Vec<i16>,
    sample_rate: u32,
    timeout_ticks: u64,
) -> PlaybackResult {
    let frames = samples.len() / 2;
    if samples.is_empty() || samples.len() % 2 != 0 || sample_rate == 0 {
        return PlaybackResult {
            submit_status: asnd_sys::SND_INVALID,
            dma_callbacks: 0,
            voice_callbacks: 0,
            final_voice_status: asnd_sys::SND_INVALID,
            sample_frames: frames,
            timed_out: false,
        };
    }

    // ASND's LE format is explicit so the byte order does not depend on the
    // PowerPC CPU's native endianness. Convert the owned input in place, then
    // copy once into aligned ASND storage. This avoids three simultaneous clip
    // buffers when the decode probe hands off a large PCM vector.
    for sample in &mut samples {
        *sample = sample.to_le();
    }
    // SAFETY: `samples` is initialized `i16` storage. After `to_le`, its byte
    // representation is exactly the little-endian sample stream ASND expects.
    let little_endian_bytes =
        unsafe { core::slice::from_raw_parts(samples.as_ptr().cast::<u8>(), samples.len() * 2) };
    let buffer = match OwnedVoiceBuffer::copy_from_slice(little_endian_bytes) {
        Ok(buffer) => buffer,
        Err(_) => {
            return PlaybackResult {
                submit_status: asnd_sys::SND_INVALID,
                dma_callbacks: 0,
                voice_callbacks: 0,
                final_voice_status: asnd_sys::SND_INVALID,
                sample_frames: frames,
                timed_out: false,
            };
        }
    };
    // Release the input before ASND playback; the owned, aligned buffer now
    // keeps the clip alive until completion.
    drop(samples);

    DMA_CALLBACKS.store(0, Ordering::Release);
    VOICE_CALLBACKS.store(0, Ordering::Release);
    asnd_sys::init();
    asnd_sys::set_callback(Some(on_dma));
    let voice = asnd_sys::get_first_unused_voice();
    let submitted: Result<OwnedVoice, asnd_sys::OwnedVoiceError> = asnd_sys::set_voice_owned(
        OwnedVoiceConfig {
            voice,
            format: VOICE_STEREO_16BIT_LE,
            pitch: sample_rate.min(i32::MAX as u32) as i32,
            delay: 0,
            volume_l: MAX_VOLUME,
            volume_r: MAX_VOLUME,
            callback: Some(on_voice),
        },
        buffer,
    );

    let owned_voice = match submitted {
        Ok(voice) => voice,
        Err(error) => {
            asnd_sys::set_callback(None);
            asnd_sys::end();
            return PlaybackResult {
                submit_status: error.status(),
                dma_callbacks: DMA_CALLBACKS.load(Ordering::Acquire),
                voice_callbacks: VOICE_CALLBACKS.load(Ordering::Acquire),
                final_voice_status: asnd_sys::SND_INVALID,
                sample_frames: frames,
                timed_out: false,
            };
        }
    };

    asnd_sys::pause(0);
    let started = ogc_lwp::sys::now_ticks();
    let mut timed_out = false;
    while VOICE_CALLBACKS.load(Ordering::Acquire) == 0 {
        if ogc_lwp::sys::now_ticks().wrapping_sub(started) >= timeout_ticks {
            timed_out = true;
            break;
        }
        ogc_lwp::yield_now();
    }

    asnd_sys::pause(1);
    asnd_sys::set_callback(None);
    // End DMA before dropping the owner, so ASND cannot read a released buffer.
    asnd_sys::end();
    owned_voice.stop();
    // A finite voice with a callback remains active while it waits for an
    // optional next buffer. Its callback reports completion of this buffer;
    // stopping the owned voice releases it and makes the final status unused.
    let final_voice_status = asnd_sys::status_voice(voice);

    PlaybackResult {
        submit_status: SND_OK,
        dma_callbacks: DMA_CALLBACKS.load(Ordering::Acquire),
        voice_callbacks: VOICE_CALLBACKS.load(Ordering::Acquire),
        final_voice_status,
        sample_frames: frames,
        timed_out,
    }
}
