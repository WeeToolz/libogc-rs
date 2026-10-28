//! Shared controller discovery and input normalization for probes and examples.
//!
//! The manager merges GameCube PAD, WPAD, USB HID, and Bluetooth HID inputs into
//! a common button/axis model. Bluetooth HID support includes Switch Joy-Con and
//! Pro Controller report parsing plus a generic HID fallback for other pads.

use crate::*;
use ogc_bt_sync as bt;
use ogc_input as pad;
use ogc_usb as usb;
use wpad_sys as wpad;

extern crate alloc;
use alloc::boxed::Box;
use alloc::vec;
use alloc::vec::Vec;

use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};

use crate::usb_report_state::{
    UsbBusOwnership, UsbReportState, UsbRetirementWorker, usb_device_id_is_owned,
};
use crate::xbox_report;

const GC_SLOT_BASE: usize = 0;
const WPAD_SLOT_BASE: usize = GC_SLOT_BASE + MAX_GAMECUBE_CONTROLLERS;
const USB_SLOT_BASE: usize = WPAD_SLOT_BASE + MAX_WPAD_CONTROLLERS;
const USB_GAMECUBE_SLOT_BASE: usize = USB_SLOT_BASE + MAX_USB_CONTROLLERS;
const BT_SLOT_BASE: usize = USB_GAMECUBE_SLOT_BASE + MAX_USB_GAMECUBE_PORTS;
const BLUETOOTH_SEARCH_RETRY_FRAMES: u32 = 60;
const USB_REPORT_BYTES: usize = 64;
const USB_HID_DESCRIPTOR_MAX_BYTES: usize = 4096;

fn frame_deadline_reached(frame: u32, deadline: u32) -> bool {
    frame.wrapping_sub(deadline) < (u32::MAX / 2)
}

#[repr(align(32))]
struct UsbReportBuffer([u8; USB_REPORT_BYTES]);

struct UsbReadSlot {
    report: UnsafeCell<UsbReportBuffer>,
    completion: UsbReportState,
    device_id: AtomicI32,
}

// IOS owns a slot's bytes from submission until the callback publishes READY.
// Only the main scan reads completed bytes and starts another request. Slots
// have static lifetime, so dropping a manager cannot invalidate an IOS buffer.
unsafe impl Sync for UsbReadSlot {}

impl UsbReadSlot {
    const fn new() -> Self {
        Self {
            report: UnsafeCell::new(UsbReportBuffer([0; USB_REPORT_BYTES])),
            completion: UsbReportState::new(),
            device_id: AtomicI32::new(-1),
        }
    }
}

static USB_READ_SLOTS: [UsbReadSlot; MAX_USB_CONTROLLERS] =
    [const { UsbReadSlot::new() }; MAX_USB_CONTROLLERS];

struct UsbWriteSlot {
    report: UnsafeCell<UsbReportBuffer>,
    completion: UsbReportState,
    device_id: AtomicI32,
}

unsafe impl Sync for UsbWriteSlot {}

impl UsbWriteSlot {
    const fn new() -> Self {
        Self {
            report: UnsafeCell::new(UsbReportBuffer([0; USB_REPORT_BYTES])),
            completion: UsbReportState::new(),
            device_id: AtomicI32::new(-1),
        }
    }
}

static USB_WRITE_SLOTS: [UsbWriteSlot; MAX_USB_CONTROLLERS] =
    [const { UsbWriteSlot::new() }; MAX_USB_CONTROLLERS];
static USB_BUS_OWNERSHIP: UsbBusOwnership = UsbBusOwnership::new();
static NEXT_USB_OWNER: AtomicU32 = AtomicU32::new(1);
static USB_RETIREMENT_WORKER: UsbRetirementWorker = UsbRetirementWorker::new();
static USB_RETIREMENT_SETUP_FAILED: AtomicU32 = AtomicU32::new(0);
static USB_RETIREMENT_FDS: [AtomicI32; MAX_USB_CONTROLLERS] =
    [const { AtomicI32::new(-1) }; MAX_USB_CONTROLLERS];

fn next_usb_owner_id() -> u32 {
    loop {
        let owner = NEXT_USB_OWNER.fetch_add(1, Ordering::Relaxed);
        if owner != 0 && owner != u32::MAX {
            return owner;
        }
    }
}

unsafe extern "C" fn usb_controller_read_complete(
    result: i32,
    userdata: *mut core::ffi::c_void,
) -> i32 {
    let slot = userdata as usize;
    if let Some(read) = USB_READ_SLOTS.get(slot) {
        read.completion.complete(result);
    }
    usb::USB_OK
}

unsafe extern "C" fn usb_controller_write_complete(
    result: i32,
    userdata: *mut core::ffi::c_void,
) -> i32 {
    let slot = userdata as usize;
    if let Some(write) = USB_WRITE_SLOTS.get(slot) {
        write.completion.complete(result);
    }
    usb::USB_OK
}

/// USB HID controller slot discovered by descriptor probing.
pub struct UsbControllerDevice {
    pub connected: bool,
    pub device_id: i32,
    pub vid: u16,
    pub pid: u16,
    fd: i32,
    pub configuration: u8,
    pub interface: u8,
    pub endpoint: u8,
    endpoint_out: Option<u8>,
    endpoint_out_size: u16,
    pub endpoint_size: u16,
    pub state: ControllerState,
    pub metadata: ControllerMetadata,
    hid_decoder: Option<Box<usb_hid::UsbHidDecoder>>,
    gc_adapter: bool,
    switch_setup: Option<crate::usb_switch::UsbSwitchSetup>,
    switch_calibration: Option<bt::switch::SwitchCalibration>,
    gc_ports: [ControllerState; MAX_GAMECUBE_CONTROLLERS],
    gc_metadata: [ControllerMetadata; MAX_GAMECUBE_CONTROLLERS],
}

/// Normalized snapshot and raw hardware samples captured by the same scan.
/// The raw values allow applications to retain source-specific button and
/// pointer behavior without polling PAD or WPAD a second time.
#[derive(Clone, Copy)]
pub struct RawInputFrame {
    pub snapshot: ControllerSnapshot,
    pub pad_status: [pad::PadStatus; MAX_GAMECUBE_CONTROLLERS],
    pub pad_buttons_down: [u16; MAX_GAMECUBE_CONTROLLERS],
    pub pad_buttons_held: [u16; MAX_GAMECUBE_CONTROLLERS],
    pub pad_buttons_up: [u16; MAX_GAMECUBE_CONTROLLERS],
    pub wpad_buttons_down: [u32; MAX_WPAD_CONTROLLERS],
    pub wpad_buttons_held: [u32; MAX_WPAD_CONTROLLERS],
    pub wpad_buttons_up: [u32; MAX_WPAD_CONTROLLERS],
    pub wpad_extension_type: [i32; MAX_WPAD_CONTROLLERS],
    pub wpad_data: [Option<wpad::WpadData>; MAX_WPAD_CONTROLLERS],
}

#[derive(Clone, Copy, Default)]
struct RawInputButtons {
    pad_down: [u16; MAX_GAMECUBE_CONTROLLERS],
    pad_held: [u16; MAX_GAMECUBE_CONTROLLERS],
    pad_up: [u16; MAX_GAMECUBE_CONTROLLERS],
    wpad_down: [u32; MAX_WPAD_CONTROLLERS],
    wpad_held: [u32; MAX_WPAD_CONTROLLERS],
    wpad_up: [u32; MAX_WPAD_CONTROLLERS],
}

impl UsbControllerDevice {
    fn retire(&mut self) {
        self.connected = false;
        self.state.connected = false;
        self.state.buttons = Buttons::empty();
        self.state.axes = Axes::default();
        if let Some(decoder) = self.hid_decoder.as_mut() {
            decoder.reset();
        }
        for port in &mut self.gc_ports {
            port.connected = false;
            port.buttons = Buttons::empty();
            port.axes = Axes::default();
        }
    }

    const fn empty(slot: usize) -> Self {
        Self {
            connected: false,
            device_id: -1,
            vid: 0,
            pid: 0,
            fd: -1,
            configuration: 0,
            interface: 0,
            endpoint: 0,
            endpoint_out: None,
            endpoint_out_size: 0,
            endpoint_size: 0,
            hid_decoder: None,
            gc_adapter: false,
            switch_setup: None,
            switch_calibration: None,
            gc_ports: [ControllerState::empty(); MAX_GAMECUBE_CONTROLLERS],
            gc_metadata: [ControllerMetadata::empty(); MAX_GAMECUBE_CONTROLLERS],
            state: ControllerState {
                source: ControllerSource::UsbHid(slot as u8),
                kind: ControllerKind::GenericUsbHid,
                connected: false,
                buttons: Buttons::empty(),
                buttons_down: Buttons::empty(),
                buttons_up: Buttons::empty(),
                axes: Axes {
                    left_x: 0,
                    left_y: 0,
                    right_x: 0,
                    right_y: 0,
                    left_trigger: 0,
                    right_trigger: 0,
                },
            },
            metadata: ControllerMetadata::empty(),
        }
    }
}

struct UsbControllerBus {
    enabled: bool,
    initialized: bool,
    closing: bool,
    startup_hook: Option<fn(&'static str)>,
    devices: [UsbControllerDevice; MAX_USB_CONTROLLERS],
    profiles: [UsbControllerProfile; MAX_USB_CONTROLLERS],
    trace_errors: [Option<ControllerError>; MAX_USB_CONTROLLERS],
    errors: [Option<ControllerError>; MAX_USB_CONTROLLERS],
    reset_reasons: [Option<ResetReason>; MAX_USB_CONTROLLERS],
}

#[derive(Clone, Copy)]
struct UsbControllerProfile {
    vid: u16,
    pid: u16,
    bindings: usb_hid::HidButtonBindings,
}

impl UsbControllerProfile {
    const EMPTY: Self = Self {
        vid: 0,
        pid: 0,
        bindings: usb_hid::HidButtonBindings::empty(),
    };
}

impl UsbControllerBus {
    const fn new() -> Self {
        Self {
            enabled: false,
            initialized: false,
            closing: false,
            startup_hook: None,
            errors: [None; MAX_USB_CONTROLLERS],
            reset_reasons: [None; MAX_USB_CONTROLLERS],
            devices: [
                UsbControllerDevice::empty(0),
                UsbControllerDevice::empty(1),
                UsbControllerDevice::empty(2),
                UsbControllerDevice::empty(3),
            ],
            profiles: [UsbControllerProfile::EMPTY; MAX_USB_CONTROLLERS],
            trace_errors: [None; MAX_USB_CONTROLLERS],
        }
    }

    fn init(&mut self) {
        if self.enabled && !self.initialized {
            self.stage("USB INITIALIZE BEGIN");
            let code = usb::initialize();
            self.initialized = code == usb::USB_OK;
            if !self.initialized {
                self.errors[0] = Some(usb_error(0, "USB initialize", InputFailure::Source(code)));
            }
            self.stage(if self.initialized {
                "USB INITIALIZE OK"
            } else {
                "USB INITIALIZE FAILED"
            });
        }
    }

    fn stage(&self, message: &'static str) {
        if let Some(hook) = self.startup_hook {
            hook(message);
        }
    }

    fn refresh(&mut self) {
        if !self.enabled {
            return;
        }

        self.init();
        if !self.initialized {
            return;
        }

        let mut entries = [usb::UsbDeviceEntry::default(); 8];
        self.stage("USB DEVICE LIST BEGIN");
        let count = match usb::device_list(&mut entries, usb::USB_CLASS_HID) {
            Ok(count) if count <= entries.len() => count,
            Ok(_) => {
                self.errors[0] = Some(usb_error(
                    0,
                    "USB device list",
                    InputFailure::MalformedReport,
                ));
                return;
            }
            Err(error) => {
                self.errors[0] = Some(usb_error(
                    0,
                    "USB device list",
                    InputFailure::Source(error.raw()),
                ));
                self.stage("USB DEVICE LIST FAILED");
                // An enumeration failure cannot establish a trustworthy state.
                for slot in 0..MAX_USB_CONTROLLERS {
                    if self.devices[slot].connected {
                        self.devices[slot].retire();
                        self.reset_reasons[slot] = Some(ResetReason::SourceError(error.raw()));
                    }
                }
                return;
            }
        };
        self.stage(if count == 0 {
            "USB DEVICE LIST EMPTY"
        } else {
            "USB DEVICE LIST OK"
        });

        for (slot, device) in self.devices.iter_mut().enumerate() {
            if device.connected
                && !entries[..count].iter().any(|entry| {
                    entry.device_id == device.device_id
                        && entry.vid == device.vid
                        && entry.pid == device.pid
                })
            {
                device.retire();
                self.reset_reasons[slot] = Some(ResetReason::Disconnected);
            }
        }
        for entry in entries.iter().take(count) {
            if self
                .devices
                .iter()
                .any(|device| usb_device_id_is_owned(device.fd, device.device_id, entry.device_id))
            {
                continue;
            }

            let Some(slot) = self.devices.iter().enumerate().position(|(slot, device)| {
                !device.connected
                    && device.fd < 0
                    && USB_READ_SLOTS[slot].completion.is_idle()
                    && USB_WRITE_SLOTS[slot].completion.is_idle()
            }) else {
                break;
            };

            let mut device = UsbControllerDevice::empty(slot);
            let bindings = self
                .profiles
                .iter()
                .find(|profile| profile.vid == entry.vid && profile.pid == entry.pid)
                .map(|profile| profile.bindings)
                .unwrap_or_else(usb_hid::HidButtonBindings::empty);
            match open_usb_controller(&mut device, entry, self.startup_hook, bindings) {
                Ok(()) => {
                    self.devices[slot] = device;
                }
                Err(error) => {
                    self.errors[slot] = Some(error);
                    // A failed open still owns its descriptor until close succeeds.
                    self.devices[slot] = device;
                }
            }
        }
    }

    fn poll(&mut self, observer: Option<InputTraceObserver>, timestamp_ticks: Option<u64>) {
        if !self.enabled && !self.closing {
            return;
        }
        // poll() reports whether work progressed; it is not an I/O result.
        ogc_ipc::poll();
        for (slot, device) in self.devices.iter_mut().enumerate() {
            let read = &USB_READ_SLOTS[slot];
            let write = &USB_WRITE_SLOTS[slot];
            let now = ogc_system::time();
            if let Some(result) = write.completion.take_result() {
                let owner = write.device_id.load(Ordering::Acquire);
                write.completion.release();
                if device.connected
                    && owner == device.device_id
                    && let Some(setup) = device.switch_setup.as_mut()
                {
                    match setup.output_complete(result, now) {
                        Ok(Some(crate::usb_switch::UsbSwitchInputProgress::Ready(calibration))) => {
                            device.switch_calibration = Some(calibration);
                            device.state.kind = ControllerKind::SwitchPro;
                        }
                        Ok(Some(_)) | Ok(None) => {}
                        Err(error) => {
                            let (operation, failure, reason) = usb_switch_failure(error);
                            self.errors[slot] = Some(usb_error(slot, operation, failure));
                            self.reset_reasons[slot] = Some(reason);
                            device.retire();
                        }
                    }
                }
            }
            if device.connected
                && let Some(setup) = device.switch_setup.as_mut()
            {
                let timeout_operation = setup.timeout_operation();
                if let Err(error) = setup.expire(now) {
                    let (operation, failure, reason) = usb_switch_failure(error);
                    let operation = if failure == InputFailure::Timeout {
                        timeout_operation
                    } else {
                        operation
                    };
                    self.errors[slot] = Some(usb_error(slot, operation, failure));
                    self.reset_reasons[slot] = Some(reason);
                    device.retire();
                }
            }
            if let Some(code) = read.completion.take_result() {
                let owner = read.device_id.load(Ordering::Acquire);
                let report = unsafe { (*read.report.get()).0 };
                read.completion.release();
                if device.connected && owner == device.device_id {
                    if code < 0 {
                        if let Some(setup) = device.switch_setup.as_mut() {
                            let error = setup.input_failed(code);
                            let (operation, failure, reason) = usb_switch_failure(error);
                            self.errors[slot] = Some(usb_error(slot, operation, failure));
                            self.reset_reasons[slot] = Some(reason);
                        } else {
                            self.errors[slot] =
                                Some(usb_error(slot, "USB read", InputFailure::Source(code)));
                            self.reset_reasons[slot] = Some(ResetReason::SourceError(code));
                        }
                        device.retire();
                    } else if usb_hid::validate_usb_input_length(
                        code as usize,
                        report.len(),
                        device.endpoint_size as usize,
                    )
                    .is_err()
                    {
                        self.errors[slot] = Some(usb_error(
                            slot,
                            "USB read length",
                            InputFailure::MalformedReport,
                        ));
                        self.reset_reasons[slot] = Some(ResetReason::InvalidReport);
                        device.retire();
                    } else if code > 0 {
                        let used = code as usize;
                        let source = ControllerSource::UsbHid(slot as u8);
                        if let Some(observer) = observer {
                            let event = InputTraceEvent::RawReport {
                                source,
                                generation: None,
                                usb_identity: Some(UsbTraceIdentity {
                                    device_id: device.device_id,
                                    vid: device.vid,
                                    pid: device.pid,
                                }),
                                timestamp_ticks,
                                bytes: &report[..used],
                            };
                            if let Err(error) = observer(&event) {
                                self.trace_errors[slot] = Some(ControllerError {
                                    source,
                                    operation: "input trace observer",
                                    failure: InputFailure::Trace(error),
                                    cleanup_code: None,
                                });
                            }
                        }
                        let switch_setup_pending = device
                            .switch_setup
                            .as_ref()
                            .is_some_and(|setup| !setup.is_ready());
                        let decoded = if switch_setup_pending {
                            let result = device
                                .switch_setup
                                .as_mut()
                                .unwrap()
                                .receive(&report[..used], now);
                            match result {
                                Ok(crate::usb_switch::UsbSwitchInputProgress::Ignore)
                                | Ok(crate::usb_switch::UsbSwitchInputProgress::Send) => {
                                    UsbHidReadResult::SetupProgress
                                }
                                Ok(crate::usb_switch::UsbSwitchInputProgress::Ready(
                                    calibration,
                                )) => {
                                    device.switch_calibration = Some(calibration);
                                    device.state.kind = ControllerKind::SwitchPro;
                                    UsbHidReadResult::SetupProgress
                                }
                                Err(error) => {
                                    let (operation, failure, reason) = usb_switch_failure(error);
                                    self.errors[slot] = Some(usb_error(slot, operation, failure));
                                    self.reset_reasons[slot] = Some(reason);
                                    device.retire();
                                    UsbHidReadResult::SetupProgress
                                }
                            }
                        } else if let Some(calibration) = device.switch_calibration {
                            match crate::usb_switch::classify_ready_input(&report[..used]) {
                                crate::usb_switch::SwitchInputDispatch::Ignore => {
                                    UsbHidReadResult::OtherApplication
                                }
                                crate::usb_switch::SwitchInputDispatch::Malformed => {
                                    UsbHidReadResult::Error(InputFailure::MalformedReport)
                                }
                                crate::usb_switch::SwitchInputDispatch::Unsupported => {
                                    UsbHidReadResult::Error(InputFailure::UnsupportedReport)
                                }
                                crate::usb_switch::SwitchInputDispatch::StandardInput => {
                                    match map_switch_hid_report_calibrated(
                                        ControllerKind::SwitchPro,
                                        &report[..used],
                                        calibration,
                                    ) {
                                        Ok(mapped) => UsbHidReadResult::Controller(
                                            mapped.buttons,
                                            mapped.axes,
                                            ControllerMetadata {
                                                capabilities: ControllerCapabilities {
                                                    buttons: true,
                                                    left_stick: true,
                                                    right_stick: true,
                                                    triggers: true,
                                                    ..ControllerCapabilities::default()
                                                },
                                                pointer: None,
                                            },
                                        ),
                                        Err(error) => UsbHidReadResult::Error(match error {
                                            SwitchReportError::MalformedReport => {
                                                InputFailure::MalformedReport
                                            }
                                            SwitchReportError::UnsupportedKind => {
                                                InputFailure::UnsupportedReport
                                            }
                                            SwitchReportError::InvalidCalibration => {
                                                InputFailure::MalformedReport
                                            }
                                            SwitchReportError::MissingLeftCalibration
                                            | SwitchReportError::MissingRightCalibration => {
                                                InputFailure::MalformedReport
                                            }
                                        }),
                                    }
                                }
                            }
                        } else if device.gc_adapter {
                            match usb_hid::decode_usb_gamecube_adapter_report(&report[..used]) {
                                Ok(ports) => {
                                    device.gc_ports =
                                        usb_hid::usb_gamecube_port_states(slot as u8, ports);
                                    for metadata in &mut device.gc_metadata {
                                        *metadata = ControllerMetadata {
                                            capabilities: ControllerCapabilities {
                                                buttons: true,
                                                left_stick: true,
                                                right_stick: true,
                                                triggers: true,
                                                ..ControllerCapabilities::default()
                                            },
                                            pointer: None,
                                        };
                                    }
                                    UsbHidReadResult::AdapterPorts
                                }
                                Err(error) => {
                                    UsbHidReadResult::Error(InputFailure::HidReport(error))
                                }
                            }
                        } else if device.vid == 0x0f0d && device.pid == 0x0092 {
                            match map_pokken_hid_report(&report[..used]) {
                                Some(mapped) => UsbHidReadResult::Controller(
                                    mapped.buttons,
                                    mapped.axes.clamp_normalized(),
                                    ControllerMetadata {
                                        capabilities: ControllerCapabilities {
                                            buttons: true,
                                            left_stick: true,
                                            right_stick: true,
                                            triggers: true,
                                            ..ControllerCapabilities::default()
                                        },
                                        pointer: None,
                                    },
                                ),
                                None => UsbHidReadResult::Error(InputFailure::MalformedReport),
                            }
                        } else {
                            match device.hid_decoder.as_mut() {
                                Some(decoder) => {
                                    match decoder.decode_with_controls(&report[..used]) {
                                        Ok(decoded) if decoded.controller_report => {
                                            UsbHidReadResult::Controller(
                                                decoded.buttons,
                                                decoded.axes,
                                                hid_capabilities(decoded.capabilities),
                                            )
                                        }
                                        Ok(_) => UsbHidReadResult::OtherApplication,
                                        Err(error) => {
                                            UsbHidReadResult::Error(InputFailure::HidReport(error))
                                        }
                                    }
                                }
                                None => UsbHidReadResult::Error(InputFailure::UnsupportedReport),
                            }
                        };
                        match decoded {
                            UsbHidReadResult::Controller(buttons, axes, metadata) => {
                                device.state.connected = true;
                                device.state.buttons = buttons;
                                device.state.axes = axes;
                                device.metadata = metadata;
                            }
                            UsbHidReadResult::AdapterPorts => {}
                            UsbHidReadResult::OtherApplication => {}
                            UsbHidReadResult::SetupProgress => {}
                            UsbHidReadResult::Error(failure) => {
                                self.errors[slot] = Some(usb_error(slot, "USB report", failure));
                                self.reset_reasons[slot] = Some(ResetReason::InvalidReport);
                                device.state.connected = false;
                                device.state.buttons = Buttons::empty();
                                device.state.axes = Axes::default();
                                for port in &mut device.gc_ports {
                                    port.connected = false;
                                    port.buttons = Buttons::empty();
                                    port.axes = Axes::default();
                                }
                            }
                        }
                    }
                }
            }
            if device.connected && write.completion.start() {
                let output = device
                    .switch_setup
                    .as_mut()
                    .and_then(|setup| setup.next_output(now));
                if let Some(output) = output {
                    if let Some(endpoint) = device.endpoint_out {
                        let buffer = unsafe { &mut (*write.report.get()).0 };
                        buffer.fill(0);
                        buffer[..output.bytes().len()].copy_from_slice(output.bytes());
                        write.device_id.store(device.device_id, Ordering::Release);
                        let code = unsafe {
                            usb::write_intr_msg_async(
                                device.fd,
                                endpoint,
                                output.bytes().len() as u16,
                                buffer.as_mut_ptr().cast(),
                                Some(usb_controller_write_complete),
                                slot as *mut core::ffi::c_void,
                            )
                        };
                        if code < 0 {
                            write.completion.complete(code);
                        }
                    } else {
                        write.completion.release();
                        self.errors[slot] = Some(usb_error(
                            slot,
                            "Switch USB output endpoint missing",
                            InputFailure::UnsupportedReport,
                        ));
                        self.reset_reasons[slot] = Some(ResetReason::InvalidReport);
                        device.retire();
                    }
                } else {
                    write.completion.release();
                }
            }
            if !device.connected {
                // Never reuse a retired descriptor or IOS-owned buffer. A late
                // completion is drained above before close/reuse is permitted.
                if device.fd >= 0 && read.completion.is_idle() && write.completion.is_idle() {
                    let code = unsafe { usb::close_device(&mut device.fd) };
                    if code != usb::USB_OK {
                        if let Some(error) = self.errors[slot].as_mut() {
                            error.cleanup_code = Some(code);
                        } else {
                            self.errors[slot] = Some(ControllerError {
                                cleanup_code: Some(code),
                                ..usb_error(slot, "USB close", InputFailure::Source(code))
                            });
                        }
                    } else {
                        if let Some(error) = self.errors[slot].as_mut() {
                            error.cleanup_code = None;
                        }
                        *device = UsbControllerDevice::empty(slot);
                    }
                }
                continue;
            }
            if !read.completion.start() {
                continue;
            }
            read.device_id.store(device.device_id, Ordering::Release);
            if let Some(hook) = self.startup_hook {
                hook("USB ASYNC READ SUBMIT");
            }
            let code = unsafe {
                usb::read_intr_msg_async(
                    device.fd,
                    device.endpoint,
                    device.endpoint_size,
                    (*read.report.get()).0.as_mut_ptr().cast(),
                    Some(usb_controller_read_complete),
                    slot as *mut core::ffi::c_void,
                )
            };
            if code < 0 {
                // Rejected submission has no callback. Complete its owner state
                // here and neutralize controls in this same scan.
                read.completion.complete(code);
                self.errors[slot] = Some(usb_error(
                    slot,
                    "USB read submission",
                    InputFailure::Source(code),
                ));
                self.reset_reasons[slot] = Some(ResetReason::SourceError(code));
                device.retire();
            }
        }
    }
}

enum UsbHidReadResult {
    SetupProgress,
    Controller(Buttons, Axes, ControllerMetadata),
    AdapterPorts,
    OtherApplication,
    Error(InputFailure),
}

fn usb_switch_failure(
    error: crate::usb_switch::UsbSwitchSetupError,
) -> (&'static str, InputFailure, ResetReason) {
    use crate::usb_switch::UsbSwitchSetupError;
    match error {
        UsbSwitchSetupError::Output(code) => (
            "Switch USB output completion",
            InputFailure::Source(code),
            ResetReason::SourceError(code),
        ),
        UsbSwitchSetupError::Input(code) => (
            "Switch USB input completion",
            InputFailure::Source(code),
            ResetReason::SourceError(code),
        ),
        UsbSwitchSetupError::Timeout => (
            "Switch USB setup timeout",
            InputFailure::Timeout,
            ResetReason::Timeout,
        ),
        UsbSwitchSetupError::Protocol => (
            "Switch USB setup protocol",
            InputFailure::MalformedReport,
            ResetReason::InvalidReport,
        ),
    }
}

fn hid_capabilities(input: hid::HidControllerCapabilities) -> ControllerMetadata {
    ControllerMetadata {
        capabilities: ControllerCapabilities {
            buttons: input.buttons,
            left_stick: input.left_stick,
            right_stick: input.right_stick,
            triggers: input.triggers,
            ..ControllerCapabilities::default()
        },
        pointer: None,
    }
}

/// Bluetooth HID slot managed by pairing probes.
#[derive(Copy, Clone)]
pub struct BluetoothHidSlot {
    pub connected: bool,
    pub generation: u32,
    pub kind: ControllerKind,
    pub address: bt::BluetoothAddress,
    pub connection_handle: u16,
    pub report_count: u32,
    pub last_report_id: u8,
    pub last_ack_ok: bool,
    pub last_ack_subcommand: u8,
    pub last_disconnect_reason: Option<u8>,
    xbox_guide_held: bool,
    switch_calibration: Option<bt::switch::SwitchCalibration>,
    pub state: ControllerState,
}

impl BluetoothHidSlot {
    /// Creates an empty Bluetooth HID slot with stable slot numbering.
    pub const fn empty(slot: usize) -> Self {
        Self {
            connected: false,
            generation: 0,
            kind: ControllerKind::Unknown,
            address: bt::BluetoothAddress { bytes: [0; 6] },
            connection_handle: 0,
            report_count: 0,
            last_report_id: 0,
            last_ack_ok: false,
            last_ack_subcommand: 0,
            last_disconnect_reason: None,
            xbox_guide_held: false,
            switch_calibration: None,
            state: ControllerState {
                source: ControllerSource::BluetoothHid(slot as u8),
                kind: ControllerKind::Unknown,
                connected: false,
                buttons: Buttons::empty(),
                buttons_down: Buttons::empty(),
                buttons_up: Buttons::empty(),
                axes: Axes {
                    left_x: 0,
                    left_y: 0,
                    right_x: 0,
                    right_y: 0,
                    left_trigger: 0,
                    right_trigger: 0,
                },
            },
        }
    }
}

/// Stateful controller scanner with per-slot button transition tracking.
pub struct ControllerManager {
    usb: UsbControllerBus,
    usb_owner_id: u32,
    wpad_enabled: bool,
    bluetooth_scan_frames: u32,
    bluetooth_next_search_frame: u32,
    bluetooth_search_started: bool,
    bluetooth_guest_search_next: bool,
    bluetooth_startup_hook: Option<fn(&'static str)>,
    bluetooth_event_hook: Option<fn(&'static str)>,
    bluetooth_diagnostics_hook: Option<fn(bt::HostDiagnostics)>,
    input_scan_hook: Option<fn(&'static str)>,
    input_trace_observer: Option<InputTraceObserver>,
    input_trace_ticks: Option<u64>,
    bluetooth_reported_attempts: u32,
    bluetooth_reported_failures: u32,
    bluetooth_reported_hci_failures: u32,
    bluetooth_reported_auth_failures: u32,
    bluetooth_reported_key_requests: u32,
    bluetooth_reported_pin_requests: u32,
    bluetooth_stage: Option<bt::HostStage>,
    bluetooth_connections_seen: u32,
    bluetooth_disconnects_seen: u32,
    bluetooth: [BluetoothHidSlot; MAX_BLUETOOTH_HID_CONTROLLERS],
    joycon_pair: [u8; 2],
    joycon_pair_active: bool,
    transitions: InputTransitions<MAX_CONTROLLERS>,
    raw_button_transitions:
        RawButtonTransitions<{ MAX_GAMECUBE_CONTROLLERS + MAX_WPAD_CONTROLLERS }>,
    raw_wpad_extension_type: [i32; MAX_WPAD_CONTROLLERS],
    source_errors: [Option<ControllerError>; MAX_CONTROLLERS],
    player_sources: [ControllerSource; MAX_CONTROLLERS],
}

impl ControllerManager {
    /// Creates a manager without touching hardware.
    pub const fn new() -> Self {
        Self {
            usb: UsbControllerBus::new(),
            usb_owner_id: 0,
            wpad_enabled: false,
            bluetooth_scan_frames: 0,
            bluetooth_next_search_frame: 0,
            bluetooth_search_started: false,
            bluetooth_guest_search_next: true,
            bluetooth_startup_hook: None,
            bluetooth_event_hook: None,
            bluetooth_diagnostics_hook: None,
            input_scan_hook: None,
            input_trace_observer: None,
            input_trace_ticks: None,
            bluetooth_reported_attempts: 0,
            bluetooth_reported_failures: 0,
            bluetooth_reported_hci_failures: 0,
            bluetooth_reported_auth_failures: 0,
            bluetooth_reported_key_requests: 0,
            bluetooth_reported_pin_requests: 0,
            bluetooth_stage: None,
            bluetooth_connections_seen: 0,
            bluetooth_disconnects_seen: 0,
            bluetooth: [
                BluetoothHidSlot::empty(0),
                BluetoothHidSlot::empty(1),
                BluetoothHidSlot::empty(2),
                BluetoothHidSlot::empty(3),
                BluetoothHidSlot::empty(4),
                BluetoothHidSlot::empty(5),
            ],
            joycon_pair: [0, 0],
            joycon_pair_active: false,
            transitions: InputTransitions::new(),
            raw_button_transitions: RawButtonTransitions::new(),
            raw_wpad_extension_type: [wpad::WPAD_EXP_NONE as i32; MAX_WPAD_CONTROLLERS],
            source_errors: [None; MAX_CONTROLLERS],
            player_sources: [ControllerSource::Unknown; MAX_CONTROLLERS],
        }
    }

    /// Initializes the non-blocking controller subsystem used by the manager.
    ///
    /// USB HID and Bluetooth probing are opt-in. Bluetooth requests progress
    /// asynchronously; USB device setup still uses synchronous IOS requests.
    pub fn init(&mut self) {
        if pad::init() == 0 {
            self.source_errors[GC_SLOT_BASE] = Some(ControllerError {
                source: ControllerSource::GameCube(0),
                operation: "PAD initialize",
                failure: InputFailure::Source(0),
                cleanup_code: None,
            });
        }
    }

    /// Starts the shared Bluetooth adapter service. Pairing and reports progress
    /// during scan; this call does not wait for an HCI reply.
    pub fn enable_bluetooth_hid(&mut self) -> i32 {
        let rc = wpad::init();
        self.wpad_enabled = rc == wpad::WPAD_ERR_NONE as i32;
        rc
    }

    /// Enables Wii Remote input through the same Bluetooth service.
    pub fn enable_wpad(&mut self) -> i32 {
        self.enable_bluetooth_hid()
    }

    pub fn set_bluetooth_startup_hook(&mut self, hook: Option<fn(&'static str)>) {
        self.bluetooth_startup_hook = hook;
    }
    /// Sets an observer for new Bluetooth HID connections and disconnects.
    pub fn set_bluetooth_event_hook(&mut self, hook: Option<fn(&'static str)>) {
        self.bluetooth_event_hook = hook;
    }
    /// Sets an observer for new pairing and connection diagnostics.
    pub fn set_bluetooth_diagnostics_hook(&mut self, hook: Option<fn(bt::HostDiagnostics)>) {
        self.bluetooth_diagnostics_hook = hook;
    }
    /// Sets an optional observer for the blocking boundaries in one input scan.
    pub fn set_input_scan_hook(&mut self, hook: Option<fn(&'static str)>) {
        self.input_scan_hook = hook;
    }
    pub fn bluetooth_diagnostics(&self) -> bt::HostDiagnostics {
        wpad::bluetooth_diagnostics()
    }
    /// Starts adapter shutdown. Keep scanning until shutdown is complete.
    pub fn shutdown_bluetooth(&mut self) -> i32 {
        wpad::shutdown()
    }
    pub fn bluetooth_shutdown_complete(&self) -> bool {
        wpad::bluetooth_shutdown_complete()
    }

    fn poll_bluetooth(&mut self) -> bool {
        let code = wpad::scan_pads();
        // The event queue backpressure code means the producer retained input
        // for retry. Drain it even when this poll could not enqueue more.
        self.drain_bluetooth_input_events(wpad::bluetooth_hid_next_event);
        if code < 0 && code != wpad::WPAD_ERR_HID_QUEUE_FULL {
            self.record_wpad_poll_failure(code);
            return false;
        }
        let diagnostic = wpad::bluetooth_diagnostics();
        if self.bluetooth_stage != Some(diagnostic.stage) {
            self.bluetooth_stage = Some(diagnostic.stage);
            if let Some(hook) = self.bluetooth_startup_hook {
                hook(bluetooth_stage_text(diagnostic.stage));
            }
        }
        report_bluetooth_count_delta(
            diagnostic.connections,
            &mut self.bluetooth_connections_seen,
            "BLUETOOTH HID CONNECTED",
            self.bluetooth_event_hook,
        );
        if diagnostic.connection_attempts != self.bluetooth_reported_attempts
            || diagnostic.connection_failures != self.bluetooth_reported_failures
            || diagnostic.hci_command_failures != self.bluetooth_reported_hci_failures
            || diagnostic.authentication_failures != self.bluetooth_reported_auth_failures
            || diagnostic.link_key_requests != self.bluetooth_reported_key_requests
            || diagnostic.pin_code_requests != self.bluetooth_reported_pin_requests
        {
            self.bluetooth_reported_attempts = diagnostic.connection_attempts;
            self.bluetooth_reported_failures = diagnostic.connection_failures;
            self.bluetooth_reported_hci_failures = diagnostic.hci_command_failures;
            self.bluetooth_reported_auth_failures = diagnostic.authentication_failures;
            self.bluetooth_reported_key_requests = diagnostic.link_key_requests;
            self.bluetooth_reported_pin_requests = diagnostic.pin_code_requests;
            if let Some(hook) = self.bluetooth_diagnostics_hook {
                hook(diagnostic);
            }
        }
        report_bluetooth_count_delta(
            diagnostic.disconnects,
            &mut self.bluetooth_disconnects_seen,
            "BLUETOOTH HID DISCONNECTED",
            self.bluetooth_event_hook,
        );
        self.bluetooth_scan_frames = self.bluetooth_scan_frames.wrapping_add(1);
        if diagnostic.stage == bt::HostStage::Ready && diagnostic.searching {
            self.bluetooth_next_search_frame = self
                .bluetooth_scan_frames
                .wrapping_add(BLUETOOTH_SEARCH_RETRY_FRAMES);
        } else if diagnostic.stage == bt::HostStage::Ready
            && !diagnostic.searching
            && (!self.bluetooth_search_started
                || frame_deadline_reached(
                    self.bluetooth_scan_frames,
                    self.bluetooth_next_search_frame,
                ))
        {
            let result = if self.bluetooth_guest_search_next {
                wpad::search()
            } else {
                wpad::search_bluetooth_hid()
            };
            self.bluetooth_search_started = result == wpad::WPAD_ERR_NONE as i32;
            self.bluetooth_next_search_frame = self
                .bluetooth_scan_frames
                .wrapping_add(BLUETOOTH_SEARCH_RETRY_FRAMES);
            if self.bluetooth_search_started {
                self.bluetooth_guest_search_next = !self.bluetooth_guest_search_next;
            }
        }
        true
    }

    fn drain_bluetooth_input_events(
        &mut self,
        mut next_event: impl FnMut() -> Option<bt::BluetoothInputEvent>,
    ) -> usize {
        let mut drained = 0;
        while drained < bt::BLUETOOTH_INPUT_QUEUE_CAPACITY {
            let Some(event) = next_event() else {
                break;
            };
            drained += 1;
            self.process_bluetooth_input_event(event);
        }
        drained
    }

    fn process_bluetooth_input_event(&mut self, event: bt::BluetoothInputEvent) {
        let bt::BluetoothInputEvent {
            slot,
            generation,
            kind,
            bytes,
        } = event;
        match kind {
            bt::BluetoothInputKind::Connected {
                address,
                handle,
                family,
                calibration,
            } => {
                let Some(kind) = controller_kind_from_bt_family(family) else {
                    return;
                };
                if slot >= MAX_BLUETOOTH_HID_CONTROLLERS {
                    self.record_unsupported_bluetooth_slot(slot, "Bluetooth connect slot");
                    return;
                }
                if generation == 0 || generation <= self.bluetooth[slot].generation {
                    self.record_stale_bluetooth_event(slot, "Bluetooth stale connect");
                    return;
                }
                if self.bluetooth[slot].connected {
                    self.clear_bluetooth_slot(slot);
                }
                self.set_bluetooth_connected_with_generation(
                    slot, address, kind, handle, generation,
                );
                if let Some(calibration) = calibration {
                    if let Err(failure) = self.set_bluetooth_calibration_checked(slot, calibration)
                    {
                        self.record_bluetooth_failure(slot, kind, &[], failure, true);
                    }
                } else if is_switch_kind(kind) {
                    self.record_bluetooth_failure(
                        slot,
                        kind,
                        &[],
                        InputFailure::UnsupportedReport,
                        true,
                    );
                }
            }
            bt::BluetoothInputKind::Report { len } => {
                if slot >= MAX_BLUETOOTH_HID_CONTROLLERS {
                    self.record_unsupported_bluetooth_slot(slot, "Bluetooth report slot");
                    return;
                }
                let current = self.bluetooth[slot];
                if !current.connected || current.generation != generation {
                    self.record_stale_bluetooth_event(slot, "Bluetooth stale report");
                    return;
                }
                let used = usize::from(len);
                if used > bytes.len() {
                    self.record_bluetooth_failure(
                        slot,
                        current.kind,
                        &[],
                        InputFailure::MalformedReport,
                        true,
                    );
                    return;
                }
                if let Some(observer) = self.input_trace_observer {
                    let trace = InputTraceEvent::RawReport {
                        source: ControllerSource::BluetoothHid(slot as u8),
                        generation: Some(generation),
                        usb_identity: None,
                        timestamp_ticks: self.input_trace_ticks,
                        bytes: &bytes[..used],
                    };
                    if let Err(error) = observer(&trace) {
                        self.record_bluetooth_failure(
                            slot,
                            current.kind,
                            &[],
                            InputFailure::Trace(error),
                            false,
                        );
                    }
                }
                if self
                    .set_bluetooth_report_checked(slot, current.kind, &bytes[..used])
                    .is_err()
                    && let Some(hook) = self.bluetooth_event_hook
                {
                    hook("BLUETOOTH REPORT REJECTED");
                }
            }
            bt::BluetoothInputKind::Disconnected { reason } => {
                if slot >= MAX_BLUETOOTH_HID_CONTROLLERS {
                    self.record_unsupported_bluetooth_slot(slot, "Bluetooth disconnect slot");
                    return;
                }
                if !self.bluetooth[slot].connected || self.bluetooth[slot].generation != generation
                {
                    self.record_stale_bluetooth_event(slot, "Bluetooth stale disconnect");
                    return;
                }
                self.clear_bluetooth_slot(slot);
                self.bluetooth[slot].last_disconnect_reason = Some(reason);
            }
        }
    }

    fn record_wpad_poll_failure(&mut self, code: i32) {
        for slot in WPAD_SLOT_BASE..MAX_CONTROLLERS {
            if (USB_SLOT_BASE..BT_SLOT_BASE).contains(&slot) {
                continue;
            }
            let source = if slot < USB_SLOT_BASE {
                ControllerSource::WiiBluetooth((slot - WPAD_SLOT_BASE) as u8)
            } else {
                ControllerSource::BluetoothHid((slot - BT_SLOT_BASE) as u8)
            };
            self.source_errors[slot] = Some(ControllerError {
                source,
                operation: "WPAD poll",
                failure: InputFailure::Source(code),
                cleanup_code: None,
            });
            self.reset_input(slot, ResetReason::SourceError(code));
        }
    }

    fn record_unsupported_bluetooth_slot(&mut self, slot: usize, operation: &'static str) {
        let error_slot = BT_SLOT_BASE + MAX_BLUETOOTH_HID_CONTROLLERS - 1;
        self.source_errors[error_slot] = Some(ControllerError {
            source: ControllerSource::BluetoothHid(slot.min(u8::MAX as usize) as u8),
            operation,
            failure: InputFailure::UnsupportedReport,
            cleanup_code: None,
        });
    }

    fn record_stale_bluetooth_event(&mut self, slot: usize, operation: &'static str) {
        self.source_errors[BT_SLOT_BASE + slot] = Some(ControllerError {
            source: ControllerSource::BluetoothHid(slot as u8),
            operation,
            failure: InputFailure::UnsupportedReport,
            cleanup_code: None,
        });
    }

    /// Enables experimental USB HID controller probing for diagnostics.
    pub fn enable_usb_hid(&mut self) {
        if self.usb.enabled || self.usb.closing {
            return;
        }
        if self.usb_owner_id == 0 {
            self.usb_owner_id = next_usb_owner_id();
        }
        self.progress_usb_retirement();
        if USB_BUS_OWNERSHIP.try_acquire(self.usb_owner_id) {
            self.usb.enabled = true;
        } else {
            if self.usb.errors.iter().all(Option::is_none) {
                self.usb.errors[0] = Some(usb_error(
                    0,
                    "USB bus ownership",
                    InputFailure::AlreadyOwned,
                ));
            }
        }
    }

    /// Stops USB discovery and drains all requests and descriptors.
    ///
    /// Returns `Ok(false)` while IOS still owns a report buffer or a descriptor
    /// close is pending. Returns `Ok(true)` only after the request slots are
    /// idle, every descriptor is closed, and this manager releases bus ownership.
    /// Returns the latest close error while retaining the descriptor for retry.
    pub fn shutdown_usb_hid(&mut self) -> Result<bool, ControllerError> {
        if self.usb_owner_id == 0 || USB_BUS_OWNERSHIP.owner() != self.usb_owner_id {
            if USB_BUS_OWNERSHIP.is_retiring() {
                self.progress_usb_retirement();
            }
            if let Some(error) = self.usb.errors.iter().flatten().find(|error| {
                error.operation == "USB retirement close"
                    || error.operation == "USB retirement read"
            }) {
                return Err(*error);
            }
            if USB_BUS_OWNERSHIP.owner() == 0 {
                return Ok(true);
            }
            return Err(usb_error(
                0,
                "USB bus ownership",
                InputFailure::AlreadyOwned,
            ));
        }
        self.usb.enabled = false;
        self.usb.closing = true;
        for device in &mut self.usb.devices {
            device.retire();
        }
        self.usb.poll(None, None);
        if let Some(error) = self
            .usb
            .errors
            .iter()
            .flatten()
            .find(|error| error.cleanup_code.is_some())
        {
            return Err(*error);
        }
        let pending = self.usb.devices.iter().enumerate().any(|(slot, device)| {
            device.fd >= 0
                || !USB_READ_SLOTS[slot].completion.is_idle()
                || !USB_WRITE_SLOTS[slot].completion.is_idle()
        });
        if pending {
            return Ok(false);
        }
        let requests_idle = USB_READ_SLOTS.iter().all(|read| read.completion.is_idle())
            && USB_WRITE_SLOTS
                .iter()
                .all(|write| write.completion.is_idle());
        let descriptors_closed = self.usb.devices.iter().all(|device| device.fd < 0);
        if USB_BUS_OWNERSHIP.release_when_drained(
            self.usb_owner_id,
            requests_idle,
            descriptors_closed,
        ) {
            self.usb_owner_id = 0;
            self.usb.closing = false;
            Ok(true)
        } else {
            Err(usb_error(
                0,
                "USB bus ownership release",
                InputFailure::AlreadyOwned,
            ))
        }
    }

    fn progress_usb_retirement(&mut self) {
        let Some(_worker) = USB_RETIREMENT_WORKER.try_lock() else {
            return;
        };
        let owner = USB_BUS_OWNERSHIP.owner();
        if !USB_BUS_OWNERSHIP.is_retiring() || owner == self.usb_owner_id {
            return;
        }
        if USB_RETIREMENT_SETUP_FAILED.load(Ordering::Acquire) == owner {
            self.usb.errors[0] = Some(usb_error(
                0,
                "USB retirement setup",
                InputFailure::AlreadyOwned,
            ));
            return;
        }
        if !USB_BUS_OWNERSHIP.retirement_ready() {
            return;
        }
        ogc_ipc::poll();
        for slot in 0..MAX_USB_CONTROLLERS {
            let read = &USB_READ_SLOTS[slot];
            let write = &USB_WRITE_SLOTS[slot];
            if let Some(result) = read.completion.take_result() {
                if result < 0 {
                    self.usb.errors[slot] = Some(usb_error(
                        slot,
                        "USB retirement read",
                        InputFailure::Source(result),
                    ));
                    self.usb.reset_reasons[slot] = Some(ResetReason::SourceError(result));
                }
                read.completion.release();
            }
            if !read.completion.is_idle() {
                continue;
            }
            if let Some(result) = write.completion.take_result() {
                if result < 0 {
                    self.usb.errors[slot] = Some(usb_error(
                        slot,
                        "USB retirement write",
                        InputFailure::Source(result),
                    ));
                    self.usb.reset_reasons[slot] = Some(ResetReason::SourceError(result));
                }
                write.completion.release();
            }
            if !write.completion.is_idle() {
                continue;
            }
            let mut fd = USB_RETIREMENT_FDS[slot].load(Ordering::Acquire);
            if fd < 0 {
                continue;
            }
            let code = unsafe { usb::close_device(&mut fd) };
            if code == usb::USB_OK {
                USB_RETIREMENT_FDS[slot].store(-1, Ordering::Release);
                if self.usb.errors[slot]
                    .map(|error| error.operation == "USB retirement close")
                    .unwrap_or(false)
                {
                    self.usb.errors[slot] = None;
                } else if let Some(error) = self.usb.errors[slot].as_mut()
                    && error.cleanup_code.is_some()
                {
                    error.cleanup_code = None;
                }
            } else {
                if let Some(error) = self.usb.errors[slot].as_mut() {
                    if error.operation == "USB retirement read" {
                        error.cleanup_code = Some(code);
                    } else {
                        *error = ControllerError {
                            cleanup_code: Some(code),
                            ..usb_error(slot, "USB retirement close", InputFailure::Source(code))
                        };
                    }
                } else {
                    self.usb.errors[slot] = Some(ControllerError {
                        cleanup_code: Some(code),
                        ..usb_error(slot, "USB retirement close", InputFailure::Source(code))
                    });
                }
            }
        }
        let descriptors_closed = USB_RETIREMENT_FDS
            .iter()
            .all(|fd| fd.load(Ordering::Acquire) < 0);
        let requests_idle = USB_READ_SLOTS.iter().all(|read| read.completion.is_idle())
            && USB_WRITE_SLOTS
                .iter()
                .all(|write| write.completion.is_idle());
        if descriptors_closed && requests_idle {
            let owner = USB_BUS_OWNERSHIP.owner();
            let released =
                USB_BUS_OWNERSHIP.release_when_drained(owner, requests_idle, descriptors_closed);
            if !released {
                self.usb.errors[0] = Some(usb_error(
                    0,
                    "USB bus ownership release",
                    InputFailure::AlreadyOwned,
                ));
            }
        }
    }

    /// Sets a visible startup observer for USB calls. Disable it after startup
    /// so normal polling does not overwrite the application display.
    pub fn set_usb_startup_hook(&mut self, hook: Option<fn(&'static str)>) {
        self.usb.startup_hook = hook;
    }

    /// Set a synchronous borrowed-event observer. The callback must not call
    /// back into this manager or perform blocking I/O.
    pub fn set_input_trace_observer(&mut self, observer: Option<InputTraceObserver>) {
        self.input_trace_observer = observer;
    }

    /// Polls all supported controller sources and returns a normalized snapshot.
    pub fn scan(&mut self) -> ControllerSnapshot {
        self.scan_at_inner(None)
    }

    /// Scan and attach the caller's monotonic tick sample to trace events.
    pub fn scan_at(&mut self, timestamp_ticks: u64) -> ControllerSnapshot {
        self.scan_at_inner(Some(timestamp_ticks))
    }

    fn scan_at_inner(&mut self, timestamp_ticks: Option<u64>) -> ControllerSnapshot {
        let mut status = [pad::PadStatus::default(); MAX_GAMECUBE_CONTROLLERS];
        let gc_scan = pad::scan_pads_with_status(&mut status);
        self.record_pad_errors(&status);
        self.input_trace_ticks = timestamp_ticks;
        self.scan_after_pad(gc_scan, None).0
    }

    /// Returns raw PAD status and normalized controls from one hardware scan.
    pub fn scan_with_pad_status(
        &mut self,
        status: &mut [pad::PadStatus; MAX_GAMECUBE_CONTROLLERS],
    ) -> ControllerSnapshot {
        let gc_scan = pad::scan_pads_with_status(status);
        self.record_pad_errors(status);
        self.input_trace_ticks = None;
        self.scan_after_pad(gc_scan, None).0
    }

    /// Capture raw PAD and WPAD samples with the normalized snapshot. This
    /// performs one PAD scan and uses WPAD data copied during its normal poll.
    pub fn scan_with_raw(&mut self) -> RawInputFrame {
        self.scan_with_raw_at_inner(None)
    }

    /// Raw scan with the caller's monotonic tick sample attached to trace events.
    pub fn scan_with_raw_at(&mut self, timestamp_ticks: u64) -> RawInputFrame {
        self.scan_with_raw_at_inner(Some(timestamp_ticks))
    }

    fn scan_with_raw_at_inner(&mut self, timestamp_ticks: Option<u64>) -> RawInputFrame {
        let mut pad_status = [pad::PadStatus::default(); MAX_GAMECUBE_CONTROLLERS];
        if let Some(hook) = self.input_scan_hook {
            hook("PAD SCAN START");
        }
        let gc_scan = pad::scan_pads_with_status(&mut pad_status);
        if let Some(hook) = self.input_scan_hook {
            hook("PAD SCAN DONE");
        }
        self.record_pad_errors(&pad_status);
        self.input_trace_ticks = timestamp_ticks;
        let mut wpad_data = [None; MAX_WPAD_CONTROLLERS];
        let (snapshot, raw_buttons) = self.scan_after_pad(gc_scan, Some(&mut wpad_data));
        RawInputFrame {
            snapshot,
            pad_status,
            pad_buttons_down: raw_buttons.pad_down,
            pad_buttons_held: raw_buttons.pad_held,
            pad_buttons_up: raw_buttons.pad_up,
            wpad_buttons_down: raw_buttons.wpad_down,
            wpad_buttons_held: raw_buttons.wpad_held,
            wpad_buttons_up: raw_buttons.wpad_up,
            wpad_extension_type: self.raw_wpad_extension_type,
            wpad_data,
        }
    }

    fn record_pad_errors(&mut self, status: &[pad::PadStatus; MAX_GAMECUBE_CONTROLLERS]) {
        for (slot, sample) in status.iter().enumerate() {
            let code = sample.err as i32;
            if code != pad::PAD_ERR_NONE && code != pad::PAD_ERR_NO_CONTROLLER {
                self.source_errors[slot] = Some(ControllerError {
                    source: ControllerSource::GameCube(slot as u8),
                    operation: "PAD scan",
                    failure: InputFailure::Source(code),
                    cleanup_code: None,
                });
                self.reset_input(slot, ResetReason::SourceError(code));
            }
        }
    }

    fn sample_raw_buttons(
        &mut self,
        transition_slot: usize,
        source_slot: usize,
        sample: RawButtonSample,
    ) -> RawButtonFrame {
        match self.raw_button_transitions.sample(transition_slot, sample) {
            Ok(frame) => frame,
            Err(error) => {
                self.source_errors[source_slot] = Some(ControllerError {
                    source: sample.source,
                    operation: "raw button transitions",
                    failure: InputFailure::Transition(error),
                    cleanup_code: None,
                });
                RawButtonFrame::default()
            }
        }
    }

    fn dispatch_trace(&mut self, event: InputTraceEvent<'_>) {
        let Some(observer) = self.input_trace_observer else {
            return;
        };
        if let Err(failure) = observer(&event) {
            let source = match event {
                InputTraceEvent::RawReport { source, .. } => source,
                InputTraceEvent::Decoded { state, .. } => state.source,
                InputTraceEvent::Reset { reset, .. } => reset.state.source,
                InputTraceEvent::Error { error, .. } => error.source,
            };
            if let Some(slot) = trace_slot(source)
                && self.source_errors[slot].is_none()
            {
                self.source_errors[slot] = Some(ControllerError {
                    source,
                    operation: "input trace observer",
                    failure: InputFailure::Trace(failure),
                    cleanup_code: None,
                });
            }
        }
    }

    fn scan_after_pad(
        &mut self,
        gc_scan: u32,
        mut raw_wpad: Option<&mut [Option<wpad::WpadData>; MAX_WPAD_CONTROLLERS]>,
    ) -> (ControllerSnapshot, RawInputButtons) {
        let mut raw_buttons = RawInputButtons::default();
        if self.wpad_enabled
            && let Some(hook) = self.input_scan_hook
        {
            hook("WPAD POLL START");
        }
        let wpad_ready = !self.wpad_enabled || self.poll_bluetooth();
        if self.wpad_enabled
            && let Some(hook) = self.input_scan_hook
        {
            hook("WPAD POLL DONE");
        }
        if let Some(hook) = self.input_scan_hook {
            hook("USB RETIREMENT START");
        }
        self.progress_usb_retirement();
        if let Some(hook) = self.input_scan_hook {
            hook("USB RETIREMENT DONE");
            hook("USB OWNERSHIP START");
        }
        if !self.usb.enabled
            && !self.usb.closing
            && self.usb_owner_id != 0
            && USB_BUS_OWNERSHIP.try_acquire(self.usb_owner_id)
        {
            self.usb.enabled = true;
            if self.usb.errors[0]
                .map(|error| error.failure == InputFailure::AlreadyOwned)
                .unwrap_or(false)
            {
                self.usb.errors[0] = None;
            }
        }
        if let Some(hook) = self.input_scan_hook {
            hook("USB OWNERSHIP DONE");
            hook("USB REFRESH START");
        }
        self.usb.refresh();
        if let Some(hook) = self.input_scan_hook {
            hook("USB REFRESH DONE");
        }
        if let Some(hook) = self.input_scan_hook {
            hook("USB POLL START");
        }
        self.usb
            .poll(self.input_trace_observer, self.input_trace_ticks);
        if let Some(hook) = self.input_scan_hook {
            hook("USB POLL DONE");
        }
        for slot in 0..MAX_USB_CONTROLLERS {
            if let Some(error) = self.usb.errors[slot].take() {
                self.source_errors[USB_SLOT_BASE + slot] = Some(error);
            }
            if let Some(error) = self.usb.trace_errors[slot].take() {
                let destination = &mut self.source_errors[USB_SLOT_BASE + slot];
                if destination.is_none() {
                    *destination = Some(error);
                }
            }
            if let Some(reason) = self.usb.reset_reasons[slot].take() {
                if self.usb.devices[slot].gc_adapter {
                    for port in 0..MAX_GAMECUBE_CONTROLLERS {
                        self.reset_input(
                            USB_GAMECUBE_SLOT_BASE + slot * MAX_GAMECUBE_CONTROLLERS + port,
                            reason,
                        );
                    }
                } else {
                    self.reset_input(USB_SLOT_BASE + slot, reason);
                }
            }
        }

        let mut snapshot = ControllerSnapshot::empty();
        let mut seen = [false; MAX_CONTROLLERS];

        for chan in 0..MAX_GAMECUBE_CONTROLLERS {
            let source = ControllerSource::GameCube(chan as u8);
            let raw_frame = self.sample_raw_buttons(
                chan,
                chan,
                RawButtonSample {
                    source,
                    connected: gc_scan & (1 << chan) != 0,
                    held: pad::buttons_held(chan as i32) as u32,
                    down: pad::buttons_down(chan as i32) as u32,
                    up: pad::buttons_up(chan as i32) as u32,
                },
            );
            raw_buttons.pad_down[chan] = raw_frame.down as u16;
            raw_buttons.pad_held[chan] = raw_frame.held as u16;
            raw_buttons.pad_up[chan] = raw_frame.up as u16;
            if let Some(state) = read_gamecube_controller(chan as i32, gc_scan) {
                self.assign_with_transitions(&mut snapshot, &mut seen, GC_SLOT_BASE + chan, state);
            }
        }

        for chan in 0..MAX_WPAD_CONTROLLERS {
            if self.wpad_enabled
                && let Some(hook) = self.input_scan_hook
            {
                hook(match chan {
                    0 => "WPAD DATA 0 START",
                    1 => "WPAD DATA 1 START",
                    2 => "WPAD DATA 2 START",
                    _ => "WPAD DATA 3 START",
                });
            }
            let sample = if self.wpad_enabled && wpad_ready {
                // Copy once and use this exact sample for both the raw frame
                // and the normalized controller state.
                unsafe { wpad::data(chan as i32).as_ref().copied() }
            } else {
                None
            };
            if self.wpad_enabled
                && let Some(hook) = self.input_scan_hook
            {
                hook(match chan {
                    0 => "WPAD DATA 0 DONE",
                    1 => "WPAD DATA 1 DONE",
                    2 => "WPAD DATA 2 DONE",
                    _ => "WPAD DATA 3 DONE",
                });
            }
            if let Some(raw) = raw_wpad.as_deref_mut() {
                raw[chan] = sample;
            }
            if let Some(sample) = sample.filter(|sample| sample.err == wpad::WPAD_ERR_NONE) {
                self.raw_wpad_extension_type[chan] = sample.exp.type_;
            }
            let state = sample.and_then(|data| read_wpad_controller(chan as i32, data));
            let valid_raw = if state.is_some() { sample } else { None };
            let source = ControllerSource::WiiBluetooth(chan as u8);
            let raw_frame = self.sample_raw_buttons(
                MAX_GAMECUBE_CONTROLLERS + chan,
                WPAD_SLOT_BASE + chan,
                RawButtonSample {
                    source,
                    connected: state.is_some(),
                    held: valid_raw.map(|data| data.btns_h).unwrap_or(0),
                    down: valid_raw.map(|data| data.btns_d).unwrap_or(0),
                    up: valid_raw.map(|data| data.btns_u).unwrap_or(0),
                },
            );
            raw_buttons.wpad_down[chan] = raw_frame.down;
            raw_buttons.wpad_held[chan] = raw_frame.held;
            raw_buttons.wpad_up[chan] = raw_frame.up;
            if let Some(state) = state {
                let metadata = sample.map(wpad_metadata).unwrap_or_default();
                self.assign_with_transitions(
                    &mut snapshot,
                    &mut seen,
                    WPAD_SLOT_BASE + chan,
                    state,
                );
                snapshot.set_metadata(state.source, metadata);
            }
        }

        for slot in 0..MAX_USB_CONTROLLERS {
            let connected = self.usb.devices[slot].connected;
            let state = self.usb.devices[slot].state;
            if connected && self.usb.devices[slot].gc_adapter {
                for port in 0..MAX_GAMECUBE_CONTROLLERS {
                    let state = self.usb.devices[slot].gc_ports[port];
                    if state.connected {
                        let transition_slot =
                            USB_GAMECUBE_SLOT_BASE + slot * MAX_GAMECUBE_CONTROLLERS + port;
                        self.assign_with_transitions(
                            &mut snapshot,
                            &mut seen,
                            transition_slot,
                            state,
                        );
                        snapshot
                            .set_metadata(state.source, self.usb.devices[slot].gc_metadata[port]);
                    }
                }
                continue;
            }
            if connected && state.connected {
                self.assign_with_transitions(&mut snapshot, &mut seen, USB_SLOT_BASE + slot, state);
                snapshot.set_metadata(state.source, self.usb.devices[slot].metadata);
            }
        }

        let active_pair = if wpad_ready {
            self.active_joycon_pair()
        } else {
            None
        };
        for slot in 0..MAX_BLUETOOTH_HID_CONTROLLERS {
            if active_pair
                .map(|(left, right)| slot == left || slot == right)
                .unwrap_or(false)
            {
                continue;
            }
            let connected = self.bluetooth[slot].connected;
            let state = self.bluetooth[slot].state;
            if wpad_ready && connected && state.connected {
                self.assign_with_transitions(&mut snapshot, &mut seen, BT_SLOT_BASE + slot, state);
            }
        }

        if let Some((left, right)) = active_pair {
            let state =
                combine_joycon_pair(left, right, self.bluetooth[left], self.bluetooth[right]);
            self.assign_with_transitions(&mut snapshot, &mut seen, BT_SLOT_BASE + left, state);
        }

        for (idx, was_seen) in seen.iter().copied().enumerate() {
            if !was_seen {
                self.reset_input(idx, ResetReason::Disconnected);
            }
        }

        for state in snapshot.controllers().iter().copied() {
            self.dispatch_trace(InputTraceEvent::Decoded {
                timestamp_ticks: self.input_trace_ticks,
                id: snapshot.controller_id(state.source),
                state,
            });
        }
        snapshot.take_resets(&mut self.transitions);
        for reset in snapshot.resets().copied() {
            self.dispatch_trace(InputTraceEvent::Reset {
                timestamp_ticks: self.input_trace_ticks,
                reset,
            });
        }
        let trace_errors = self.source_errors;
        for error in trace_errors
            .into_iter()
            .flatten()
            .filter(|error| !matches!(error.failure, InputFailure::Trace(_)))
        {
            self.dispatch_trace(InputTraceEvent::Error {
                timestamp_ticks: self.input_trace_ticks,
                error,
            });
        }
        snapshot.take_errors(&mut self.source_errors);
        self.release_disconnected_players(&snapshot);

        (snapshot, raw_buttons)
    }

    /// Current non-WPAD Bluetooth HID slots.
    pub fn bluetooth_slots(&self) -> &[BluetoothHidSlot; MAX_BLUETOOTH_HID_CONTROLLERS] {
        &self.bluetooth
    }

    /// Current logical Joy-Con L+R pair slots, when paired.
    pub fn joycon_pair(&self) -> Option<(usize, usize)> {
        self.joycon_pair_active
            .then_some((self.joycon_pair[0] as usize, self.joycon_pair[1] as usize))
    }

    /// Current USB HID controller slots.
    pub fn usb_slots(&self) -> &[UsbControllerDevice; MAX_USB_CONTROLLERS] {
        &self.usb.devices
    }

    /// Bind one HID Button usage for an exact USB VID/PID profile. This mapping
    /// is retained for later reconnects and applied to a matching live decoder.
    pub fn bind_usb_hid_button(
        &mut self,
        vid: u16,
        pid: u16,
        usage: u16,
        button: Buttons,
    ) -> Result<(), usb_hid::UsbHidProfileError> {
        if vid == 0 && pid == 0 {
            return Err(usb_hid::UsbHidProfileError::InvalidIdentity);
        }
        if !(1..=64).contains(&usage) || button.is_empty() {
            return Err(usb_hid::UsbHidProfileError::InvalidBinding);
        }
        let profile_index = self
            .usb
            .profiles
            .iter()
            .position(|profile| profile.vid == vid && profile.pid == pid)
            .or_else(|| {
                self.usb
                    .profiles
                    .iter()
                    .position(|profile| profile.vid == 0 && profile.pid == 0)
            })
            .ok_or(usb_hid::UsbHidProfileError::Capacity)?;
        let profile = &mut self.usb.profiles[profile_index];
        profile.vid = vid;
        profile.pid = pid;
        if !profile.bindings.bind(usage, button) {
            return Err(usb_hid::UsbHidProfileError::InvalidBinding);
        }
        for device in self
            .usb
            .devices
            .iter_mut()
            .filter(|device| device.vid == vid && device.pid == pid && device.connected)
        {
            if let Some(decoder) = device.hid_decoder.as_mut()
                && !decoder.bind_button(usage, button)
            {
                return Err(usb_hid::UsbHidProfileError::InvalidBinding);
            }
        }
        Ok(())
    }

    /// Marks a Bluetooth HID slot as connected after pairing completes.
    pub fn set_bluetooth_connected(
        &mut self,
        slot: usize,
        address: bt::BluetoothAddress,
        kind: ControllerKind,
    ) {
        self.set_bluetooth_connected_with_handle(slot, address, kind, 0);
    }

    /// Marks a Bluetooth HID slot as connected and records the HCI connection handle.
    pub fn set_bluetooth_connected_with_handle(
        &mut self,
        slot: usize,
        address: bt::BluetoothAddress,
        kind: ControllerKind,
        connection_handle: u16,
    ) {
        self.set_bluetooth_connected_with_generation(slot, address, kind, connection_handle, 0);
    }

    fn set_bluetooth_connected_with_generation(
        &mut self,
        slot: usize,
        address: bt::BluetoothAddress,
        kind: ControllerKind,
        connection_handle: u16,
        generation: u32,
    ) {
        if let Some(bt_slot) = self.bluetooth.get_mut(slot) {
            if !bt_slot.connected
                || bt_slot.address != address
                || bt_slot.connection_handle != connection_handle
                || bt_slot.kind != kind
                || bt_slot.generation != generation
            {
                bt_slot.switch_calibration = None;
            }
            bt_slot.connected = true;
            bt_slot.generation = generation;
            bt_slot.kind = kind;
            bt_slot.address = address;
            bt_slot.connection_handle = connection_handle;
            bt_slot.last_disconnect_reason = None;
            bt_slot.state.connected = true;
            bt_slot.state.kind = kind;
        }
    }

    /// Store validated calibration for a ready Switch Bluetooth connection.
    /// This supports deterministic manager tests and explicit platform adapters.
    pub fn set_bluetooth_calibration_checked(
        &mut self,
        slot: usize,
        calibration: bt::switch::SwitchCalibration,
    ) -> Result<(), InputFailure> {
        let Some(bt_slot) = self.bluetooth.get(slot) else {
            return Err(InputFailure::Transition(TransitionError::InvalidSlot));
        };
        let kind = bt_slot.kind;
        if !matches!(
            kind,
            ControllerKind::SwitchPro | ControllerKind::JoyConLeft | ControllerKind::JoyConRight
        ) {
            return Err(InputFailure::UnsupportedReport);
        }
        if !switch_calibration_valid_for_kind(kind, calibration) {
            self.bluetooth[slot].switch_calibration = None;
            self.record_bluetooth_failure(slot, kind, &[], InputFailure::MalformedReport, false);
            return Err(InputFailure::MalformedReport);
        }
        self.bluetooth[slot].switch_calibration = Some(calibration);
        Ok(())
    }

    /// Parses and stores one Bluetooth HID input report for a slot.
    pub fn set_bluetooth_report(
        &mut self,
        slot: usize,
        kind: ControllerKind,
        report: &[u8],
    ) -> bool {
        self.set_bluetooth_report_checked(slot, kind, report)
            .is_ok()
    }

    /// Decode a report and retain the error in the next snapshot. A malformed
    /// known input report resets controls; ACK/status reports preserve them.
    pub fn set_bluetooth_report_checked(
        &mut self,
        slot: usize,
        kind: ControllerKind,
        report: &[u8],
    ) -> Result<(), InputFailure> {
        if slot >= MAX_BLUETOOTH_HID_CONTROLLERS {
            return Err(InputFailure::Transition(TransitionError::InvalidSlot));
        }
        let report = switch_report_from_hidp_input(report).unwrap_or(report);
        let result = self.decode_bluetooth_sample(slot, kind, report);
        if let Err(failure) = result {
            self.record_bluetooth_failure(slot, kind, report, failure, false);
        }
        result
    }

    fn record_bluetooth_failure(
        &mut self,
        slot: usize,
        kind: ControllerKind,
        report: &[u8],
        failure: InputFailure,
        force_reset: bool,
    ) {
        self.source_errors[BT_SLOT_BASE + slot] = Some(ControllerError {
            source: ControllerSource::BluetoothHid(slot as u8),
            operation: "Bluetooth report",
            failure,
            cleanup_code: None,
        });
        let is_switch_standard_report = matches!(
            kind,
            ControllerKind::JoyConLeft | ControllerKind::JoyConRight | ControllerKind::SwitchPro
        ) && matches!(report.first(), Some(0x30..=0x33));
        if force_reset
            || failure == InputFailure::MalformedReport
            || (failure == InputFailure::UnsupportedReport && is_switch_standard_report)
        {
            self.bluetooth[slot].state.connected = false;
            self.bluetooth[slot].state.buttons = Buttons::empty();
            self.bluetooth[slot].state.axes = Axes::default();
            // One malformed half invalidates the current logical pair.
            if let Some((left, right)) = self.joycon_pair()
                && (slot == left || slot == right)
            {
                self.clear_joycon_pair();
            }
            self.reset_input(BT_SLOT_BASE + slot, ResetReason::InvalidReport);
        }
    }

    fn decode_bluetooth_sample(
        &mut self,
        slot: usize,
        kind: ControllerKind,
        report: &[u8],
    ) -> Result<(), InputFailure> {
        let bt_slot = &mut self.bluetooth[slot];
        if let Some(report_id) = report.first().copied() {
            bt_slot.report_count = bt_slot.report_count.saturating_add(1);
            bt_slot.last_report_id = report_id;
        }
        let controls_changed = if kind == ControllerKind::XboxOne && report.first() == Some(&2) {
            let Some(xbox_report::XboxInput::Guide(held)) = xbox_report::decode(report) else {
                return Err(InputFailure::MalformedReport);
            };
            bt_slot.xbox_guide_held = held;
            if held {
                bt_slot.state.buttons.insert(Buttons::HOME);
            } else {
                bt_slot.state.buttons.remove(Buttons::HOME);
            }
            // A guide-only report cannot establish a complete input sample.
            bt_slot.state.connected
        } else if matches!(
            kind,
            ControllerKind::JoyConLeft | ControllerKind::JoyConRight | ControllerKind::SwitchPro
        ) && report.first() == Some(&SWITCH_HID_INPUT_SUBCOMMAND_ACK)
        {
            let ack = parse_switch_subcommand_ack(report).ok_or(InputFailure::MalformedReport)?;
            bt_slot.last_ack_ok = ack.ack;
            bt_slot.last_ack_subcommand = ack.subcommand;
            return Ok(());
        } else {
            let mut mapped = if matches!(
                kind,
                ControllerKind::JoyConLeft
                    | ControllerKind::JoyConRight
                    | ControllerKind::SwitchPro
            ) && matches!(report.first(), Some(0x30..=0x33))
            {
                let calibration = bt_slot
                    .switch_calibration
                    .ok_or(InputFailure::UnsupportedReport)?;
                map_switch_hid_report_calibrated(kind, report, calibration).map_err(|error| {
                    match error {
                        SwitchReportError::MissingLeftCalibration
                        | SwitchReportError::MissingRightCalibration
                        | SwitchReportError::UnsupportedKind => InputFailure::UnsupportedReport,
                        SwitchReportError::MalformedReport
                        | SwitchReportError::InvalidCalibration => InputFailure::MalformedReport,
                    }
                })?
            } else {
                map_bluetooth_hid_report(kind, report).ok_or_else(|| {
                    if report.is_empty()
                        || (matches!(
                            kind,
                            ControllerKind::JoyConLeft
                                | ControllerKind::JoyConRight
                                | ControllerKind::SwitchPro
                        ) && matches!(report.first(), Some(0x30..=0x33)))
                        || (kind == ControllerKind::XboxOne && report.first() == Some(&1))
                    {
                        InputFailure::MalformedReport
                    } else {
                        InputFailure::UnsupportedReport
                    }
                })?
            };
            if kind == ControllerKind::XboxOne && report.len() == 16 && bt_slot.xbox_guide_held {
                mapped.buttons.insert(Buttons::HOME);
            }
            bt_slot.connected = true;
            bt_slot.kind = mapped.kind;
            bt_slot.state.kind = mapped.kind;
            bt_slot.state.connected = true;
            bt_slot.state.buttons = mapped.buttons;
            bt_slot.state.axes = mapped.axes;
            true
        };
        if controls_changed {
            if let Some((left, right)) = self.active_joycon_pair() {
                let paired =
                    combine_joycon_pair(left, right, self.bluetooth[left], self.bluetooth[right]);
                self.transitions
                    .sample(BT_SLOT_BASE + left, paired)
                    .map_err(InputFailure::Transition)?;
            } else {
                self.transitions
                    .sample(BT_SLOT_BASE + slot, self.bluetooth[slot].state)
                    .map_err(InputFailure::Transition)?;
            }
        }
        Ok(())
    }

    /// Clears a Bluetooth HID slot and its transition history.
    pub fn clear_bluetooth_slot(&mut self, slot: usize) {
        if self
            .joycon_pair()
            .map(|(left, right)| slot == left || slot == right)
            .unwrap_or(false)
        {
            self.clear_joycon_pair();
        }
        if let Some(bt_slot) = self.bluetooth.get_mut(slot) {
            let generation = bt_slot.generation;
            *bt_slot = BluetoothHidSlot::empty(slot);
            bt_slot.generation = generation;
            self.reset_input(BT_SLOT_BASE + slot, ResetReason::Disconnected);
        }
    }

    /// Combines one connected Joy-Con L and one connected Joy-Con R into one logical controller.
    pub fn pair_joycons(&mut self) -> bool {
        let Some(left) = self.find_bluetooth_kind(ControllerKind::JoyConLeft) else {
            return false;
        };
        let Some(right) = self.find_bluetooth_kind(ControllerKind::JoyConRight) else {
            return false;
        };
        if self.joycon_pair() == Some((left, right)) {
            return true;
        }
        if self.joycon_pair_active {
            self.clear_joycon_pair();
        }
        self.joycon_pair = [left as u8, right as u8];
        self.joycon_pair_active = true;
        self.reset_input(BT_SLOT_BASE + left, ResetReason::Replaced);
        self.reset_input(BT_SLOT_BASE + right, ResetReason::Replaced);
        self.release_player_source(ControllerSource::BluetoothHid(left as u8));
        self.release_player_source(ControllerSource::BluetoothHid(right as u8));
        true
    }

    /// Splits a logical Joy-Con L+R pair back into individual Joy-Con controllers.
    pub fn clear_joycon_pair(&mut self) {
        if self.joycon_pair_active {
            let source =
                ControllerSource::BluetoothHidPair(self.joycon_pair[0], self.joycon_pair[1]);
            self.reset_input(
                BT_SLOT_BASE + self.joycon_pair[0] as usize,
                ResetReason::Replaced,
            );
            self.release_player_source(source);
            self.reset_input(
                BT_SLOT_BASE + self.joycon_pair[1] as usize,
                ResetReason::Replaced,
            );
        }
        self.joycon_pair_active = false;
    }

    fn active_joycon_pair(&self) -> Option<(usize, usize)> {
        let pair = self.joycon_pair()?;
        let left = self.bluetooth[pair.0];
        let right = self.bluetooth[pair.1];
        (left.connected
            && right.connected
            && left.state.connected
            && right.state.connected
            && left.kind == ControllerKind::JoyConLeft
            && right.kind == ControllerKind::JoyConRight)
            .then_some(pair)
    }

    fn find_bluetooth_kind(&self, kind: ControllerKind) -> Option<usize> {
        self.bluetooth
            .iter()
            .position(|slot| slot.connected && slot.state.connected && slot.kind == kind)
    }

    fn reset_input(&mut self, slot: usize, reason: ResetReason) {
        if let Err(error) = self.transitions.reset(slot, reason) {
            self.source_errors[slot] = Some(ControllerError {
                source: ControllerSource::Unknown,
                operation: "input reset",
                failure: InputFailure::Transition(error),
                cleanup_code: None,
            });
        }
    }

    fn push_with_transitions(
        &mut self,
        snapshot: &mut ControllerSnapshot,
        seen: &mut [bool; MAX_CONTROLLERS],
        slot: usize,
        mut state: ControllerState,
    ) {
        let id = match self.transitions.sample(slot, state) {
            Ok(id) => id,
            Err(error) => {
                self.source_errors[slot] = Some(ControllerError {
                    source: state.source,
                    operation: "input transition",
                    failure: InputFailure::Transition(error),
                    cleanup_code: None,
                });
                return;
            }
        };
        let Some(sample) = self.transitions.take(slot) else {
            return;
        };
        state = sample;
        seen[slot] = true;
        snapshot.push_identified(state, id);
    }

    fn assign_with_transitions(
        &mut self,
        snapshot: &mut ControllerSnapshot,
        seen: &mut [bool; MAX_CONTROLLERS],
        slot: usize,
        state: ControllerState,
    ) {
        let mut one = ControllerSnapshot::empty();
        self.push_with_transitions(&mut one, seen, slot, state);
        let Some(state) = one.controllers().first().copied() else {
            return;
        };
        let player = self.assigned_player(state.source).or_else(|| {
            let player = self.assign_new_player(state.source)?;
            self.player_sources[player] = state.source;
            Some(player)
        });
        if let Some(player) = player {
            snapshot.assign_player(player, state);
        }
        if let Some(id) = one.controller_id(state.source) {
            snapshot.push_identified(state, id);
        }
    }

    fn assigned_player(&self, source: ControllerSource) -> Option<usize> {
        if source == ControllerSource::Unknown {
            return None;
        }
        self.player_sources
            .iter()
            .position(|assigned| *assigned == source)
    }

    fn assign_new_player(&self, source: ControllerSource) -> Option<usize> {
        if source == ControllerSource::Unknown {
            return None;
        }
        if let Some(player) = preferred_player_slot(source) {
            return (self.player_sources[player] == ControllerSource::Unknown).then_some(player);
        }
        self.player_sources
            .iter()
            .position(|source| *source == ControllerSource::Unknown)
    }

    fn release_disconnected_players(&mut self, snapshot: &ControllerSnapshot) {
        for player_source in &mut self.player_sources {
            if *player_source == ControllerSource::Unknown {
                continue;
            }
            if !snapshot
                .controllers()
                .iter()
                .any(|controller| controller.source == *player_source)
            {
                *player_source = ControllerSource::Unknown;
            }
        }
    }

    fn release_player_source(&mut self, source: ControllerSource) {
        for player_source in &mut self.player_sources {
            if *player_source == source {
                *player_source = ControllerSource::Unknown;
            }
        }
    }
}

fn trace_slot(source: ControllerSource) -> Option<usize> {
    match source {
        ControllerSource::GameCube(slot) if (slot as usize) < MAX_GAMECUBE_CONTROLLERS => {
            Some(slot as usize)
        }
        ControllerSource::WiiBluetooth(slot) if (slot as usize) < MAX_WPAD_CONTROLLERS => {
            Some(WPAD_SLOT_BASE + slot as usize)
        }
        ControllerSource::UsbHid(slot) if (slot as usize) < MAX_USB_CONTROLLERS => {
            Some(USB_SLOT_BASE + slot as usize)
        }
        ControllerSource::UsbGameCube { adapter_slot, .. }
            if (adapter_slot as usize) < MAX_USB_CONTROLLERS =>
        {
            Some(USB_SLOT_BASE + adapter_slot as usize)
        }
        ControllerSource::BluetoothHid(slot) if (slot as usize) < MAX_BLUETOOTH_HID_CONTROLLERS => {
            Some(BT_SLOT_BASE + slot as usize)
        }
        ControllerSource::BluetoothHidPair(left, _)
            if (left as usize) < MAX_BLUETOOTH_HID_CONTROLLERS =>
        {
            Some(BT_SLOT_BASE + left as usize)
        }
        _ => None,
    }
}

impl Drop for ControllerManager {
    fn drop(&mut self) {
        if self.usb_owner_id == 0 || !USB_BUS_OWNERSHIP.begin_retirement(self.usb_owner_id) {
            return;
        }
        self.usb.enabled = false;
        self.usb.closing = false;
        for (slot, device) in self.usb.devices.iter_mut().enumerate() {
            if device.fd >= 0 {
                USB_RETIREMENT_FDS[slot].store(device.fd, Ordering::Release);
                device.fd = -1;
            }
        }
        if !USB_BUS_OWNERSHIP.finish_retirement(self.usb_owner_id) {
            USB_RETIREMENT_SETUP_FAILED.store(self.usb_owner_id, Ordering::Release);
        }
    }
}

fn combine_joycon_pair(
    left_slot: usize,
    right_slot: usize,
    left: BluetoothHidSlot,
    right: BluetoothHidSlot,
) -> ControllerState {
    ControllerState {
        source: ControllerSource::BluetoothHidPair(left_slot as u8, right_slot as u8),
        kind: ControllerKind::JoyConPair,
        connected: true,
        buttons: left.state.buttons | right.state.buttons,
        buttons_down: Buttons::empty(),
        buttons_up: Buttons::empty(),
        axes: Axes {
            left_x: left.state.axes.left_x,
            left_y: left.state.axes.left_y,
            right_x: right.state.axes.right_x,
            right_y: right.state.axes.right_y,
            left_trigger: left.state.axes.left_trigger,
            right_trigger: right.state.axes.right_trigger,
        },
    }
}

fn controller_kind_from_bt_family(family: bt::ControllerFamily) -> Option<ControllerKind> {
    match family {
        bt::ControllerFamily::WiiRemote => None,
        bt::ControllerFamily::SwitchPro => Some(ControllerKind::SwitchPro),
        bt::ControllerFamily::JoyConLeft => Some(ControllerKind::JoyConLeft),
        bt::ControllerFamily::JoyConRight => Some(ControllerKind::JoyConRight),
        bt::ControllerFamily::Xbox => Some(ControllerKind::XboxOne),
        bt::ControllerFamily::Generic => Some(ControllerKind::GenericBluetoothHid),
    }
}

const fn is_switch_kind(kind: ControllerKind) -> bool {
    matches!(
        kind,
        ControllerKind::SwitchPro | ControllerKind::JoyConLeft | ControllerKind::JoyConRight
    )
}

fn switch_calibration_valid_for_kind(
    kind: ControllerKind,
    calibration: bt::switch::SwitchCalibration,
) -> bool {
    use bt::switch::SwitchAxisCalibration;

    let valid_axis = |axis: SwitchAxisCalibration| {
        axis.normalize(axis.min) == Some(-32767)
            && axis.normalize(axis.center) == Some(0)
            && axis.normalize(axis.max) == Some(32767)
    };
    let valid_stick =
        |stick: bt::switch::SwitchStickCalibration| valid_axis(stick.x) && valid_axis(stick.y);
    let left_valid = calibration.left.is_some_and(valid_stick);
    let right_valid = calibration.right.is_some_and(valid_stick);
    match kind {
        ControllerKind::SwitchPro => left_valid && right_valid,
        ControllerKind::JoyConLeft => left_valid,
        ControllerKind::JoyConRight => right_valid,
        _ => false,
    }
}

fn preferred_player_slot(source: ControllerSource) -> Option<usize> {
    match source {
        ControllerSource::GameCube(chan) | ControllerSource::WiiBluetooth(chan) => {
            Some(chan as usize).filter(|idx| *idx < MAX_CONTROLLERS)
        }
        ControllerSource::UsbGameCube { adapter_slot, port } => {
            Some(adapter_slot as usize * MAX_GAMECUBE_CONTROLLERS + port as usize)
                .filter(|idx| *idx < MAX_CONTROLLERS)
        }
        _ => None,
    }
}

fn report_bluetooth_count_delta(
    current: u32,
    observed: &mut u32,
    event: &'static str,
    hook: Option<fn(&'static str)>,
) {
    let count = current.saturating_sub(*observed);
    *observed = current;
    if let Some(hook) = hook {
        for _ in 0..count {
            hook(event);
        }
    }
}

impl Default for ControllerManager {
    fn default() -> Self {
        Self::new()
    }
}

fn read_gamecube_controller(chan: i32, scan: u32) -> Option<ControllerState> {
    if scan & (1 << chan) == 0 {
        return None;
    }
    let held = pad::buttons_held(chan);
    let axes = Axes {
        left_x: normalize_i8(pad::stick_x(chan)),
        left_y: normalize_i8(pad::stick_y(chan)),
        right_x: normalize_i8(pad::substick_x(chan)),
        right_y: normalize_i8(pad::substick_y(chan)),
        left_trigger: pad::trigger_l(chan),
        right_trigger: pad::trigger_r(chan),
    };
    let buttons = map_gamecube_buttons(held);
    let connected = true;
    connected.then_some(ControllerState {
        source: ControllerSource::GameCube(chan as u8),
        kind: ControllerKind::GameCube,
        connected,
        buttons,
        buttons_down: Buttons::empty(),
        buttons_up: Buttons::empty(),
        axes,
    })
}

fn read_wpad_controller(chan: i32, sample: wpad::WpadData) -> Option<ControllerState> {
    let mut exp_type = 0u32;
    // SAFETY: `exp_type` is a valid output for the duration of the call.
    if unsafe { wpad::probe(chan, &mut exp_type) } != wpad::WPAD_ERR_NONE as i32 {
        return None;
    }

    if sample.err != wpad::WPAD_ERR_NONE {
        return None;
    }
    let (kind, buttons, axes) = map_wpad_state(&sample);
    let mut edge_sample = sample;
    edge_sample.btns_h = sample.btns_d;
    let (_, down, _) = map_wpad_state(&edge_sample);
    edge_sample.btns_h = sample.btns_u;
    let (_, up, _) = map_wpad_state(&edge_sample);

    Some(ControllerState {
        source: ControllerSource::WiiBluetooth(chan as u8),
        kind,
        connected: true,
        buttons,
        buttons_down: down,
        buttons_up: up,
        axes,
    })
}

fn wpad_metadata(sample: wpad::WpadData) -> ControllerMetadata {
    let expansion = sample.data_present & wpad::WPAD_DATA_EXPANSION != 0;
    let classic = sample.exp.type_ == wpad::WPAD_EXP_CLASSIC as i32;
    let classic_pro = classic && unsafe { sample.exp.u.classic.type_ == 2 };
    let stick = expansion
        && matches!(
            sample.exp.type_,
            value if value == wpad::WPAD_EXP_NUNCHUK as i32
                || value == wpad::WPAD_EXP_CLASSIC as i32
                || value == wpad::WPAD_EXP_GUITARHERO3 as i32
        );
    let pointer = (sample.data_present & wpad::WPAD_DATA_IR != 0 && sample.ir.valid != 0)
        .then(|| PointerMetadata::new(sample.ir.x, sample.ir.y, sample.ir.vres))
        .flatten();
    ControllerMetadata {
        capabilities: ControllerCapabilities {
            buttons: true,
            left_stick: stick,
            right_stick: classic,
            triggers: classic && !classic_pro,
            pointer: pointer.is_some(),
            motion: sample.data_present & wpad::WPAD_DATA_ACCEL != 0,
            expansion,
        },
        pointer,
    }
}

fn map_wpad_state(data: &wpad::WpadData) -> (ControllerKind, Buttons, Axes) {
    let raw = data.btns_h;
    let mut axes = Axes::default();
    if data.exp.type_ == wpad::WPAD_EXP_CLASSIC as i32 {
        let classic = unsafe { data.exp.u.classic };
        let pro = classic.type_ == 2;
        let mut buttons = if pro {
            Buttons::empty()
        } else {
            map_wpad_buttons(raw)
        };
        for (mask, button) in [
            (wpad::WPAD_CLASSIC_BUTTON_A, Buttons::A),
            (wpad::WPAD_CLASSIC_BUTTON_B, Buttons::B),
            (wpad::WPAD_CLASSIC_BUTTON_X, Buttons::X),
            (wpad::WPAD_CLASSIC_BUTTON_Y, Buttons::Y),
            (wpad::WPAD_CLASSIC_BUTTON_UP, Buttons::UP),
            (wpad::WPAD_CLASSIC_BUTTON_DOWN, Buttons::DOWN),
            (wpad::WPAD_CLASSIC_BUTTON_LEFT, Buttons::LEFT),
            (wpad::WPAD_CLASSIC_BUTTON_RIGHT, Buttons::RIGHT),
            (wpad::WPAD_CLASSIC_BUTTON_PLUS, Buttons::START),
            (wpad::WPAD_CLASSIC_BUTTON_MINUS, Buttons::SELECT),
            (wpad::WPAD_CLASSIC_BUTTON_HOME, Buttons::HOME),
            (wpad::WPAD_CLASSIC_BUTTON_FULL_L, Buttons::L),
            (wpad::WPAD_CLASSIC_BUTTON_FULL_R, Buttons::R),
            (wpad::WPAD_CLASSIC_BUTTON_FULL_L, Buttons::SELECT),
            (wpad::WPAD_CLASSIC_BUTTON_FULL_R, Buttons::START),
            (wpad::WPAD_CLASSIC_BUTTON_ZL, Buttons::ZL),
            (wpad::WPAD_CLASSIC_BUTTON_ZR, Buttons::ZR),
        ] {
            map_if_u32(&mut buttons, raw, mask, button);
        }
        if pro {
            map_if_u32(&mut buttons, raw, 0x0002, Buttons::L_STICK);
            map_if_u32(&mut buttons, raw, 0x0001, Buttons::R_STICK);
        }
        axes.left_x = normalize_calibrated_axis(
            classic.ljs.pos.x,
            classic.ljs.min.x,
            classic.ljs.center.x,
            classic.ljs.max.x,
        );
        axes.left_y = normalize_calibrated_axis(
            classic.ljs.pos.y,
            classic.ljs.min.y,
            classic.ljs.center.y,
            classic.ljs.max.y,
        );
        axes.right_x = normalize_calibrated_axis(
            classic.rjs.pos.x,
            classic.rjs.min.x,
            classic.rjs.center.x,
            classic.rjs.max.x,
        );
        axes.right_y = normalize_calibrated_axis(
            classic.rjs.pos.y,
            classic.rjs.min.y,
            classic.rjs.center.y,
            classic.rjs.max.y,
        );
        axes.left_trigger = (u16::from(classic.ls_raw.min(31)) * 255 / 31) as u8;
        axes.right_trigger = (u16::from(classic.rs_raw.min(31)) * 255 / 31) as u8;
        return (
            if pro {
                ControllerKind::WiiUPro
            } else {
                ControllerKind::WiiClassic
            },
            buttons,
            axes,
        );
    }
    let mut buttons = map_wpad_buttons(raw);
    if data.exp.type_ == wpad::WPAD_EXP_NUNCHUK as i32 {
        map_if_u32(&mut buttons, raw, wpad::WPAD_NUNCHUK_BUTTON_Z, Buttons::Z);
        map_if_u32(&mut buttons, raw, wpad::WPAD_NUNCHUK_BUTTON_C, Buttons::L);
        let nunchuk = unsafe { data.exp.u.nunchuk };
        axes.left_x = normalize_calibrated_axis(
            nunchuk.js.pos.x,
            nunchuk.js.min.x,
            nunchuk.js.center.x,
            nunchuk.js.max.x,
        );
        axes.left_y = normalize_calibrated_axis(
            nunchuk.js.pos.y,
            nunchuk.js.min.y,
            nunchuk.js.center.y,
            nunchuk.js.max.y,
        );
    } else if data.exp.type_ == wpad::WPAD_EXP_GUITARHERO3 as i32 {
        let guitar = unsafe { data.exp.u.gh3 };
        axes.left_x = normalize_calibrated_axis(
            guitar.js.pos.x,
            guitar.js.min.x,
            guitar.js.center.x,
            guitar.js.max.x,
        );
        axes.left_y = normalize_calibrated_axis(
            guitar.js.pos.y,
            guitar.js.min.y,
            guitar.js.center.y,
            guitar.js.max.y,
        );
        for (mask, button) in [
            (wpad::WPAD_GUITAR_HERO_3_BUTTON_STRUM_UP, Buttons::UP),
            (wpad::WPAD_GUITAR_HERO_3_BUTTON_STRUM_DOWN, Buttons::DOWN),
            (wpad::WPAD_GUITAR_HERO_3_BUTTON_GREEN, Buttons::A),
            (wpad::WPAD_GUITAR_HERO_3_BUTTON_RED, Buttons::B),
            (wpad::WPAD_GUITAR_HERO_3_BUTTON_PLUS, Buttons::START),
            (wpad::WPAD_GUITAR_HERO_3_BUTTON_MINUS, Buttons::SELECT),
            (wpad::WPAD_GUITAR_HERO_3_BUTTON_YELLOW, Buttons::LEFT),
            (wpad::WPAD_GUITAR_HERO_3_BUTTON_BLUE, Buttons::RIGHT),
            (wpad::WPAD_GUITAR_HERO_3_BUTTON_ORANGE, Buttons::X),
        ] {
            map_if_u32(&mut buttons, raw, mask, button);
        }
    }
    (ControllerKind::WiiRemote, buttons, axes)
}

fn map_gamecube_buttons(raw: u16) -> Buttons {
    let mut buttons = Buttons::empty();
    map_if_u16(&mut buttons, raw, pad::PAD_BUTTON_A, Buttons::A);
    map_if_u16(&mut buttons, raw, pad::PAD_BUTTON_B, Buttons::B);
    map_if_u16(&mut buttons, raw, pad::PAD_BUTTON_X, Buttons::X);
    map_if_u16(&mut buttons, raw, pad::PAD_BUTTON_Y, Buttons::Y);
    map_if_u16(&mut buttons, raw, pad::PAD_BUTTON_UP, Buttons::UP);
    map_if_u16(&mut buttons, raw, pad::PAD_BUTTON_DOWN, Buttons::DOWN);
    map_if_u16(&mut buttons, raw, pad::PAD_BUTTON_LEFT, Buttons::LEFT);
    map_if_u16(&mut buttons, raw, pad::PAD_BUTTON_RIGHT, Buttons::RIGHT);
    map_if_u16(&mut buttons, raw, pad::PAD_BUTTON_START, Buttons::START);
    map_if_u16(&mut buttons, raw, pad::PAD_TRIGGER_L, Buttons::L);
    map_if_u16(&mut buttons, raw, pad::PAD_TRIGGER_R, Buttons::R);
    map_if_u16(&mut buttons, raw, pad::PAD_TRIGGER_Z, Buttons::Z);
    buttons
}

fn map_wpad_buttons(raw: u32) -> Buttons {
    let mut buttons = Buttons::empty();
    map_if_u32(&mut buttons, raw, wpad::WPAD_BUTTON_A, Buttons::A);
    map_if_u32(&mut buttons, raw, wpad::WPAD_BUTTON_B, Buttons::B);
    map_if_u32(&mut buttons, raw, wpad::WPAD_BUTTON_1, Buttons::X);
    map_if_u32(&mut buttons, raw, wpad::WPAD_BUTTON_2, Buttons::Y);
    map_if_u32(&mut buttons, raw, wpad::WPAD_BUTTON_UP, Buttons::UP);
    map_if_u32(&mut buttons, raw, wpad::WPAD_BUTTON_DOWN, Buttons::DOWN);
    map_if_u32(&mut buttons, raw, wpad::WPAD_BUTTON_LEFT, Buttons::LEFT);
    map_if_u32(&mut buttons, raw, wpad::WPAD_BUTTON_RIGHT, Buttons::RIGHT);
    map_if_u32(&mut buttons, raw, wpad::WPAD_BUTTON_PLUS, Buttons::START);
    map_if_u32(&mut buttons, raw, wpad::WPAD_BUTTON_MINUS, Buttons::SELECT);
    map_if_u32(&mut buttons, raw, wpad::WPAD_BUTTON_HOME, Buttons::HOME);
    buttons
}

fn usb_error(slot: usize, operation: &'static str, failure: InputFailure) -> ControllerError {
    ControllerError {
        source: ControllerSource::UsbHid(slot as u8),
        operation,
        failure,
        cleanup_code: None,
    }
}

/// Leave the descriptor in `device` on failure; the bus owns the single cleanup
/// path and records its result. No early return discards a close failure.
fn open_usb_controller(
    device: &mut UsbControllerDevice,
    entry: &usb::UsbDeviceEntry,
    hook: Option<fn(&'static str)>,
    bindings: usb_hid::HidButtonBindings,
) -> Result<(), ControllerError> {
    let slot = match device.state.source {
        ControllerSource::UsbHid(slot) => slot as usize,
        _ => 0,
    };
    if entry.vid == 0 && entry.pid == 0 {
        return Err(usb_error(
            slot,
            "USB identity",
            InputFailure::MalformedReport,
        ));
    }
    if let Some(hook) = hook {
        hook("USB OPEN DEVICE BEGIN");
    }
    let code = unsafe { usb::open_device(entry.device_id, entry.vid, entry.pid, &mut device.fd) };
    if code != usb::USB_OK {
        return Err(usb_error(slot, "USB open", InputFailure::Source(code)));
    }
    device.device_id = entry.device_id;
    device.vid = entry.vid;
    device.pid = entry.pid;
    device.gc_adapter = entry.vid == 0x057e && entry.pid == 0x0337;
    let mut desc = usb::UsbDevDesc::default();
    if let Some(hook) = hook {
        hook("USB DESCRIPTORS BEGIN");
    }
    let code = unsafe { usb::get_descriptors(device.fd, &mut desc) };
    let candidates = if code == usb::USB_OK {
        find_hid_interfaces(&desc)
    } else {
        Vec::new()
    };
    unsafe {
        usb::free_descriptors(&mut desc);
    }
    if code != usb::USB_OK {
        return Err(usb_error(
            slot,
            "USB descriptors",
            InputFailure::Source(code),
        ));
    }
    let switch_pro_usb = device.vid == 0x057e && device.pid == 0x2009;
    let first_candidate = candidates
        .iter()
        .copied()
        .find(|candidate| {
            if device.gc_adapter {
                candidate.endpoint_size >= 37 && candidate.endpoint_out.is_some()
            } else if switch_pro_usb {
                usb_hid::is_switch_pro_usb_interface(*candidate)
            } else {
                true
            }
        })
        .ok_or_else(|| {
            usb_error(
                slot,
                "USB controller interface",
                InputFailure::UnsupportedReport,
            )
        })?;
    if let Some(hook) = hook {
        hook("USB CONFIGURATION BEGIN");
    }
    let code = usb::set_configuration(device.fd, first_candidate.configuration);
    if code != usb::USB_OK {
        return Err(usb_error(
            slot,
            "USB configuration",
            InputFailure::Source(code),
        ));
    }
    let (candidate, hid_decoder) = if device.gc_adapter {
        (first_candidate, None)
    } else if switch_pro_usb {
        // This exact Nintendo identity has a calibrated Switch report path.
        (first_candidate, None)
    } else if device.vid == 0x0f0d && device.pid == 0x0092 {
        // This identified Pokken device has an explicit report profile below.
        let candidate = first_candidate;
        (candidate, None)
    } else {
        let mut active_configuration = first_candidate.configuration;
        let selected = usb_hid::select_controller_interface(&candidates, bindings, |candidate| {
            if candidate.configuration != active_configuration {
                let code = usb::set_configuration(device.fd, candidate.configuration);
                if code != usb::USB_OK {
                    return Err(usb_error(
                        slot,
                        "USB configuration",
                        InputFailure::Source(code),
                    ));
                }
                active_configuration = candidate.configuration;
            }
            if let Some(hook) = hook {
                hook("USB INTERFACE BEGIN");
            }
            let code = usb::set_alternative_interface(
                device.fd,
                candidate.interface,
                candidate.alternate_setting,
            );
            if code != usb::USB_OK {
                return Err(usb_error(slot, "USB interface", InputFailure::Source(code)));
            }
            if let Some(hook) = hook {
                hook("USB HID REPORT DESCRIPTOR SIZE BEGIN");
            }
            let descriptor_size = usb::report_descriptor_size(device.fd, candidate.interface)
                .map_err(|error| {
                    usb_error(
                        slot,
                        "USB HID report descriptor size",
                        InputFailure::Source(error.raw()),
                    )
                })?;
            if descriptor_size == 0 || descriptor_size > USB_HID_DESCRIPTOR_MAX_BYTES {
                return Ok(None);
            }
            let mut descriptor = vec![0; descriptor_size];
            if let Some(hook) = hook {
                hook("USB HID REPORT DESCRIPTOR BEGIN");
            }
            usb::report_descriptor(device.fd, candidate.interface, &mut descriptor).map_err(
                |error| {
                    usb_error(
                        slot,
                        "USB HID report descriptor",
                        InputFailure::Source(error.raw()),
                    )
                },
            )?;
            Ok(Some(descriptor))
        })
        .map_err(|error| match error {
            usb_hid::UsbHidInterfaceLoadError::Transport(error) => error,
            usb_hid::UsbHidInterfaceLoadError::Selection(error) => {
                let failure = match error {
                    usb_hid::UsbHidSelectionError::Descriptor(error) => {
                        InputFailure::HidDescriptor(error)
                    }
                    usb_hid::UsbHidSelectionError::Unsupported => InputFailure::UnsupportedReport,
                };
                usb_error(slot, "USB HID controller interface", failure)
            }
        })?;
        (selected.candidate, Some(selected.decoder))
    };
    if candidate.configuration != first_candidate.configuration {
        let code = usb::set_configuration(device.fd, candidate.configuration);
        if code != usb::USB_OK {
            return Err(usb_error(
                slot,
                "USB configuration",
                InputFailure::Source(code),
            ));
        }
    }
    if let Some(hook) = hook {
        hook("USB INTERFACE BEGIN");
    }
    let code =
        usb::set_alternative_interface(device.fd, candidate.interface, candidate.alternate_setting);
    if code != usb::USB_OK {
        return Err(usb_error(slot, "USB interface", InputFailure::Source(code)));
    }
    device.configuration = candidate.configuration;
    device.interface = candidate.interface;
    device.endpoint = candidate.endpoint;
    device.endpoint_out = candidate.endpoint_out;
    device.endpoint_size = candidate.endpoint_size;
    device.endpoint_out_size = candidate.endpoint_out_size;
    device.hid_decoder = hid_decoder;
    if switch_pro_usb {
        let ticks_per_second = ogc_video::timer_clock_hz();
        device.switch_setup = Some(crate::usb_switch::UsbSwitchSetup::new(ticks_per_second));
    }
    if device.gc_adapter {
        let output_endpoint = candidate.endpoint_out.ok_or_else(|| {
            usb_error(
                slot,
                "USB GameCube adapter output endpoint",
                InputFailure::UnsupportedReport,
            )
        })?;
        usb_hid::initialize_usb_gamecube_adapter(|report| unsafe {
            let mut output = UsbReportBuffer([0; USB_REPORT_BYTES]);
            output.0[..report.len()].copy_from_slice(report);
            usb::write_intr_msg(
                device.fd,
                output_endpoint,
                report.len() as u16,
                output.0.as_mut_ptr().cast(),
            )
        })
        .map_err(|code| {
            usb_error(
                slot,
                "USB GameCube adapter initialization report",
                InputFailure::Source(code),
            )
        })?;
    }
    device.connected = true;
    // Enumeration is not a complete input sample. Wait for a decoded report.
    device.state.connected = false;
    if let Some(hook) = hook {
        hook("USB CONTROLLER OPEN OK");
    }
    Ok(())
}

fn find_hid_interfaces(desc: &usb::UsbDevDesc) -> Vec<usb_hid::UsbHidInterfaceCandidate> {
    if desc.configurations.is_null() {
        return Vec::new();
    }

    let mut candidates = Vec::new();
    for config_idx in 0..desc.b_num_configurations as usize {
        let config = unsafe { *desc.configurations.add(config_idx) };
        if config.interfaces.is_null() {
            continue;
        }

        for interface_idx in 0..config.b_num_interfaces as usize {
            let interface = unsafe { *config.interfaces.add(interface_idx) };
            if interface.b_interface_class != usb::USB_CLASS_HID {
                continue;
            }
            if interface.b_interface_sub_class == usb::USB_SUBCLASS_BOOT
                && (interface.b_interface_protocol == usb::USB_PROTOCOL_KEYBOARD
                    || interface.b_interface_protocol == usb::USB_PROTOCOL_MOUSE)
            {
                continue;
            }
            if interface.endpoints.is_null() {
                continue;
            }

            let endpoint_out = (0..interface.b_num_endpoints as usize)
                .map(|idx| unsafe { *interface.endpoints.add(idx) })
                .find(|endpoint| {
                    endpoint.bm_attributes & 0x03 == usb::USB_ENDPOINT_INTERRUPT
                        && endpoint.b_endpoint_address & usb::USB_ENDPOINT_IN == 0
                        && endpoint.w_max_packet_size != 0
                        && endpoint.w_max_packet_size & 0xf800 == 0
                })
                .map(|endpoint| (endpoint.b_endpoint_address, endpoint.w_max_packet_size));

            for endpoint_idx in 0..interface.b_num_endpoints as usize {
                let endpoint = unsafe { *interface.endpoints.add(endpoint_idx) };
                if endpoint.bm_attributes & 0x03 != usb::USB_ENDPOINT_INTERRUPT {
                    continue;
                }
                if endpoint.b_endpoint_address & usb::USB_ENDPOINT_IN == 0 {
                    continue;
                }
                if endpoint.w_max_packet_size == 0 {
                    continue;
                }
                // The IOS interrupt read path accepts a byte count, not USB
                // high-bandwidth transaction flags. Reject unsupported sizes.
                if endpoint.w_max_packet_size & 0xf800 != 0 {
                    continue;
                }
                candidates.push(usb_hid::UsbHidInterfaceCandidate {
                    configuration: config.b_configuration_value,
                    interface: interface.b_interface_number,
                    alternate_setting: interface.b_alternate_setting,
                    endpoint: endpoint.b_endpoint_address,
                    endpoint_out: endpoint_out.map(|(address, _)| address),
                    endpoint_out_size: endpoint_out.map(|(_, size)| size).unwrap_or(0),
                    endpoint_size: endpoint.w_max_packet_size,
                });
            }
        }
    }

    candidates
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    fn test_switch_calibration() -> bt::switch::SwitchCalibration {
        let axis = bt::switch::SwitchAxisCalibration {
            min: 1000,
            center: 2048,
            max: 3000,
        };
        let stick = bt::switch::SwitchStickCalibration {
            x: axis,
            y: axis,
            source: bt::switch::SwitchCalibrationSource::User,
        };
        bt::switch::SwitchCalibration {
            left: Some(stick),
            right: Some(stick),
        }
    }

    fn drain_queue(manager: &mut ControllerManager, queue: &mut bt::BluetoothInputQueue) -> usize {
        manager.drain_bluetooth_input_events(|| queue.pop())
    }

    #[test]
    fn wpad_classic_and_pro_map_extension_buttons_without_nunchuk_aliases() {
        let mut data = wpad::WpadData::default();
        data.exp.type_ = wpad::WPAD_EXP_CLASSIC as i32;
        data.btns_h = 0xfeff_0000;
        let (kind, buttons, _) = map_wpad_state(&data);
        assert_eq!(kind, ControllerKind::WiiClassic);
        assert_eq!(
            buttons,
            Buttons::A
                | Buttons::B
                | Buttons::X
                | Buttons::Y
                | Buttons::UP
                | Buttons::DOWN
                | Buttons::LEFT
                | Buttons::RIGHT
                | Buttons::START
                | Buttons::SELECT
                | Buttons::HOME
                | Buttons::L
                | Buttons::R
                | Buttons::ZL
                | Buttons::ZR
        );
        assert!(!buttons.contains(Buttons::Z));
        data.exp.u.classic.type_ = 2;
        data.btns_h = 0x0010_0003;
        let (kind, buttons, _) = map_wpad_state(&data);
        assert_eq!(kind, ControllerKind::WiiUPro);
        assert_eq!(buttons, Buttons::A | Buttons::L_STICK | Buttons::R_STICK);
    }

    #[test]
    fn wpad_classic_full_triggers_and_guitar_buttons_map_to_hbc_gated_actions() {
        let mut data = wpad::WpadData::default();
        data.exp.type_ = wpad::WPAD_EXP_CLASSIC as i32;
        data.btns_h = wpad::WPAD_CLASSIC_BUTTON_FULL_L | wpad::WPAD_CLASSIC_BUTTON_FULL_R;
        let (_, classic_buttons, _) = map_wpad_state(&data);
        assert_eq!(
            classic_buttons,
            Buttons::SELECT | Buttons::START | Buttons::L | Buttons::R
        );

        data.exp.type_ = wpad::WPAD_EXP_GUITARHERO3 as i32;
        data.btns_h = wpad::WPAD_GUITAR_HERO_3_BUTTON_STRUM_UP
            | wpad::WPAD_GUITAR_HERO_3_BUTTON_STRUM_DOWN
            | wpad::WPAD_GUITAR_HERO_3_BUTTON_GREEN
            | wpad::WPAD_GUITAR_HERO_3_BUTTON_RED
            | wpad::WPAD_GUITAR_HERO_3_BUTTON_PLUS
            | wpad::WPAD_GUITAR_HERO_3_BUTTON_MINUS
            | wpad::WPAD_GUITAR_HERO_3_BUTTON_YELLOW
            | wpad::WPAD_GUITAR_HERO_3_BUTTON_BLUE
            | wpad::WPAD_GUITAR_HERO_3_BUTTON_ORANGE;
        let (_, guitar_buttons, _) = map_wpad_state(&data);
        assert_eq!(
            guitar_buttons,
            Buttons::UP
                | Buttons::DOWN
                | Buttons::A
                | Buttons::B
                | Buttons::START
                | Buttons::SELECT
                | Buttons::LEFT
                | Buttons::RIGHT
                | Buttons::X
        );
    }

    #[test]
    fn wpad_nunchuk_maps_buttons_and_calibrated_stick() {
        let mut data = wpad::WpadData::default();
        data.exp.type_ = wpad::WPAD_EXP_NUNCHUK as i32;
        data.btns_h = 0x0003_0008;
        let mut nunchuk = wpad::Nunchuk::default();
        nunchuk.js.min.x = 27;
        nunchuk.js.center.x = 126;
        nunchuk.js.max.x = 223;
        nunchuk.js.min.y = 30;
        nunchuk.js.center.y = 131;
        nunchuk.js.max.y = 222;
        nunchuk.js.pos.x = 27;
        nunchuk.js.pos.y = 222;
        data.exp.u.nunchuk = nunchuk;
        let (_, buttons, axes) = map_wpad_state(&data);
        assert_eq!(buttons, Buttons::A | Buttons::Z | Buttons::L);
        assert_eq!([axes.left_x, axes.left_y], [-32767, 32767]);
        assert_eq!(normalize_calibrated_axis(126, 27, 126, 223), 0);
        assert_eq!(normalize_calibrated_axis(255, 27, 126, 223), 32767);
        assert_eq!(normalize_calibrated_axis(0, 126, 126, 126), 0);
    }

    #[test]
    fn wpad_guitar_hero_maps_calibrated_stick_axes() {
        let mut data = wpad::WpadData::default();
        data.exp.type_ = wpad::WPAD_EXP_GUITARHERO3 as i32;
        let mut guitar = wpad::GuitarHero3::default();
        guitar.js.min.x = 27;
        guitar.js.center.x = 126;
        guitar.js.max.x = 223;
        guitar.js.min.y = 30;
        guitar.js.center.y = 131;
        guitar.js.max.y = 222;
        guitar.js.pos.x = 223;
        guitar.js.pos.y = 30;
        data.exp.u.gh3 = guitar;

        let (kind, _, axes) = map_wpad_state(&data);

        assert_eq!(kind, ControllerKind::WiiRemote);
        assert_eq!([axes.left_x, axes.left_y], [32767, -32767]);
    }

    #[test]
    fn wpad_metadata_distinguishes_classic_and_pro_trigger_capabilities() {
        let mut data = wpad::WpadData {
            data_present: wpad::WPAD_DATA_EXPANSION,
            ..wpad::WpadData::default()
        };
        data.exp.type_ = wpad::WPAD_EXP_CLASSIC as i32;

        let classic = wpad_metadata(data).capabilities;
        assert!(classic.left_stick);
        assert!(classic.right_stick);
        assert!(classic.triggers);

        let mut extension = unsafe { data.exp.u.classic };
        extension.type_ = 2;
        data.exp.u.classic = extension;
        let pro = wpad_metadata(data).capabilities;
        assert!(pro.left_stick);
        assert!(pro.right_stick);
        assert!(!pro.triggers);
    }

    use super::*;

    #[test]
    fn builds_switch_init_led_and_keepalive_packets() {
        let init = build_switch_init_packets(0x0e);
        assert_eq!(
            init[0].bytes(),
            &[0x01, 0x0e, 0, 1, 0x40, 0x40, 0, 1, 0x40, 0x40, 0x08, 0x00]
        );
        assert_eq!(
            init[1].bytes(),
            &[0x01, 0x0f, 0, 1, 0x40, 0x40, 0, 1, 0x40, 0x40, 0x03, 0x30]
        );

        let led = build_switch_player_led_packet(0x10, 0x05);
        assert_eq!(
            led.bytes(),
            &[0x01, 0x00, 0, 1, 0x40, 0x40, 0, 1, 0x40, 0x40, 0x30, 0x05]
        );

        let keepalive = build_switch_rumble_packet(0x21, SWITCH_NEUTRAL_RUMBLE);
        assert_eq!(
            keepalive.bytes(),
            &[0x10, 0x01, 0, 1, 0x40, 0x40, 0, 1, 0x40, 0x40]
        );
    }

    #[test]
    fn wraps_switch_output_packets_for_bluetooth_hidp() {
        let init = build_switch_init_packets(0);
        let mut hidp = [0u8; 65];
        let used = write_switch_hidp_output_payload(&init[0], &mut hidp).unwrap();

        assert_eq!(used, 13);
        assert_eq!(hidp[0], SWITCH_HIDP_OUTPUT_HEADER);
        assert_eq!(&hidp[1..used], init[0].bytes());
        assert_eq!(
            write_switch_hidp_output_payload(&init[0], &mut hidp[..4]),
            None
        );
    }

    #[test]
    fn unwraps_bluetooth_hidp_input_payloads() {
        let payload = [
            SWITCH_HIDP_INPUT_HEADER,
            0x30,
            0,
            0,
            0x08,
            0,
            0,
            0x00,
            0x08,
            0x80,
            0x00,
            0x08,
            0x80,
        ];
        let report = switch_report_from_hidp_input(&payload).unwrap();
        assert_eq!(report[0], SWITCH_HID_INPUT_STANDARD);

        let mut manager = ControllerManager::new();
        manager.set_bluetooth_connected(
            0,
            bt::BluetoothAddress { bytes: [1; 6] },
            ControllerKind::JoyConRight,
        );
        manager
            .set_bluetooth_calibration_checked(0, test_switch_calibration())
            .unwrap();
        assert!(manager.set_bluetooth_report(0, ControllerKind::JoyConRight, &payload));
        assert_eq!(
            manager.bluetooth_slots()[0].last_report_id,
            SWITCH_HID_INPUT_STANDARD
        );
        assert!(
            manager.bluetooth_slots()[0]
                .state
                .buttons
                .contains(Buttons::A)
        );
    }

    #[test]
    fn snapshot_merges_buttons_and_preserves_capacity() {
        let mut snapshot = ControllerSnapshot::empty();
        for idx in 0..(MAX_CONTROLLERS + 2) {
            let mut state = ControllerState::empty();
            state.connected = true;
            state.buttons = if idx % 2 == 0 { Buttons::A } else { Buttons::B };
            snapshot.push(state);
        }

        assert_eq!(snapshot.len(), MAX_CONTROLLERS);
        assert!(snapshot.merged_buttons().contains(Buttons::A));
        assert!(snapshot.merged_buttons().contains(Buttons::B));
    }

    #[test]
    fn manager_tracks_bluetooth_button_transitions() {
        let mut manager = ControllerManager::new();
        let mut seen = [false; MAX_CONTROLLERS];
        let mut first = ControllerSnapshot::empty();
        let mut second = ControllerSnapshot::empty();
        let mut state = ControllerState {
            source: ControllerSource::BluetoothHid(0),
            kind: ControllerKind::JoyConRight,
            connected: true,
            buttons: Buttons::A,
            buttons_down: Buttons::empty(),
            buttons_up: Buttons::empty(),
            axes: Axes::default(),
        };

        manager.push_with_transitions(&mut first, &mut seen, BT_SLOT_BASE, state);
        assert!(first.controllers()[0].buttons.contains(Buttons::A));
        assert!(!first.controllers()[0].buttons_down.contains(Buttons::A));
        assert!(!first.controllers()[0].buttons_up.contains(Buttons::A));

        state.buttons = Buttons::B;
        manager.push_with_transitions(&mut second, &mut seen, BT_SLOT_BASE, state);
        assert!(second.controllers()[0].buttons.contains(Buttons::B));
        assert!(second.controllers()[0].buttons_down.contains(Buttons::B));
        assert!(second.controllers()[0].buttons_up.contains(Buttons::A));
    }

    #[test]
    fn manager_tracks_switch_ack_reports_for_diagnostics() {
        let mut manager = ControllerManager::new();
        let mut ack = [0u8; 15];
        ack[0] = SWITCH_HID_INPUT_SUBCOMMAND_ACK;
        ack[13] = 0x80;
        ack[14] = SWITCH_SUBCMD_LOW_POWER_MODE;

        assert!(manager.set_bluetooth_report(0, ControllerKind::JoyConLeft, &ack));
        let slot = manager.bluetooth_slots()[0];
        assert_eq!(slot.report_count, 1);
        assert_eq!(slot.last_report_id, SWITCH_HID_INPUT_SUBCOMMAND_ACK);
        assert!(slot.last_ack_ok);
        assert_eq!(slot.last_ack_subcommand, SWITCH_SUBCMD_LOW_POWER_MODE);
        assert!(!slot.state.connected);
    }

    #[test]
    fn paired_joycon_quick_tap_accumulates_edges_between_scans() {
        let neutral = [0x30, 0, 0, 0, 0, 0, 0x08, 0x80, 0, 0x08, 0x80, 0];
        let mut pressed = neutral;
        pressed[3] = 0x08; // Joy-Con R A button.
        let mut manager = ControllerManager::new();
        manager.set_bluetooth_connected(
            0,
            bt::BluetoothAddress { bytes: [1; 6] },
            ControllerKind::JoyConLeft,
        );
        manager.set_bluetooth_connected(
            1,
            bt::BluetoothAddress { bytes: [2; 6] },
            ControllerKind::JoyConRight,
        );
        manager
            .set_bluetooth_calibration_checked(0, test_switch_calibration())
            .unwrap();
        manager
            .set_bluetooth_calibration_checked(1, test_switch_calibration())
            .unwrap();
        manager
            .set_bluetooth_report_checked(0, ControllerKind::JoyConLeft, &neutral)
            .unwrap();
        manager
            .set_bluetooth_report_checked(1, ControllerKind::JoyConRight, &neutral)
            .unwrap();
        assert!(manager.pair_joycons());
        assert!(manager.pair_joycons());
        assert_eq!(
            manager
                .transitions
                .take_reset(BT_SLOT_BASE)
                .unwrap()
                .id
                .source,
            ControllerSource::BluetoothHid(0)
        );
        assert_eq!(
            manager
                .transitions
                .take_reset(BT_SLOT_BASE + 1)
                .unwrap()
                .id
                .source,
            ControllerSource::BluetoothHid(1)
        );

        manager
            .set_bluetooth_report_checked(1, ControllerKind::JoyConRight, &neutral)
            .unwrap();
        let _ = manager.transitions.take(BT_SLOT_BASE);
        manager
            .set_bluetooth_report_checked(1, ControllerKind::JoyConRight, &pressed)
            .unwrap();
        manager
            .set_bluetooth_report_checked(1, ControllerKind::JoyConRight, &neutral)
            .unwrap();

        let tap = manager.transitions.take(BT_SLOT_BASE).unwrap();
        assert_eq!(tap.source, ControllerSource::BluetoothHidPair(0, 1));
        assert!(tap.buttons_down.contains(Buttons::A));
        assert!(tap.buttons_up.contains(Buttons::A));
        assert_eq!(manager.transitions.take(BT_SLOT_BASE + 1), None);
    }

    #[test]
    fn malformed_paired_half_releases_pair_and_splits_sources() {
        let neutral = [0x30, 0, 0, 0, 0, 0, 0x08, 0x80, 0, 0x08, 0x80, 0];
        let mut manager = ControllerManager::new();
        manager.set_bluetooth_connected(
            0,
            bt::BluetoothAddress { bytes: [1; 6] },
            ControllerKind::JoyConLeft,
        );
        manager.set_bluetooth_connected(
            1,
            bt::BluetoothAddress { bytes: [2; 6] },
            ControllerKind::JoyConRight,
        );
        manager
            .set_bluetooth_calibration_checked(0, test_switch_calibration())
            .unwrap();
        manager
            .set_bluetooth_calibration_checked(1, test_switch_calibration())
            .unwrap();
        manager
            .set_bluetooth_report_checked(0, ControllerKind::JoyConLeft, &neutral)
            .unwrap();
        manager
            .set_bluetooth_report_checked(1, ControllerKind::JoyConRight, &neutral)
            .unwrap();
        assert!(manager.pair_joycons());
        let _ = manager.transitions.take_reset(BT_SLOT_BASE);
        let _ = manager.transitions.take_reset(BT_SLOT_BASE + 1);
        manager
            .set_bluetooth_report_checked(1, ControllerKind::JoyConRight, &neutral)
            .unwrap();
        let mut pressed = neutral;
        pressed[3] = 0x08;
        manager
            .set_bluetooth_report_checked(1, ControllerKind::JoyConRight, &pressed)
            .unwrap();

        assert_eq!(
            manager.set_bluetooth_report_checked(1, ControllerKind::JoyConRight, &[0x30]),
            Err(InputFailure::MalformedReport)
        );
        assert_eq!(manager.joycon_pair(), None);
        let reset = manager.transitions.take_reset(BT_SLOT_BASE).unwrap();
        assert_eq!(reset.id.source, ControllerSource::BluetoothHidPair(0, 1));
        assert!(reset.state.buttons_up.contains(Buttons::A));
        assert_eq!(manager.transitions.take(BT_SLOT_BASE), None);
    }

    #[test]
    fn manager_records_bluetooth_connection_handle() {
        let mut manager = ControllerManager::new();
        manager.set_bluetooth_connected_with_handle(
            0,
            bt::BluetoothAddress { bytes: [1; 6] },
            ControllerKind::JoyConLeft,
            0x002a,
        );

        let slot = manager.bluetooth_slots()[0];
        assert!(slot.connected);
        assert_eq!(slot.connection_handle, 0x002a);
        assert_eq!(slot.kind, ControllerKind::JoyConLeft);
    }

    #[test]
    fn switch_report_without_calibration_fails_and_resets_controls() {
        let mut manager = ControllerManager::new();
        manager.set_bluetooth_connected(
            0,
            bt::BluetoothAddress { bytes: [1; 6] },
            ControllerKind::SwitchPro,
        );
        let pressed = [0x30, 0, 0, 0x08, 0, 0, 0, 0x08, 0x80, 0, 0x08, 0x80];

        assert_eq!(
            manager.set_bluetooth_report_checked(0, ControllerKind::SwitchPro, &pressed),
            Err(InputFailure::UnsupportedReport)
        );
        assert!(!manager.bluetooth_slots()[0].state.connected);
        assert!(manager.bluetooth_slots()[0].state.buttons.is_empty());
        assert_eq!(
            manager.source_errors[BT_SLOT_BASE].unwrap().failure,
            InputFailure::UnsupportedReport
        );

        let mut manager = ControllerManager::new();
        manager.set_bluetooth_connected(
            0,
            bt::BluetoothAddress { bytes: [2; 6] },
            ControllerKind::SwitchPro,
        );
        manager
            .set_bluetooth_calibration_checked(0, test_switch_calibration())
            .unwrap();
        let pressed = [0x30, 0, 0, 0x08, 0, 0, 0, 0x08, 0x80, 0, 0x08, 0x80];
        manager
            .set_bluetooth_report_checked(0, ControllerKind::SwitchPro, &pressed)
            .unwrap();
        let _ = manager.transitions.take(BT_SLOT_BASE);
        let mut invalid = test_switch_calibration();
        invalid.left.as_mut().unwrap().x.center = 5000;
        assert_eq!(
            manager.set_bluetooth_calibration_checked(0, invalid),
            Err(InputFailure::MalformedReport)
        );
        assert!(!manager.bluetooth_slots()[0].state.connected);
        assert_eq!(
            manager.source_errors[BT_SLOT_BASE].unwrap().failure,
            InputFailure::MalformedReport
        );
        assert_eq!(
            manager
                .transitions
                .take_reset(BT_SLOT_BASE)
                .unwrap()
                .state
                .buttons_up,
            Buttons::A
        );
    }

    #[test]
    fn calibrated_single_joycon_reports_filter_the_other_half_controls() {
        let left_report = [0x30, 0, 0, 0xff, 0x3f, 0, 0, 0x08, 0x80, 0, 0x08, 0x80];
        let mut manager = ControllerManager::new();
        manager.set_bluetooth_connected(
            0,
            bt::BluetoothAddress { bytes: [1; 6] },
            ControllerKind::JoyConLeft,
        );
        manager
            .set_bluetooth_calibration_checked(0, test_switch_calibration())
            .unwrap();
        manager
            .set_bluetooth_report_checked(0, ControllerKind::JoyConLeft, &left_report)
            .unwrap();
        let buttons = manager.bluetooth_slots()[0].state.buttons;
        assert!(buttons.contains(Buttons::SELECT));
        assert!(buttons.contains(Buttons::L_STICK));
        assert!(buttons.contains(Buttons::CAPTURE));
        assert!(!buttons.contains(Buttons::A));
        assert!(!buttons.contains(Buttons::START));
        assert!(!buttons.contains(Buttons::R_STICK));
        assert!(!buttons.contains(Buttons::HOME));

        let right_report = [0x30, 0, 0, 0x08, 0x3f, 0xff, 0, 0x08, 0x80, 0, 0x08, 0x80];
        manager.clear_bluetooth_slot(0);
        manager.set_bluetooth_connected(
            0,
            bt::BluetoothAddress { bytes: [2; 6] },
            ControllerKind::JoyConRight,
        );
        manager
            .set_bluetooth_calibration_checked(0, test_switch_calibration())
            .unwrap();
        manager
            .set_bluetooth_report_checked(0, ControllerKind::JoyConRight, &right_report)
            .unwrap();
        let buttons = manager.bluetooth_slots()[0].state.buttons;
        assert!(buttons.contains(Buttons::A));
        assert!(buttons.contains(Buttons::START));
        assert!(buttons.contains(Buttons::R_STICK));
        assert!(buttons.contains(Buttons::HOME));
        assert!(!buttons.contains(Buttons::SELECT));
        assert!(!buttons.contains(Buttons::L_STICK));
        assert!(!buttons.contains(Buttons::CAPTURE));
        assert!(!buttons.contains(Buttons::UP));
    }

    #[test]
    fn queued_pair_tap_keeps_down_and_up_edges_between_manager_frames() {
        let mut queue = bt::BluetoothInputQueue::new();
        let calibration = test_switch_calibration();
        let left = queue
            .push_connected(
                0,
                bt::BluetoothAddress { bytes: [1; 6] },
                1,
                bt::ControllerFamily::JoyConLeft,
                Some(calibration),
            )
            .unwrap();
        let right = queue
            .push_connected(
                1,
                bt::BluetoothAddress { bytes: [2; 6] },
                2,
                bt::ControllerFamily::JoyConRight,
                Some(calibration),
            )
            .unwrap();
        let neutral = [0x30, 0, 0, 0, 0, 0, 0, 0x08, 0x80, 0, 0x08, 0x80];
        queue.push_report(0, left, &neutral).unwrap();
        queue.push_report(1, right, &neutral).unwrap();

        let mut manager = ControllerManager::new();
        assert_eq!(drain_queue(&mut manager, &mut queue), 4);
        assert!(manager.pair_joycons());
        let _ = manager.transitions.take_reset(BT_SLOT_BASE);
        let _ = manager.transitions.take_reset(BT_SLOT_BASE + 1);
        let _ = manager.transitions.take(BT_SLOT_BASE);

        let mut down = neutral;
        down[3] = 0x08;
        queue.push_report(1, right, &down).unwrap();
        queue.push_report(1, right, &neutral).unwrap();
        assert_eq!(drain_queue(&mut manager, &mut queue), 2);
        let tap = manager.transitions.take(BT_SLOT_BASE).unwrap();
        assert_eq!(tap.source, ControllerSource::BluetoothHidPair(0, 1));
        assert!(tap.buttons_down.contains(Buttons::A));
        assert!(tap.buttons_up.contains(Buttons::A));
    }

    #[test]
    fn queued_disconnect_reconnect_preserves_identity_and_held_gate() {
        let mut queue = bt::BluetoothInputQueue::new();
        let calibration = test_switch_calibration();
        let first = queue
            .push_connected(
                0,
                bt::BluetoothAddress { bytes: [1; 6] },
                1,
                bt::ControllerFamily::SwitchPro,
                Some(calibration),
            )
            .unwrap();
        let mut held = [0x30, 0, 0, 0x08, 0, 0, 0, 0x08, 0x80, 0, 0x08, 0x80];
        queue.push_report(0, first, &held).unwrap();

        let mut manager = ControllerManager::new();
        assert_eq!(drain_queue(&mut manager, &mut queue), 2);
        let original_id = manager.transitions.id(BT_SLOT_BASE).unwrap();
        assert_eq!(
            manager.transitions.take(BT_SLOT_BASE).unwrap().buttons_down,
            Buttons::empty()
        );

        queue.push_disconnected(0, first, 0x13).unwrap();
        let second = queue
            .push_connected(
                0,
                bt::BluetoothAddress { bytes: [2; 6] },
                2,
                bt::ControllerFamily::SwitchPro,
                Some(calibration),
            )
            .unwrap();
        held[1] = 9;
        queue.push_report(0, second, &held).unwrap();
        assert_eq!(drain_queue(&mut manager, &mut queue), 3);

        let reset = manager.transitions.take_reset(BT_SLOT_BASE).unwrap();
        assert_eq!(reset.id, original_id);
        assert_eq!(reset.state.buttons_up, Buttons::A);
        assert_eq!(manager.bluetooth_slots()[0].address.bytes, [2; 6]);
        assert_eq!(manager.bluetooth_slots()[0].generation, second);
        let reconnected = manager.transitions.id(BT_SLOT_BASE).unwrap();
        assert_eq!(reconnected.generation, original_id.generation + 1);
        assert_eq!(
            manager.transitions.take(BT_SLOT_BASE).unwrap().buttons_down,
            Buttons::empty()
        );

        let neutral = [0x30, 0, 0, 0, 0, 0, 0, 0x08, 0x80, 0, 0x08, 0x80];
        queue.push_report(0, second, &neutral).unwrap();
        queue.push_report(0, second, &held).unwrap();
        assert_eq!(drain_queue(&mut manager, &mut queue), 2);
        assert!(
            manager
                .transitions
                .take(BT_SLOT_BASE)
                .unwrap()
                .buttons_down
                .contains(Buttons::A)
        );
    }

    #[test]
    fn bounded_queue_drain_releases_backpressure_for_retry() {
        let mut queue = bt::BluetoothInputQueue::new();
        let generation = queue
            .push_connected(
                0,
                bt::BluetoothAddress { bytes: [1; 6] },
                1,
                bt::ControllerFamily::Generic,
                None,
            )
            .unwrap();
        for _ in 1..bt::BLUETOOTH_INPUT_QUEUE_CAPACITY {
            queue.push_report(0, generation, &[1, 0, 0]).unwrap();
        }
        assert!(queue.is_full());
        assert_eq!(
            queue.push_report(0, generation, &[0, 0, 0]),
            Err(bt::BluetoothInputQueueError::Full)
        );

        let mut manager = ControllerManager::new();
        assert_eq!(
            drain_queue(&mut manager, &mut queue),
            bt::BLUETOOTH_INPUT_QUEUE_CAPACITY
        );
        assert!(queue.is_empty());
        queue.push_report(0, generation, &[0, 0, 0]).unwrap();
        queue.push_report(0, generation, &[1, 0, 0]).unwrap();
        assert_eq!(drain_queue(&mut manager, &mut queue), 2);
        assert!(
            manager
                .transitions
                .take(BT_SLOT_BASE)
                .unwrap()
                .buttons_down
                .contains(Buttons::A)
        );
    }

    #[test]
    fn manager_usb_hid_is_explicit_opt_in() {
        let mut manager = ControllerManager::new();

        assert!(!manager.usb.enabled);
        assert!(!manager.usb.initialized);

        manager.enable_usb_hid();
        assert!(manager.usb.enabled);
        assert!(!manager.usb.initialized);
    }

    #[test]
    fn gamecube_port_one_takes_player_one_before_hid() {
        let mut manager = ControllerManager::new();
        let mut snapshot = ControllerSnapshot::empty();
        let mut seen = [false; MAX_CONTROLLERS];

        manager.assign_with_transitions(
            &mut snapshot,
            &mut seen,
            GC_SLOT_BASE,
            controller_state(ControllerSource::GameCube(0), Buttons::A),
        );
        manager.assign_with_transitions(
            &mut snapshot,
            &mut seen,
            USB_SLOT_BASE,
            controller_state(ControllerSource::UsbHid(0), Buttons::B),
        );

        assert_eq!(
            snapshot.player(0).unwrap().source,
            ControllerSource::GameCube(0)
        );
        assert_eq!(
            snapshot.player(1).unwrap().source,
            ControllerSource::UsbHid(0)
        );
        assert!(snapshot.player_buttons(0).contains(Buttons::A));
        assert!(snapshot.player_buttons(1).contains(Buttons::B));
    }

    #[test]
    fn gamecube_port_two_stays_player_two_with_hid_as_player_one() {
        let mut manager = ControllerManager::new();
        let mut snapshot = ControllerSnapshot::empty();
        let mut seen = [false; MAX_CONTROLLERS];

        manager.assign_with_transitions(
            &mut snapshot,
            &mut seen,
            GC_SLOT_BASE + 1,
            controller_state(ControllerSource::GameCube(1), Buttons::A),
        );
        manager.assign_with_transitions(
            &mut snapshot,
            &mut seen,
            USB_SLOT_BASE,
            controller_state(ControllerSource::UsbHid(0), Buttons::B),
        );

        assert_eq!(
            snapshot.player(0).unwrap().source,
            ControllerSource::UsbHid(0)
        );
        assert_eq!(
            snapshot.player(1).unwrap().source,
            ControllerSource::GameCube(1)
        );
        assert!(snapshot.player_buttons(0).contains(Buttons::B));
        assert!(snapshot.player_buttons(1).contains(Buttons::A));
    }

    #[test]
    fn gamecube_port_two_does_not_steal_existing_hid_player_two() {
        let mut manager = ControllerManager::new();
        let mut snapshot = ControllerSnapshot::empty();
        let mut seen = [false; MAX_CONTROLLERS];

        manager.assign_with_transitions(
            &mut snapshot,
            &mut seen,
            GC_SLOT_BASE,
            controller_state(ControllerSource::GameCube(0), Buttons::A),
        );
        manager.assign_with_transitions(
            &mut snapshot,
            &mut seen,
            USB_SLOT_BASE,
            controller_state(ControllerSource::UsbHid(0), Buttons::B),
        );

        let mut next = ControllerSnapshot::empty();
        let mut next_seen = [false; MAX_CONTROLLERS];
        manager.assign_with_transitions(
            &mut next,
            &mut next_seen,
            GC_SLOT_BASE,
            controller_state(ControllerSource::GameCube(0), Buttons::A),
        );
        manager.assign_with_transitions(
            &mut next,
            &mut next_seen,
            GC_SLOT_BASE + 1,
            controller_state(ControllerSource::GameCube(1), Buttons::X),
        );
        manager.assign_with_transitions(
            &mut next,
            &mut next_seen,
            USB_SLOT_BASE,
            controller_state(ControllerSource::UsbHid(0), Buttons::B),
        );

        assert_eq!(
            next.player(0).unwrap().source,
            ControllerSource::GameCube(0)
        );
        assert_eq!(next.player(1).unwrap().source, ControllerSource::UsbHid(0));
        assert_eq!(next.player(2), None);
        assert!(next.player_buttons(1).contains(Buttons::B));
    }

    #[test]
    fn wii_remote_two_does_not_steal_existing_bluetooth_hid_player_two() {
        let mut manager = ControllerManager::new();
        let mut snapshot = ControllerSnapshot::empty();
        let mut seen = [false; MAX_CONTROLLERS];

        manager.assign_with_transitions(
            &mut snapshot,
            &mut seen,
            GC_SLOT_BASE,
            controller_state(ControllerSource::GameCube(0), Buttons::A),
        );
        manager.assign_with_transitions(
            &mut snapshot,
            &mut seen,
            BT_SLOT_BASE,
            controller_state(ControllerSource::BluetoothHid(0), Buttons::B),
        );

        let mut next = ControllerSnapshot::empty();
        let mut next_seen = [false; MAX_CONTROLLERS];
        manager.assign_with_transitions(
            &mut next,
            &mut next_seen,
            GC_SLOT_BASE,
            controller_state(ControllerSource::GameCube(0), Buttons::A),
        );
        manager.assign_with_transitions(
            &mut next,
            &mut next_seen,
            WPAD_SLOT_BASE + 1,
            controller_state(ControllerSource::WiiBluetooth(1), Buttons::X),
        );
        manager.assign_with_transitions(
            &mut next,
            &mut next_seen,
            BT_SLOT_BASE,
            controller_state(ControllerSource::BluetoothHid(0), Buttons::B),
        );

        assert_eq!(
            next.player(0).unwrap().source,
            ControllerSource::GameCube(0)
        );
        assert_eq!(
            next.player(1).unwrap().source,
            ControllerSource::BluetoothHid(0)
        );
        assert_eq!(next.player(2), None);
        assert!(next.player_buttons(1).contains(Buttons::B));
    }

    #[test]
    fn joycon_pair_combines_left_and_right_into_one_player() {
        let mut manager = ControllerManager::new();
        manager.bluetooth[0].connected = true;
        manager.bluetooth[0].kind = ControllerKind::JoyConLeft;
        manager.bluetooth[0].state.connected = true;
        manager.bluetooth[0].state.kind = ControllerKind::JoyConLeft;
        manager.bluetooth[0].state.buttons = Buttons::LEFT | Buttons::L;
        manager.bluetooth[0].state.axes.left_x = -123;
        manager.bluetooth[0].state.axes.left_y = 456;
        manager.bluetooth[0].state.axes.left_trigger = 255;

        manager.bluetooth[1].connected = true;
        manager.bluetooth[1].kind = ControllerKind::JoyConRight;
        manager.bluetooth[1].state.connected = true;
        manager.bluetooth[1].state.kind = ControllerKind::JoyConRight;
        manager.bluetooth[1].state.buttons = Buttons::A | Buttons::R;
        manager.bluetooth[1].state.axes.right_x = 789;
        manager.bluetooth[1].state.axes.right_y = -321;
        manager.bluetooth[1].state.axes.right_trigger = 255;

        assert!(manager.pair_joycons());

        let mut snapshot = ControllerSnapshot::empty();
        let mut seen = [false; MAX_CONTROLLERS];
        let state = combine_joycon_pair(0, 1, manager.bluetooth[0], manager.bluetooth[1]);
        manager.assign_with_transitions(&mut snapshot, &mut seen, BT_SLOT_BASE, state);

        let paired = snapshot.player(0).unwrap();
        assert_eq!(paired.source, ControllerSource::BluetoothHidPair(0, 1));
        assert_eq!(paired.kind, ControllerKind::JoyConPair);
        assert!(paired.buttons.contains(Buttons::LEFT));
        assert!(paired.buttons.contains(Buttons::A));
        assert_eq!(paired.axes.left_x, -123);
        assert_eq!(paired.axes.right_x, 789);
        assert_eq!(paired.axes.left_trigger, 255);
        assert_eq!(paired.axes.right_trigger, 255);
    }

    #[test]
    fn finds_controller_hid_endpoint_and_skips_boot_mouse() {
        let mut controller_endpoint = usb::UsbEndpointDesc {
            b_endpoint_address: usb::USB_ENDPOINT_IN | 2,
            bm_attributes: usb::USB_ENDPOINT_INTERRUPT,
            w_max_packet_size: 32,
            ..usb::UsbEndpointDesc::default()
        };
        let mut mouse_endpoint = usb::UsbEndpointDesc {
            b_endpoint_address: usb::USB_ENDPOINT_IN | 1,
            bm_attributes: usb::USB_ENDPOINT_INTERRUPT,
            w_max_packet_size: 8,
            ..usb::UsbEndpointDesc::default()
        };
        let mut interfaces = [
            usb::UsbInterfaceDesc {
                b_num_endpoints: 1,
                b_interface_class: usb::USB_CLASS_HID,
                b_interface_sub_class: usb::USB_SUBCLASS_BOOT,
                b_interface_protocol: usb::USB_PROTOCOL_MOUSE,
                endpoints: &mut mouse_endpoint,
                ..usb::UsbInterfaceDesc::default()
            },
            usb::UsbInterfaceDesc {
                b_interface_number: 3,
                b_num_endpoints: 1,
                b_interface_class: usb::USB_CLASS_HID,
                b_interface_sub_class: usb::USB_SUBCLASS_NONE,
                b_interface_protocol: usb::USB_PROTOCOL_NONE,
                endpoints: &mut controller_endpoint,
                ..usb::UsbInterfaceDesc::default()
            },
        ];
        let mut config = usb::UsbConfigDesc {
            b_num_interfaces: interfaces.len() as u8,
            b_configuration_value: 5,
            interfaces: interfaces.as_mut_ptr(),
            ..usb::UsbConfigDesc::default()
        };
        let desc = usb::UsbDevDesc {
            b_num_configurations: 1,
            configurations: &mut config,
            ..usb::UsbDevDesc::default()
        };

        assert_eq!(
            find_hid_interfaces(&desc),
            vec![usb_hid::UsbHidInterfaceCandidate {
                configuration: 5,
                interface: 3,
                alternate_setting: 0,
                endpoint: usb::USB_ENDPOINT_IN | 2,
                endpoint_out: None,
                endpoint_out_size: 0,
                endpoint_size: 32,
            }]
        );
    }

    fn controller_state(source: ControllerSource, buttons: Buttons) -> ControllerState {
        ControllerState {
            source,
            kind: ControllerKind::Unknown,
            connected: true,
            buttons,
            buttons_down: Buttons::empty(),
            buttons_up: Buttons::empty(),
            axes: Axes::default(),
        }
    }
}
