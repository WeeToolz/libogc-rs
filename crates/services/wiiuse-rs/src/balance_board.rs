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
    /// Top-right load on Wiiuse's calibration scale (not clamped).
    pub top_right: f32,
    /// Bottom-right load on Wiiuse's calibration scale (not clamped).
    pub bottom_right: f32,
    /// Top-left load on Wiiuse's calibration scale (not clamped).
    pub top_left: f32,
    /// Bottom-left load on Wiiuse's calibration scale (not clamped).
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

    /// Return Wiiuse's right-minus-left and bottom-minus-top offsets.
    pub fn center_offsets(&self) -> [f32; 2] {
        [
            ((self.top_right + self.bottom_right) - (self.top_left + self.bottom_left)) / 2.0,
            ((self.bottom_left + self.bottom_right) - (self.top_left + self.top_right)) / 2.0,
        ]
    }
}

fn interpolate(raw: u16, calibration: [u16; 3]) -> f32 {
    // C wii_board_t stores the raw readings and calibration in signed shorts.
    // Widen before subtracting, just as C integer promotion does.
    let raw = i32::from(raw as i16);
    let [zero, middle, full] = calibration.map(|value| i32::from(value as i16));
    if zero >= middle || middle >= full {
        // Malformed calibration must not produce infinity or NaN.
        return 0.0;
    }
    if raw < middle {
        17.0 * (raw - zero) as f32 / (middle - zero) as f32
    } else {
        17.0 * (raw - middle) as f32 / (full - middle) as f32 + 17.0
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
        assert_eq!(report.bottom_left, 37.4);
    }

    #[test]
    fn valid_calibration_extrapolates_on_both_sides() {
        // Independent values from libogc calc_balanceboard_state, using a
        // 100-unit distance between the three calibration points.
        let calibration = BalanceBoardCalibration {
            top_right: [100, 200, 300],
            bottom_right: [100, 200, 300],
            top_left: [100, 200, 300],
            bottom_left: [100, 200, 300],
        };
        let report =
            BalanceBoardReport::decode(&[0, 50, 1, 94, 0, 100, 1, 44], &calibration).unwrap();
        assert_eq!(
            [
                report.top_right,
                report.bottom_right,
                report.top_left,
                report.bottom_left
            ],
            [-8.5, 42.5, 0.0, 34.0]
        );
        assert_eq!(report.center_offsets(), [0.0, 42.5]);
    }

    #[test]
    fn signed_sensor_values_match_c_and_invalid_calibration_is_finite() {
        assert_eq!(interpolate(0xffff, [0, 10, 20]), -1.7);
        assert_eq!(interpolate(20, [1, 1, 20]), 0.0);
        assert_eq!(interpolate(20, [1, 20, 20]), 0.0);
        assert_eq!(interpolate(20, [20, 10, 30]), 0.0);
        assert_eq!(interpolate(20, [0; 3]), 0.0);
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
