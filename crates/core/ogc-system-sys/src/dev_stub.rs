//! Read-only decoding of the known HBC real-mode startup sequence.
pub fn real_destination(image: &[u8]) -> Result<u32, i32> {
    let prefix = image.get(..0x48).ok_or(-22)?;
    let word = |offset| u32::from_be_bytes(prefix[offset..offset + 4].try_into().unwrap());
    // Recognize the complete sequence before interpreting its address words.
    // Both known versions clear EE and FP; one uses r3 for the second mask.
    if word(0) != 0x48000010
        || &prefix[4..12] != b"STUBHAXX"
        || word(0x10) != 0x7c6000a6
        || word(0x14) != 0x5464045e
        || ![0x548406b0, 0x546406b0].contains(&word(0x18))
        || word(0x1c) != 0x7c800124
        || word(0x20) != 0x4c00012c
        || word(0x24) & 0xffff0000 != 0x3c600000
        || word(0x28) & 0xffff0000 != 0x60630000
        || word(0x2c) != 0x546300be
        || word(0x30) != 0x7c7a03a6
        || word(0x34) != 0x7c6000a6
        || word(0x38) != 0x38800030
        || word(0x3c) != 0x7c632078
        || word(0x40) != 0x7c7b03a6
        || word(0x44) != 0x4c000064
    {
        return Err(-95);
    }
    Ok((((word(0x24) & 0xffff) << 16) | (word(0x28) & 0xffff)) & 0x3fffffff)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn image() -> [u8; 72] {
        let words: [u32; 18] = [
            0x48000010, 0x53545542, 0x48415858, 0, 0x7c6000a6, 0x5464045e, 0x548406b0, 0x7c800124,
            0x4c00012c, 0x3c608000, 0x60631848, 0x546300be, 0x7c7a03a6, 0x7c6000a6, 0x38800030,
            0x7c632078, 0x7c7b03a6, 0x4c000064,
        ];
        let mut image = [0; 72];
        for (bytes, word) in image.chunks_exact_mut(4).zip(words) {
            bytes.copy_from_slice(&word.to_be_bytes());
        }
        image
    }
    #[test]
    fn reports_actual_destinations_without_changing_bytes() {
        let mut bytes = image();
        assert_eq!(real_destination(&bytes), Ok(0x1848));
        bytes[0x28..0x2c].copy_from_slice(&0x60633f48u32.to_be_bytes());
        let before = bytes;
        assert_eq!(real_destination(&bytes), Ok(0x3f48));
        assert_eq!(bytes, before);
    }
    #[test]
    fn rejects_truncated_and_unknown_sequences() {
        assert_eq!(real_destination(&image()[..71]), Err(-22));
        for offset in [0, 4, 0x10, 0x18, 0x24, 0x28, 0x30, 0x44] {
            let mut bytes = image();
            bytes[offset] ^= 1;
            assert_eq!(real_destination(&bytes), Err(-95));
        }
    }
}
