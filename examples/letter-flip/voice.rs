//! Production MP3 transport. All clips remain in static storage through completion.
use crate::narration::Narration;
pub use crate::speech::VoiceError;
use crate::speech::{Backend, Player};
use core::ffi::c_void;
use mp3player_sys as mp3player;
struct VoiceClip {
    name: &'static str,
    data: &'static [u8],
}
include!(concat!(env!("OUT_DIR"), "/letter_flip_voices.rs"));
struct Mp3;
impl Backend for Mp3 {
    fn start(&mut self, bytes: &'static [u8]) -> Result<(), VoiceError> {
        let length = i32::try_from(bytes.len()).map_err(|_| VoiceError::TooLarge)?;
        // SAFETY: the embedded immutable buffer has a static lifetime through final completion.
        let result =
            unsafe { mp3player::play_buffer(bytes.as_ptr().cast::<c_void>(), length, None) };
        if result == 0 {
            Ok(())
        } else {
            Err(VoiceError::Backend(result))
        }
    }
    fn stop(&mut self) -> Result<(), VoiceError> {
        mp3player::stop();
        if mp3player::is_playing() {
            Err(VoiceError::StopFailed)
        } else {
            Ok(())
        }
    }
    fn playing(&self) -> bool {
        mp3player::is_playing()
    }
    fn final_error(&self) -> Option<i32> {
        let code = mp3player::last_error();
        (code != 0).then_some(code)
    }
    fn volume(&mut self, percent: u8) {
        mp3player::volume(u32::from(percent) * 255 / 100);
    }
}
pub struct VoicePlayer {
    player: Player<Mp3>,
}
impl VoicePlayer {
    pub fn new() -> Self {
        mp3player::init();
        Self {
            player: Player::new(Mp3),
        }
    }
    pub fn cancel(&mut self) -> Result<(), VoiceError> {
        self.player.cancel()
    }
    pub fn set_preferences(&mut self, enabled: bool, volume: u8) -> Result<(), VoiceError> {
        self.player.set_preferences(enabled, volume)
    }
    pub fn queue(&mut self, symbol: u8, now: u64) -> Result<(), VoiceError> {
        let clip = crate::font::voice_asset(symbol)
            .and_then(|(_, name)| VOICE_CLIPS.iter().find(|clip| clip.name == name))
            .map(|clip| clip.data);
        self.player.queue(clip, now)
    }
    #[cfg(feature = "letter-flip-hardware-probe")]
    pub fn completed(&self) -> bool {
        self.player.progress == crate::speech::Progress::Complete
    }
    pub fn busy(&self) -> bool {
        self.player.busy()
    }
    pub fn narrate(&mut self, narration: Narration, now: u64) -> Result<(), VoiceError> {
        if !self.player.enabled() {
            return self.player.queue_sequence([None; 3], now);
        }
        let mut clips = [None; 3];
        for (i, name) in narration.names.into_iter().enumerate() {
            if let Some(name) = name {
                let Some(clip) = VOICE_CLIPS.iter().find(|clip| clip.name == name) else {
                    self.player.cancel()?;
                    return Err(VoiceError::Missing);
                };
                clips[i] = Some(clip.data);
            }
        }
        self.player.queue_sequence(clips, now)
    }
    pub fn tick(&mut self, now: u64) -> Result<(), VoiceError> {
        self.player.tick(now)
    }
}
