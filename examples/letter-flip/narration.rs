//! Spoken screen content uses the same catalog as the Python generator and the build.
use crate::session::Screen;
use crate::settings::{Mode, Settings};
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Narration {
    pub names: [Option<&'static str>; 3],
}
impl Narration {
    pub const fn single(name: &'static str) -> Self {
        Self {
            names: [Some(name), None, None],
        }
    }
    pub const fn pair(a: &'static str, b: &'static str) -> Self {
        Self {
            names: [Some(a), Some(b), None],
        }
    }
    pub fn screen(screen: Screen, selection: usize, s: Settings, remote: bool) -> Self {
        match screen {
            Screen::Title => Self::single(["play", "how_to_play", "settings"][selection.min(2)]),
            Screen::Help => Self::pair(
                "help",
                if !remote {
                    "controls_gc"
                } else if s.sideways {
                    "controls_sideways"
                } else {
                    "controls_upright"
                },
            ),
            Screen::Settings => Self::setting(selection, s),
            Screen::Pause => {
                Self::single(["resume", "restart", "change_controller", "menu"][selection.min(3)])
            }
            Screen::Over => Self::single("round_complete"),
            _ => Self::default(),
        }
    }
    pub fn review(correct: bool, mirrored: bool, answered: u32) -> Self {
        if correct {
            Self::single(
                ["good_job", "well_done", "continue"][(answered.saturating_sub(1) as usize) % 3],
            )
        } else {
            Self::pair(
                "great_try",
                if mirrored {
                    "backwards_answer"
                } else {
                    "normal_answer"
                },
            )
        }
    }
    pub fn setting(row: usize, s: Settings) -> Self {
        fn toggle(value: bool) -> &'static str {
            if value { "on" } else { "off" }
        }
        let (name, value) = match row {
            0 => (
                "symbols",
                ["all_letters", "uppercase", "lowercase", "numbers"][s.category.index()],
            ),
            1 => (
                "mode",
                if s.mode == Mode::Practice {
                    "practice"
                } else {
                    "challenge"
                },
            ),
            2 => ("chance_limit", number(s.chances)),
            3 => ("practice_cards", number(s.cards)),
            4 => (
                "symbol_pool",
                if s.asymmetric_only {
                    "mirror_shapes"
                } else {
                    "all_shapes"
                },
            ),
            5 => ("review_time", catalog_value("review_", s.feedback_ms)),
            6 => ("voice", toggle(s.voice)),
            7 => ("voice_volume", number(s.voice_volume)),
            8 => ("effects", toggle(s.effects)),
            9 => ("effects_volume", number(s.effects_volume)),
            10 => ("flashes", toggle(s.flashes)),
            11 => ("remote", if s.sideways { "sideways" } else { "upright" }),
            12 => return Self::single("preview"),
            13 => return Self::single("retry_save"),
            _ => return Self::single("back"),
        };
        Self {
            names: [
                Some(name),
                Some(value),
                if row == 7 || row == 9 {
                    Some("percent")
                } else {
                    None
                },
            ],
        }
    }
}
fn catalog_value(prefix: &str, value: u16) -> &'static str {
    include_str!("voice-lines.tsv")
        .lines()
        .filter_map(|line| line.split_once('\t').map(|(name, _)| name))
        .find(|name| {
            name.strip_prefix(prefix)
                .and_then(|number| number.parse::<u16>().ok())
                == Some(value)
        })
        .expect("validated setting has a spoken value in the catalog")
}
fn number(value: u8) -> &'static str {
    catalog_value("value_", u16::from(value))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_screen_and_setting_value_has_a_catalog_entry() {
        let catalog = include_str!("voice-lines.tsv");
        let available: std::collections::BTreeSet<_> = catalog
            .lines()
            .filter_map(|l| l.split_once('\t').map(|(name, _)| name))
            .collect();
        fn check(n: Narration, available: &std::collections::BTreeSet<&str>) {
            for name in n.names.into_iter().flatten() {
                assert!(available.contains(name), "missing spoken text: {name}");
            }
        }
        let mut s = Settings::default();
        for row in 0..15 {
            for _ in 0..101 {
                check(Narration::setting(row, s), &available);
                s.change(row, 1);
            }
            for _ in 0..101 {
                s.change(row, -1);
                check(Narration::setting(row, s), &available);
            }
        }
        for screen in [Screen::Title, Screen::Help, Screen::Pause, Screen::Over] {
            for selection in 0..4 {
                check(Narration::screen(screen, selection, s, true), &available);
            }
        }
        for name in [
            "welcome",
            "good_job",
            "well_done",
            "continue",
            "great_try",
            "normal_answer",
            "backwards_answer",
            "pause",
            "saving",
            "saved",
            "save_failed",
            "no_card",
            "save_recovered",
            "connect_controller",
        ] {
            assert!(available.contains(name));
        }
    }
}
