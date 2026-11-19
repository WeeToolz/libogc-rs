use core::cell::Cell;

use ogc_usb as usb;

use crate::probe_support::ProbeLogger;

pub fn run(logger: &mut ProbeLogger) {
    logger.section("USB HID DESCRIPTOR POLICY");
    probe_invalid_arguments(logger);
    probe_unavailable_transfers(logger);

    let mut descriptor = [
        9,
        usb::sys::USB_DT_HID,
        0x11,
        0x01,
        0,
        3,
        usb::sys::USB_DT_REPORT,
        0x34,
        0x12,
        usb::sys::USB_DT_REPORT,
        0x78,
        0x56,
        0x22,
        0xbc,
        0x9a,
    ];
    usb::sys::convert_hid_descriptor_lengths(&mut descriptor);
    logger.record(
        "HID lengths convert for every complete entry",
        descriptor
            == [
                9,
                usb::sys::USB_DT_HID,
                0x01,
                0x11,
                0,
                3,
                usb::sys::USB_DT_REPORT,
                0x12,
                0x34,
                usb::sys::USB_DT_REPORT,
                0x56,
                0x78,
                0x22,
                0x9a,
                0xbc,
            ],
    );

    let mut truncated_descriptor = [
        9,
        usb::sys::USB_DT_HID,
        0x11,
        0x01,
        0,
        3,
        usb::sys::USB_DT_REPORT,
        0x34,
        0x12,
        usb::sys::USB_DT_REPORT,
        0x78,
        0x56,
        0x22,
        0xbc,
    ];
    usb::sys::convert_hid_descriptor_lengths(&mut truncated_descriptor);
    logger.record(
        "HID truncated entry is left unchanged",
        truncated_descriptor[10..14] == [0x56, 0x78, 0x22, 0xbc],
    );

    let mut short = [9, 0x21, 0x11, 0x01, 0, 1, 0x22, 0x34];
    let original = short;
    usb::sys::convert_hid_descriptor_lengths(&mut short);
    logger.record("HID short header remains unchanged", short == original);

    let phase = Cell::new(0u8);
    let free_called = Cell::new(false);
    let suspend_called = Cell::new(false);
    let result = usb::sys::prepare_hid_v5_device_with(
        42,
        |id| {
            phase.set(if id == 42 { 1 } else { 9 });
            -7
        },
        |id, _descriptor| {
            phase.set(if id == 42 && phase.get() == 1 { 2 } else { 9 });
            usb::sys::USB_OK
        },
        |_descriptor| {
            free_called.set(phase.get() == 2);
            phase.set(3);
        },
        |_| {
            suspend_called.set(true);
            usb::sys::USB_OK
        },
    );
    logger.record(
        "HID v5 open checks and frees descriptors after best-effort resume",
        result == usb::sys::USB_OK
            && phase.get() == 3
            && free_called.get()
            && !suspend_called.get(),
    );

    let phase = Cell::new(0u8);
    let free_called = Cell::new(false);
    let suspend_called = Cell::new(false);
    let descriptor_error = -0x321;
    let result = usb::sys::prepare_hid_v5_device_with(
        42,
        |id| {
            phase.set(if id == 42 { 1 } else { 9 });
            usb::sys::USB_OK
        },
        |id, _descriptor| {
            phase.set(if id == 42 && phase.get() == 1 { 2 } else { 9 });
            descriptor_error
        },
        |_| free_called.set(true),
        |id| {
            suspend_called.set(id == 42 && phase.get() == 2);
            phase.set(3);
            usb::sys::USB_OK
        },
    );
    logger.record(
        "HID v5 open suspends and returns descriptor failure",
        result == descriptor_error
            && phase.get() == 3
            && !free_called.get()
            && suspend_called.get(),
    );

    let freed = Cell::new(false);
    let suspended = Cell::new(false);
    let result = usb::sys::prepare_hid_v5_device_with(
        42,
        |_| -1,
        |_, _| 7,
        |_| freed.set(true),
        |_| {
            suspended.set(true);
            -2
        },
    );
    logger.record(
        "HID v5 open accepts positive descriptor status",
        result == 0 && freed.get() && !suspended.get(),
    );

    let result = usb::sys::prepare_hid_v5_device_with(42, |_| 0, |_, _| -23, |_| {}, |_| -9);
    logger.record(
        "HID v5 open preserves descriptor error when suspend fails",
        result == -23,
    );
}

fn probe_invalid_arguments(logger: &mut ProbeLogger) {
    // These invalid inputs are rejected before any IOS request is made.
    // SAFETY: Null pointers are permitted by the respective API contracts.
    unsafe {
        logger.record(
            "USB open rejects null output",
            usb::open_device(0, 0, 0, core::ptr::null_mut()) == 1,
        );
        logger.record(
            "USB close rejects null output",
            usb::close_device(core::ptr::null_mut()) == 1,
        );
        logger.record(
            "USB device description rejects null output",
            usb::get_device_description(-1, core::ptr::null_mut()) == 1,
        );
        logger.record(
            "USB descriptors reject null output",
            usb::get_descriptors(-1, core::ptr::null_mut()) == 1,
        );
        let mut byte = 0u8;
        logger.record(
            "USB generic descriptor rejects invalid buffers",
            usb::get_generic_descriptor(-1, 1, 0, 0, core::ptr::null_mut(), 1) == 1
                && usb::get_generic_descriptor(-1, 1, 0, 0, (&mut byte as *mut u8).cast(), 0) == 1,
        );
        let mut short_report = [0u8; 7];
        let mut hid = usb::UsbHidDesc::default();
        logger.record(
            "USB HID descriptor rejects invalid buffers",
            usb::get_hid_descriptor(-1, 0, core::ptr::null_mut(), 9) == -4
                && usb::get_hid_descriptor(-1, 0, &mut hid, 8) == -4,
        );
        logger.record(
            "USB report descriptor rejects invalid buffers",
            usb::get_report_descriptor(-1, 0, core::ptr::null_mut(), 8) == -4
                && usb::get_report_descriptor(-1, 0, short_report.as_mut_ptr().cast(), 7) == -4,
        );
        logger.record(
            "USB configuration rejects null output",
            usb::get_configuration(-1, core::ptr::null_mut()) == -4,
        );
    }
}

#[repr(align(32))]
struct TransferData([u8; 32]);
#[repr(align(32))]
struct PacketSizes([u16; 1]);
static TRANSFER_CALLBACKS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

unsafe extern "C" fn transfer_callback(result: i32, _: *mut core::ffi::c_void) -> i32 {
    TRANSFER_CALLBACKS.fetch_add(1, core::sync::atomic::Ordering::Relaxed);
    result
}

fn probe_unavailable_transfers(logger: &mut ProbeLogger) {
    let mut data = TransferData([0xa5; 32]);
    let mut sizes = PacketSizes([32]);
    TRANSFER_CALLBACKS.store(0, core::sync::atomic::Ordering::Relaxed);
    // SAFETY: Both aligned buffers remain valid throughout each rejected call.
    unsafe {
        logger.record(
            "USB unavailable read_intr_msg",
            usb::read_intr_msg(-1, 0x81, 32, data.0.as_mut_ptr().cast()) == -6,
        );
        logger.record(
            "USB unavailable read_intr_msg_async",
            usb::read_intr_msg_async(
                -1,
                0x81,
                32,
                data.0.as_mut_ptr().cast(),
                Some(transfer_callback),
                core::ptr::null_mut(),
            ) == -6,
        );
        logger.record(
            "USB unavailable read_blk_msg",
            usb::read_blk_msg(-1, 0x81, 32, data.0.as_mut_ptr().cast()) == -6,
        );
        logger.record(
            "USB unavailable read_blk_msg_async",
            usb::read_blk_msg_async(
                -1,
                0x81,
                32,
                data.0.as_mut_ptr().cast(),
                Some(transfer_callback),
                core::ptr::null_mut(),
            ) == -6,
        );
        logger.record(
            "USB unavailable write_intr_msg",
            usb::write_intr_msg(-1, 1, 32, data.0.as_mut_ptr().cast()) == -6,
        );
        logger.record(
            "USB unavailable write_intr_msg_async",
            usb::write_intr_msg_async(
                -1,
                1,
                32,
                data.0.as_mut_ptr().cast(),
                Some(transfer_callback),
                core::ptr::null_mut(),
            ) == -6,
        );
        logger.record(
            "USB unavailable write_blk_msg",
            usb::write_blk_msg(-1, 1, 32, data.0.as_mut_ptr().cast()) == -6,
        );
        logger.record(
            "USB unavailable write_blk_msg_async",
            usb::write_blk_msg_async(
                -1,
                1,
                32,
                data.0.as_mut_ptr().cast(),
                Some(transfer_callback),
                core::ptr::null_mut(),
            ) == -6,
        );
        logger.record(
            "USB unavailable read_ctrl_msg",
            usb::read_ctrl_msg(-1, 0x80, 6, 0, 0, 32, data.0.as_mut_ptr().cast()) == -6,
        );
        logger.record(
            "USB unavailable write_ctrl_msg",
            usb::write_ctrl_msg(-1, 0, 9, 0, 0, 32, data.0.as_mut_ptr().cast()) == -6,
        );
        logger.record(
            "USB unavailable read_iso_msg",
            usb::read_iso_msg(
                -1,
                0x81,
                1,
                sizes.0.as_mut_ptr(),
                data.0.as_mut_ptr().cast(),
            ) == -6,
        );
        logger.record(
            "USB unavailable read_iso_msg_async",
            usb::read_iso_msg_async(
                -1,
                0x81,
                1,
                sizes.0.as_mut_ptr(),
                data.0.as_mut_ptr().cast(),
                Some(transfer_callback),
                core::ptr::null_mut(),
            ) == -6,
        );
        logger.record(
            "USB unavailable write_iso_msg",
            usb::write_iso_msg(-1, 1, 1, sizes.0.as_mut_ptr(), data.0.as_mut_ptr().cast()) == -6,
        );
        logger.record(
            "USB unavailable write_iso_msg_async",
            usb::write_iso_msg_async(
                -1,
                1,
                1,
                sizes.0.as_mut_ptr(),
                data.0.as_mut_ptr().cast(),
                Some(transfer_callback),
                core::ptr::null_mut(),
            ) == -6,
        );
    }
    logger.record(
        "USB rejected transfers leave buffers and callbacks untouched",
        data.0 == [0xa5; 32]
            && sizes.0 == [32]
            && TRANSFER_CALLBACKS.load(core::sync::atomic::Ordering::Relaxed) == 0,
    );
}
