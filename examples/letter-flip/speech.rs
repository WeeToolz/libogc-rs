//! Speech state machine shared by production MP3 playback and failure-injection tests.
use crate::session::{TICKS_PER_SECOND, millis_ticks};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VoiceError {
    Missing,
    Backend(i32),
    NotStarted,
    TooLarge,
    StopFailed,
    Timeout,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Progress {
    Idle,
    Starting,
    Playing,
    Complete,
}
pub trait Backend {
    fn start(&mut self, bytes: &'static [u8]) -> Result<(), VoiceError>;
    fn stop(&mut self) -> Result<(), VoiceError>;
    fn playing(&self) -> bool;
    fn final_error(&self) -> Option<i32>;
    fn volume(&mut self, percent: u8);
}
pub struct Player<B: Backend> {
    backend: B,
    pub progress: Progress,
    observed: bool,
    started: u64,
    enabled: bool,
    volume: u8,
    pending: [Option<&'static [u8]>; 2],
}
impl<B: Backend> Player<B> {
    pub fn new(backend: B) -> Self {
        Self {
            backend,
            progress: Progress::Idle,
            observed: false,
            started: 0,
            enabled: true,
            volume: 50,
            pending: [None; 2],
        }
    }
    pub fn cancel(&mut self) -> Result<(), VoiceError> {
        if matches!(self.progress, Progress::Starting | Progress::Playing) || self.backend.playing()
        {
            self.backend.stop()?;
        }
        self.progress = Progress::Idle;
        self.pending = [None; 2];
        self.observed = false;
        Ok(())
    }
    pub fn set_preferences(&mut self, enabled: bool, volume: u8) -> Result<(), VoiceError> {
        self.enabled = enabled;
        self.volume = volume.min(100);
        self.backend.volume(self.volume);
        if !enabled || self.volume == 0 {
            self.cancel()?;
        }
        Ok(())
    }
    pub fn enabled(&self) -> bool {
        self.enabled && self.volume > 0
    }
    pub fn busy(&self) -> bool {
        matches!(self.progress, Progress::Starting | Progress::Playing)
    }
    pub fn queue(&mut self, bytes: Option<&'static [u8]>, now: u64) -> Result<(), VoiceError> {
        self.queue_sequence([bytes, None, None], now)
    }
    pub fn queue_sequence(
        &mut self,
        bytes: [Option<&'static [u8]>; 3],
        now: u64,
    ) -> Result<(), VoiceError> {
        self.cancel()?;
        if !self.enabled || self.volume == 0 {
            return Ok(());
        }
        if bytes.iter().flatten().any(|b| b.is_empty()) {
            return Err(VoiceError::Missing);
        }
        let first = bytes[0].ok_or(VoiceError::Missing)?;
        if bytes[1].is_none() && bytes[2].is_some() {
            return Err(VoiceError::Missing);
        }
        self.begin(first, now)?;
        self.pending = [bytes[1], bytes[2]];
        Ok(())
    }
    fn begin(&mut self, bytes: &'static [u8], now: u64) -> Result<(), VoiceError> {
        if bytes.len() > i32::MAX as usize {
            return Err(VoiceError::TooLarge);
        }
        self.backend.start(bytes)?;
        self.progress = Progress::Starting;
        self.observed = false;
        self.started = now;
        Ok(())
    }
    pub fn tick(&mut self, now: u64) -> Result<(), VoiceError> {
        if !matches!(self.progress, Progress::Starting | Progress::Playing) {
            return Ok(());
        }
        if now.saturating_sub(self.started) > 30 * TICKS_PER_SECOND {
            self.cancel()?;
            return Err(VoiceError::Timeout);
        }
        if self.backend.playing() {
            self.observed = true;
            self.progress = Progress::Playing;
            return Ok(());
        }
        if now.saturating_sub(self.started) < millis_ticks(500) {
            return Ok(());
        }
        self.progress = Progress::Idle;
        if let Some(error) = self.backend.final_error() {
            self.pending = [None; 2];
            return Err(VoiceError::Backend(error));
        }
        if !self.observed {
            self.pending = [None; 2];
            return Err(VoiceError::NotStarted);
        }
        if let Some(next) = self.pending[0] {
            self.pending = [self.pending[1], None];
            if let Err(error) = self.begin(next, now) {
                self.pending = [None; 2];
                return Err(error);
            }
        } else {
            self.progress = Progress::Complete;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    // Explicit controlled transport failures, not an alternative speech implementation.
    #[derive(Default)]
    struct Transport {
        playing: bool,
        error: Option<i32>,
        fail_start: bool,
        fail_stop: bool,
        volume: u8,
        bytes: Option<&'static [u8]>,
    }
    impl Backend for Transport {
        fn start(&mut self, b: &'static [u8]) -> Result<(), VoiceError> {
            if self.fail_start {
                return Err(VoiceError::Backend(7));
            }
            self.bytes = Some(b);
            Ok(())
        }
        fn stop(&mut self) -> Result<(), VoiceError> {
            if self.fail_stop {
                return Err(VoiceError::StopFailed);
            }
            self.playing = false;
            self.bytes = None;
            Ok(())
        }
        fn playing(&self) -> bool {
            self.playing
        }
        fn final_error(&self) -> Option<i32> {
            self.error
        }
        fn volume(&mut self, v: u8) {
            self.volume = v;
        }
    }
    #[test]
    fn acceptance_is_not_completion_and_final_errors_are_returned() {
        let mut p = Player::new(Transport::default());
        p.queue(Some(b"first clip"), 0).unwrap();
        assert_eq!(p.progress, Progress::Starting);
        p.backend.playing = true;
        p.tick(1).unwrap();
        assert_eq!(p.progress, Progress::Playing);
        p.backend.playing = false;
        p.backend.error = Some(19);
        assert_eq!(p.tick(TICKS_PER_SECOND), Err(VoiceError::Backend(19)));
        assert_ne!(p.progress, Progress::Complete);
        p.backend.error = None;
        p.queue(Some(b"second clip"), TICKS_PER_SECOND).unwrap();
        p.backend.playing = true;
        p.tick(TICKS_PER_SECOND + 1).unwrap();
        p.backend.playing = false;
        p.tick(2 * TICKS_PER_SECOND).unwrap();
        assert_eq!(p.progress, Progress::Complete);
    }
    #[test]
    fn missing_and_failed_start_do_not_report_completion() {
        let mut p = Player::new(Transport::default());
        assert_eq!(p.queue(None, 0), Err(VoiceError::Missing));
        p.backend.fail_start = true;
        assert_eq!(p.queue(Some(b"clip"), 0), Err(VoiceError::Backend(7)));
        assert_eq!(p.progress, Progress::Idle);
        p.backend.fail_start = false;
        p.queue(Some(b"clip"), 0).unwrap();
        assert_eq!(p.tick(TICKS_PER_SECOND), Err(VoiceError::NotStarted));
    }
    #[test]
    fn replacing_and_muting_cancel_obsolete_clips_and_propagate_stop_failure() {
        let mut p = Player::new(Transport::default());
        p.queue(Some(b"old"), 0).unwrap();
        p.queue(Some(b"current"), 1).unwrap();
        assert_eq!(p.backend.bytes, Some(b"current" as &[u8]));
        p.backend.fail_stop = true;
        assert_eq!(p.queue(Some(b"next"), 2), Err(VoiceError::StopFailed));
        assert_eq!(p.backend.bytes, Some(b"current" as &[u8]));
        p.backend.fail_stop = false;
        p.set_preferences(false, 0).unwrap();
        assert_eq!(p.backend.bytes, None);
        assert_eq!(p.progress, Progress::Idle);
    }
    #[test]
    fn queued_narration_waits_for_final_completion_and_replacement_discards_the_tail() {
        let mut p = Player::new(Transport::default());
        p.queue_sequence([Some(b"label"), Some(b"value"), Some(b"percent")], 0)
            .unwrap();
        assert!(p.busy());
        for (index, expected) in [b"label" as &[u8], b"value", b"percent"]
            .into_iter()
            .enumerate()
        {
            assert_eq!(p.backend.bytes, Some(expected));
            p.backend.playing = true;
            let now = index as u64 * TICKS_PER_SECOND;
            p.tick(now + 1).unwrap();
            p.backend.playing = false;
            p.tick(now + TICKS_PER_SECOND).unwrap();
        }
        assert_eq!(p.progress, Progress::Complete);
        assert!(!p.busy());
        p.queue_sequence(
            [Some(b"old label"), Some(b"old value"), None],
            4 * TICKS_PER_SECOND,
        )
        .unwrap();
        p.queue(Some(b"current card"), 4 * TICKS_PER_SECOND + 1)
            .unwrap();
        assert_eq!(p.pending, [None, None]);
        assert_eq!(p.backend.bytes, Some(b"current card" as &[u8]));
    }
    #[test]
    fn disabled_speech_needs_no_clip_and_invalid_sequence_is_rejected() {
        let mut p = Player::new(Transport::default());
        p.set_preferences(false, 50).unwrap();
        assert!(!p.enabled());
        p.queue(None, 0).unwrap();
        assert!(!p.busy());
        p.set_preferences(true, 50).unwrap();
        assert!(p.enabled());
        assert_eq!(
            p.queue_sequence([Some(b"a"), None, Some(b"b")], 0),
            Err(VoiceError::Missing)
        );
        assert!(!p.busy());
    }
    #[test]
    fn long_playback_has_a_bounded_failure_path() {
        let mut p = Player::new(Transport::default());
        p.queue(Some(b"clip"), 0).unwrap();
        p.backend.playing = true;
        assert_eq!(p.tick(31 * TICKS_PER_SECOND), Err(VoiceError::Timeout));
        assert_eq!(p.progress, Progress::Idle);
        assert!(!p.backend.playing);
    }
}
