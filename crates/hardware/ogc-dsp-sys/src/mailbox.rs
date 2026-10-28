//! DSP mailbox reads must acknowledge the low half, even during shutdown.

pub(crate) fn read_from(mut read_register: impl FnMut(usize) -> u16) -> u32 {
    let high = read_register(2);
    let low = read_register(3);
    (u32::from(high) << 16) | u32::from(low)
}

pub(crate) fn wait_for_empty(mut read_register: impl FnMut(usize) -> u16) {
    while read_from(&mut read_register) & 0x8000_0000 != 0 {
        core::hint::spin_loop();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_preserves_message_and_acknowledges_low_half() {
        let mut registers = [2, 3].into_iter();
        let message = read_from(|index| {
            assert_eq!(Some(index), registers.next());
            if index == 2 { 0x8071 } else { 0xfeed }
        });
        assert_eq!(message, 0x8071_feed);
        assert_eq!(registers.next(), None);
    }

    #[test]
    fn shutdown_consumes_pending_message_before_mailbox_becomes_empty() {
        let mut pending = true;
        let mut reads = 0;
        wait_for_empty(|index| {
            reads += 1;
            assert!(reads <= 4, "mailbox was not acknowledged");
            match index {
                2 => {
                    if pending {
                        0x8071
                    } else {
                        0x0071
                    }
                }
                3 => {
                    pending = false;
                    0xfeed
                }
                _ => panic!("unexpected mailbox register"),
            }
        });
        assert!(!pending);
        assert_eq!(reads, 4);
    }
}
