use core::fmt::{self, Write};
use ogc_controller::ControllerSource;

const EVENT_LINE_CAPACITY: usize = 1300;

pub fn source_text(source: ControllerSource) -> &'static str {
    match source {
        ControllerSource::GameCube(0) => "gc0",
        ControllerSource::GameCube(1) => "gc1",
        ControllerSource::GameCube(2) => "gc2",
        ControllerSource::GameCube(3) => "gc3",
        ControllerSource::WiiBluetooth(0) => "wpad0",
        ControllerSource::WiiBluetooth(1) => "wpad1",
        ControllerSource::WiiBluetooth(2) => "wpad2",
        ControllerSource::WiiBluetooth(3) => "wpad3",
        ControllerSource::UsbGameCube { .. } => "usb_gamecube",
        ControllerSource::UsbHid(0) => "usb0",
        ControllerSource::UsbHid(1) => "usb1",
        ControllerSource::UsbHid(2) => "usb2",
        ControllerSource::UsbHid(3) => "usb3",
        ControllerSource::BluetoothHid(0) => "bt0",
        ControllerSource::BluetoothHid(1) => "bt1",
        ControllerSource::BluetoothHid(2) => "bt2",
        ControllerSource::BluetoothHid(3) => "bt3",
        ControllerSource::BluetoothHidPair(_, _) => "bt_pair",
        ControllerSource::Unknown
        | ControllerSource::GameCube(_)
        | ControllerSource::WiiBluetooth(_)
        | ControllerSource::UsbHid(_)
        | ControllerSource::BluetoothHid(_) => "unknown",
    }
}

pub fn source_slots(source: ControllerSource) -> (u8, u8) {
    match source {
        ControllerSource::GameCube(slot)
        | ControllerSource::WiiBluetooth(slot)
        | ControllerSource::UsbHid(slot)
        | ControllerSource::BluetoothHid(slot) => (slot, u8::MAX),
        ControllerSource::UsbGameCube { adapter_slot, port } => {
            (adapter_slot.saturating_mul(4).saturating_add(port), port)
        }
        ControllerSource::BluetoothHidPair(left, right) => (left, right),
        ControllerSource::Unknown => (u8::MAX, u8::MAX),
    }
}

/// Fixed-capacity formatter for one JSON Lines event.
///
/// Formatting errors are sticky. Call [`as_str`](Self::as_str) before output;
/// it rejects the whole line if an intermediate formatting result failed.
pub struct EventLineBuf {
    bytes: [u8; EVENT_LINE_CAPACITY],
    len: usize,
    failed: bool,
}

impl EventLineBuf {
    pub const fn new() -> Self {
        Self {
            bytes: [0; EVENT_LINE_CAPACITY],
            len: 0,
            failed: false,
        }
    }

    pub fn as_str(&self) -> Result<&str, fmt::Error> {
        if self.failed {
            return Err(fmt::Error);
        }
        core::str::from_utf8(&self.bytes[..self.len]).map_err(|_| fmt::Error)
    }

    /// Write a quoted JSON string and escape every character JSON forbids.
    pub fn write_json_string(&mut self, value: &str) -> fmt::Result {
        self.write_char('"')?;
        for character in value.chars() {
            match character {
                '"' => self.write_str("\\\"")?,
                '\\' => self.write_str("\\\\")?,
                '\u{08}' => self.write_str("\\b")?,
                '\u{0c}' => self.write_str("\\f")?,
                '\n' => self.write_str("\\n")?,
                '\r' => self.write_str("\\r")?,
                '\t' => self.write_str("\\t")?,
                character if character <= '\u{1f}' => {
                    write!(self, "\\u{:04x}", character as u32)?;
                }
                character => self.write_char(character)?,
            }
        }
        self.write_char('"')
    }

    pub fn write_hex_bytes(&mut self, bytes: &[u8]) -> fmt::Result {
        for byte in bytes {
            write!(self, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl Default for EventLineBuf {
    fn default() -> Self {
        Self::new()
    }
}

impl Write for EventLineBuf {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let Some(end) = self.len.checked_add(value.len()) else {
            self.failed = true;
            return Err(fmt::Error);
        };
        if end > self.bytes.len() {
            self.failed = true;
            return Err(fmt::Error);
        }
        self.bytes[self.len..end].copy_from_slice(value.as_bytes());
        self.len = end;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::EventLineBuf;
    use core::fmt::Write;

    #[test]
    fn json_string_escapes_quotes_slashes_and_control_bytes() {
        let mut line = EventLineBuf::new();
        line.write_json_string("quote \" slash \\ newline\n nul\0 tab\t café")
            .unwrap();
        assert_eq!(
            line.as_str(),
            Ok(r#""quote \" slash \\ newline\n nul\u0000 tab\t café""#)
        );
    }

    #[test]
    fn overflow_is_reported_and_partial_json_cannot_be_read() {
        let mut line = EventLineBuf::new();
        let oversized = "x".repeat(1301);
        assert!(line.write_str(&oversized).is_err());
        assert!(line.as_str().is_err());

        let mut escaped = EventLineBuf::new();
        let control_chars = "\n".repeat(700);
        assert!(escaped.write_json_string(&control_chars).is_err());
        assert!(escaped.as_str().is_err());
    }

    #[test]
    fn exact_capacity_line_is_preserved() {
        let mut line = EventLineBuf::new();
        let exact = "x".repeat(1300);
        line.write_str(&exact).unwrap();
        assert_eq!(line.as_str().unwrap().len(), 1300);
    }

    #[test]
    fn maximum_sized_usb_raw_event_fits_and_has_complete_json_shape() {
        let mut line = EventLineBuf::new();
        write!(
            &mut line,
            "{{\"type\":\"raw_report\",\"source\":\"usb3\",\"slot_a\":3,\"slot_b\":255,\"generation\":4294967295,\"generation_valid\":true,\"timestamp_ticks\":18446744073709551615,\"device_id\":2147483647,\"vid\":65535,\"pid\":65535,\"usb_identity_valid\":true,\"data\":\""
        )
        .unwrap();
        line.write_hex_bytes(&[0xff; 512]).unwrap();
        line.write_str("\"}").unwrap();

        let rendered = line.as_str().unwrap();
        let expected_hex = "ff".repeat(512);
        assert!(rendered.starts_with(
            r#"{"type":"raw_report","source":"usb3","slot_a":3,"slot_b":255,"generation":4294967295,"generation_valid":true,"timestamp_ticks":18446744073709551615,"device_id":2147483647,"vid":65535,"pid":65535,"usb_identity_valid":true,"data":""#
        ));
        assert!(rendered.ends_with(&format!("{expected_hex}\"}}")));
        assert!(rendered.len() <= 1300);
    }
}
