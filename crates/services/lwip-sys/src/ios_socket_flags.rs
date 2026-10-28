//! Convert the public lwIP message flags to the Wii IOS message ABI.

pub(crate) fn to_ios(flags: u32) -> u32 {
    // lwIP MSG_DONTWAIT is 0x40; IOS SO_MSG_NONBLOCK is 0x04.
    (flags & !0x40) | if flags & 0x40 != 0 { 0x04 } else { 0 }
}

#[cfg(test)]
mod tests {
    use super::to_ios;

    #[test]
    fn converts_nonblocking_and_preserves_other_flags() {
        assert_eq!(to_ios(0), 0);
        assert_eq!(to_ios(0x40), 4);
        assert_eq!(to_ios(0x43), 7);
        assert_eq!(to_ios(0x03), 3);
        assert_eq!(to_ios(0x80), 0x80);
    }
}

/// IOS uses a private TCP_NODELAY option number, unlike the public BSD ABI.
pub fn option_to_ios(level: u32, name: u32) -> u32 {
    if level == 6 && name == 1 {
        0x2001
    } else {
        name
    }
}
#[cfg(test)]
mod option_tests {
    #[test]
    fn tcp_option_number_changes_only_at_tcp_level() {
        assert_eq!(super::option_to_ios(6, 1), 0x2001);
        assert_eq!(super::option_to_ios(0xffff, 1), 1);
        assert_eq!(super::option_to_ios(0xffff, 0x1002), 0x1002);
        assert_eq!(super::option_to_ios(0, 4), 4);
    }
}
