use core::sync::atomic::{AtomicBool, AtomicI32, Ordering};
use ogc_aes_sys as aes;
use ogc_conf as conf;
use ogc_es as es;
use ogc_filesystem as filesystem;
use ogc_ios as ios;
use ogc_lwp::sys as lwp;
use ogc_sha as sha;
use ogc_stm as stm;
use ogc_sysbase as sysbase;
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

fn run_isfs_async_and_wait(start: impl FnOnce() -> i32) -> (i32, i32) {
    reset_isfs_callback();
    let submission = start();
    if submission < 0 {
        return (submission, i32::MIN);
    }
    while !ISFS_CALLBACK_DONE.load(Ordering::Acquire) {
        lwp::sleep_ticks(1);
    }
    (submission, ISFS_CALLBACK_RESULT.load(Ordering::Acquire))
}

fn run_isfs_async_and_wait_bounded(start: impl FnOnce() -> i32) -> (i32, bool, i32) {
    reset_isfs_callback();
    let submission = start();
    if submission < 0 {
        return (submission, false, i32::MIN);
    }
    let completed = wait_isfs_callback();
    let result = if completed {
        ISFS_CALLBACK_RESULT.load(Ordering::Acquire)
    } else {
        i32::MIN
    };
    (submission, completed, result)
}

fn isfs_file_exists(path: &core::ffi::CStr) -> Option<bool> {
    match filesystem::open(path, filesystem::ISFS_OPEN_READ) {
        Ok(file) => Some(file.close() == filesystem::ISFS_OK),
        Err(filesystem::ISFS_ENOENT) => Some(false),
        Err(_) => None,
    }
}

fn isfs_directory_exists(path: &core::ffi::CStr) -> Option<bool> {
    let mut usage_a = 0;
    let mut usage_b = 0;
    match filesystem::get_usage(path, &mut usage_a, &mut usage_b) {
        filesystem::ISFS_OK => Some(true),
        filesystem::ISFS_ENOENT => Some(false),
        _ => None,
    }
}

fn cleanup_isfs_test_tree(directory: &core::ffi::CStr, file: &core::ffi::CStr) -> bool {
    let file_removed = match isfs_file_exists(file) {
        Some(true) => filesystem::delete(file) == filesystem::ISFS_OK,
        Some(false) => true,
        None => false,
    };
    let directory_removed = match isfs_directory_exists(directory) {
        Some(true) if file_removed => filesystem::delete(directory) == filesystem::ISFS_OK,
        Some(false) => true,
        _ => false,
    };
    file_removed && directory_removed
}

unsafe extern "C" fn iso_ready() -> bool {
    true
}

fn write_iso_directory_record(
    bytes: &mut [u8],
    offset: &mut usize,
    sector: u32,
    size: u32,
    flags: u8,
    name: &[u8],
) -> bool {
    let record_len = 33 + name.len() + usize::from(name.len().is_multiple_of(2));
    let Some(record_end) = offset.checked_add(record_len) else {
        return false;
    };
    if name.len() > u8::MAX as usize || record_end > bytes.len() {
        return false;
    }

    bytes[*offset] = record_len as u8;
    bytes[*offset + 2..*offset + 6].copy_from_slice(&sector.to_le_bytes());
    bytes[*offset + 6..*offset + 10].copy_from_slice(&sector.to_be_bytes());
    bytes[*offset + 10..*offset + 14].copy_from_slice(&size.to_le_bytes());
    bytes[*offset + 14..*offset + 18].copy_from_slice(&size.to_be_bytes());
    bytes[*offset + 25] = flags;
    bytes[*offset + 28..*offset + 30].copy_from_slice(&1u16.to_le_bytes());
    bytes[*offset + 30..*offset + 32].copy_from_slice(&1u16.to_be_bytes());
    bytes[*offset + 32] = name.len() as u8;
    bytes[*offset + 33..*offset + 33 + name.len()].copy_from_slice(name);
    *offset = record_end;
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
            } else if case == 4 {
                bytes[136..140].copy_from_slice(&10u32.to_be_bytes());
                bytes[148..152].copy_from_slice(&20u32.to_be_bytes());
                let mut root_record = 156;
                if !write_iso_directory_record(bytes, &mut root_record, 21, 2048, 2, &[0]) {
                    return false;
                }
            }
        }
        17 => {
            bytes[0] = 0xff;
            bytes[1..6].copy_from_slice(b"CD001");
            bytes[6] = 1;
        }
        20 if case == 4 => {
            bytes[0] = 1;
            bytes[2..6].copy_from_slice(&21u32.to_le_bytes());
            bytes[6..8].copy_from_slice(&1u16.to_be_bytes());
        }
        20 => {}
        21 if case == 4 => {
            let mut offset = 0;
            let root_records = write_iso_directory_record(bytes, &mut offset, 21, 2048, 2, &[0])
                && write_iso_directory_record(bytes, &mut offset, 21, 2048, 2, &[1])
                && write_iso_directory_record(bytes, &mut offset, 22, 5, 0, b"HELLO.TXT;1");
            if !root_records {
                return false;
            }
        }
        22 if case == 4 => bytes[..5].copy_from_slice(b"hello"),
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
    ISO_CASE.store(4, Ordering::Relaxed);
    let mounted = unsafe { iso9660::ISO9660_Mount(name.as_ptr(), &disc) };
    let label = unsafe { iso9660::ISO9660_GetVolumeLabel(name.as_ptr()) };
    let label_ok =
        !label.is_null() && unsafe { core::ffi::CStr::from_ptr(label).to_bytes() } == b"TESTVOL";
    if mounted {
        probe_iso9660_files(logger, name);
    }
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
    if rust_mounted {
        probe_iso9660_files(logger, rust_name);
    }
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
    probe_isfs_successful_file_cycle(logger);
}

fn probe_isfs_successful_file_cycle(logger: &mut ProbeLogger) {
    logger.section("ISFS SUCCESSFUL FILE CYCLE");
    let directory = c"/tmp/ogc-rs-isfs-verify";
    let file_path = c"/tmp/ogc-rs-isfs-verify/file";
    let renamed_path = c"/tmp/ogc-rs-isfs-verify/renamed";
    let initialized = filesystem::initialize() == filesystem::ISFS_OK;
    let mut service_stats = filesystem::IsfsStats {
        a: u32::MAX,
        b: u32::MAX,
        c: u32::MAX,
        d: u32::MAX,
        e: u32::MAX,
        f: u32::MAX,
        g: u32::MAX,
    };
    let service_stats_result = if initialized {
        filesystem::get_stats(&mut service_stats)
    } else {
        filesystem::ISFS_EINVAL
    };
    logger.record(
        "ISFS get volume stats",
        service_stats_result == filesystem::ISFS_OK
            && service_stats
                != filesystem::IsfsStats {
                    a: u32::MAX,
                    b: u32::MAX,
                    c: u32::MAX,
                    d: u32::MAX,
                    e: u32::MAX,
                    f: u32::MAX,
                    g: u32::MAX,
                },
    );
    let directory_created =
        initialized && filesystem::create_dir(directory, 0, 3, 3, 3) == filesystem::ISFS_OK;
    logger.record("ISFS create test directory", directory_created);

    let file_created =
        directory_created && filesystem::create_file(file_path, 0, 3, 3, 3) == filesystem::ISFS_OK;
    logger.record("ISFS create test file", file_created);

    let mut aligned_write = AlignedIsfsBytes([0; 32]);
    aligned_write.0[..8].copy_from_slice(b"ISFS-rs!");
    let file = if file_created {
        filesystem::open(file_path, filesystem::ISFS_OPEN_RW).ok()
    } else {
        None
    };
    let write_result = file.as_ref().map_or(Err(filesystem::ISFS_EINVAL), |file| {
        file.write_all(&aligned_write.0[..8])
    });
    let seek_result = file
        .as_ref()
        .map_or(Err(filesystem::ISFS_EINVAL), |file| file.seek(0, 0));
    let mut aligned_read = AlignedIsfsBytes([0; 32]);
    let read_result = file.as_ref().map_or(Err(filesystem::ISFS_EINVAL), |file| {
        file.read(&mut aligned_read.0[..8])
    });
    let mut stats = AlignedFstats(filesystem::Fstats::default());
    let stats_result = file.as_ref().map_or(filesystem::ISFS_EINVAL, |file| {
        filesystem::get_file_stats(file, &mut stats.0)
    });
    let file_closed = file.is_none_or(|file| file.close() == filesystem::ISFS_OK);
    logger.record(
        "ISFS file write seek read stats and close",
        write_result == Ok(())
            && seek_result == Ok(0)
            && read_result == Ok(8)
            && aligned_read.0[..8] == *b"ISFS-rs!"
            && stats_result == filesystem::ISFS_OK
            && stats.0
                == filesystem::Fstats {
                    file_length: 8,
                    file_pos: 8,
                }
            && file_closed,
    );

    let mut owner_id = u32::MAX;
    let mut group_id = u16::MAX;
    let mut attributes = u8::MAX;
    let mut owner_perm = u8::MAX;
    let mut group_perm = u8::MAX;
    let mut other_perm = u8::MAX;
    let get_attr_result = if file_created {
        filesystem::get_attr(
            file_path,
            &mut owner_id,
            &mut group_id,
            &mut attributes,
            &mut owner_perm,
            &mut group_perm,
            &mut other_perm,
        )
    } else {
        filesystem::ISFS_EINVAL
    };
    let set_attr_result = if get_attr_result == filesystem::ISFS_OK {
        filesystem::set_attr(file_path, owner_id, group_id, attributes, 3, 1, 1)
    } else {
        filesystem::ISFS_EINVAL
    };
    let mut changed_owner_id = u32::MAX;
    let mut changed_group_id = u16::MAX;
    let mut changed_attributes = u8::MAX;
    let mut changed_owner_perm = u8::MAX;
    let mut changed_group_perm = u8::MAX;
    let mut changed_other_perm = u8::MAX;
    let changed_attr_result = if set_attr_result == filesystem::ISFS_OK {
        filesystem::get_attr(
            file_path,
            &mut changed_owner_id,
            &mut changed_group_id,
            &mut changed_attributes,
            &mut changed_owner_perm,
            &mut changed_group_perm,
            &mut changed_other_perm,
        )
    } else {
        filesystem::ISFS_EINVAL
    };
    let mut usage_blocks = u32::MAX;
    let mut usage_inodes = u32::MAX;
    let usage_result = if directory_created {
        filesystem::get_usage(directory, &mut usage_blocks, &mut usage_inodes)
    } else {
        filesystem::ISFS_EINVAL
    };
    logger.record(
        "ISFS get set get attributes and usage",
        get_attr_result == filesystem::ISFS_OK
            && set_attr_result == filesystem::ISFS_OK
            && changed_attr_result == filesystem::ISFS_OK
            && changed_owner_id == owner_id
            && changed_group_id == group_id
            && changed_attributes == attributes
            && changed_owner_perm == 3
            && changed_group_perm == 1
            && changed_other_perm == 1
            && usage_result == filesystem::ISFS_OK
            && usage_blocks != u32::MAX
            && usage_inodes != u32::MAX,
    );

    let mut directory_count = 4;
    let mut directory_names = AlignedIsfsBytes([0; 32 * 4]);
    let read_dir_result = if directory_created {
        filesystem::read_dir(
            directory,
            Some(&mut directory_names.0),
            &mut directory_count,
        )
    } else {
        filesystem::ISFS_EINVAL
    };
    let entry_present = directory_names
        .0
        .split(|byte| *byte == 0)
        .any(|entry| entry == b"file");
    logger.note_i32("ISFS successful readdir result", read_dir_result);
    logger.note_i32("ISFS successful readdir count", directory_count as i32);
    logger.note_i32("ISFS successful readdir file entry", entry_present as i32);
    logger.record(
        "ISFS read directory lists test file",
        read_dir_result == filesystem::ISFS_OK && directory_count > 0 && entry_present,
    );
    probe_isfs_async_file_cycle(directory, logger);
    probe_isfs_async_directory_cycle(directory, logger);
    probe_isfs_async_rename_cycle(logger);

    let rename_result = if file_created {
        filesystem::rename(file_path, renamed_path)
    } else {
        filesystem::ISFS_EINVAL
    };
    logger.note_i32("ISFS successful rename result", rename_result);
    let renamed_open = if rename_result == filesystem::ISFS_OK {
        filesystem::open(renamed_path, filesystem::ISFS_OPEN_READ).ok()
    } else {
        None
    };
    let renamed_file_exists = renamed_open.is_some();
    let renamed_file_closed = renamed_open.is_none_or(|file| file.close() == filesystem::ISFS_OK);
    if rename_result == filesystem::ISFS_OK {
        logger.record(
            "ISFS open renamed file and close",
            renamed_file_exists && renamed_file_closed,
        );
    } else {
        logger.note("ISFS renamed-file reopen skipped because Dolphin rejected rename");
    }

    let delete_result = if rename_result == filesystem::ISFS_OK {
        filesystem::delete(renamed_path)
    } else if file_created {
        filesystem::delete(file_path)
    } else {
        filesystem::ISFS_OK
    };
    let directory_delete_result = if directory_created && delete_result == filesystem::ISFS_OK {
        filesystem::delete(directory)
    } else {
        filesystem::ISFS_OK
    };
    let deleted_path = if rename_result == filesystem::ISFS_OK {
        renamed_path
    } else {
        file_path
    };
    let deleted_file_open = if delete_result == filesystem::ISFS_OK && file_created {
        filesystem::open(deleted_path, filesystem::ISFS_OPEN_READ)
    } else {
        Err(filesystem::ISFS_EINVAL)
    };
    let mut deleted_directory_usage_a = u32::MAX;
    let mut deleted_directory_usage_b = u32::MAX;
    let deleted_directory_usage =
        if directory_delete_result == filesystem::ISFS_OK && directory_created {
            filesystem::get_usage(
                directory,
                &mut deleted_directory_usage_a,
                &mut deleted_directory_usage_b,
            )
        } else {
            filesystem::ISFS_EINVAL
        };
    logger.note_i32(
        "ISFS open after file delete result",
        deleted_file_open
            .as_ref()
            .map_or_else(|error| *error, |_| filesystem::ISFS_OK),
    );
    logger.note_i32(
        "ISFS usage after directory delete result",
        deleted_directory_usage,
    );
    logger.record(
        "ISFS delete test file and directory",
        delete_result == filesystem::ISFS_OK && directory_delete_result == filesystem::ISFS_OK,
    );
    logger.record(
        "ISFS file and directory are absent after delete",
        deleted_file_open.is_err()
            && deleted_directory_usage == filesystem::ISFS_ENOENT
            && deleted_directory_usage_a == u32::MAX
            && deleted_directory_usage_b == u32::MAX,
    );
    probe_isfs_format_apis(logger);
}

fn probe_isfs_format_apis(logger: &mut ProbeLogger) {
    logger.section("ISFS FORMAT API PROBE");
    let directory = c"/tmp/ogc-rs-format-verify";
    let marker = c"/tmp/ogc-rs-format-verify/marker";

    let initialized = filesystem::initialize() == filesystem::ISFS_OK;
    let directory_created =
        initialized && filesystem::create_dir(directory, 0, 3, 3, 3) == filesystem::ISFS_OK;
    let marker_created =
        directory_created && filesystem::create_file(marker, 0, 3, 3, 3) == filesystem::ISFS_OK;
    let (submission, completed, result) = if marker_created {
        run_isfs_async_and_wait_bounded(|| unsafe {
            filesystem::format_async(Some(isfs_probe_callback), core::ptr::null_mut())
        })
    } else {
        (-1, false, i32::MIN)
    };
    logger.note_i32("ISFS format_async submission result", submission);
    logger.note_i32("ISFS format_async callback result", result);
    let deinitialize = filesystem::deinitialize();
    let reinitialize = filesystem::initialize();
    let file_exists = isfs_file_exists(marker);
    let directory_exists = isfs_directory_exists(directory);
    logger.record(
        "ISFS format_async removes the marker file",
        file_exists == Some(false),
    );
    logger.record(
        "ISFS format_async removes the marker directory",
        directory_exists == Some(false),
    );
    logger.record(
        "ISFS format_async completes and removes marker state",
        marker_created
            && submission >= 0
            && completed
            && result == filesystem::ISFS_OK
            && deinitialize == filesystem::ISFS_OK
            && reinitialize == filesystem::ISFS_OK
            && file_exists == Some(false)
            && directory_exists == Some(false),
    );
    let cleanup = cleanup_isfs_test_tree(directory, marker);
    logger.record("ISFS format_async marker cleanup", cleanup);
}

fn probe_isfs_async_file_cycle(directory: &core::ffi::CStr, logger: &mut ProbeLogger) {
    logger.section("ISFS ASYNC FILE CYCLE");
    let path = c"/tmp/ogc-rs-isfs-verify/async-file";
    let (create_submit, create_result) = run_isfs_async_and_wait(|| unsafe {
        filesystem::create_file_async(
            path,
            0,
            3,
            3,
            3,
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    });
    logger.record(
        "ISFS create_file_async completes successfully",
        create_submit >= 0 && create_result == filesystem::ISFS_OK,
    );

    let (open_submit, open_result) = if create_result == filesystem::ISFS_OK {
        run_isfs_async_and_wait(|| unsafe {
            filesystem::open_async(
                path,
                filesystem::ISFS_OPEN_RW,
                Some(isfs_probe_callback),
                core::ptr::null_mut(),
            )
        })
    } else {
        (-1, i32::MIN)
    };
    let mut file =
        (open_submit >= 0 && open_result >= 0).then(|| filesystem::IsfsFile::from_raw(open_result));
    logger.record("ISFS open_async returns an open descriptor", file.is_some());

    let mut write_buffer = AlignedIsfsBytes([0; 32]);
    write_buffer.0[..8].copy_from_slice(b"async-rs");
    let (write_submit, write_result) = if let Some(file) = file.as_ref() {
        run_isfs_async_and_wait(|| unsafe {
            file.write_async(
                &write_buffer.0[..8],
                Some(isfs_probe_callback),
                core::ptr::null_mut(),
            )
        })
    } else {
        (-1, i32::MIN)
    };
    let (seek_submit, seek_result) = if let Some(file) = file.as_ref() {
        run_isfs_async_and_wait(|| unsafe {
            file.seek_async(0, 0, Some(isfs_probe_callback), core::ptr::null_mut())
        })
    } else {
        (-1, i32::MIN)
    };
    let mut read_buffer = AlignedIsfsBytes([0; 32]);
    let (read_submit, read_result) = if let Some(file) = file.as_ref() {
        run_isfs_async_and_wait(|| unsafe {
            file.read_async(
                &mut read_buffer.0[..8],
                Some(isfs_probe_callback),
                core::ptr::null_mut(),
            )
        })
    } else {
        (-1, i32::MIN)
    };
    let mut stats = AlignedFstats(filesystem::Fstats::default());
    let (stats_submit, stats_result) = if let Some(file) = file.as_ref() {
        run_isfs_async_and_wait(|| unsafe {
            file.get_stats_async(
                &mut stats.0,
                Some(isfs_probe_callback),
                core::ptr::null_mut(),
            )
        })
    } else {
        (-1, i32::MIN)
    };
    logger.record(
        "ISFS async write seek read and file stats",
        write_submit >= 0
            && write_result == 8
            && seek_submit >= 0
            && seek_result == 0
            && read_submit >= 0
            && read_result == 8
            && read_buffer.0[..8] == *b"async-rs"
            && stats_submit >= 0
            && stats_result == filesystem::ISFS_OK
            && stats.0
                == filesystem::Fstats {
                    file_length: 8,
                    file_pos: 8,
                },
    );

    let mut owner_id = u32::MAX;
    let mut group_id = u16::MAX;
    let mut attributes = u8::MAX;
    let mut owner_perm = u8::MAX;
    let mut group_perm = u8::MAX;
    let mut other_perm = u8::MAX;
    let (get_attr_submit, get_attr_result) = run_isfs_async_and_wait(|| unsafe {
        filesystem::get_attr_async(
            path,
            filesystem::FsAttributeOutputs {
                owner_id: &mut owner_id,
                group_id: &mut group_id,
                attributes: &mut attributes,
                owner_perm: &mut owner_perm,
                group_perm: &mut group_perm,
                other_perm: &mut other_perm,
            },
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    });
    let (set_attr_submit, set_attr_result) = if get_attr_result == filesystem::ISFS_OK {
        run_isfs_async_and_wait(|| unsafe {
            filesystem::set_attr_async(
                path,
                filesystem::FsAttributes {
                    owner_id,
                    group_id,
                    attributes,
                    owner_perm: 3,
                    group_perm: 1,
                    other_perm: 1,
                },
                Some(isfs_probe_callback),
                core::ptr::null_mut(),
            )
        })
    } else {
        (-1, i32::MIN)
    };
    let mut changed_owner_id = u32::MAX;
    let mut changed_group_id = u16::MAX;
    let mut changed_attributes = u8::MAX;
    let mut changed_owner_perm = u8::MAX;
    let mut changed_group_perm = u8::MAX;
    let mut changed_other_perm = u8::MAX;
    let (changed_attr_submit, changed_attr_result) = if set_attr_result == filesystem::ISFS_OK {
        run_isfs_async_and_wait(|| unsafe {
            filesystem::get_attr_async(
                path,
                filesystem::FsAttributeOutputs {
                    owner_id: &mut changed_owner_id,
                    group_id: &mut changed_group_id,
                    attributes: &mut changed_attributes,
                    owner_perm: &mut changed_owner_perm,
                    group_perm: &mut changed_group_perm,
                    other_perm: &mut changed_other_perm,
                },
                Some(isfs_probe_callback),
                core::ptr::null_mut(),
            )
        })
    } else {
        (-1, i32::MIN)
    };

    let mut usage_a = u32::MAX;
    let mut usage_b = u32::MAX;
    let (usage_submit, usage_result) = run_isfs_async_and_wait(|| unsafe {
        filesystem::get_usage_async(
            directory,
            &mut usage_a,
            &mut usage_b,
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    });
    let mut num = 4;
    let mut names = AlignedIsfsBytes([0; 32 * 4]);
    let (readdir_submit, readdir_result) = run_isfs_async_and_wait(|| unsafe {
        filesystem::read_dir_async(
            directory,
            Some(&mut names.0),
            &mut num,
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    });
    let async_entry_present = names
        .0
        .split(|byte| *byte == 0)
        .any(|entry| entry == b"async-file");
    logger.record(
        "ISFS async attributes usage and directory listing",
        get_attr_submit >= 0
            && get_attr_result == filesystem::ISFS_OK
            && set_attr_submit >= 0
            && set_attr_result == filesystem::ISFS_OK
            && changed_attr_submit >= 0
            && changed_attr_result == filesystem::ISFS_OK
            && changed_owner_id == owner_id
            && changed_group_id == group_id
            && changed_attributes == attributes
            && changed_owner_perm == 3
            && changed_group_perm == 1
            && changed_other_perm == 1
            && usage_submit >= 0
            && usage_result == filesystem::ISFS_OK
            && usage_a != u32::MAX
            && usage_b != u32::MAX
            && readdir_submit >= 0
            && readdir_result == filesystem::ISFS_OK
            && num > 0
            && async_entry_present,
    );

    let mut closed = false;
    if let Some(file) = file.take() {
        let (close_submit, close_result) = run_isfs_async_and_wait(|| unsafe {
            file.close_async(Some(isfs_probe_callback), core::ptr::null_mut())
        });
        closed = close_submit >= 0 && close_result == filesystem::ISFS_OK;
    }
    let (delete_submit, delete_result) = run_isfs_async_and_wait(|| unsafe {
        filesystem::delete_async(path, Some(isfs_probe_callback), core::ptr::null_mut())
    });
    let deleted_file_open = filesystem::open(path, filesystem::ISFS_OPEN_READ);
    logger.record(
        "ISFS async close delete and cleanup",
        closed
            && delete_submit >= 0
            && delete_result == filesystem::ISFS_OK
            && deleted_file_open.is_err_and(|error| error == filesystem::ISFS_ENOENT),
    );
}

fn probe_isfs_async_directory_cycle(directory: &core::ffi::CStr, logger: &mut ProbeLogger) {
    logger.section("ISFS ASYNC DIRECTORY CYCLE");
    let path = c"/tmp/ogc-rs-isfs-verify/async-dir";
    let mut stats = filesystem::IsfsStats {
        a: u32::MAX,
        b: u32::MAX,
        c: u32::MAX,
        d: u32::MAX,
        e: u32::MAX,
        f: u32::MAX,
        g: u32::MAX,
    };
    let (stats_submit, stats_result) = run_isfs_async_and_wait(|| unsafe {
        filesystem::get_stats_async(&mut stats, Some(isfs_probe_callback), core::ptr::null_mut())
    });
    logger.record(
        "ISFS get_stats_async returns service stats",
        stats_submit >= 0
            && stats_result == filesystem::ISFS_OK
            && stats
                != filesystem::IsfsStats {
                    a: u32::MAX,
                    b: u32::MAX,
                    c: u32::MAX,
                    d: u32::MAX,
                    e: u32::MAX,
                    f: u32::MAX,
                    g: u32::MAX,
                },
    );

    let (create_submit, create_result) = run_isfs_async_and_wait(|| unsafe {
        filesystem::create_dir_async(
            path,
            0,
            3,
            3,
            3,
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    });
    let mut usage_a = u32::MAX;
    let mut usage_b = u32::MAX;
    let (usage_submit, usage_result) = if create_result == filesystem::ISFS_OK {
        run_isfs_async_and_wait(|| unsafe {
            filesystem::get_usage_async(
                path,
                &mut usage_a,
                &mut usage_b,
                Some(isfs_probe_callback),
                core::ptr::null_mut(),
            )
        })
    } else {
        (-1, i32::MIN)
    };

    let mut count = 8;
    let mut names = AlignedIsfsBytes([0; 32 * 8]);
    let (read_dir_submit, read_dir_result) = run_isfs_async_and_wait(|| unsafe {
        filesystem::read_dir_async(
            directory,
            Some(&mut names.0),
            &mut count,
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    });
    let directory_entry_present = names
        .0
        .split(|byte| *byte == 0)
        .any(|entry| entry == b"async-dir");
    logger.record(
        "ISFS create_dir_async usage and read_dir_async",
        create_submit >= 0
            && create_result == filesystem::ISFS_OK
            && usage_submit >= 0
            && usage_result == filesystem::ISFS_OK
            && usage_a != u32::MAX
            && usage_b != u32::MAX
            && read_dir_submit >= 0
            && read_dir_result == filesystem::ISFS_OK
            && count > 0
            && directory_entry_present,
    );

    let (delete_submit, delete_result) = if create_result == filesystem::ISFS_OK {
        run_isfs_async_and_wait(|| unsafe {
            filesystem::delete_async(path, Some(isfs_probe_callback), core::ptr::null_mut())
        })
    } else {
        (-1, i32::MIN)
    };
    let mut absent_usage_a = u32::MAX;
    let mut absent_usage_b = u32::MAX;
    let (absent_usage_submit, absent_usage_result) = if delete_result == filesystem::ISFS_OK {
        run_isfs_async_and_wait(|| unsafe {
            filesystem::get_usage_async(
                path,
                &mut absent_usage_a,
                &mut absent_usage_b,
                Some(isfs_probe_callback),
                core::ptr::null_mut(),
            )
        })
    } else {
        (-1, i32::MIN)
    };
    logger.record(
        "ISFS delete_async removes directory and preserves error outputs",
        delete_submit >= 0
            && delete_result == filesystem::ISFS_OK
            && absent_usage_submit >= 0
            && absent_usage_result == filesystem::ISFS_ENOENT
            && absent_usage_a == u32::MAX
            && absent_usage_b == u32::MAX,
    );
}

fn probe_isfs_async_rename_cycle(logger: &mut ProbeLogger) {
    logger.section("ISFS ASYNC RENAME CYCLE");
    let source = c"/tmp/ogc-rs-isfs-verify/async-rename-source";
    let destination = c"/tmp/ogc-rs-isfs-verify/async-rename-destination";
    let source_created = filesystem::create_file(source, 0, 3, 3, 3) == filesystem::ISFS_OK;
    let (rename_submit, rename_result) = if source_created {
        run_isfs_async_and_wait(|| unsafe {
            filesystem::rename_async(
                source,
                destination,
                Some(isfs_probe_callback),
                core::ptr::null_mut(),
            )
        })
    } else {
        (-1, i32::MIN)
    };
    logger.note_i32("ISFS rename_async callback result", rename_result);

    let source_open = filesystem::open(source, filesystem::ISFS_OPEN_READ);
    let source_exists = source_open.is_ok();
    let source_absent = source_open
        .as_ref()
        .is_err_and(|error| *error == filesystem::ISFS_ENOENT);
    let source_closed = match source_open {
        Ok(file) => file.close() == filesystem::ISFS_OK,
        Err(_) => true,
    };
    let destination_open = filesystem::open(destination, filesystem::ISFS_OPEN_READ);
    let destination_exists = destination_open.is_ok();
    let destination_absent = destination_open
        .as_ref()
        .is_err_and(|error| *error == filesystem::ISFS_ENOENT);
    let destination_closed = match destination_open {
        Ok(file) => file.close() == filesystem::ISFS_OK,
        Err(_) => true,
    };
    let state_matches_result = match rename_result {
        filesystem::ISFS_OK => source_absent && destination_exists && destination_closed,
        filesystem::ISFS_EINVAL => source_exists && source_closed && destination_absent,
        _ => false,
    };
    logger.record(
        "ISFS rename_async callback matches file state",
        source_created && rename_submit >= 0 && state_matches_result,
    );

    let source_cleanup = if source_exists {
        filesystem::delete(source) == filesystem::ISFS_OK
    } else {
        source_absent
    };
    let destination_cleanup = if destination_exists {
        filesystem::delete(destination) == filesystem::ISFS_OK
    } else {
        destination_absent
    };
    let source_removed = filesystem::open(source, filesystem::ISFS_OPEN_READ)
        .is_err_and(|error| error == filesystem::ISFS_ENOENT);
    let destination_removed = filesystem::open(destination, filesystem::ISFS_OPEN_READ)
        .is_err_and(|error| error == filesystem::ISFS_ENOENT);
    logger.record(
        "ISFS async rename test files are cleaned up",
        source_cleanup && destination_cleanup && source_removed && destination_removed,
    );
}

fn probe_iso9660_files(logger: &mut ProbeLogger, mount_name: &core::ffi::CStr) {
    let file_path = if mount_name.to_bytes() == b"iso_test" {
        c"iso_test:/HELLO.TXT"
    } else {
        c"isfs:/HELLO.TXT"
    };
    let device_found = sysbase::device_op_tab(file_path).is_some();
    let (file, open_error) = match sysbase::DeviceFile::open_checked(file_path, sysbase::O_RDONLY) {
        Ok(file) => (Some(file), 0),
        Err(error) => (None, error),
    };
    logger.note_i32("ISO9660 mounted device lookup", device_found as i32);
    logger.note_i32("ISO9660 file open error", open_error);
    let file_opened = file.is_some();
    let mut bytes = [0xA5; 8];
    let file_read = file
        .as_ref()
        .and_then(|file| file.read(&mut bytes).ok())
        .is_some_and(|count| count == 5 && bytes[..5] == *b"hello");
    let file_seek = file
        .as_ref()
        .and_then(|file| file.seek_off_t(1, sysbase::SEEK_SET).ok())
        == Some(1);
    let mut suffix = [0; 4];
    let file_seek_read = file
        .as_ref()
        .and_then(|file| file.read(&mut suffix).ok())
        .is_some_and(|count| count == 4 && suffix == *b"ello");
    let file_closed = file.is_some_and(|file| file.close().is_ok());

    let stat_fn = sysbase::device_op_tab(file_path).and_then(|tab| unsafe { (*tab).stat_r });
    let mut stat = sysbase::Stat::default();
    let stat_result = stat_fn.map_or(-1, |callback| unsafe {
        callback(
            sysbase::current_reent(),
            file_path.as_ptr(),
            (&mut stat as *mut sysbase::Stat).cast(),
        )
    });
    logger.note_i32("ISO9660 stat callback result", stat_result);
    let stat_ok = stat_result == 0
        && stat.st_ino == 22
        && stat.st_size == 5
        && stat.st_blocks == 1
        && stat.st_mode & 0o170000 == 0o100000;

    let root_path = if mount_name.to_bytes() == b"iso_test" {
        c"iso_test:/"
    } else {
        c"isfs:/"
    };
    let chdir_fn = sysbase::device_op_tab(root_path).and_then(|tab| unsafe { (*tab).chdir_r });
    let chdir_ok = chdir_fn.is_some_and(|callback| unsafe {
        callback(sysbase::current_reent(), root_path.as_ptr()) == 0
    });

    logger.record("ISO9660 mounted file opens", file_opened);
    logger.record("ISO9660 file read returns expected bytes", file_read);
    logger.record("ISO9660 file seek returns expected position", file_seek);
    logger.record(
        "ISO9660 read after seek returns expected bytes",
        file_seek_read,
    );
    logger.record("ISO9660 file close completes", file_closed);
    logger.record("ISO9660 stat reports file metadata", stat_ok);
    logger.record("ISO9660 chdir accepts the root directory", chdir_ok);
}

fn probe_isfs_uninitialized(logger: &mut ProbeLogger) {
    logger.section("ISFS INVALID-STATE API PROBE");

    // The probe logger can hold a file handle, but it does not use the shared
    // /dev/fs service descriptor. Close that descriptor to exercise safe
    // pre-initialization results without issuing an IOS write operation.
    let init_before_deinit = filesystem::initialize();
    let mut owner = 31;
    let mut group = 32;
    let mut attributes = 33;
    let mut owner_perm = 34;
    let mut group_perm = 35;
    let mut other_perm = 36;
    let mut long_path_bytes = [b'P'; 65];
    long_path_bytes[64] = 0;
    let long_path = core::ffi::CStr::from_bytes_with_nul(&long_path_bytes)
        .expect("fixed overlong ISFS path is NUL terminated");
    let mut long_path_count = 41;
    let long_read_dir = filesystem::read_dir(long_path, None, &mut long_path_count);
    let long_create_dir = filesystem::create_dir(long_path, 0, 0, 0, 0);
    let long_create_file = filesystem::create_file(long_path, 0, 0, 0, 0);
    let long_get_attr = filesystem::get_attr(
        long_path,
        &mut owner,
        &mut group,
        &mut attributes,
        &mut owner_perm,
        &mut group_perm,
        &mut other_perm,
    );
    let long_set_attr = filesystem::set_attr(long_path, 0, 0, 0, 0, 0, 0);
    let mut long_usage_a = 42;
    let mut long_usage_b = 43;
    let long_get_usage = filesystem::get_usage(long_path, &mut long_usage_a, &mut long_usage_b);
    let long_delete = filesystem::delete(long_path);
    let long_rename_old = filesystem::rename(long_path, c"/tmp/short");
    let long_rename_new = filesystem::rename(c"/tmp/short", long_path);
    let long_open = filesystem::open(long_path, filesystem::ISFS_OPEN_READ);

    reset_isfs_callback();
    // SAFETY: every call must reject the overlong path before IOS submission.
    let long_read_dir_async = unsafe {
        filesystem::read_dir_async(
            long_path,
            None,
            &mut long_path_count,
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    };
    let long_create_dir_async = unsafe {
        filesystem::create_dir_async(
            long_path,
            0,
            0,
            0,
            0,
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    };
    let long_create_file_async = unsafe {
        filesystem::create_file_async(
            long_path,
            0,
            0,
            0,
            0,
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    };
    let long_get_attr_async = unsafe {
        filesystem::get_attr_async(
            long_path,
            filesystem::FsAttributeOutputs {
                owner_id: &mut owner,
                group_id: &mut group,
                attributes: &mut attributes,
                owner_perm: &mut owner_perm,
                group_perm: &mut group_perm,
                other_perm: &mut other_perm,
            },
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    };
    let long_set_attr_async = unsafe {
        filesystem::set_attr_async(
            long_path,
            filesystem::FsAttributes::default(),
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    };
    let long_get_usage_async = unsafe {
        filesystem::get_usage_async(
            long_path,
            &mut long_usage_a,
            &mut long_usage_b,
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    };
    let long_delete_async = unsafe {
        filesystem::delete_async(long_path, Some(isfs_probe_callback), core::ptr::null_mut())
    };
    let long_rename_old_async = unsafe {
        filesystem::rename_async(
            long_path,
            c"/tmp/short",
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    };
    let long_rename_new_async = unsafe {
        filesystem::rename_async(
            c"/tmp/short",
            long_path,
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    };
    let long_open_async = unsafe {
        filesystem::open_async(
            long_path,
            filesystem::ISFS_OPEN_READ,
            Some(isfs_probe_callback),
            core::ptr::null_mut(),
        )
    };
    let long_path_async_callback_not_called = !ISFS_CALLBACK_DONE.load(Ordering::Acquire)
        && ISFS_CALLBACK_RESULT.load(Ordering::Acquire) == i32::MIN;
    logger.record(
        "ISFS sync path APIs reject overlong paths and preserve outputs",
        [
            long_read_dir,
            long_create_dir,
            long_create_file,
            long_get_attr,
            long_set_attr,
            long_get_usage,
            long_delete,
            long_rename_old,
            long_rename_new,
        ]
        .iter()
        .all(|result| *result == filesystem::ISFS_EINVAL)
            && matches!(long_open, Err(filesystem::ISFS_EINVAL))
            && long_path_count == 41
            && long_usage_a == 42
            && long_usage_b == 43
            && owner == 31
            && group == 32
            && attributes == 33
            && owner_perm == 34
            && group_perm == 35
            && other_perm == 36,
    );
    logger.record(
        "ISFS async path APIs reject overlong paths before callback",
        [
            long_read_dir_async,
            long_create_dir_async,
            long_create_file_async,
            long_get_attr_async,
            long_set_attr_async,
            long_get_usage_async,
            long_delete_async,
            long_rename_old_async,
            long_rename_new_async,
            long_open_async,
        ]
        .iter()
        .all(|result| *result == filesystem::ISFS_EINVAL)
            && long_path_async_callback_not_called
            && long_path_count == 41
            && long_usage_a == 42
            && long_usage_b == 43
            && owner == 31
            && group == 32
            && attributes == 33
            && owner_perm == 34
            && group_perm == 35
            && other_perm == 36,
    );
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
    reset_isfs_callback();
    // SAFETY: the initialized service is closed, so the call must return
    // before it can submit an IOS request or retain the output buffer.
    let get_stats_async = unsafe {
        filesystem::get_stats_async(&mut stats, Some(isfs_probe_callback), core::ptr::null_mut())
    };
    let get_stats_async_callback_not_called = !ISFS_CALLBACK_DONE.load(Ordering::Acquire)
        && ISFS_CALLBACK_RESULT.load(Ordering::Acquire) == i32::MIN;
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
        "ISFS get_stats_async rejects a closed service and preserves output",
        get_stats_async == filesystem::ISFS_EINVAL
            && get_stats_async_callback_not_called
            && stats
                == filesystem::IsfsStats {
                    a: 1,
                    b: 2,
                    c: 3,
                    d: 4,
                    e: 5,
                    f: 6,
                    g: 7,
                },
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

#[repr(align(32))]
struct AlignedIsfsBytes<const N: usize>([u8; N]);
