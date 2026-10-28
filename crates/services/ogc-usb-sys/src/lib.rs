#![no_std]

#[cfg(test)]
extern crate std;

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

#[cfg(target_arch = "powerpc")]
use core::ffi::CStr;
use core::ffi::c_void;
#[cfg(target_arch = "powerpc")]
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU8, AtomicU32, Ordering};

#[cfg(target_arch = "powerpc")]
use ogc_cache::{dc_flush_range, dc_invalidate_range};
#[cfg(target_arch = "powerpc")]
use ogc_ipc::{
    FileDescriptor, HeapId, IPC_EINVAL, IPC_ENOENT, IPC_ENOMEM, IPC_EQUEUEFULL, IPC_OPEN_NONE,
    IoctlBuffers, Ioctlv, IoctlvEntry, open_file,
};
#[cfg(target_arch = "powerpc")]
use ogc_lwp_sys as lwp;
pub use ogc_sys::Bool;
#[cfg(target_arch = "powerpc")]
use ogc_usb_events::{CallbackBatch, EventError, EventState, RegistrationError};

mod v5_layout;
#[cfg(target_arch = "powerpc")]
use v5_layout::HostKind;

#[cfg(target_arch = "powerpc")]
const V5_DEVICE_WAIT_TIMEOUT_TICKS: u64 = lwp::TICKS_PER_SECOND / 10;
#[cfg(target_arch = "powerpc")]
const V5_DEVICE_WAIT_POLL_TICKS: u64 = lwp::TICKS_PER_SECOND / 1_000;

pub const USB_OK: i32 = 0;
pub const USB_FAILED: i32 = 1;
pub const USB_MAXPATH: usize = 64;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum UsbV5FailureStage {
    None = 0,
    DeviceChange = 1,
    AttachFinish = 2,
}

/// Return the latest IOS error from the V5 device watcher, if one occurred.
pub fn v5_failure_status() -> Option<(UsbV5FailureStage, i32)> {
    let stage = match USB_V5_FAILURE_STAGE.load(Ordering::Acquire) {
        1 => UsbV5FailureStage::DeviceChange,
        2 => UsbV5FailureStage::AttachFinish,
        _ => return None,
    };
    Some((stage, USB_V5_FAILURE_RESULT.load(Ordering::Relaxed)))
}

fn clear_v5_failure() {
    USB_V5_FAILURE_RESULT.store(0, Ordering::Relaxed);
    USB_V5_FAILURE_STAGE.store(UsbV5FailureStage::None as u8, Ordering::Release);
}

pub const USB_CLASS_HID: u8 = 0x03;
pub const USB_SUBCLASS_NONE: u8 = 0x00;
pub const USB_SUBCLASS_BOOT: u8 = 0x01;
pub const USB_PROTOCOL_NONE: u8 = 0x00;
pub const USB_PROTOCOL_KEYBOARD: u8 = 0x01;
pub const USB_PROTOCOL_MOUSE: u8 = 0x02;

pub const USB_REPTYPE_INPUT: u8 = 0x01;
pub const USB_REPTYPE_OUTPUT: u8 = 0x02;
pub const USB_REPTYPE_FEATURE: u8 = 0x03;

pub const USB_DT_DEVICE: u8 = 0x01;
pub const USB_DT_CONFIG: u8 = 0x02;
pub const USB_DT_STRING: u8 = 0x03;
pub const USB_DT_INTERFACE: u8 = 0x04;
pub const USB_DT_ENDPOINT: u8 = 0x05;
pub const USB_DT_DEVICE_QUALIFIER: u8 = 0x06;
pub const USB_DT_OTHER_SPEED_CONFIG: u8 = 0x07;
pub const USB_DT_INTERFACE_POWER: u8 = 0x08;
pub const USB_DT_OTG: u8 = 0x09;
pub const USB_DT_DEBUG: u8 = 0x10;
pub const USB_DT_INTERFACE_ASSOCIATION: u8 = 0x11;
pub const USB_DT_HID: u8 = 0x21;
pub const USB_DT_REPORT: u8 = 0x22;
pub const USB_DT_PHYSICAL: u8 = 0x23;
pub const USB_DT_CLASS_SPECIFIC_INTERFACE: u8 = 0x24;
pub const USB_DT_CLASS_SPECIFIC_ENDPOINT: u8 = 0x25;
pub const USB_DT_HUB: u8 = 0x29;

pub const USB_REQ_GETSTATUS: u8 = 0x00;
pub const USB_REQ_CLEARFEATURE: u8 = 0x01;
pub const USB_REQ_SETFEATURE: u8 = 0x03;
pub const USB_REQ_SETADDRESS: u8 = 0x05;
pub const USB_REQ_GETDESCRIPTOR: u8 = 0x06;
pub const USB_REQ_SETDESCRIPTOR: u8 = 0x07;
pub const USB_REQ_GETCONFIG: u8 = 0x08;
pub const USB_REQ_SETCONFIG: u8 = 0x09;
pub const USB_REQ_GETINTERFACE: u8 = 0x0a;
pub const USB_REQ_SETINTERFACE: u8 = 0x0b;
pub const USB_REQ_SYNCFRAME: u8 = 0x0c;

pub const USB_REQ_GETREPORT: u8 = 0x01;
pub const USB_REQ_GETIDLE: u8 = 0x02;
pub const USB_REQ_GETPROTOCOL: u8 = 0x03;
pub const USB_REQ_SETREPORT: u8 = 0x09;
pub const USB_REQ_SETIDLE: u8 = 0x0a;
pub const USB_REQ_SETPROTOCOL: u8 = 0x0b;

pub const USB_DT_DEVICE_SIZE: usize = 18;
pub const USB_DT_CONFIG_SIZE: usize = 9;
pub const USB_DT_INTERFACE_SIZE: usize = 9;
pub const USB_DT_ENDPOINT_SIZE: usize = 7;
pub const USB_DT_ENDPOINT_AUDIO_SIZE: usize = 9;
pub const USB_DT_HID_SIZE: usize = 9;
pub const USB_DT_MINREPORT_SIZE: usize = 8;
pub const USB_DT_HUB_NONVAR_SIZE: usize = 7;

pub const USB_CTRLTYPE_DIR_HOST2DEVICE: u8 = 0 << 7;
pub const USB_CTRLTYPE_DIR_DEVICE2HOST: u8 = 1 << 7;
pub const USB_CTRLTYPE_TYPE_STANDARD: u8 = 0 << 5;
pub const USB_CTRLTYPE_TYPE_CLASS: u8 = 1 << 5;
pub const USB_CTRLTYPE_TYPE_VENDOR: u8 = 2 << 5;
pub const USB_CTRLTYPE_TYPE_RESERVED: u8 = 3 << 5;
pub const USB_CTRLTYPE_REC_DEVICE: u8 = 0;
pub const USB_CTRLTYPE_REC_INTERFACE: u8 = 1;
pub const USB_CTRLTYPE_REC_ENDPOINT: u8 = 2;
pub const USB_CTRLTYPE_REC_OTHER: u8 = 3;

pub const USB_REQTYPE_INTERFACE_GET: u8 =
    USB_CTRLTYPE_DIR_DEVICE2HOST | USB_CTRLTYPE_TYPE_CLASS | USB_CTRLTYPE_REC_INTERFACE;
pub const USB_REQTYPE_INTERFACE_SET: u8 =
    USB_CTRLTYPE_DIR_HOST2DEVICE | USB_CTRLTYPE_TYPE_CLASS | USB_CTRLTYPE_REC_INTERFACE;
pub const USB_REQTYPE_ENDPOINT_GET: u8 =
    USB_CTRLTYPE_DIR_DEVICE2HOST | USB_CTRLTYPE_TYPE_CLASS | USB_CTRLTYPE_REC_ENDPOINT;
pub const USB_REQTYPE_ENDPOINT_SET: u8 =
    USB_CTRLTYPE_DIR_HOST2DEVICE | USB_CTRLTYPE_TYPE_CLASS | USB_CTRLTYPE_REC_ENDPOINT;

pub const USB_FEATURE_ENDPOINT_HALT: u16 = 0;

pub const USB_ENDPOINT_INTERRUPT: u8 = 0x03;
pub const USB_ENDPOINT_IN: u8 = 0x80;
pub const USB_ENDPOINT_OUT: u8 = 0x00;

pub const USB_OH0_DEVICE_ID: i32 = 0x0000_0000;
pub const USB_OH1_DEVICE_ID: i32 = 0x0020_0000;

#[cfg(target_arch = "powerpc")]
const USB_DESC_HEAP_SIZE: i32 = 0x2000;
#[cfg(target_arch = "powerpc")]
static USB_DESC_HEAP_ID: AtomicI32 = AtomicI32::new(-1);
#[cfg(target_arch = "powerpc")]
const USB_MAX_DEVICES: usize = 32;
#[cfg(target_arch = "powerpc")]
const USBV0_IOCTL_CTRLMSG: i32 = 0;
#[cfg(target_arch = "powerpc")]
const USBV0_IOCTL_BLKMSG: i32 = 1;
#[cfg(target_arch = "powerpc")]
const USBV0_IOCTL_INTRMSG: i32 = 2;
#[cfg(target_arch = "powerpc")]
const USBV0_IOCTL_SUSPENDDEV: i32 = 5;
#[cfg(target_arch = "powerpc")]
const USBV0_IOCTL_RESUMEDEV: i32 = 6;
#[cfg(target_arch = "powerpc")]
const USBV0_IOCTL_ISOMSG: i32 = 9;
#[cfg(target_arch = "powerpc")]
const USBV0_IOCTL_GETDEVLIST: i32 = 12;
#[cfg(target_arch = "powerpc")]
const USBV0_IOCTL_DEVREMOVALHOOK: i32 = 26;
#[cfg(target_arch = "powerpc")]
const USBV0_IOCTL_DEVICECLASSCHANGE: i32 = 28;
#[cfg(target_arch = "powerpc")]
const USBV5_IOCTL_GETVERSION: i32 = 0;
#[cfg(target_arch = "powerpc")]
const USBV5_IOCTL_GETDEVICECHANGE: i32 = 1;
#[cfg(target_arch = "powerpc")]
const USBV5_IOCTL_SHUTDOWN: i32 = 2;
#[cfg(target_arch = "powerpc")]
const USBV5_IOCTL_GETDEVPARAMS: i32 = 3;
#[cfg(target_arch = "powerpc")]
const USBV5_IOCTL_ATTACHFINISH: i32 = 6;
#[cfg(target_arch = "powerpc")]
const USBV5_IOCTL_SUSPEND_RESUME: i32 = 16;
#[cfg(target_arch = "powerpc")]
const USBV5_IOCTL_CTRLMSG: i32 = 18;
const USBV5_IOCTL_ISOMSG: i32 = 20;
#[cfg(target_arch = "powerpc")]
const USBV5_IOCTL_BULKMSG: i32 = 21;
#[cfg(target_arch = "powerpc")]
const USBV5_IOCTL_INTRMSG: i32 = 19;
#[cfg(target_arch = "powerpc")]
const USB_REQ_GETCONFIGURATION: u8 = 0x08;
#[cfg(target_arch = "powerpc")]
const USB_REQ_SETCONFIGURATION: u8 = 0x09;

pub type UsbCallback = Option<unsafe extern "C" fn(result: i32, usrdata: *mut c_void) -> i32>;

/// Callback state supplied with an asynchronous USB request.
#[derive(Copy, Clone)]
pub struct UsbAsyncCompletion {
    pub callback: UsbCallback,
    pub userdata: *mut c_void,
}

/// Parameters for a USB control transfer.
#[derive(Copy, Clone)]
pub struct UsbControlRequest {
    pub fd: i32,
    pub request_type: u8,
    pub request: u8,
    pub value: u16,
    pub index: u16,
    pub length: u16,
    pub data: *mut c_void,
}

#[repr(C, packed)]
#[derive(Copy, Clone, Default)]
pub struct UsbEndpointDesc {
    pub b_length: u8,
    pub b_descriptor_type: u8,
    pub b_endpoint_address: u8,
    pub bm_attributes: u8,
    pub w_max_packet_size: u16,
    pub b_interval: u8,
}

#[repr(C, packed)]
#[derive(Copy, Clone, Default)]
pub struct UsbInterfaceDesc {
    pub b_length: u8,
    pub b_descriptor_type: u8,
    pub b_interface_number: u8,
    pub b_alternate_setting: u8,
    pub b_num_endpoints: u8,
    pub b_interface_class: u8,
    pub b_interface_sub_class: u8,
    pub b_interface_protocol: u8,
    pub i_interface: u8,
    pub extra: *mut u8,
    pub extra_size: u16,
    pub endpoints: *mut UsbEndpointDesc,
}

#[repr(C, packed)]
#[derive(Copy, Clone, Default)]
pub struct UsbConfigDesc {
    pub b_length: u8,
    pub b_descriptor_type: u8,
    pub w_total_length: u16,
    pub b_num_interfaces: u8,
    pub b_configuration_value: u8,
    pub i_configuration: u8,
    pub bm_attributes: u8,
    pub b_max_power: u8,
    pub interfaces: *mut UsbInterfaceDesc,
}

#[repr(C, packed)]
#[derive(Copy, Clone, Default)]
pub struct UsbDevDesc {
    pub b_length: u8,
    pub b_descriptor_type: u8,
    pub bcd_usb: u16,
    pub b_device_class: u8,
    pub b_device_sub_class: u8,
    pub b_device_protocol: u8,
    pub b_max_packet_size0: u8,
    pub id_vendor: u16,
    pub id_product: u16,
    pub bcd_device: u16,
    pub i_manufacturer: u8,
    pub i_product: u8,
    pub i_serial_number: u8,
    pub b_num_configurations: u8,
    pub configurations: *mut UsbConfigDesc,
}

#[repr(C, packed)]
#[derive(Copy, Clone, Default)]
pub struct UsbHidDesc {
    pub b_length: u8,
    pub b_descriptor_type: u8,
    pub bcd_hid: u16,
    pub b_country_code: u8,
    pub b_num_descriptors: u8,
    pub descr_b_descriptor_type: u8,
    pub descr_w_descriptor_length: u16,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct UsbDeviceEntry {
    pub device_id: i32,
    pub vid: u16,
    pub pid: u16,
    pub token: u32,
}

const EMPTY_USB_DEVICE_ENTRY: UsbDeviceEntry = UsbDeviceEntry {
    device_id: 0,
    vid: 0,
    pid: 0,
    token: 0,
};

static USB_INITED: AtomicBool = AtomicBool::new(false);
static USB_V5_FAILURE_STAGE: AtomicU8 = AtomicU8::new(UsbV5FailureStage::None as u8);
static USB_V5_FAILURE_RESULT: AtomicI32 = AtomicI32::new(0);
#[cfg(target_arch = "powerpc")]
static USB_V5_VEN_FD: AtomicI32 = AtomicI32::new(-1);
#[cfg(target_arch = "powerpc")]
static USB_V5_VEN_READY: AtomicBool = AtomicBool::new(false);
#[cfg(target_arch = "powerpc")]
static USB_V5_VEN_DETECTED: AtomicBool = AtomicBool::new(false);
#[cfg(target_arch = "powerpc")]
static USB_V5_GENERATION: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "powerpc")]
static USB_V5_EVENT_MUTEX: AtomicU32 = AtomicU32::new(lwp::LWP_MUTEX_NULL);
#[cfg(target_arch = "powerpc")]
static USB_TRANSIENT_FD: AtomicI32 = AtomicI32::new(-1);
#[cfg(target_arch = "powerpc")]
#[repr(align(32))]
struct AlignedDeviceEntries([UsbDeviceEntry; USB_MAX_DEVICES]);
#[cfg(target_arch = "powerpc")]
static mut USB_V5_VEN_DEVICES: AlignedDeviceEntries =
    AlignedDeviceEntries([EMPTY_USB_DEVICE_ENTRY; USB_MAX_DEVICES]);
#[cfg(target_arch = "powerpc")]
static USB_V5_VEN_DMA_ADDRESS: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "powerpc")]
static mut USB_V5_EVENTS: EventState = EventState::new();
#[cfg(target_arch = "powerpc")]
static USB_V5_HID_FD: AtomicI32 = AtomicI32::new(-1);
#[cfg(target_arch = "powerpc")]
static USB_V5_HID_READY: AtomicBool = AtomicBool::new(false);
#[cfg(target_arch = "powerpc")]
static USB_V5_HID_DETECTED: AtomicBool = AtomicBool::new(false);
#[cfg(target_arch = "powerpc")]
static USB_V5_HID_GENERATION: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "powerpc")]
static USB_V5_HID_EVENT_MUTEX: AtomicU32 = AtomicU32::new(lwp::LWP_MUTEX_NULL);
#[cfg(target_arch = "powerpc")]
static mut USB_V5_HID_DEVICES: AlignedDeviceEntries =
    AlignedDeviceEntries([EMPTY_USB_DEVICE_ENTRY; USB_MAX_DEVICES]);
#[cfg(target_arch = "powerpc")]
static USB_V5_HID_DMA_ADDRESS: AtomicU32 = AtomicU32::new(0);
#[cfg(target_arch = "powerpc")]
static mut USB_V5_HID_EVENTS: EventState = EventState::new();
#[cfg(not(target_arch = "powerpc"))]
static USB_NEXT_FD: AtomicI32 = AtomicI32::new(1);
#[cfg(not(target_arch = "powerpc"))]
static USB_LAST_DEVICE_COUNT: AtomicU32 = AtomicU32::new(0);
#[cfg(not(target_arch = "powerpc"))]
static USB_LAST_CONFIGURATION: [AtomicU32; 64] = [const { AtomicU32::new(0) }; 64];

#[cfg(target_arch = "powerpc")]
#[repr(align(32))]
struct AlignedU8([u8; 1]);

#[cfg(target_arch = "powerpc")]
#[repr(align(32))]
struct AlignedU16([u8; 2]);

#[cfg(target_arch = "powerpc")]
#[repr(align(32))]
struct AlignedIoctlv3([Ioctlv; 3]);

#[cfg(target_arch = "powerpc")]
#[repr(align(32))]
struct AlignedIoctlv5([Ioctlv; 5]);

#[cfg(target_arch = "powerpc")]
#[repr(align(32))]
struct AlignedIoctlv7([Ioctlv; 7]);

#[cfg(target_arch = "powerpc")]
fn ensure_desc_heap() -> Result<HeapId, i32> {
    loop {
        match USB_DESC_HEAP_ID.load(Ordering::Acquire) {
            id if id >= 0 => return Ok(HeapId::new(id)),
            -2 => {
                core::hint::spin_loop();
                lwp::yield_thread();
            }
            _ => {
                if USB_DESC_HEAP_ID
                    .compare_exchange(-1, -2, Ordering::AcqRel, Ordering::Acquire)
                    .is_err()
                {
                    continue;
                }
                match HeapId::create(USB_DESC_HEAP_SIZE) {
                    Ok(heap) => {
                        USB_DESC_HEAP_ID.store(heap.raw(), Ordering::Release);
                        return Ok(heap);
                    }
                    Err(error) => {
                        USB_DESC_HEAP_ID.store(-1, Ordering::Release);
                        return Err(error);
                    }
                }
            }
        }
    }
}

#[cfg(target_arch = "powerpc")]
fn begin_transient_operation() -> Result<(), i32> {
    loop {
        let pending = USB_TRANSIENT_FD.load(Ordering::Acquire);
        if pending == -2 {
            return Err(IPC_EQUEUEFULL);
        }
        if USB_TRANSIENT_FD
            .compare_exchange(pending, -2, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            continue;
        }
        if pending < 0 {
            return Ok(());
        }
        match FileDescriptor::new(pending).close() {
            Ok(()) => return Ok(()),
            Err(error) => {
                USB_TRANSIENT_FD.store(pending, Ordering::Release);
                return Err(error);
            }
        }
    }
}

#[cfg(target_arch = "powerpc")]
fn finish_transient_operation(fd: i32) -> i32 {
    match FileDescriptor::new(fd).close() {
        Ok(()) => {
            USB_TRANSIENT_FD.store(-1, Ordering::Release);
            USB_OK
        }
        Err(error) => {
            USB_TRANSIENT_FD.store(fd, Ordering::Release);
            error
        }
    }
}

#[cfg(target_arch = "powerpc")]
fn retry_transient_close() -> i32 {
    match begin_transient_operation() {
        Ok(()) => {
            USB_TRANSIENT_FD.store(-1, Ordering::Release);
            USB_OK
        }
        Err(error) => error,
    }
}

#[cfg(target_arch = "powerpc")]
fn desc_recipient(type_: u8) -> u8 {
    match type_ {
        USB_DT_HID | USB_DT_REPORT | USB_DT_PHYSICAL => USB_CTRLTYPE_REC_INTERFACE,
        _ => USB_CTRLTYPE_REC_DEVICE,
    }
}

#[cfg(target_arch = "powerpc")]
fn descriptor_step(buf: &[u8], offset: usize) -> Option<usize> {
    if offset + 2 > buf.len() {
        return None;
    }
    let len = buf[offset] as usize;
    if len < 2 || offset + len > buf.len() {
        return None;
    }
    Some(len)
}

#[cfg(target_arch = "powerpc")]
fn find_next_descriptor(buf: &[u8], align: u8) -> usize {
    let mut size = buf.len();
    let mut offset = 0usize;
    while size > 2 && buf[offset] != 0 {
        if buf[offset + 1] == USB_DT_ENDPOINT || buf[offset + 1] == USB_DT_INTERFACE {
            break;
        }
        let step = (buf[offset] as usize + align as usize) & !(align as usize);
        if step == 0 || step > size {
            break;
        }
        size -= step;
        offset += step;
    }
    offset
}

#[cfg(target_arch = "powerpc")]
fn alloc_zeroed(heap: HeapId, size: usize) -> Result<NonNull<c_void>, i32> {
    let ptr = heap.alloc(size.max(1) as i32).ok_or(IPC_ENOMEM)?;
    unsafe {
        core::ptr::write_bytes(ptr.as_ptr(), 0, size.max(1));
    }
    Ok(ptr)
}

#[cfg(target_arch = "powerpc")]
struct HeapTemp {
    heap: HeapId,
    ptr: NonNull<c_void>,
}

#[cfg(target_arch = "powerpc")]
impl HeapTemp {
    fn new_zeroed(heap: HeapId, size: usize) -> Result<Self, i32> {
        Ok(Self {
            heap,
            ptr: alloc_zeroed(heap, size)?,
        })
    }

    fn as_ptr(&self) -> *mut c_void {
        self.ptr.as_ptr()
    }

    unsafe fn as_mut_slice(&mut self, size: usize) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut(self.ptr.as_ptr().cast::<u8>(), size) }
    }
}

#[cfg(target_arch = "powerpc")]
impl Drop for HeapTemp {
    fn drop(&mut self) {
        unsafe { self.heap.free(self.ptr) };
    }
}

#[cfg(target_arch = "powerpc")]
fn usb_aligned(ptr: *const c_void) -> bool {
    (ptr as usize & 31) == 0
}

#[cfg(target_arch = "powerpc")]
fn usb_transfer_buffer_valid(data: *mut c_void, length: u16) -> bool {
    if length == 0 {
        data.is_null()
    } else {
        !data.is_null() && usb_aligned(data as *const c_void)
    }
}

#[cfg(target_arch = "powerpc")]
#[repr(C)]
struct UsbClassChangeMessage {
    class: u8,
    _pad: [u8; 3],
    cb: UsbCallback,
    userdata: *mut c_void,
}

#[cfg(target_arch = "powerpc")]
#[repr(C, align(32))]
struct UsbV0DataAsyncMessage {
    argv: AlignedIoctlv3,
    endpoint: AlignedU8,
    length: AlignedU16,
    cb: UsbCallback,
    userdata: *mut c_void,
    data: *mut c_void,
    length_value: u16,
    read: bool,
}

#[cfg(target_arch = "powerpc")]
#[repr(C, align(32))]
struct UsbV0ControlAsyncMessage {
    argv: AlignedIoctlv7,
    rq_type: AlignedU8,
    rq: AlignedU8,
    value: AlignedU16,
    index: AlignedU16,
    length: AlignedU16,
    null: AlignedU8,
    cb: UsbCallback,
    userdata: *mut c_void,
    data: *mut c_void,
    length_value: u16,
    read: bool,
}

#[cfg(target_arch = "powerpc")]
#[repr(C, align(32))]
struct UsbV0IsoAsyncMessage {
    argv: AlignedIoctlv5,
    endpoint: AlignedU8,
    length: AlignedU16,
    packets: AlignedU8,
    cb: UsbCallback,
    userdata: *mut c_void,
    packet_sizes: *mut u16,
    data: *mut c_void,
    packet_bytes: u32,
    length_value: u16,
    read: bool,
}

#[cfg(target_arch = "powerpc")]
#[derive(Copy, Clone)]
struct UsbV0IsoAsyncRequest {
    fd: i32,
    endpoint: u8,
    packets: u8,
    packet_sizes: *mut u16,
    data: *mut c_void,
    read: bool,
    completion: UsbAsyncCompletion,
}

#[cfg(target_arch = "powerpc")]
#[derive(Copy, Clone)]
struct UsbV0DataAsyncRequest {
    fd: i32,
    ioctl: i32,
    endpoint: u8,
    length: u16,
    data: *mut c_void,
    read: bool,
    completion: UsbAsyncCompletion,
}

#[cfg(target_arch = "powerpc")]
unsafe fn free_heap_message<T>(ptr: *mut T) {
    unsafe {
        if let Some(ptr) = NonNull::new(ptr.cast::<c_void>()) {
            // Every USB message is allocated from the descriptor heap, which
            // is created once and kept for the process lifetime.
            HeapId::new(USB_DESC_HEAP_ID.load(Ordering::Acquire)).free(ptr);
        }
    }
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn usb_class_change_cb(result: i32, userdata: *mut c_void) -> i32 {
    unsafe {
        let msg = userdata.cast::<UsbClassChangeMessage>();
        if msg.is_null() {
            return IPC_EINVAL;
        }

        let cb = (*msg).cb;
        let user = (*msg).userdata;
        if let Some(cb) = cb {
            cb(result, user);
        }

        free_heap_message(msg);
        USB_OK
    }
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn usb_v0_data_async_cb(result: i32, userdata: *mut c_void) -> i32 {
    unsafe {
        let msg = userdata.cast::<UsbV0DataAsyncMessage>();
        if msg.is_null() {
            return IPC_EINVAL;
        }

        if (*msg).read && !(*msg).data.is_null() && (*msg).length_value != 0 {
            dc_invalidate_range((*msg).data.cast_const(), (*msg).length_value as u32);
        }

        let cb = (*msg).cb;
        let user = (*msg).userdata;
        if let Some(cb) = cb {
            cb(result, user);
        }

        free_heap_message(msg);
        USB_OK
    }
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn usb_v0_control_async_cb(result: i32, userdata: *mut c_void) -> i32 {
    unsafe {
        let msg = userdata.cast::<UsbV0ControlAsyncMessage>();
        if msg.is_null() {
            return IPC_EINVAL;
        }

        if (*msg).read && !(*msg).data.is_null() && (*msg).length_value != 0 {
            dc_invalidate_range((*msg).data.cast_const(), (*msg).length_value as u32);
        }

        let cb = (*msg).cb;
        let user = (*msg).userdata;
        if let Some(cb) = cb {
            cb(result, user);
        }

        free_heap_message(msg);
        USB_OK
    }
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn usb_v0_iso_async_cb(result: i32, userdata: *mut c_void) -> i32 {
    unsafe {
        let msg = userdata.cast::<UsbV0IsoAsyncMessage>();
        if msg.is_null() {
            return IPC_EINVAL;
        }

        if (*msg).read {
            if !(*msg).packet_sizes.is_null() && (*msg).packet_bytes != 0 {
                dc_invalidate_range(
                    (*msg).packet_sizes.cast_const().cast::<c_void>(),
                    (*msg).packet_bytes,
                );
            }
            if !(*msg).data.is_null() && (*msg).length_value != 0 {
                dc_invalidate_range((*msg).data.cast_const(), (*msg).length_value as u32);
            }
        }

        let cb = (*msg).cb;
        let user = (*msg).userdata;
        if let Some(cb) = cb {
            cb(result, user);
        }

        free_heap_message(msg);
        USB_OK
    }
}

#[cfg(target_arch = "powerpc")]
unsafe fn free_descriptor_tree(heap: HeapId, udd: &mut UsbDevDesc) {
    unsafe {
        if !udd.configurations.is_null() {
            for i_conf in 0..udd.b_num_configurations as usize {
                let cfg = &mut *udd.configurations.add(i_conf);
                if !cfg.interfaces.is_null() {
                    for i_interface in 0..cfg.b_num_interfaces as usize {
                        let iface = &mut *cfg.interfaces.add(i_interface);
                        if !iface.endpoints.is_null() {
                            heap.free(NonNull::new_unchecked(iface.endpoints.cast()));
                            iface.endpoints = core::ptr::null_mut();
                        }
                        if !iface.extra.is_null() {
                            heap.free(NonNull::new_unchecked(iface.extra.cast()));
                            iface.extra = core::ptr::null_mut();
                        }
                    }
                    heap.free(NonNull::new_unchecked(cfg.interfaces.cast()));
                    cfg.interfaces = core::ptr::null_mut();
                }
            }
            heap.free(NonNull::new_unchecked(udd.configurations.cast()));
            udd.configurations = core::ptr::null_mut();
        }
    }
}

#[cfg(target_arch = "powerpc")]
fn count_interfaces(buf: &[u8], mut offset: usize) -> usize {
    let mut count = 0usize;
    while let Some(step) = descriptor_step(buf, offset) {
        if buf[offset + 1] == USB_DT_INTERFACE {
            count += 1;
        }
        offset += step;
    }
    count
}

#[cfg(target_arch = "powerpc")]
unsafe fn fill_config_desc(dst: &mut UsbConfigDesc, raw: &[u8]) {
    dst.b_length = raw[0];
    dst.b_descriptor_type = raw[1];
    dst.w_total_length = u16::from_le_bytes([raw[2], raw[3]]);
    dst.b_num_interfaces = raw[4];
    dst.b_configuration_value = raw[5];
    dst.i_configuration = raw[6];
    dst.bm_attributes = raw[7];
    dst.b_max_power = raw[8];
    dst.interfaces = core::ptr::null_mut();
}

#[cfg(target_arch = "powerpc")]
unsafe fn fill_interface_desc(dst: &mut UsbInterfaceDesc, raw: &[u8]) {
    dst.b_length = raw[0];
    dst.b_descriptor_type = raw[1];
    dst.b_interface_number = raw[2];
    dst.b_alternate_setting = raw[3];
    dst.b_num_endpoints = raw[4];
    dst.b_interface_class = raw[5];
    dst.b_interface_sub_class = raw[6];
    dst.b_interface_protocol = raw[7];
    dst.i_interface = raw[8];
    dst.extra = core::ptr::null_mut();
    dst.extra_size = 0;
    dst.endpoints = core::ptr::null_mut();
}

#[cfg(target_arch = "powerpc")]
unsafe fn fill_endpoint_desc(dst: &mut UsbEndpointDesc, raw: &[u8]) {
    dst.b_length = raw[0];
    dst.b_descriptor_type = raw[1];
    dst.b_endpoint_address = raw[2];
    dst.bm_attributes = raw[3];
    dst.w_max_packet_size = u16::from_le_bytes([raw[4], raw[5]]);
    dst.b_interval = raw[6];
}

#[cfg(target_arch = "powerpc")]
unsafe fn parse_descriptor_tree(heap: HeapId, fd: i32, udd: &mut UsbDevDesc) -> Result<(), i32> {
    unsafe {
        let config_count = udd.b_num_configurations as usize;
        if config_count == 0 {
            return Ok(());
        }

        let configs = alloc_zeroed(heap, core::mem::size_of::<UsbConfigDesc>() * config_count)?;
        udd.configurations = configs.as_ptr().cast();

        for i_conf in 0..config_count {
            let mut header = [0u8; USB_DT_CONFIG_SIZE];
            let rc = get_generic_descriptor(
                fd,
                USB_DT_CONFIG,
                i_conf as u8,
                0,
                header.as_mut_ptr().cast(),
                header.len() as u32,
            );
            if rc != USB_OK {
                return Err(rc);
            }
            if header[0] as usize != USB_DT_CONFIG_SIZE || header[1] != USB_DT_CONFIG {
                return Err(USB_FAILED);
            }

            let total_len = u16::from_le_bytes([header[2], header[3]]) as usize;
            if total_len < USB_DT_CONFIG_SIZE {
                return Err(USB_FAILED);
            }

            let raw_alloc = HeapTemp::new_zeroed(heap, total_len)?;
            let raw_ptr = raw_alloc.as_ptr().cast::<u8>();
            let rc = get_generic_descriptor(
                fd,
                USB_DT_CONFIG,
                i_conf as u8,
                0,
                raw_ptr.cast(),
                total_len as u32,
            );
            if rc != USB_OK {
                return Err(rc);
            }

            let raw = core::slice::from_raw_parts(raw_ptr, total_len);
            let cfg = &mut *udd.configurations.add(i_conf);
            fill_config_desc(cfg, &raw[..USB_DT_CONFIG_SIZE]);

            let actual_interfaces = core::cmp::max(
                cfg.b_num_interfaces as usize,
                count_interfaces(raw, cfg.b_length as usize),
            );
            if actual_interfaces == 0 {
                continue;
            }
            if actual_interfaces > u8::MAX as usize {
                return Err(USB_FAILED);
            }

            let ifaces = alloc_zeroed(
                heap,
                core::mem::size_of::<UsbInterfaceDesc>() * actual_interfaces,
            )?;
            cfg.interfaces = ifaces.as_ptr().cast();
            cfg.b_num_interfaces = actual_interfaces as u8;

            let mut offset = cfg.b_length as usize;
            let mut i_interface = 0usize;
            while i_interface < actual_interfaces && offset < raw.len() {
                let skipped = find_next_descriptor(&raw[offset..], 0);
                offset += skipped;
                if offset + USB_DT_INTERFACE_SIZE > raw.len() || raw[offset + 1] != USB_DT_INTERFACE
                {
                    break;
                }

                let iface = &mut *cfg.interfaces.add(i_interface);
                fill_interface_desc(iface, &raw[offset..offset + USB_DT_INTERFACE_SIZE]);
                offset += iface.b_length as usize;
                if offset > raw.len() {
                    return Err(USB_FAILED);
                }

                let extra_size = find_next_descriptor(&raw[offset..], 0);
                if extra_size > 0 {
                    let extra = alloc_zeroed(heap, extra_size)?;
                    iface.extra = extra.as_ptr().cast();
                    iface.extra_size = extra_size as u16;
                    core::ptr::copy_nonoverlapping(
                        raw.as_ptr().add(offset),
                        iface.extra,
                        extra_size,
                    );
                    offset += extra_size;
                }

                if iface.b_num_endpoints > 0 {
                    let endpoints = alloc_zeroed(
                        heap,
                        core::mem::size_of::<UsbEndpointDesc>() * iface.b_num_endpoints as usize,
                    )?;
                    iface.endpoints = endpoints.as_ptr().cast();

                    for i_endpoint in 0..iface.b_num_endpoints as usize {
                        if offset + USB_DT_ENDPOINT_SIZE > raw.len()
                            || raw[offset + 1] != USB_DT_ENDPOINT
                        {
                            return Err(USB_FAILED);
                        }
                        let endpoint = &mut *iface.endpoints.add(i_endpoint);
                        fill_endpoint_desc(endpoint, &raw[offset..offset + USB_DT_ENDPOINT_SIZE]);
                        offset += endpoint.b_length as usize;
                        if offset > raw.len() {
                            return Err(USB_FAILED);
                        }
                    }
                }

                i_interface += 1;
            }

            cfg.b_num_interfaces = i_interface as u8;
        }

        Ok(())
    }
}

#[cfg(not(target_arch = "powerpc"))]
fn next_fd() -> i32 {
    USB_NEXT_FD.fetch_add(1, Ordering::AcqRel)
}

#[cfg(not(target_arch = "powerpc"))]
fn fd_index(fd: i32) -> Option<usize> {
    if fd <= 0 {
        None
    } else {
        Some(fd as usize % USB_LAST_CONFIGURATION.len())
    }
}

#[cfg(target_arch = "powerpc")]
fn hex_digit(value: u8) -> u8 {
    match value {
        0..=9 => b'0' + value,
        _ => b'a' + (value - 10),
    }
}

#[cfg(target_arch = "powerpc")]
fn push_bytes(dst: &mut [u8], len: &mut usize, src: &[u8]) -> Result<(), i32> {
    if *len + src.len() > dst.len() {
        return Err(USB_FAILED);
    }
    dst[*len..*len + src.len()].copy_from_slice(src);
    *len += src.len();
    Ok(())
}

#[cfg(target_arch = "powerpc")]
fn push_hex_u16(dst: &mut [u8], len: &mut usize, value: u16) -> Result<(), i32> {
    let mut started = false;
    for shift in [12, 8, 4, 0] {
        let nibble = ((value >> shift) & 0x0f) as u8;
        if nibble != 0 || started || shift == 0 {
            push_bytes(dst, len, &[hex_digit(nibble)])?;
            started = true;
        }
    }
    Ok(())
}

#[cfg(target_arch = "powerpc")]
fn build_device_path(device_id: i32, vid: u16, pid: u16) -> Result<([u8; 64], usize), i32> {
    let root = match device_id {
        USB_OH0_DEVICE_ID => b"/dev/usb/oh0/",
        USB_OH1_DEVICE_ID => b"/dev/usb/oh1/",
        _ => return Err(USB_FAILED),
    };

    let mut path = [0u8; 64];
    let mut len = 0usize;
    push_bytes(&mut path, &mut len, root)?;
    push_hex_u16(&mut path, &mut len, vid)?;
    push_bytes(&mut path, &mut len, b"/")?;
    push_hex_u16(&mut path, &mut len, pid)?;
    push_bytes(&mut path, &mut len, b"\0")?;
    Ok((path, len))
}

#[cfg(target_arch = "powerpc")]
fn oh0_root_path() -> &'static CStr {
    c"/dev/usb/oh0"
}

#[cfg(target_arch = "powerpc")]
fn ven_root_path() -> &'static CStr {
    c"/dev/usb/ven"
}

#[cfg(target_arch = "powerpc")]
fn is_v0_fd(fd: i32) -> bool {
    (0..0x20).contains(&fd)
}

#[cfg(target_arch = "powerpc")]
impl HostKind {
    fn fd_cell(self) -> &'static AtomicI32 {
        match self {
            Self::Vendor => &USB_V5_VEN_FD,
            Self::Hid => &USB_V5_HID_FD,
        }
    }
    fn fd(self) -> Option<i32> {
        let fd = self.fd_cell().load(Ordering::Acquire);
        (fd >= 0).then_some(fd)
    }
    fn ready(self) -> &'static AtomicBool {
        match self {
            Self::Vendor => &USB_V5_VEN_READY,
            Self::Hid => &USB_V5_HID_READY,
        }
    }
    fn detected(self) -> &'static AtomicBool {
        match self {
            Self::Vendor => &USB_V5_VEN_DETECTED,
            Self::Hid => &USB_V5_HID_DETECTED,
        }
    }
    fn mutex(self) -> &'static AtomicU32 {
        match self {
            Self::Vendor => &USB_V5_EVENT_MUTEX,
            Self::Hid => &USB_V5_HID_EVENT_MUTEX,
        }
    }
    fn generation(self) -> &'static AtomicU32 {
        match self {
            Self::Vendor => &USB_V5_GENERATION,
            Self::Hid => &USB_V5_HID_GENERATION,
        }
    }
    fn events(self) -> *mut EventState {
        match self {
            Self::Vendor => core::ptr::addr_of_mut!(USB_V5_EVENTS),
            Self::Hid => core::ptr::addr_of_mut!(USB_V5_HID_EVENTS),
        }
    }
    fn devices(self) -> *mut [UsbDeviceEntry; USB_MAX_DEVICES] {
        // Form pointers only; callers access snapshots under the host event lock.
        match self {
            Self::Vendor => unsafe { core::ptr::addr_of_mut!(USB_V5_VEN_DEVICES.0) },
            Self::Hid => unsafe { core::ptr::addr_of_mut!(USB_V5_HID_DEVICES.0) },
        }
    }
    fn dma_address(self) -> &'static AtomicU32 {
        match self {
            Self::Vendor => &USB_V5_VEN_DMA_ADDRESS,
            Self::Hid => &USB_V5_HID_DMA_ADDRESS,
        }
    }
    fn dma(self) -> Option<*mut [UsbDeviceEntry; USB_MAX_DEVICES]> {
        let address = self.dma_address().load(Ordering::Acquire) as usize;
        v5_layout::host_buffer_valid(address, core::mem::size_of::<AlignedDeviceEntries>())
            .then_some(address as *mut [UsbDeviceEntry; USB_MAX_DEVICES])
    }
    fn path(self) -> &'static CStr {
        match self {
            Self::Vendor => ven_root_path(),
            Self::Hid => c"/dev/usb/hid",
        }
    }
}

#[cfg(target_arch = "powerpc")]
struct UsbV5EventGuard {
    mutex: lwp::Mutex,
}

#[cfg(target_arch = "powerpc")]
impl Drop for UsbV5EventGuard {
    fn drop(&mut self) {
        let _ = lwp::mutex_unlock(self.mutex);
    }
}

#[cfg(target_arch = "powerpc")]
fn lock_usb_v5_events(host: HostKind) -> Result<UsbV5EventGuard, i32> {
    let mut mutex = host.mutex().load(Ordering::Acquire);
    if mutex == lwp::LWP_MUTEX_NULL {
        let mut candidate = lwp::LWP_MUTEX_NULL;
        if unsafe { lwp::mutex_init(&mut candidate, 0) } != lwp::LWP_SUCCESSFUL {
            return Err(IPC_ENOMEM);
        }
        match host.mutex().compare_exchange(
            lwp::LWP_MUTEX_NULL,
            candidate,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => mutex = candidate,
            Err(existing) => {
                let _ = lwp::mutex_destroy(candidate);
                mutex = existing;
            }
        }
    }
    if lwp::mutex_lock(mutex) != lwp::LWP_SUCCESSFUL {
        return Err(IPC_EQUEUEFULL);
    }
    Ok(UsbV5EventGuard { mutex })
}

#[cfg(target_arch = "powerpc")]
fn with_usb_v5_host_events<R>(
    host: HostKind,
    operation: impl FnOnce(&mut EventState) -> R,
) -> Result<R, i32> {
    let guard = lock_usb_v5_events(host)?;
    let state = host.events();
    let result = unsafe { operation(&mut *state) };
    drop(guard);
    Ok(result)
}

#[cfg(target_arch = "powerpc")]
fn next_usb_v5_generation(host: HostKind) -> u32 {
    let generation = host
        .generation()
        .fetch_add(1, Ordering::AcqRel)
        .wrapping_add(1)
        & 0x7fff_ffff;
    if generation == 0 {
        host.generation()
            .fetch_add(1, Ordering::AcqRel)
            .wrapping_add(1)
            & 0x7fff_ffff
    } else {
        generation
    }
}

#[cfg(target_arch = "powerpc")]
fn usb_v5_generation_userdata(host: HostKind, generation: u32) -> *mut c_void {
    (generation
        | if host == HostKind::Hid {
            0x8000_0000
        } else {
            0
        }) as usize as *mut c_void
}

#[cfg(target_arch = "powerpc")]
fn usb_v5_userdata_generation(userdata: *mut c_void) -> (HostKind, u32) {
    let value = userdata as usize as u32;
    (
        if value & 0x8000_0000 != 0 {
            HostKind::Hid
        } else {
            HostKind::Vendor
        },
        value & 0x7fff_ffff,
    )
}

#[cfg(target_arch = "powerpc")]
fn invoke_usb_v5_callbacks(callbacks: &CallbackBatch, result: i32) {
    for pending in callbacks.iter() {
        unsafe {
            (pending.callback)(result, pending.userdata);
        }
    }
}

#[cfg(target_arch = "powerpc")]
fn usb_v5_registration_result(result: Result<(), RegistrationError>, full: i32) -> i32 {
    match result {
        Ok(()) => USB_OK,
        Err(RegistrationError::MissingCallback | RegistrationError::DuplicateDevice) => IPC_EINVAL,
        Err(RegistrationError::UnknownDevice) => IPC_ENOENT,
        Err(RegistrationError::Full) => full,
        Err(RegistrationError::HostUnavailable(error)) if error < 0 => error,
        Err(RegistrationError::HostUnavailable(_)) => IPC_ENOENT,
    }
}

#[cfg(target_arch = "powerpc")]
fn fail_usb_v5_watcher(host: HostKind, generation: u32, stage: UsbV5FailureStage, error: i32) {
    let mut removals = CallbackBatch::new();
    let mut changes = CallbackBatch::new();
    let failed = with_usb_v5_host_events(host, |state| {
        state.fail(generation, error, &mut removals, &mut changes)
    });
    match failed {
        Ok(Ok(())) => {
            USB_V5_FAILURE_RESULT.store(error, Ordering::Relaxed);
            USB_V5_FAILURE_STAGE.store(stage as u8, Ordering::Release);
            host.ready().store(false, Ordering::Release);
            invoke_usb_v5_callbacks(&removals, error);
            invoke_usb_v5_callbacks(&changes, error);
        }
        Ok(Err(EventError::StaleGeneration)) => {}
        Ok(Err(EventError::TooManyDevices)) => {
            USB_V5_FAILURE_RESULT.store(IPC_EINVAL, Ordering::Relaxed);
            USB_V5_FAILURE_STAGE.store(stage as u8, Ordering::Release);
        }
        Err(lock_error) => {
            USB_V5_FAILURE_RESULT.store(lock_error, Ordering::Relaxed);
            USB_V5_FAILURE_STAGE.store(stage as u8, Ordering::Release);
        }
    }
}

#[cfg(target_arch = "powerpc")]
fn submit_usb_v5_device_change(host: HostKind, fd: i32, generation: u32) -> i32 {
    let Some(dma) = host.dma() else {
        return IPC_EINVAL;
    };
    unsafe {
        dc_invalidate_range(
            dma.cast::<c_void>(),
            core::mem::size_of::<AlignedDeviceEntries>() as u32,
        );
    }
    let devices = unsafe {
        core::slice::from_raw_parts_mut(
            dma.cast::<u8>(),
            core::mem::size_of::<AlignedDeviceEntries>(),
        )
    };
    unsafe {
        FileDescriptor::new(fd).ioctl_async(
            USBV5_IOCTL_GETDEVICECHANGE,
            IoctlBuffers::from_slices(None, Some(devices)),
            Some(usb_v5_devicechange_cb),
            usb_v5_generation_userdata(host, generation),
        )
    }
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn usb_v5_attachfinish_cb(result: i32, userdata: *mut c_void) -> i32 {
    let (host, generation) = usb_v5_userdata_generation(userdata);
    if result < 0 {
        fail_usb_v5_watcher(host, generation, UsbV5FailureStage::AttachFinish, result);
        return USB_OK;
    }
    let mut changes = CallbackBatch::new();
    match with_usb_v5_host_events(host, |state| state.finish_change(generation, &mut changes)) {
        Ok(Ok(())) => {
            clear_v5_failure();
            invoke_usb_v5_callbacks(&changes, result);
        }
        Ok(Err(EventError::StaleGeneration)) => return USB_OK,
        Ok(Err(EventError::TooManyDevices)) => {
            fail_usb_v5_watcher(
                host,
                generation,
                UsbV5FailureStage::AttachFinish,
                IPC_EINVAL,
            );
            return USB_OK;
        }
        Err(error) => {
            fail_usb_v5_watcher(host, generation, UsbV5FailureStage::AttachFinish, error);
            return USB_OK;
        }
    }

    let Some(fd) = host.fd() else {
        fail_usb_v5_watcher(
            host,
            generation,
            UsbV5FailureStage::AttachFinish,
            IPC_ENOENT,
        );
        return USB_OK;
    };
    let rc = submit_usb_v5_device_change(host, fd, generation);
    if rc < 0 {
        fail_usb_v5_watcher(host, generation, UsbV5FailureStage::AttachFinish, rc);
    }
    USB_OK
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn usb_v5_devicechange_cb(result: i32, userdata: *mut c_void) -> i32 {
    let (host, generation) = usb_v5_userdata_generation(userdata);
    if result < 0 || result as usize > USB_MAX_DEVICES {
        fail_usb_v5_watcher(
            host,
            generation,
            UsbV5FailureStage::DeviceChange,
            if result < 0 { result } else { IPC_EINVAL },
        );
        return USB_OK;
    }

    let used = result as usize;
    let Some(dma) = host.dma() else {
        fail_usb_v5_watcher(
            host,
            generation,
            UsbV5FailureStage::DeviceChange,
            IPC_EINVAL,
        );
        return USB_OK;
    };

    let mut removals = CallbackBatch::new();
    let applied = with_usb_v5_host_events(host, |state| {
        // A deferred completion from an old session must not read the buffer
        // after a new watcher has acquired it.
        if state.generation() != generation {
            return Err(EventError::StaleGeneration);
        }
        let mut entries = [EMPTY_USB_DEVICE_ENTRY; USB_MAX_DEVICES];
        let mut device_ids = [0i32; USB_MAX_DEVICES];
        unsafe {
            dc_invalidate_range(
                dma.cast(),
                core::mem::size_of::<AlignedDeviceEntries>() as u32,
            );
            let source = dma.cast::<UsbDeviceEntry>();
            for index in 0..used {
                entries[index] = core::ptr::read_volatile(source.add(index));
                device_ids[index] = entries[index].device_id;
            }
        }
        let result = state.apply_snapshot(generation, &device_ids[..used], &mut removals);
        if result.is_ok() {
            unsafe {
                *host.devices() = entries;
            }
        }
        result
    });
    match applied {
        Ok(Ok(())) => {
            clear_v5_failure();
            host.ready().store(true, Ordering::Release);
        }
        Ok(Err(EventError::StaleGeneration)) => return USB_OK,
        Ok(Err(EventError::TooManyDevices)) => {
            fail_usb_v5_watcher(
                host,
                generation,
                UsbV5FailureStage::DeviceChange,
                IPC_EINVAL,
            );
            return USB_OK;
        }
        Err(error) => {
            fail_usb_v5_watcher(host, generation, UsbV5FailureStage::DeviceChange, error);
            return USB_OK;
        }
    }
    invoke_usb_v5_callbacks(&removals, USB_OK);

    let Some(fd) = host.fd() else {
        fail_usb_v5_watcher(
            host,
            generation,
            UsbV5FailureStage::DeviceChange,
            IPC_ENOENT,
        );
        return USB_OK;
    };
    let rc = unsafe {
        FileDescriptor::new(fd).ioctl_async(
            USBV5_IOCTL_ATTACHFINISH,
            IoctlBuffers::empty(),
            Some(usb_v5_attachfinish_cb),
            usb_v5_generation_userdata(host, generation),
        )
    };
    if rc < 0 {
        fail_usb_v5_watcher(host, generation, UsbV5FailureStage::DeviceChange, rc);
    }
    USB_OK
}

#[cfg(target_arch = "powerpc")]
fn start_v5_host(host: HostKind) -> i32 {
    if host.fd().is_some() {
        return USB_OK;
    }

    let Ok(fd) = open_file(host.path(), IPC_OPEN_NONE) else {
        host.detected().store(false, Ordering::Release);
        return USB_OK;
    };

    let heap = match ensure_desc_heap() {
        Ok(heap) => heap,
        Err(error) => {
            let _ = fd.close();
            return error;
        }
    };
    let mut version = match HeapTemp::new_zeroed(heap, 0x20) {
        Ok(version) => version,
        Err(error) => {
            let _ = fd.close();
            return error;
        }
    };

    let vendor_v5 = HostKind::Vendor.detected().load(Ordering::Acquire);
    let valid_version = v5_layout::check_host_version(host, vendor_v5, |request| {
        if request != USBV5_IOCTL_GETVERSION {
            return (fd.ioctl(request, IoctlBuffers::empty()), 0);
        }
        let rc = fd.ioctl(
            request,
            IoctlBuffers::from_slices(None, Some(unsafe { version.as_mut_slice(0x20) })),
        );
        let bytes = unsafe { version.as_mut_slice(4) };
        (rc, u32::from_ne_bytes(bytes[..4].try_into().unwrap()))
    });
    if !valid_version {
        let _ = fd.close();
        host.detected().store(false, Ordering::Release);
        return USB_OK;
    }

    clear_v5_failure();
    host.detected().store(true, Ordering::Release);
    host.ready().store(false, Ordering::Release);
    host.fd_cell().store(fd.raw(), Ordering::Release);
    let generation = next_usb_v5_generation(host);
    let started = with_usb_v5_host_events(host, |state| {
        let dma = match host.dma() {
            Some(dma) => dma,
            None => {
                let buffer = heap
                    .alloc(core::mem::size_of::<AlignedDeviceEntries>() as i32)
                    .ok_or(IPC_ENOMEM)?;
                if !v5_layout::host_buffer_valid(
                    buffer.as_ptr() as usize,
                    core::mem::size_of::<AlignedDeviceEntries>(),
                ) {
                    unsafe { heap.free(buffer) };
                    return Err(IPC_EINVAL);
                }
                // Reserve one MEM2 buffer per host for the process lifetime.
                // Reuse it on reinitialization; never free storage that a
                // deferred IOS completion can still reference.
                host.dma_address()
                    .store(buffer.as_ptr() as u32, Ordering::Release);
                buffer.as_ptr().cast::<[UsbDeviceEntry; USB_MAX_DEVICES]>()
            }
        };
        state.start(generation);
        unsafe {
            *host.devices() = [EMPTY_USB_DEVICE_ENTRY; USB_MAX_DEVICES];
            *dma = [EMPTY_USB_DEVICE_ENTRY; USB_MAX_DEVICES];
        }
        Ok(())
    });
    if let Err(error) = started.and_then(core::convert::identity) {
        host.fd_cell().store(-1, Ordering::Release);
        host.detected().store(false, Ordering::Release);
        let _ = fd.close();
        return error;
    }

    let rc = submit_usb_v5_device_change(host, fd.raw(), generation);
    if rc < 0 {
        fail_usb_v5_watcher(host, generation, UsbV5FailureStage::DeviceChange, rc);
        host.fd_cell().store(-1, Ordering::Release);
        let _ = fd.close();
        return rc;
    }
    USB_OK
}

#[cfg(target_arch = "powerpc")]
fn wait_for_v5_devices(host: HostKind) {
    if !host.detected().load(Ordering::Acquire) {
        return;
    }
    let start = lwp::now_ticks();
    while !host.ready().load(Ordering::Acquire) {
        let _ = ogc_ipc::poll();
        if host.ready().load(Ordering::Acquire)
            || v5_layout::timeout_elapsed(start, lwp::now_ticks(), V5_DEVICE_WAIT_TIMEOUT_TICKS)
        {
            break;
        }
        lwp::sleep_ticks(V5_DEVICE_WAIT_POLL_TICKS);
    }
}

#[cfg(target_arch = "powerpc")]
fn usb_v5_host_contains(host: HostKind, device_id: i32) -> bool {
    with_usb_v5_host_events(host, |state| state.contains(device_id)).unwrap_or(false)
}

#[cfg(target_arch = "powerpc")]
fn usb_v5_device_host(device_id: i32) -> Option<HostKind> {
    let host = v5_layout::host_for_device(device_id)?;
    (host.fd().is_some() && usb_v5_host_contains(host, device_id)).then_some(host)
}

#[cfg(target_arch = "powerpc")]
fn usb_v5_suspend_resume(device_id: i32, resumed: bool) -> i32 {
    let Some(host) = usb_v5_device_host(device_id) else {
        return IPC_EINVAL;
    };
    let Some(fd) = host.fd() else {
        return IPC_EINVAL;
    };

    let heap = match ensure_desc_heap() {
        Ok(heap) => heap,
        Err(rc) => return rc,
    };
    let mut input = match HeapTemp::new_zeroed(heap, 32) {
        Ok(input) => input,
        Err(rc) => return rc,
    };
    let input = unsafe { input.as_mut_slice(32) };
    input[0..4].copy_from_slice(&device_id.to_ne_bytes());
    input[8..12].copy_from_slice(&(resumed as i32).to_ne_bytes());
    FileDescriptor::new(fd).ioctl(
        USBV5_IOCTL_SUSPEND_RESUME,
        IoctlBuffers::from_slices(Some(input), None),
    )
}

#[cfg(not(target_arch = "powerpc"))]
pub fn initialize() -> i32 {
    USB_INITED.store(true, Ordering::Release);
    USB_OK
}

#[cfg(target_arch = "powerpc")]
fn initialize_hosts(hosts: &[HostKind]) -> i32 {
    match ensure_desc_heap() {
        Ok(_) => {
            for &host in hosts {
                let result = start_v5_host(host);
                if result < 0 {
                    return result;
                }
            }
            // C USB_Initialize submits the device-change request for each
            // available host before it returns. Start every watcher before
            // polling either one, so IOS can enumerate vendor and HID hosts
            // at the same time.
            for &host in hosts {
                wait_for_v5_devices(host);
            }
            USB_INITED.store(true, Ordering::Release);
            USB_OK
        }
        Err(rc) => rc,
    }
}

#[cfg(target_arch = "powerpc")]
pub fn initialize() -> i32 {
    initialize_hosts(&[HostKind::Vendor, HostKind::Hid])
}

#[cfg(target_arch = "powerpc")]
pub fn initialize_vendor() -> i32 {
    initialize_hosts(&[HostKind::Vendor])
}

#[cfg(not(target_arch = "powerpc"))]
pub fn initialize_vendor() -> i32 {
    USB_INITED.store(true, Ordering::Release);
    USB_OK
}

pub fn deinitialize() -> i32 {
    #[cfg(target_arch = "powerpc")]
    {
        deinitialize_hosts(&[HostKind::Vendor, HostKind::Hid])
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        USB_INITED.store(false, Ordering::Release);
        USB_OK
    }
}

#[cfg(target_arch = "powerpc")]
pub fn deinitialize_vendor() -> i32 {
    deinitialize_hosts(&[HostKind::Vendor])
}

#[cfg(not(target_arch = "powerpc"))]
pub fn deinitialize_vendor() -> i32 {
    USB_INITED.store(false, Ordering::Release);
    USB_OK
}

#[cfg(target_arch = "powerpc")]
fn deinitialize_hosts(hosts: &[HostKind]) -> i32 {
    let mut first_error = USB_OK;
    for &host in hosts {
        if host.detected().load(Ordering::Acquire)
            && let Err(error) = with_usb_v5_host_events(host, |state| state.stop_silent(IPC_ENOENT))
            && first_error == USB_OK
        {
            first_error = error;
        }
        host.ready().store(false, Ordering::Release);
        host.detected().store(false, Ordering::Release);
        if let Some(fd) = host.fd() {
            let shutdown =
                FileDescriptor::new(fd).ioctl(USBV5_IOCTL_SHUTDOWN, IoctlBuffers::empty());
            if shutdown < 0 && first_error == USB_OK {
                first_error = shutdown;
            }
            if let Err(error) = FileDescriptor::new(fd).close()
                && first_error == USB_OK
            {
                first_error = error;
            }
            host.fd_cell().store(-1, Ordering::Release);
        }
    }
    let transient_close = retry_transient_close();
    if transient_close != USB_OK && first_error == USB_OK {
        first_error = transient_close;
    }
    USB_INITED.store(false, Ordering::Release);
    first_error
}

/// # Safety
///
/// `fd` must be null or a valid writable pointer.
#[cfg(not(target_arch = "powerpc"))]
pub unsafe fn open_device(device_id: i32, vid: u16, pid: u16, fd: *mut i32) -> i32 {
    if fd.is_null() {
        return USB_FAILED;
    }
    let assigned = next_fd();
    unsafe {
        *fd = assigned;
    }
    USB_LAST_DEVICE_COUNT.fetch_add(1, Ordering::AcqRel);
    let _ = (device_id, vid, pid);
    USB_OK
}

#[cfg(target_arch = "powerpc")]
fn prepare_v5_device_with(
    device_id: i32,
    host: HostKind,
    resume: impl FnOnce(i32) -> i32,
    get_descriptors: impl FnOnce(i32, &mut UsbDevDesc) -> i32,
    free_descriptors: impl FnOnce(&mut UsbDevDesc),
    suspend: impl FnOnce(i32) -> i32,
) -> i32 {
    if host != HostKind::Hid {
        // C treats vendor-device resume as best effort.
        let _ = resume(device_id);
        return USB_OK;
    }

    prepare_hid_v5_device_with(
        device_id,
        resume,
        get_descriptors,
        free_descriptors,
        suspend,
    )
}

/// Executes the HID v5 open preflight using the same operations as production.
/// This helper is public so the Dolphin probe can check the production policy
/// with fixed operation results, without claiming IOS or hardware coverage.
#[doc(hidden)]
pub fn prepare_hid_v5_device_with(
    device_id: i32,
    resume: impl FnOnce(i32) -> i32,
    get_descriptors: impl FnOnce(i32, &mut UsbDevDesc) -> i32,
    free_descriptors: impl FnOnce(&mut UsbDevDesc),
    suspend: impl FnOnce(i32) -> i32,
) -> i32 {
    // C treats resume as a best-effort prerequisite; HID devices require a
    // descriptor round-trip before they are considered successfully open.
    let _ = resume(device_id);
    let mut descriptor = UsbDevDesc::default();
    let rc = get_descriptors(device_id, &mut descriptor);
    if rc >= 0 {
        free_descriptors(&mut descriptor);
        USB_OK
    } else {
        let _ = suspend(device_id);
        rc
    }
}

#[cfg(target_arch = "powerpc")]
unsafe fn prepare_v5_device_for_open(device_id: i32, host: HostKind) -> i32 {
    prepare_v5_device_with(
        device_id,
        host,
        resume_device,
        |id, descriptor| unsafe { get_descriptors(id, descriptor) },
        |descriptor| unsafe { free_descriptors(descriptor) },
        suspend_device,
    )
}

/// # Safety
///
/// `fd` must be null or a valid writable pointer.
#[cfg(target_arch = "powerpc")]
pub unsafe fn open_device(device_id: i32, vid: u16, pid: u16, fd: *mut i32) -> i32 {
    if fd.is_null() {
        return USB_FAILED;
    }

    unsafe {
        *fd = -1;
    }

    if !is_v0_fd(device_id)
        && let Some(host) = usb_v5_device_host(device_id)
    {
        let rc = unsafe { prepare_v5_device_for_open(device_id, host) };
        if rc < 0 {
            return rc;
        }
        unsafe {
            *fd = device_id;
        }
        return USB_OK;
    }

    let (path, len) = match build_device_path(device_id, vid, pid) {
        Ok(value) => value,
        Err(rc) => return rc,
    };
    let path = unsafe { CStr::from_bytes_with_nul_unchecked(&path[..len]) };
    match open_file(path, IPC_OPEN_NONE) {
        Ok(handle) => {
            unsafe {
                *fd = handle.raw();
            }
            USB_OK
        }
        Err(rc) => rc,
    }
}

/// # Safety
///
/// `fd` must be null or a valid writable pointer.
#[cfg(not(target_arch = "powerpc"))]
pub unsafe fn close_device(fd: *mut i32) -> i32 {
    if fd.is_null() {
        return USB_FAILED;
    }
    unsafe {
        *fd = -1;
    }
    USB_OK
}

fn close_device_async_result(close_result: i32, cb: UsbCallback, usrdata: *mut c_void) -> i32 {
    match cb {
        Some(cb) => unsafe { cb(close_result, usrdata) },
        None => close_result,
    }
}

/// # Safety
///
/// `fd` must be null or a valid writable pointer.
#[cfg(target_arch = "powerpc")]
pub unsafe fn close_device(fd: *mut i32) -> i32 {
    if fd.is_null() {
        return USB_FAILED;
    }

    let raw = unsafe { *fd };
    if !is_v0_fd(raw) {
        if let Some(host) = usb_v5_device_host(raw) {
            let mut callbacks = CallbackBatch::new();
            if let Err(error) = with_usb_v5_host_events(host, |state| {
                state.close_device(raw, &mut callbacks);
            }) {
                return error;
            }
            unsafe {
                *fd = -1;
            }
            invoke_usb_v5_callbacks(&callbacks, USB_OK);
            return USB_OK;
        }
        if raw <= 0 {
            return IPC_EINVAL;
        }
    }

    match FileDescriptor::new(raw).close() {
        Ok(()) => {
            unsafe {
                *fd = -1;
            }
            USB_OK
        }
        Err(rc) => rc,
    }
}

/// # Safety
///
/// `fd` must be null or a valid writable pointer.
#[cfg(not(target_arch = "powerpc"))]
pub unsafe fn close_device_async(fd: *mut i32, cb: UsbCallback, usrdata: *mut c_void) -> i32 {
    let rc = close_device(fd);
    close_device_async_result(rc, cb, usrdata)
}

/// # Safety
///
/// `fd` must be null or a valid writable pointer. `cb` and `usrdata` must
/// remain valid until the IOS close callback returns.
#[cfg(target_arch = "powerpc")]
pub unsafe fn close_device_async(fd: *mut i32, cb: UsbCallback, usrdata: *mut c_void) -> i32 {
    if fd.is_null() {
        return IPC_EINVAL;
    }

    let raw = unsafe { *fd };
    if !is_v0_fd(raw) {
        if usb_v5_device_host(raw).is_some() {
            let rc = unsafe { close_device(fd) };
            return close_device_async_result(rc, cb, usrdata);
        }
        if raw <= 0 {
            return IPC_EINVAL;
        }
    }

    unsafe { FileDescriptor::new(raw).close_async(cb, usrdata) }
}

#[cfg(target_arch = "powerpc")]
unsafe fn fill_device_desc(dst: &mut UsbDevDesc, raw: &[u8]) {
    dst.b_length = raw[0];
    dst.b_descriptor_type = raw[1];
    dst.bcd_usb = u16::from_le_bytes([raw[2], raw[3]]);
    dst.b_device_class = raw[4];
    dst.b_device_sub_class = raw[5];
    dst.b_device_protocol = raw[6];
    dst.b_max_packet_size0 = raw[7];
    dst.id_vendor = u16::from_le_bytes([raw[8], raw[9]]);
    dst.id_product = u16::from_le_bytes([raw[10], raw[11]]);
    dst.bcd_device = u16::from_le_bytes([raw[12], raw[13]]);
    dst.i_manufacturer = raw[14];
    dst.i_product = raw[15];
    dst.i_serial_number = raw[16];
    dst.b_num_configurations = raw[17];
    dst.configurations = core::ptr::null_mut();
}

#[cfg(target_arch = "powerpc")]
unsafe fn usb_v5_get_descriptors(device_id: i32, udd: &mut UsbDevDesc) -> i32 {
    let rc = unsafe { usb_v5_read_descriptors(device_id, udd) };
    if rc != USB_OK
        && let Ok(heap) = ensure_desc_heap()
    {
        unsafe { free_descriptor_tree(heap, udd) };
    }
    rc
}

#[cfg(target_arch = "powerpc")]
unsafe fn usb_v5_read_descriptors(device_id: i32, udd: &mut UsbDevDesc) -> i32 {
    unsafe {
        let Some(host) = usb_v5_device_host(device_id) else {
            return IPC_EINVAL;
        };
        let Some(fd) = host.fd() else {
            return IPC_EINVAL;
        };

        let heap = match ensure_desc_heap() {
            Ok(heap) => heap,
            Err(rc) => return rc,
        };
        let mut input = match HeapTemp::new_zeroed(heap, 0x20) {
            Ok(input) => input,
            Err(rc) => return rc,
        };
        let (output_size, mut offset) = v5_layout::descriptor_layout(host);
        let mut output = match HeapTemp::new_zeroed(heap, output_size) {
            Ok(output) => output,
            Err(rc) => return rc,
        };
        {
            let input = input.as_mut_slice(0x20);
            input[0..4].copy_from_slice(&device_id.to_ne_bytes());
        }
        let rc = FileDescriptor::new(fd).ioctl(
            USBV5_IOCTL_GETDEVPARAMS,
            IoctlBuffers::from_slices(
                Some(input.as_mut_slice(0x20)),
                Some(output.as_mut_slice(output_size)),
            ),
        );
        if rc != USB_OK {
            return rc;
        }

        let raw = output.as_mut_slice(output_size);
        if offset + USB_DT_DEVICE_SIZE > raw.len() {
            return USB_FAILED;
        }
        fill_device_desc(udd, &raw[offset..offset + USB_DT_DEVICE_SIZE]);
        udd.bcd_usb = u16::from_ne_bytes([raw[offset + 2], raw[offset + 3]]);
        udd.id_vendor = u16::from_ne_bytes([raw[offset + 8], raw[offset + 9]]);
        udd.id_product = u16::from_ne_bytes([raw[offset + 10], raw[offset + 11]]);
        udd.bcd_device = u16::from_ne_bytes([raw[offset + 12], raw[offset + 13]]);
        offset += (udd.b_length as usize + 3) & !3;

        let config_count = udd.b_num_configurations as usize;
        if config_count == 0 {
            return USB_OK;
        }
        let configs = match alloc_zeroed(heap, core::mem::size_of::<UsbConfigDesc>() * config_count)
        {
            Ok(configs) => configs,
            Err(rc) => return rc,
        };
        udd.configurations = configs.as_ptr().cast();

        for i_conf in 0..config_count {
            if offset + USB_DT_CONFIG_SIZE > raw.len() {
                return USB_FAILED;
            }
            let cfg = &mut *udd.configurations.add(i_conf);
            fill_config_desc(cfg, &raw[offset..offset + USB_DT_CONFIG_SIZE]);
            cfg.w_total_length = u16::from_ne_bytes([raw[offset + 2], raw[offset + 3]]);
            offset += (USB_DT_CONFIG_SIZE + 3) & !3;
            if cfg.b_num_interfaces == 0 {
                continue;
            }

            let ifaces = match alloc_zeroed(heap, core::mem::size_of::<UsbInterfaceDesc>()) {
                Ok(ifaces) => ifaces,
                Err(rc) => return rc,
            };
            cfg.interfaces = ifaces.as_ptr().cast();
            cfg.b_num_interfaces = 1;

            if offset + USB_DT_INTERFACE_SIZE > raw.len()
                || raw[offset] as usize != USB_DT_INTERFACE_SIZE
                || raw[offset + 1] != USB_DT_INTERFACE
            {
                return USB_FAILED;
            }
            let iface = &mut *cfg.interfaces;
            fill_interface_desc(iface, &raw[offset..offset + USB_DT_INTERFACE_SIZE]);
            offset += (iface.b_length as usize + 3) & !3;
            if offset > raw.len() {
                return USB_FAILED;
            }

            let extra_size = find_next_descriptor(&raw[offset..], 3);
            if extra_size > 0 {
                let extra = match alloc_zeroed(heap, extra_size) {
                    Ok(extra) => extra,
                    Err(rc) => return rc,
                };
                iface.extra = extra.as_ptr().cast();
                iface.extra_size = extra_size as u16;
                core::ptr::copy_nonoverlapping(raw.as_ptr().add(offset), iface.extra, extra_size);
                offset += extra_size;
            }

            if iface.b_num_endpoints > 0 {
                let endpoints = match alloc_zeroed(
                    heap,
                    core::mem::size_of::<UsbEndpointDesc>() * iface.b_num_endpoints as usize,
                ) {
                    Ok(endpoints) => endpoints,
                    Err(rc) => return rc,
                };
                iface.endpoints = endpoints.as_ptr().cast();

                for i_endpoint in 0..iface.b_num_endpoints as usize {
                    if offset + USB_DT_ENDPOINT_SIZE > raw.len()
                        || raw[offset] as usize != USB_DT_ENDPOINT_SIZE
                        || raw[offset + 1] != USB_DT_ENDPOINT
                    {
                        return USB_FAILED;
                    }
                    let endpoint = &mut *iface.endpoints.add(i_endpoint);
                    fill_endpoint_desc(endpoint, &raw[offset..offset + USB_DT_ENDPOINT_SIZE]);
                    endpoint.w_max_packet_size =
                        u16::from_ne_bytes([raw[offset + 4], raw[offset + 5]]);
                    offset += (endpoint.b_length as usize + 3) & !3;
                    if offset > raw.len() {
                        return USB_FAILED;
                    }
                }
            }
        }

        USB_OK
    }
}

/// # Safety
///
/// `udd` must be null or a valid writable pointer to a `UsbDevDesc`.
#[cfg(not(target_arch = "powerpc"))]
pub unsafe fn get_device_description(fd: i32, devdesc: *mut UsbDevDesc) -> i32 {
    if devdesc.is_null() {
        return USB_FAILED;
    }
    unsafe {
        *devdesc = UsbDevDesc {
            b_length: 18,
            b_descriptor_type: USB_DT_DEVICE,
            bcd_usb: 0x0200,
            b_device_class: USB_CLASS_HID,
            b_device_sub_class: USB_SUBCLASS_NONE,
            b_device_protocol: USB_PROTOCOL_NONE,
            b_max_packet_size0: 64,
            id_vendor: 0,
            id_product: fd as u16,
            bcd_device: 0x0100,
            i_manufacturer: 0,
            i_product: 0,
            i_serial_number: 0,
            b_num_configurations: 1,
            configurations: core::ptr::null_mut(),
        };
    }
    USB_OK
}

/// # Safety
///
/// `udd` must be null or a valid writable pointer to a `UsbDevDesc`.
#[cfg(target_arch = "powerpc")]
pub unsafe fn get_device_description(fd: i32, devdesc: *mut UsbDevDesc) -> i32 {
    unsafe {
        if devdesc.is_null() {
            return USB_FAILED;
        }

        {
            *devdesc = UsbDevDesc::default();
        }

        if !is_v0_fd(fd) {
            let mut full = UsbDevDesc::default();
            let rc = { usb_v5_get_descriptors(fd, &mut full) };
            if rc == USB_OK {
                {
                    *devdesc = full;
                    (*devdesc).configurations = core::ptr::null_mut();
                }
                if let Ok(heap) = ensure_desc_heap() {
                    {
                        free_descriptor_tree(heap, &mut full);
                    }
                }
            }
            return rc;
        }

        let rc = get_generic_descriptor(
            fd,
            USB_DT_DEVICE,
            0,
            0,
            devdesc.cast(),
            USB_DT_DEVICE_SIZE as u32,
        );
        if rc == USB_OK {
            {
                (*devdesc).bcd_usb = u16::from_le((*devdesc).bcd_usb);
                (*devdesc).id_vendor = u16::from_le((*devdesc).id_vendor);
                (*devdesc).id_product = u16::from_le((*devdesc).id_product);
                (*devdesc).bcd_device = u16::from_le((*devdesc).bcd_device);
                (*devdesc).configurations = core::ptr::null_mut();
            }
        }
        rc
    }
}

/// # Safety
///
/// `udd` must be null or a valid writable pointer to a `UsbDevDesc`.
#[cfg(not(target_arch = "powerpc"))]
pub unsafe fn get_descriptors(fd: i32, udd: *mut UsbDevDesc) -> i32 {
    get_device_description(fd, udd)
}

/// # Safety
///
/// `udd` must be null or a valid writable pointer to a `UsbDevDesc`.
#[cfg(target_arch = "powerpc")]
pub unsafe fn get_descriptors(fd: i32, udd: *mut UsbDevDesc) -> i32 {
    unsafe {
        if udd.is_null() {
            return USB_FAILED;
        }

        if !is_v0_fd(fd) {
            {
                *udd = UsbDevDesc::default();
                return usb_v5_get_descriptors(fd, &mut *udd);
            }
        }

        let rc = get_device_description(fd, udd);
        if rc != USB_OK {
            return rc;
        }

        let heap = match ensure_desc_heap() {
            Ok(heap) => heap,
            Err(rc) => return rc,
        };

        let result = { parse_descriptor_tree(heap, fd, &mut *udd) };
        match result {
            Ok(()) => USB_OK,
            Err(rc) => {
                {
                    free_descriptor_tree(heap, &mut *udd);
                }
                rc
            }
        }
    }
}

/// # Safety
///
/// `udd` must be a valid pointer returned by this crate or null.
#[cfg(not(target_arch = "powerpc"))]
pub unsafe fn free_descriptors(udd: *mut UsbDevDesc) {
    let _ = udd;
}

/// # Safety
///
/// `udd` must be a valid pointer returned by this crate or null.
#[cfg(target_arch = "powerpc")]
pub unsafe fn free_descriptors(udd: *mut UsbDevDesc) {
    if udd.is_null() {
        return;
    }
    if let Ok(heap) = ensure_desc_heap() {
        unsafe {
            free_descriptor_tree(heap, &mut *udd);
        }
    }
}

/// # Safety
///
/// `data` must be null or a valid writable buffer of `size` bytes.
#[cfg(not(target_arch = "powerpc"))]
pub unsafe fn get_generic_descriptor(
    fd: i32,
    type_: u8,
    index: u8,
    interface: u8,
    data: *mut c_void,
    size: u32,
) -> i32 {
    let _ = (fd, index, interface);
    if data.is_null() || size == 0 {
        return USB_FAILED;
    }
    unsafe {
        let bytes = core::slice::from_raw_parts_mut(data.cast::<u8>(), size as usize);
        bytes[0] = size.min(u8::MAX as u32) as u8;
        if bytes.len() > 1 {
            bytes[1] = type_;
        }
    }
    USB_OK
}

/// # Safety
///
/// `data` must be null or a valid writable buffer of `size` bytes.
#[cfg(target_arch = "powerpc")]
pub unsafe fn get_generic_descriptor(
    fd: i32,
    type_: u8,
    index: u8,
    interface: u8,
    data: *mut c_void,
    size: u32,
) -> i32 {
    unsafe {
        if data.is_null() || size == 0 || size > u16::MAX as u32 {
            return USB_FAILED;
        }

        let heap = match ensure_desc_heap() {
            Ok(heap) => heap,
            Err(rc) => return rc,
        };
        let buffer = match HeapTemp::new_zeroed(heap, size as usize) {
            Ok(buffer) => buffer,
            Err(rc) => return rc,
        };

        let rc = read_ctrl_msg(
            fd,
            USB_CTRLTYPE_DIR_DEVICE2HOST | USB_CTRLTYPE_TYPE_STANDARD | desc_recipient(type_),
            USB_REQ_GETDESCRIPTOR,
            ((type_ as u16) << 8) | index as u16,
            interface as u16,
            size as u16,
            buffer.as_ptr(),
        );
        if rc >= 0 {
            {
                core::ptr::copy_nonoverlapping(
                    buffer.as_ptr().cast::<u8>(),
                    data.cast::<u8>(),
                    size as usize,
                );
            }
            USB_OK
        } else {
            rc
        }
    }
}

/// # Safety
///
/// `uhd` must be null or point to a writable buffer of at least `size` bytes.
#[cfg(not(target_arch = "powerpc"))]
pub unsafe fn get_hid_descriptor(fd: i32, interface: u8, uhd: *mut UsbHidDesc, size: u32) -> i32 {
    let _ = (fd, interface, size);
    if uhd.is_null() || size < 9 {
        return IPC_EINVAL;
    }
    unsafe {
        *uhd = UsbHidDesc {
            b_length: 9,
            b_descriptor_type: USB_DT_HID,
            bcd_hid: 0x0111,
            b_country_code: 0,
            b_num_descriptors: 1,
            descr_b_descriptor_type: USB_DT_REPORT,
            descr_w_descriptor_length: 64,
        };
    }
    USB_OK
}

/// # Safety
///
/// `uhd` must be null or point to a writable buffer of at least `size` bytes.
#[cfg(target_arch = "powerpc")]
pub unsafe fn get_hid_descriptor(fd: i32, interface: u8, uhd: *mut UsbHidDesc, size: u32) -> i32 {
    unsafe {
        if uhd.is_null() || size < 9 {
            return IPC_EINVAL;
        }

        let rc = get_generic_descriptor(fd, USB_DT_HID, 0, interface, uhd.cast(), size);
        if rc == USB_OK {
            {
                let bytes = core::slice::from_raw_parts_mut(uhd.cast::<u8>(), size as usize);
                convert_hid_descriptor_lengths(bytes);
            }
        }
        rc
    }
}

/// Convert HID descriptor length fields in the byte buffer used by production.
/// Exposed for the focused Dolphin regression probe.
#[doc(hidden)]
pub fn convert_hid_descriptor_lengths(bytes: &mut [u8]) {
    const HID_HEADER_SIZE: usize = 9;
    const HID_DESCRIPTOR_ITEM_SIZE: usize = 3;

    if bytes.len() < HID_HEADER_SIZE {
        return;
    }

    let bcd_hid = u16::from_le_bytes([bytes[2], bytes[3]]).to_ne_bytes();
    bytes[2..4].copy_from_slice(&bcd_hid);

    let first_length = u16::from_le_bytes([bytes[7], bytes[8]]).to_ne_bytes();
    bytes[7..9].copy_from_slice(&first_length);

    let num_descriptors = bytes[5] as usize;
    let mut index = 1;
    let mut offset = HID_HEADER_SIZE;
    while index < num_descriptors && bytes.len().saturating_sub(offset) >= HID_DESCRIPTOR_ITEM_SIZE
    {
        let length_offset = offset + 1;
        let length =
            u16::from_le_bytes([bytes[length_offset], bytes[length_offset + 1]]).to_ne_bytes();
        bytes[length_offset..length_offset + 2].copy_from_slice(&length);
        index += 1;
        offset += HID_DESCRIPTOR_ITEM_SIZE;
    }
}

pub fn get_report_descriptor_size(fd: i32, interface: u8) -> i32 {
    let mut hid = UsbHidDesc::default();
    let ret = unsafe {
        get_hid_descriptor(
            fd,
            interface,
            &mut hid,
            core::mem::size_of::<UsbHidDesc>() as u32,
        )
    };
    if ret != USB_OK {
        return ret;
    }
    if hid.b_length > 9 {
        return -1;
    }

    if hid.b_num_descriptors == 0 || hid.descr_b_descriptor_type != USB_DT_REPORT {
        return -2;
    }

    hid.descr_w_descriptor_length as i32
}

/// # Safety
///
/// `data` must be null or a valid writable buffer of `size` bytes.
#[cfg(not(target_arch = "powerpc"))]
pub unsafe fn get_report_descriptor(_fd: i32, _interface: u8, data: *mut c_void, size: u16) -> i32 {
    if data.is_null() || size < 8 {
        return IPC_EINVAL;
    }
    unsafe {
        core::ptr::write_bytes(data.cast::<u8>(), 0, size as usize);
    }
    USB_OK
}

/// # Safety
///
/// `data` must be null or a valid writable buffer of `size` bytes.
#[cfg(target_arch = "powerpc")]
pub unsafe fn get_report_descriptor(fd: i32, interface: u8, data: *mut c_void, size: u16) -> i32 {
    unsafe {
        if data.is_null() || size < USB_DT_MINREPORT_SIZE as u16 {
            return IPC_EINVAL;
        }
        get_generic_descriptor(fd, USB_DT_REPORT, 0, interface, data, size as u32)
    }
}

#[cfg(target_arch = "powerpc")]
fn usb_v0_data_message(
    fd: i32,
    ioctl: i32,
    endpoint: u8,
    length: u16,
    data: *mut c_void,
    flush_before_submit: bool,
) -> i32 {
    if fd < 0 || !usb_transfer_buffer_valid(data, length) {
        return IPC_EINVAL;
    }

    let endpoint_buf = AlignedU8([endpoint]);
    let length_buf = AlignedU16(length.to_be_bytes());
    let data_buf = if length == 0 {
        &mut []
    } else {
        unsafe { core::slice::from_raw_parts_mut(data.cast::<u8>(), length as usize) }
    };

    if flush_before_submit && length != 0 {
        unsafe { dc_flush_range(data.cast_const(), length as u32) };
    } else if length != 0 {
        unsafe { dc_invalidate_range(data.cast_const(), length as u32) };
    }

    FileDescriptor::new(fd).ioctlv::<3>(
        ioctl,
        2,
        [
            IoctlvEntry::In(&endpoint_buf.0),
            IoctlvEntry::In(&length_buf.0),
            IoctlvEntry::Out(data_buf),
        ],
    )
}

#[cfg(target_arch = "powerpc")]
fn write_ptr(dst: &mut [u8], offset: usize, value: *mut c_void) {
    dst[offset..offset + core::mem::size_of::<usize>()]
        .copy_from_slice(&(value as usize).to_ne_bytes());
}

#[cfg(target_arch = "powerpc")]
#[repr(C, align(32))]
struct UsbV5Message {
    header: [u8; 64],
    vectors: AlignedIoctlv2,
    callback: UsbCallback,
    userdata: *mut c_void,
    data: *mut c_void,
    transfer: *mut c_void,
    length: u16,
    read: bool,
    heap: HeapId,
}

#[cfg(target_arch = "powerpc")]
#[repr(C, align(32))]
struct UsbV5IsoMessage {
    header: [u8; 64],
    vectors: AlignedIoctlv7,
    callback: UsbCallback,
    userdata: *mut c_void,
    data: *mut c_void,
    transfer: *mut c_void,
    packet_sizes: *mut u16,
    packet_sizes_transfer: *mut u16,
    length: u16,
    packet_bytes: u32,
    read: bool,
    heap: HeapId,
}

#[cfg(target_arch = "powerpc")]
#[repr(align(32))]
struct AlignedIoctlv2([Ioctlv; 2]);

#[cfg(target_arch = "powerpc")]
unsafe fn finish_usb_v5_message(
    message: *mut UsbV5Message,
    result: i32,
    invoke_callback: bool,
    completed: bool,
) {
    unsafe {
        let msg = &mut *message;
        if completed && msg.read && msg.length != 0 {
            dc_invalidate_range(msg.transfer.cast_const(), msg.length as u32);
            if msg.transfer != msg.data {
                core::ptr::copy_nonoverlapping(
                    msg.transfer.cast::<u8>(),
                    msg.data.cast::<u8>(),
                    msg.length as usize,
                );
            }
        }
        let heap = msg.heap;
        let transfer = msg.transfer;
        let owns_transfer = transfer != msg.data;
        if invoke_callback && let Some(callback) = msg.callback {
            callback(result, msg.userdata);
        }
        if owns_transfer && let Some(buffer) = NonNull::new(transfer) {
            heap.free(buffer);
        }
        heap.free(NonNull::new_unchecked(message.cast()));
    }
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn usb_v5_message_cb(result: i32, userdata: *mut c_void) -> i32 {
    let message = userdata.cast::<UsbV5Message>();
    if message.is_null() {
        return IPC_EINVAL;
    }
    unsafe { finish_usb_v5_message(message, result, true, true) };
    USB_OK
}

#[cfg(target_arch = "powerpc")]
unsafe fn finish_usb_v5_iso_message(
    message: *mut UsbV5IsoMessage,
    result: i32,
    invoke_callback: bool,
    completed: bool,
) {
    unsafe {
        let msg = &mut *message;
        if completed && msg.read && msg.length != 0 {
            dc_invalidate_range(msg.transfer.cast_const(), msg.length as u32);
            if msg.transfer != msg.data {
                core::ptr::copy_nonoverlapping(
                    msg.transfer.cast::<u8>(),
                    msg.data.cast::<u8>(),
                    msg.length as usize,
                );
            }
        }
        if completed {
            dc_invalidate_range(
                msg.packet_sizes_transfer.cast_const().cast(),
                msg.packet_bytes,
            );
            core::ptr::copy_nonoverlapping(
                msg.packet_sizes_transfer,
                msg.packet_sizes,
                msg.packet_bytes as usize / core::mem::size_of::<u16>(),
            );
        }
        let heap = msg.heap;
        let transfer = msg.transfer;
        let packet_sizes_transfer = msg.packet_sizes_transfer;
        let owns_transfer = transfer != msg.data;
        let owns_packet_sizes = packet_sizes_transfer != msg.packet_sizes;
        if invoke_callback && let Some(callback) = msg.callback {
            callback(result, msg.userdata);
        }
        if owns_transfer && let Some(buffer) = NonNull::new(transfer) {
            heap.free(buffer);
        }
        if owns_packet_sizes && let Some(buffer) = NonNull::new(packet_sizes_transfer.cast()) {
            heap.free(buffer);
        }
        heap.free(NonNull::new_unchecked(message.cast()));
    }
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn usb_v5_iso_message_cb(result: i32, userdata: *mut c_void) -> i32 {
    let message = userdata.cast::<UsbV5IsoMessage>();
    if message.is_null() {
        return IPC_EINVAL;
    }
    unsafe { finish_usb_v5_iso_message(message, result, true, result >= 0) };
    USB_OK
}

#[cfg(target_arch = "powerpc")]
fn usb_v5_iso_message_async(request: UsbV0IsoAsyncRequest) -> i32 {
    let UsbV0IsoAsyncRequest {
        fd,
        endpoint,
        packets,
        packet_sizes,
        data,
        read,
        completion,
    } = request;
    if (0..32).contains(&fd) {
        return usb_v0_iso_message_async(UsbV0IsoAsyncRequest {
            fd,
            endpoint,
            packets,
            packet_sizes,
            data,
            read,
            completion,
        });
    }
    if packets == 0 || packet_sizes.is_null() || data.is_null() {
        return IPC_EINVAL;
    }
    let Some(host_fd) = usb_v5_device_host(fd).and_then(HostKind::fd) else {
        return IPC_ENOENT;
    };
    let packet_sizes_input = unsafe { core::slice::from_raw_parts(packet_sizes, packets as usize) };
    let Some(length) = packet_sizes_input
        .iter()
        .copied()
        .try_fold(0u16, u16::checked_add)
    else {
        return IPC_EINVAL;
    };
    if length == 0 {
        return IPC_EINVAL;
    }

    let heap = match ensure_desc_heap() {
        Ok(heap) => heap,
        Err(error) => return error,
    };
    let allocation = match alloc_zeroed(heap, core::mem::size_of::<UsbV5IsoMessage>()) {
        Ok(message) => message,
        Err(error) => return error,
    };
    let message = allocation.as_ptr().cast::<UsbV5IsoMessage>();
    let packet_bytes = packets as usize * core::mem::size_of::<u16>();
    let packet_sizes_transfer = match alloc_zeroed(heap, packet_bytes) {
        Ok(buffer) => buffer.as_ptr().cast::<u16>(),
        Err(error) => {
            unsafe { heap.free(allocation) };
            return error;
        }
    };
    let transfer = if v5_layout::needs_bounce(data as usize, length as usize) {
        match alloc_zeroed(heap, length as usize) {
            Ok(buffer) => buffer.as_ptr(),
            Err(error) => {
                unsafe {
                    heap.free(NonNull::new_unchecked(packet_sizes_transfer.cast()));
                    heap.free(allocation);
                }
                return error;
            }
        }
    } else {
        data
    };
    unsafe {
        core::ptr::copy_nonoverlapping(packet_sizes, packet_sizes_transfer, packets as usize);
        if !read && transfer != data {
            core::ptr::copy_nonoverlapping(
                data.cast::<u8>(),
                transfer.cast::<u8>(),
                length as usize,
            );
        }
        (*message).callback = completion.callback;
        (*message).userdata = completion.userdata;
        (*message).data = data;
        (*message).transfer = transfer;
        (*message).packet_sizes = packet_sizes;
        (*message).packet_sizes_transfer = packet_sizes_transfer;
        (*message).length = length;
        (*message).packet_bytes = packet_bytes as u32;
        (*message).read = read;
        (*message).heap = heap;
        (*message).header = v5_layout::iso_header(
            fd,
            endpoint,
            packets,
            packet_sizes_transfer as usize,
            transfer as usize,
        );
        dc_flush_range(
            packet_sizes_transfer.cast_const().cast(),
            packet_bytes as u32,
        );
        if length != 0 {
            if read {
                dc_invalidate_range(transfer.cast_const(), length as u32);
            } else {
                dc_flush_range(transfer.cast_const(), length as u32);
            }
        }
        let vectors = &mut (*message).vectors.0;
        vectors[0] = Ioctlv {
            data: core::ptr::addr_of_mut!((*message).header).cast(),
            len: 64,
        };
        vectors[1] = Ioctlv {
            data: packet_sizes_transfer.cast(),
            len: packet_bytes as u32,
        };
        vectors[2] = Ioctlv {
            data: transfer,
            len: length as u32,
        };
        vectors[3] = vectors[1];
        let direction_in = endpoint & USB_ENDPOINT_IN != 0;
        let (inputs, outputs) = v5_layout::iso_vector_counts(direction_in);
        let args = core::ptr::addr_of_mut!((*message).vectors.0).cast::<Ioctlv>();
        let result = if completion.callback.is_some() {
            FileDescriptor::new(host_fd).ioctlv_async_raw(
                USBV5_IOCTL_ISOMSG,
                inputs,
                outputs,
                args,
                Some(usb_v5_iso_message_cb),
                message.cast(),
            )
        } else {
            let result = FileDescriptor::new(host_fd).ioctlv_raw(
                USBV5_IOCTL_ISOMSG,
                inputs,
                outputs,
                &mut (*message).vectors.0,
            );
            finish_usb_v5_iso_message(message, result, false, result >= 0);
            return result;
        };
        if result < 0 {
            finish_usb_v5_iso_message(message, result, false, false);
        }
        result
    }
}

/// Shared V5 transport. Headers, vectors and any bounce buffer stay in the
/// USB IOS heap until the final reply. Partial cache lines and MEM1 caller
/// buffers are copied through an exclusively owned MEM2 allocation.
#[cfg(target_arch = "powerpc")]
struct UsbTransferBuffer {
    data: *mut c_void,
    length: u16,
}

#[cfg(target_arch = "powerpc")]
fn usb_v5_message(
    fd: i32,
    ioctl: i32,
    header: [u8; 64],
    pointer_offset: Option<usize>,
    buffer: UsbTransferBuffer,
    read: bool,
    completion: UsbAsyncCompletion,
) -> i32 {
    let UsbTransferBuffer { data, length } = buffer;
    let Some(host_fd) = usb_v5_device_host(fd).and_then(HostKind::fd) else {
        return IPC_ENOENT;
    };
    if !usb_transfer_buffer_valid(data, length) {
        return IPC_EINVAL;
    }
    let heap = match ensure_desc_heap() {
        Ok(heap) => heap,
        Err(error) => return error,
    };
    let allocation = match alloc_zeroed(heap, core::mem::size_of::<UsbV5Message>()) {
        Ok(message) => message,
        Err(error) => return error,
    };
    let message = allocation.as_ptr().cast::<UsbV5Message>();
    let transfer = if v5_layout::needs_bounce(data as usize, length as usize) {
        match alloc_zeroed(heap, length as usize) {
            Ok(buffer) => buffer.as_ptr(),
            Err(error) => {
                unsafe { heap.free(allocation) };
                return error;
            }
        }
    } else {
        data
    };
    unsafe {
        if length != 0 && transfer != data {
            core::ptr::copy_nonoverlapping(
                data.cast::<u8>(),
                transfer.cast::<u8>(),
                length as usize,
            );
        }
        (*message).header = header;
        if let Some(offset) = pointer_offset {
            write_ptr(&mut (*message).header, offset, transfer);
        }
        (*message).callback = completion.callback;
        (*message).userdata = completion.userdata;
        (*message).data = data;
        (*message).transfer = transfer;
        (*message).length = length;
        (*message).read = read;
        (*message).heap = heap;
        (*message).vectors.0 = [
            Ioctlv {
                data: core::ptr::addr_of_mut!((*message).header).cast(),
                len: 64,
            },
            Ioctlv {
                data: transfer,
                len: length as u32,
            },
        ];
        let inputs = if read { 1 } else { 2 };
        if completion.callback.is_some() {
            let result = FileDescriptor::new(host_fd).ioctlv_async_raw(
                ioctl,
                inputs,
                2 - inputs,
                core::ptr::addr_of_mut!((*message).vectors.0).cast(),
                Some(usb_v5_message_cb),
                message.cast(),
            );
            if result < 0 {
                finish_usb_v5_message(message, result, false, false);
            }
            result
        } else {
            let result = FileDescriptor::new(host_fd).ioctlv_raw(
                ioctl,
                inputs,
                2 - inputs,
                &mut (*message).vectors.0,
            );
            finish_usb_v5_message(message, result, false, true);
            result
        }
    }
}

#[cfg(target_arch = "powerpc")]
fn usb_v5_data_message(fd: i32, endpoint: u8, length: u16, data: *mut c_void, read: bool) -> i32 {
    usb_v5_data_message_async(
        fd,
        endpoint,
        length,
        data,
        read,
        UsbAsyncCompletion {
            callback: None,
            userdata: core::ptr::null_mut(),
        },
    )
}

#[cfg(target_arch = "powerpc")]
fn usb_v5_data_message_async(
    fd: i32,
    endpoint: u8,
    length: u16,
    data: *mut c_void,
    read: bool,
    completion: UsbAsyncCompletion,
) -> i32 {
    let header = v5_layout::bulk_header(fd, endpoint, length);
    usb_v5_message(
        fd,
        USBV5_IOCTL_BULKMSG,
        header,
        Some(8),
        UsbTransferBuffer { data, length },
        read,
        completion,
    )
}

#[cfg(target_arch = "powerpc")]
fn usb_v5_interrupt_message(
    fd: i32,
    endpoint: u8,
    length: u16,
    data: *mut c_void,
    completion: UsbAsyncCompletion,
) -> i32 {
    let header = v5_layout::interrupt_header(fd, endpoint, length, data as usize as u32);
    let pointer_offset = (v5_layout::host_for_device(fd) == Some(HostKind::Vendor)).then_some(8);
    usb_v5_message(
        fd,
        USBV5_IOCTL_INTRMSG,
        header,
        pointer_offset,
        UsbTransferBuffer { data, length },
        endpoint & USB_ENDPOINT_IN != 0,
        completion,
    )
}

#[cfg(target_arch = "powerpc")]
fn usb_v5_control_message(
    fd: i32,
    request_type: u8,
    request: u8,
    value: u16,
    index: u16,
    length: u16,
    data: *mut c_void,
) -> i32 {
    usb_v5_control_message_async(
        UsbControlRequest {
            fd,
            request_type,
            request,
            value,
            index,
            length,
            data,
        },
        UsbAsyncCompletion {
            callback: None,
            userdata: core::ptr::null_mut(),
        },
    )
}

#[cfg(target_arch = "powerpc")]
fn usb_v5_control_message_async(request: UsbControlRequest, completion: UsbAsyncCompletion) -> i32 {
    let header = v5_layout::control_header(
        request.fd,
        request.request_type,
        request.request,
        request.value,
        request.index,
        request.length,
    );
    usb_v5_message(
        request.fd,
        USBV5_IOCTL_CTRLMSG,
        header,
        Some(16),
        UsbTransferBuffer {
            data: request.data,
            length: request.length,
        },
        request.request_type & USB_CTRLTYPE_DIR_DEVICE2HOST != 0,
        completion,
    )
}

#[cfg(target_arch = "powerpc")]
fn usb_v0_iso_message_async(request: UsbV0IsoAsyncRequest) -> i32 {
    let UsbV0IsoAsyncRequest {
        fd,
        endpoint,
        packets,
        packet_sizes,
        data,
        read,
        completion,
    } = request;
    let UsbAsyncCompletion { callback, userdata } = completion;
    if callback.is_none() {
        return usb_v0_iso_message(fd, endpoint, packets, packet_sizes, data, !read);
    }
    if fd < 0
        || packets == 0
        || packet_sizes.is_null()
        || data.is_null()
        || !usb_aligned(packet_sizes as *const c_void)
        || !usb_aligned(data as *const c_void)
    {
        return IPC_EINVAL;
    }

    let packet_sizes_buf = unsafe { core::slice::from_raw_parts(packet_sizes, packets as usize) };
    let Some(length) = packet_sizes_buf
        .iter()
        .copied()
        .try_fold(0u16, u16::checked_add)
    else {
        return IPC_EINVAL;
    };
    if length == 0 {
        return IPC_EINVAL;
    }

    let heap = match ensure_desc_heap() {
        Ok(heap) => heap,
        Err(rc) => return rc,
    };
    let msg_ptr = match alloc_zeroed(heap, core::mem::size_of::<UsbV0IsoAsyncMessage>()) {
        Ok(ptr) => ptr.as_ptr().cast::<UsbV0IsoAsyncMessage>(),
        Err(rc) => return rc,
    };

    unsafe {
        (*msg_ptr).endpoint.0[0] = endpoint;
        (*msg_ptr).length.0 = length.to_ne_bytes();
        (*msg_ptr).packets.0[0] = packets;
        (*msg_ptr).cb = callback;
        (*msg_ptr).userdata = userdata;
        (*msg_ptr).packet_sizes = packet_sizes;
        (*msg_ptr).data = data;
        (*msg_ptr).packet_bytes = packets as u32 * core::mem::size_of::<u16>() as u32;
        (*msg_ptr).length_value = length;
        (*msg_ptr).read = read;

        let endpoint_ptr = core::ptr::addr_of_mut!((*msg_ptr).endpoint.0).cast::<u8>();
        let length_ptr = core::ptr::addr_of_mut!((*msg_ptr).length.0).cast::<u8>();
        let packets_ptr = core::ptr::addr_of_mut!((*msg_ptr).packets.0).cast::<u8>();

        (*msg_ptr).argv.0[0] = Ioctlv {
            data: endpoint_ptr.cast(),
            len: 1,
        };
        (*msg_ptr).argv.0[1] = Ioctlv {
            data: length_ptr.cast(),
            len: 2,
        };
        (*msg_ptr).argv.0[2] = Ioctlv {
            data: packets_ptr.cast(),
            len: 1,
        };
        (*msg_ptr).argv.0[3] = Ioctlv {
            data: packet_sizes.cast::<c_void>(),
            len: (*msg_ptr).packet_bytes,
        };
        (*msg_ptr).argv.0[4] = Ioctlv {
            data,
            len: length as u32,
        };
    }

    let rc = unsafe {
        FileDescriptor::new(fd).ioctlv_async_raw(
            USBV0_IOCTL_ISOMSG,
            3,
            2,
            core::ptr::addr_of_mut!((*msg_ptr).argv.0).cast::<Ioctlv>(),
            Some(usb_v0_iso_async_cb),
            msg_ptr.cast(),
        )
    };
    if rc < 0 {
        unsafe {
            free_heap_message(msg_ptr);
        }
    }
    rc
}

#[cfg(target_arch = "powerpc")]
fn usb_v0_data_message_async(request: UsbV0DataAsyncRequest) -> i32 {
    let UsbV0DataAsyncRequest {
        fd,
        ioctl,
        endpoint,
        length,
        data,
        read,
        completion,
    } = request;
    let UsbAsyncCompletion { callback, userdata } = completion;
    if callback.is_none() {
        return usb_v0_data_message(fd, ioctl, endpoint, length, data, !read);
    }
    if fd < 0 || !usb_transfer_buffer_valid(data, length) {
        return IPC_EINVAL;
    }

    let heap = match ensure_desc_heap() {
        Ok(heap) => heap,
        Err(rc) => return rc,
    };
    let msg_ptr = match alloc_zeroed(heap, core::mem::size_of::<UsbV0DataAsyncMessage>()) {
        Ok(ptr) => ptr.as_ptr().cast::<UsbV0DataAsyncMessage>(),
        Err(rc) => return rc,
    };

    unsafe {
        (*msg_ptr).endpoint.0[0] = endpoint;
        (*msg_ptr).length.0 = length.to_be_bytes();
        (*msg_ptr).cb = callback;
        (*msg_ptr).userdata = userdata;
        (*msg_ptr).data = data;
        (*msg_ptr).length_value = length;
        (*msg_ptr).read = read;

        let endpoint_ptr = core::ptr::addr_of_mut!((*msg_ptr).endpoint.0).cast::<u8>();
        let length_ptr = core::ptr::addr_of_mut!((*msg_ptr).length.0).cast::<u8>();
        (*msg_ptr).argv.0[0] = Ioctlv {
            data: endpoint_ptr.cast(),
            len: 1,
        };
        (*msg_ptr).argv.0[1] = Ioctlv {
            data: length_ptr.cast(),
            len: 2,
        };
        (*msg_ptr).argv.0[2] = Ioctlv {
            data,
            len: length as u32,
        };
    }

    let rc = unsafe {
        FileDescriptor::new(fd).ioctlv_async_raw(
            ioctl,
            2,
            1,
            core::ptr::addr_of_mut!((*msg_ptr).argv.0).cast::<Ioctlv>(),
            Some(usb_v0_data_async_cb),
            msg_ptr.cast(),
        )
    };
    if rc < 0 {
        unsafe {
            free_heap_message(msg_ptr);
        }
    }
    rc
}

#[cfg(target_arch = "powerpc")]
fn usb_v0_iso_message(
    fd: i32,
    endpoint: u8,
    packets: u8,
    packet_sizes: *mut u16,
    data: *mut c_void,
    flush_data: bool,
) -> i32 {
    if fd < 0
        || packets == 0
        || packet_sizes.is_null()
        || data.is_null()
        || !usb_aligned(packet_sizes as *const c_void)
        || !usb_aligned(data as *const c_void)
    {
        return IPC_EINVAL;
    }

    let packet_sizes_buf =
        unsafe { core::slice::from_raw_parts_mut(packet_sizes, packets as usize) };
    let Some(length) = packet_sizes_buf
        .iter()
        .copied()
        .try_fold(0u16, u16::checked_add)
    else {
        return IPC_EINVAL;
    };
    if length == 0 {
        return IPC_EINVAL;
    }

    let endpoint_buf = AlignedU8([endpoint]);
    let length_buf = AlignedU16(length.to_ne_bytes());
    let packets_buf = AlignedU8([packets]);
    let data_buf = unsafe { core::slice::from_raw_parts_mut(data.cast::<u8>(), length as usize) };

    if flush_data {
        unsafe { dc_flush_range(data.cast_const(), length as u32) };
    } else {
        unsafe { dc_invalidate_range(data.cast_const(), length as u32) };
    }

    FileDescriptor::new(fd).ioctlv::<5>(
        USBV0_IOCTL_ISOMSG,
        3,
        [
            IoctlvEntry::In(&endpoint_buf.0),
            IoctlvEntry::In(&length_buf.0),
            IoctlvEntry::In(&packets_buf.0),
            IoctlvEntry::Out(unsafe {
                core::slice::from_raw_parts_mut(packet_sizes.cast::<u8>(), packets as usize * 2)
            }),
            IoctlvEntry::Out(data_buf),
        ],
    )
}

/// # Safety
///
/// `userdata` is passed to `cb` and must satisfy that callback's lifetime and
/// aliasing requirements.
#[cfg(not(target_arch = "powerpc"))]
pub unsafe fn device_removal_notify_async(
    _fd: i32,
    _cb: UsbCallback,
    _userdata: *mut c_void,
) -> i32 {
    USB_OK
}

/// # Safety
///
/// After successful registration, `cb` and `userdata` must remain valid until
/// one callback reports device removal, close, watcher failure, or USB
/// deinitialization. The callback is one-shot and can register a replacement.
#[cfg(target_arch = "powerpc")]
pub unsafe fn device_removal_notify_async(fd: i32, cb: UsbCallback, userdata: *mut c_void) -> i32 {
    if !is_v0_fd(fd) {
        let Some(host) = usb_v5_device_host(fd) else {
            return IPC_ENOENT;
        };
        return match with_usb_v5_host_events(host, |state| state.register_removal(fd, cb, userdata))
        {
            Ok(result) => usb_v5_registration_result(result, IPC_EINVAL),
            Err(error) => error,
        };
    }
    unsafe {
        FileDescriptor::new(fd).ioctl_async(
            USBV0_IOCTL_DEVREMOVALHOOK,
            IoctlBuffers::empty(),
            cb,
            userdata,
        )
    }
}

/// # Safety
///
/// After successful v5 registration, `cb` and `userdata` must remain valid
/// until the one-shot change callback runs, the watcher fails, or USB is
/// deinitialized.
#[cfg(target_arch = "powerpc")]
pub unsafe fn device_change_notify_async(
    interface_class: u8,
    cb: UsbCallback,
    userdata: *mut c_void,
) -> i32 {
    let host = if interface_class == USB_CLASS_HID {
        HostKind::Hid
    } else {
        HostKind::Vendor
    };
    if host.detected().load(Ordering::Acquire) {
        return match with_usb_v5_host_events(host, |state| state.register_change(cb, userdata)) {
            Ok(result) => usb_v5_registration_result(result, IPC_ENOMEM),
            Err(error) => error,
        };
    }
    if !v5_layout::use_legacy_list(
        HostKind::Vendor.detected().load(Ordering::Acquire),
        HostKind::Hid.detected().load(Ordering::Acquire),
    ) {
        return IPC_ENOENT;
    }

    let heap = match ensure_desc_heap() {
        Ok(heap) => heap,
        Err(rc) => return rc,
    };
    let msg_ptr = match alloc_zeroed(heap, core::mem::size_of::<UsbClassChangeMessage>()) {
        Ok(ptr) => ptr.as_ptr().cast::<UsbClassChangeMessage>(),
        Err(rc) => return rc,
    };

    unsafe {
        (*msg_ptr).class = interface_class;
        (*msg_ptr).cb = cb;
        (*msg_ptr).userdata = userdata;
    }

    if let Err(error) = begin_transient_operation() {
        unsafe {
            free_heap_message(msg_ptr);
        }
        return error;
    }
    let fd = match open_file(oh0_root_path(), IPC_OPEN_NONE) {
        Ok(fd) => fd,
        Err(rc) => {
            USB_TRANSIENT_FD.store(-1, Ordering::Release);
            unsafe {
                free_heap_message(msg_ptr);
            }
            return rc;
        }
    };

    let input =
        unsafe { core::slice::from_raw_parts_mut(core::ptr::addr_of_mut!((*msg_ptr).class), 1) };
    let rc = unsafe {
        fd.ioctl_async(
            USBV0_IOCTL_DEVICECLASSCHANGE,
            IoctlBuffers::from_slices(Some(input), None),
            Some(usb_class_change_cb),
            msg_ptr.cast(),
        )
    };
    let close_rc = finish_transient_operation(fd.raw());

    if rc < 0 {
        unsafe {
            free_heap_message(msg_ptr);
        }
        return rc;
    }
    if close_rc < 0 {
        return close_rc;
    }
    rc
}

#[cfg(not(target_arch = "powerpc"))]
pub fn suspend_device(_fd: i32) -> i32 {
    USB_OK
}

#[cfg(target_arch = "powerpc")]
pub fn suspend_device(fd: i32) -> i32 {
    if !is_v0_fd(fd) {
        return usb_v5_suspend_resume(fd, false);
    }
    FileDescriptor::new(fd).ioctl(USBV0_IOCTL_SUSPENDDEV, IoctlBuffers::empty())
}

#[cfg(not(target_arch = "powerpc"))]
pub fn resume_device(_fd: i32) -> i32 {
    USB_OK
}

#[cfg(target_arch = "powerpc")]
pub fn resume_device(fd: i32) -> i32 {
    if !is_v0_fd(fd) {
        return usb_v5_suspend_resume(fd, true);
    }
    FileDescriptor::new(fd).ioctl(USBV0_IOCTL_RESUMEDEV, IoctlBuffers::empty())
}

/// # Safety
///
/// The packet-size and data pointers must be valid for the underlying USB
/// transfer contract if a real backend is substituted.
#[cfg(not(target_arch = "powerpc"))]
pub unsafe fn read_iso_msg(
    _fd: i32,
    _b_endpoint: u8,
    _b_packets: u8,
    rp_packet_sizes: *mut u16,
    rp_data: *mut c_void,
) -> i32 {
    if rp_packet_sizes.is_null() || rp_data.is_null() {
        USB_FAILED
    } else {
        USB_OK
    }
}

/// # Safety
///
/// The packet-size and data pointers must be valid for the underlying USB
/// transfer contract.
#[cfg(target_arch = "powerpc")]
pub unsafe fn read_iso_msg(
    fd: i32,
    b_endpoint: u8,
    b_packets: u8,
    rp_packet_sizes: *mut u16,
    rp_data: *mut c_void,
) -> i32 {
    usb_v5_iso_message_async(UsbV0IsoAsyncRequest {
        fd,
        endpoint: b_endpoint,
        packets: b_packets,
        packet_sizes: rp_packet_sizes,
        data: rp_data,
        read: true,
        completion: UsbAsyncCompletion {
            callback: None,
            userdata: core::ptr::null_mut(),
        },
    })
}

/// # Safety
///
/// The packet-size and data pointers must be valid for the underlying USB
/// transfer contract if a real backend is substituted.
#[cfg(not(target_arch = "powerpc"))]
pub unsafe fn read_iso_msg_async(
    _fd: i32,
    _b_endpoint: u8,
    _b_packets: u8,
    _rp_packet_sizes: *mut u16,
    _rp_data: *mut c_void,
    cb: UsbCallback,
    userdata: *mut c_void,
) -> i32 {
    if let Some(cb) = cb {
        unsafe {
            cb(USB_OK, userdata);
        }
    }
    USB_OK
}

/// # Safety
///
/// The packet-size and data pointers must be valid for the underlying USB
/// transfer contract.
#[cfg(target_arch = "powerpc")]
pub unsafe fn read_iso_msg_async(
    fd: i32,
    b_endpoint: u8,
    b_packets: u8,
    rp_packet_sizes: *mut u16,
    rp_data: *mut c_void,
    cb: UsbCallback,
    userdata: *mut c_void,
) -> i32 {
    usb_v5_iso_message_async(UsbV0IsoAsyncRequest {
        fd,
        endpoint: b_endpoint,
        packets: b_packets,
        packet_sizes: rp_packet_sizes,
        data: rp_data,
        read: true,
        completion: UsbAsyncCompletion {
            callback: cb,
            userdata,
        },
    })
}

/// # Safety
///
/// `rp_data` must be a valid writable buffer for the requested transfer.
#[cfg(not(target_arch = "powerpc"))]
pub unsafe fn read_intr_msg(_fd: i32, _b_endpoint: u8, w_length: u16, rp_data: *mut c_void) -> i32 {
    if rp_data.is_null() && w_length != 0 {
        USB_FAILED
    } else {
        USB_OK
    }
}

/// # Safety
///
/// `rp_data` must be a valid writable buffer for the requested transfer.
#[cfg(target_arch = "powerpc")]
pub unsafe fn read_intr_msg(fd: i32, b_endpoint: u8, w_length: u16, rp_data: *mut c_void) -> i32 {
    if !is_v0_fd(fd) {
        return usb_v5_interrupt_message(
            fd,
            b_endpoint,
            w_length,
            rp_data,
            UsbAsyncCompletion {
                callback: None,
                userdata: core::ptr::null_mut(),
            },
        );
    }

    usb_v0_data_message(
        fd,
        USBV0_IOCTL_INTRMSG,
        b_endpoint,
        w_length,
        rp_data,
        false,
    )
}

/// # Safety
///
/// `rp_data` must be a valid writable buffer for the requested transfer.
#[cfg(not(target_arch = "powerpc"))]
pub unsafe fn read_intr_msg_async(
    _fd: i32,
    _b_endpoint: u8,
    _w_length: u16,
    _rp_data: *mut c_void,
    cb: UsbCallback,
    userdata: *mut c_void,
) -> i32 {
    if let Some(cb) = cb {
        unsafe {
            cb(USB_OK, userdata);
        }
    }
    USB_OK
}

/// # Safety
///
/// `rp_data` must be a valid writable buffer for the requested transfer.
#[cfg(target_arch = "powerpc")]
pub unsafe fn read_intr_msg_async(
    fd: i32,
    b_endpoint: u8,
    w_length: u16,
    rp_data: *mut c_void,
    cb: UsbCallback,
    userdata: *mut c_void,
) -> i32 {
    if !is_v0_fd(fd) {
        return usb_v5_interrupt_message(
            fd,
            b_endpoint,
            w_length,
            rp_data,
            UsbAsyncCompletion {
                callback: cb,
                userdata,
            },
        );
    }

    usb_v0_data_message_async(UsbV0DataAsyncRequest {
        fd,
        ioctl: USBV0_IOCTL_INTRMSG,
        endpoint: b_endpoint,
        length: w_length,
        data: rp_data,
        read: true,
        completion: UsbAsyncCompletion {
            callback: cb,
            userdata,
        },
    })
}

/// # Safety
///
/// `rp_data` must be a valid writable buffer for the requested transfer.
#[cfg(not(target_arch = "powerpc"))]
pub unsafe fn read_blk_msg(_fd: i32, _b_endpoint: u8, w_length: u16, rp_data: *mut c_void) -> i32 {
    if rp_data.is_null() && w_length != 0 {
        USB_FAILED
    } else {
        USB_OK
    }
}

/// # Safety
///
/// `rp_data` must be a valid writable buffer for the requested transfer.
#[cfg(target_arch = "powerpc")]
pub unsafe fn read_blk_msg(fd: i32, b_endpoint: u8, w_length: u16, rp_data: *mut c_void) -> i32 {
    if !is_v0_fd(fd) {
        return usb_v5_data_message(fd, b_endpoint, w_length, rp_data, true);
    }
    usb_v0_data_message(fd, USBV0_IOCTL_BLKMSG, b_endpoint, w_length, rp_data, false)
}

/// # Safety
///
/// `rp_data` must be a valid writable buffer for the requested transfer.
#[cfg(not(target_arch = "powerpc"))]
pub unsafe fn read_blk_msg_async(
    _fd: i32,
    _b_endpoint: u8,
    _w_length: u16,
    _rp_data: *mut c_void,
    cb: UsbCallback,
    userdata: *mut c_void,
) -> i32 {
    if let Some(cb) = cb {
        unsafe {
            cb(USB_OK, userdata);
        }
    }
    USB_OK
}

/// # Safety
///
/// `rp_data` must be a valid writable buffer for the requested transfer.
#[cfg(target_arch = "powerpc")]
pub unsafe fn read_blk_msg_async(
    fd: i32,
    b_endpoint: u8,
    w_length: u16,
    rp_data: *mut c_void,
    cb: UsbCallback,
    userdata: *mut c_void,
) -> i32 {
    if !is_v0_fd(fd) {
        return usb_v5_data_message_async(
            fd,
            b_endpoint,
            w_length,
            rp_data,
            true,
            UsbAsyncCompletion {
                callback: cb,
                userdata,
            },
        );
    }
    usb_v0_data_message_async(UsbV0DataAsyncRequest {
        fd,
        ioctl: USBV0_IOCTL_BLKMSG,
        endpoint: b_endpoint,
        length: w_length,
        data: rp_data,
        read: true,
        completion: UsbAsyncCompletion {
            callback: cb,
            userdata,
        },
    })
}

/// # Safety
///
/// `rp_data` must be a valid writable buffer for the requested transfer.
#[cfg(not(target_arch = "powerpc"))]
pub unsafe fn read_ctrl_msg(
    _fd: i32,
    _bm_request_type: u8,
    _bm_request: u8,
    _w_value: u16,
    _w_index: u16,
    w_length: u16,
    rp_data: *mut c_void,
) -> i32 {
    if rp_data.is_null() && w_length != 0 {
        USB_FAILED
    } else {
        USB_OK
    }
}

/// # Safety
///
/// `rp_data` must be a valid writable buffer for the requested transfer.
#[cfg(target_arch = "powerpc")]
pub unsafe fn read_ctrl_msg(
    fd: i32,
    bm_request_type: u8,
    bm_request: u8,
    w_value: u16,
    w_index: u16,
    w_length: u16,
    rp_data: *mut c_void,
) -> i32 {
    if !is_v0_fd(fd) {
        return usb_v5_control_message(
            fd,
            bm_request_type,
            bm_request,
            w_value,
            w_index,
            w_length,
            rp_data,
        );
    }
    usb_control_message(
        fd,
        bm_request_type,
        bm_request,
        w_value,
        w_index,
        w_length,
        rp_data,
    )
}

#[cfg(target_arch = "powerpc")]
fn usb_control_message(
    fd: i32,
    bm_request_type: u8,
    bm_request: u8,
    w_value: u16,
    w_index: u16,
    w_length: u16,
    data: *mut c_void,
) -> i32 {
    if fd < 0 || !usb_transfer_buffer_valid(data, w_length) {
        return IPC_EINVAL;
    }

    let rq_type = AlignedU8([bm_request_type]);
    let rq = AlignedU8([bm_request]);
    let value = AlignedU16(w_value.to_le_bytes());
    let index = AlignedU16(w_index.to_le_bytes());
    let length = AlignedU16(w_length.to_le_bytes());
    let null = AlignedU8([0]);

    let data_buf = if w_length == 0 {
        &mut []
    } else {
        unsafe { core::slice::from_raw_parts_mut(data.cast::<u8>(), w_length as usize) }
    };

    FileDescriptor::new(fd).ioctlv::<7>(
        USBV0_IOCTL_CTRLMSG,
        6,
        [
            IoctlvEntry::In(&rq_type.0),
            IoctlvEntry::In(&rq.0),
            IoctlvEntry::In(&value.0),
            IoctlvEntry::In(&index.0),
            IoctlvEntry::In(&length.0),
            IoctlvEntry::In(&null.0),
            IoctlvEntry::Out(data_buf),
        ],
    )
}

/// # Safety
///
/// If `request.length` is zero, `request.data` must be null. Otherwise it must point to a writable,
/// 32-byte-aligned buffer of at least `request.length` bytes that remains valid until completion.
/// If present, the callback must be safe to invoke asynchronously with `completion.userdata`, and
/// the userdata it accesses must remain valid until then.
#[cfg(not(target_arch = "powerpc"))]
pub unsafe fn read_ctrl_msg_async(
    _request: UsbControlRequest,
    completion: UsbAsyncCompletion,
) -> i32 {
    if let Some(callback) = completion.callback {
        unsafe {
            callback(USB_OK, completion.userdata);
        }
    }
    USB_OK
}

#[cfg(target_arch = "powerpc")]
fn usb_v0_control_message_async(
    request: UsbControlRequest,
    read: bool,
    completion: UsbAsyncCompletion,
) -> i32 {
    let UsbControlRequest {
        fd,
        request_type,
        request,
        value,
        index,
        length,
        data,
    } = request;
    if completion.callback.is_none() {
        return usb_control_message(fd, request_type, request, value, index, length, data);
    }
    if fd < 0 || !usb_transfer_buffer_valid(data, length) {
        return IPC_EINVAL;
    }

    let heap = match ensure_desc_heap() {
        Ok(heap) => heap,
        Err(rc) => return rc,
    };
    let msg_ptr = match alloc_zeroed(heap, core::mem::size_of::<UsbV0ControlAsyncMessage>()) {
        Ok(ptr) => ptr.as_ptr().cast::<UsbV0ControlAsyncMessage>(),
        Err(rc) => return rc,
    };

    unsafe {
        (*msg_ptr).rq_type.0[0] = request_type;
        (*msg_ptr).rq.0[0] = request;
        (*msg_ptr).value.0 = value.to_le_bytes();
        (*msg_ptr).index.0 = index.to_le_bytes();
        (*msg_ptr).length.0 = length.to_le_bytes();
        (*msg_ptr).null.0[0] = 0;
        (*msg_ptr).cb = completion.callback;
        (*msg_ptr).userdata = completion.userdata;
        (*msg_ptr).data = data;
        (*msg_ptr).length_value = length;
        (*msg_ptr).read = read;

        let rq_type_ptr = core::ptr::addr_of_mut!((*msg_ptr).rq_type.0).cast::<u8>();
        let rq_ptr = core::ptr::addr_of_mut!((*msg_ptr).rq.0).cast::<u8>();
        let value_ptr = core::ptr::addr_of_mut!((*msg_ptr).value.0).cast::<u8>();
        let index_ptr = core::ptr::addr_of_mut!((*msg_ptr).index.0).cast::<u8>();
        let length_ptr = core::ptr::addr_of_mut!((*msg_ptr).length.0).cast::<u8>();
        let null_ptr = core::ptr::addr_of_mut!((*msg_ptr).null.0).cast::<u8>();

        (*msg_ptr).argv.0[0] = Ioctlv {
            data: rq_type_ptr.cast(),
            len: 1,
        };
        (*msg_ptr).argv.0[1] = Ioctlv {
            data: rq_ptr.cast(),
            len: 1,
        };
        (*msg_ptr).argv.0[2] = Ioctlv {
            data: value_ptr.cast(),
            len: 2,
        };
        (*msg_ptr).argv.0[3] = Ioctlv {
            data: index_ptr.cast(),
            len: 2,
        };
        (*msg_ptr).argv.0[4] = Ioctlv {
            data: length_ptr.cast(),
            len: 2,
        };
        (*msg_ptr).argv.0[5] = Ioctlv {
            data: null_ptr.cast(),
            len: 1,
        };
        (*msg_ptr).argv.0[6] = Ioctlv {
            data,
            len: length as u32,
        };
    }

    let rc = unsafe {
        FileDescriptor::new(fd).ioctlv_async_raw(
            USBV0_IOCTL_CTRLMSG,
            6,
            1,
            core::ptr::addr_of_mut!((*msg_ptr).argv.0).cast::<Ioctlv>(),
            Some(usb_v0_control_async_cb),
            msg_ptr.cast(),
        )
    };
    if rc < 0 {
        unsafe {
            free_heap_message(msg_ptr);
        }
    }
    rc
}

/// # Safety
///
/// If `request.length` is zero, `request.data` must be null. Otherwise it must point to a writable,
/// 32-byte-aligned buffer of at least `request.length` bytes that remains valid until completion.
/// If present, the callback must be safe to invoke asynchronously with `completion.userdata`, and
/// the userdata it accesses must remain valid until then.
#[cfg(target_arch = "powerpc")]
pub unsafe fn read_ctrl_msg_async(
    request: UsbControlRequest,
    completion: UsbAsyncCompletion,
) -> i32 {
    if !is_v0_fd(request.fd) {
        return usb_v5_control_message_async(request, completion);
    }
    usb_v0_control_message_async(request, true, completion)
}

/// # Safety
///
/// The packet-size and data pointers must be valid for the underlying USB
/// transfer contract if a real backend is substituted.
#[cfg(not(target_arch = "powerpc"))]
pub unsafe fn write_iso_msg(
    _fd: i32,
    _b_endpoint: u8,
    _b_packets: u8,
    rp_packet_sizes: *mut u16,
    rp_data: *mut c_void,
) -> i32 {
    if rp_packet_sizes.is_null() || rp_data.is_null() {
        USB_FAILED
    } else {
        USB_OK
    }
}

/// # Safety
///
/// The packet-size and data pointers must be valid for the underlying USB
/// transfer contract.
#[cfg(target_arch = "powerpc")]
pub unsafe fn write_iso_msg(
    fd: i32,
    b_endpoint: u8,
    b_packets: u8,
    rp_packet_sizes: *mut u16,
    rp_data: *mut c_void,
) -> i32 {
    usb_v5_iso_message_async(UsbV0IsoAsyncRequest {
        fd,
        endpoint: b_endpoint,
        packets: b_packets,
        packet_sizes: rp_packet_sizes,
        data: rp_data,
        read: false,
        completion: UsbAsyncCompletion {
            callback: None,
            userdata: core::ptr::null_mut(),
        },
    })
}

/// # Safety
///
/// The packet-size and data pointers must be valid for the underlying USB
/// transfer contract if a real backend is substituted.
#[cfg(not(target_arch = "powerpc"))]
pub unsafe fn write_iso_msg_async(
    _fd: i32,
    _b_endpoint: u8,
    _b_packets: u8,
    _rp_packet_sizes: *mut u16,
    _rp_data: *mut c_void,
    cb: UsbCallback,
    userdata: *mut c_void,
) -> i32 {
    if let Some(cb) = cb {
        unsafe {
            cb(USB_OK, userdata);
        }
    }
    USB_OK
}

/// # Safety
///
/// The packet-size and data pointers must be valid for the underlying USB
/// transfer contract.
#[cfg(target_arch = "powerpc")]
pub unsafe fn write_iso_msg_async(
    fd: i32,
    b_endpoint: u8,
    b_packets: u8,
    rp_packet_sizes: *mut u16,
    rp_data: *mut c_void,
    cb: UsbCallback,
    userdata: *mut c_void,
) -> i32 {
    usb_v5_iso_message_async(UsbV0IsoAsyncRequest {
        fd,
        endpoint: b_endpoint,
        packets: b_packets,
        packet_sizes: rp_packet_sizes,
        data: rp_data,
        read: false,
        completion: UsbAsyncCompletion {
            callback: cb,
            userdata,
        },
    })
}

/// # Safety
///
/// `rp_data` must point to a readable buffer for the requested transfer. On
/// PowerPC, it must be 32-byte aligned for a nonzero transfer.
#[cfg(not(target_arch = "powerpc"))]
pub unsafe fn write_intr_msg(
    _fd: i32,
    _b_endpoint: u8,
    w_length: u16,
    rp_data: *mut c_void,
) -> i32 {
    if rp_data.is_null() && w_length != 0 {
        USB_FAILED
    } else {
        USB_OK
    }
}

/// # Safety
///
/// `rp_data` must point to a readable buffer for the requested transfer. On
/// PowerPC, it must be 32-byte aligned for a nonzero transfer.
#[cfg(target_arch = "powerpc")]
pub unsafe fn write_intr_msg(fd: i32, b_endpoint: u8, w_length: u16, rp_data: *mut c_void) -> i32 {
    if !is_v0_fd(fd) {
        return usb_v5_interrupt_message(
            fd,
            b_endpoint,
            w_length,
            rp_data,
            UsbAsyncCompletion {
                callback: None,
                userdata: core::ptr::null_mut(),
            },
        );
    }

    usb_v0_data_message(fd, USBV0_IOCTL_INTRMSG, b_endpoint, w_length, rp_data, true)
}

/// # Safety
///
/// `rp_data` must point to a readable buffer for the requested transfer. On
/// PowerPC, it must be 32-byte aligned for a nonzero transfer and remain valid
/// until the completion callback runs.
#[cfg(not(target_arch = "powerpc"))]
pub unsafe fn write_intr_msg_async(
    _fd: i32,
    _b_endpoint: u8,
    _w_length: u16,
    _rp_data: *mut c_void,
    cb: UsbCallback,
    userdata: *mut c_void,
) -> i32 {
    if let Some(cb) = cb {
        unsafe {
            cb(USB_OK, userdata);
        }
    }
    USB_OK
}

/// # Safety
///
/// `rp_data` must point to a readable buffer for the requested transfer. On
/// PowerPC, it must be 32-byte aligned for a nonzero transfer and remain valid
/// until the completion callback runs.
#[cfg(target_arch = "powerpc")]
pub unsafe fn write_intr_msg_async(
    fd: i32,
    b_endpoint: u8,
    w_length: u16,
    rp_data: *mut c_void,
    cb: UsbCallback,
    userdata: *mut c_void,
) -> i32 {
    if !is_v0_fd(fd) {
        return usb_v5_interrupt_message(
            fd,
            b_endpoint,
            w_length,
            rp_data,
            UsbAsyncCompletion {
                callback: cb,
                userdata,
            },
        );
    }

    usb_v0_data_message_async(UsbV0DataAsyncRequest {
        fd,
        ioctl: USBV0_IOCTL_INTRMSG,
        endpoint: b_endpoint,
        length: w_length,
        data: rp_data,
        read: false,
        completion: UsbAsyncCompletion {
            callback: cb,
            userdata,
        },
    })
}

/// # Safety
///
/// `rp_data` must be a valid readable buffer for the requested transfer.
#[cfg(not(target_arch = "powerpc"))]
pub unsafe fn write_blk_msg(_fd: i32, _b_endpoint: u8, w_length: u16, rp_data: *mut c_void) -> i32 {
    if rp_data.is_null() && w_length != 0 {
        USB_FAILED
    } else {
        USB_OK
    }
}

/// # Safety
///
/// `rp_data` must be a valid readable buffer for the requested transfer.
#[cfg(target_arch = "powerpc")]
pub unsafe fn write_blk_msg(fd: i32, b_endpoint: u8, w_length: u16, rp_data: *mut c_void) -> i32 {
    if !is_v0_fd(fd) {
        return usb_v5_data_message(fd, b_endpoint, w_length, rp_data, false);
    }
    usb_v0_data_message(fd, USBV0_IOCTL_BLKMSG, b_endpoint, w_length, rp_data, true)
}

/// # Safety
///
/// `rp_data` must be a valid readable buffer for the requested transfer.
#[cfg(not(target_arch = "powerpc"))]
pub unsafe fn write_blk_msg_async(
    _fd: i32,
    _b_endpoint: u8,
    _w_length: u16,
    _rp_data: *mut c_void,
    cb: UsbCallback,
    userdata: *mut c_void,
) -> i32 {
    if let Some(cb) = cb {
        unsafe {
            cb(USB_OK, userdata);
        }
    }
    USB_OK
}

/// # Safety
///
/// `rp_data` must be a valid readable buffer for the requested transfer.
#[cfg(target_arch = "powerpc")]
pub unsafe fn write_blk_msg_async(
    fd: i32,
    b_endpoint: u8,
    w_length: u16,
    rp_data: *mut c_void,
    cb: UsbCallback,
    userdata: *mut c_void,
) -> i32 {
    if !is_v0_fd(fd) {
        return usb_v5_data_message_async(
            fd,
            b_endpoint,
            w_length,
            rp_data,
            false,
            UsbAsyncCompletion {
                callback: cb,
                userdata,
            },
        );
    }
    usb_v0_data_message_async(UsbV0DataAsyncRequest {
        fd,
        ioctl: USBV0_IOCTL_BLKMSG,
        endpoint: b_endpoint,
        length: w_length,
        data: rp_data,
        read: false,
        completion: UsbAsyncCompletion {
            callback: cb,
            userdata,
        },
    })
}

/// # Safety
///
/// `rp_data` must be a valid readable buffer for the requested transfer.
#[cfg(not(target_arch = "powerpc"))]
pub unsafe fn write_ctrl_msg(
    _fd: i32,
    _bm_request_type: u8,
    _bm_request: u8,
    _w_value: u16,
    _w_index: u16,
    w_length: u16,
    rp_data: *mut c_void,
) -> i32 {
    if rp_data.is_null() && w_length != 0 {
        USB_FAILED
    } else {
        USB_OK
    }
}

/// # Safety
///
/// `rp_data` must be a valid readable buffer for the requested transfer.
#[cfg(target_arch = "powerpc")]
pub unsafe fn write_ctrl_msg(
    fd: i32,
    bm_request_type: u8,
    bm_request: u8,
    w_value: u16,
    w_index: u16,
    w_length: u16,
    rp_data: *mut c_void,
) -> i32 {
    if !is_v0_fd(fd) {
        return usb_v5_control_message(
            fd,
            bm_request_type,
            bm_request,
            w_value,
            w_index,
            w_length,
            rp_data,
        );
    }
    usb_control_message(
        fd,
        bm_request_type,
        bm_request,
        w_value,
        w_index,
        w_length,
        rp_data,
    )
}

/// # Safety
///
/// If `request.length` is zero, `request.data` must be null. Otherwise it must point to a readable,
/// 32-byte-aligned buffer of at least `request.length` bytes that remains valid until completion.
/// If present, the callback must be safe to invoke asynchronously with `completion.userdata`, and
/// the userdata it accesses must remain valid until then.
#[cfg(not(target_arch = "powerpc"))]
pub unsafe fn write_ctrl_msg_async(
    _request: UsbControlRequest,
    completion: UsbAsyncCompletion,
) -> i32 {
    if let Some(callback) = completion.callback {
        unsafe {
            callback(USB_OK, completion.userdata);
        }
    }
    USB_OK
}

/// # Safety
///
/// If `request.length` is zero, `request.data` must be null. Otherwise it must point to a readable,
/// 32-byte-aligned buffer of at least `request.length` bytes that remains valid until completion.
/// If present, the callback must be safe to invoke asynchronously with `completion.userdata`, and
/// the userdata it accesses must remain valid until then.
#[cfg(target_arch = "powerpc")]
pub unsafe fn write_ctrl_msg_async(
    request: UsbControlRequest,
    completion: UsbAsyncCompletion,
) -> i32 {
    if !is_v0_fd(request.fd) {
        return usb_v5_control_message_async(request, completion);
    }
    usb_v0_control_message_async(request, false, completion)
}

/// # Safety
///
/// `configuration` must be null or a valid writable pointer.
#[cfg(not(target_arch = "powerpc"))]
pub unsafe fn get_configuration(fd: i32, configuration: *mut u8) -> i32 {
    let Some(slot) = fd_index(fd) else {
        return USB_FAILED;
    };
    if configuration.is_null() {
        return USB_FAILED;
    }
    unsafe {
        *configuration = USB_LAST_CONFIGURATION[slot].load(Ordering::Acquire) as u8;
    }
    USB_OK
}

/// # Safety
///
/// `configuration` must be null or a valid writable pointer.
#[cfg(target_arch = "powerpc")]
pub unsafe fn get_configuration(fd: i32, configuration: *mut u8) -> i32 {
    unsafe {
        if configuration.is_null() {
            return IPC_EINVAL;
        }

        let heap = match ensure_desc_heap() {
            Ok(heap) => heap,
            Err(rc) => return rc,
        };
        let buffer = match HeapTemp::new_zeroed(heap, 1) {
            Ok(buffer) => buffer,
            Err(rc) => return rc,
        };

        let rc = read_ctrl_msg(
            fd,
            USB_CTRLTYPE_DIR_DEVICE2HOST | USB_CTRLTYPE_TYPE_STANDARD | USB_CTRLTYPE_REC_DEVICE,
            USB_REQ_GETCONFIGURATION,
            0,
            0,
            1,
            buffer.as_ptr(),
        );
        if rc >= 0 {
            {
                *configuration = *buffer.as_ptr().cast::<u8>();
            }
            USB_OK
        } else {
            rc
        }
    }
}

#[cfg(not(target_arch = "powerpc"))]
pub fn set_configuration(fd: i32, configuration: u8) -> i32 {
    let Some(slot) = fd_index(fd) else {
        return USB_FAILED;
    };
    USB_LAST_CONFIGURATION[slot].store(configuration as u32, Ordering::Release);
    USB_OK
}

#[cfg(target_arch = "powerpc")]
pub fn set_configuration(fd: i32, configuration: u8) -> i32 {
    let rc = unsafe {
        write_ctrl_msg(
            fd,
            USB_CTRLTYPE_DIR_HOST2DEVICE | USB_CTRLTYPE_TYPE_STANDARD | USB_CTRLTYPE_REC_DEVICE,
            USB_REQ_SETCONFIGURATION,
            configuration as u16,
            0,
            0,
            core::ptr::null_mut(),
        )
    };
    if rc >= 0 { USB_OK } else { rc }
}

#[cfg(not(target_arch = "powerpc"))]
pub fn set_alternative_interface(fd: i32, _interface: u8, _alternate_setting: u8) -> i32 {
    if fd_index(fd).is_none() {
        return USB_FAILED;
    }
    USB_OK
}

#[cfg(target_arch = "powerpc")]
pub fn set_alternative_interface(fd: i32, interface: u8, alternate_setting: u8) -> i32 {
    let rc = unsafe {
        write_ctrl_msg(
            fd,
            USB_CTRLTYPE_DIR_HOST2DEVICE | USB_CTRLTYPE_TYPE_STANDARD | USB_CTRLTYPE_REC_INTERFACE,
            USB_REQ_SETINTERFACE,
            alternate_setting as u16,
            interface as u16,
            0,
            core::ptr::null_mut(),
        )
    };
    if rc >= 0 { USB_OK } else { rc }
}

#[cfg(not(target_arch = "powerpc"))]
pub fn clear_halt(fd: i32, _endpoint_address: u8) -> i32 {
    if fd_index(fd).is_none() {
        return USB_FAILED;
    }
    USB_OK
}

#[cfg(target_arch = "powerpc")]
pub fn clear_halt(fd: i32, endpoint_address: u8) -> i32 {
    let rc = unsafe {
        write_ctrl_msg(
            fd,
            USB_CTRLTYPE_DIR_HOST2DEVICE | USB_CTRLTYPE_TYPE_STANDARD | USB_CTRLTYPE_REC_ENDPOINT,
            USB_REQ_CLEARFEATURE,
            USB_FEATURE_ENDPOINT_HALT,
            endpoint_address as u16,
            0,
            core::ptr::null_mut(),
        )
    };
    if rc >= 0 { USB_OK } else { rc }
}

/// # Safety
///
/// `descr_buffer` and `cnt_descr` must be valid writable pointers when non-null.
#[cfg(target_arch = "powerpc")]
pub unsafe fn get_device_list(
    descr_buffer: *mut UsbDeviceEntry,
    num_descr: u8,
    interface_class: u8,
    cnt_descr: *mut u8,
) -> i32 {
    if num_descr != 0 && descr_buffer.is_null() {
        return USB_FAILED;
    }

    if !cnt_descr.is_null() {
        unsafe {
            *cnt_descr = 0;
        }
    }

    let mut count_out = 0u8;
    let vendor = HostKind::Vendor.detected().load(Ordering::Acquire);
    let hid = HostKind::Hid.detected().load(Ordering::Acquire);
    if !v5_layout::use_legacy_list(vendor, hid) {
        for host in [HostKind::Vendor, HostKind::Hid] {
            let selected = match host {
                HostKind::Vendor => interface_class != USB_CLASS_HID,
                HostKind::Hid => interface_class == 0 || interface_class == USB_CLASS_HID,
            };
            if !selected || !host.detected().load(Ordering::Acquire) {
                continue;
            }
            let snapshot = with_usb_v5_host_events(host, |state| {
                if let Some(status) = state.unavailable_status() {
                    return Err(if status < 0 { status } else { IPC_ENOENT });
                }
                Ok((state.snapshot().len(), unsafe { *host.devices() }))
            });
            let (count, entries) = match snapshot {
                Ok(Ok(snapshot)) => snapshot,
                Ok(Err(error)) | Err(error) => return error,
            };
            for entry in entries.iter().take(count) {
                if count_out >= num_descr {
                    break;
                }
                unsafe {
                    *descr_buffer.add(count_out as usize) = *entry;
                }
                count_out += 1;
            }
        }
        if !cnt_descr.is_null() {
            unsafe {
                *cnt_descr = count_out;
            }
        }
        return USB_OK;
    }

    let remaining = num_descr.saturating_sub(count_out);
    if remaining == 0 {
        if !cnt_descr.is_null() {
            unsafe {
                *cnt_descr = count_out;
            }
        }
        return USB_OK;
    }

    let heap = match ensure_desc_heap() {
        Ok(heap) => heap,
        Err(rc) => return rc,
    };
    let raw_len = (remaining as usize) * 8;
    let mut raw = match HeapTemp::new_zeroed(heap, raw_len) {
        Ok(raw) => raw,
        Err(rc) => return rc,
    };
    if let Err(error) = begin_transient_operation() {
        return error;
    }
    let fd = match open_file(oh0_root_path(), IPC_OPEN_NONE) {
        Ok(fd) => fd,
        Err(rc) => {
            USB_TRANSIENT_FD.store(-1, Ordering::Release);
            return rc;
        }
    };

    let num = AlignedU8([remaining]);
    let class = AlignedU8([interface_class]);
    let mut count = AlignedU8([0]);
    let rc = fd.ioctlv::<4>(
        USBV0_IOCTL_GETDEVLIST,
        2,
        [
            IoctlvEntry::In(&num.0),
            IoctlvEntry::In(&class.0),
            IoctlvEntry::Out(&mut count.0),
            IoctlvEntry::Out(unsafe { raw.as_mut_slice(raw_len) }),
        ],
    );
    let close_rc = finish_transient_operation(fd.raw());

    if rc >= 0 {
        let count = count.0[0].min(remaining);
        if !cnt_descr.is_null() {
            unsafe {
                *cnt_descr = count_out.saturating_add(count);
            }
        }
        let raw = unsafe { raw.as_mut_slice(raw_len) };
        for idx in 0..count as usize {
            let word_offset = idx * 8 + 4;
            let word = u32::from_ne_bytes([
                raw[word_offset],
                raw[word_offset + 1],
                raw[word_offset + 2],
                raw[word_offset + 3],
            ]);
            unsafe {
                *descr_buffer.add(count_out as usize + idx) = UsbDeviceEntry {
                    device_id: USB_OH0_DEVICE_ID,
                    vid: (word >> 16) as u16,
                    pid: word as u16,
                    token: 0,
                };
            }
        }
        if close_rc < 0 {
            return close_rc;
        }
    } else {
        return rc;
    }

    rc
}

/// # Safety
///
/// `rp_data` must be null or a valid writable buffer.
#[cfg(not(target_arch = "powerpc"))]
pub unsafe fn get_ascii_string(
    _fd: i32,
    b_index: u8,
    _w_lang_id: u16,
    w_length: u16,
    rp_data: *mut c_void,
) -> i32 {
    unsafe {
        if rp_data.is_null() || w_length == 0 {
            return USB_FAILED;
        }
        let w_length = w_length.min(255);
        const STRING_DESC: &[u8] = &[
            20,
            USB_DT_STRING,
            b'l',
            0,
            b'i',
            0,
            b'b',
            0,
            b'o',
            0,
            b'g',
            0,
            b'c',
            0,
            b'-',
            0,
            b'r',
            0,
            b's',
            0,
        ];
        unsafe {
            let out = core::slice::from_raw_parts_mut(rp_data.cast::<u8>(), w_length as usize);
            if b_index == 0 {
                let lang = [4u8, USB_DT_STRING, 0x09, 0x04];
                let n = out.len().min(lang.len());
                out[..n].copy_from_slice(&lang[..n]);
                return w_length as i32;
            }

            let mut ro = 0usize;
            let mut bo = 2usize;
            while ro < out.len().saturating_sub(1) && bo + 1 < STRING_DESC[0] as usize {
                out[ro] = if STRING_DESC[bo + 1] != 0 {
                    b'?'
                } else {
                    STRING_DESC[bo]
                };
                ro += 1;
                bo += 2;
            }
            if ro < out.len() {
                out[ro] = 0;
            }

            ro.saturating_sub(1) as i32
        }
    }
}

/// # Safety
///
/// `rp_data` must be null or a valid writable buffer.
#[cfg(target_arch = "powerpc")]
pub unsafe fn get_ascii_string(
    fd: i32,
    b_index: u8,
    w_lang_id: u16,
    w_length: u16,
    rp_data: *mut c_void,
) -> i32 {
    unsafe {
        if rp_data.is_null() || w_length == 0 {
            return IPC_EINVAL;
        }

        let heap = match ensure_desc_heap() {
            Ok(heap) => heap,
            Err(rc) => return rc,
        };
        let mut descriptor = match HeapTemp::new_zeroed(heap, 255) {
            Ok(descriptor) => descriptor,
            Err(rc) => return rc,
        };
        let output_len = w_length.min(255) as usize;
        // C always requests the complete 255-byte string descriptor and only
        // applies wLength while copying/converting the result.
        let rc = read_ctrl_msg(
            fd,
            USB_CTRLTYPE_DIR_DEVICE2HOST | USB_CTRLTYPE_TYPE_STANDARD | USB_CTRLTYPE_REC_DEVICE,
            USB_REQ_GETDESCRIPTOR,
            ((USB_DT_STRING as u16) << 8) | b_index as u16,
            w_lang_id,
            255,
            descriptor.as_ptr(),
        );
        if rc < 0 {
            return rc;
        }

        let descriptor = { descriptor.as_mut_slice(255) };
        let desc_len = descriptor[0] as usize;
        if desc_len < 2 || descriptor[1] != USB_DT_STRING {
            return IPC_EINVAL;
        }

        let out = { core::slice::from_raw_parts_mut(rp_data.cast::<u8>(), output_len) };
        if b_index == 0 {
            out.copy_from_slice(&descriptor[..output_len]);
            return rc;
        }

        let mut written = 0usize;
        let mut offset = 2usize;
        while offset + 1 < desc_len && written + 1 < out.len() {
            out[written] = if descriptor[offset + 1] == 0 {
                descriptor[offset]
            } else {
                b'?'
            };
            written += 1;
            offset += 2;
        }
        if written < out.len() {
            out[written] = 0;
        }
        written as i32 - 1
    }
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;
    use core::mem::{align_of, offset_of, size_of};
    use core::sync::atomic::{AtomicI32, AtomicUsize, Ordering};

    static CALLBACK_RESULT: AtomicI32 = AtomicI32::new(i32::MIN);
    static CALLBACK_USERDATA: AtomicUsize = AtomicUsize::new(0);

    #[test]
    fn hid_descriptor_conversion_handles_all_complete_entries() {
        let mut descriptor = [
            9,
            USB_DT_HID,
            0x11,
            0x01,
            0,
            3,
            USB_DT_REPORT,
            0x34,
            0x12,
            USB_DT_REPORT,
            0x78,
            0x56,
            0x22,
            0xbc,
            0x9a,
        ];

        convert_hid_descriptor_lengths(&mut descriptor);

        assert_eq!(&descriptor[2..4], &0x0111u16.to_ne_bytes());
        assert_eq!(&descriptor[7..9], &0x1234u16.to_ne_bytes());
        assert_eq!(descriptor[9], USB_DT_REPORT);
        assert_eq!(&descriptor[10..12], &0x5678u16.to_ne_bytes());
        assert_eq!(descriptor[12], 0x22);
        assert_eq!(&descriptor[13..15], &0x9abcu16.to_ne_bytes());
    }

    #[test]
    fn hid_descriptor_conversion_skips_incomplete_trailing_entry() {
        let mut descriptor = [
            9,
            USB_DT_HID,
            0x11,
            0x01,
            0,
            3,
            USB_DT_REPORT,
            0x34,
            0x12,
            USB_DT_REPORT,
            0x78,
            0x56,
            0x22,
            0xbc,
        ];

        convert_hid_descriptor_lengths(&mut descriptor);

        assert_eq!(&descriptor[10..12], &0x5678u16.to_ne_bytes());
        assert_eq!(descriptor[12], 0x22);
        assert_eq!(descriptor[13], 0xbc);
    }

    unsafe extern "C" fn record_callback(result: i32, usrdata: *mut c_void) -> i32 {
        CALLBACK_RESULT.store(result, Ordering::Release);
        CALLBACK_USERDATA.store(usrdata as usize, Ordering::Release);
        result
    }

    unsafe extern "C" fn close_callback_with_distinct_result(
        result: i32,
        _usrdata: *mut c_void,
    ) -> i32 {
        CALLBACK_RESULT.store(result, Ordering::Release);
        -0x1234
    }

    #[test]
    fn async_close_returns_the_callback_result() {
        CALLBACK_RESULT.store(i32::MIN, Ordering::Release);
        let close_result = -0x2345;

        let returned = close_device_async_result(
            close_result,
            Some(close_callback_with_distinct_result),
            core::ptr::null_mut(),
        );

        assert_eq!(CALLBACK_RESULT.load(Ordering::Acquire), close_result);
        assert_eq!(returned, -0x1234);
        assert_ne!(returned, close_result);
        assert_eq!(
            close_device_async_result(close_result, None, core::ptr::null_mut()),
            close_result
        );
    }

    #[test]
    fn hid_v5_open_validates_and_frees_descriptors_before_success() {
        use core::cell::Cell;

        let phase = Cell::new(0);
        let device_id = 42;
        let result = prepare_v5_device_with(
            device_id,
            HostKind::Hid,
            |id| {
                assert_eq!(id, device_id);
                phase.set(1);
                -7 // Resume is best effort in C.
            },
            |id, _descriptor| {
                assert_eq!(id, device_id);
                assert_eq!(phase.get(), 1);
                phase.set(2);
                USB_OK
            },
            |_descriptor| {
                assert_eq!(phase.get(), 2);
                phase.set(3);
            },
            |_| panic!("successful descriptor validation must not suspend"),
        );

        assert_eq!(result, USB_OK);
        assert_eq!(phase.get(), 3);
    }

    #[test]
    fn hid_v5_open_suspends_and_returns_descriptor_error() {
        use core::cell::Cell;

        let phase = Cell::new(0);
        let device_id = 42;
        let descriptor_error = -0x321;
        let result = prepare_v5_device_with(
            device_id,
            HostKind::Hid,
            |id| {
                assert_eq!(id, device_id);
                phase.set(1);
                USB_OK
            },
            |id, _descriptor| {
                assert_eq!(id, device_id);
                assert_eq!(phase.get(), 1);
                phase.set(2);
                descriptor_error
            },
            |_| panic!("failed descriptor validation must not free descriptors"),
            |id| {
                assert_eq!(id, device_id);
                assert_eq!(phase.get(), 2);
                phase.set(3);
                -8 // Suspend is best effort in C.
            },
        );

        assert_eq!(result, descriptor_error);
        assert_eq!(phase.get(), 3);
    }

    #[test]
    fn vendor_v5_open_does_not_read_hid_descriptors() {
        let device_id = -1;
        let result = prepare_v5_device_with(
            device_id,
            HostKind::Vendor,
            |id| {
                assert_eq!(id, device_id);
                -7 // Resume is best effort in C.
            },
            |_, _| panic!("vendor open must not perform HID descriptor validation"),
            |_| panic!("vendor open has no HID descriptors to free"),
            |_| panic!("vendor open must not suspend after resume"),
        );

        assert_eq!(result, USB_OK);
    }

    #[test]
    fn exports_exist() {
        let _ = initialize as fn() -> i32;
        let _ = deinitialize as fn() -> i32;
        let _ = open_device as unsafe fn(i32, u16, u16, *mut i32) -> i32;
        let _ = close_device as unsafe fn(*mut i32) -> i32;
        let _ = close_device_async as unsafe fn(*mut i32, UsbCallback, *mut c_void) -> i32;
        let _ = get_device_description as unsafe fn(i32, *mut UsbDevDesc) -> i32;
        let _ = get_descriptors as unsafe fn(i32, *mut UsbDevDesc) -> i32;
        let _ = free_descriptors as unsafe fn(*mut UsbDevDesc);
        let _ = get_generic_descriptor as unsafe fn(i32, u8, u8, u8, *mut c_void, u32) -> i32;
        let _ = get_hid_descriptor as unsafe fn(i32, u8, *mut UsbHidDesc, u32) -> i32;
        let _ = get_report_descriptor_size as fn(i32, u8) -> i32;
        let _ = get_report_descriptor as unsafe fn(i32, u8, *mut c_void, u16) -> i32;
        let _ = device_removal_notify_async as unsafe fn(i32, UsbCallback, *mut c_void) -> i32;
        let _ = device_change_notify_async as unsafe fn(u8, UsbCallback, *mut c_void) -> i32;
        let _ = suspend_device as fn(i32) -> i32;
        let _ = resume_device as fn(i32) -> i32;
        let _ = read_iso_msg as unsafe fn(i32, u8, u8, *mut u16, *mut c_void) -> i32;
        let _ = read_iso_msg_async
            as unsafe fn(i32, u8, u8, *mut u16, *mut c_void, UsbCallback, *mut c_void) -> i32;
        let _ = read_intr_msg as unsafe fn(i32, u8, u16, *mut c_void) -> i32;
        let _ = read_intr_msg_async
            as unsafe fn(i32, u8, u16, *mut c_void, UsbCallback, *mut c_void) -> i32;
        let _ = read_blk_msg as unsafe fn(i32, u8, u16, *mut c_void) -> i32;
        let _ = read_blk_msg_async
            as unsafe fn(i32, u8, u16, *mut c_void, UsbCallback, *mut c_void) -> i32;
        let _ = read_ctrl_msg as unsafe fn(i32, u8, u8, u16, u16, u16, *mut c_void) -> i32;
        let _ = read_ctrl_msg_async as unsafe fn(UsbControlRequest, UsbAsyncCompletion) -> i32;
        let _ = write_iso_msg as unsafe fn(i32, u8, u8, *mut u16, *mut c_void) -> i32;
        let _ = write_iso_msg_async
            as unsafe fn(i32, u8, u8, *mut u16, *mut c_void, UsbCallback, *mut c_void) -> i32;
        let _ = write_intr_msg as unsafe fn(i32, u8, u16, *mut c_void) -> i32;
        let _ = write_intr_msg_async
            as unsafe fn(i32, u8, u16, *mut c_void, UsbCallback, *mut c_void) -> i32;
        let _ = write_blk_msg as unsafe fn(i32, u8, u16, *mut c_void) -> i32;
        let _ = write_blk_msg_async
            as unsafe fn(i32, u8, u16, *mut c_void, UsbCallback, *mut c_void) -> i32;
        let _ = write_ctrl_msg as unsafe fn(i32, u8, u8, u16, u16, u16, *mut c_void) -> i32;
        let _ = write_ctrl_msg_async as unsafe fn(UsbControlRequest, UsbAsyncCompletion) -> i32;
        let _ = get_configuration as unsafe fn(i32, *mut u8) -> i32;
        let _ = set_configuration as fn(i32, u8) -> i32;
        let _ = set_alternative_interface as fn(i32, u8, u8) -> i32;
        let _ = clear_halt as fn(i32, u8) -> i32;
        let _ = get_device_list as unsafe fn(*mut UsbDeviceEntry, u8, u8, *mut u8) -> i32;
        let _ = get_ascii_string as unsafe fn(i32, u8, u16, u16, *mut c_void) -> i32;
    }

    #[test]
    fn descriptor_layouts_are_c_compatible() {
        assert_eq!(size_of::<UsbEndpointDesc>(), 7);
        assert_eq!(align_of::<UsbEndpointDesc>(), 1);
        assert_eq!(offset_of!(UsbEndpointDesc, b_endpoint_address), 2);
        assert_eq!(offset_of!(UsbEndpointDesc, w_max_packet_size), 4);

        assert_eq!(size_of::<UsbConfigDesc>(), 13);
        assert_eq!(align_of::<UsbConfigDesc>(), 1);
        assert_eq!(offset_of!(UsbConfigDesc, interfaces), 9);

        assert_eq!(size_of::<UsbInterfaceDesc>(), 19);
        assert_eq!(align_of::<UsbInterfaceDesc>(), 1);
        assert_eq!(offset_of!(UsbInterfaceDesc, extra), 9);
        assert_eq!(offset_of!(UsbInterfaceDesc, extra_size), 13);
        assert_eq!(offset_of!(UsbInterfaceDesc, endpoints), 15);

        assert_eq!(size_of::<UsbDevDesc>(), 22);
        assert_eq!(align_of::<UsbDevDesc>(), 1);
        assert_eq!(offset_of!(UsbDevDesc, id_vendor), 8);
        assert_eq!(offset_of!(UsbDevDesc, configurations), 18);

        assert_eq!(size_of::<UsbHidDesc>(), 9);
        assert_eq!(align_of::<UsbHidDesc>(), 1);
        assert_eq!(offset_of!(UsbHidDesc, descr_w_descriptor_length), 7);

        assert_eq!(size_of::<UsbDeviceEntry>(), 12);
        assert_eq!(offset_of!(UsbDeviceEntry, token), 8);
    }

    #[test]
    fn device_registry_open_close_and_configuration_state() {
        assert_eq!(initialize(), USB_OK);

        let mut fd = -1;
        unsafe {
            assert_eq!(
                open_device(USB_OH0_DEVICE_ID, 0x057e, 0x0306, &mut fd),
                USB_OK
            );
        }
        assert!(fd > 0);

        let mut desc = UsbDevDesc::default();
        unsafe {
            assert_eq!(get_device_description(fd, &mut desc), USB_OK);
        }
        assert_eq!(desc.b_length, 18);
        assert_eq!(desc.b_descriptor_type, USB_DT_DEVICE);
        assert_eq!(desc.b_max_packet_size0, 64);
        let id_product = desc.id_product;
        assert_eq!(id_product, fd as u16);

        assert_eq!(set_configuration(fd, 3), USB_OK);
        let mut configuration = 0;
        unsafe {
            assert_eq!(get_configuration(fd, &mut configuration), USB_OK);
        }
        assert_eq!(configuration, 3);
        assert_eq!(set_alternative_interface(fd, 1, 2), USB_OK);
        assert_eq!(clear_halt(fd, USB_ENDPOINT_IN | 1), USB_OK);
        assert_eq!(set_alternative_interface(-1, 1, 2), USB_FAILED);
        assert_eq!(clear_halt(-1, USB_ENDPOINT_IN | 1), USB_FAILED);

        unsafe {
            assert_eq!(close_device(&mut fd), USB_OK);
        }
        assert_eq!(fd, -1);
        assert_eq!(deinitialize(), USB_OK);
    }

    #[test]
    fn device_list_descriptors_and_ascii_string_are_host_compat() {
        let mut entry = UsbDeviceEntry::default();
        let mut count = 0;
        unsafe {
            assert_eq!(
                get_device_list(&mut entry, 1, USB_CLASS_HID, &mut count),
                USB_OK
            );
        }
        assert_eq!(count, 1);
        assert_eq!(entry.device_id, USB_OH0_DEVICE_ID);

        count = 99;
        entry = UsbDeviceEntry::default();
        unsafe {
            assert_eq!(get_device_list(&mut entry, 1, 0x08, &mut count), USB_OK);
        }
        assert_eq!(count, 1);
        assert_eq!(entry.device_id, USB_OH0_DEVICE_ID);

        let mut generic = [0xaa; 4];
        unsafe {
            assert_eq!(
                get_generic_descriptor(
                    1,
                    USB_DT_CONFIG,
                    0,
                    0,
                    generic.as_mut_ptr().cast(),
                    generic.len() as u32,
                ),
                USB_OK
            );
        }
        assert_eq!(generic[0], generic.len() as u8);
        assert_eq!(generic[1], USB_DT_CONFIG);

        let mut hid = UsbHidDesc::default();
        unsafe {
            assert_eq!(
                get_hid_descriptor(1, 0, &mut hid, size_of::<UsbHidDesc>() as u32),
                USB_OK
            );
        }
        assert_eq!(hid.b_descriptor_type, USB_DT_HID);
        assert_eq!(hid.descr_b_descriptor_type, USB_DT_REPORT);
        let report_descriptor_length = hid.descr_w_descriptor_length;
        assert_eq!(report_descriptor_length, 64);
        unsafe {
            assert_eq!(get_hid_descriptor(1, 0, &mut hid, 8), IPC_EINVAL);
        }

        assert_eq!(get_report_descriptor_size(1, 0), 64);
        unsafe {
            assert_eq!(
                get_report_descriptor(1, 0, core::ptr::null_mut(), 8),
                IPC_EINVAL
            );
        }

        let mut report = [0xff; 8];
        unsafe {
            assert_eq!(
                get_report_descriptor(1, 0, report.as_mut_ptr().cast(), 7),
                IPC_EINVAL
            );
            assert_eq!(
                get_report_descriptor(1, 0, report.as_mut_ptr().cast(), report.len() as u16),
                USB_OK
            );
        }
        assert_eq!(&report[..2], &[0, 0]);

        let mut string = [0xff; 12];
        unsafe {
            assert_eq!(
                get_ascii_string(
                    1,
                    1,
                    0x0409,
                    string.len() as u16,
                    string.as_mut_ptr().cast()
                ),
                8
            );
        }
        assert_eq!(&string[..9], b"libogc-rs");
        assert_eq!(string[9], 0);

        let mut lang = [0xaa; 9];
        unsafe {
            assert_eq!(
                get_ascii_string(1, 0, 0x0409, 9, lang.as_mut_ptr().cast()),
                9
            );
        }
        assert_eq!(&lang[..4], &[4, USB_DT_STRING, 0x09, 0x04]);
    }

    #[test]
    fn async_transfer_completion_is_host_compat() {
        let userdata = 0x1234usize as *mut c_void;
        CALLBACK_RESULT.store(i32::MIN, Ordering::Release);
        unsafe {
            assert_eq!(
                device_change_notify_async(USB_CLASS_HID, Some(record_callback), userdata),
                USB_OK
            );
        }
        assert_eq!(CALLBACK_RESULT.load(Ordering::Acquire), i32::MIN);

        let mut data = [0u8; 8];
        let mut packet_sizes = [4u16, 4];
        unsafe {
            assert_eq!(
                read_iso_msg(
                    1,
                    USB_ENDPOINT_IN | 1,
                    2,
                    packet_sizes.as_mut_ptr(),
                    data.as_mut_ptr().cast()
                ),
                USB_OK
            );
            assert_eq!(
                read_intr_msg(
                    1,
                    USB_ENDPOINT_IN | 1,
                    data.len() as u16,
                    data.as_mut_ptr().cast()
                ),
                USB_OK
            );
            assert_eq!(
                read_blk_msg(
                    1,
                    USB_ENDPOINT_IN | 2,
                    data.len() as u16,
                    data.as_mut_ptr().cast()
                ),
                USB_OK
            );
            assert_eq!(
                read_ctrl_msg(
                    1,
                    0x80,
                    6,
                    0x0100,
                    0,
                    data.len() as u16,
                    data.as_mut_ptr().cast()
                ),
                USB_OK
            );
            assert_eq!(
                write_iso_msg(
                    1,
                    USB_ENDPOINT_OUT | 1,
                    2,
                    packet_sizes.as_mut_ptr(),
                    data.as_mut_ptr().cast()
                ),
                USB_OK
            );
            assert_eq!(
                write_intr_msg(
                    1,
                    USB_ENDPOINT_OUT | 1,
                    data.len() as u16,
                    data.as_mut_ptr().cast()
                ),
                USB_OK
            );
            assert_eq!(
                write_blk_msg(
                    1,
                    USB_ENDPOINT_OUT | 2,
                    data.len() as u16,
                    data.as_mut_ptr().cast()
                ),
                USB_OK
            );
            assert_eq!(
                write_ctrl_msg(1, 0, 9, 1, 0, data.len() as u16, data.as_mut_ptr().cast()),
                USB_OK
            );
        }

        CALLBACK_RESULT.store(i32::MIN, Ordering::Release);
        unsafe {
            assert_eq!(
                read_blk_msg_async(
                    1,
                    USB_ENDPOINT_IN | 2,
                    data.len() as u16,
                    data.as_mut_ptr().cast(),
                    Some(record_callback),
                    userdata,
                ),
                USB_OK
            );
        }
        assert_eq!(CALLBACK_RESULT.load(Ordering::Acquire), USB_OK);
    }

    #[test]
    fn invalid_inputs_return_failed() {
        unsafe {
            assert_eq!(
                open_device(USB_OH0_DEVICE_ID, 0, 0, core::ptr::null_mut()),
                USB_FAILED
            );
            assert_eq!(get_configuration(-1, core::ptr::null_mut()), USB_FAILED);
            assert_eq!(
                get_ascii_string(1, 0, 0, 0, core::ptr::null_mut()),
                USB_FAILED
            );
            assert_eq!(read_intr_msg(1, 1, 1, core::ptr::null_mut()), USB_FAILED);
            assert_eq!(
                write_iso_msg(1, 1, 1, core::ptr::null_mut(), core::ptr::null_mut()),
                USB_FAILED
            );
        }
    }
}
