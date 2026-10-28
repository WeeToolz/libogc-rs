//! Shared ASND submission with one permanent aligned sample buffer per voice.
//! A player must remain at its static address while ASND uses its buffers.
use asnd_sys as asnd;
use core::cell::UnsafeCell;
use core::fmt;

#[repr(C, align(32))]
struct Buffer<const N: usize>(UnsafeCell<[u8; N]>);
#[derive(Debug)]
pub enum PlaybackError {
    InvalidSamples,
    InvalidVolume,
    NoVoice(i32),
    Backend(i32),
}
impl fmt::Display for PlaybackError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSamples => formatter.write_str("sample count is outside the buffer"),
            Self::InvalidVolume => formatter.write_str("volume is outside the ASND range"),
            Self::NoVoice(status) => write!(formatter, "no free ASND voice (status {status})"),
            Self::Backend(status) => write!(formatter, "ASND rejected the voice (status {status})"),
        }
    }
}

pub struct Player<const N: usize> {
    buffers: [Buffer<N>; asnd::MAX_SND_VOICES],
}
// SAFETY: Buffer access occurs only in the unsafe single-submitter `play` method.
unsafe impl<const N: usize> Sync for Player<N> {}

impl<const N: usize> Player<N> {
    pub const fn new() -> Self {
        Self {
            buffers: [const { Buffer(UnsafeCell::new([128; N])) }; asnd::MAX_SND_VOICES],
        }
    }
    pub fn init() {
        asnd::init();
        asnd::pause(0);
    }
    /// SAFETY: `self` must have a permanent address and one submitting thread.
    /// Buffer mutation is permitted only after ASND releases its voice.
    pub unsafe fn play(
        &self,
        count: usize,
        volume: i32,
        fill: impl FnOnce(&mut [u8]),
    ) -> Result<i32, PlaybackError> {
        if count == 0 || count > N || count > i32::MAX as usize {
            return Err(PlaybackError::InvalidSamples);
        }
        if !(0..=255).contains(&volume) {
            return Err(PlaybackError::InvalidVolume);
        }
        let voice = asnd::get_first_unused_voice();
        if voice < 0 || voice as usize >= asnd::MAX_SND_VOICES {
            return Err(PlaybackError::NoVoice(voice));
        }
        let buffer = unsafe { core::ptr::addr_of!((*self.buffers.as_ptr().add(voice as usize)).0) };
        let samples = UnsafeCell::raw_get(buffer).cast::<u8>();
        fill(unsafe { core::slice::from_raw_parts_mut(samples, count) });
        let result = unsafe {
            asnd::set_voice(asnd::VoiceConfig {
                voice,
                format: asnd::VOICE_MONO_8BIT_U,
                pitch: asnd::F44100HZ_PITCH,
                delay: 0,
                snd: samples.cast(),
                size_snd: count as i32,
                volume_l: volume,
                volume_r: volume,
                callback: None,
            })
        };
        if result == asnd::SND_OK {
            Ok(voice)
        } else {
            Err(PlaybackError::Backend(result))
        }
    }
}
