// SI communication control fields follow libogc's sicomcsr layout.
const OUT_LENGTH: u32 = 0x007f_0000;
const IN_LENGTH: u32 = 0x0000_7f00;
const CHANNEL: u32 = 0x0000_0006;

pub(super) fn transfer_control(previous: u32, channel: i32, out_len: u32, in_len: u32) -> u32 {
    // Clear the interrupt status and replace both lengths. A zero length field
    // represents 128 bytes.
    let clear = (1 << 31) | (1 << 30) | OUT_LENGTH | IN_LENGTH | CHANNEL;
    (previous & !clear)
        | ((out_len & 0x7f) << 16)
        | ((in_len & 0x7f) << 8)
        | ((channel as u32 & 3) << 1)
        | 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn origin_read_replaces_previous_type_query_length() {
        let type_query = transfer_control(0, 0, 1, 3);
        let origin_read = transfer_control(type_query, 0, 1, 10);
        assert_eq!((origin_read & IN_LENGTH) >> 8, 10);
        assert_eq!((origin_read & OUT_LENGTH) >> 16, 1);
    }

    #[test]
    fn every_length_and_channel_replaces_stale_fields() {
        for channel in 0..4 {
            for length in 1..=128 {
                let word = transfer_control(u32::MAX, channel, length, length);
                assert_eq!((word & OUT_LENGTH) >> 16, length & 0x7f);
                assert_eq!((word & IN_LENGTH) >> 8, length & 0x7f);
                assert_eq!((word & CHANNEL) >> 1, channel as u32);
                assert_eq!(word & 1, 1);
                assert_ne!(word & (1 << 31), 0);
                assert_eq!(word & (1 << 30), 0);
                assert_eq!(word & (1 << 29), u32::MAX & (1 << 29));
                assert_eq!(word & (1 << 28), u32::MAX & (1 << 28));
                assert_eq!(word & (1 << 27), u32::MAX & (1 << 27));
            }
        }
    }
}
