//! Allocation-free report protocol shared by applications and the development runtime.
#![no_std]

#[cfg(test)]
extern crate std;
use core::fmt;

pub const LOG_CAPACITY: usize = 16 * 1024;

/// Report identity validated by [`parse`]. Direct construction must preserve
/// its run length, ASCII identity fields, endpoint and deadline invariants.
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

/// Encode a development progress record. Use a validated identity and a
/// payload length that fits in `u32`.
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

/// Encode a development final report without a framebuffer. Use a validated
/// identity and a payload length that fits in `u32`.
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
/// development runtime. The callback must write each complete slice or return
/// its real transport error. Use a validated identity and a log whose length
/// plus the generated text fits in `u32`.
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

/// Bounded byte log. Keep `len <= N` when accessing the public fields directly.
/// Truncation is measured in bytes and can split a UTF-8 character.
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
    fn identity_rejects_invalid_and_duplicate_fields() {
        let endpoint = "--report=192.168.2.1:4298";
        let run = "--run-id=dev-1";
        let build = "--build-id=0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let deadline = "--recovery-reset-ms=30000";
        for (args, error) in [
            (
                std::vec![endpoint, "--run-id=", build, deadline],
                "invalid run identity",
            ),
            (
                std::vec![endpoint, "--run-id=bad/id", build, deadline],
                "invalid run identity",
            ),
            (
                std::vec![endpoint, run, "--build-id=abcd", deadline],
                "invalid build identity",
            ),
            (
                std::vec!["--report=224.1.2.3:4298", run, build, deadline],
                "invalid report address",
            ),
            (
                std::vec![endpoint, run, run, build, deadline],
                "duplicate run identity",
            ),
            (
                std::vec![endpoint, run, build, build, deadline],
                "duplicate build identity",
            ),
            (
                std::vec![endpoint, run, build, deadline, deadline],
                "duplicate recovery deadline",
            ),
            (std::vec![endpoint, run, build], "missing recovery deadline"),
        ] {
            assert_eq!(parse(&args), Err(error));
        }
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

    #[test]
    fn identity_field_boundaries_and_missing_fields() {
        let endpoint = "--report=223.255.255.255:65535";
        let build = "--build-id=ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789ABCDEF0123456789";
        let deadline = "--recovery-reset-ms=2147483647";
        let longest = std::format!("--run-id={}", "a".repeat(64));
        let id = parse(&[deadline, "--unrelated", &longest, build, endpoint]).unwrap();
        assert_eq!(id.host, [223, 255, 255, 255]);
        assert_eq!(id.port, u16::MAX);
        assert_eq!(id.run, [b'a'; 64]);
        assert_eq!(id.run_len, 64);
        assert_eq!(id.deadline_ms, i32::MAX as u32);
        assert_eq!(&id.build, build[11..].as_bytes());
        let short = parse(&[endpoint, "--run-id=A-_.9", build, deadline]).unwrap();
        assert_eq!(&short.run[..5], b"A-_.9");
        assert_eq!(short.run[5..], [0; 59]);
        assert_eq!(
            parse(&[endpoint, build, deadline]),
            Err("missing run identity")
        );
        assert_eq!(
            parse(&[endpoint, "--run-id=x", deadline]),
            Err("missing build identity")
        );
        for value in ["0.1.2.3:1", "224.1.2.3:1", "255.255.255.255:1"] {
            let arg = std::format!("--report={value}");
            assert_eq!(
                parse(&[&arg, "--run-id=x", build, deadline]),
                Err("invalid report address")
            );
        }
        for value in ["a b", "a/b", "é", "a\n", "a\0"] {
            let arg = std::format!("--run-id={value}");
            assert_eq!(
                parse(&[endpoint, &arg, build, deadline]),
                Err("invalid run identity")
            );
        }
        let too_long = std::format!("--run-id={}", "a".repeat(65));
        assert_eq!(
            parse(&[endpoint, &too_long, build, deadline]),
            Err("invalid run identity")
        );
        for value in ["a".repeat(63), "a".repeat(65), "g".repeat(64)] {
            let arg = std::format!("--build-id={value}");
            assert_eq!(
                parse(&[endpoint, "--run-id=x", &arg, deadline]),
                Err("invalid build identity")
            );
        }
    }

    #[test]
    fn full_width_wire_fields_do_not_overlap() {
        let mut id = identity();
        id.run = [b'R'; 64];
        id.run_len = 64;
        id.build = [b'B'; 64];
        let progress = progress_header(&id, u32::MAX, u32::MAX as usize);
        assert_eq!(
            &progress,
            b"WIP1\0\x01\0\0\xff\xff\xff\xff\0\0\0\0\0\x40\0\x40\xff\xff\xff\xff"
        );
        let final_bytes = final_header(&id, u32::MAX as usize, true);
        assert_eq!(&final_bytes[..12], b"WIR2\0\x02\0\x01\xff\xff\xff\xff");
        assert_eq!(final_bytes[12..24], [0; 12]);
        assert_eq!(&final_bytes[24..28], b"\0\0\0\x24");
        assert_eq!(final_bytes[28..92], [b'R'; 64]);
        assert_eq!(final_bytes[92..156], [b'B'; 64]);
    }

    #[test]
    fn complete_log_and_maximum_loss_counters_have_exact_wire_lengths() {
        let id = identity();
        let log = [b'x'; LOG_CAPACITY];
        let mut wire = std::vec::Vec::new();
        write_final_report(&id, &log, usize::MAX, usize::MAX, i32::MIN, |part| {
            wire.extend_from_slice(part);
            Ok::<_, ()>(())
        })
        .unwrap();
        let text_len = u32::from_be_bytes(wire[8..12].try_into().unwrap()) as usize;
        let tail = std::format!(
            "INFO dev log_lost_bytes={} log_contention={}\nFAIL application return or development log loss\nFAIL summary passes=0 fails=1\n",
            usize::MAX,
            usize::MAX
        );
        assert_eq!(text_len, LOG_CAPACITY + tail.len());
        assert_eq!(wire.len(), 156 + text_len + 36);
        assert_eq!(&wire[156..156 + LOG_CAPACITY], &log);
        assert_eq!(&wire[156 + LOG_CAPACITY..156 + text_len], tail.as_bytes());
        assert_eq!(wire[156 + text_len..], [0; 36]);
        assert_eq!(&wire[6..8], b"\0\x01");
        for (lost, contention, flag) in [(0, 1, 0), (1, 0, 1)] {
            write_final_report(&id, b"", lost, contention, 0, |part| {
                if part.starts_with(b"WIR2") {
                    assert_eq!(part[6..8], [0, flag]);
                }
                Ok::<_, ()>(())
            })
            .unwrap();
        }
    }

    #[test]
    fn log_zero_capacity_utf8_and_saturating_loss_are_byte_based() {
        let mut empty = LogBuffer::<0>::default();
        empty.write_str("").unwrap();
        empty.write_str("é").unwrap();
        assert_eq!((empty.len, empty.lost), (0, 2));
        let mut short = LogBuffer::<1>::default();
        short.write_str("é").unwrap();
        assert_eq!(short.bytes, [0xc3]);
        assert_eq!((short.len, short.lost), (1, 1));
        short.lost = usize::MAX - 1;
        short.write_str("more").unwrap();
        assert_eq!(short.bytes, [0xc3]);
        assert_eq!((short.len, short.lost), (1, usize::MAX));
    }
}
