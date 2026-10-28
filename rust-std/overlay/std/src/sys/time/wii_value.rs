//! Clock arithmetic also tested directly on the host. No hardware fallback.
use core::time::Duration;

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct Instant(pub(super) Duration);

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Debug, Hash)]
pub struct SystemTime(pub(super) i128);

const MAX_NANOS: i128 = Duration::MAX.as_nanos() as i128;
pub const UNIX_EPOCH: SystemTime = SystemTime(0);

impl Instant {
    pub fn checked_sub_instant(&self, other: &Instant) -> Option<Duration> {
        self.0.checked_sub(other.0)
    }
    pub fn checked_add_duration(&self, duration: &Duration) -> Option<Self> {
        self.0.checked_add(*duration).map(Self)
    }
    pub fn checked_sub_duration(&self, duration: &Duration) -> Option<Self> {
        self.0.checked_sub(*duration).map(Self)
    }
}

fn duration(nanos: u128) -> Duration {
    Duration::new(
        (nanos / 1_000_000_000) as u64,
        (nanos % 1_000_000_000) as u32,
    )
}

impl SystemTime {
    // Bound the full span so every difference fits Rust Duration.
    pub const MIN: Self = Self(-(MAX_NANOS / 2));
    pub const MAX: Self = Self(MAX_NANOS / 2);

    pub(super) fn from_duration(value: Duration) -> Option<Self> {
        Self::checked_value(value.as_nanos() as i128)
    }
    fn checked_value(nanos: i128) -> Option<Self> {
        (Self::MIN.0..=Self::MAX.0)
            .contains(&nanos)
            .then_some(Self(nanos))
    }
    pub fn sub_time(&self, other: &Self) -> Result<Duration, Duration> {
        let difference = self.0 - other.0;
        if difference < 0 {
            Err(duration(difference.unsigned_abs()))
        } else {
            Ok(duration(difference as u128))
        }
    }
    pub fn checked_add_duration(&self, value: &Duration) -> Option<Self> {
        Self::checked_value(self.0.checked_add(value.as_nanos() as i128)?)
    }
    pub fn checked_sub_duration(&self, value: &Duration) -> Option<Self> {
        Self::checked_value(self.0.checked_sub(value.as_nanos() as i128)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instant_checked_arithmetic_preserves_nanoseconds_and_bounds() {
        let instant = Instant(Duration::new(7, 900_000_000));
        let later = instant
            .checked_add_duration(&Duration::from_millis(200))
            .unwrap();
        assert_eq!(later.0, Duration::new(8, 100_000_000));
        assert_eq!(
            later.checked_sub_instant(&instant),
            Some(Duration::from_millis(200))
        );
        assert_eq!(instant.checked_sub_instant(&later), None);
        assert_eq!(
            Instant(Duration::ZERO).checked_sub_duration(&Duration::from_nanos(1)),
            None
        );
        assert_eq!(
            Instant(Duration::MAX).checked_add_duration(&Duration::from_nanos(1)),
            None
        );
    }

    #[test]
    fn wall_arithmetic_can_cross_the_unix_epoch_in_both_directions() {
        let earlier = UNIX_EPOCH
            .checked_sub_duration(&Duration::from_nanos(1))
            .unwrap();
        assert_eq!(earlier.sub_time(&UNIX_EPOCH), Err(Duration::from_nanos(1)));
        assert_eq!(UNIX_EPOCH.sub_time(&earlier), Ok(Duration::from_nanos(1)));
        assert_eq!(
            earlier.checked_add_duration(&Duration::from_nanos(1)),
            Some(UNIX_EPOCH)
        );
        let real = SystemTime::from_duration(Duration::new(946_684_800, 123)).unwrap();
        assert_eq!(
            real.sub_time(&UNIX_EPOCH),
            Ok(Duration::new(946_684_800, 123))
        );
    }

    #[test]
    fn extreme_wall_arithmetic_is_checked_and_differences_fit_duration() {
        assert_eq!(
            SystemTime::MAX.checked_add_duration(&Duration::from_nanos(1)),
            None
        );
        assert_eq!(
            SystemTime::MIN.checked_sub_duration(&Duration::from_nanos(1)),
            None
        );
        assert_eq!(
            SystemTime::MAX.sub_time(&SystemTime::MIN),
            Ok(Duration::new(u64::MAX, 999_999_998))
        );
        assert_eq!(SystemTime::from_duration(Duration::MAX), None);
        assert_eq!(UNIX_EPOCH.checked_add_duration(&Duration::MAX), None);
    }
}
