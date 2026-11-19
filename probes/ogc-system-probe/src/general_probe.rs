use ogc_es_sys as es;
use ogc_system::{self as system};
use ogc_video as video;

use crate::probe_support::ProbeLogger;

pub fn run(logger: &mut ProbeLogger) {
    logger.section("GENERAL PROBE");
    probe_es_layout(logger);
    probe_es_certificate_chain(logger);
    probe_es_catalog_paths(logger);
    probe_es_disc_views(logger);
    probe_es_device_identity(logger);
    probe_es_content_invalid(logger);
    probe_es_crypto_inputs(logger);
    probe_es_crypto_vector(logger);
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

fn probe_es_crypto_vector(logger: &mut ProbeLogger) {
    const IV: [u32; 4] = [0x0001_0203, 0x0405_0607, 0x0809_0a0b, 0x0c0d_0e0f];
    const PLAINTEXT: [u8; 32] = [
        0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e,
        0x0f, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d,
        0x1e, 0x1f,
    ];
    // Independently computed AES-128-CBC vector using the retail common key.
    const CIPHERTEXT: [u8; 32] = [
        0xaa, 0x9e, 0x6e, 0x32, 0xce, 0x5e, 0xad, 0x73, 0xae, 0x09, 0x6f, 0xda, 0xcc, 0xee, 0xd2,
        0xe6, 0xae, 0xf2, 0xcf, 0xb8, 0x27, 0x42, 0xb7, 0xf8, 0x8a, 0x08, 0x35, 0xce, 0x12, 0x36,
        0x72, 0xc5,
    ];
    const FINAL_IV: [u32; 4] = [0xaef2_cfb8, 0x2742_b7f8, 0x8a08_35ce, 0x1236_72c5];

    #[repr(align(32))]
    struct Buffer([u8; 64]);
    let initialized = es::init();
    logger.record("ES crypto vector opens service", initialized == 0);
    if initialized != 0 {
        return;
    }
    let mut input = Buffer([0xa5; 64]);
    input.0[..32].copy_from_slice(&PLAINTEXT);
    let mut output = Buffer([0xa5; 64]);
    let mut iv = IV;
    // SAFETY: All three buffers are live, separate, aligned, and sized for
    // the requested two AES blocks. The key handle belongs to Dolphin.
    let encrypted = unsafe {
        es::encrypt(
            4,
            iv.as_mut_ptr(),
            input.0.as_ptr(),
            32,
            output.0.as_mut_ptr(),
        )
    };
    logger.note_i32("ES crypto vector encrypt result", encrypted);
    logger.record(
        "ES encrypt matches independent CBC vector and updates IV",
        encrypted == 0
            && output.0[..32] == CIPHERTEXT
            && output.0[32..] == [0xa5; 32]
            && iv == FINAL_IV,
    );

    input.0[..32].copy_from_slice(&CIPHERTEXT);
    output.0.fill(0xa5);
    iv = IV;
    let decrypted = unsafe {
        es::decrypt(
            4,
            iv.as_mut_ptr(),
            input.0.as_ptr(),
            32,
            output.0.as_mut_ptr(),
        )
    };
    logger.note_i32("ES crypto vector decrypt result", decrypted);
    logger.record(
        "ES decrypt matches independent CBC vector and updates IV",
        decrypted == 0
            && output.0[..32] == PLAINTEXT
            && output.0[32..] == [0xa5; 32]
            && iv == FINAL_IV,
    );
    logger.record("ES crypto vector closes service", es::close() == 0);
}

fn probe_es_crypto_inputs(logger: &mut ProbeLogger) {
    let initialized = es::init();
    logger.record("ES crypto probe opens service", initialized == 0);
    if initialized != 0 {
        return;
    }

    #[repr(align(32))]
    struct Bytes([u8; 416]);
    let mut cert = Bytes([0xa5; 416]);
    // SAFETY: The buffer contains 0x180 writable bytes followed by a guard.
    let cert_rc = unsafe { es::get_device_cert(cert.0.as_mut_ptr()) };
    let null_cert = unsafe { es::get_device_cert(core::ptr::null_mut()) };
    let unaligned_cert = unsafe { es::get_device_cert(cert.0.as_mut_ptr().add(1)) };
    logger.record(
        "ES device certificate has ECC format and bounded output",
        cert_rc == 0
            && cert.0[..4] == [0, 1, 0, 2]
            && cert.0[0xc0..0xc4] == [0, 0, 0, 2]
            && &cert.0[0xc4..0xce] == b"NG06eeed45"
            && cert.0[0x180..].iter().all(|&byte| byte == 0xa5)
            && null_cert == es::ES_EINVAL
            && unaligned_cert == es::ES_EALIGN,
    );

    let mut iv = [0x1234_5678u32; 4];
    let source = Bytes([0x3c; 416]);
    let mut dest = Bytes([0xa5; 416]);
    // SAFETY: The pointers are live and the 16-byte input and output ranges
    // are valid. Each call is rejected before IOS uses those ranges.
    let encrypt_empty = unsafe {
        es::encrypt(
            0,
            iv.as_mut_ptr(),
            source.0.as_ptr(),
            0,
            dest.0.as_mut_ptr(),
        )
    };
    let encrypt_size = unsafe {
        es::encrypt(
            0,
            iv.as_mut_ptr(),
            source.0.as_ptr(),
            15,
            dest.0.as_mut_ptr(),
        )
    };
    let encrypt_null = unsafe {
        es::encrypt(
            0,
            core::ptr::null_mut(),
            source.0.as_ptr(),
            16,
            dest.0.as_mut_ptr(),
        )
    };
    logger.record(
        "ES encrypt rejects invalid buffers without mutation",
        encrypt_empty == es::ES_EINVAL
            && encrypt_size == es::ES_EALIGN
            && encrypt_null == es::ES_EINVAL
            && iv == [0x1234_5678; 4]
            && dest.0 == [0xa5; 416],
    );
    let decrypt_empty = unsafe {
        es::decrypt(
            0,
            iv.as_mut_ptr(),
            source.0.as_ptr(),
            0,
            dest.0.as_mut_ptr(),
        )
    };
    let decrypt_size = unsafe {
        es::decrypt(
            0,
            iv.as_mut_ptr(),
            source.0.as_ptr(),
            15,
            dest.0.as_mut_ptr(),
        )
    };
    let decrypt_null = unsafe {
        es::decrypt(
            0,
            core::ptr::null_mut(),
            source.0.as_ptr(),
            16,
            dest.0.as_mut_ptr(),
        )
    };
    logger.record(
        "ES decrypt rejects invalid buffers without mutation",
        decrypt_empty == es::ES_EINVAL
            && decrypt_size == es::ES_EALIGN
            && decrypt_null == es::ES_EINVAL
            && iv == [0x1234_5678; 4]
            && dest.0 == [0xa5; 416],
    );
    let initial_iv = [0x1234_5678u32; 4];
    iv = initial_iv;
    dest.0.fill(0xa5);
    // SAFETY: The 16-byte source and destination are separately aligned,
    // writable for IOS output where appropriate, and live through each call.
    let invalid_key_encrypt = unsafe {
        es::encrypt(
            u32::MAX,
            iv.as_mut_ptr(),
            source.0.as_ptr(),
            16,
            dest.0.as_mut_ptr(),
        )
    };
    logger.record(
        "ES encrypt invalid key preserves IV and destination",
        invalid_key_encrypt < 0 && iv == initial_iv && dest.0 == [0xa5; 416],
    );
    logger.note_i32("ES encrypt invalid key result", invalid_key_encrypt);

    iv = initial_iv;
    dest.0.fill(0xa5);
    let invalid_key_decrypt = unsafe {
        es::decrypt(
            u32::MAX,
            iv.as_mut_ptr(),
            source.0.as_ptr(),
            16,
            dest.0.as_mut_ptr(),
        )
    };
    logger.record(
        "ES decrypt invalid key preserves IV and destination",
        invalid_key_decrypt < 0 && iv == initial_iv && dest.0 == [0xa5; 416],
    );
    logger.note_i32("ES decrypt invalid key result", invalid_key_decrypt);
    logger.record("ES crypto probe closes service", es::close() == 0);
}

fn probe_es_content_invalid(logger: &mut ProbeLogger) {
    let initialized = es::init();
    logger.record("ES content validation opens service", initialized == 0);
    if initialized != 0 {
        return;
    }

    #[repr(align(32))]
    struct Buffer([u8; 64]);
    let mut buffer = Buffer([0xa5; 64]);
    // SAFETY: The aligned buffer and its one-byte-offset pointer have
    // sufficient writable storage for every requested read.
    let invalid_fd = unsafe { es::read_content(-1, buffer.0.as_mut_ptr(), 32) };
    let null = unsafe { es::read_content(1, core::ptr::null_mut(), 32) };
    let empty = unsafe { es::read_content(1, buffer.0.as_mut_ptr(), 0) };
    let unaligned = unsafe { es::read_content(1, buffer.0.as_mut_ptr().add(1), 32) };
    logger.record(
        "ES content read rejects invalid inputs and preserves output",
        invalid_fd == es::ES_EINVAL
            && null == es::ES_EINVAL
            && empty == es::ES_EINVAL
            && unaligned == es::ES_EALIGN
            && buffer.0 == [0xa5; 64],
    );
    logger.record(
        "ES content seek rejects negative descriptor",
        es::seek_content(-1, 0, 0) == es::ES_EINVAL,
    );
    logger.record(
        "ES content close rejects negative descriptor",
        es::close_content(-1) == es::ES_EINVAL,
    );
    logger.record("ES content validation closes service", es::close() == 0);
}

fn probe_es_device_identity(logger: &mut ProbeLogger) {
    let initialized = es::init();
    logger.record("ES identity probe opens service", initialized == 0);
    if initialized != 0 {
        return;
    }

    let mut device = u32::MAX;
    // SAFETY: The output pointers refer to live writable scalars.
    let device_rc = unsafe { es::get_device_id(&mut device) };
    let null_device = unsafe { es::get_device_id(core::ptr::null_mut()) };
    logger.record(
        "ES device ID matches imported NAND and rejects null",
        device_rc == 0 && device == 0x06ee_ed45 && null_device == es::ES_EINVAL,
    );

    let mut title = 0x1234_5678_9abc_def0u64;
    let title_rc = unsafe { es::get_title_id(&mut title) };
    let null_title = unsafe { es::get_title_id(core::ptr::null_mut()) };
    logger.record(
        "ES title ID unavailable preserves output and rejects null",
        title_rc == -1017 && title == 0x1234_5678_9abc_def0 && null_title == es::ES_EINVAL,
    );
    logger.record(
        "ES Korean key query returns IOS error",
        es::check_has_korean_key() == -1017,
    );
    logger.record("ES identity probe closes service", es::close() == 0);
}

fn probe_es_disc_views(logger: &mut ProbeLogger) {
    const MENU: u64 = 0x0000_0001_0000_0002;
    #[repr(align(64))]
    struct Tmd([u8; 832]);
    #[repr(align(32))]
    struct View([u8; 256]);

    let initialized = es::init();
    logger.record("ES disc view probe opens service", initialized == 0);
    if initialized != 0 {
        return;
    }

    let mut tmd = Tmd([0xa5; 832]);
    // SAFETY: TMD storage is aligned and holds the fixture's 808-byte TMD.
    let fetched = unsafe { es::get_stored_tmd(MENU, tmd.0.as_mut_ptr().cast(), 808) };
    let mut size = u32::MAX;
    let size_rc = if fetched == 0 {
        // SAFETY: The fetched TMD and scalar output remain live and aligned.
        unsafe { es::di_get_tmd_view_size(tmd.0.as_ptr().cast(), 808, &mut size) }
    } else {
        fetched
    };
    logger.record(
        "ES disc TMD view size matches system menu",
        fetched == 0 && size_rc == 0 && size == 236 && tmd.0[808..].iter().all(|&b| b == 0xa5),
    );

    let mut view = View([0xa5; 256]);
    // SAFETY: The output buffer is aligned and contains 236 writable bytes.
    let view_rc = if size_rc == 0 && size == 236 {
        unsafe { es::di_get_tmd_view(tmd.0.as_ptr().cast(), 808, view.0.as_mut_ptr().cast(), size) }
    } else {
        size_rc
    };
    // SAFETY: A null output is passed to verify rejection before use.
    let null_rc =
        unsafe { es::di_get_tmd_view(tmd.0.as_ptr().cast(), 808, core::ptr::null_mut(), 236) };
    logger.record(
        "ES disc TMD view matches menu and rejects null",
        fetched == 0
            && size_rc == 0
            && size == 236
            && view_rc == 0
            && view.0[12..20] == MENU.to_be_bytes()
            && view.0[90..92] == [0, 9]
            && view.0[236..].iter().all(|&b| b == 0xa5)
            && null_rc == es::ES_EINVAL,
    );

    let mut missing_size = u32::MAX;
    // SAFETY: A null TMD requests the current title, and outputs are writable.
    let missing_rc = unsafe { es::di_get_tmd_view_size(core::ptr::null(), 0, &mut missing_size) };
    let mut ticket = View([0xa5; 256]);
    let ticket_rc = unsafe { es::get_ticket_view(core::ptr::null(), ticket.0.as_mut_ptr().cast()) };
    let null_ticket = unsafe { es::get_ticket_view(core::ptr::null(), core::ptr::null_mut()) };
    logger.record(
        "ES disc view size reports no active title",
        missing_rc == -1017 && missing_size == u32::MAX,
    );
    logger.record(
        "ES disc ticket view reports no active title",
        ticket_rc == -1017 && null_ticket == es::ES_EINVAL && ticket.0 == [0xa5; 256],
    );
    logger.record("ES disc view probe closes service", es::close() == 0);
}

fn probe_es_catalog_paths(logger: &mut ProbeLogger) {
    const MENU: u64 = 0x0000_0001_0000_0002;
    let initialized = es::init();
    logger.record(
        "ES catalog opens for path and content checks",
        initialized == 0,
    );
    if initialized != 0 {
        return;
    }

    let mut path = [0xa5u8; 32];
    // SAFETY: The output has 30 writable bytes and two guard bytes.
    let directory = unsafe { es::get_data_dir(MENU, path.as_mut_ptr()) };
    logger.record(
        "ES system menu data path matches NAND",
        directory == 0
            && &path[..29] == b"/title/00000001/00000002/data"
            && path[29..] == [0xa5; 3],
    );

    #[repr(align(64))]
    struct Tmd([u8; 832]);
    let mut tmd = Tmd([0xa5; 832]);
    // SAFETY: The aligned buffer accommodates the fixture's 808-byte TMD.
    let fetched = unsafe { es::get_stored_tmd(MENU, tmd.0.as_mut_ptr().cast(), 808) };
    let mut contents = [u32::MAX; 10];
    let listed = if fetched == 0 {
        // SAFETY: The signed TMD and output buffer remain live and aligned.
        unsafe { es::get_stored_tmd_contents(tmd.0.as_ptr().cast(), 808, contents.as_mut_ptr(), 9) }
    } else {
        fetched
    };
    logger.record(
        "ES rejected stored content list preserves output",
        fetched == 0 && listed == -2011 && contents.iter().all(|&word| word == u32::MAX),
    );
    logger.record(
        "ES catalog closes after path and content checks",
        es::close() == 0,
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
