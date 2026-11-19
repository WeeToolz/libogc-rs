use ogc_system::{self as system, SysSram, SysSramEx};
use ogc_system_sys as raw;

use crate::probe_support::ProbeLogger;

pub fn run(logger: &mut ProbeLogger) {
    logger.section("SRAM PROBE");

    let Some(sram) = system::snapshot_sram() else {
        logger.record("sram snapshot available", false);
        return;
    };
    let Some(sram_ex) = system::snapshot_sram_ex() else {
        logger.record("sram ex snapshot available", false);
        return;
    };

    logger.record(
        "sram checksum matches live snapshot",
        checksum_matches(&sram),
    );
    logger.record("sram getter parity", getter_parity(&sram, &sram_ex));
    logger.record("counter bias setter restore", test_counter_bias(&sram));
    logger.record("display offset setter restore", test_display_offset(&sram));
    logger.record("eu rgb60 setter restore", test_eu_rgb60(&sram));
    logger.record("language setter restore", test_language(&sram));
    logger.record("progressive setter restore", test_progressive_scan(&sram));
    logger.record("sound mode setter restore", test_sound_mode(&sram));
    logger.record("video mode setter restore", test_video_mode(&sram));
    logger.record("wireless id setter restore", test_wireless_id(&sram_ex));
    logger.record("gbs setter restore", test_gbs_mode(&sram_ex));
    logger.record(
        "raw standard SRAM values restore",
        raw_standard_values(&sram),
    );
    logger.record(
        "raw extended SRAM values restore",
        raw_extended_values(&sram_ex),
    );
    logger.record(
        "final sram restore",
        system::snapshot_sram() == Some(sram) && system::snapshot_sram_ex() == Some(sram_ex),
    );
}

fn checksum_matches(sram: &SysSram) -> bool {
    let (checksum, checksum_inv) = system::sram_checksum(sram);
    let mut refreshed = *sram;
    system::refresh_sram_checksum(&mut refreshed);
    checksum == sram.checksum
        && checksum_inv == sram.checksum_inv
        && refreshed.checksum == sram.checksum
        && refreshed.checksum_inv == sram.checksum_inv
}

fn getter_parity(sram: &SysSram, sram_ex: &SysSramEx) -> bool {
    system::sram_counter_bias(sram) == system::get_counter_bias()
        && system::sram_display_offset_h(sram) == system::get_display_offset_h()
        && system::sram_eu_rgb60(sram) == system::get_eu_rgb60()
        && system::sram_language(sram) == system::get_language()
        && system::sram_progressive_scan(sram) == system::get_progressive_scan()
        && system::sram_sound_mode(sram) == system::get_sound_mode()
        && system::sram_video_mode(sram) == system::get_video_mode()
        && system::sram_ex_wireless_id(sram_ex, 0) == system::get_wireless_id(0)
        && system::sram_ex_wireless_id(sram_ex, 1) == system::get_wireless_id(1)
        && system::sram_ex_wireless_id(sram_ex, 2) == system::get_wireless_id(2)
        && system::sram_ex_wireless_id(sram_ex, 3) == system::get_wireless_id(3)
        && system::sram_ex_gbs_mode(sram_ex) == system::get_gbs_mode()
}

fn test_counter_bias(original: &SysSram) -> bool {
    let mut expected = *original;
    let test = original.counter_bias ^ 1;
    let _ = system::set_sram_counter_bias(&mut expected, test);
    system::set_counter_bias(test);
    let ok = system::get_counter_bias() == system::sram_counter_bias(&expected);
    system::set_counter_bias(original.counter_bias);
    ok && system::get_counter_bias() == original.counter_bias
}

fn test_display_offset(original: &SysSram) -> bool {
    let mut expected = *original;
    let test = original.display_offset_h.wrapping_add(1);
    let _ = system::set_sram_display_offset_h(&mut expected, test);
    system::set_display_offset_h(test);
    let ok = system::get_display_offset_h() == system::sram_display_offset_h(&expected);
    system::set_display_offset_h(original.display_offset_h);
    ok && system::get_display_offset_h() == original.display_offset_h
}

fn test_eu_rgb60(original: &SysSram) -> bool {
    let mut expected = *original;
    let test = system::sram_eu_rgb60(original) ^ 1;
    let _ = system::set_sram_eu_rgb60(&mut expected, test);
    system::set_eu_rgb60(test);
    let ok = system::get_eu_rgb60() == system::sram_eu_rgb60(&expected);
    system::set_eu_rgb60(system::sram_eu_rgb60(original));
    ok && system::get_eu_rgb60() == system::sram_eu_rgb60(original)
}

fn test_language(original: &SysSram) -> bool {
    let mut expected = *original;
    let test = (original.lang + 1) % 6;
    let _ = system::set_sram_language(&mut expected, test);
    system::set_language(test);
    let ok = system::get_language() == system::sram_language(&expected);
    system::set_language(original.lang);
    ok && system::get_language() == original.lang
}

fn test_progressive_scan(original: &SysSram) -> bool {
    let mut expected = *original;
    let test = system::sram_progressive_scan(original) ^ 1;
    let _ = system::set_sram_progressive_scan(&mut expected, test);
    system::set_progressive_scan(test);
    let ok = system::get_progressive_scan() == system::sram_progressive_scan(&expected);
    system::set_progressive_scan(system::sram_progressive_scan(original));
    ok && system::get_progressive_scan() == system::sram_progressive_scan(original)
}

fn test_sound_mode(original: &SysSram) -> bool {
    let mut expected = *original;
    let test = system::sram_sound_mode(original) ^ 1;
    let _ = system::set_sram_sound_mode(&mut expected, test);
    system::set_sound_mode(test);
    let ok = system::get_sound_mode() == system::sram_sound_mode(&expected);
    system::set_sound_mode(system::sram_sound_mode(original));
    ok && system::get_sound_mode() == system::sram_sound_mode(original)
}

fn test_video_mode(original: &SysSram) -> bool {
    let mut expected = *original;
    let test = (system::sram_video_mode(original) + 1) % 3;
    let _ = system::set_sram_video_mode(&mut expected, test);
    system::set_video_mode(test);
    let ok = system::get_video_mode() == system::sram_video_mode(&expected);
    system::set_video_mode(system::sram_video_mode(original));
    ok && system::get_video_mode() == system::sram_video_mode(original)
}

fn test_wireless_id(original: &SysSramEx) -> bool {
    let mut expected = *original;
    let base = system::sram_ex_wireless_id(original, 0);
    let test = base ^ 0x55aa;
    let _ = system::set_sram_ex_wireless_id(&mut expected, 0, test);
    system::set_wireless_id(0, test);
    let ok = system::get_wireless_id(0) == system::sram_ex_wireless_id(&expected, 0);
    system::set_wireless_id(0, base);
    ok && system::get_wireless_id(0) == base
}

fn test_gbs_mode(original: &SysSramEx) -> bool {
    let mut expected = *original;
    let _ = system::set_sram_ex_gbs_mode(&mut expected, 0xffff);
    system::set_gbs_mode(0xffff);
    let ok = system::get_gbs_mode() == system::sram_ex_gbs_mode(&expected);
    system::set_gbs_mode(system::sram_ex_gbs_mode(original));
    ok && system::get_gbs_mode() == system::sram_ex_gbs_mode(original)
}

// The Dolphin runner uses a private profile. Each changed field is restored
// before the next field is changed, even if a comparison fails.
fn raw_standard_values(original: &SysSram) -> bool {
    // SAFETY: This probe owns the SRAM operations and uses the saved values
    // to restore every field after the check.
    unsafe {
        let mut ok = true;
        let bias = original.counter_bias;
        ok &= raw::sys_get_counter_bias() == bias;
        raw::sys_set_counter_bias(bias ^ 1);
        ok &= raw::sys_get_counter_bias() == bias ^ 1;
        raw::sys_set_counter_bias(bias);
        ok &= raw::sys_get_counter_bias() == bias;

        let offset = original.display_offset_h;
        ok &= raw::sys_get_display_offset_h() == offset;
        raw::sys_set_display_offset_h(offset.wrapping_add(1));
        ok &= raw::sys_get_display_offset_h() == offset.wrapping_add(1);
        raw::sys_set_display_offset_h(offset);
        ok &= raw::sys_get_display_offset_h() == offset;

        let rgb60 = (original.ntd >> 6) & 1;
        ok &= raw::sys_get_eu_rgb60() == rgb60;
        raw::sys_set_eu_rgb60(rgb60 ^ 1);
        ok &= raw::sys_get_eu_rgb60() == rgb60 ^ 1;
        raw::sys_set_eu_rgb60(rgb60);
        ok &= raw::sys_get_eu_rgb60() == rgb60;

        let language = original.lang;
        let changed_language = language.wrapping_add(1) % 6;
        ok &= raw::sys_get_language() == language;
        raw::sys_set_language(changed_language);
        ok &= raw::sys_get_language() == changed_language;
        raw::sys_set_language(language);
        ok &= raw::sys_get_language() == language;

        let progressive = (original.flags >> 7) & 1;
        ok &= raw::sys_get_progressive_scan() == progressive;
        raw::sys_set_progressive_scan(progressive ^ 1);
        ok &= raw::sys_get_progressive_scan() == progressive ^ 1;
        raw::sys_set_progressive_scan(progressive);
        ok &= raw::sys_get_progressive_scan() == progressive;

        let sound = (original.flags >> 2) & 1;
        ok &= raw::sys_get_sound_mode() == sound;
        raw::sys_set_sound_mode(sound ^ 1);
        ok &= raw::sys_get_sound_mode() == sound ^ 1;
        raw::sys_set_sound_mode(sound);
        ok &= raw::sys_get_sound_mode() == sound;

        let video = original.flags & 3;
        let changed_video = (video + 1) & 3;
        ok &= raw::sys_get_video_mode() == video;
        raw::sys_set_video_mode(changed_video);
        ok &= raw::sys_get_video_mode() == changed_video;
        raw::sys_set_video_mode(video);
        ok &= raw::sys_get_video_mode() == video;
        ok
    }
}

fn raw_extended_values(original: &SysSramEx) -> bool {
    // SAFETY: Packed fields are read without creating unaligned references.
    // Each setter is followed by restoration of the saved value.
    unsafe {
        let wireless = core::ptr::read_unaligned(core::ptr::addr_of!(original.wireless_pad_id[0]));
        let gbs = core::ptr::read_unaligned(core::ptr::addr_of!(original.gbs));
        let valid_gbs = ((gbs >> 10) & 0x1f) < 20 && ((gbs >> 6) & 3) != 3 && (gbs & 0x3f) < 60;
        let mut ok = raw::sys_get_wireless_id(0) == wireless && raw::sys_get_wireless_id(4) == 0;
        raw::sys_set_wireless_id(0, wireless ^ 0x55aa);
        ok &= raw::sys_get_wireless_id(0) == wireless ^ 0x55aa;
        raw::sys_set_wireless_id(0, wireless);
        ok &= raw::sys_get_wireless_id(0) == wireless;
        ok &= raw::sys_get_gbs_mode() == gbs && valid_gbs;
        if valid_gbs {
            let changed = u16::from(gbs == 0);
            raw::sys_set_gbs_mode(changed);
            ok &= raw::sys_get_gbs_mode() == changed;
            raw::sys_set_gbs_mode(gbs);
            ok &= raw::sys_get_gbs_mode() == gbs;
        }
        ok
    }
}
