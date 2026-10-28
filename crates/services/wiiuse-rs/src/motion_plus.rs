//! Motion Plus report decoding.

/// Decoded gyroscope axes and flags from one six-byte Motion Plus report.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct MotionPlusReport {
    /// Pitch axis reading.
    pub rx: i16,
    /// Roll axis reading.
    pub ry: i16,
    /// Yaw axis reading.
    pub rz: i16,
    /// Expansion data flag from bit 0 of byte 4.
    pub ext: u8,
    /// Roll, yaw, and pitch speed flags from the report.
    pub status: u8,
}

impl MotionPlusReport {
    /// Decode a Motion Plus expansion payload.
    pub fn decode(data: &[u8]) -> Option<Self> {
        let data = data.get(..6)?;
        Some(Self {
            rx: (i16::from(data[5] & 0xfc) << 6) | i16::from(data[2]),
            ry: (i16::from(data[4] & 0xfc) << 6) | i16::from(data[1]),
            rz: (i16::from(data[3] & 0xfc) << 6) | i16::from(data[0]),
            ext: data[4] & 0x01,
            status: (data[3] & 0x03) | ((data[4] & 0x02) << 1),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_axis_words_and_status_bits() {
        let report = MotionPlusReport::decode(&[0x12, 0x34, 0x56, 0xab, 0xcf, 0xef]).unwrap();
        assert_eq!(report.rx, 0x3b56);
        assert_eq!(report.ry, 0x3334);
        assert_eq!(report.rz, 0x2a12);
        assert_eq!(report.ext, 1);
        assert_eq!(report.status, 7);
    }

    #[test]
    fn rejects_short_reports() {
        for length in 0..6 {
            assert!(MotionPlusReport::decode(&[0; 6][..length]).is_none());
        }
    }
}
