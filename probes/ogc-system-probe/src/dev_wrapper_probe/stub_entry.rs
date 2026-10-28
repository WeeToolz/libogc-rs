//! Checked relative branch encoding for the temporary entry observer.

pub fn observer_branch(original: u32, observer: usize) -> Result<u32, i32> {
    let displacement = ((original & 0x03ff_fffc) as i32) << 6 >> 6;
    let body = 0x80001800u32.wrapping_add_signed(displacement);
    if original & 0xfc00_0003 != 0x4800_0000 || !(0x80001810..0x80003000).contains(&body) {
        return Err(-95);
    }
    let delta = observer as i64 - 0x80001800i64;
    if observer & 3 != 0 || !(-0x0200_0000..0x0200_0000).contains(&delta) {
        return Err(-34);
    }
    Ok(0x4800_0000 | (delta as u32 & 0x03ff_fffc))
}

#[cfg(test)]
mod tests {
    use super::observer_branch;

    #[test]
    fn accepts_both_observed_stub_entry_offsets() {
        assert_eq!(observer_branch(0x48000010, 0x80004000), Ok(0x48002800));
        assert_eq!(observer_branch(0x48000018, 0x80006000), Ok(0x48004800));
    }

    #[test]
    fn rejects_other_opcodes_absolute_branches_and_linking_branches() {
        for word in [0x60000000, 0x48000011, 0x48000012, 0x48000013] {
            assert_eq!(observer_branch(word, 0x80004000), Err(-95));
        }
    }

    #[test]
    fn rejects_branches_into_the_header_or_outside_the_saved_image() {
        for word in [
            0x48000000, 0x48000004, 0x48000008, 0x4800000c, 0x4bfffffc, 0x48001800,
        ] {
            assert_eq!(observer_branch(word, 0x80004000), Err(-95));
        }
        assert_eq!(observer_branch(0x480017fc, 0x80004000), Ok(0x48002800));
    }

    #[test]
    fn checks_observer_alignment_and_signed_branch_range() {
        assert_eq!(observer_branch(0x48000018, 0x80004002), Err(-34));
        assert_eq!(observer_branch(0x48000018, 0x82001800), Err(-34));
        assert_eq!(observer_branch(0x48000018, 0x7e0017fc), Err(-34));
        assert_eq!(observer_branch(0x48000018, 0x7e001800), Ok(0x4a000000));
        assert_eq!(observer_branch(0x48000018, 0x820017fc), Ok(0x49fffffc));
    }
}
