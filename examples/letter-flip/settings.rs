//! Validated options shared by the game, screens, and save format.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Letters,
    Uppercase,
    Lowercase,
    Numbers,
}
impl Category {
    pub const ALL: [Self; 4] = [
        Self::Letters,
        Self::Uppercase,
        Self::Lowercase,
        Self::Numbers,
    ];
    pub const fn index(self) -> usize {
        self as usize
    }
    pub const fn label(self) -> &'static [u8] {
        match self {
            Self::Letters => b"ALL LETTERS",
            Self::Uppercase => b"UPPERCASE",
            Self::Lowercase => b"LOWERCASE",
            Self::Numbers => b"NUMBERS",
        }
    }
    pub const fn count(self) -> usize {
        match self {
            Self::Letters => 52,
            Self::Numbers => 10,
            _ => 26,
        }
    }
    pub fn symbol(self, index: usize) -> Option<u8> {
        if index >= self.count() {
            return None;
        }
        Some(match self {
            Self::Letters if index >= 26 => b'a' + (index - 26) as u8,
            Self::Letters | Self::Uppercase => b'A' + index as u8,
            Self::Lowercase => b'a' + index as u8,
            Self::Numbers => b'0' + index as u8,
        })
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Practice,
    Challenge,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Settings {
    pub category: Category,
    pub mode: Mode,
    pub chances: u8,
    pub cards: u8,
    pub asymmetric_only: bool,
    pub feedback_ms: u16,
    pub voice: bool,
    pub voice_volume: u8,
    pub effects: bool,
    pub effects_volume: u8,
    pub flashes: bool,
    pub sideways: bool,
}
impl Default for Settings {
    fn default() -> Self {
        Self {
            category: Category::Letters,
            mode: Mode::Practice,
            chances: 10,
            cards: 20,
            asymmetric_only: false,
            feedback_ms: 2000,
            voice: true,
            voice_volume: 50,
            effects: true,
            effects_volume: 35,
            flashes: false,
            sideways: false,
        }
    }
}
impl Settings {
    pub fn valid(self) -> bool {
        (1..=20).contains(&self.chances)
            && (10..=100).contains(&self.cards)
            && self.cards % 10 == 0
            && (500..=5000).contains(&self.feedback_ms)
            && self.feedback_ms % 500 == 0
            && self.voice_volume <= 100
            && self.effects_volume <= 100
    }
    /// Records are separate for category, pool, mode, and round length or chance limit.
    pub fn record_index(self) -> usize {
        let limit = match self.mode {
            Mode::Practice => self.cards / 10 - 1,
            Mode::Challenge => self.chances - 1,
        };
        ((self.category.index() * 2 + usize::from(self.asymmetric_only)) * 2 + self.mode as usize)
            * 20
            + limit as usize
    }
    pub fn change(&mut self, row: usize, direction: i8) {
        fn step(value: u8, delta: i8, low: u8, high: u8, amount: u8) -> u8 {
            if delta < 0 {
                value.saturating_sub(amount).max(low)
            } else {
                value.saturating_add(amount).min(high)
            }
        }
        match row {
            0 => {
                self.category =
                    Category::ALL[(self.category.index() + if direction < 0 { 3 } else { 1 }) % 4]
            }
            1 => {
                self.mode = if self.mode == Mode::Practice {
                    Mode::Challenge
                } else {
                    Mode::Practice
                }
            }
            2 => self.chances = step(self.chances, direction, 1, 20, 1),
            3 => self.cards = step(self.cards, direction, 10, 100, 10),
            4 => self.asymmetric_only = !self.asymmetric_only,
            5 => {
                self.feedback_ms = if direction < 0 {
                    self.feedback_ms.saturating_sub(500).max(500)
                } else {
                    (self.feedback_ms + 500).min(5000)
                }
            }
            6 => self.voice = !self.voice,
            7 => self.voice_volume = step(self.voice_volume, direction, 0, 100, 5),
            8 => self.effects = !self.effects,
            9 => self.effects_volume = step(self.effects_volume, direction, 0, 100, 5),
            10 => self.flashes = !self.flashes,
            11 => self.sideways = !self.sideways,
            _ => {}
        }
    }
}
pub const RECORD_COUNT: usize = 4 * 2 * 2 * 20;
pub const SETTING_ROWS: usize = 15;
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn all_changes_remain_valid_and_records_do_not_collide() {
        let mut s = Settings::default();
        for row in 0..12 {
            for _ in 0..150 {
                s.change(row, 1);
                assert!(s.valid());
            }
            for _ in 0..150 {
                s.change(row, -1);
                assert!(s.valid());
            }
        }
        let mut seen = [false; RECORD_COUNT];
        for category in Category::ALL {
            for asymmetric_only in [false, true] {
                for mode in [Mode::Practice, Mode::Challenge] {
                    for limit in 1..=if mode == Mode::Practice { 10 } else { 20 } {
                        let s = Settings {
                            category,
                            asymmetric_only,
                            mode,
                            chances: limit,
                            cards: limit.min(10) * 10,
                            ..Settings::default()
                        };
                        let i = s.record_index();
                        assert!(!seen[i]);
                        seen[i] = true;
                    }
                }
            }
        }
    }
}
