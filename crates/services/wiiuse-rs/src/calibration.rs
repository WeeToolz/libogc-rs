//! Wii Remote accelerometer calibration decoding from Wiiuse.

const FALLBACK_ZERO: [u16; 3] = [499, 509, 507];
const FALLBACK_ONE_G: [u16; 3] = [703, 709, 709];

/// Decoded accelerometer zero and one-g calibration values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AccelCalibration {
    pub zero: [u16; 3],
    pub one_g: [u16; 3],
}

impl AccelCalibration {
    /// Return the existing WPAD values used when calibration data is absent.
    pub const fn fallback() -> Self {
        Self {
            zero: FALLBACK_ZERO,
            one_g: FALLBACK_ONE_G,
        }
    }

    /// Decode the eight-byte calibration block returned by a Wii Remote.
    ///
    /// A short block is rejected. Zero or malformed calibration values use the
    /// existing WPAD fallback values to avoid zero divisors in orientation code.
    pub fn decode(data: &[u8]) -> Option<Self> {
        let bytes: &[u8; 8] = data.get(..8)?.try_into().ok()?;
        let zero = [
            (u16::from(bytes[0]) << 2) | u16::from((bytes[3] >> 4) & 3),
            (u16::from(bytes[1]) << 2) | u16::from((bytes[3] >> 2) & 3),
            (u16::from(bytes[2]) << 2) | u16::from(bytes[3] & 3),
        ];
        let one_g = [
            ((u16::from(bytes[4]) << 2) | u16::from((bytes[7] >> 4) & 3)).saturating_sub(zero[0]),
            ((u16::from(bytes[5]) << 2) | u16::from((bytes[7] >> 2) & 3)).saturating_sub(zero[1]),
            ((u16::from(bytes[6]) << 2) | u16::from(bytes[7] & 3)).saturating_sub(zero[2]),
        ];

        Some(Self {
            zero: with_fallback(zero, FALLBACK_ZERO),
            one_g: with_fallback(one_g, FALLBACK_ONE_G),
        })
    }
}

fn with_fallback(values: [u16; 3], fallback: [u16; 3]) -> [u16; 3] {
    core::array::from_fn(|axis| {
        if values[axis] == 0 {
            fallback[axis]
        } else {
            values[axis]
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_low_bits_into_independent_zero_and_one_g_values() {
        let calibration =
            AccelCalibration::decode(&[10, 20, 30, 0b10_01_11, 40, 50, 60, 0b01_10_11])
                .expect("eight calibration bytes");
        assert_eq!(calibration.zero, [42, 81, 123]);
        assert_eq!(calibration.one_g, [119, 121, 120]);
    }

    #[test]
    fn zero_values_use_existing_wpad_defaults() {
        let calibration = AccelCalibration::decode(&[0; 8]).expect("eight calibration bytes");
        assert_eq!(calibration.zero, [499, 509, 507]);
        assert_eq!(calibration.one_g, [703, 709, 709]);
    }

    #[test]
    fn malformed_one_g_values_do_not_wrap_below_zero() {
        let calibration = AccelCalibration::decode(&[10, 20, 30, 0, 0, 0, 0, 0])
            .expect("eight calibration bytes");
        assert_eq!(calibration.one_g, [703, 709, 709]);
    }

    #[test]
    fn fallback_calibration_has_expected_values() {
        assert_eq!(
            AccelCalibration::fallback(),
            AccelCalibration {
                zero: [499, 509, 507],
                one_g: [703, 709, 709],
            }
        );
    }

    #[test]
    fn rejects_truncated_calibration_blocks() {
        for length in 0..8 {
            assert_eq!(AccelCalibration::decode(&[0; 8][..length]), None);
        }
    }
}
