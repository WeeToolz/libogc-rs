//! Checked clock state shared by the hardware service and host tests.

use core::time::Duration;

pub const RTC_UNIX_EPOCH_OFFSET: u64 = 946_684_800;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClockError {
    NotInitialized,
    UnsupportedHardware,
    ExceptionContext,
    InterruptsDisabled,
    Busy,
    TimebaseReversed,
    RtcFailure {
        stage: RtcStage,
        cleanup_failures: u8,
    },
    RtcUnstable,
    CounterBias(i32),
    UtcOffsetMissing,
    InvalidUtcOffset,
    Arithmetic,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RtcStage {
    Lock,
    Select,
    Write,
    WriteSync,
    Read,
    ReadSync,
    Deselect,
    Unlock,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ClockSnapshot {
    pub monotonic_initialized: bool,
    pub monotonic_error: Option<ClockError>,
    pub wall_error: Option<ClockError>,
    pub rtc_seconds: Option<u32>,
    pub counter_bias: Option<u32>,
    /// Seconds east of UTC, supplied explicitly for the Wii's local calendar.
    pub utc_offset_seconds: Option<i32>,
}

pub(crate) struct ClockState {
    frequency: u64,
    origin: Option<u64>,
    last: u64,
    wall: Option<(u64, u64)>,
    snapshot: ClockSnapshot,
}

/// Divide before multiplication. A full u64 tick counter is representable.
pub(crate) fn ticks_duration(ticks: u64, frequency: u64) -> Result<Duration, ClockError> {
    if frequency == 0 {
        return Err(ClockError::Arithmetic);
    }
    let seconds = ticks / frequency;
    let nanos = u128::from(ticks % frequency) * 1_000_000_000 / u128::from(frequency);
    Ok(Duration::new(seconds, nanos as u32))
}

impl ClockState {
    pub(crate) const fn new(frequency: u64) -> Self {
        Self {
            frequency,
            origin: None,
            last: 0,
            wall: None,
            snapshot: ClockSnapshot {
                monotonic_initialized: false,
                monotonic_error: None,
                wall_error: Some(ClockError::NotInitialized),
                rtc_seconds: None,
                counter_bias: None,
                utc_offset_seconds: None,
            },
        }
    }

    pub(crate) fn initialize(&mut self, ticks: u64) -> Result<(), ClockError> {
        if self.frequency == 0 {
            self.snapshot.monotonic_error = Some(ClockError::Arithmetic);
            return Err(ClockError::Arithmetic);
        }
        if self.origin.is_some() {
            return self.check_ticks(ticks);
        }
        self.origin = Some(ticks);
        self.last = ticks;
        self.snapshot.monotonic_initialized = true;
        Ok(())
    }

    fn check_ticks(&mut self, ticks: u64) -> Result<(), ClockError> {
        if let Some(error) = self.snapshot.monotonic_error {
            return Err(error);
        }
        if self.origin.is_none() {
            return Err(ClockError::NotInitialized);
        }
        if ticks < self.last {
            self.snapshot.monotonic_error = Some(ClockError::TimebaseReversed);
            return Err(ClockError::TimebaseReversed);
        }
        self.last = ticks;
        Ok(())
    }

    pub(crate) fn monotonic(&mut self, ticks: u64) -> Result<Duration, ClockError> {
        self.check_ticks(ticks)?;
        ticks_duration(
            ticks - self.origin.ok_or(ClockError::NotInitialized)?,
            self.frequency,
        )
    }

    pub(crate) fn set_utc_offset(&mut self, seconds: i32) -> Result<(), ClockError> {
        if !(-86_399..=86_399).contains(&seconds) {
            return Err(ClockError::InvalidUtcOffset);
        }
        self.snapshot.utc_offset_seconds = Some(seconds);
        if self.wall.is_some() {
            self.snapshot.wall_error = None;
        }
        Ok(())
    }

    /// Capture the actual RTC and bias at the end of its read interval. Bias
    /// addition uses the console's 32-bit calendar counter representation.
    pub(crate) fn synchronize(
        &mut self,
        ticks: u64,
        sample: Result<(u32, u32), ClockError>,
    ) -> Result<(), ClockError> {
        self.check_ticks(ticks)?;
        match sample {
            Ok((rtc, bias)) => {
                self.wall = Some((ticks, u64::from(rtc.wrapping_add(bias))));
                self.snapshot.rtc_seconds = Some(rtc);
                self.snapshot.counter_bias = Some(bias);
                self.snapshot.wall_error = if self.snapshot.utc_offset_seconds.is_some() {
                    None
                } else {
                    Some(ClockError::UtcOffsetMissing)
                };
                Ok(())
            }
            Err(error) => {
                // A failed refresh cannot continue to advertise a valid clock.
                self.wall = None;
                self.snapshot.wall_error = Some(error);
                Err(error)
            }
        }
    }

    pub(crate) fn wall(&mut self, ticks: u64) -> Result<Duration, ClockError> {
        self.check_ticks(ticks)?;
        if let Some(error) = self.snapshot.wall_error {
            return Err(error);
        }
        let offset = self
            .snapshot
            .utc_offset_seconds
            .ok_or(ClockError::UtcOffsetMissing)?;
        let (anchor, local) = self.wall.ok_or(ClockError::NotInitialized)?;
        let utc = i64::try_from(local).map_err(|_| ClockError::Arithmetic)?
            + RTC_UNIX_EPOCH_OFFSET as i64
            - i64::from(offset);
        let utc = u64::try_from(utc).map_err(|_| ClockError::Arithmetic)?;
        Duration::from_secs(utc)
            .checked_add(ticks_duration(ticks - anchor, self.frequency)?)
            .ok_or(ClockError::Arithmetic)
    }

    pub(crate) fn record_wall_failure(&mut self, error: ClockError) {
        self.wall = None;
        self.snapshot.wall_error = Some(error);
    }

    pub(crate) fn snapshot(&self) -> ClockSnapshot {
        self.snapshot
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tick_conversion_uses_fixed_independent_values() {
        assert_eq!(ticks_duration(1, 0), Err(ClockError::Arithmetic));
        assert_eq!(
            ticks_duration(u64::MAX, u64::MAX),
            Ok(Duration::from_secs(1))
        );
        let mut invalid = ClockState::new(0);
        assert_eq!(invalid.initialize(7), Err(ClockError::Arithmetic));
        assert_eq!(invalid.monotonic(8), Err(ClockError::Arithmetic));
        assert_eq!(
            invalid.snapshot().monotonic_error,
            Some(ClockError::Arithmetic)
        );

        assert_eq!(ticks_duration(0, 60_750_000).unwrap(), Duration::ZERO);
        assert_eq!(
            ticks_duration(1, 60_750_000).unwrap(),
            Duration::from_nanos(16)
        );
        assert_eq!(
            ticks_duration(60_749_999, 60_750_000).unwrap(),
            Duration::new(0, 999_999_983)
        );
        assert_eq!(
            ticks_duration(60_750_000, 60_750_000).unwrap(),
            Duration::from_secs(1)
        );
        assert_eq!(
            ticks_duration(u64::MAX, 60_750_000).unwrap(),
            Duration::new(303_650_108_209, 210_726_172)
        );
    }

    #[test]
    fn monotonic_origin_is_kept_across_repeated_initialization_and_wall_refresh() {
        let mut state = ClockState::new(60_750_000);
        assert_eq!(state.monotonic(7), Err(ClockError::NotInitialized));
        state.initialize(7).unwrap();
        state.initialize(60_750_007).unwrap();
        assert_eq!(state.monotonic(60_750_007), Ok(Duration::from_secs(1)));
        state.synchronize(121_500_007, Ok((100, 200))).unwrap();
        assert_eq!(state.monotonic(121_500_007), Ok(Duration::from_secs(2)));
        state.synchronize(182_250_007, Ok((20, 10))).unwrap();
        assert_eq!(state.monotonic(182_250_007), Ok(Duration::from_secs(3)));
    }

    #[test]
    fn reversed_timebase_failure_is_sticky_and_is_not_clamped_to_success() {
        let mut state = ClockState::new(60_750_000);
        state.initialize(100).unwrap();
        assert_eq!(state.monotonic(99), Err(ClockError::TimebaseReversed));
        assert_eq!(state.monotonic(200), Err(ClockError::TimebaseReversed));
        assert_eq!(state.wall(300), Err(ClockError::TimebaseReversed));
        assert_eq!(
            state.snapshot().monotonic_error,
            Some(ClockError::TimebaseReversed)
        );
    }

    #[test]
    fn rtc_bias_and_explicit_local_offset_produce_unix_seconds() {
        let mut state = ClockState::new(60_750_000);
        state.initialize(50).unwrap();
        state.synchronize(100, Ok((86_400, 3_600))).unwrap();
        assert_eq!(state.wall(100), Err(ClockError::UtcOffsetMissing));
        state.set_utc_offset(3_600).unwrap();
        assert_eq!(state.wall(60_750_100), Ok(Duration::from_secs(946_771_201)));
        state.set_utc_offset(-18_000).unwrap();
        assert_eq!(
            state.wall(121_500_100),
            Ok(Duration::from_secs(946_792_802))
        );
        let before = state.snapshot();
        assert_eq!(
            state.set_utc_offset(86_400),
            Err(ClockError::InvalidUtcOffset)
        );
        assert_eq!(state.snapshot(), before);
    }

    #[test]
    fn recorded_context_failure_remains_visible_after_offset_changes() {
        let mut state = ClockState::new(60_750_000);
        state.initialize(0).unwrap();
        state.synchronize(100, Ok((100, 0))).unwrap();
        for error in [
            ClockError::UnsupportedHardware,
            ClockError::ExceptionContext,
            ClockError::InterruptsDisabled,
            ClockError::Busy,
        ] {
            state.record_wall_failure(error);
            state.set_utc_offset(0).unwrap();
            assert_eq!(state.wall(60_750_000), Err(error));
            assert_eq!(state.monotonic(60_750_000), Ok(Duration::from_secs(1)));
            state.synchronize(60_750_000, Ok((101, 0))).unwrap();
        }
    }

    #[test]
    fn real_read_or_bias_failure_invalidates_wall_time_and_can_be_retried() {
        let mut state = ClockState::new(60_750_000);
        state.initialize(0).unwrap();
        state.set_utc_offset(0).unwrap();
        state.synchronize(100, Ok((101, u32::MAX))).unwrap();
        assert_eq!(state.wall(100), Ok(Duration::from_secs(946_684_900)));
        for error in [
            ClockError::RtcFailure {
                stage: RtcStage::Read,
                cleanup_failures: 0,
            },
            ClockError::RtcUnstable,
            ClockError::CounterBias(-7),
        ] {
            assert_eq!(state.synchronize(200, Err(error)), Err(error));
            assert_eq!(state.wall(200), Err(error));
        }
        state.synchronize(300, Ok((102, 0))).unwrap();
        assert_eq!(state.wall(300), Ok(Duration::from_secs(946_684_902)));
    }
}
