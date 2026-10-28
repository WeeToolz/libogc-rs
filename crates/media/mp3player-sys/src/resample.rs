const ONSET_MIN_LEVEL: u32 = 512;
const ONSET_RISE_NUMERATOR: u64 = 3;
const ONSET_RISE_DENOMINATOR: u64 = 2;

pub(super) fn is_note_onset(previous_level: u32, current_level: u32) -> bool {
    if current_level < ONSET_MIN_LEVEL {
        return false;
    }

    previous_level < ONSET_MIN_LEVEL
        || (u64::from(current_level) * ONSET_RISE_DENOMINATOR
            >= u64::from(previous_level) * ONSET_RISE_NUMERATOR)
}

pub(super) fn interpolate_sample(first: i16, second: i16, fraction: u32) -> i16 {
    let delta = i64::from(second) - i64::from(first);
    (i64::from(first) + ((delta * i64::from(fraction)) >> 16)) as i16
}

#[cfg(test)]
mod tests {
    use super::{interpolate_sample, is_note_onset};

    #[test]
    fn note_onset_requires_a_clear_level_rise() {
        assert!(!is_note_onset(2_000, 2_800));
        assert!(is_note_onset(2_000, 3_000));
        assert!(is_note_onset(0, 512));
        assert!(!is_note_onset(0, 511));
    }

    #[test]
    fn interpolation_blends_positive_and_negative_samples() {
        assert_eq!(interpolate_sample(0, 10_000, 32_768), 5_000);
        assert_eq!(interpolate_sample(-10_000, 10_000, 32_768), 0);
        assert_eq!(interpolate_sample(10_000, -10_000, 0), 10_000);
    }
}
