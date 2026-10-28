//! Error codes and text from `stream.h` and `stream.c`.

#[cfg(test)]
use super::abi::*;

pub const fn mad_recoverable(error: i32) -> bool {
    libmad_rs::stream::error_is_recoverable(error)
}

pub const fn mad_error_string(error: i32) -> Option<&'static [u8]> {
    if error < 0 || error > u16::MAX as i32 {
        return None;
    }
    match libmad_rs::Error::from_code(error as u16) {
        Some(error) => Some(error.c_description()),
        None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_error_codes_and_text_match_fixed_abi_values() {
        // Fixed expectations must also catch changes to the shared definition.
        let cases: &[(i32, i32, &[u8])] = &[
            (MAD_ERROR_NONE, 0x0000, b"no error\0"),
            (
                MAD_ERROR_BUFLEN,
                0x0001,
                b"input buffer too small (or EOF)\0",
            ),
            (MAD_ERROR_BUFPTR, 0x0002, b"invalid (null) buffer pointer\0"),
            (MAD_ERROR_NOMEM, 0x0031, b"not enough memory\0"),
            (MAD_ERROR_LOSTSYNC, 0x0101, b"lost synchronization\0"),
            (MAD_ERROR_BADLAYER, 0x0102, b"reserved header layer value\0"),
            (MAD_ERROR_BADBITRATE, 0x0103, b"forbidden bitrate value\0"),
            (
                MAD_ERROR_BADSAMPLERATE,
                0x0104,
                b"reserved sample frequency value\0",
            ),
            (MAD_ERROR_BADEMPHASIS, 0x0105, b"reserved emphasis value\0"),
            (MAD_ERROR_BADCRC, 0x0201, b"CRC check failed\0"),
            (
                MAD_ERROR_BADBITALLOC,
                0x0211,
                b"forbidden bit allocation value\0",
            ),
            (MAD_ERROR_BADSCALEFACTOR, 0x0221, b"bad scalefactor index\0"),
            (MAD_ERROR_BADMODE, 0x0222, b"bad bitrate/mode combination\0"),
            (MAD_ERROR_BADFRAMELEN, 0x0231, b"bad frame length\0"),
            (MAD_ERROR_BADBIGVALUES, 0x0232, b"bad big_values count\0"),
            (MAD_ERROR_BADBLOCKTYPE, 0x0233, b"reserved block_type\0"),
            (
                MAD_ERROR_BADSCFSI,
                0x0234,
                b"bad scalefactor selection info\0",
            ),
            (
                MAD_ERROR_BADDATAPTR,
                0x0235,
                b"bad main_data_begin pointer\0",
            ),
            (MAD_ERROR_BADPART3LEN, 0x0236, b"bad audio data length\0"),
            (
                MAD_ERROR_BADHUFFTABLE,
                0x0237,
                b"bad Huffman table select\0",
            ),
            (MAD_ERROR_BADHUFFDATA, 0x0238, b"Huffman data overrun\0"),
            (
                MAD_ERROR_BADSTEREO,
                0x0239,
                b"incompatible block_type for JS\0",
            ),
        ];
        for &(actual_code, expected_code, expected_text) in cases {
            assert_eq!(actual_code, expected_code);
            assert_eq!(mad_error_string(expected_code), Some(expected_text));
            assert_eq!(
                libmad_rs::Error::from_code(expected_code as u16)
                    .unwrap()
                    .description()
                    .as_bytes(),
                &expected_text[..expected_text.len() - 1],
            );
        }
    }

    #[test]
    fn unknown_and_out_of_range_codes_do_not_alias_known_error_text() {
        for code in [-1, i32::MIN, 0x7fff, 0xffff, 0x10000, 0x10001, i32::MAX] {
            assert_eq!(mad_error_string(code), None, "code={code}");
        }
        assert!(!mad_recoverable(0x10001));
        assert!(mad_recoverable(0x7fff));
        assert!(mad_recoverable(-1));
    }

    #[test]
    fn classifies_recoverable_errors_by_high_byte() {
        assert!(!mad_recoverable(MAD_ERROR_BUFLEN));
        assert!(mad_recoverable(MAD_ERROR_LOSTSYNC));
        assert!(mad_recoverable(MAD_ERROR_BADMODE));
    }

    #[test]
    fn includes_bad_mode_error() {
        assert_eq!(
            mad_error_string(MAD_ERROR_BADMODE),
            Some(&b"bad bitrate/mode combination\0"[..])
        );
        assert_eq!(mad_error_string(0x7fff), None);
    }
}
