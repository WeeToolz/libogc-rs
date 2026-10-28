//! Millisecond timestamps for the agent watchdog, including the RTC epoch.

/// `frequency` is the nonzero Wii/GameCube timer frequency.
pub fn timestamp_ms(ticks: u64, frequency: u64) -> u32 {
    // Convert whole seconds separately to avoid saturating the RTC timestamp.
    let whole_ms = (ticks / frequency).wrapping_mul(1_000);
    let fraction_ms = (ticks % frequency) * 1_000 / frequency;
    whole_ms.wrapping_add(fraction_ms) as u32
}

#[cfg(test)]
mod tests {
    use super::timestamp_ms;

    #[test]
    fn clock_advances_with_a_large_rtc_epoch() {
        assert_eq!(
            timestamp_ms(60_750_000_000_000_000, 60_750_000),
            3_567_587_328
        );
        assert_eq!(
            timestamp_ms(60_750_000_015_187_500, 60_750_000),
            3_567_587_578
        );
    }

    #[test]
    fn elapsed_milliseconds_survive_counter_wrap() {
        let before = timestamp_ms(260_919_262_928_250, 60_750_000);
        let after = timestamp_ms(260_919_263_535_750, 60_750_000);
        assert_eq!(before, u32::MAX - 4);
        assert_eq!(after, 5);
        assert_eq!(after.wrapping_sub(before), 10);
    }
}
