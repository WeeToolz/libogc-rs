//! Shared menu transitions used by the game and host-side tests.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Screen {
    Title,
    MainMenu,
    HowTo,
    Options,
    Playing,
    Pause,
    ConfirmRestart,
    ConfirmTitle,
    Won,
    GameOver,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RestartTransition {
    pub destination: Screen,
    pub reset_game: bool,
}

pub const fn restart_transition(confirmed: bool) -> RestartTransition {
    if confirmed {
        RestartTransition {
            destination: Screen::Playing,
            reset_game: true,
        }
    } else {
        RestartTransition {
            destination: Screen::Pause,
            reset_game: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn confirming_restart_starts_a_new_game() {
        assert_eq!(
            restart_transition(true),
            RestartTransition {
                destination: Screen::Playing,
                reset_game: true,
            }
        );
    }

    #[test]
    fn cancelling_restart_returns_to_pause_without_reset() {
        assert_eq!(
            restart_transition(false),
            RestartTransition {
                destination: Screen::Pause,
                reset_game: false,
            }
        );
    }
}
