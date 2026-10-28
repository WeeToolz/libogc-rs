//! Wii Balance Board calibration and load report decoding.

/// Three-point calibration values for the four Balance Board sensors.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct BalanceBoardCalibration {
    /// Top-right sensor calibration values.
    pub top_right: [u16; 3],
    /// Bottom-right sensor calibration values.
    pub bottom_right: [u16; 3],
    /// Top-left sensor calibration values.
    pub top_left: [u16; 3],
    /// Bottom-left sensor calibration values.
    pub bottom_left: [u16; 3],
}

impl BalanceBoardCalibration {
    /// Decode the calibration block read from expansion memory.
    ///
    /// Wiiuse starts at byte 4. It stores three calibration levels, with
    /// top-right, bottom-right, top-left, and bottom-left values at each level.
    pub fn decode(data: &[u8]) -> Option<Self> {
        if Self::requires_retry(data) {
            return None;
        }
        let offset = if data.first() == Some(&0xff) { 16 } else { 0 };
        let data = data.get(offset + 4..offset + 28)?;
        let mut values = [0u16; 12];
        for (value, bytes) in values.iter_mut().zip(data.as_chunks::<2>().0) {
            *value = u16::from_be_bytes(*bytes);
        }
        Some(Self {
            top_right: [values[0], values[4], values[8]],
            bottom_right: [values[1], values[5], values[9]],
            top_left: [values[2], values[6], values[10]],
            bottom_left: [values[3], values[7], values[11]],
        })
    }

    /// Return true when libogc Wiiuse requires another calibration read.
    pub fn requires_retry(data: &[u8]) -> bool {
        data.first() == Some(&0xff) && data.get(16) == Some(&0xff)
    }
}

/// Raw sensor values and calibrated loads from one Balance Board report.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct BalanceBoardReport {
    /// Top-right raw sensor value.
    pub raw_top_right: u16,
    /// Bottom-right raw sensor value.
    pub raw_bottom_right: u16,
    /// Top-left raw sensor value.
    pub raw_top_left: u16,
    /// Bottom-left raw sensor value.
    pub raw_bottom_left: u16,
    /// Top-right load on Wiiuse's 0 to 34 scale.
    pub top_right: f32,
    /// Bottom-right load on Wiiuse's 0 to 34 scale.
    pub bottom_right: f32,
    /// Top-left load on Wiiuse's 0 to 34 scale.
    pub top_left: f32,
    /// Bottom-left load on Wiiuse's 0 to 34 scale.
    pub bottom_left: f32,
}

impl BalanceBoardReport {
    /// Decode four big-endian sensor values and apply their calibration.
    pub fn decode(data: &[u8], calibration: &BalanceBoardCalibration) -> Option<Self> {
        let data = data.get(..8)?;
        let raw_top_right = u16::from_be_bytes([data[0], data[1]]);
        let raw_bottom_right = u16::from_be_bytes([data[2], data[3]]);
        let raw_top_left = u16::from_be_bytes([data[4], data[5]]);
        let raw_bottom_left = u16::from_be_bytes([data[6], data[7]]);
        Some(Self {
            raw_top_right,
            raw_bottom_right,
            raw_top_left,
            raw_bottom_left,
            top_right: interpolate(raw_top_right, calibration.top_right),
            bottom_right: interpolate(raw_bottom_right, calibration.bottom_right),
            top_left: interpolate(raw_top_left, calibration.top_left),
            bottom_left: interpolate(raw_bottom_left, calibration.bottom_left),
        })
    }
}

fn interpolate(raw: u16, calibration: [u16; 3]) -> f32 {
    const MIDDLE_CALIBRATION: f32 = 17.0;
    let [zero, middle, full] = calibration;
    if raw < zero {
        0.0
    } else if raw < middle {
        (f32::from(raw - zero) * MIDDLE_CALIBRATION) / f32::from(middle - zero)
    } else if raw < full {
        (f32::from(raw - middle) * MIDDLE_CALIBRATION) / f32::from(full - middle)
            + MIDDLE_CALIBRATION
    } else {
        MIDDLE_CALIBRATION * 2.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_calibration_and_interpolates_each_sensor() {
        let mut calibration_data = [0u8; 28];
        let values = [
            100u16, 110, 120, 130, // zero calibration, TR BR TL BL
            200, 210, 220, 230, // middle calibration, TR BR TL BL
            300, 310, 320, 330, // full calibration, TR BR TL BL
        ];
        for (index, value) in values.into_iter().enumerate() {
            let offset = 4 + index * 2;
            calibration_data[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
        }

        let calibration = BalanceBoardCalibration::decode(&calibration_data).unwrap();
        assert_eq!(calibration.top_right, [100, 200, 300]);
        assert_eq!(calibration.bottom_right, [110, 210, 310]);
        assert_eq!(calibration.top_left, [120, 220, 320]);
        assert_eq!(calibration.bottom_left, [130, 230, 330]);

        let readings = [
            150u16, // halfway to the middle point
            260,    // halfway between middle and full
            220,    // middle point
            350,    // at or above full
        ];
        let mut report_data = [0u8; 8];
        for (index, value) in readings.into_iter().enumerate() {
            report_data[index * 2..index * 2 + 2].copy_from_slice(&value.to_be_bytes());
        }
        let report = BalanceBoardReport::decode(&report_data, &calibration).unwrap();
        assert_eq!(report.raw_top_right, 150);
        assert_eq!(report.raw_bottom_right, 260);
        assert_eq!(report.raw_top_left, 220);
        assert_eq!(report.raw_bottom_left, 350);
        assert_eq!(report.top_right, 8.5);
        assert_eq!(report.bottom_right, 25.5);
        assert_eq!(report.top_left, 17.0);
        assert_eq!(report.bottom_left, 34.0);
    }

    #[test]
    fn rejects_truncated_calibration_and_sensor_reports() {
        assert!(BalanceBoardCalibration::decode(&[0; 27]).is_none());
        assert!(BalanceBoardReport::decode(&[0; 7], &BalanceBoardCalibration::default()).is_none());
    }

    #[test]
    fn skips_repeated_calibration_prefix_and_flags_retry_for_repeated_invalid_blocks() {
        let values = [
            100u16, 110, 120, 130, 200, 210, 220, 230, 300, 310, 320, 330,
        ];
        let mut offset_data = [0u8; 44];
        offset_data[0] = 0xff;
        for (index, value) in values.into_iter().enumerate() {
            let offset = 16 + 4 + index * 2;
            offset_data[offset..offset + 2].copy_from_slice(&value.to_be_bytes());
        }
        let calibration = BalanceBoardCalibration::decode(&offset_data).unwrap();
        assert_eq!(calibration.top_right, [100, 200, 300]);
        assert!(!BalanceBoardCalibration::requires_retry(&offset_data));

        let mut invalid_data = [0u8; 224];
        invalid_data[0] = 0xff;
        invalid_data[16] = 0xff;
        assert!(BalanceBoardCalibration::requires_retry(&invalid_data));
        assert!(BalanceBoardCalibration::decode(&invalid_data).is_none());
    }
}
