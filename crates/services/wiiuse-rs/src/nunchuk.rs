//! Nunchuk expansion report decoding.

/// Nunchuk accelerometer and joystick calibration values.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NunchukCalibration {
    /// Accelerometer readings at rest.
    pub accel_zero: [u16; 3],
    /// Accelerometer change from rest to one g.
    pub accel_g: [u16; 3],
    /// Joystick maximum values, X then Y.
    pub stick_max: [u8; 2],
    /// Joystick minimum values, X then Y.
    pub stick_min: [u8; 2],
    /// Joystick center values, X then Y.
    pub stick_center: [u8; 2],
}

impl NunchukCalibration {
    /// Decode the 224-byte Nunchuk expansion calibration read.
    pub fn decode(data: &[u8]) -> Option<Self> {
        let data = data.get(..224)?;
        if data.get(220..224)? != 0xa420_0000u32.to_be_bytes() {
            return None;
        }
        if Self::requires_retry(data) {
            return None;
        }
        let offset = if data[0] == 0xff && data[16] != 0xff {
            16
        } else {
            0
        };
        let block = data.get(offset..offset + 14)?;
        let packed_zero = block[3];
        let packed_g = block[7];
        let accel_zero = [
            (u16::from(block[0]) << 2) | u16::from((packed_zero >> 4) & 3),
            (u16::from(block[1]) << 2) | u16::from((packed_zero >> 2) & 3),
            (u16::from(block[2]) << 2) | u16::from(packed_zero & 3),
        ];
        let accel_one_g = [
            (u16::from(block[4]) << 2) | u16::from((packed_g >> 4) & 3),
            (u16::from(block[5]) << 2) | u16::from((packed_g >> 2) & 3),
            (u16::from(block[6]) << 2) | u16::from(packed_g & 3),
        ];
        let accel_g = core::array::from_fn(|axis| accel_one_g[axis].wrapping_sub(accel_zero[axis]));
        let accel_zero = with_fallback_u16(accel_zero, [499, 509, 507]);
        let accel_g = with_fallback_u16(accel_g, [703, 709, 709]);
        let stick_max = [block[8], block[11]];
        let stick_min = [block[9], block[12]];
        let stick_center = [block[10], block[13]];
        Some(Self {
            accel_zero,
            accel_g,
            stick_max: with_fallback_u8(stick_max, [223, 222]),
            stick_min: with_fallback_u8(stick_min, [27, 30]),
            stick_center: with_fallback_u8(stick_center, [126, 131]),
        })
    }

    /// Return true when the calibration read has the invalid repeated prefix
    /// that libogc Wiiuse handles by requesting the block again.
    pub fn requires_retry(data: &[u8]) -> bool {
        data.len() >= 17 && data[0] == 0xff && data[16] == 0xff
    }
}

fn with_fallback_u16<const N: usize>(values: [u16; N], fallback: [u16; N]) -> [u16; N] {
    core::array::from_fn(|axis| {
        if values[axis] == 0 {
            fallback[axis]
        } else {
            values[axis]
        }
    })
}

fn with_fallback_u8<const N: usize>(values: [u8; N], fallback: [u8; N]) -> [u8; N] {
    core::array::from_fn(|axis| {
        if values[axis] == 0 {
            fallback[axis]
        } else {
            values[axis]
        }
    })
}

/// Raw joystick, accelerometer, and button values from one six-byte report.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct NunchukReport {
    /// Joystick X byte.
    pub stick_x: u8,
    /// Joystick Y byte.
    pub stick_y: u8,
    /// Ten-bit accelerometer X value.
    pub accel_x: u16,
    /// Ten-bit accelerometer Y value.
    pub accel_y: u16,
    /// Ten-bit accelerometer Z value.
    pub accel_z: u16,
    /// Active-low button byte. Z and C are bits 0 and 1.
    pub buttons: u8,
}

impl NunchukReport {
    /// Decode a Nunchuk's six-byte expansion payload.
    pub fn decode(data: &[u8]) -> Option<Self> {
        let data = data.get(..6)?;
        let packed = data[5];
        Some(Self {
            stick_x: data[0],
            stick_y: data[1],
            accel_x: (u16::from(data[2]) << 2) | u16::from((packed >> 2) & 3),
            accel_y: (u16::from(data[3]) << 2) | u16::from((packed >> 4) & 3),
            accel_z: (u16::from(data[4]) << 2) | u16::from((packed >> 6) & 3),
            buttons: packed,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_stick_and_packed_ten_bit_accelerometer_values() {
        let report = NunchukReport::decode(&[0x2a, 0x35, 0x12, 0x34, 0x56, 0xc9]).unwrap();
        assert_eq!(report.stick_x, 0x2a);
        assert_eq!(report.stick_y, 0x35);
        assert_eq!(
            [report.accel_x, report.accel_y, report.accel_z],
            [74, 208, 347]
        );
        assert_eq!(report.buttons, 0xc9);
    }

    #[test]
    fn rejects_short_nunchuk_report() {
        assert!(NunchukReport::decode(&[0; 5]).is_none());
    }

    #[test]
    fn decodes_nunchuk_calibration_and_applies_wiiuse_defaults() {
        let mut data = [0; 224];
        data[..14].copy_from_slice(&[
            10, 20, 30, 0b10_01_11, 40, 50, 60, 0b01_10_11, 223, 27, 126, 222, 30, 131,
        ]);
        data[220..].copy_from_slice(&0xa420_0000u32.to_be_bytes());
        let calibration = NunchukCalibration::decode(&data).unwrap();
        assert_eq!(calibration.accel_zero, [42, 81, 123]);
        assert_eq!(calibration.accel_g, [119, 121, 120]);
        assert_eq!(calibration.stick_max, [223, 222]);
        assert_eq!(calibration.stick_min, [27, 30]);
        assert_eq!(calibration.stick_center, [126, 131]);

        data[..14].fill(0);
        let defaults = NunchukCalibration::decode(&data).unwrap();
        assert_eq!(defaults.accel_zero, [499, 509, 507]);
        assert_eq!(defaults.accel_g, [703, 709, 709]);
        assert_eq!(defaults.stick_max, [223, 222]);
        assert_eq!(defaults.stick_min, [27, 30]);
        assert_eq!(defaults.stick_center, [126, 131]);
    }

    #[test]
    fn handles_nunchuk_calibration_prefix_and_retry_marker() {
        let mut data = [0; 224];
        data[0] = 0xff;
        data[16..30].copy_from_slice(&[
            10, 20, 30, 0b10_01_11, 40, 50, 60, 0b01_10_11, 223, 27, 126, 222, 30, 131,
        ]);
        data[220..].copy_from_slice(&0xa420_0000u32.to_be_bytes());
        assert!(!NunchukCalibration::requires_retry(&data));
        assert_eq!(
            NunchukCalibration::decode(&data).unwrap().accel_zero,
            [42, 81, 123]
        );
        data[16] = 0xff;
        assert!(NunchukCalibration::requires_retry(&data));
        assert!(NunchukCalibration::decode(&data).is_none());
        assert!(NunchukCalibration::decode(&[0; 14]).is_none());
    }

    #[test]
    fn preserves_c_unsigned_calibration_subtraction() {
        let mut data = [0; 224];
        data[0] = 25; // zero X = 100
        data[4] = 12; // one-g X = 48
        data[220..].copy_from_slice(&0xa420_0000u32.to_be_bytes());
        let calibration = NunchukCalibration::decode(&data).unwrap();
        assert_eq!(calibration.accel_g[0], 65_484);
    }
}
