use ogc_es_sys as es;
use ogc_system::{self as system};
use ogc_video as video;

use crate::probe_support::ProbeLogger;

pub fn run(logger: &mut ProbeLogger) {
    logger.section("GENERAL PROBE");
    probe_es_layout(logger);

    logger.record("counter bias roundtrip", roundtrip_counter_bias());
    logger.record("display offset roundtrip", roundtrip_display_offset());
    logger.record("EU RGB60 roundtrip", roundtrip_eu_rgb60());
    logger.record("language roundtrip", roundtrip_language());
    logger.record("progressive scan roundtrip", roundtrip_progressive_scan());
    logger.record("sound mode roundtrip", roundtrip_sound_mode());
    logger.record("video mode roundtrip", roundtrip_video_mode());

    let hardware = system::hardware_proof();
    logger.note_u64("system time", hardware.time_after);
    logger.record("system time advanced", hardware.time_monotonic);
    logger.note_hex32("hollywood revision", hardware.hollywood_revision);
    logger.record(
        "hollywood revision observable",
        hardware.hollywood_revision != 0,
    );
    logger.note_u32("font encoding", hardware.font_encoding);
    logger.record("font encoding sane", hardware.font_encoding <= 1);
    logger.record("reset button state sane", hardware.reset_button_down <= 1);
    logger.note_u32("video tv mode", video::get_current_tv_mode());
    logger.note_u32("component cable", video::have_component_cable());

    logger.record(
        "console banner write",
        crate::probe_support::screen_write(b"ogc-system-probe general\n").is_ok(),
    );
    logger.record(
        "console summary write",
        crate::probe_support::screen_write(b"system and video state captured\n").is_ok(),
    );
    logger.record(
        "console footer write",
        crate::probe_support::screen_write(b"general probe uses direct runtime calls\n").is_ok(),
    );
}

fn probe_es_layout(logger: &mut ProbeLogger) {
    let signatures = [
        (es::ES_SIG_RSA2048, core::mem::size_of::<es::SigRsa2048>()),
        (es::ES_SIG_RSA4096, core::mem::size_of::<es::SigRsa4096>()),
        (es::ES_SIG_ECDSA, core::mem::size_of::<es::SigEcdsa>()),
    ];
    logger.record(
        "ES signature sizes match public layouts",
        signatures
            .iter()
            .all(|(kind, size)| es::signature_size(*kind) == *size)
            && es::signature_size(u32::MAX) == 0,
    );
    logger.record(
        "ES signature types accept only supported values",
        signatures
            .iter()
            .all(|(kind, _)| es::is_valid_signature(*kind))
            && !es::is_valid_signature(u32::MAX),
    );

    let certificates = [
        (es::ES_CERT_RSA2048, core::mem::size_of::<es::CertRsa2048>()),
        (es::ES_CERT_RSA4096, core::mem::size_of::<es::CertRsa4096>()),
        (es::ES_CERT_ECDSA, core::mem::size_of::<es::CertEcdsa>()),
    ];
    logger.record(
        "ES certificate sizes match public layouts",
        certificates
            .iter()
            .all(|(kind, size)| es::certificate_size(*kind) == *size)
            && es::certificate_size(u32::MAX) == 0,
    );
    logger.record(
        "ES certificate types accept only supported values",
        certificates
            .iter()
            .all(|(kind, _)| es::is_valid_cert(*kind))
            && !es::is_valid_cert(u32::MAX),
    );

    logger.record(
        "ES signed certificate sizes combine signature and certificate",
        signatures.iter().all(|(signature, signature_bytes)| {
            certificates.iter().all(|(certificate, certificate_bytes)| {
                es::signed_cert_size(*signature, *certificate)
                    == signature_bytes + certificate_bytes
            })
        }),
    );
    logger.record(
        "ES signed ticket sizes include signature",
        signatures.iter().all(|(signature, signature_bytes)| {
            es::signed_tik_size(*signature) == core::mem::size_of::<es::Tik>() + signature_bytes
        }),
    );

    let tmd_base = core::mem::size_of::<es::Tmd>();
    let content_size = core::mem::size_of::<es::TmdContent>();
    let tmd_count = 3u16;
    logger.record(
        "ES title metadata sizes match public layouts",
        es::tmd_size(0) == tmd_base
            && es::tmd_size(tmd_count) == tmd_base + usize::from(tmd_count) * content_size,
    );
    logger.record(
        "ES signed title metadata adds signature size",
        signatures.iter().all(|(signature, signature_bytes)| {
            es::signed_tmd_size(*signature, tmd_count) == es::tmd_size(tmd_count) + signature_bytes
        }),
    );
}

fn roundtrip_counter_bias() -> bool {
    let value = system::get_counter_bias();
    system::set_counter_bias(value);
    system::get_counter_bias() == value
}

fn roundtrip_display_offset() -> bool {
    let value = system::get_display_offset_h();
    system::set_display_offset_h(value);
    system::get_display_offset_h() == value
}

fn roundtrip_eu_rgb60() -> bool {
    let value = system::get_eu_rgb60();
    system::set_eu_rgb60(value);
    system::get_eu_rgb60() == value
}

fn roundtrip_language() -> bool {
    let value = system::get_language();
    system::set_language(value);
    system::get_language() == value
}

fn roundtrip_progressive_scan() -> bool {
    let value = system::get_progressive_scan();
    system::set_progressive_scan(value);
    system::get_progressive_scan() == value
}

fn roundtrip_sound_mode() -> bool {
    let value = system::get_sound_mode();
    system::set_sound_mode(value);
    system::get_sound_mode() == value
}

fn roundtrip_video_mode() -> bool {
    let value = system::get_video_mode();
    system::set_video_mode(value);
    system::get_video_mode() == value
}
