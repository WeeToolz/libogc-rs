use modplay_capi::{Mod, get_effect, get_effect_op, get_instr, get_note, mod_set_mod_from_bytes};
use ogc_sys::Bool;

fn write_be_u16_words(buf: &mut [u8], offset: usize, word_value: u16) {
    let [high, low] = word_value.to_be_bytes();
    buf[offset] = high;
    buf[offset + 1] = low;
}

fn pattern_base_offset(num_instr: usize, num_voices: usize) -> usize {
    let mut offset = 20 + (num_instr * 30) + 1 + 1 + 128;
    if num_instr != 15 {
        offset += 4;
    }
    let _ = num_voices;
    offset
}

fn build_mod_with_id(id: [u8; 4], voices: usize) -> Vec<u8> {
    let num_instr = 31usize;
    let num_patterns = 2usize;
    let pattern_bytes = 4 * 64 * voices * num_patterns;
    let sample_bytes = 8usize + 4usize;
    let total = pattern_base_offset(num_instr, voices) + pattern_bytes + sample_bytes;
    let mut bytes = vec![0u8; total];

    bytes[..20].copy_from_slice(b"TEST MOD SONG NAME!!");
    let mut inst0_name = [b' '; 22];
    inst0_name[..11].copy_from_slice(b"BASS SAMPLE");
    bytes[20..42].copy_from_slice(&inst0_name);
    write_be_u16_words(&mut bytes, 42, 4);
    bytes[44] = 0x0f;
    bytes[45] = 0x7f;
    write_be_u16_words(&mut bytes, 46, 1);
    write_be_u16_words(&mut bytes, 48, 2);

    let inst1 = 50usize;
    let mut inst1_name = [b' '; 22];
    inst1_name[..8].copy_from_slice(b"NO LOOP ");
    bytes[inst1..inst1 + 22].copy_from_slice(&inst1_name);
    write_be_u16_words(&mut bytes, inst1 + 22, 2);
    bytes[inst1 + 24] = 0x00;
    bytes[inst1 + 25] = 0x20;
    write_be_u16_words(&mut bytes, inst1 + 26, 0);
    write_be_u16_words(&mut bytes, inst1 + 28, 1);

    let song_len_offset = 20 + (num_instr * 30);
    bytes[song_len_offset] = 2;
    bytes[song_len_offset + 1] = 0x7f;
    bytes[song_len_offset + 2] = 0;
    bytes[song_len_offset + 3] = 65;

    let id_offset = song_len_offset + 2 + 128;
    bytes[id_offset..id_offset + 4].copy_from_slice(&id);

    let patterns_offset = pattern_base_offset(num_instr, voices);
    let line0_ch0 = patterns_offset;
    bytes[line0_ch0] = 0x11;
    bytes[line0_ch0 + 1] = 0x23;
    bytes[line0_ch0 + 2] = 0x45;
    bytes[line0_ch0 + 3] = 0x67;

    let line1_ch2 = patterns_offset + (4 * voices) + (2 * 4);
    bytes[line1_ch2] = 0xab;
    bytes[line1_ch2 + 1] = 0xcd;
    bytes[line1_ch2 + 2] = 0xef;
    bytes[line1_ch2 + 3] = 0x01;

    let pattern1_base = patterns_offset + (4 * 64 * voices);
    bytes[pattern1_base] = 0x20;
    bytes[pattern1_base + 1] = 0x10;
    bytes[pattern1_base + 2] = 0x30;
    bytes[pattern1_base + 3] = 0x40;

    let sample_offset = patterns_offset + pattern_bytes;
    for idx in 0..8 {
        bytes[sample_offset + idx] = idx as u8;
    }
    for idx in 0..4 {
        bytes[sample_offset + 8 + idx] = (idx as u8) + 8;
    }

    bytes
}

fn build_legacy_15_instr_mod() -> Vec<u8> {
    let num_instr = 15usize;
    let voices = 4usize;
    let pattern_bytes = 4 * 64 * voices;
    let total = pattern_base_offset(num_instr, voices) + pattern_bytes;
    let mut bytes = vec![0u8; total];

    bytes[..20].copy_from_slice(b"LEGACY MODULE DEMO!!");
    let song_len_offset = 20 + (num_instr * 30);
    bytes[song_len_offset] = 1;
    bytes[song_len_offset + 1] = 0;
    bytes[song_len_offset + 2] = 0;

    let patterns_offset = pattern_base_offset(num_instr, voices);
    bytes[patterns_offset] = 0x01;
    bytes[patterns_offset + 1] = 0x02;
    bytes[patterns_offset + 2] = 0x34;
    bytes[patterns_offset + 3] = 0x56;

    bytes
}

#[test]
fn mod_set_mod_parses_31_instrument_metadata_and_patterns() {
    let mut bytes = build_mod_with_id(*b"M.K.", 4);
    let mut parsed = unsafe { core::mem::zeroed::<Mod>() };

    assert_eq!(mod_set_mod_from_bytes(&mut parsed, &mut bytes), 0);
    assert_eq!(parsed.num_instr, 31);
    assert_eq!(parsed.num_voices, 4);
    assert_eq!(parsed.num_channels, 4);
    assert_eq!(parsed.shiftval, 6);
    assert_eq!(parsed.song_length, 2);
    assert_eq!(parsed.num_patterns, 2);
    assert_eq!(parsed.musicvolume, 0x40);
    assert_eq!(parsed.sfxvolume, 0x40);
    assert_eq!(parsed.set, 1 as Bool);
    assert_eq!(parsed.id.map(|v| v as u8), *b"M.K.");
    assert_eq!(
        &parsed.name[..19],
        &[
            b'T' as i8, b'E' as i8, b'S' as i8, b'T' as i8, b' ' as i8, b'M' as i8, b'O' as i8,
            b'D' as i8, b' ' as i8, b'S' as i8, b'O' as i8, b'N' as i8, b'G' as i8, b' ' as i8,
            b'N' as i8, b'A' as i8, b'M' as i8, b'E' as i8, b'!' as i8,
        ]
    );

    assert_eq!(parsed.instrument[0].length, 8);
    assert_eq!(parsed.instrument[0].finetune, 7);
    assert_eq!(parsed.instrument[0].volume, 0x40);
    assert_eq!(parsed.instrument[0].loop_start, 2);
    assert_eq!(parsed.instrument[0].loop_end, 6);
    assert_eq!(parsed.instrument[0].loop_length, 4);
    assert_eq!(parsed.instrument[0].looped, 1);
    assert_eq!(parsed.instrument[0].name[11], 0);

    assert_eq!(parsed.instrument[1].length, 4);
    assert_eq!(parsed.instrument[1].looped, 0);
    assert_eq!(parsed.instrument[1].loop_start, 4);
    assert_eq!(parsed.instrument[1].loop_end, 4);
    assert_eq!(parsed.instrument[1].loop_length, 0);

    assert_eq!(parsed.song[0], 0);
    assert_eq!(parsed.song[1], 1);
    // SAFETY: `parsed` contains a validated module image, and all requested
    // pattern lines and channels are within that parsed image.
    unsafe {
        assert_eq!(get_note(&mut parsed, 0, 0), 0x123);
        assert_eq!(get_instr(&mut parsed, 0, 0), 0x14);
        assert_eq!(get_effect(&mut parsed, 0, 0), 0x05);
        assert_eq!(get_effect_op(&mut parsed, 0, 0), 0x67);
        assert_eq!(get_note(&mut parsed, 1, 2), 0x0bcd);
        assert_eq!(get_instr(&mut parsed, 1, 2), 0xae);
        assert_eq!(get_effect(&mut parsed, 1, 2), 0x0f);
        assert_eq!(get_effect_op(&mut parsed, 1, 2), 0x01);

        parsed.songpos = 1;
        assert_eq!(get_note(&mut parsed, 0, 0), 0x010);
        assert_eq!(get_instr(&mut parsed, 0, 0), 0x23);
        assert_eq!(get_effect(&mut parsed, 0, 0), 0x00);
        assert_eq!(get_effect_op(&mut parsed, 0, 0), 0x40);
    }
}

#[test]
fn mod_set_mod_falls_back_to_15_instrument_legacy_layout() {
    let mut bytes = build_legacy_15_instr_mod();
    let mut parsed = unsafe { core::mem::zeroed::<Mod>() };

    assert_eq!(mod_set_mod_from_bytes(&mut parsed, &mut bytes), 0);
    assert_eq!(parsed.num_instr, 15);
    assert_eq!(parsed.num_voices, 4);
    assert_eq!(parsed.num_channels, 4);
    assert_eq!(parsed.num_patterns, 1);
    assert_eq!(parsed.id.map(|value| value as u8), [0, 0, 0, 0]);
    assert!(parsed.patterndata as usize > parsed.modraw as usize);
    // SAFETY: `parsed` contains a validated module image, and pattern line 0
    // and channel 0 are within that parsed image.
    unsafe {
        assert_eq!(get_note(&mut parsed, 0, 0), 0x102);
        assert_eq!(get_instr(&mut parsed, 0, 0), 0x03);
        assert_eq!(get_effect(&mut parsed, 0, 0), 0x04);
        assert_eq!(get_effect_op(&mut parsed, 0, 0), 0x56);
    }
}

#[test]
fn mod_set_mod_rejects_truncated_images() {
    let mut bytes = build_mod_with_id(*b"M.K.", 4);
    bytes.pop();
    let mut parsed = unsafe { core::mem::zeroed::<Mod>() };

    assert_eq!(mod_set_mod_from_bytes(&mut parsed, &mut bytes), -1);
    assert_eq!(parsed.set, 0);
    assert!(parsed.modraw.is_null());
}
