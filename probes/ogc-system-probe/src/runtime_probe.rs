use core::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use ogc_aes_sys as aes;
use ogc_conf as conf;
use ogc_es as es;
use ogc_filesystem as filesystem;
use ogc_ios as ios;
use ogc_lwp::sys as lwp;
use ogc_sha as sha;
use ogc_stm as stm;
use ogc_system as system;

use crate::probe_support::ProbeLogger;

#[repr(align(16))]
struct Align16([u8; 16]);

pub fn run(logger: &mut ProbeLogger) {
    logger.section("RUNTIME PROBE");

    probe_owned_audio_buffer(logger);
    probe_l2cap_packets(logger);
    probe_l2cap_parsers(logger);
    probe_hci_events(logger);
    probe_ui_adapters(logger);
    probe_system(logger);
    probe_ios_stm(logger);
    probe_conf(logger);
    probe_es(logger);
    probe_crypto(logger);
    probe_iso9660(logger);
}

fn probe_hci_events(logger: &mut ProbeLogger) {
    use ogc_bt_sync as bt;
    let complete = [0x0e, 6, 1, 3, 0x0c, 0, 0x12, 0x34, 0xee];
    let status = [0x0f, 4, 0x0c, 1, 3, 0x0c];
    let command_ok = matches!(bt::parse_event(&complete), Ok(bt::HciEvent::CommandComplete {
        opcode, status: 0, params: [0x12, 0x34] }) if opcode.raw() == 0x0c03)
        && matches!(bt::parse_event(&status), Ok(bt::HciEvent::CommandStatus {
            opcode, status: 0x0c }) if opcode.raw() == 0x0c03);
    let short_complete =
        (2..8).all(|n| bt::parse_event(&complete[..n]) == Err(bt::Error::MalformedEvent));
    let short_status =
        (2..6).all(|n| bt::parse_event(&status[..n]) == Err(bt::Error::MalformedEvent));
    logger.record(
        "HCI command events and truncated packets",
        command_ok && short_complete && short_status,
    );

    let connection = [3, 11, 0, 0x34, 0x02, 1, 2, 3, 4, 5, 6, 1, 0];
    let request = [4, 10, 1, 2, 3, 4, 5, 6, 0x11, 0x22, 0x33, 1];
    let disconnect = [5, 4, 0, 0x34, 0x02, 0x13];
    let connection_ok = matches!(bt::parse_event(&connection), Ok(bt::HciEvent::ConnectionComplete {
        status: 0, address, handle: 0x234, link_type: 1, encryption: 0 })
        if address.bytes == [1, 2, 3, 4, 5, 6]);
    let request_ok = matches!(bt::parse_event(&request), Ok(bt::HciEvent::ConnectionRequest {
        address, class_of_device: [0x11, 0x22, 0x33], link_type: 1 })
        if address.bytes == [1, 2, 3, 4, 5, 6]);
    let disconnect_ok = matches!(
        bt::parse_event(&disconnect),
        Ok(bt::HciEvent::DisconnectionComplete {
            status: 0,
            handle: 0x234,
            reason: 0x13
        })
    );
    logger.record(
        "HCI connection and disconnection events",
        connection_ok && request_ok && disconnect_ok,
    );

    let unknown = [0xfe, 2, 0xaa, 0xbb, 0xcc];
    let malformed = [3, 1, 0];
    logger.record(
        "HCI unknown and malformed events",
        matches!(
            bt::parse_event(&unknown),
            Ok(bt::HciEvent::Unknown {
                event_code: 0xfe,
                payload: [0xaa, 0xbb]
            })
        ) && bt::parse_event(&[]) == Err(bt::Error::EventBufferTooSmall)
            && bt::parse_event(&[3]) == Err(bt::Error::EventBufferTooSmall)
            && bt::parse_event(&malformed) == Err(bt::Error::MalformedEvent),
    );
}

fn probe_l2cap_parsers(logger: &mut ProbeLogger) {
    use ogc_bt_sync as bt;

    // These bytes are fixed independently of the packet builders.
    let acl = [
        0x23, 0xa1, 12, 0, 8, 0, 1, 0, 2, 7, 4, 0, 0x11, 0, 0x40, 0, 0xee,
    ];
    let parsed_acl = bt::parse_acl_packet(&acl);
    let acl_ok = matches!(parsed_acl, Ok(p) if p.handle == 0x123
        && p.packet_boundary == 2 && p.broadcast == 2 && p.payload == &acl[4..16]);
    let acl_truncated =
        (0..16).all(|n| bt::parse_acl_packet(&acl[..n]) == Err(bt::Error::MalformedAcl));
    logger.record("ACL parsing fields and truncation", acl_ok && acl_truncated);

    let l2cap = &acl[4..16];
    let l2cap_ok = matches!(bt::parse_l2cap_packet(l2cap), Ok(p)
        if p.length == 8 && p.channel_id == 1 && p.payload == &acl[8..16]);
    let l2cap_truncated =
        (0..12).all(|n| bt::parse_l2cap_packet(&l2cap[..n]) == Err(bt::Error::MalformedL2cap));
    logger.record(
        "L2CAP parsing fields and truncation",
        l2cap_ok && l2cap_truncated,
    );

    let signal_bytes = &acl[8..16];
    let signal_ok = matches!(bt::parse_l2cap_signal(signal_bytes), Ok(s)
        if s.code == 2 && s.identifier == 7 && s.payload == [0x11, 0, 0x40, 0]);
    let signal_truncated = (0..8)
        .all(|n| bt::parse_l2cap_signal(&signal_bytes[..n]) == Err(bt::Error::MalformedL2cap));
    let request = bt::L2capSignal {
        code: 2,
        identifier: 7,
        payload: &[0x11, 0, 0x40, 0],
    };
    let request_ok = bt::parse_l2cap_signal_payload(request)
        == Ok(bt::L2capSignalPayload::ConnectionRequest {
            psm: 0x11,
            source_cid: 0x40,
        });
    let short = bt::L2capSignal {
        payload: &[0x11, 0, 0x40],
        ..request
    };
    let short_rejected = bt::parse_l2cap_signal_payload(short) == Err(bt::Error::MalformedL2cap);
    logger.record(
        "L2CAP signal parsing and payload",
        signal_ok && signal_truncated && request_ok && short_rejected,
    );

    let commands = [2, 7, 4, 0, 0x11, 0, 0x40, 0, 6, 9, 0, 0];
    let mut remaining = &commands[..];
    let first = bt::parse_next_l2cap_signal(&mut remaining);
    let second = bt::parse_next_l2cap_signal(&mut remaining);
    let end = bt::parse_next_l2cap_signal(&mut remaining);
    let mut incomplete = &commands[..7];
    let rejected = bt::parse_next_l2cap_signal(&mut incomplete);
    logger.record("L2CAP signal iteration and error cursor",
        matches!(first, Ok(Some(s)) if s == request)
        && matches!(second, Ok(Some(s)) if s.code == 6 && s.identifier == 9 && s.payload.is_empty())
        && end == Ok(None) && rejected == Err(bt::Error::MalformedL2cap)
        && incomplete == &commands[..7]);

    let hidp = [0xa1, 0x12, 0x34];
    logger.record(
        "HIDP parsing fields and empty input",
        matches!(bt::parse_hidp_packet(&hidp), Ok(p)
            if p.transaction == 0xa0 && p.parameter == 1 && p.payload == [0x12, 0x34])
            && bt::parse_hidp_packet(&[]) == Err(bt::Error::MalformedL2cap),
    );
}

fn probe_owned_audio_buffer(logger: &mut ProbeLogger) {
    logger.section("OWNED AUDIO BUFFER");
    let mut source = [0u8; 65];
    for (index, byte) in source.iter_mut().enumerate() {
        *byte = index as u8 ^ 0x5a;
    }
    let mut valid = true;
    for len in [0, 1, 31, 32, 33, 64, 65] {
        let expected = source;
        let buffer = match aesnd::OwnedAudioBuffer::copy_from_slice(&source[..len]) {
            Ok(buffer) => buffer,
            Err(_) => {
                valid = false;
                continue;
            }
        };
        source.fill(0);
        valid &= buffer.len() == len as u32
            && buffer.is_empty() == (len == 0)
            && buffer.as_bytes() == &expected[..len]
            && (buffer.as_bytes().as_ptr() as usize & 31) == 0;
        source = expected;
    }
    logger.record("owned audio buffers preserve aligned copied bytes", valid);
    let handle = aesnd::VoiceHandle::new(core::ptr::null_mut());
    let null = handle.is_null() && handle.as_ptr().is_null() && handle.into_raw().is_null();
    let mut storage = 0u32;
    let pointer = (&mut storage as *mut u32).cast::<aesnd::sys::Aesndpb>();
    let handle = aesnd::VoiceHandle::new(pointer);
    let nonnull = !handle.is_null() && handle.as_ptr() == pointer && handle.into_raw() == pointer;
    logger.record("audio voice handle pointer roundtrip", null && nonnull);
}

fn probe_l2cap_packets(logger: &mut ProbeLogger) {
    use ogc_bt_sync as bt;
    let mut bytes = [0xa5; 32];
    let n = bt::build_l2cap_connection_request(&mut bytes, 0x123, 7, 0x11, 0x40);
    logger.record(
        "L2CAP connection request bytes",
        n == Ok(16)
            && bytes[..16] == [0x23, 0x21, 12, 0, 8, 0, 1, 0, 2, 7, 4, 0, 0x11, 0, 0x40, 0]
            && bytes[16] == 0xa5,
    );
    bytes.fill(0xa5);
    let n = bt::build_l2cap_configuration_request(&mut bytes, 0x123, 8, 0x41);
    logger.record(
        "L2CAP configuration request bytes",
        n == Ok(16)
            && bytes[..16] == [0x23, 0x21, 12, 0, 8, 0, 1, 0, 4, 8, 4, 0, 0x41, 0, 0, 0]
            && bytes[16] == 0xa5,
    );
    bytes.fill(0xa5);
    let n = bt::build_l2cap_configuration_request_with_mtu(&mut bytes, 0x123, 9, 0x41, 0x2a0);
    logger.record(
        "L2CAP configuration MTU bytes",
        n == Ok(20)
            && bytes[..20]
                == [
                    0x23, 0x21, 16, 0, 12, 0, 1, 0, 4, 9, 8, 0, 0x41, 0, 0, 0, 1, 2, 0xa0, 2,
                ]
            && bytes[20] == 0xa5,
    );
    bytes.fill(0xa5);
    let n = bt::build_l2cap_connection_response(&mut bytes, 0x123, 10, 0x40, 0x50, 0);
    logger.record(
        "L2CAP connection response bytes and errors",
        n == Ok(20)
            && bytes[..20]
                == [
                    0x23, 0x21, 16, 0, 12, 0, 1, 0, 3, 10, 8, 0, 0x40, 0, 0x50, 0, 0, 0, 0, 0,
                ]
            && bytes[20] == 0xa5
            && bt::build_l2cap_connection_response(&mut bytes[..19], 0x123, 10, 0x40, 0x50, 0)
                == Err(bt::Error::BufferTooSmall)
            && bt::build_l2cap_connection_response(&mut bytes, 0x1000, 10, 0x40, 0x50, 0)
                == Err(bt::Error::MalformedAcl),
    );
    bytes.fill(0xa5);
    let n = bt::build_l2cap_configuration_response(&mut bytes, 0x123, 11, 0x40, 0);
    logger.record(
        "L2CAP configuration response bytes and errors",
        n == Ok(18)
            && bytes[..18]
                == [
                    0x23, 0x21, 14, 0, 10, 0, 1, 0, 5, 11, 6, 0, 0x40, 0, 0, 0, 0, 0,
                ]
            && bytes[18] == 0xa5
            && bt::build_l2cap_configuration_response(&mut bytes[..17], 0x123, 11, 0x40, 0)
                == Err(bt::Error::BufferTooSmall)
            && bt::build_l2cap_configuration_response(&mut bytes, 0x1000, 11, 0x40, 0)
                == Err(bt::Error::MalformedAcl),
    );
    bytes.fill(0xa5);
    let n = bt::build_l2cap_disconnection_response(&mut bytes, 0x123, 12, 0x40, 0x50);
    logger.record(
        "L2CAP disconnection response bytes and errors",
        n == Ok(16)
            && bytes[..16] == [0x23, 0x21, 12, 0, 8, 0, 1, 0, 7, 12, 4, 0, 0x40, 0, 0x50, 0]
            && bytes[16] == 0xa5
            && bt::build_l2cap_disconnection_response(&mut bytes[..15], 0x123, 12, 0x40, 0x50)
                == Err(bt::Error::BufferTooSmall)
            && bt::build_l2cap_disconnection_response(&mut bytes, 0x1000, 12, 0x40, 0x50)
                == Err(bt::Error::MalformedAcl),
    );
    bytes.fill(0xa5);
    let n = bt::build_l2cap_command_reject(&mut bytes, 0x123, 13, 2);
    logger.record(
        "L2CAP command reject bytes and errors",
        n == Ok(14)
            && bytes[..14] == [0x23, 0x21, 10, 0, 6, 0, 1, 0, 1, 13, 2, 0, 2, 0]
            && bytes[14] == 0xa5
            && bt::build_l2cap_command_reject(&mut bytes[..13], 0x123, 13, 2)
                == Err(bt::Error::BufferTooSmall)
            && bt::build_l2cap_command_reject(&mut bytes, 0x1000, 13, 2)
                == Err(bt::Error::MalformedAcl),
    );
    bytes.fill(0xa5);
    let n = bt::build_l2cap_invalid_cid_reject(&mut bytes, 0x123, 14, 0x40);
    logger.record(
        "L2CAP invalid CID reject bytes and errors",
        n == Ok(18)
            && bytes[..18]
                == [
                    0x23, 0x21, 14, 0, 10, 0, 1, 0, 1, 14, 6, 0, 2, 0, 0x40, 0, 0, 0,
                ]
            && bytes[18] == 0xa5
            && bt::build_l2cap_invalid_cid_reject(&mut bytes[..17], 0x123, 14, 0x40)
                == Err(bt::Error::BufferTooSmall)
            && bt::build_l2cap_invalid_cid_reject(&mut bytes, 0x1000, 14, 0x40)
                == Err(bt::Error::MalformedAcl),
    );
    bytes.fill(0xa5);
    let n = bt::build_l2cap_data(&mut bytes, 0x123, 0x42, &[0xaa, 0xbb]);
    logger.record(
        "L2CAP data bytes and errors",
        n == Ok(10)
            && bytes[..10] == [0x23, 0x21, 6, 0, 2, 0, 0x42, 0, 0xaa, 0xbb]
            && bytes[10] == 0xa5
            && bt::build_l2cap_data(&mut bytes[..9], 0x123, 0x42, &[0xaa, 0xbb])
                == Err(bt::Error::BufferTooSmall)
            && bt::build_l2cap_data(&mut bytes, 0x1000, 0x42, &[]) == Err(bt::Error::MalformedAcl),
    );
}

fn probe_ui_adapters(logger: &mut ProbeLogger) {
    use ogc_ui::{Direction, PadInputAdapter, UiInput, WpadInputAdapter};
    logger.section("UI INPUT ADAPTERS");
    let mut adapter = PadInputAdapter::default();
    let mut pad = ogc_input::PadStatus {
        button: ogc_input::PAD_BUTTON_A | ogc_input::PAD_BUTTON_UP,
        ..Default::default()
    };
    let first = adapter.update(&pad);
    let held = adapter.update(&pad);
    adapter.reset();
    let reset = adapter.update(&pad);
    pad.err = -1;
    let disconnected = adapter.update(&pad);
    pad.err = ogc_input::PAD_ERR_NONE as i8;
    let reconnected = adapter.update(&pad);
    logger.record(
        "UI PAD edges reset and reconnect",
        first.activate
            && first.direction == Some(Direction::Up)
            && held == UiInput::default()
            && reset == first
            && disconnected == UiInput::default()
            && reconnected == first,
    );

    let invalid = WpadInputAdapter::new(0, 480).is_err() && WpadInputAdapter::new(640, 0).is_err();
    let adapter = WpadInputAdapter::new(640, 480).expect("valid UI viewport");
    let mut sample = wpad_sys::WpadData {
        btns_d: wpad_sys::WPAD_BUTTON_A | wpad_sys::WPAD_BUTTON_LEFT,
        data_present: wpad_sys::WPAD_DATA_IR,
        ..Default::default()
    };
    sample.ir.valid = 1;
    sample.ir.x = 320.0;
    sample.ir.y = 240.0;
    sample.ir.vres = [640, 480];
    let mapped = adapter.update(&sample);
    sample.err = -1;
    let disconnected = adapter.update(&sample);
    logger.record(
        "UI WPAD edges pointer and errors",
        invalid
            && mapped.activate
            && mapped.direction == Some(Direction::Left)
            && mapped.pointer == Some((320, 240))
            && mapped.pointer_activate
            && disconnected == UiInput::default(),
    );
}

fn probe_system(logger: &mut ProbeLogger) {
    // The runtime has already initialized IOS. Repeating pre_main closes
    // live descriptors and invalidates the services this probe must test.
    logger.note_hex32("hollywood revision", system::get_hollywood_revision());
    let revision = ogc_core::powerpc::hollywood_revision();
    logger.record(
        "core hollywood revision matches system",
        revision == system::get_hollywood_revision(),
    );
    let timebase_before = ogc_core::powerpc::read_timebase();
    let immediate = ogc_core::powerpc::read_timebase();
    // A timebase tick need not occur between adjacent reads. Bound polling by
    // iterations so a stopped timebase cannot hang its own test.
    let mut timebase_after = immediate;
    for _ in 0..100_000 {
        if timebase_after != timebase_before {
            break;
        }
        core::hint::spin_loop();
        timebase_after = ogc_core::powerpc::read_timebase();
    }
    let timebase_delta = timebase_after.wrapping_sub(timebase_before);
    logger.note_u64(
        "PowerPC immediate delta",
        immediate.wrapping_sub(timebase_before),
    );
    logger.note_u64("PowerPC timebase", timebase_after);
    logger.note_u64("PowerPC timebase delta", timebase_delta);
    logger.record(
        "PowerPC timebase advances",
        timebase_delta > 0 && timebase_delta < (1u64 << 63),
    );
    logger.note_u32("font encoding", system::get_font_encoding());
    logger.note_u32("arena1 size", system::get_arena1_size());
    logger.note_u32("arena2 size", system::get_arena2_size());
    logger.note_u64("system time", system::time());
    system::dump_pmc();
    let pmc = system::sys::snapshot_pmc();
    logger.record("performance counters read", pmc.dumped);
    logger.note_u32("pmc load/stores", pmc.pmc1);
    logger.note_u32("pmc miss cycles", pmc.pmc2);
    logger.note_u32("pmc cycles", pmc.pmc3);
    logger.note_u32("pmc instructions", pmc.pmc4);
}

fn probe_ios_stm(logger: &mut ProbeLogger) {
    logger.section("IOS STM PROBE");
    logger.note_i32("ios preferred", ios::get_preferred_version());
    logger.note_i32("ios version", ios::get_version());
    logger.note_i32("ios revision", ios::get_revision());
    logger.note_i32("ios revision major", ios::get_revision_major());
    logger.note_i32("ios revision minor", ios::get_revision_minor());

    let led = stm::set_led_mode(0);
    logger.note_i32("stm led rc", led);
    logger.record("stm reachable", led == 0);
}

fn probe_conf(logger: &mut ProbeLogger) {
    logger.section("CONF PROBE");
    let init = conf::init();
    logger.note_i32("conf init rc", init);
    if init != conf::CONF_ERR_OK {
        return;
    }

    logger.note_i32("conf video", conf::get_video());
    logger.note_i32("conf region", conf::get_region());
    logger.note_i32("conf area", conf::get_area());
    logger.note_i32("conf language", conf::get_language());
    logger.note_i32("conf sound", conf::get_sound_mode());
    logger.note_i32("conf aspect", conf::get_aspect_ratio());
    logger.note_i32("conf progressive", conf::get_progressive_scan());
    logger.note_i32("conf sensorbar", conf::get_sensor_bar_position());

    let mut bias = 0u32;
    let bias_rc = conf::get_counter_bias(&mut bias);
    logger.note_i32("conf counter bias rc", bias_rc);
    logger.note_u32("conf counter bias", bias);

    let mut offset = 0i8;
    let offset_rc = conf::get_display_offset_h(&mut offset);
    logger.note_i32("conf display offset rc", offset_rc);
    logger.note_i32("conf display offset", offset as i32);

    let mut pads = conf::ConfPads::default();
    let pads_rc = conf::get_pad_devices(&mut pads);
    logger.note_i32("conf pad devices rc", pads_rc);
    logger.note_u32("conf registered pads", pads.num_registered as u32);
}

fn probe_es(logger: &mut ProbeLogger) {
    logger.section("ES PROBE");
    let init = es::init();
    logger.note_i32("es init rc", init);
    if init < 0 {
        return;
    }

    let mut title_id = 0u64;
    // SAFETY: each argument below is a valid unique output reference.
    logger.note_i32("es title id rc", unsafe { es::get_title_id(&mut title_id) });
    logger.note_hex32("es title hi", (title_id >> 32) as u32);
    logger.note_hex32("es title lo", title_id as u32);

    let mut device_id = 0u32;
    logger.note_i32("es device id rc", unsafe {
        es::get_device_id(&mut device_id)
    });
    logger.note_hex32("es device id", device_id);

    let mut boot2 = 0u32;
    logger.note_i32("es boot2 rc", unsafe { es::get_boot2_version(&mut boot2) });
    logger.note_u32("es boot2", boot2);
    logger.note_i32("es korean key rc", es::check_has_korean_key());
    logger.note_i32("es close rc", es::close());
}

fn probe_crypto(logger: &mut ProbeLogger) {
    logger.section("CRYPTO PROBE");

    let sha_init = sha::init();
    logger.record("sha init", sha_init == 0);

    let mut context = sha::AlignedShaContext::new();
    let aligned = (context.as_ptr() as usize) & 31 == 0;
    let mutable_pointer_matches = context.as_mut_ptr() == &mut context.context;
    let mut plain_context = sha::ShaContext::new();
    plain_context.set_bit_length(u64::MAX);
    let length_roundtrip = plain_context.bit_length() == u64::MAX;

    let mut context_digest = [0u8; 20];
    let full_block = [b'a'; 64];
    let context_rc = sha::initialize_context(&mut context);
    let input_rc = sha::input(&mut context, &full_block);
    let length_updated = context.context.bit_length() == 512;
    let finalize_rc = sha::finalize(&mut context, b"x", &mut context_digest);
    let expected_digest = [
        0x8e, 0x18, 0x94, 0x0b, 0x19, 0xbe, 0xf3, 0x15, 0xd3, 0x42, 0x61, 0xeb, 0xf8, 0x26, 0x6d,
        0xd2, 0x9a, 0x1f, 0x3e, 0x66,
    ];
    logger.record(
        "sha context helpers",
        aligned
            && mutable_pointer_matches
            && length_roundtrip
            && context_rc == 0
            && input_rc == 0
            && length_updated
            && finalize_rc == 0
            && context_digest == expected_digest,
    );

    let mut digest = [0u8; 20];
    logger.record("sha calculate", sha::calculate(b"abc", &mut digest) == 0);
    logger.record(
        "sha abc vector",
        digest
            == [
                0xa9, 0x99, 0x3e, 0x36, 0x47, 0x06, 0x81, 0x6a, 0xba, 0x3e, 0x25, 0x71, 0x78, 0x50,
                0xc2, 0x6c, 0x9c, 0xd0, 0xd8, 0x9d,
            ],
    );
    logger.record("sha close", sha::close() == 0);

    let key = Align16([0; 16]);
    let mut iv = Align16([0; 16]);
    let input = Align16([0; 16]);
    let mut output = Align16([0; 16]);
    // SAFETY: all buffers are 16-byte aligned, correctly sized, live for the
    // call, and occupy distinct storage.
    let aes_rc = unsafe {
        aes::aes_encrypt(aes::AesCommandConfig {
            key: key.0.as_ptr().cast(),
            key_size: 15,
            iv: iv.0.as_mut_ptr().cast(),
            iv_size: 16,
            in_data: input.0.as_ptr().cast(),
            out_data: output.0.as_mut_ptr().cast(),
            data_size: 16,
        })
    };
    logger.note_i32("aes bad key rc", aes_rc);
    logger.record("aes rejects bad key size", aes_rc == -4);

    let init = aes::aes_init();
    logger.note_i32("aes init rc", init);
    if init == 0 {
        let mut encrypt_iv = Align16([0; 16]);
        let mut decrypt_iv = Align16([0; 16]);
        let mut ciphertext = Align16([0; 16]);
        let mut decrypted = Align16([0; 16]);
        let plaintext = Align16(*b"Rust AES test!!!");
        // SAFETY: all buffers are 16-byte aligned, correctly sized, live for
        // each call, and writable buffers do not alias other arguments.
        let encrypt_rc = unsafe {
            aes::aes_encrypt(aes::AesCommandConfig {
                key: key.0.as_ptr().cast(),
                key_size: 16,
                iv: encrypt_iv.0.as_mut_ptr().cast(),
                iv_size: 16,
                in_data: plaintext.0.as_ptr().cast(),
                out_data: ciphertext.0.as_mut_ptr().cast(),
                data_size: 16,
            })
        };
        let decrypt_rc = if encrypt_rc == 0 {
            // SAFETY: the inputs and outputs meet the same requirements as
            // the encrypt call above, with a reset IV for the block.
            unsafe {
                aes::aes_decrypt(aes::AesCommandConfig {
                    key: key.0.as_ptr().cast(),
                    key_size: 16,
                    iv: decrypt_iv.0.as_mut_ptr().cast(),
                    iv_size: 16,
                    in_data: ciphertext.0.as_ptr().cast(),
                    out_data: decrypted.0.as_mut_ptr().cast(),
                    data_size: 16,
                })
            }
        } else {
            -1
        };
        logger.record(
            "aes encrypt/decrypt roundtrip",
            encrypt_rc == 0 && decrypt_rc == 0 && decrypted.0 == plaintext.0,
        );
        logger.note_i32("aes close rc", aes::aes_close());
    }
}

// A controlled read-only medium exercises the production mount path.
static ISO_CASE: core::sync::atomic::AtomicU8 = core::sync::atomic::AtomicU8::new(0);
static ISFS_CALLBACK_DONE: AtomicBool = AtomicBool::new(false);
static ISFS_CALLBACK_RESULT: AtomicI32 = AtomicI32::new(i32::MIN);

unsafe extern "C" fn isfs_probe_callback(result: i32, _userdata: *mut core::ffi::c_void) -> i32 {
    ISFS_CALLBACK_RESULT.store(result, Ordering::Release);
    ISFS_CALLBACK_DONE.store(true, Ordering::Release);
    filesystem::ISFS_OK
}

fn reset_isfs_callback() {
    ISFS_CALLBACK_RESULT.store(i32::MIN, Ordering::Release);
    ISFS_CALLBACK_DONE.store(false, Ordering::Release);
}

fn wait_isfs_callback() -> bool {
    for _ in 0..3000 {
        if ISFS_CALLBACK_DONE.load(Ordering::Acquire) {
            return true;
        }
        lwp::sleep_ticks(1);
    }
    ISFS_CALLBACK_DONE.load(Ordering::Acquire)
}

unsafe extern "C" fn iso_ready() -> bool {
    true
}

unsafe extern "C" fn iso_read(sector: u32, count: u32, buffer: *mut core::ffi::c_void) -> bool {
    use core::sync::atomic::Ordering;
    let case = ISO_CASE.load(Ordering::Relaxed);
    if count != 1 || buffer.is_null() || (case == 1 && sector == 16) || (case == 2 && sector == 20)
    {
        return false;
    }
    // SAFETY: the disc interface requests one writable 2048-byte sector.
    let bytes = unsafe { core::slice::from_raw_parts_mut(buffer.cast::<u8>(), 2048) };
    bytes.fill(0);
    match sector {
        16 => {
            bytes[0] = 1;
            bytes[1..6].copy_from_slice(b"CD001");
            bytes[6] = 1;
            bytes[40..47].copy_from_slice(b"TESTVOL");
            if case == 2 || case == 3 {
                bytes[136..140]
                    .copy_from_slice(&(if case == 3 { 7u32 } else { 8u32 }).to_be_bytes());
                bytes[148..152].copy_from_slice(&20u32.to_be_bytes());
            }
        }
        17 => {
            bytes[0] = 0xff;
            bytes[1..6].copy_from_slice(b"CD001");
            bytes[6] = 1;
        }
        20 => {}
        _ => return false,
    }
    true
}

fn probe_iso9660(logger: &mut ProbeLogger) {
    use core::sync::atomic::Ordering;
    logger.section("ISO9660 PROBE");
    let disc = filesystem::DiscInterface {
        io_type: 0,
        features: 0,
        startup: Some(iso_ready),
        is_inserted: Some(iso_ready),
        read_sectors: Some(iso_read),
        write_sectors: None,
        clear_status: None,
        shutdown: None,
    };
    let name = c"iso_test";
    let null_arguments_rejected = unsafe {
        !iso9660::ISO9660_Mount(core::ptr::null(), &disc)
            && !iso9660::ISO9660_Mount(name.as_ptr(), core::ptr::null())
            && !iso9660::ISO9660_Unmount(core::ptr::null())
            && iso9660::ISO9660_GetVolumeLabel(core::ptr::null()).is_null()
    };
    logger.record(
        "ISO9660 C exports reject null arguments",
        null_arguments_rejected,
    );
    for (case, label) in [
        (1, "iso descriptor read failure"),
        (2, "iso path table read failure"),
        (3, "iso truncated path table"),
    ] {
        ISO_CASE.store(case, Ordering::Relaxed);
        let rejected = unsafe { !iso9660::ISO9660_Mount(name.as_ptr(), &disc) };
        let absent = unsafe { iso9660::ISO9660_GetVolumeLabel(name.as_ptr()).is_null() };
        let cleanup = if rejected {
            true
        } else {
            unsafe { iso9660::ISO9660_Unmount(name.as_ptr()) }
        };
        logger.record(label, rejected && absent && cleanup);
    }
    ISO_CASE.store(0, Ordering::Relaxed);
    let mounted = unsafe { iso9660::ISO9660_Mount(name.as_ptr(), &disc) };
    let label = unsafe { iso9660::ISO9660_GetVolumeLabel(name.as_ptr()) };
    let label_ok =
        !label.is_null() && unsafe { core::ffi::CStr::from_ptr(label).to_bytes() } == b"TESTVOL";
    let removed = mounted && unsafe { iso9660::ISO9660_Unmount(name.as_ptr()) };
    logger.record(
        "iso mount after failures",
        mounted
            && label_ok
            && removed
            && unsafe { iso9660::ISO9660_GetVolumeLabel(name.as_ptr()).is_null() },
    );

    let rust_name = c"isfs";
    let rust_mounted = filesystem::mount(rust_name, &disc);
    let rust_label = filesystem::volume_label(rust_name);
    let rust_label_ok = !rust_label.is_null()
        && unsafe { core::ffi::CStr::from_ptr(rust_label).to_bytes() } == b"TESTVOL";
    let rust_removed = filesystem::unmount(rust_name);
    let rust_label_cleared = filesystem::volume_label(rust_name).is_null();
    logger.note_i32("Rust filesystem ISO mount", rust_mounted as i32);
    logger.note_i32("Rust filesystem ISO volume label", rust_label_ok as i32);
    logger.note_i32("Rust filesystem ISO unmount", rust_removed as i32);
    logger.note_i32(
        "Rust filesystem ISO label cleared",
        rust_label_cleared as i32,
    );
    logger.record("Rust filesystem ISO mount", rust_mounted);
    logger.record("Rust filesystem ISO volume label", rust_label_ok);
    logger.record("Rust filesystem ISO unmount", rust_removed);
    logger.record("Rust filesystem ISO cleanup", rust_label_cleared);
    probe_isfs_uninitialized(logger);
}

fn probe_isfs_uninitialized(logger: &mut ProbeLogger) {
    logger.section("ISFS INVALID-STATE API PROBE");

    // The probe logger can hold a file handle, but it does not use the shared
    // /dev/fs service descriptor. Close that descriptor to exercise safe
    // pre-initialization results without issuing an IOS write operation.
    let init_before_deinit = filesystem::initialize();
    let deinit = filesystem::deinitialize();
    let repeated_deinit = filesystem::deinitialize();

    let mut stats = filesystem::IsfsStats {
        a: 1,
        b: 2,
        c: 3,
        d: 4,
        e: 5,
        f: 6,
        g: 7,
    };
    let get_stats = filesystem::get_stats(&mut stats);
    // SAFETY: the initialized service is closed, so this call returns before
    // it can submit the destructive format ioctl.
    let format_async = unsafe { filesystem::format_async(None, core::ptr::null_mut()) };
    let format = filesystem::format();

    let path = c"/tmp/libogc-fs-probe";
    let mut directory_count = 11;
    let read_dir = filesystem::read_dir(path, None, &mut directory_count);
    // SAFETY: the uninitialized service returns an error before submission.
    let read_dir_async = unsafe {
        filesystem::read_dir_async(
            path,
            None,
            &mut directory_count,
            None,
            core::ptr::null_mut(),
        )
    };
    let mut usage_a = 21;
    let mut usage_b = 22;
    let get_usage = filesystem::get_usage(path, &mut usage_a, &mut usage_b);
    // SAFETY: the uninitialized service returns an error before submission.
    let get_usage_async = unsafe {
        filesystem::get_usage_async(
            path,
            &mut usage_a,
            &mut usage_b,
            None,
            core::ptr::null_mut(),
        )
    };
    let create_dir = filesystem::create_dir(path, 0, 0, 0, 0);
    // SAFETY: the uninitialized service returns an error before submission.
    let create_dir_async =
        unsafe { filesystem::create_dir_async(path, 0, 0, 0, 0, None, core::ptr::null_mut()) };
    let create_file = filesystem::create_file(path, 0, 0, 0, 0);
    // SAFETY: the uninitialized service returns an error before submission.
    let create_file_async =
        unsafe { filesystem::create_file_async(path, 0, 0, 0, 0, None, core::ptr::null_mut()) };
    let delete = filesystem::delete(path);
    // SAFETY: the uninitialized service returns an error before submission.
    let delete_async = unsafe { filesystem::delete_async(path, None, core::ptr::null_mut()) };
    let rename = filesystem::rename(path, c"/tmp/libogc-fs-probe-renamed");
    // SAFETY: the uninitialized service returns an error before submission.
    let rename_async = unsafe {
        filesystem::rename_async(
            path,
            c"/tmp/libogc-fs-probe-renamed",
            None,
            core::ptr::null_mut(),
        )
    };

    let mut owner = 31;
    let mut group = 32;
    let mut attributes = 33;
    let mut owner_perm = 34;
    let mut group_perm = 35;
    let mut other_perm = 36;
    let get_attr = filesystem::get_attr(
        path,
        &mut owner,
        &mut group,
        &mut attributes,
        &mut owner_perm,
        &mut group_perm,
        &mut other_perm,
    );
    // SAFETY: the uninitialized service returns an error before submission.
    let get_attr_async = unsafe {
        filesystem::get_attr_async(
            path,
            filesystem::FsAttributeOutputs {
                owner_id: &mut owner,
                group_id: &mut group,
                attributes: &mut attributes,
                owner_perm: &mut owner_perm,
                group_perm: &mut group_perm,
                other_perm: &mut other_perm,
            },
            None,
            core::ptr::null_mut(),
        )
    };
    let set_attr = filesystem::set_attr(path, 0, 0, 0, 0, 0, 0);
    // SAFETY: the uninitialized service returns an error before submission.
    let set_attr_async = unsafe {
        filesystem::set_attr_async(
            path,
            filesystem::FsAttributes::default(),
            None,
            core::ptr::null_mut(),
        )
    };

    let open = filesystem::open(path, filesystem::ISFS_OPEN_READ);
    // SAFETY: the uninitialized service returns before it submits an IOS open.
    let open_async = unsafe {
        filesystem::open_async(
            path,
            filesystem::ISFS_OPEN_READ,
            None,
            core::ptr::null_mut(),
        )
    };
    let file = filesystem::IsfsFile::from_raw(-1);
    let raw = file.raw();
    let empty_write = file.write(&[]);
    let empty_write_all = file.write_all(&[]);
    let empty_read = file.read(&mut []);
    // SAFETY: these empty slices hit validation errors before IOS submission.
    let empty_read_async = unsafe { file.read_async(&mut [], None, core::ptr::null_mut()) };
    // SAFETY: the empty slice hits validation before IOS submission.
    let empty_write_async = unsafe { file.write_async(&[], None, core::ptr::null_mut()) };
    let seek = file.seek(0, 0);
    reset_isfs_callback();
    // SAFETY: the callback uses atomics and remains valid until IOS completes.
    let seek_async =
        unsafe { file.seek_async(0, 0, Some(isfs_probe_callback), core::ptr::null_mut()) };
    let seek_async_done = wait_isfs_callback();
    let seek_async_result = ISFS_CALLBACK_RESULT.load(Ordering::Acquire);
    let mut file_stats = AlignedFstats(filesystem::Fstats::default());
    let get_file_stats = filesystem::get_file_stats(&file, &mut file_stats.0);
    reset_isfs_callback();
    // SAFETY: the callback uses atomics. The API owns its request context and
    // frees it after IOS reports the failed descriptor.
    let get_file_stats_async = unsafe {
        file.get_stats_async(
            &mut file_stats.0,
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    };
    let get_file_stats_async_done = wait_isfs_callback();
    let get_file_stats_async_result = ISFS_CALLBACK_RESULT.load(Ordering::Acquire);
    // SAFETY: a negative raw descriptor is rejected by the ownership guard.
    let close_async = unsafe { file.close_async(None, core::ptr::null_mut()) };
    let close_invalid = filesystem::IsfsFile::from_raw(-1).close();

    let init = filesystem::initialize();
    logger.note_i32("ISFS deinitialize result", deinit);
    logger.note_i32("ISFS repeated deinitialize result", repeated_deinit);
    logger.note_i32("ISFS reinitialize result", init);
    logger.record(
        "ISFS service deinitialize and reinitialize",
        init_before_deinit == filesystem::ISFS_OK
            && deinit == filesystem::ISFS_OK
            && repeated_deinit == filesystem::ISFS_EINVAL
            && init == filesystem::ISFS_OK,
    );
    logger.record(
        "ISFS uninitialized synchronous APIs reject and preserve outputs",
        [
            get_stats,
            format,
            read_dir,
            get_usage,
            create_dir,
            create_file,
            delete,
            rename,
            get_attr,
            set_attr,
        ]
        .iter()
        .all(|result| *result == filesystem::ISFS_EINVAL)
            && stats
                == filesystem::IsfsStats {
                    a: 1,
                    b: 2,
                    c: 3,
                    d: 4,
                    e: 5,
                    f: 6,
                    g: 7,
                }
            && directory_count == 11
            && usage_a == 21
            && usage_b == 22
            && owner == 31
            && group == 32
            && attributes == 33
            && owner_perm == 34
            && group_perm == 35
            && other_perm == 36
            && matches!(open, Err(filesystem::ISFS_EINVAL))
            && seek.is_err()
            && get_file_stats < 0
            && close_invalid == filesystem::ISFS_OK,
    );
    logger.record(
        "ISFS uninitialized async APIs reject without destructive submission",
        [
            format_async,
            read_dir_async,
            get_usage_async,
            create_dir_async,
            create_file_async,
            delete_async,
            rename_async,
            get_attr_async,
            set_attr_async,
            open_async,
        ]
        .iter()
        .all(|result| *result == filesystem::ISFS_EINVAL)
            && close_async == filesystem::ISFS_OK
            && raw == -1
            && empty_write == Err(filesystem::ISFS_EINVAL)
            && empty_write_all == Err(filesystem::ISFS_EINVAL)
            && empty_read == Err(filesystem::ISFS_EINVAL)
            && empty_read_async == filesystem::ISFS_EINVAL
            && empty_write_async == filesystem::ISFS_EINVAL
            && seek_async >= 0
            && seek_async_done
            && seek_async_result < 0
            && get_file_stats_async >= 0
            && get_file_stats_async_done
            && get_file_stats_async_result < 0,
    );
}

#[repr(align(32))]
struct AlignedFstats(filesystem::Fstats);
