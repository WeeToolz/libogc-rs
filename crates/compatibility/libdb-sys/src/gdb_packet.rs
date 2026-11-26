pub(crate) fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

pub(crate) fn signal_for_exception(exid: u32) -> u8 {
    match exid {
        2 | 3 => 11,      // SIGSEGV: machine check and data storage
        4 | 6 => 10,      // SIGBUS: instruction storage and alignment
        5 => 2,           // SIGINT: external interrupt
        7 | 13 | 19 => 5, // SIGTRAP: undefined, trace, and breakpoint
        8 => 8,           // SIGFPE: floating point unavailable
        9 => 14,          // SIGALRM: decrementer
        12 => 12,         // SIGSYS: system call
        _ => 1,           // SIGHUP
    }
}

/// Parse one C-compatible variable-width hexadecimal `u32` field.
///
/// C's `vhstr2int` accepts at most eight hexadecimal digits. It leaves the
/// first non-hexadecimal byte for the caller to validate as a separator.
pub(crate) fn parse_hex_u32(bytes: &[u8]) -> Option<(u32, usize)> {
    let mut value = 0u32;
    let mut count = 0usize;
    for &byte in bytes {
        let Some(nibble) = hex_nibble(byte) else {
            break;
        };
        if count == 8 {
            return None;
        }
        value = value.checked_mul(16)?.checked_add(nibble as u32)?;
        count += 1;
    }
    (count != 0).then_some((value, count))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn variable_hex_accepts_one_to_eight_digits_and_leaves_delimiter() {
        assert_eq!(parse_hex_u32(b"0,"), Some((0, 1)));
        assert_eq!(parse_hex_u32(b"7fffFFFF,"), Some((0x7fff_ffff, 8)));
        assert_eq!(parse_hex_u32(b"ffffffff,"), Some((u32::MAX, 8)));
    }

    #[test]
    fn variable_hex_rejects_more_than_eight_digits() {
        assert_eq!(parse_hex_u32(b"100000000,"), None);
        assert_eq!(parse_hex_u32(b"fffffffff"), None);
    }

    #[test]
    fn variable_hex_rejects_empty_and_non_hex_fields() {
        assert_eq!(parse_hex_u32(b","), None);
        assert_eq!(parse_hex_u32(b"xyz"), None);
    }

    #[test]
    fn exception_signals_match_the_c_hard_trap_table() {
        for (exid, signal) in [
            (2, 11),
            (3, 11),
            (4, 10),
            (5, 2),
            (6, 10),
            (7, 5),
            (8, 8),
            (9, 14),
            (12, 12),
            (13, 5),
            (19, 5),
            (1, 1),
            (15, 1),
        ] {
            assert_eq!(signal_for_exception(exid), signal, "exception {exid}");
        }
    }
}
