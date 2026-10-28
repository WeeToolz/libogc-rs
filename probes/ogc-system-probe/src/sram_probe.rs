use ogc_system::{self as system, SysSram, SysSramEx};

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
