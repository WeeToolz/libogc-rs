//! Wii Remote status-report decoding.

const STATUS_REPORT_ID: u8 = 0x20;
const BATTERY_CRITICAL: u8 = 0x01;
const ATTACHMENT: u8 = 0x02;
const SPEAKER_ENABLED: u8 = 0x04;
const IR_ENABLED: u8 = 0x08;

/// State reported by a Wii Remote status packet.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RemoteStatusReport {
    /// The Remote reports a critical battery level.
    pub battery_critical: bool,
    /// An expansion device is attached.
    pub attachment: bool,
    /// The Remote speaker is enabled.
    pub speaker: bool,
    /// Infrared sensing is enabled.
    pub ir: bool,
    /// Raw battery level byte from the status report.
    pub battery_level: u8,
}

impl RemoteStatusReport {
    /// Decode a full `0x20` HID report using Wiiuse's payload offsets.
    pub fn decode(report: &[u8]) -> Option<Self> {
        if report.len() < 7 || report[0] != STATUS_REPORT_ID {
            return None;
        }
        // C's event handler receives the payload after the report ID. The
        // full Rust slice therefore has each C payload offset shifted by one.
        let flags = report[3];
        Some(Self {
            battery_critical: flags & BATTERY_CRITICAL != 0,
            attachment: flags & ATTACHMENT != 0,
            speaker: flags & SPEAKER_ENABLED != 0,
            ir: flags & IR_ENABLED != 0,
            battery_level: report[6],
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decodes_each_status_flag_and_battery_byte() {
        let status = RemoteStatusReport::decode(&[0x20, 0, 0, 0x0f, 0, 0, 0x81]).unwrap();
        assert!(status.battery_critical);
        assert!(status.attachment);
        assert!(status.speaker);
        assert!(status.ir);
        assert_eq!(status.battery_level, 0x81);
    }

    #[test]
    fn rejects_wrong_report_ids_and_short_reports() {
        assert!(RemoteStatusReport::decode(&[0x22, 0, 0, 0x0f, 0, 0, 0x81]).is_none());
        assert!(RemoteStatusReport::decode(&[0x20, 0, 0, 0x0f, 0, 0]).is_none());
    }

    #[test]
    fn absent_flags_remain_clear() {
        let status = RemoteStatusReport::decode(&[0x20, 0, 0, 0, 0, 0, 0x42]).unwrap();
        assert_eq!(
            status,
            RemoteStatusReport {
                battery_level: 0x42,
                ..Default::default()
            }
        );
    }
}
