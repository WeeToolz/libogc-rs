//! Decode the IOS getsockopt output before publishing caller data.

pub(crate) fn payload(reply: &[u8; 36], capacity: usize) -> Result<&[u8], i32> {
    let length = u32::from_be_bytes(reply[12..16].try_into().unwrap()) as usize;
    if length > 20 || length > capacity {
        return Err(-22);
    }
    Ok(&reply[16..16 + length])
}

#[cfg(test)]
mod tests {
    use super::payload;

    #[test]
    fn decodes_fixed_ios_value_bytes() {
        let mut reply = [0; 36];
        reply[15] = 4;
        reply[16..20].copy_from_slice(&[0, 0, 0x40, 0]);
        assert_eq!(payload(&reply, 4), Ok(&[0, 0, 0x40, 0][..]));
    }

    #[test]
    fn rejects_oversized_ios_reply_and_small_caller_buffer() {
        let mut reply = [0; 36];
        reply[15] = 21;
        assert_eq!(payload(&reply, 32), Err(-22));
        reply[15] = 4;
        assert_eq!(payload(&reply, 3), Err(-22));
    }
}
