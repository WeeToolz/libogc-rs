//! Allocation-free report protocol shared by applications and the development runtime.
#![no_std]

#[cfg(test)]
extern crate std;
use core::fmt;

pub const LOG_CAPACITY: usize = 16 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity {
    pub host: [u8; 4],
    pub port: u16,
    pub run: [u8; 64],
    pub run_len: usize,
    pub build: [u8; 64],
    pub deadline_ms: u32,
}

pub fn parse(args: &[&str]) -> Result<Identity, &'static str> {
    let mut endpoint = None;
    let mut run = None;
    let mut build = None;
    let mut deadline = None;
    for arg in args {
        if let Some(v) = arg.strip_prefix("--report=") {
            if endpoint.replace(v).is_some() {
                return Err("duplicate report endpoint");
            }
        } else if let Some(v) = arg.strip_prefix("--run-id=") {
            if run.replace(v).is_some() {
                return Err("duplicate run identity");
            }
        } else if let Some(v) = arg.strip_prefix("--build-id=") {
            if build.replace(v).is_some() {
                return Err("duplicate build identity");
            }
        } else if let Some(v) = arg.strip_prefix("--recovery-reset-ms=")
            && deadline.replace(v).is_some()
        {
            return Err("duplicate recovery deadline");
        }
    }
    let (address, port) = parse_report_endpoint(endpoint.ok_or("missing report endpoint")?)?;
    if address[0] == 0 || address[0] >= 224 {
        return Err("invalid report address");
    }
    let run = run.ok_or("missing run identity")?;
    if run.is_empty()
        || run.len() > 64
        || !run
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.".contains(&b))
    {
        return Err("invalid run identity");
    }
    let build = build.ok_or("missing build identity")?;
    if build.len() != 64 || !build.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err("invalid build identity");
    }
    let deadline_ms = parse_recovery_deadline(deadline.ok_or("missing recovery deadline")?)?;
    let mut identity = Identity {
        host: address,
        port,
        run: [0; 64],
        run_len: run.len(),
        build: [0; 64],
        deadline_ms,
    };
    identity.run[..run.len()].copy_from_slice(run.as_bytes());
    identity.build.copy_from_slice(build.as_bytes());
    Ok(identity)
}

/// Parse an IPv4 report endpoint. Address policy belongs to the caller.
pub fn parse_report_endpoint(value: &str) -> Result<([u8; 4], u16), &'static str> {
    let (host, port) = value.split_once(':').ok_or("invalid endpoint")?;
    let mut octets = host.split('.');
    let mut address = [0; 4];
    for octet in &mut address {
        *octet = octets
            .next()
            .ok_or("invalid IPv4 address")?
            .parse()
            .map_err(|_| "invalid IPv4 address")?;
    }
    if octets.next().is_some() {
        return Err("invalid IPv4 address");
    }
    let port = port.parse::<u16>().map_err(|_| "invalid report port")?;
    if port == 0 {
        return Err("invalid report port");
    }
    Ok((address, port))
}

/// Parse the supported positive recovery deadline, in milliseconds.
pub fn parse_recovery_deadline(value: &str) -> Result<u32, &'static str> {
    let milliseconds = value
        .parse::<u32>()
        .map_err(|_| "invalid recovery deadline")?;
    if !(2000..=i32::MAX as u32).contains(&milliseconds) {
        return Err("invalid recovery deadline");
    }
    Ok(milliseconds)
}

/// WIP1 fields. Callers validate payload bounds before encoding the header.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProgressHeader {
    pub sequence: u32,
    pub stage: u32,
    pub run_length: u16,
    pub build_length: u16,
    pub text_length: u32,
    pub truncated: bool,
}

impl ProgressHeader {
    pub fn encode(self) -> [u8; 24] {
        let mut out = [0; 24];
        out[..4].copy_from_slice(b"WIP1");
        out[4..6].copy_from_slice(&1u16.to_be_bytes());
        out[6..8].copy_from_slice(&u16::from(self.truncated).to_be_bytes());
        out[8..12].copy_from_slice(&self.sequence.to_be_bytes());
        out[12..16].copy_from_slice(&self.stage.to_be_bytes());
        out[16..18].copy_from_slice(&self.run_length.to_be_bytes());
        out[18..20].copy_from_slice(&self.build_length.to_be_bytes());
        out[20..24].copy_from_slice(&self.text_length.to_be_bytes());
        out
    }
}

pub fn progress_header(identity: &Identity, sequence: u32, length: usize) -> [u8; 24] {
    ProgressHeader {
        sequence,
        stage: 0,
        run_length: identity.run_len as u16,
        build_length: 64,
        text_length: length as u32,
        truncated: false,
    }
    .encode()
}

pub fn final_header(identity: &Identity, length: usize, truncated: bool) -> [u8; 156] {
    let mut out = [0; 156];
    out[..4].copy_from_slice(b"WIR2");
    out[4..6].copy_from_slice(&2u16.to_be_bytes());
    out[6..8].copy_from_slice(&u16::from(truncated).to_be_bytes());
    out[8..12].copy_from_slice(&(length as u32).to_be_bytes());
    out[24..28].copy_from_slice(&36u32.to_be_bytes());
    out[28..28 + identity.run_len].copy_from_slice(&identity.run[..identity.run_len]);
    out[92..156].copy_from_slice(&identity.build);
    out
}

/// Write the complete final report through the same ordered path used by the
/// development runtime. The callback must return its real transport error.
pub fn write_final_report<E>(
    identity: &Identity,
    log: &[u8],
    log_lost: usize,
    log_contention: usize,
    application_status: i32,
    mut write: impl FnMut(&[u8]) -> Result<(), E>,
) -> Result<(), E> {
    let mut tail = LogBuffer::<128>::new();
    append(&mut tail, b"INFO dev log_lost_bytes=");
    append_decimal(&mut tail, log_lost);
    append(&mut tail, b" log_contention=");
    append_decimal(&mut tail, log_contention);
    append(&mut tail, b"\n");
    let success = application_status == 0 && log_lost == 0 && log_contention == 0;
    let summary = if success {
        b"PASS application main returned zero\nPASS summary passes=1 fails=0\n".as_slice()
    } else {
        b"FAIL application return or development log loss\nFAIL summary passes=0 fails=1\n"
            .as_slice()
    };
    let header = final_header(
        identity,
        log.len() + tail.len + summary.len(),
        log_lost != 0,
    );
    write(&header)?;
    write(log)?;
    write(&tail.bytes[..tail.len])?;
    write(summary)?;
    write(&[0; 36])
}

fn append<const N: usize>(buffer: &mut LogBuffer<N>, bytes: &[u8]) {
    let end = buffer.len + bytes.len();
    buffer.bytes[buffer.len..end].copy_from_slice(bytes);
    buffer.len = end;
}

fn append_decimal<const N: usize>(buffer: &mut LogBuffer<N>, mut value: usize) {
    let mut digits = [0; 20];
    let mut start = digits.len();
    loop {
        start -= 1;
        digits[start] = b'0' + (value % 10) as u8;
        value /= 10;
        if value == 0 {
            break;
        }
    }
    append(buffer, &digits[start..]);
}

pub struct LogBuffer<const N: usize = LOG_CAPACITY> {
    pub bytes: [u8; N],
    pub len: usize,
    pub lost: usize,
}
impl<const N: usize> Default for LogBuffer<N> {
    fn default() -> Self {
        Self::new()
    }
}
impl<const N: usize> LogBuffer<N> {
    pub const fn new() -> Self {
        Self {
            bytes: [0; N],
            len: 0,
            lost: 0,
        }
    }
}
impl<const N: usize> fmt::Write for LogBuffer<N> {
    fn write_str(&mut self, value: &str) -> fmt::Result {
        let count = value.len().min(self.bytes.len() - self.len);
        self.bytes[self.len..self.len + count].copy_from_slice(&value.as_bytes()[..count]);
        self.len += count;
        self.lost = self.lost.saturating_add(value.len() - count);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::fmt::Write;
    #[test]
    fn endpoints_preserve_caller_address_policy_and_reject_malformed_input() {
        assert_eq!(
            parse_report_endpoint("192.168.2.1:4298"),
            Ok(([192, 168, 2, 1], 4298))
        );
        assert_eq!(parse_report_endpoint("0.0.0.0:1"), Ok(([0, 0, 0, 0], 1)));
        for invalid in [
            "1.2.3:4298",
            "1.2.3.4.5:4298",
            "256.2.3.4:4298",
            "1.2.3.4:0",
            "1.2.3.4:65536",
            "1.2.3.4",
            "host:4298",
        ] {
            assert!(parse_report_endpoint(invalid).is_err(), "{invalid}");
        }
    }
    #[test]
    fn deadline_boundaries_are_checked() {
        assert_eq!(parse_recovery_deadline("2000"), Ok(2000));
        assert_eq!(parse_recovery_deadline("2147483647"), Ok(2147483647));
        for invalid in ["0", "1999", "-1", "2147483648", "", "abc"] {
            assert!(parse_recovery_deadline(invalid).is_err(), "{invalid}");
        }
    }
    #[test]
    fn application_progress_preserves_stage_and_truncation() {
        let bytes = ProgressHeader {
            sequence: 3,
            stage: 8,
            run_length: 11,
            build_length: 64,
            text_length: 256,
            truncated: true,
        }
        .encode();
        assert_eq!(
            bytes,
            [
                b'W', b'I', b'P', b'1', 0, 1, 0, 1, 0, 0, 0, 3, 0, 0, 0, 8, 0, 11, 0, 64, 0, 0, 1,
                0,
            ]
        );
    }
    fn identity() -> Identity {
        parse(&[
            "--report=192.168.2.1:4298",
            "--run-id=dev-1",
            "--build-id=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
            "--recovery-reset-ms=30000",
        ])
        .unwrap()
    }
    #[test]
    fn validates_identity_and_requires_an_enabled_deadline() {
        let id = identity();
        assert_eq!(id.host, [192, 168, 2, 1]);
        assert_eq!(id.port, 4298);
        assert_eq!(id.deadline_ms, 30000);
        for invalid in ["0", "-1", "1999", "2147483648"] {
            let option = std::format!("--recovery-reset-ms={invalid}");
            assert!(
                parse(&[
                    "--report=192.168.2.1:4298",
                    "--run-id=x",
                    "--build-id=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
                    &option
                ])
                .is_err()
            );
        }
        assert_eq!(parse(&[]), Err("missing report endpoint"));
        assert_eq!(
            parse(&["--report=x", "--report=y"]),
            Err("duplicate report endpoint")
        );
    }
    #[test]
    fn wire_headers_match_independent_protocol_bytes() {
        let id = identity();
        let p = progress_header(&id, 7, 13);
        assert_eq!(&p[..12], b"WIP1\0\x01\0\0\0\0\0\x07");
        assert_eq!(&p[12..], b"\0\0\0\0\0\x05\0\x40\0\0\0\x0d");
        let f = final_header(&id, 21, false);
        assert_eq!(
            &f[..28],
            b"WIR2\0\x02\0\0\0\0\0\x15\0\0\0\0\0\0\0\0\0\0\0\0\0\0\0\x24"
        );
        assert_eq!(&f[28..33], b"dev-1");
        assert_eq!(&f[92..], &id.build);
    }
    #[test]
    fn final_report_writes_exact_sections_and_preserves_application_failure() {
        let id = identity();
        let mut sections = std::vec::Vec::new();
        write_final_report(&id, b"FAIL app result=-7\n", 3, 2, -7, |part| {
            sections.push(part.to_vec());
            Ok::<_, ()>(())
        })
        .unwrap();
        assert_eq!(sections.len(), 5);
        assert_eq!(&sections[0][..8], b"WIR2\0\x02\0\x01");
        let payload_len = u32::from_be_bytes(sections[0][8..12].try_into().unwrap()) as usize;
        assert_eq!(
            payload_len,
            sections[1..4].iter().map(std::vec::Vec::len).sum()
        );
        assert_eq!(sections[1], b"FAIL app result=-7\n");
        assert_eq!(sections[2], b"INFO dev log_lost_bytes=3 log_contention=2\n");
        assert_eq!(
            sections[3],
            b"FAIL application return or development log loss\nFAIL summary passes=0 fails=1\n"
        );
        assert_eq!(sections[4], [0; 36]);
    }
    #[test]
    fn final_report_marks_success_only_for_zero_status_without_log_loss() {
        let id = identity();
        for (lost, contention, status, expected) in [
            (
                0,
                0,
                0,
                b"PASS application main returned zero\nPASS summary passes=1 fails=0\n".as_slice(),
            ),
            (
                0,
                0,
                -7,
                b"FAIL application return or development log loss\nFAIL summary passes=0 fails=1\n"
                    .as_slice(),
            ),
            (
                1,
                0,
                0,
                b"FAIL application return or development log loss\nFAIL summary passes=0 fails=1\n"
                    .as_slice(),
            ),
            (
                0,
                1,
                0,
                b"FAIL application return or development log loss\nFAIL summary passes=0 fails=1\n"
                    .as_slice(),
            ),
        ] {
            let mut sections = std::vec::Vec::new();
            write_final_report(&id, b"log\n", lost, contention, status, |part| {
                sections.push(part.to_vec());
                Ok::<_, ()>(())
            })
            .unwrap();
            assert_eq!(sections[3], expected);
        }
    }
    #[test]
    fn final_report_returns_failure_from_each_transport_write() {
        let id = identity();
        for failure_index in 0..5 {
            let mut index = 0;
            let result = write_final_report(&id, b"log", 0, 0, 0, |_| {
                let current = index;
                index += 1;
                if current == failure_index {
                    Err(current)
                } else {
                    Ok(())
                }
            });
            assert_eq!(result, Err(failure_index));
            assert_eq!(index, failure_index + 1);
        }
    }
    #[test]
    fn log_preserves_real_bytes_and_counts_capacity_loss() {
        let mut log = LogBuffer::<LOG_CAPACITY>::new();
        log.write_str("actual application log\n").unwrap();
        assert_eq!(&log.bytes[..log.len], b"actual application log\n");
        let text = std::string::String::from_utf8(std::vec![b'x';LOG_CAPACITY]).unwrap();
        log.write_str(&text).unwrap();
        assert_eq!(log.len, LOG_CAPACITY);
        assert_eq!(log.lost, 23);
    }
}
