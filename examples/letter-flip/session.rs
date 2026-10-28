//! Screen transitions and elapsed-time rules used directly by the Wii application.
use crate::engine::{Game, ResultKind, answer_from_buttons};
use crate::narration::Narration;
use crate::save::Data;
use crate::settings::{Mode, SETTING_ROWS};

pub const TICKS_PER_SECOND: u64 = 60_750_000;
pub const UP: u16 = 1;
pub const DOWN: u16 = 2;
pub const LEFT: u16 = 4;
pub const RIGHT: u16 = 8;
pub const A: u16 = 16;
pub const B: u16 = 32;
pub const MENU: u16 = 64;
pub const REPEAT: u16 = 128;
pub const ONE: u16 = 256;
pub const TWO: u16 = 512;
/// Rotate only the remote's controls. The shared controller service remains unchanged.
pub fn controls(raw: u16, remote: bool, sideways: bool) -> u16 {
    if !remote || !sideways {
        return raw;
    }
    let mut out = raw & (MENU | REPEAT);
    for (from, to) in [
        (LEFT, DOWN),
        (RIGHT, UP),
        (UP, LEFT),
        (DOWN, RIGHT),
        (ONE, B),
        (TWO, A),
    ] {
        if raw & from != 0 {
            out |= to;
        }
    }
    out
}
pub fn millis_ticks(ms: u16) -> u64 {
    u64::from(ms) * TICKS_PER_SECOND / 1000
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Screen {
    Title,
    Help,
    Settings,
    Playing,
    Feedback,
    Pause,
    Over,
}
#[derive(Default, Debug)]
pub struct Events {
    pub speak: Option<u8>,
    pub narration: Narration,
    pub cancel_voice: bool,
    pub effect: Option<bool>,
    pub retry_save: bool,
    pub preview: bool,
}
pub struct Session {
    pub data: Data,
    pub game: Game,
    pub screen: Screen,
    pub owner: Option<u8>,
    pub selection: usize,
    pub feedback: ResultKind,
    pub dirty: bool,
    pub audio_error: bool,
    pub feedback_elapsed: u64,
    pub speech_busy: bool,
    welcome_pending: bool,
    startup_elapsed: u64,
    resume: Screen,
    last_time: u64,
    seed: u32,
}
impl Session {
    pub fn new(data: Data, now: u64) -> Self {
        let game = Game::new(now as u32, data.settings);
        Self {
            data,
            game,
            screen: Screen::Title,
            owner: None,
            selection: 0,
            feedback: ResultKind::Correct,
            dirty: false,
            audio_error: false,
            feedback_elapsed: 0,
            speech_busy: false,
            welcome_pending: true,
            startup_elapsed: 0,
            resume: Screen::Playing,
            last_time: now,
            seed: now as u32,
        }
    }
    fn start(&mut self, events: &mut Events) {
        self.seed = self.seed.wrapping_add(0x9E37_79B9) ^ self.last_time as u32;
        self.game = Game::new(self.seed, self.data.settings);
        self.screen = Screen::Playing;
        self.feedback_elapsed = 0;
        events.cancel_voice = true;
        events.speak = Some(self.game.current.symbol);
    }
    fn title(&mut self, events: &mut Events) {
        self.screen = Screen::Title;
        self.selection = 0;
        self.owner = None;
        events.cancel_voice = true;
    }
    fn pause(&mut self, events: &mut Events) {
        self.resume = self.screen;
        self.screen = Screen::Pause;
        self.selection = 0;
        events.cancel_voice = true;
        events.narration = Narration::single("pause");
    }
    pub fn audio_failure(&mut self) {
        self.audio_error = true;
    }
    pub fn merge_disk(&mut self, data: &Data) {
        self.data.merge_records(data);
    }
    /// Input belongs to one controller. Other controllers cannot answer or move menus.
    pub fn step(&mut self, now: u64, input: Option<(u8, u16)>, owner_connected: bool) -> Events {
        let mut events = Events::default();
        let previous = (self.screen, self.selection, self.data.settings);
        let delta = now.saturating_sub(self.last_time);
        self.last_time = now;
        if self.welcome_pending && self.screen == Screen::Title {
            self.startup_elapsed = self.startup_elapsed.saturating_add(delta);
        }
        if matches!(self.screen, Screen::Playing | Screen::Feedback)
            && self.owner.is_some()
            && !owner_connected
        {
            self.pause(&mut events);
            self.owner = None;
            events.narration = Narration::pair("connect_controller", "pause");
            return events;
        }
        if self.owner.is_some() && !owner_connected {
            self.owner = None;
        }
        let keys = match input {
            Some((id, keys)) if keys != 0 && (self.owner.is_none() || self.owner == Some(id)) => {
                self.owner = Some(id);
                keys
            }
            _ => 0,
        };
        match self.screen {
            Screen::Title => {
                self.navigate(keys, 3);
                if keys & A != 0 || keys & MENU != 0 {
                    match self.selection {
                        0 => self.start(&mut events),
                        1 => self.screen = Screen::Help,
                        _ => {
                            self.screen = Screen::Settings;
                            self.selection = 0;
                        }
                    }
                }
            }
            Screen::Help => {
                if keys & (A | B | MENU) != 0 {
                    self.title(&mut events);
                }
            }
            Screen::Settings => {
                if keys & (B | MENU) != 0 {
                    self.title(&mut events);
                } else {
                    self.navigate(keys, SETTING_ROWS);
                    let direction = if keys & LEFT != 0 { -1 } else { 1 };
                    if keys & (LEFT | RIGHT | A) != 0 {
                        match self.selection {
                            12 => events.preview = keys & A != 0,
                            13 => events.retry_save = keys & A != 0,
                            14 => {
                                if keys & A != 0 {
                                    self.title(&mut events);
                                }
                            }
                            row => {
                                let old = self.data.settings;
                                self.data.settings.change(row, direction);
                                self.dirty |= old != self.data.settings;
                            }
                        }
                    }
                }
            }
            Screen::Playing => {
                if keys & MENU != 0 {
                    self.pause(&mut events);
                } else if keys & REPEAT != 0 {
                    events.speak = Some(self.game.current.symbol);
                } else if let Some(answer) = answer_from_buttons(keys & A != 0, keys & B != 0) {
                    if let Some(result) = self.game.answer(answer) {
                        self.feedback = result;
                        self.feedback_elapsed = 0;
                        self.screen = Screen::Feedback;
                        events.cancel_voice = true;
                        events.effect = Some(result == ResultKind::Correct);
                        events.narration = Narration::review(
                            result == ResultKind::Correct,
                            self.game.current.is_mirrored(),
                            self.game.answered,
                        );
                        self.dirty |= self.data.record(self.game.settings, self.game.score);
                    }
                }
            }
            Screen::Feedback => {
                if keys & MENU != 0 {
                    self.pause(&mut events);
                } else {
                    if keys & REPEAT != 0 {
                        events.narration = Narration::review(
                            self.feedback == ResultKind::Correct,
                            self.game.current.is_mirrored(),
                            self.game.answered,
                        );
                    }
                    self.feedback_elapsed = self.feedback_elapsed.saturating_add(delta);
                    let duration = if self.feedback == ResultKind::Correct {
                        millis_ticks(500)
                    } else {
                        millis_ticks(self.game.settings.feedback_ms)
                    };
                    if self.feedback_elapsed >= duration && !self.speech_busy && keys & REPEAT == 0
                    {
                        if self.game.finished {
                            self.screen = Screen::Over;
                            self.selection = 0;
                            self.owner = None;
                            events.cancel_voice = true;
                        } else {
                            self.game.next_card();
                            self.screen = Screen::Playing;
                            events.speak = Some(self.game.current.symbol);
                        }
                    }
                }
            }
            Screen::Pause => {
                self.navigate(keys, 4);
                if keys & A != 0 {
                    match self.selection {
                        0 => {
                            self.screen = self.resume;
                            if self.resume == Screen::Playing {
                                events.speak = Some(self.game.current.symbol);
                            }
                        }
                        1 => self.start(&mut events),
                        2 => {
                            self.owner = None;
                            self.selection = 0;
                            events.narration = Narration::single("change_controller");
                        }
                        _ => self.title(&mut events),
                    }
                }
            }
            Screen::Over => {
                if keys & A != 0 {
                    self.start(&mut events);
                } else if keys & (B | MENU) != 0 {
                    self.title(&mut events);
                }
            }
        }
        if self.welcome_pending && (self.screen != Screen::Title || keys != 0) {
            self.welcome_pending = false;
        }
        if events.speak.is_none() && events.narration.names[0].is_none() {
            let changed = previous != (self.screen, self.selection, self.data.settings);
            if self.welcome_pending && self.startup_elapsed >= millis_ticks(3_000) {
                events.narration = Narration::single("welcome");
                self.welcome_pending = false;
            } else if !self.welcome_pending && (changed || keys & REPEAT != 0) {
                events.narration = Narration::screen(
                    self.screen,
                    self.selection,
                    self.data.settings,
                    self.owner.is_some_and(|id| id >= 4),
                );
            }
        }
        events
    }
    fn navigate(&mut self, keys: u16, count: usize) {
        if keys & UP != 0 {
            self.selection = (self.selection + count - 1) % count;
        } else if keys & DOWN != 0 {
            self.selection = (self.selection + 1) % count;
        }
    }
    pub fn best(&self) -> u32 {
        self.data.best(
            if matches!(
                self.screen,
                Screen::Playing | Screen::Feedback | Screen::Pause | Screen::Over
            ) {
                self.game.settings
            } else {
                self.data.settings
            },
        )
    }
    pub fn mode_label(&self) -> &'static [u8] {
        if self.data.settings.mode == Mode::Practice {
            b"PRACTICE"
        } else {
            b"CHALLENGE"
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    fn started() -> Session {
        let mut s = Session::new(Data::default(), 0);
        s.step(0, Some((4, A)), true);
        s
    }
    #[test]
    fn only_owner_can_answer_and_disconnect_pauses() {
        let mut s = started();
        s.step(1, Some((5, A)), true);
        assert_eq!(s.game.answered, 0);
        s.step(2, None, false);
        assert_eq!(s.screen, Screen::Pause);
        assert_eq!(s.game.answered, 0);
        s.selection = 2;
        s.step(3, Some((4, A)), true);
        assert_eq!(s.owner, None);
        s.step(4, Some((5, A)), true);
        assert_eq!(s.owner, Some(5));
        assert_eq!(s.screen, Screen::Playing);
    }
    #[test]
    fn feedback_uses_elapsed_time_and_pause_does_not_consume_it() {
        let mut s = started();
        let correct = if s.game.current.is_mirrored() { B } else { A };
        s.step(1, Some((4, correct)), true);
        assert_eq!(s.screen, Screen::Feedback);
        s.step(1 + millis_ticks(200), None, true);
        s.step(1 + millis_ticks(200), Some((4, MENU)), true);
        assert_eq!(s.screen, Screen::Pause);
        s.step(10 * TICKS_PER_SECOND, None, true);
        s.step(10 * TICKS_PER_SECOND, Some((4, A)), true);
        assert_eq!(s.screen, Screen::Feedback);
        s.step(10 * TICKS_PER_SECOND + millis_ticks(299), None, true);
        assert_eq!(s.screen, Screen::Feedback);
        s.step(10 * TICKS_PER_SECOND + millis_ticks(300), None, true);
        assert_eq!(s.screen, Screen::Playing);
    }
    #[test]
    fn settings_change_is_pending_and_retry_does_not_claim_save_success() {
        let mut s = Session::new(Data::default(), 0);
        s.screen = Screen::Settings;
        s.selection = 2;
        s.step(0, Some((4, RIGHT)), true);
        assert_eq!(s.data.settings.chances, 11);
        assert!(s.dirty);
        s.selection = 13;
        assert!(s.step(1, Some((4, A)), true).retry_save);
        assert!(s.dirty);
    }
    #[test]
    fn audio_errors_are_visible_and_recovered_records_do_not_change_local_settings() {
        let mut s = started();
        s.audio_failure();
        assert!(s.audio_error);
        let mut disk = Data::default();
        disk.settings.chances = 19;
        disk.record(disk.settings, 13);
        s.merge_disk(&disk);
        assert_eq!(s.data.settings.chances, 10);
        assert_eq!(s.data.best(disk.settings), 13);
    }
    #[test]
    fn welcome_voice_waits_three_seconds_after_boot() {
        let mut s = Session::new(Data::default(), 0);
        let delay = millis_ticks(3_000);
        assert_eq!(s.step(0, None, false).narration.names[0], None);
        assert_eq!(s.step(delay - 1, None, false).narration.names[0], None);
        assert_eq!(
            s.step(delay, None, false).narration.names[0],
            Some("welcome")
        );
        assert_eq!(s.step(delay + 1, None, false).narration.names[0], None);
    }
    #[test]
    fn disconnected_controller_can_be_replaced_without_losing_the_round() {
        let mut s = started();
        let card = s.game.current;
        s.step(1, None, false);
        assert_eq!(s.owner, None);
        assert_eq!(s.screen, Screen::Pause);
        s.step(2, Some((5, A)), false);
        assert_eq!(s.owner, Some(5));
        assert_eq!(s.screen, Screen::Playing);
        assert_eq!(s.game.current, card);
    }
    #[test]
    fn spoken_feedback_finishes_before_the_next_card_and_can_be_repeated() {
        let mut s = started();
        let correct = if s.game.current.is_mirrored() { B } else { A };
        let events = s.step(1, Some((4, correct)), true);
        assert_eq!(events.narration.names[0], Some("good_job"));
        s.speech_busy = true;
        s.step(TICKS_PER_SECOND, None, true);
        assert_eq!(s.screen, Screen::Feedback);
        s.speech_busy = false;
        let repeat = s.step(2 * TICKS_PER_SECOND, Some((4, REPEAT)), true);
        assert!(repeat.narration.names[0].is_some());
        assert_eq!(s.screen, Screen::Feedback);
        s.step(2 * TICKS_PER_SECOND + 1, None, true);
        assert_eq!(s.screen, Screen::Playing);
    }
    #[test]
    fn screen_changes_and_setting_values_are_read_aloud() {
        let mut s = Session::new(Data::default(), 0);
        assert_eq!(s.step(0, None, false).narration.names[0], None);
        assert_eq!(
            s.step(1, Some((4, DOWN)), false).narration.names[0],
            Some("how_to_play")
        );
        assert_eq!(
            s.step(2, Some((4, A)), true).narration.names[0],
            Some("help")
        );
        s.screen = Screen::Settings;
        s.selection = 2;
        let read = s.step(3, Some((4, RIGHT)), true);
        assert_eq!(
            read.narration.names,
            [Some("chance_limit"), Some("value_11"), None]
        );
    }
    #[test]
    fn sideways_rotation_and_buttons_are_device_specific() {
        assert_eq!(controls(LEFT | TWO | MENU, true, true), DOWN | A | MENU);
        assert_eq!(controls(RIGHT | ONE, true, true), UP | B);
        assert_eq!(controls(A | UP, false, true), A | UP);
        assert_eq!(controls(A | UP, true, false), A | UP);
    }
    #[test]
    fn repeat_and_exit_cancel_obsolete_audio() {
        let mut s = started();
        assert_eq!(
            s.step(1, Some((4, REPEAT)), true).speak,
            Some(s.game.current.symbol)
        );
        assert!(s.step(2, Some((4, MENU)), true).cancel_voice);
        s.selection = 3;
        assert!(s.step(3, Some((4, A)), true).cancel_voice);
        assert_eq!(s.screen, Screen::Title);
    }
}
