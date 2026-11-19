use ogc_es_sys as es;
use ogc_system::{self as system};
use ogc_video as video;

use crate::probe_support::ProbeLogger;

pub fn run(logger: &mut ProbeLogger) {
    logger.section("GENERAL PROBE");
    probe_es_layout(logger);
    probe_es_certificate_chain(logger);
    probe_wpad_speaker_encoder(logger);
    probe_bluetooth_idle_host(logger);
    probe_break_callback(logger);

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

fn probe_es_certificate_chain(logger: &mut ProbeLogger) {
    // Values are the public C ABI sizes, not calculated by the Rust helpers.
    let signatures = [
        (es::ES_SIG_RSA4096, 0x240usize),
        (es::ES_SIG_RSA2048, 0x140),
        (es::ES_SIG_ECDSA, 0x80),
    ];
    let certificates = [
        (es::ES_CERT_RSA4096, 0x2c0usize),
        (es::ES_CERT_RSA2048, 0x1c0),
        (es::ES_CERT_ECDSA, 0x100),
    ];
    let mut chain = [0u32; 321];
    let bytes = unsafe {
        core::slice::from_raw_parts_mut(chain.as_mut_ptr().cast::<u8>(), chain.len() * 4)
    };
    let mut passed = true;
    for (signature, signature_len) in signatures {
        for (certificate, certificate_len) in certificates {
            bytes.fill(0);
            bytes[..4].copy_from_slice(&signature.to_be_bytes());
            bytes[signature_len + 64..signature_len + 68]
                .copy_from_slice(&certificate.to_be_bytes());
            let first = bytes.as_ptr().cast::<es::SignedBlob>();
            // SAFETY: The initialized buffer contains a complete signed certificate,
            // and the expected next pointer remains inside the allocation.
            let next = unsafe { es::next_cert(first) };
            passed &= next as usize == first as usize + signature_len + certificate_len;
        }
    }
    logger.record(
        "ES next certificate follows all supported ABI layouts",
        passed,
    );
    bytes.fill(0);
    bytes[..4].copy_from_slice(&es::ES_SIG_RSA2048.to_be_bytes());
    bytes[0x140 + 64..0x140 + 68].copy_from_slice(&u32::MAX.to_be_bytes());
    // SAFETY: The signature and complete certificate header are readable.
    let invalid_type = unsafe { es::next_cert(bytes.as_ptr().cast()) };
    logger.record(
        "ES next certificate rejects unknown certificate type",
        invalid_type.is_null(),
    );
    bytes[..4].copy_from_slice(&u32::MAX.to_be_bytes());
    // SAFETY: The signature type is readable, and null is allowed.
    let invalid_signature = unsafe { es::next_cert(bytes.as_ptr().cast()) };
    let null_chain = unsafe { es::next_cert(core::ptr::null()) };
    logger.record(
        "ES next certificate rejects invalid signature and null",
        invalid_signature.is_null() && null_chain.is_null(),
    );
}

fn probe_bluetooth_idle_host(logger: &mut ProbeLogger) {
    use ogc_bt_sync::{Failure, HostStage, WiiHidHost};

    let mut host = WiiHidHost::new();
    logger.record(
        "Bluetooth idle host starts disabled",
        host.diagnostics().stage == HostStage::Disabled && !host.shutdown_complete(),
    );
    logger.record(
        "Bluetooth idle host rejects search and report",
        !host.search(10) && !host.send_report(0, &[0xa2, 1]),
    );
    host.stop_search();
    host.disconnect(0);
    host.disconnect(usize::MAX);
    logger.record(
        "Bluetooth idle host rejects disconnect request",
        host.request_disconnect(0) == Err(Failure::Protocol)
            && host.request_disconnect(usize::MAX) == Err(Failure::Protocol)
            && host.switch_calibration(0).is_none(),
    );
    logger.record(
        "Bluetooth idle host has no queued events",
        host.peek_event().is_none() && host.next_event().is_none(),
    );
    host.poll(11);
    logger.record(
        "Bluetooth idle host poll leaves it disabled",
        host.diagnostics().stage == HostStage::Disabled && host.next_event().is_none(),
    );
    host.shutdown(12);
    logger.record(
        "Bluetooth idle host shutdown completes without adapter",
        host.shutdown_complete() && host.diagnostics().stage == HostStage::Closing,
    );
    host.poll(13);
    logger.record(
        "Bluetooth closed host remains complete after poll",
        host.shutdown_complete() && host.peek_event().is_none(),
    );
}

fn probe_wpad_speaker_encoder(logger: &mut ProbeLogger) {
    use wpad_sys as wpad;

    // Expected bytes and final state come from libogc's C speaker encoder.
    let samples = [0i16, 1000, -1000, 32767, -32768, 1234, 42, -42];
    let mut state = wpad::WpadEncStatus::default();
    let mut encoded = [0xa5; 5];
    // SAFETY: The input has eight samples and the output has room for four bytes.
    unsafe { wpad::encode_data(&mut state, 0, samples.as_ptr(), 8, encoded.as_mut_ptr()) };
    logger.record(
        "WPAD speaker encoding matches C reference",
        encoded == [0x07, 0xf7, 0xf3, 0x98, 0xa5]
            && i32::from_be_bytes(state.data[0..4].try_into().unwrap()) == -394
            && i32::from_be_bytes(state.data[8..12].try_into().unwrap()) == 3039,
    );

    let mut split_state = wpad::WpadEncStatus::default();
    let mut split = [0u8; 4];
    // SAFETY: Each call has four samples and two output bytes.
    unsafe {
        wpad::encode_data(&mut split_state, 0, samples.as_ptr(), 4, split.as_mut_ptr());
        wpad::encode_data(
            &mut split_state,
            wpad::WPAD_ENC_CONT,
            samples[4..].as_ptr(),
            4,
            split[2..].as_mut_ptr(),
        );
    }
    logger.record(
        "WPAD speaker continuation preserves encoder state",
        split == encoded[..4]
            && split_state.data[0..4] == state.data[0..4]
            && split_state.data[8..12] == state.data[8..12],
    );

    let mut odd_state = wpad::WpadEncStatus::default();
    let mut odd = [0xa5; 3];
    // SAFETY: Three input samples produce two output bytes; the final sample is padded with zero.
    unsafe { wpad::encode_data(&mut odd_state, 0, samples.as_ptr(), 3, odd.as_mut_ptr()) };
    logger.record(
        "WPAD speaker odd sample uses zero padding",
        odd == [0x07, 0xf1, 0xa5],
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

unsafe extern "C" fn probe_break_callback_a() {}
unsafe extern "C" fn probe_break_callback_b() {}

fn probe_break_callback(logger: &mut ProbeLogger) {
    use ogc_gx as gx;

    let original = gx::set_break_pt_callback(Some(probe_break_callback_a));
    let previous_a = gx::set_break_pt_callback(Some(probe_break_callback_b));
    let previous_b = gx::set_break_pt_callback(None);
    let previous_none = gx::set_break_pt_callback(original);
    logger.record(
        "GX breakpoint callback replacement and restoration",
        previous_a.map(|callback| callback as usize)
            == Some(probe_break_callback_a as *const () as usize)
            && previous_b.map(|callback| callback as usize)
                == Some(probe_break_callback_b as *const () as usize)
            && previous_none.is_none(),
    );
}
