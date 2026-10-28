use core::cell::Cell;

use ogc_usb as usb;

use crate::probe_support::ProbeLogger;

pub fn run(logger: &mut ProbeLogger) {
    logger.section("USB HID DESCRIPTOR POLICY");

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
}
