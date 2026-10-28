//! Short, soft effects on voice 1. MP3 speech owns voice 0.
use crate::tone::{self, SAMPLE_COUNT};
use asnd_sys as asnd;
use core::cell::UnsafeCell;
const EFFECT_VOICE: i32 = 1;
#[repr(C, align(32))]
struct Buffer(UnsafeCell<[u8; SAMPLE_COUNT]>);
// SAFETY: the game main thread is the only submitter; stop completes before buffer changes.
unsafe impl Sync for Buffer {}
static BUFFER: Buffer = Buffer(UnsafeCell::new([128; SAMPLE_COUNT]));
#[derive(Debug)]
pub struct SoundError(pub i32);
pub fn stop() -> Result<(), SoundError> {
    let code = asnd::stop_voice(EFFECT_VOICE);
    if code == asnd::SND_OK {
        Ok(())
    } else {
        Err(SoundError(code))
    }
}
pub fn play(correct: bool, enabled: bool, volume: u8) -> Result<(), SoundError> {
    if !enabled || volume == 0 {
        return Ok(());
    }
    stop()?;
    // SAFETY: this permanent aligned buffer is unique to voice 1. stop released the previous playback.
    let samples = unsafe { &mut *BUFFER.0.get() };
    tone::fill(samples, correct);
    let level = i32::from(volume.min(100)) * 180 / 100;
    // SAFETY: the buffer is aligned, static, initialized, and unchanged until stop or completion.
    let code = unsafe {
        asnd::set_voice(asnd::VoiceConfig {
            voice: EFFECT_VOICE,
            format: asnd::VOICE_MONO_8BIT_U,
            pitch: asnd::F44100HZ_PITCH,
            delay: 0,
            snd: samples.as_mut_ptr().cast(),
            size_snd: SAMPLE_COUNT as i32,
            volume_l: level,
            volume_r: level,
            callback: None,
        })
    };
    if code == asnd::SND_OK {
        Ok(())
    } else {
        Err(SoundError(code))
    }
}
