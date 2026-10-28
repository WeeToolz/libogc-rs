const GAMECUBE_TIMER_CLOCK_HZ: u32 = 40_500_000;
const WII_TIMER_CLOCK_HZ: u32 = 60_750_000;

pub(super) fn timer_clock_hz(hollywood_revision: u32) -> u32 {
    if hollywood_revision == 0 {
        GAMECUBE_TIMER_CLOCK_HZ
    } else {
        WII_TIMER_CLOCK_HZ
    }
}

// BUG FIX: C scales gettick() by the platform timer clock. A Wii-only rate made
// the same timeout expire 1.5 times late on GameCube.
pub(super) fn timeout_elapsed_ticks(
    elapsed_ticks: u64,
    timeout_ms: u64,
    ticks_per_ms: u64,
) -> bool {
    elapsed_ticks / ticks_per_ms > timeout_ms
}

#[cfg(test)]
mod tests {
    use super::{timeout_elapsed_ticks, timer_clock_hz};

    #[test]
    fn platform_timer_clock_matches_wii_and_gamecube() {
        assert_eq!(timer_clock_hz(0), 40_500_000);
        assert_eq!(timer_clock_hz(1), 60_750_000);
    }

    #[test]
    fn timeout_conversion_uses_gamecube_tick_rate() {
        let ticks_per_ms = u64::from(timer_clock_hz(0) / 1_000);
        assert!(!timeout_elapsed_ticks(
            1_500 * ticks_per_ms,
            1_500,
            ticks_per_ms
        ));
        assert!(timeout_elapsed_ticks(
            1_501 * ticks_per_ms,
            1_500,
            ticks_per_ms
        ));
    }

    #[test]
    fn timeout_conversion_uses_wii_tick_rate() {
        let ticks_per_ms = u64::from(timer_clock_hz(1) / 1_000);
        assert!(!timeout_elapsed_ticks(
            1_500 * ticks_per_ms,
            1_500,
            ticks_per_ms
        ));
        assert!(timeout_elapsed_ticks(
            1_501 * ticks_per_ms,
            1_500,
            ticks_per_ms
        ));
    }
}
