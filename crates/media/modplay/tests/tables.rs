use modplay::{FREQTAB, FREQTAB_LEN, SEMITONETAB, SEMITONETAB_LEN, TARGET_ENDIANNESS};
use ogc_sys::BYTE_ORDER_BIG_ENDIAN;

#[test]
fn table_lengths_and_anchor_values_match_review_expectations() {
    assert_eq!(TARGET_ENDIANNESS, BYTE_ORDER_BIG_ENDIAN);
    assert_eq!(FREQTAB_LEN, 140);
    assert_eq!(SEMITONETAB_LEN, 4096);

    assert_eq!(FREQTAB[0], 27_392);
    assert_eq!(FREQTAB[21], 8_128);
    assert_eq!(FREQTAB[139], 9);

    assert_eq!(SEMITONETAB[0], 0);
    assert_eq!(SEMITONETAB[1], 143);
    assert_eq!(SEMITONETAB[4_095], 0);
}

#[test]
fn frequency_values_have_expected_big_endian_serialization() {
    let encoded: Vec<u8> = FREQTAB
        .iter()
        .flat_map(|value| value.to_be_bytes())
        .collect();

    assert_eq!(encoded[0], 0x6b);
    assert_eq!(encoded[1], 0x00);
    assert_eq!(encoded[2], 0x65);
    assert_eq!(encoded[3], 0x00);
    assert_eq!(encoded.len(), FREQTAB_LEN * core::mem::size_of::<u16>());
}
