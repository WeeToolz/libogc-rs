use core::fmt::{self, Write};

use ogc_usb as usb;
use wiikeyboard_sys as keyboard;

use crate::probe_support::ProbeLogger;

pub fn run(logger: &mut ProbeLogger) {
    logger.section("KEYBOARD PROBE");

    let mut stdin_device = 0x1234usize;
    let start_result =
        keyboard::start_worker_before_stdin_install(|| -6, || stdin_device = 0x5678usize);
    logger.record(
        "worker creation failure preserves prior stdin",
        start_result == -6 && stdin_device == 0x1234,
    );

    probe_usb_hid(logger);

    logger.record("usb keyboard init", keyboard::initialize() == 0);
    // SAFETY: no callback is installed, so no borrowed state can outlive this call.
    logger.record("keyboard init", unsafe { keyboard::init(None) } == 0);
    logger.record(
        "keyboard disconnected by default",
        !keyboard::is_connected(),
    );
    logger.record("keyboard scan without device", keyboard::scan() == -1);
    logger.record(
        "keyboard led rejected without device",
        keyboard::set_led(keyboard::USBKEYBOARD_LEDNUM, true) == -1,
    );
    logger.record(
        "keyboard toggle rejected without device",
        keyboard::toggle_led(keyboard::USBKEYBOARD_LEDCAPS) == -1,
    );

    let mut event = keyboard::KeyboardEvent::default();
    logger.record(
        "keyboard event queue empty",
        // SAFETY: `event` is a valid unique output reference.
        unsafe { keyboard::get_event(&mut event) } == 0,
    );
    logger.record("keyboard flush empty", keyboard::flush_events() == 0);

    keyboard::close();
    logger.record("keyboard deinit", keyboard::deinit() == 1);
    let mut entries = [usb::UsbDeviceEntry::default(); 8];
    logger.record(
        "usb core device list works after keyboard deinit",
        usb::device_list(&mut entries, 0).is_ok(),
    );
}

fn probe_usb_hid(logger: &mut ProbeLogger) {
    logger.section("USB HID RAW PROBE");

    let init = usb::initialize();
    logger.record("usb core init", init == usb::USB_OK);
    if init != usb::USB_OK {
        return;
    }

    let mut entries = [usb::UsbDeviceEntry::default(); 8];
    match usb::device_list(&mut entries, usb::USB_CLASS_HID) {
        Ok(count) => {
            note_u32(logger, "hid device count", count as u32);
            for entry in entries.iter().take(count) {
                probe_hid_device(logger, *entry);
            }
        }
        Err(err) => note_i32(logger, "hid device list error", err.raw()),
    }
}

fn probe_hid_device(logger: &mut ProbeLogger, entry: usb::UsbDeviceEntry) {
    let mut line = LineBuf::new();
    let _ = write!(
        &mut line,
        "INFO hid device id={} vid={:04x} pid={:04x}",
        entry.device_id, entry.vid, entry.pid
    );
    logger.note(line.as_str());

    let mut fd = -1;
    let open = unsafe { usb::open_device(entry.device_id, entry.vid, entry.pid, &mut fd) };
    note_i32(logger, "hid open rc", open);
    if open != usb::USB_OK {
        return;
    }

    if let Ok(mut desc) = usb::descriptors(fd) {
        let report_ep = unsafe { log_hid_descriptors(logger, fd, &desc) };
        if let Some((endpoint, packet_len)) = report_ep {
            read_one_hid_report(logger, fd, endpoint, packet_len);
        } else {
            logger.note("hid raw read skipped: no interrupt IN endpoint");
        }
        // SAFETY: `desc` was returned by `get_descriptors` and is freed once.
        unsafe { usb::free_descriptors(&mut desc) };
    }

    let _ = unsafe { usb::close_device(&mut fd) };
}

unsafe fn log_hid_descriptors(
    logger: &mut ProbeLogger,
    fd: i32,
    desc: &usb::UsbDevDesc,
) -> Option<(u8, u16)> {
    let mut first_interrupt_in = None;
    let configs = desc.configurations;
    if configs.is_null() {
        return None;
    }

    for config_idx in 0..desc.b_num_configurations as usize {
        let config = unsafe { *configs.add(config_idx) };
        let interfaces = config.interfaces;
        if interfaces.is_null() {
            continue;
        }

        for interface_idx in 0..config.b_num_interfaces as usize {
            let interface = unsafe { *interfaces.add(interface_idx) };
            let mut line = LineBuf::new();
            let _ = write!(
                &mut line,
                "hid iface cfg={} if={} class={:02x} sub={:02x} proto={:02x}",
                config_idx,
                interface.b_interface_number,
                interface.b_interface_class,
                interface.b_interface_sub_class,
                interface.b_interface_protocol
            );
            logger.note(line.as_str());

            let report_size = usb::get_report_descriptor_size(fd, interface.b_interface_number);
            note_i32(logger, "hid report descriptor size", report_size);

            let endpoints = interface.endpoints;
            if endpoints.is_null() {
                continue;
            }
            for endpoint_idx in 0..interface.b_num_endpoints as usize {
                let endpoint = unsafe { *endpoints.add(endpoint_idx) };
                let max_packet_size = endpoint.w_max_packet_size;
                let mut ep_line = LineBuf::new();
                let _ = write!(
                    &mut ep_line,
                    "hid ep addr={:02x} attr={:02x} max={} interval={}",
                    endpoint.b_endpoint_address,
                    endpoint.bm_attributes,
                    max_packet_size,
                    endpoint.b_interval
                );
                logger.note(ep_line.as_str());

                if first_interrupt_in.is_none()
                    && endpoint.b_endpoint_address & usb::USB_ENDPOINT_IN != 0
                    && endpoint.bm_attributes & 0x03 == usb::USB_ENDPOINT_INTERRUPT
                {
                    first_interrupt_in =
                        Some((endpoint.b_endpoint_address, max_packet_size.min(32)));
                }
            }
        }
    }

    first_interrupt_in
}

fn read_one_hid_report(logger: &mut ProbeLogger, fd: i32, endpoint: u8, packet_len: u16) {
    let mut report = [0u8; 32];
    let len = packet_len.min(report.len() as u16);
    let rc = unsafe { usb::read_intr_msg(fd, endpoint, len, report.as_mut_ptr().cast()) };
    note_i32(logger, "hid raw read rc", rc);
    if rc >= 0 {
        let used = (rc as usize).min(report.len());
        let mut line = LineBuf::new();
        let _ = write!(&mut line, "hid raw");
        for byte in report.iter().take(used) {
            let _ = write!(&mut line, " {:02x}", byte);
        }
        logger.note(line.as_str());
    }
}

fn note_u32(logger: &mut ProbeLogger, label: &str, value: u32) {
    let mut line = LineBuf::new();
    let _ = write!(&mut line, "{}={}", label, value);
    logger.note(line.as_str());
}

fn note_i32(logger: &mut ProbeLogger, label: &str, value: i32) {
    let mut line = LineBuf::new();
    let _ = write!(&mut line, "{}={}", label, value);
    logger.note(line.as_str());
}

struct LineBuf {
    bytes: [u8; 160],
    len: usize,
}

impl LineBuf {
    const fn new() -> Self {
        Self {
            bytes: [0; 160],
            len: 0,
        }
    }

    fn as_str(&self) -> &str {
        core::str::from_utf8(&self.bytes[..self.len]).unwrap_or("")
    }
}

impl Write for LineBuf {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        let remaining = self.bytes.len().saturating_sub(self.len);
        if s.len() > remaining {
            return Err(fmt::Error);
        }
        self.bytes[self.len..self.len + s.len()].copy_from_slice(s.as_bytes());
        self.len += s.len();
        Ok(())
    }
}
