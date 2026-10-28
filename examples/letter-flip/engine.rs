//! UI-independent rules and shuffling for the mirror-identification game.
use crate::font;

pub const LETTER_COUNT: usize = 52;
pub use crate::settings::Category;
use crate::settings::{Mode, Settings};

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct Card {
    pub symbol: u8,
    pub mirrored: bool,
}

impl Card {
    pub fn is_mirrored(self) -> bool {
        self.mirrored && font::visibly_mirrored(self.symbol)
    }

    pub fn expected_answer(self) -> Answer {
        if self.is_mirrored() {
            Answer::Backwards
        } else {
            Answer::Normal
        }
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum Answer {
    Normal,
    Backwards,
}

pub fn answer_from_buttons(a_pressed: bool, b_pressed: bool) -> Option<Answer> {
    match (a_pressed, b_pressed) {
        (true, false) => Some(Answer::Normal),
        (false, true) => Some(Answer::Backwards),
        _ => None,
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ResultKind {
    Correct,
    Wrong,
    Out,
}

pub struct Game {
    rng: u32,
    order: [u8; LETTER_COUNT],
    index: usize,
    category: Category,
    pub current: Card,
    pub score: u32,
    pub chances: u8,
    pub answered: u32,
    pub settings: Settings,
    pub finished: bool,
    orientations: [bool; LETTER_COUNT],
    deck_count: usize,
    answered_current: bool,
}

fn xorshift(mut state: u32) -> u32 {
    if state == 0 {
        state = 0xA173_582F;
    }
    state ^= state << 13;
    state ^= state >> 17;
    state ^= state << 5;
    state
}

impl Game {
    pub fn new(seed: u32, settings: Settings) -> Self {
        assert!(settings.valid(), "invalid game settings");
        let mut game = Self {
            rng: seed,
            order: [0; LETTER_COUNT],
            index: LETTER_COUNT,
            category: settings.category,
            current: Card {
                symbol: b'A',
                mirrored: false,
            },
            score: 0,
            chances: settings.chances,
            answered: 0,
            settings,
            finished: false,
            orientations: [false; LETTER_COUNT],
            deck_count: 0,
            answered_current: false,
        };
        game.next_card();
        game
    }

    fn random(&mut self) -> u32 {
        self.rng = xorshift(self.rng);
        self.rng
    }

    fn shuffle(&mut self) {
        let previous = self.current.symbol;
        let has_previous = self.deck_count != 0;
        let mut count = 0;
        for i in 0..self.category.count() {
            let symbol = self.category.symbol(i).expect("valid category index");
            if !self.settings.asymmetric_only || font::visibly_mirrored(symbol) {
                self.order[count] = symbol;
                count += 1;
            }
        }
        self.deck_count = count;
        let mirrored_count = count / 2
            + if count % 2 == 1 && self.random() & 1 == 0 {
                1
            } else {
                0
            };
        for i in 0..count {
            self.orientations[i] = i < mirrored_count;
        }
        for i in (1..count).rev() {
            let j = self.random() as usize % (i + 1);
            self.orientations.swap(i, j);
        }
        for i in (1..count).rev() {
            let j = (self.random() as usize) % (i + 1);
            self.order.swap(i, j);
        }
        if has_previous && count > 1 && self.order[0] == previous {
            self.order.swap(0, 1);
        }
        self.index = 0;
    }

    pub fn next_card(&mut self) {
        if self.finished {
            return;
        }
        if self.index >= self.deck_count {
            self.shuffle();
        }
        let symbol = self.order[self.index];
        self.index += 1;
        // Every symbol can be tested in the normal orientation, but do not
        // ask the player to distinguish pixels that look the same when mirrored.
        let mirrored = if self.settings.asymmetric_only {
            self.orientations[self.index - 1]
        } else {
            font::visibly_mirrored(symbol) && self.random() & 1 == 0
        };
        self.current = Card { symbol, mirrored };
        self.answered_current = false;
    }

    pub fn answer(&mut self, answer: Answer) -> Option<ResultKind> {
        if self.finished || self.answered_current {
            return None;
        }
        self.answered_current = true;
        self.answered = self.answered.saturating_add(1);
        let correct = answer == self.current.expected_answer();
        if correct {
            self.score = self.score.saturating_add(1);
        } else if self.settings.mode == Mode::Challenge {
            self.chances -= 1;
        }
        self.finished = match self.settings.mode {
            Mode::Practice => self.answered >= u32::from(self.settings.cards),
            Mode::Challenge => self.chances == 0,
        };
        Some(if correct {
            ResultKind::Correct
        } else if self.finished {
            ResultKind::Out
        } else {
            ResultKind::Wrong
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn each_category_covers_all_symbols_and_does_not_repeat_at_boundary() {
        for category in Category::ALL {
            for seed in [0, 7, 42, 67] {
                let mut g = Game::new(
                    seed,
                    Settings {
                        category,
                        ..Settings::default()
                    },
                );
                let mut seen = [false; 128];
                for _ in 0..category.count() {
                    let c = g.current;
                    assert!(!seen[c.symbol as usize]);
                    seen[c.symbol as usize] = true;
                    g.next_card();
                    assert_ne!(c.symbol, g.current.symbol);
                }
                assert_eq!(seen.iter().filter(|&&b| b).count(), category.count());
            }
        }
    }
    #[test]
    fn practice_finishes_after_card_count_without_losing_chances() {
        let mut g = Game::new(
            7,
            Settings {
                cards: 10,
                ..Settings::default()
            },
        );
        for i in 0..10 {
            let wrong = if g.current.is_mirrored() {
                Answer::Normal
            } else {
                Answer::Backwards
            };
            assert!(g.answer(wrong).is_some());
            assert_eq!(g.chances, 10);
            assert_eq!(g.finished, i == 9);
            assert_eq!(g.answer(wrong), None);
            g.next_card();
        }
        assert_eq!((g.score, g.answered), (0, 10));
    }
    #[test]
    fn challenge_uses_selected_chances_and_scores_once_per_card() {
        let mut g = Game::new(
            7,
            Settings {
                mode: Mode::Challenge,
                chances: 2,
                ..Settings::default()
            },
        );
        g.current = Card {
            symbol: b'R',
            mirrored: true,
        };
        assert_eq!(g.answer(Answer::Backwards), Some(ResultKind::Correct));
        assert_eq!(g.answer(Answer::Backwards), None);
        assert_eq!(g.score, 1);
        for remaining in [1, 0] {
            g.next_card();
            let wrong = if g.current.is_mirrored() {
                Answer::Normal
            } else {
                Answer::Backwards
            };
            assert!(g.answer(wrong).is_some());
            assert_eq!(g.chances, remaining);
        }
        assert!(g.finished);
    }
    #[test]
    fn asymmetric_pool_balances_orientation_and_symmetric_cards_are_normal() {
        for category in Category::ALL {
            let mut g = Game::new(
                42,
                Settings {
                    category,
                    asymmetric_only: true,
                    ..Settings::default()
                },
            );
            for _ in 0..10 {
                g.shuffle();
                let count = g.deck_count;
                let mut mirrored = 0;
                for _ in 0..count {
                    g.next_card();
                    assert!(font::visibly_mirrored(g.current.symbol));
                    mirrored += usize::from(g.current.mirrored);
                }
                assert!(mirrored.abs_diff(count - mirrored) <= 1);
            }
        }
        for symbol in [b'O', b'0', b'8'] {
            assert_eq!(
                Card {
                    symbol,
                    mirrored: true
                }
                .expected_answer(),
                Answer::Normal
            );
        }
    }
    #[test]
    fn answer_buttons_reject_ambiguous_input() {
        assert_eq!(answer_from_buttons(true, false), Some(Answer::Normal));
        assert_eq!(answer_from_buttons(false, true), Some(Answer::Backwards));
        assert_eq!(answer_from_buttons(true, true), None);
        assert_eq!(answer_from_buttons(false, false), None);
    }
}
