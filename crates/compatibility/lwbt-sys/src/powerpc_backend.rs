use core::ffi::c_void;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use ogc_bt_sync::{
    BluetoothAddress, BtSyncHost, HIDP_DATA_RTYPE_OUTPUT, HIDP_TRANS_DATA, HciEvent, HciOpcode,
    HidL2capState, L2CAP_CFG_SUCCESS, L2CAP_CID_SIGNALING, L2CAP_HID_CTRL_LOCAL_CID,
    L2CAP_HID_INTR_LOCAL_CID, L2CAP_PSM_HID_CTRL, L2CAP_PSM_HID_INTR, L2capSignalPayload,
    ScanEnable,
};
use ogc_conf::{self as conf, ConfPadGuestDevice, ConfPadGuests};
use ogc_lwp_driver::{TICKS_PER_SECOND, critical_enter, critical_exit, now_ticks, yield_thread};

use crate::{
    BD_LINK_KEY_LEN, BdAddr, BteCallback, ERR_ARG, ERR_CONN, ERR_IF, ERR_OK, LinkKeyInfo,
    PAIR_MODE_NORMAL, PAIR_MODE_TEMPORARY, PadNameInfo,
};

const INQUIRY_CANCEL: HciOpcode = HciOpcode::new(0x01, 0x0002);
const SET_EVENT_FILTER: HciOpcode = HciOpcode::new(0x03, 0x0005);
const WRITE_CLASS_OF_DEVICE: HciOpcode = HciOpcode::new(0x03, 0x0024);
const WRITE_PIN_TYPE: HciOpcode = HciOpcode::new(0x03, 0x000a);
const WRITE_LOCAL_NAME: HciOpcode = HciOpcode::new(0x03, 0x0013);
const WRITE_PAGE_TIMEOUT: HciOpcode = HciOpcode::new(0x03, 0x0018);
const WRITE_HOST_BUFFER_SIZE: HciOpcode = HciOpcode::new(0x03, 0x0033);
const WRITE_INQUIRY_MODE: HciOpcode = HciOpcode::new(0x03, 0x0045);
const WRITE_PAGE_SCAN_TYPE: HciOpcode = HciOpcode::new(0x03, 0x0047);
const WRITE_INQUIRY_SCAN_TYPE: HciOpcode = HciOpcode::new(0x03, 0x0043);
const VENDOR_PATCH_START: HciOpcode = HciOpcode::new(0x3f, 0x004f);
const VENDOR_PATCH_CONTINUE: HciOpcode = HciOpcode::new(0x3f, 0x004c);
const VENDOR_PATCH_END: HciOpcode = HciOpcode::new(0x3f, 0x004f);
const GUEST_SLOTS: usize = 6;
const ACL_DISPATCH_STACK_SIZE: usize = 16 * 1024;
const CONTROL_REQUEST_COUNT: usize = 256;
const MAX_BTE_MESSAGE_SIZE: usize = 672;
const MAX_HIDP_PAYLOAD_SIZE: usize = MAX_BTE_MESSAGE_SIZE + 1;
const HID_L2CAP_MTU: u16 = MAX_HIDP_PAYLOAD_SIZE as u16;
const MAX_ACL_PACKET_SIZE: usize = MAX_HIDP_PAYLOAD_SIZE + 8;
const CONTROL_NONE: u16 = u16::MAX;
const CONTROL_READY: u8 = 0;
const CONTROL_SENDING: u8 = 1;
const CONTROL_COMPLETE: u8 = 2;
const CONNECTION_TASK_IDLE: u32 = 0;
const CONNECTION_TASK_RESERVED: u32 = 1;
const CONNECTION_TASK_CONNECT: u32 = 2;
const CONNECTION_TASK_LISTEN: u32 = 3;
const CONNECTION_TASK_CONNECT_STEP2: u32 = 4;
const CONNECTION_TASK_LISTEN_STEP2: u32 = 5;
const CONNECTION_WORKER_STACK_SIZE: usize = 16 * 1024;

const PATCH0: [u8; 184] = [
    0x70, 0x99, 0x08, 0x00, 0x88, 0x43, 0xd1, 0x07, 0x09, 0x0c, 0x08, 0x43, 0xa0, 0x62, 0x19, 0x23,
    0xdb, 0x01, 0x33, 0x80, 0x7c, 0xf7, 0x88, 0xf8, 0x28, 0x76, 0x80, 0xf7, 0x17, 0xff, 0x43, 0x78,
    0xeb, 0x70, 0x19, 0x23, 0xdb, 0x01, 0x33, 0x87, 0x7c, 0xf7, 0xbc, 0xfb, 0x0b, 0x60, 0xa3, 0x7b,
    0x01, 0x49, 0x0b, 0x60, 0x90, 0xf7, 0x96, 0xfb, 0xd8, 0x1d, 0x08, 0x00, 0x00, 0xf0, 0x04, 0xf8,
    0x00, 0x23, 0x79, 0xf7, 0xe3, 0xfa, 0x00, 0x00, 0x00, 0xb5, 0x00, 0x23, 0x11, 0x49, 0x0b, 0x60,
    0x1d, 0x21, 0xc9, 0x03, 0x0b, 0x60, 0x7d, 0x20, 0x80, 0x01, 0x01, 0x38, 0xfd, 0xd1, 0x0e, 0x4b,
    0x0e, 0x4a, 0x13, 0x60, 0x47, 0x20, 0x00, 0x21, 0x96, 0xf7, 0x96, 0xff, 0x46, 0x20, 0x00, 0x21,
    0x96, 0xf7, 0x92, 0xff, 0x0a, 0x4a, 0x13, 0x68, 0x0a, 0x48, 0x03, 0x40, 0x13, 0x60, 0x0a, 0x4a,
    0x13, 0x68, 0x0a, 0x48, 0x03, 0x40, 0x13, 0x60, 0x09, 0x4a, 0x13, 0x68, 0x09, 0x48, 0x03, 0x40,
    0x13, 0x60, 0x00, 0xbd, 0x24, 0x80, 0x0e, 0x00, 0x81, 0x03, 0x0f, 0xfe, 0x5c, 0x00, 0x0f, 0x00,
    0x60, 0xfc, 0x0e, 0x00, 0xfe, 0xff, 0x00, 0x00, 0xfc, 0xfc, 0x0e, 0x00, 0xff, 0x9f, 0x00, 0x00,
    0x30, 0xfc, 0x0e, 0x00, 0x7f, 0xff, 0x00, 0x00,
];

const PATCH1: [u8; 92] = [
    0x07, 0x20, 0xbc, 0x65, 0x01, 0x00, 0x84, 0x42, 0x09, 0xd2, 0x84, 0x42, 0x09, 0xd1, 0x21, 0x84,
    0x5a, 0x00, 0x00, 0x83, 0xf0, 0x74, 0xff, 0x09, 0x0c, 0x08, 0x43, 0x22, 0x00, 0x61, 0x00, 0x00,
    0x83, 0xf0, 0x40, 0xfc, 0x00, 0x00, 0x00, 0x00, 0x23, 0xcc, 0x9f, 0x01, 0x00, 0x6f, 0xf0, 0xe4,
    0xfc, 0x03, 0x28, 0x7d, 0xd1, 0x24, 0x3c, 0x62, 0x01, 0x00, 0x28, 0x20, 0x00, 0xe0, 0x60, 0x8d,
    0x23, 0x68, 0x25, 0x04, 0x12, 0x01, 0x00, 0x20, 0x1c, 0x20, 0x1c, 0x24, 0xe0, 0xb0, 0x21, 0x26,
    0x74, 0x2f, 0x00, 0x00, 0x86, 0xf0, 0x18, 0xfd, 0x21, 0x4f, 0x3b, 0x60,
];

static mut HOST: MaybeUninit<BtSyncHost> = MaybeUninit::uninit();
static HOST_READY: AtomicBool = AtomicBool::new(false);
static PAIR_MODE: AtomicU32 = AtomicU32::new(PAIR_MODE_NORMAL as u32);
static ACL_DISPATCH_RUNNING: AtomicBool = AtomicBool::new(false);
static CONNECTION_TASK: AtomicU32 = AtomicU32::new(CONNECTION_TASK_IDLE);
static CONNECTION_TASK_INDEX: AtomicU32 = AtomicU32::new(0);
static CONNECTION_WORKER_RUNNING: AtomicBool = AtomicBool::new(false);
static mut CONNECTION_WORKER: u32 = ogc_lwp_driver::LWP_THREAD_NULL;
static mut CONNECTION_WORKER_STACK: [u8; CONNECTION_WORKER_STACK_SIZE] =
    [0; CONNECTION_WORKER_STACK_SIZE];
static mut CONNECTION_WORKER_SEMAPHORE: ogc_lwp_driver::SemaphoreHandle =
    ogc_lwp_driver::LWP_SEM_NULL;
#[derive(Copy, Clone)]
struct ControlRequest {
    used: bool,
    state: u8,
    synchronous: bool,
    len: u16,
    result: i32,
    next: u16,
    callback: crate::BteConnCfmCallback,
    payload: [u8; MAX_HIDP_PAYLOAD_SIZE],
}

impl ControlRequest {
    const EMPTY: Self = Self {
        used: false,
        state: CONTROL_READY,
        synchronous: false,
        len: 0,
        result: ERR_OK,
        next: CONTROL_NONE,
        callback: None,
        payload: [0; MAX_HIDP_PAYLOAD_SIZE],
    };
}

static mut CONTROL_REQUESTS: [ControlRequest; CONTROL_REQUEST_COUNT] =
    [ControlRequest::EMPTY; CONTROL_REQUEST_COUNT];
static mut CONTROL_HEAD: [u16; crate::PCB_SLOTS] = [CONTROL_NONE; crate::PCB_SLOTS];
static mut CONTROL_TAIL: [u16; crate::PCB_SLOTS] = [CONTROL_NONE; crate::PCB_SLOTS];

unsafe fn control_request_ptr(slot: usize) -> *mut ControlRequest {
    unsafe {
        core::ptr::addr_of_mut!(CONTROL_REQUESTS)
            .cast::<ControlRequest>()
            .add(slot)
    }
}

unsafe fn control_head_ptr(index: usize) -> *mut u16 {
    unsafe {
        core::ptr::addr_of_mut!(CONTROL_HEAD)
            .cast::<u16>()
            .add(index)
    }
}

unsafe fn control_tail_ptr(index: usize) -> *mut u16 {
    unsafe {
        core::ptr::addr_of_mut!(CONTROL_TAIL)
            .cast::<u16>()
            .add(index)
    }
}
static mut ACL_DISPATCH_THREAD: u32 = ogc_lwp_driver::LWP_THREAD_NULL;
static mut ACL_DISPATCH_STACK: [u8; ACL_DISPATCH_STACK_SIZE] = [0; ACL_DISPATCH_STACK_SIZE];

#[derive(Copy, Clone)]
struct PcbTransport {
    address: BluetoothAddress,
    handle: u16,
    listening: bool,
    l2cap: HidL2capState,
    config_sent: [bool; 2],
    disconnecting: bool,
    disconnect_signal_id: u8,
    disconnect_local_cid: u16,
    disconnect_remote_cid: u16,
}

impl PcbTransport {
    const fn new() -> Self {
        Self {
            address: BluetoothAddress { bytes: [0; 6] },
            handle: 0,
            listening: false,
            l2cap: HidL2capState::new(),
            config_sent: [false; 2],
            disconnecting: false,
            disconnect_signal_id: 0,
            disconnect_local_cid: 0,
            disconnect_remote_cid: 0,
        }
    }
}

static mut PCB_TRANSPORTS: [PcbTransport; crate::PCB_SLOTS] =
    [const { PcbTransport::new() }; crate::PCB_SLOTS];

fn map_error<T>(result: ogc_bt_sync::Result<T>) -> Result<T, i32> {
    result.map_err(|_| ERR_IF)
}

fn host() -> Option<&'static BtSyncHost> {
    HOST_READY
        .load(Ordering::Acquire)
        .then(|| unsafe { &*core::ptr::addr_of!(HOST).cast::<BtSyncHost>() })
}

fn complete(cb: BteCallback, result: i32) -> i32 {
    unsafe {
        if let Some(cb) = cb {
            cb(result, core::ptr::null_mut());
        }
    }
    result
}

fn command(host: &BtSyncHost, opcode: HciOpcode, params: &[u8]) -> Result<(), i32> {
    host.send_hci_command(opcode, params).map_err(|_| ERR_IF)?;
    let mut events = [0u8; ogc_bt_sync::MAX_EVENT_PACKET_BYTES];
    for _ in 0..8 {
        let used = host.read_event(&mut events).map_err(|_| ERR_IF)?;
        match host.parse_event(&events[..used]).map_err(|_| ERR_IF)? {
            HciEvent::CommandComplete {
                opcode: seen,
                status,
                ..
            } if seen == opcode => return (status == 0).then_some(()).ok_or(ERR_CONN),
            HciEvent::CommandStatus {
                opcode: seen,
                status,
            } if seen == opcode => {
                if status != 0 {
                    return Err(ERR_CONN);
                }
            }
            HciEvent::CommandComplete { .. } | HciEvent::CommandStatus { .. } => {}
            other => handle_pairing_event(host, other),
        }
    }
    Err(ERR_IF)
}

fn write_host_configuration(host: &BtSyncHost, inquiry_scan: bool) -> Result<(), i32> {
    command(host, WRITE_CLASS_OF_DEVICE, &[0x04, 0x02, 0x40])?;
    let mut name = [0u8; 248];
    if inquiry_scan {
        name[..3].copy_from_slice(b"Wii");
    }
    command(host, WRITE_LOCAL_NAME, &name)?;
    command(host, WRITE_PIN_TYPE, &[0])?;
    command(
        host,
        WRITE_HOST_BUFFER_SIZE,
        &[0x9b, 0x06, 0xff, 0x14, 0x00, 0x01, 0x00],
    )?;
    Ok(())
}

pub(crate) fn init() -> i32 {
    if HOST_READY.load(Ordering::Acquire) {
        return ERR_OK;
    }

    let Ok(opened_host) = BtSyncHost::open() else {
        return ERR_IF;
    };
    unsafe { core::ptr::addr_of_mut!(HOST).write(MaybeUninit::new(opened_host)) };
    HOST_READY.store(true, Ordering::Release);

    let mut events = [0u8; ogc_bt_sync::MAX_EVENT_PACKET_BYTES];
    if host().is_some_and(|host| host.reset(&mut events).is_err()) {
        shutdown();
        return ERR_IF;
    }
    if start_connection_worker().is_err() {
        shutdown();
        return ERR_IF;
    }
    ERR_OK
}

pub(crate) fn shutdown() -> i32 {
    PAIR_MODE.store(PAIR_MODE_NORMAL as u32, Ordering::Release);
    if stop_connection_worker().is_err() {
        return ERR_IF;
    }
    for index in 0..crate::PCB_SLOTS {
        if let Some(pcb) = crate::pcb_for_index(index) {
            complete_control_request(index, pcb, crate::ERR_CLSD);
        }
    }
    unsafe {
        PCB_TRANSPORTS = [const { PcbTransport::new() }; crate::PCB_SLOTS];
    }
    if HOST_READY.swap(false, Ordering::AcqRel) {
        let host = unsafe { core::ptr::addr_of!(HOST).read().assume_init() };
        if host.close().is_err() {
            return ERR_IF;
        }
    }
    ERR_OK
}

pub(crate) fn run_init_callback(cb: BteCallback) -> i32 {
    complete(cb, init())
}

pub(crate) fn init_core(cb: BteCallback) -> i32 {
    let result = host().ok_or(ERR_IF).and_then(|host| {
        let mut events = [0u8; ogc_bt_sync::MAX_EVENT_PACKET_BYTES];
        map_error(host.reset(&mut events))?;
        write_host_configuration(host, false)
    });
    complete(cb, result.map_or_else(|err| err, |_| ERR_OK))
}

pub(crate) fn init_sub(cb: BteCallback) -> i32 {
    let result = host().ok_or(ERR_IF).and_then(|host| {
        command(host, WRITE_INQUIRY_MODE, &[1])?;
        command(host, WRITE_PAGE_SCAN_TYPE, &[1])?;
        command(host, WRITE_INQUIRY_SCAN_TYPE, &[1])?;
        command(host, WRITE_CLASS_OF_DEVICE, &[0x00, 0x04, 0x48])?;
        command(host, WRITE_PAGE_TIMEOUT, &[0x00, 0x80])?;
        let mut name = [0u8; 248];
        name[..3].copy_from_slice(b"Wii");
        command(host, WRITE_LOCAL_NAME, &name)?;
        let mut events = [0u8; ogc_bt_sync::MAX_EVENT_PACKET_BYTES];
        map_error(host.set_scan_enable(ScanEnable::Page, &mut events))
    });
    complete(cb, result.map_or_else(|err| err, |_| ERR_OK))
}

pub(crate) fn apply_patch(cb: BteCallback) -> i32 {
    let result = host().ok_or(ERR_IF).and_then(|host| {
        command(host, VENDOR_PATCH_START, &[0])?;
        command(host, VENDOR_PATCH_CONTINUE, &PATCH0)?;
        command(host, VENDOR_PATCH_END, &PATCH1)?;
        let mut events = [0u8; ogc_bt_sync::MAX_EVENT_PACKET_BYTES];
        map_error(host.reset(&mut events))
    });
    complete(cb, result.map_or_else(|err| err, |_| ERR_OK))
}

pub(crate) fn set_evt_filter(
    filter_type: u8,
    filter_cond_type: u8,
    cond: *mut u8,
    cb: BteCallback,
) -> i32 {
    let cond_len = match (filter_type, filter_cond_type) {
        (0, 0) => 0,
        (1, 0) => 0,
        (1, 1 | 2) => 6,
        (2, 0) => 1,
        (2, 1 | 2) => 7,
        _ => return complete(cb, crate::ERR_VAL),
    };
    if cond_len != 0 && cond.is_null() {
        return complete(cb, ERR_ARG);
    }
    let condition = if cond_len == 0 {
        &[][..]
    } else {
        unsafe { core::slice::from_raw_parts(cond, cond_len) }
    };
    let mut params = [0u8; 9];
    params[0] = filter_type;
    params[1] = filter_cond_type;
    params[2..2 + cond_len].copy_from_slice(condition);
    let result = host()
        .ok_or(ERR_IF)
        .and_then(|host| command(host, SET_EVENT_FILTER, &params[..2 + cond_len]));
    complete(cb, result.map_or_else(|err| err, |_| ERR_OK))
}

fn ensure_conf() -> Result<(), i32> {
    let rc = conf::init();
    if rc < 0 && rc != conf::CONF_ENOTINIT {
        Err(rc)
    } else {
        Ok(())
    }
}

fn load_guest_keys() -> Result<ConfPadGuests, i32> {
    ensure_conf()?;
    let mut guests = ConfPadGuests::default();
    let rc = conf::get_pad_guest_devices(&mut guests);
    if rc < 0 {
        return Err(rc);
    }
    Ok(guests)
}

fn save_guest_keys(guests: &ConfPadGuests) -> Result<(), i32> {
    let rc = conf::set_pad_guest_devices(Some(guests));
    if rc < 0 {
        return Err(rc);
    }
    let rc = conf::save_changes();
    if rc < 0 {
        return Err(rc);
    }
    Ok(())
}

fn guest_at(guests: &ConfPadGuests, index: usize) -> ConfPadGuestDevice {
    if index < 4 {
        guests.guests[index]
    } else {
        guests.unknown[index - 4]
    }
}

fn guest_at_mut(guests: &mut ConfPadGuests, index: usize) -> &mut ConfPadGuestDevice {
    if index < 4 {
        &mut guests.guests[index]
    } else {
        &mut guests.unknown[index - 4]
    }
}

fn empty_guest(guest: &ConfPadGuestDevice) -> bool {
    guest.bdaddr == [0; 6]
}

fn bd_address(value: &BdAddr) -> BluetoothAddress {
    BluetoothAddress::new(value.addr)
}

fn find_guest(guests: &ConfPadGuests, address: [u8; 6]) -> Option<usize> {
    (0..GUEST_SLOTS).find(|index| guest_at(guests, *index).bdaddr == address)
}

fn stored_key(address: BluetoothAddress) -> Option<[u8; 16]> {
    let guests = load_guest_keys().ok()?;
    let index = find_guest(&guests, address.bytes)?;
    Some(guest_at(&guests, index).link_key)
}

fn transport_for(pcb: *mut crate::BtePcb) -> Option<&'static mut PcbTransport> {
    let index = crate::pcb_index(pcb)?;
    (index < crate::PCB_SLOTS).then(|| unsafe { &mut PCB_TRANSPORTS[index] })
}

pub(crate) fn free_transport(pcb: *mut crate::BtePcb) {
    if let Some(index) = crate::pcb_index(pcb)
        && index < crate::PCB_SLOTS
    {
        unsafe {
            PCB_TRANSPORTS[index] = PcbTransport::new();
        }
    }
}

fn start_connection_worker() -> Result<(), i32> {
    if CONNECTION_WORKER_RUNNING.load(Ordering::Acquire) {
        return Ok(());
    }
    let semaphore = core::ptr::addr_of_mut!(CONNECTION_WORKER_SEMAPHORE);
    if unsafe { ogc_lwp_driver::init_semaphore(semaphore, 0, 1) } != ogc_lwp_driver::LWP_SUCCESSFUL
    {
        return Err(ERR_IF);
    }
    CONNECTION_TASK.store(CONNECTION_TASK_IDLE, Ordering::Release);
    CONNECTION_WORKER_RUNNING.store(true, Ordering::Release);
    let result = unsafe {
        ogc_lwp_driver::create_thread(
            core::ptr::addr_of_mut!(CONNECTION_WORKER),
            Some(connection_worker_thread),
            core::ptr::null_mut(),
            core::ptr::addr_of_mut!(CONNECTION_WORKER_STACK)
                .cast::<u8>()
                .cast(),
            CONNECTION_WORKER_STACK_SIZE as u32,
            64,
        )
    };
    if result != ogc_lwp_driver::LWP_SUCCESSFUL {
        CONNECTION_WORKER_RUNNING.store(false, Ordering::Release);
        let sem = unsafe { core::ptr::read(semaphore) };
        if ogc_lwp_driver::close_semaphore(sem) != ogc_lwp_driver::LWP_SUCCESSFUL {
            return Err(ERR_IF);
        }
        unsafe { core::ptr::write(semaphore, ogc_lwp_driver::LWP_SEM_NULL) };
        return Err(ERR_IF);
    }
    Ok(())
}

fn stop_connection_worker() -> Result<(), i32> {
    let was_running = CONNECTION_WORKER_RUNNING.swap(false, Ordering::AcqRel);
    let thread = unsafe { core::ptr::read(core::ptr::addr_of!(CONNECTION_WORKER)) };
    if thread == ogc_lwp_driver::LWP_THREAD_NULL {
        return Ok(());
    }
    let sem = unsafe { core::ptr::read(core::ptr::addr_of!(CONNECTION_WORKER_SEMAPHORE)) };
    if was_running && ogc_lwp_driver::semaphore_post(sem) != ogc_lwp_driver::LWP_SUCCESSFUL {
        CONNECTION_WORKER_RUNNING.store(true, Ordering::Release);
        return Err(ERR_IF);
    }
    if unsafe { ogc_lwp_driver::join_thread(thread, core::ptr::null_mut()) }
        != ogc_lwp_driver::LWP_SUCCESSFUL
    {
        return Err(ERR_IF);
    }
    if ogc_lwp_driver::close_semaphore(sem) != ogc_lwp_driver::LWP_SUCCESSFUL {
        return Err(ERR_IF);
    }
    unsafe {
        core::ptr::write(
            core::ptr::addr_of_mut!(CONNECTION_WORKER),
            ogc_lwp_driver::LWP_THREAD_NULL,
        );
        core::ptr::write(
            core::ptr::addr_of_mut!(CONNECTION_WORKER_SEMAPHORE),
            ogc_lwp_driver::LWP_SEM_NULL,
        );
    }
    CONNECTION_TASK.store(CONNECTION_TASK_IDLE, Ordering::Release);
    Ok(())
}

fn reserve_connection_task(index: usize) -> Result<(), i32> {
    if !CONNECTION_WORKER_RUNNING.load(Ordering::Acquire) {
        return Err(ERR_IF);
    }
    try_reserve_connection_task(&CONNECTION_TASK)?;
    CONNECTION_TASK_INDEX.store(index as u32, Ordering::Relaxed);
    Ok(())
}

fn try_reserve_connection_task(task: &AtomicU32) -> Result<(), i32> {
    // The controller event endpoint has one consumer. Reject overlapping
    // setup requests until this backend has an event router for multiple PCBs.
    task.compare_exchange(
        CONNECTION_TASK_IDLE,
        CONNECTION_TASK_RESERVED,
        Ordering::AcqRel,
        Ordering::Acquire,
    )
    .map(|_| ())
    .map_err(|_| ERR_CONN)
}

fn activate_connection_task(mode: u32) -> Result<(), i32> {
    CONNECTION_TASK.store(mode, Ordering::Release);
    let sem = unsafe { core::ptr::read(core::ptr::addr_of!(CONNECTION_WORKER_SEMAPHORE)) };
    if ogc_lwp_driver::semaphore_post(sem) == ogc_lwp_driver::LWP_SUCCESSFUL {
        Ok(())
    } else {
        CONNECTION_TASK.store(CONNECTION_TASK_IDLE, Ordering::Release);
        Err(ERR_IF)
    }
}

unsafe extern "C" fn connection_worker_thread(_: *mut c_void) -> *mut c_void {
    let sem = unsafe { core::ptr::read(core::ptr::addr_of!(CONNECTION_WORKER_SEMAPHORE)) };
    while CONNECTION_WORKER_RUNNING.load(Ordering::Acquire) {
        if ogc_lwp_driver::semaphore_wait(sem) != ogc_lwp_driver::LWP_SUCCESSFUL {
            let mode = CONNECTION_TASK.swap(CONNECTION_TASK_RESERVED, Ordering::AcqRel);
            let index = CONNECTION_TASK_INDEX.load(Ordering::Acquire) as usize;
            if matches!(
                mode,
                CONNECTION_TASK_CONNECT
                    | CONNECTION_TASK_LISTEN
                    | CONNECTION_TASK_CONNECT_STEP2
                    | CONNECTION_TASK_LISTEN_STEP2
            ) {
                finish_connection_task(mode, index, Err(ERR_IF));
            }
            CONNECTION_WORKER_RUNNING.store(false, Ordering::Release);
            break;
        }
        if !CONNECTION_WORKER_RUNNING.load(Ordering::Acquire) {
            break;
        }
        let mode = CONNECTION_TASK.swap(CONNECTION_TASK_RESERVED, Ordering::AcqRel);
        let index = CONNECTION_TASK_INDEX.load(Ordering::Acquire) as usize;
        if matches!(
            mode,
            CONNECTION_TASK_CONNECT
                | CONNECTION_TASK_LISTEN
                | CONNECTION_TASK_CONNECT_STEP2
                | CONNECTION_TASK_LISTEN_STEP2
        ) {
            run_connection_task(mode, index);
        } else {
            CONNECTION_TASK.store(CONNECTION_TASK_IDLE, Ordering::Release);
        }
    }
    CONNECTION_WORKER_RUNNING.store(false, Ordering::Release);
    core::ptr::null_mut()
}

// BUG FIX: the C async entry points return after request submission. This
// worker owns the later HCI/L2CAP waits and reports their final result by callback.
fn run_connection_task(mode: u32, index: usize) {
    let Some(pcb) = crate::pcb_for_index(index) else {
        CONNECTION_TASK.store(CONNECTION_TASK_IDLE, Ordering::Release);
        return;
    };
    let transport = unsafe {
        &mut *core::ptr::addr_of_mut!(PCB_TRANSPORTS)
            .cast::<PcbTransport>()
            .add(index)
    };
    let result = host().ok_or(ERR_IF).and_then(|host| match mode {
        CONNECTION_TASK_CONNECT => wait_for_baseband_connection(host, transport, pcb, false)
            .and_then(|_| configure_l2cap_channel(host, transport, true, false)),
        CONNECTION_TASK_LISTEN => wait_for_baseband_connection(host, transport, pcb, true)
            .and_then(|_| configure_l2cap_channel(host, transport, true, true)),
        CONNECTION_TASK_CONNECT_STEP2 => configure_l2cap_channel(host, transport, false, false)
            .and_then(|_| ensure_acl_dispatcher()),
        CONNECTION_TASK_LISTEN_STEP2 => configure_l2cap_channel(host, transport, false, true)
            .and_then(|_| ensure_acl_dispatcher()),
        _ => Err(ERR_ARG),
    });
    finish_connection_task(mode, index, result);
}

fn finish_connection_task(mode: u32, index: usize, result: Result<(), i32>) {
    let Some(pcb) = crate::pcb_for_index(index) else {
        CONNECTION_TASK.store(CONNECTION_TASK_IDLE, Ordering::Release);
        return;
    };
    if matches!(mode, CONNECTION_TASK_CONNECT | CONNECTION_TASK_LISTEN) {
        unsafe {
            (*pcb).state = if result.is_ok() {
                crate::HIDP_STATE_CONNECTED
            } else {
                crate::HIDP_STATE_READY
            };
        }
        if result.is_err() {
            let transport = unsafe {
                &mut *core::ptr::addr_of_mut!(PCB_TRANSPORTS)
                    .cast::<PcbTransport>()
                    .add(index)
            };
            transport.handle = 0;
            transport.listening = false;
        }
    }
    let callback = unsafe { (*pcb).conn_cfm };
    let error = result.err().unwrap_or(ERR_OK) as u8;
    CONNECTION_TASK.store(CONNECTION_TASK_IDLE, Ordering::Release);
    if let Some(callback) = callback {
        unsafe { callback((*pcb).cbarg, pcb, error) };
    }
}

#[cfg(test)]
mod connection_worker_tests {
    use super::{CONNECTION_TASK_IDLE, ERR_CONN, try_reserve_connection_task};
    use core::sync::atomic::AtomicU32;
    use core::sync::atomic::Ordering;

    #[test]
    fn setup_task_slot_rejects_overlap_and_reopens_after_completion() {
        let task = AtomicU32::new(CONNECTION_TASK_IDLE);

        assert_eq!(try_reserve_connection_task(&task), Ok(()));
        assert_eq!(try_reserve_connection_task(&task), Err(ERR_CONN));

        task.store(CONNECTION_TASK_IDLE, Ordering::Release);
        assert_eq!(try_reserve_connection_task(&task), Ok(()));
    }
}

fn ensure_acl_dispatcher() -> Result<(), i32> {
    if ACL_DISPATCH_RUNNING.load(Ordering::Acquire) {
        return Ok(());
    }
    if ACL_DISPATCH_RUNNING
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return Ok(());
    }
    let result = unsafe {
        ogc_lwp_driver::create_thread(
            core::ptr::addr_of_mut!(ACL_DISPATCH_THREAD),
            Some(acl_dispatch_thread),
            core::ptr::null_mut(),
            core::ptr::addr_of_mut!(ACL_DISPATCH_STACK)
                .cast::<u8>()
                .cast(),
            ACL_DISPATCH_STACK_SIZE as u32,
            64,
        )
    };
    if result != ogc_lwp_driver::LWP_SUCCESSFUL {
        ACL_DISPATCH_RUNNING.store(false, Ordering::Release);
        return Err(ERR_IF);
    }
    Ok(())
}

unsafe extern "C" fn acl_dispatch_thread(_: *mut c_void) -> *mut c_void {
    let mut packet = [0u8; 1024];
    while ACL_DISPATCH_RUNNING.load(Ordering::Acquire) && HOST_READY.load(Ordering::Acquire) {
        let Some(host) = host() else {
            break;
        };
        match host.read_acl(&mut packet) {
            Ok(used) => dispatch_acl_packet(&packet[..used]),
            Err(_) => yield_thread(),
        }
    }
    ACL_DISPATCH_RUNNING.store(false, Ordering::Release);
    core::ptr::null_mut()
}

fn dispatch_acl_packet(packet: &[u8]) {
    let Ok(acl) = ogc_bt_sync::parse_acl_packet(packet) else {
        return;
    };
    if acl.packet_boundary == 0 {
        return;
    }
    let Ok(l2cap) = ogc_bt_sync::parse_l2cap_packet(acl.payload) else {
        return;
    };
    let Some(index) = (0..crate::PCB_SLOTS).find(|index| unsafe {
        let transport = core::ptr::addr_of!(PCB_TRANSPORTS)
            .cast::<PcbTransport>()
            .add(*index);
        (*transport).handle == acl.handle
    }) else {
        return;
    };
    let transport = unsafe {
        *core::ptr::addr_of!(PCB_TRANSPORTS)
            .cast::<PcbTransport>()
            .add(index)
    };
    if l2cap.channel_id == L2CAP_CID_SIGNALING {
        dispatch_l2cap_signaling(index, acl.handle, l2cap.payload);
        return;
    }
    if !transport.l2cap.ready() {
        return;
    }
    let pcb = crate::pcb_for_index(index);
    match l2cap.channel_id {
        cid if cid == transport.l2cap.control.remote_cid => {
            if crate::is_successful_hidp_handshake(l2cap.payload)
                && let Some(pcb) = pcb
            {
                complete_control_request(index, pcb, ERR_OK);
            }
        }
        cid if cid == transport.l2cap.interrupt.remote_cid => {
            let Ok(hidp) = ogc_bt_sync::parse_hidp_packet(l2cap.payload) else {
                return;
            };
            if hidp.transaction == ogc_bt_sync::HIDP_TRANS_DATA
                && hidp.parameter == ogc_bt_sync::HIDP_DATA_RTYPE_INPUT
                && let Some(pcb) = pcb
            {
                unsafe { crate::deliver_data(pcb, hidp.payload) };
            }
        }
        _ => {}
    }
}

fn dispatch_l2cap_signaling(index: usize, handle: u16, payload: &[u8]) {
    let mut signals = payload;
    while let Ok(Some(signal)) = ogc_bt_sync::parse_next_l2cap_signal(&mut signals) {
        match ogc_bt_sync::parse_l2cap_signal_payload(signal) {
            Ok(L2capSignalPayload::DisconnectionResponse {
                dest_cid,
                source_cid,
            }) => {
                let Some(last_channel) = accept_disconnect_response(
                    index,
                    handle,
                    signal.identifier,
                    source_cid,
                    dest_cid,
                ) else {
                    continue;
                };
                if last_channel {
                    let _ = finish_l2cap_disconnect(index, true, true);
                } else if send_next_l2cap_disconnect(index).is_err() {
                    abort_local_disconnect(index, ERR_IF);
                }
            }
            Ok(L2capSignalPayload::DisconnectionRequest {
                dest_cid,
                source_cid,
            }) => {
                let closed_both = unsafe {
                    let transport = core::ptr::addr_of_mut!(PCB_TRANSPORTS)
                        .cast::<PcbTransport>()
                        .add(index);
                    close_remote_channel(&mut (*transport).l2cap, dest_cid, source_cid)
                };
                let Some(last_channel) = closed_both else {
                    continue;
                };

                let mut response = [0u8; 8];
                response[0] = 0x07; // L2CAP_DISCONN_RSP
                response[1] = signal.identifier;
                response[2] = 4;
                response[4..6].copy_from_slice(&dest_cid.to_le_bytes());
                response[6..8].copy_from_slice(&source_cid.to_le_bytes());
                let mut scratch = [0u8; 32];
                if let Some(host) = host() {
                    let _ = host.send_l2cap_hid_data(
                        &mut scratch,
                        handle,
                        L2CAP_CID_SIGNALING,
                        &response,
                    );
                }

                if disconnect_is_pending(index) {
                    let mut closed_pending_channel = false;
                    unsafe {
                        let transport = core::ptr::addr_of_mut!(PCB_TRANSPORTS)
                            .cast::<PcbTransport>()
                            .add(index);
                        if (*transport).disconnect_local_cid == dest_cid {
                            closed_pending_channel = true;
                            (*transport).disconnect_signal_id = 0;
                            (*transport).disconnect_local_cid = 0;
                            (*transport).disconnect_remote_cid = 0;
                        }
                    }
                    if last_channel {
                        let _ = finish_l2cap_disconnect(index, true, true);
                    } else if closed_pending_channel && send_next_l2cap_disconnect(index).is_err() {
                        abort_local_disconnect(index, ERR_IF);
                    }
                } else if last_channel {
                    finish_l2cap_disconnect(index, false, true);
                }
            }
            _ => continue,
        }
    }
}

fn close_remote_channel(
    channels: &mut HidL2capState,
    local_cid: u16,
    remote_cid: u16,
) -> Option<bool> {
    let channel = if channels.control.local_cid == local_cid {
        &mut channels.control
    } else if channels.interrupt.local_cid == local_cid {
        &mut channels.interrupt
    } else {
        return None;
    };
    if !channel.active || channel.remote_cid != remote_cid {
        return None;
    }
    *channel = ogc_bt_sync::L2capHidChannelState::new(channel.psm, channel.local_cid);
    Some(!channels.control.active && !channels.interrupt.active)
}

fn accept_disconnect_response(
    index: usize,
    handle: u16,
    signal_id: u8,
    local_cid: u16,
    remote_cid: u16,
) -> Option<bool> {
    if index >= crate::PCB_SLOTS {
        return None;
    }
    let transport = unsafe {
        core::ptr::addr_of_mut!(PCB_TRANSPORTS)
            .cast::<PcbTransport>()
            .add(index)
    };
    if unsafe { (*transport).handle != handle } {
        return None;
    }
    unsafe { accept_disconnection_response(&mut *transport, signal_id, remote_cid, local_cid) }
}

fn build_disconnection_request(signal_id: u8, dest_cid: u16, source_cid: u16) -> [u8; 8] {
    let mut request = [0u8; 8];
    request[0] = ogc_bt_sync::L2CAP_DISCONN_REQ;
    request[1] = signal_id;
    request[2..4].copy_from_slice(&4u16.to_le_bytes());
    request[4..6].copy_from_slice(&dest_cid.to_le_bytes());
    request[6..8].copy_from_slice(&source_cid.to_le_bytes());
    request
}

fn accept_disconnection_response(
    transport: &mut PcbTransport,
    signal_id: u8,
    remote_cid: u16,
    local_cid: u16,
) -> Option<bool> {
    if !transport.disconnecting
        || transport.disconnect_signal_id != signal_id
        || transport.disconnect_local_cid != local_cid
        || transport.disconnect_remote_cid != remote_cid
    {
        return None;
    }
    let closed_both = close_remote_channel(&mut transport.l2cap, local_cid, remote_cid)?;
    transport.disconnect_signal_id = 0;
    transport.disconnect_local_cid = 0;
    transport.disconnect_remote_cid = 0;
    Some(closed_both)
}

#[cfg(feature = "dolphin-automation")]
pub(crate) fn dolphin_disconnect_response_order_probe() -> bool {
    disconnect_response_order_matches(&mut PcbTransport::new())
}

#[cfg(any(feature = "dolphin-automation", test))]
fn disconnect_response_order_matches(transport: &mut PcbTransport) -> bool {
    transport.handle = 3;
    transport.disconnecting = true;
    transport.disconnect_signal_id = 9;
    transport.disconnect_local_cid = 0x0041;
    transport.disconnect_remote_cid = 0x0061;
    transport.l2cap.interrupt.active = true;
    transport.l2cap.interrupt.local_cid = 0x0041;
    transport.l2cap.interrupt.remote_cid = 0x0061;
    transport.l2cap.control.active = true;
    transport.l2cap.control.local_cid = 0x0040;
    transport.l2cap.control.remote_cid = 0x0060;

    let mut failed_pcb = crate::BtePcb::default();
    record_disconnect_send_error(&mut failed_pcb, ERR_IF);
    if failed_pcb.state != crate::HIDP_STATE_DISCONNECTING || failed_pcb.err != ERR_IF as u8 {
        return false;
    }

    if build_disconnection_request(9, 0x0061, 0x0041) != [0x06, 9, 4, 0, 0x61, 0, 0x41, 0] {
        return false;
    }
    if accept_disconnection_response(transport, 8, 0x0061, 0x0041).is_some()
        || !transport.l2cap.interrupt.active
    {
        return false;
    }
    if accept_disconnection_response(transport, 9, 0x0061, 0x0041) != Some(false)
        || transport.l2cap.interrupt.active
        || !transport.l2cap.control.active
        || !transport.disconnecting
    {
        return false;
    }

    transport.disconnect_signal_id = 10;
    transport.disconnect_local_cid = 0x0040;
    transport.disconnect_remote_cid = 0x0060;
    accept_disconnection_response(transport, 10, 0x0060, 0x0040) == Some(true)
        && !transport.l2cap.control.active
        && transport.disconnecting
}

fn abort_local_disconnect(index: usize, error: i32) {
    if index >= crate::PCB_SLOTS {
        return;
    }
    if let Some(pcb) = crate::pcb_for_index(index) {
        unsafe { record_disconnect_send_error(&mut *pcb, error) };
    }
}

fn record_disconnect_send_error(pcb: &mut crate::BtePcb, error: i32) {
    pcb.err = error as u8;
    pcb.state = crate::HIDP_STATE_DISCONNECTING;
}

fn control_request_alloc(
    payload: &[u8],
    callback: crate::BteConnCfmCallback,
    synchronous: bool,
) -> Option<u16> {
    if payload.is_empty() || payload.len() > MAX_HIDP_PAYLOAD_SIZE {
        return None;
    }
    let irq = critical_enter();
    let slot = unsafe {
        let mut found = None;
        for slot in 0..CONTROL_REQUEST_COUNT {
            let request = &mut *control_request_ptr(slot);
            if !request.used {
                *request = ControlRequest::EMPTY;
                request.used = true;
                request.len = payload.len() as u16;
                request.callback = callback;
                request.synchronous = synchronous;
                request.payload[..payload.len()].copy_from_slice(payload);
                found = Some(slot as u16);
                break;
            }
        }
        found
    };
    critical_exit(irq);
    slot
}

fn control_request_enqueue(pcb_index: usize, slot: u16) -> bool {
    let irq = critical_enter();
    let should_send = unsafe {
        let tail = *control_tail_ptr(pcb_index);
        if tail == CONTROL_NONE {
            *control_head_ptr(pcb_index) = slot;
            *control_tail_ptr(pcb_index) = slot;
            true
        } else {
            (*control_request_ptr(tail as usize)).next = slot;
            *control_tail_ptr(pcb_index) = slot;
            false
        }
    };
    critical_exit(irq);
    should_send
}

fn control_request_pop_head(pcb_index: usize) -> Option<u16> {
    let irq = critical_enter();
    let slot = unsafe {
        let head = *control_head_ptr(pcb_index);
        if head == CONTROL_NONE {
            None
        } else {
            let request = &mut *control_request_ptr(head as usize);
            let next = request.next;
            *control_head_ptr(pcb_index) = next;
            if next == CONTROL_NONE {
                *control_tail_ptr(pcb_index) = CONTROL_NONE;
            }
            request.next = CONTROL_NONE;
            Some(head)
        }
    };
    critical_exit(irq);
    slot
}

fn control_request_free(slot: u16) {
    let irq = critical_enter();
    unsafe { *control_request_ptr(slot as usize) = ControlRequest::EMPTY };
    critical_exit(irq);
}

fn control_request_result(slot: u16) -> Option<i32> {
    let irq = critical_enter();
    let result = unsafe {
        let request = &*control_request_ptr(slot as usize);
        (request.used && request.state == CONTROL_COMPLETE).then_some(request.result)
    };
    critical_exit(irq);
    result
}

fn drain_pending_control_requests(
    mut pop_next: impl FnMut() -> Option<u16>,
    mut finish: impl FnMut(u16, i32),
    result: i32,
) {
    while let Some(slot) = pop_next() {
        finish(slot, result);
    }
}

fn send_control_head(index: usize, pcb: *mut crate::BtePcb) -> i32 {
    let irq = critical_enter();
    let prepared = unsafe {
        let head = *control_head_ptr(index);
        if head == CONTROL_NONE {
            None
        } else {
            let request = &mut *control_request_ptr(head as usize);
            if !request.used || request.state != CONTROL_READY {
                None
            } else {
                request.state = CONTROL_SENDING;
                Some((head, request.payload, request.len as usize))
            }
        }
    };
    critical_exit(irq);
    let Some((slot, payload, len)) = prepared else {
        return ERR_OK;
    };

    let result = transport_for(pcb).map_or(ERR_ARG, |transport| {
        hid_report_route(&transport.l2cap, HidReportPath::ControlSetReport).map_or(
            ERR_CONN,
            |(remote_cid, _)| {
                host().map_or(ERR_IF, |host| {
                    host.send_l2cap_hid_data(
                        &mut [0u8; MAX_ACL_PACKET_SIZE],
                        transport.handle,
                        remote_cid,
                        &payload[..len],
                    )
                    .map_or(ERR_IF, |_| ERR_OK)
                })
            },
        )
    });
    if result == ERR_OK {
        return ERR_OK;
    }

    let popped = control_request_pop_head(index);
    debug_assert_eq!(popped, Some(slot));
    let irq = critical_enter();
    let (callback, synchronous) = unsafe {
        let request = &mut *control_request_ptr(slot as usize);
        request.result = result;
        request.state = CONTROL_COMPLETE;
        (request.callback, request.synchronous)
    };
    critical_exit(irq);
    if let Some(callback) = callback {
        unsafe { callback((*pcb).cbarg, pcb, result as u8) };
    }
    if !synchronous {
        control_request_free(slot);
    }
    result
}

fn complete_control_request(index: usize, pcb: *mut crate::BtePcb, result: i32) {
    let Some(slot) = control_request_pop_head(index) else {
        return;
    };
    let irq = critical_enter();
    let (callback, synchronous) = unsafe {
        let request = &mut *control_request_ptr(slot as usize);
        request.result = result;
        request.state = CONTROL_COMPLETE;
        (request.callback, request.synchronous)
    };
    critical_exit(irq);

    if let Some(callback) = callback {
        unsafe { callback((*pcb).cbarg, pcb, result as u8) };
    }
    if !synchronous {
        control_request_free(slot);
    }

    if result == ERR_OK && !disconnect_is_pending(index) {
        let _ = send_control_head(index, pcb);
    } else if result != ERR_OK {
        // C closes every queued request when the PCB disconnects.
        drain_pending_control_requests(
            || control_request_pop_head(index),
            |next, result| {
                let irq = critical_enter();
                let (callback, synchronous) = unsafe {
                    let request = &mut *control_request_ptr(next as usize);
                    request.result = result;
                    request.state = CONTROL_COMPLETE;
                    (request.callback, request.synchronous)
                };
                critical_exit(irq);
                if let Some(callback) = callback {
                    unsafe { callback((*pcb).cbarg, pcb, result as u8) };
                }
                if !synchronous {
                    control_request_free(next);
                }
            },
            result,
        );
    }
}

fn disconnect_is_pending(index: usize) -> bool {
    if index >= crate::PCB_SLOTS {
        return false;
    }
    unsafe {
        (*core::ptr::addr_of!(PCB_TRANSPORTS)
            .cast::<PcbTransport>()
            .add(index))
        .disconnecting
    }
}

fn send_next_l2cap_disconnect(index: usize) -> Result<bool, i32> {
    if index >= crate::PCB_SLOTS {
        return Err(ERR_ARG);
    }
    let transport = unsafe {
        core::ptr::addr_of_mut!(PCB_TRANSPORTS)
            .cast::<PcbTransport>()
            .add(index)
    };
    let current = unsafe { *transport };
    if !current.disconnecting {
        return Err(ERR_CONN);
    }
    let channel = if current.l2cap.interrupt.active {
        current.l2cap.interrupt
    } else if current.l2cap.control.active {
        current.l2cap.control
    } else {
        unsafe {
            (*transport).disconnect_signal_id = 0;
            (*transport).disconnect_local_cid = 0;
            (*transport).disconnect_remote_cid = 0;
        }
        return Ok(false);
    };

    let signal_id = current.disconnect_signal_id.wrapping_add(1).max(1);
    unsafe {
        (*transport).disconnect_signal_id = signal_id;
        (*transport).disconnect_local_cid = channel.local_cid;
        (*transport).disconnect_remote_cid = channel.remote_cid;
    }
    let request = build_disconnection_request(signal_id, channel.remote_cid, channel.local_cid);
    let mut scratch = [0u8; 32];
    let result = host()
        .ok_or(ERR_IF)?
        .send_l2cap_hid_data(&mut scratch, current.handle, L2CAP_CID_SIGNALING, &request)
        .map_err(|_| ERR_IF);
    if let Err(error) = result {
        unsafe {
            if (*transport).disconnect_signal_id == signal_id {
                (*transport).disconnect_signal_id = 0;
                (*transport).disconnect_local_cid = 0;
                (*transport).disconnect_remote_cid = 0;
            }
        }
        return Err(error);
    }
    Ok(true)
}

fn finish_l2cap_disconnect(index: usize, send_hci_disconnect: bool, notify: bool) -> i32 {
    if index >= crate::PCB_SLOTS {
        return ERR_ARG;
    }
    let transport = unsafe {
        core::ptr::addr_of_mut!(PCB_TRANSPORTS)
            .cast::<PcbTransport>()
            .add(index)
    };
    let handle = unsafe { (*transport).handle };
    unsafe {
        (*transport).disconnecting = false;
        (*transport).disconnect_signal_id = 0;
        (*transport).disconnect_local_cid = 0;
        (*transport).disconnect_remote_cid = 0;
    }
    if let Some(pcb) = crate::pcb_for_index(index) {
        unsafe {
            (*pcb).err = crate::ERR_OK as u8;
            (*pcb).state = crate::HIDP_STATE_READY;
        }
        complete_control_request(index, pcb, crate::ERR_CLSD);
        if notify {
            unsafe {
                if let Some(callback) = (*pcb).disconn_cfm {
                    callback((*pcb).cbarg, pcb, crate::ERR_OK as u8);
                }
            }
        }
    }
    if !send_hci_disconnect || handle == 0 {
        return ERR_OK;
    }
    let params = [handle as u8, (handle >> 8) as u8, 0x16];
    host().map_or(ERR_IF, |host| {
        host.send_hci_command(HciOpcode::DISCONNECT, &params)
            .map_or(ERR_IF, |_| ERR_OK)
    })
}

fn store_key(address: &BdAddr, key: *const u8) -> i32 {
    if key.is_null() {
        return ERR_ARG;
    }
    let Ok(mut guests) = load_guest_keys() else {
        return ERR_IF;
    };
    let index = find_guest(&guests, address.addr)
        .or_else(|| (0..GUEST_SLOTS).find(|index| empty_guest(&guest_at(&guests, *index))));
    let Some(index) = index else {
        return ERR_IF;
    };
    let guest = guest_at_mut(&mut guests, index);
    guest.bdaddr = address.addr;
    unsafe {
        guest
            .link_key
            .copy_from_slice(core::slice::from_raw_parts(key, BD_LINK_KEY_LEN));
    }
    if index < 4 && guests.num_guests <= index as u8 {
        guests.num_guests = index as u8 + 1;
    }
    if save_guest_keys(&guests).is_err() {
        ERR_IF
    } else {
        ERR_OK
    }
}

pub(crate) unsafe fn read_stored_link_key(
    keys: *mut LinkKeyInfo,
    max_cnt: u8,
    cb: BteCallback,
) -> i32 {
    if max_cnt != 0 && keys.is_null() {
        return complete(cb, ERR_ARG);
    }
    let result = load_guest_keys().map(|guests| {
        let mut count = 0usize;
        for index in 0..GUEST_SLOTS {
            if count == max_cnt as usize {
                break;
            }
            let guest = guest_at(&guests, index);
            if empty_guest(&guest) {
                continue;
            }
            unsafe {
                let output = &mut *keys.add(count);
                output.bdaddr = BdAddr { addr: guest.bdaddr };
                output.key = guest.link_key;
            }
            count += 1;
        }
        ERR_OK
    });
    complete(cb, result.unwrap_or(ERR_IF))
}

pub(crate) unsafe fn write_stored_link_key(address: *mut BdAddr, key: *mut u8) -> i32 {
    unsafe {
        if address.is_null() {
            return ERR_ARG;
        }
        store_key(&*address, key.cast_const())
    }
}

pub(crate) fn delete_stored_link_key(address: *mut BdAddr) -> i32 {
    if address.is_null() {
        return ERR_ARG;
    }
    let Ok(mut guests) = load_guest_keys() else {
        return ERR_IF;
    };
    let Some(index) = find_guest(&guests, unsafe { (*address).addr }) else {
        return ERR_OK;
    };
    *guest_at_mut(&mut guests, index) = ConfPadGuestDevice::default();
    if index < 4 && guests.num_guests > 0 {
        guests.num_guests -= 1;
    }
    save_guest_keys(&guests).map_or(ERR_IF, |_| ERR_OK)
}

pub(crate) fn clear_stored_link_keys() -> i32 {
    let Ok(mut guests) = load_guest_keys() else {
        return ERR_IF;
    };
    guests.num_guests = 0;
    guests.guests = [ConfPadGuestDevice::default(); 4];
    guests.unknown = [ConfPadGuestDevice::default(); 2];
    save_guest_keys(&guests).map_or(ERR_IF, |_| ERR_OK)
}

pub(crate) unsafe fn read_bd_addr(output: *mut BdAddr, cb: BteCallback) -> i32 {
    if output.is_null() {
        return complete(cb, ERR_ARG);
    }
    let mut events = [0u8; ogc_bt_sync::MAX_EVENT_PACKET_BYTES];
    let result = host()
        .ok_or(ERR_IF)
        .and_then(|host| map_error(host.read_bd_addr(&mut events)))
        .map(|address| unsafe {
            *output = BdAddr {
                addr: address.bytes,
            }
        });
    complete(cb, result.map_or(ERR_IF, |_| ERR_OK))
}

pub(crate) fn inquiry(max_cnt: u8, _flush: u8, cb: BteCallback) -> i32 {
    let mut events = [0u8; ogc_bt_sync::MAX_EVENT_PACKET_BYTES];
    let mut results = [ogc_bt_sync::InquiryResult {
        address: BluetoothAddress { bytes: [0; 6] },
        class_of_device: ogc_bt_sync::ClassOfDevice { bytes: [0; 3] },
        page_scan_rep_mode: 0,
        page_scan_mode: 0,
        clock_offset: 0,
    }; ogc_bt_sync::MAX_INQUIRY_RESULTS];
    let result = host()
        .ok_or(ERR_IF)
        .and_then(|host| map_error(host.scan_inquiry(&mut events, &mut results)))
        .map(|count| count.min(max_cnt as usize));
    complete(cb, result.map_or(ERR_IF, |_| ERR_OK))
}

pub(crate) fn periodic_inquiry(max_cnt: u8, _flush: u8, cb: BteCallback) -> i32 {
    let result = host()
        .ok_or(ERR_IF)
        .and_then(|host| map_error(host.start_inquiry(ogc_bt_sync::INQUIRY_GIAC, 0x08, max_cnt)));
    if result.is_ok() {
        PAIR_MODE.store(PAIR_MODE_TEMPORARY as u32, Ordering::Release);
    }
    complete(cb, result.map_or(ERR_IF, |_| ERR_OK))
}

pub(crate) fn exit_periodic_inquiry() -> i32 {
    let result = host()
        .ok_or(ERR_IF)
        .and_then(|host| map_error(host.send_hci_command(INQUIRY_CANCEL, &[])));
    PAIR_MODE.store(PAIR_MODE_NORMAL as u32, Ordering::Release);
    result.map_or(ERR_IF, |_| ERR_OK)
}

pub(crate) unsafe fn link_key_request_reply(address: *mut BdAddr, key: *mut u8) -> i32 {
    unsafe {
        if address.is_null() {
            return ERR_ARG;
        }
        let result = store_key(&*address, key.cast_const());
        if result != ERR_OK {
            return result;
        }
        host()
            .ok_or(ERR_IF)
            .and_then(|host| {
                map_error(host.link_key_request_reply(bd_address(&*address), {
                    &*(key as *const [u8; BD_LINK_KEY_LEN])
                }))
            })
            .map_or(ERR_IF, |_| ERR_OK)
    }
}

pub(crate) fn link_key_request_negative_reply(address: *mut BdAddr) -> i32 {
    if address.is_null() {
        return ERR_ARG;
    }
    host()
        .ok_or(ERR_IF)
        .and_then(|host| {
            map_error(host.link_key_request_negative_reply(bd_address(unsafe { &*address })))
        })
        .map_or(ERR_IF, |_| ERR_OK)
}

pub(crate) fn get_pair_mode() -> u8 {
    PAIR_MODE.load(Ordering::Acquire) as u8
}

pub(crate) unsafe fn read_remote_name(padinfo: *mut PadNameInfo, cb: BteCallback) -> i32 {
    if padinfo.is_null() {
        return complete(cb, ERR_ARG);
    }
    let bdaddr = unsafe { (*padinfo).bdaddr };
    let mut events = [0u8; ogc_bt_sync::MAX_EVENT_PACKET_BYTES];
    let result = host().ok_or(ERR_IF).and_then(|host| {
        map_error(host.remote_name_request(bd_address(&bdaddr), 0, 0, 0))?;
        loop {
            let used = map_error(host.read_event(&mut events))?;
            match map_error(host.parse_event(&events[..used]))? {
                HciEvent::RemoteNameComplete { status, name, .. } => {
                    if status != 0 {
                        return Err(ERR_IF);
                    }
                    let output = unsafe { &mut *padinfo };
                    output.name.fill(0);
                    let count = name.len().min(output.name.len() - 1);
                    output.name[..count].copy_from_slice(&name[..count]);
                    return Ok(());
                }
                HciEvent::CommandStatus {
                    opcode: HciOpcode::REMOTE_NAME_REQUEST,
                    status,
                } if status != 0 => return Err(ERR_CONN),
                HciEvent::PinCodeRequest { address } => {
                    let _ = host.pin_code_request_reply(address, b"0000");
                }
                HciEvent::LinkKeyRequest { address } => {
                    let _ = host.link_key_request_negative_reply(address);
                }
                _ => {}
            }
        }
    });
    complete(cb, result.map_or(ERR_IF, |_| ERR_OK))
}

fn handle_pairing_event(host: &BtSyncHost, event: HciEvent<'_>) {
    match event {
        HciEvent::PinCodeRequest { address } => {
            let _ = host.pin_code_request_reply(address, b"0000");
        }
        HciEvent::LinkKeyRequest { address } => {
            let mut bdaddr = BdAddr {
                addr: address.bytes,
            };
            let callback_result = unsafe { crate::notify_link_key_request(&mut bdaddr) };
            if callback_result == 0 {
                let _ = host.link_key_request_negative_reply(address);
                return;
            }
            if let Some(key) = stored_key(address) {
                let _ = host.link_key_request_reply(address, &key);
            } else {
                let _ = host.link_key_request_negative_reply(address);
            }
        }
        HciEvent::LinkKeyNotification { address, key, .. } => {
            let mut bdaddr = BdAddr {
                addr: address.bytes,
            };
            let mut key_copy = key;
            let _ = store_key(&bdaddr, key_copy.as_ptr());
            let _ = unsafe { crate::notify_link_key_notification(&mut bdaddr, &mut key_copy) };
        }
        HciEvent::Unknown {
            event_code: ogc_bt_sync::HCI_EVENT_VENDOR_SPECIFIC,
            payload,
        } => match payload.first().copied() {
            Some(0x08) => unsafe { crate::notify_host_sync_button(0) },
            Some(0x09) => unsafe { crate::notify_host_sync_button(1) },
            _ => {}
        },
        _ => {}
    }
}

fn handle_connection_request(
    host: &BtSyncHost,
    transport: &mut PcbTransport,
    address: BluetoothAddress,
    class_of_device: [u8; 3],
    link_type: u8,
) -> Result<(), i32> {
    let mut bdaddr = BdAddr {
        addr: address.bytes,
    };
    let mut cod = class_of_device;
    let callback_result =
        unsafe { crate::notify_connection_request(&mut bdaddr, &mut cod, link_type) };
    if callback_result == 0 {
        let mut params = [0u8; 7];
        params[..6].copy_from_slice(&address.bytes);
        params[6] = 1;
        host.send_hci_command(HciOpcode::ACCEPT_CONNECTION_REQUEST, &params)
            .map_err(|_| ERR_IF)?;
        transport.address = address;
    } else {
        let mut params = [0u8; 7];
        params[..6].copy_from_slice(&address.bytes);
        params[6] = 0x0d;
        host.send_hci_command(HciOpcode::REJECT_CONNECTION_REQUEST, &params)
            .map_err(|_| ERR_IF)?;
    }
    Ok(())
}

fn configure_l2cap_channel(
    host: &BtSyncHost,
    transport: &mut PcbTransport,
    control: bool,
    incoming: bool,
) -> Result<(), i32> {
    let (psm, local_cid, signal_id, config_index) = if control {
        (L2CAP_PSM_HID_CTRL, L2CAP_HID_CTRL_LOCAL_CID, 1, 0)
    } else {
        (L2CAP_PSM_HID_INTR, L2CAP_HID_INTR_LOCAL_CID, 2, 1)
    };
    let mut scratch = [0u8; 128];

    if !incoming {
        host.send_l2cap_connection_request(
            &mut scratch,
            transport.handle,
            signal_id,
            psm,
            local_cid,
        )
        .map_err(|_| ERR_IF)?;
    }

    let start = now_ticks();
    let mut packet = [0u8; 1024];
    loop {
        let channel = if control {
            transport.l2cap.control
        } else {
            transport.l2cap.interrupt
        };
        if channel.ready() {
            return Ok(());
        }
        if now_ticks().wrapping_sub(start) >= TICKS_PER_SECOND * 5 {
            return Err(ERR_IF);
        }

        let used = host.read_acl(&mut packet).map_err(|_| ERR_IF)?;
        let acl = ogc_bt_sync::parse_acl_packet(&packet[..used]).map_err(|_| ERR_IF)?;
        if acl.handle != transport.handle || acl.packet_boundary == 0 {
            yield_thread();
            continue;
        }
        let l2cap = ogc_bt_sync::parse_l2cap_packet(acl.payload).map_err(|_| ERR_IF)?;
        if l2cap.channel_id != L2CAP_CID_SIGNALING {
            yield_thread();
            continue;
        }

        let mut signals = l2cap.payload;
        while let Some(signal) =
            ogc_bt_sync::parse_next_l2cap_signal(&mut signals).map_err(|_| ERR_IF)?
        {
            match ogc_bt_sync::parse_l2cap_signal_payload(signal).map_err(|_| ERR_IF)? {
                L2capSignalPayload::ConnectionRequest {
                    psm: request_psm,
                    source_cid,
                } if incoming && request_psm == psm => {
                    let len = ogc_bt_sync::build_l2cap_connection_response(
                        &mut scratch,
                        transport.handle,
                        signal.identifier,
                        local_cid,
                        source_cid,
                        0,
                    )
                    .map_err(|_| ERR_IF)?;
                    host.write_acl(&scratch[..len]).map_err(|_| ERR_IF)?;
                    let channel = if control {
                        &mut transport.l2cap.control
                    } else {
                        &mut transport.l2cap.interrupt
                    };
                    channel.remote_cid = source_cid;
                    channel.active = true;
                    channel.connected = true;
                }
                L2capSignalPayload::ConfigurationRequest { dest_cid, .. }
                    if dest_cid == local_cid =>
                {
                    host.send_l2cap_configuration_response(
                        &mut scratch,
                        transport.handle,
                        signal.identifier,
                        dest_cid,
                        L2CAP_CFG_SUCCESS,
                    )
                    .map_err(|_| ERR_IF)?;
                    transport.l2cap.observe_signal(signal).map_err(|_| ERR_IF)?;
                }
                _ => {
                    transport.l2cap.observe_signal(signal).map_err(|_| ERR_IF)?;
                }
            }
        }

        let channel = if control {
            transport.l2cap.control
        } else {
            transport.l2cap.interrupt
        };
        if channel.connected && !transport.config_sent[config_index] {
            host.send_l2cap_configuration_request_with_mtu(
                &mut scratch,
                transport.handle,
                signal_id.wrapping_add(2),
                channel.remote_cid,
                HID_L2CAP_MTU,
            )
            .map_err(|_| ERR_IF)?;
            transport.config_sent[config_index] = true;
        }
        yield_thread();
    }
}

fn wait_for_baseband_connection(
    host: &BtSyncHost,
    transport: &mut PcbTransport,
    pcb: *mut crate::BtePcb,
    incoming: bool,
) -> Result<(), i32> {
    let mut events = [0u8; ogc_bt_sync::MAX_EVENT_PACKET_BYTES];
    let started = now_ticks();
    loop {
        if now_ticks().wrapping_sub(started) >= TICKS_PER_SECOND * 10 {
            return Err(ERR_IF);
        }
        let used = host.read_event(&mut events).map_err(|_| ERR_IF)?;
        let event = host.parse_event(&events[..used]).map_err(|_| ERR_IF)?;
        match event {
            HciEvent::ConnectionRequest {
                address,
                class_of_device,
                link_type,
            } if incoming => {
                handle_connection_request(host, transport, address, class_of_device, link_type)?;
            }
            HciEvent::ConnectionComplete {
                status,
                address,
                handle,
                ..
            } if address == transport.address => {
                if status != 0 {
                    return Err(ERR_CONN);
                }
                transport.handle = handle;
                transport.listening = false;
                unsafe { (*pcb).cmdq = handle as u32 };
                return Ok(());
            }
            HciEvent::CommandStatus {
                opcode: HciOpcode::CREATE_CONNECTION,
                status,
            } if !incoming && status != 0 => return Err(ERR_CONN),
            HciEvent::DisconnectionComplete { handle, reason, .. }
                if handle == transport.handle && handle != 0 =>
            {
                let mut bdaddr = unsafe { (*pcb).bdaddr };
                unsafe { crate::notify_disconnect(&mut bdaddr, reason) };
                return Err(ERR_CONN);
            }
            other => handle_pairing_event(host, other),
        }
        yield_thread();
    }
}

pub(crate) unsafe fn connectasync(
    pcb: *mut crate::BtePcb,
    bdaddr: *mut BdAddr,
    conn_cfm: crate::BteConnCfmCallback,
) -> i32 {
    unsafe {
        if bdaddr.is_null() {
            return ERR_ARG;
        }
        let Some(index) = crate::pcb_index(pcb) else {
            return ERR_ARG;
        };
        let Some(transport) = transport_for(pcb) else {
            return ERR_ARG;
        };
        let Some(host) = host() else {
            return ERR_IF;
        };
        let address = *bdaddr;
        if let Err(error) = reserve_connection_task(index) {
            return error;
        }
        transport.address = bd_address(&address);
        transport.handle = 0;
        transport.listening = false;
        transport.l2cap = HidL2capState::new();
        transport.config_sent = [false; 2];
        transport.disconnecting = false;
        transport.disconnect_signal_id = 0;
        transport.disconnect_local_cid = 0;
        transport.disconnect_remote_cid = 0;
        (*pcb).bdaddr = address;
        (*pcb).state = crate::HIDP_STATE_CONNECTING;
        (*pcb).conn_cfm = conn_cfm;
        if host
            .create_connection(bd_address(&address), 0xcc18, 0, 0, 0, true)
            .is_err()
        {
            CONNECTION_TASK.store(CONNECTION_TASK_IDLE, Ordering::Release);
            (*pcb).state = crate::HIDP_STATE_READY;
            return ERR_IF;
        }
        activate_connection_task(CONNECTION_TASK_CONNECT).map_or_else(|error| error, |_| ERR_OK)
    }
}

pub(crate) unsafe fn connectasync_step2(
    pcb: *mut crate::BtePcb,
    conn_cfm: crate::BteConnCfmCallback,
) -> i32 {
    unsafe {
        let Some(index) = crate::pcb_index(pcb) else {
            return ERR_ARG;
        };
        let Some(transport) = transport_for(pcb) else {
            return ERR_ARG;
        };
        if transport.handle == 0 || !transport.l2cap.control.ready() {
            return ERR_CONN;
        }
        if let Err(error) = reserve_connection_task(index) {
            return error;
        }
        (*pcb).conn_cfm = conn_cfm;
        activate_connection_task(CONNECTION_TASK_CONNECT_STEP2)
            .map_or_else(|error| error, |_| ERR_OK)
    }
}

pub(crate) unsafe fn disconnect(pcb: *mut crate::BtePcb) -> i32 {
    let Some(index) = crate::pcb_index(pcb) else {
        return ERR_ARG;
    };
    if disconnect_is_pending(index) {
        return crate::ERR_CLSD;
    }
    unsafe { (*pcb).state = crate::HIDP_STATE_DISCONNECTING };
    let transport = unsafe {
        core::ptr::addr_of_mut!(PCB_TRANSPORTS)
            .cast::<PcbTransport>()
            .add(index)
    };
    let handle = unsafe { (*transport).handle };
    if handle == 0 {
        if unsafe { (*transport).listening } {
            let mut events = [0u8; ogc_bt_sync::MAX_EVENT_PACKET_BYTES];
            if let Some(host) = host() {
                let _ = host.set_scan_enable(ScanEnable::Off, &mut events);
            }
        }
        unsafe {
            (*transport).listening = false;
            (*transport).disconnecting = true;
        }
        return finish_l2cap_disconnect(index, false, false);
    }
    let has_active_channel =
        unsafe { (*transport).l2cap.interrupt.active || (*transport).l2cap.control.active };
    unsafe { (*transport).disconnecting = true };
    if has_active_channel {
        match send_next_l2cap_disconnect(index) {
            Ok(true) => ERR_OK,
            Ok(false) => finish_l2cap_disconnect(index, true, false),
            Err(error) => {
                abort_local_disconnect(index, error);
                error
            }
        }
    } else {
        finish_l2cap_disconnect(index, true, false)
    }
}

pub(crate) unsafe fn listenasync(
    pcb: *mut crate::BtePcb,
    bdaddr: *mut BdAddr,
    conn_cfm: crate::BteConnCfmCallback,
) -> i32 {
    unsafe {
        if bdaddr.is_null() {
            return ERR_ARG;
        }
        let Some(index) = crate::pcb_index(pcb) else {
            return ERR_ARG;
        };
        let Some(transport) = transport_for(pcb) else {
            return ERR_ARG;
        };
        let Some(host) = host() else {
            return ERR_IF;
        };
        if let Err(error) = reserve_connection_task(index) {
            return error;
        }
        let mut events = [0u8; ogc_bt_sync::MAX_EVENT_PACKET_BYTES];
        if host.set_scan_enable(ScanEnable::Page, &mut events).is_err() {
            CONNECTION_TASK.store(CONNECTION_TASK_IDLE, Ordering::Release);
            return ERR_IF;
        }
        let address = *bdaddr;
        transport.address = bd_address(&address);
        transport.handle = 0;
        transport.listening = true;
        transport.l2cap = HidL2capState::new();
        transport.config_sent = [false; 2];
        transport.disconnecting = false;
        transport.disconnect_signal_id = 0;
        transport.disconnect_local_cid = 0;
        transport.disconnect_remote_cid = 0;
        (*pcb).bdaddr = address;
        (*pcb).state = crate::HIDP_STATE_CONNECTING;
        (*pcb).conn_cfm = conn_cfm;

        activate_connection_task(CONNECTION_TASK_LISTEN).map_or_else(|error| error, |_| ERR_OK)
    }
}

pub(crate) unsafe fn listenasync_step2(
    pcb: *mut crate::BtePcb,
    conn_cfm: crate::BteConnCfmCallback,
) -> i32 {
    unsafe {
        let Some(index) = crate::pcb_index(pcb) else {
            return ERR_ARG;
        };
        let Some(transport) = transport_for(pcb) else {
            return ERR_ARG;
        };
        if transport.handle == 0 || !transport.l2cap.control.ready() {
            return ERR_CONN;
        }
        if let Err(error) = reserve_connection_task(index) {
            return error;
        }
        (*pcb).conn_cfm = conn_cfm;
        activate_connection_task(CONNECTION_TASK_LISTEN_STEP2)
            .map_or_else(|error| error, |_| ERR_OK)
    }
}

pub(crate) unsafe fn senddata(pcb: *mut crate::BtePcb, message: *mut c_void, len: u16) -> i32 {
    unsafe {
        if message.is_null() || len == 0 || len as usize > MAX_BTE_MESSAGE_SIZE {
            return ERR_ARG;
        }
        let Some(transport) = transport_for(pcb) else {
            return ERR_ARG;
        };
        if (*pcb).state != crate::HIDP_STATE_CONNECTED {
            return if (*pcb).state == crate::HIDP_STATE_DISCONNECTING {
                crate::ERR_CLSD
            } else {
                ERR_CONN
            };
        }
        let Some((remote_cid, header)) =
            hid_report_route(&transport.l2cap, HidReportPath::InterruptData)
        else {
            return ERR_CONN;
        };
        let Some(host) = host() else {
            return ERR_IF;
        };
        let bytes = core::slice::from_raw_parts(message.cast::<u8>(), len as usize);
        let mut payload = [0u8; MAX_HIDP_PAYLOAD_SIZE];
        let Some(used) = encode_hidp_payload(bytes, header, &mut payload) else {
            return ERR_ARG;
        };
        host.send_l2cap_hid_data(
            &mut [0u8; MAX_ACL_PACKET_SIZE],
            transport.handle,
            remote_cid,
            &payload[..used],
        )
        .map_or(ERR_IF, |_| ERR_OK)
    }
}

#[derive(Copy, Clone)]
enum HidReportPath {
    InterruptData,
    ControlSetReport,
}

fn hid_report_route(channels: &HidL2capState, path: HidReportPath) -> Option<(u16, u8)> {
    match path {
        HidReportPath::InterruptData => channels.interrupt.ready().then_some((
            channels.interrupt.remote_cid,
            HIDP_TRANS_DATA | HIDP_DATA_RTYPE_OUTPUT,
        )),
        HidReportPath::ControlSetReport => channels.control.ready().then_some((
            channels.control.remote_cid,
            crate::HIDP_TRANS_SETREPORT | crate::HIDP_DATA_RTYPE_OUPUT,
        )),
    }
}

fn encode_hidp_payload(
    message: &[u8],
    header: u8,
    output: &mut [u8; MAX_HIDP_PAYLOAD_SIZE],
) -> Option<usize> {
    if message.is_empty() || message.len() > MAX_BTE_MESSAGE_SIZE {
        return None;
    }
    output[0] = header;
    output[1..1 + message.len()].copy_from_slice(message);
    Some(message.len() + 1)
}

pub(crate) unsafe fn sendmessage(pcb: *mut crate::BtePcb, message: *mut c_void, len: u16) -> i32 {
    unsafe { sendmessage_inner(pcb, message, len, None, true) }
}

pub(crate) unsafe fn sendmessageasync(
    pcb: *mut crate::BtePcb,
    message: *mut c_void,
    len: u16,
    sent: crate::BteConnCfmCallback,
) -> i32 {
    unsafe { sendmessage_inner(pcb, message, len, sent, false) }
}

unsafe fn sendmessage_inner(
    pcb: *mut crate::BtePcb,
    message: *mut c_void,
    len: u16,
    sent: crate::BteConnCfmCallback,
    synchronous: bool,
) -> i32 {
    unsafe {
        if message.is_null() || len == 0 || len as usize > MAX_BTE_MESSAGE_SIZE {
            return ERR_ARG;
        }
        let Some(index) = crate::pcb_index(pcb) else {
            return ERR_ARG;
        };
        if disconnect_is_pending(index) {
            return crate::ERR_CLSD;
        }
        let Some(transport) = transport_for(pcb) else {
            return ERR_ARG;
        };
        if (*pcb).state != crate::HIDP_STATE_CONNECTED {
            return if (*pcb).state == crate::HIDP_STATE_DISCONNECTING {
                crate::ERR_CLSD
            } else {
                ERR_CONN
            };
        }
        let Some((_, header)) = hid_report_route(&transport.l2cap, HidReportPath::ControlSetReport)
        else {
            return ERR_CONN;
        };
        let bytes = core::slice::from_raw_parts(message.cast::<u8>(), len as usize);
        let mut payload = [0u8; MAX_HIDP_PAYLOAD_SIZE];
        let Some(used) = encode_hidp_payload(bytes, header, &mut payload) else {
            return ERR_ARG;
        };
        let Some(slot) = control_request_alloc(&payload[..used], sent, synchronous) else {
            return crate::ERR_MEM;
        };
        let should_send = control_request_enqueue(index, slot);
        let result = if should_send {
            send_control_head(index, pcb)
        } else {
            ERR_OK
        };
        if result != ERR_OK {
            if synchronous && let Some(done) = control_request_result(slot) {
                control_request_free(slot);
                return done;
            }
            return result;
        }
        if !synchronous {
            return ERR_OK;
        }

        loop {
            if let Some(result) = control_request_result(slot) {
                control_request_free(slot);
                return result;
            }
            if (*pcb).state != crate::HIDP_STATE_CONNECTED {
                return ERR_CONN;
            }
            yield_thread();
        }
    }
}

#[cfg(all(test, target_arch = "powerpc"))]
mod report_route_tests {
    use super::{
        CONTROL_NONE, HID_L2CAP_MTU, HidReportPath, MAX_BTE_MESSAGE_SIZE, MAX_HIDP_PAYLOAD_SIZE,
        PcbTransport, close_remote_channel, control_request_enqueue, control_request_pop_head,
        drain_pending_control_requests, encode_hidp_payload, hid_report_route,
    };
    use core::cell::Cell;
    use ogc_bt_sync::HidL2capState;

    #[test]
    fn per_pcb_control_queue_supports_last_pool_slot() {
        let last_pcb = crate::PCB_SLOTS - 1;
        let request = 17u16;
        unsafe {
            *super::control_request_ptr(request as usize) = super::ControlRequest::EMPTY;
        }
        assert!(control_request_enqueue(last_pcb, request));
        assert_eq!(control_request_pop_head(last_pcb), Some(request));
        assert_eq!(control_request_pop_head(last_pcb), None);
    }

    #[test]
    fn local_disconnect_completes_only_after_both_l2cap_responses() {
        assert!(super::disconnect_response_order_matches(
            &mut PcbTransport::new()
        ));
    }

    #[test]
    fn set_report_uses_ready_control_channel_and_hidp_header() {
        let mut channels = HidL2capState::new();
        channels.control.active = true;
        channels.control.connected = true;
        channels.control.local_configured = true;
        channels.control.remote_configured = true;
        channels.control.remote_cid = 0x0070;
        channels.interrupt.active = true;
        channels.interrupt.connected = true;
        channels.interrupt.local_configured = true;
        channels.interrupt.remote_configured = true;
        channels.interrupt.remote_cid = 0x0071;

        assert_eq!(
            hid_report_route(&channels, HidReportPath::ControlSetReport),
            Some((0x0070, 0x52))
        );
        assert_eq!(
            hid_report_route(&channels, HidReportPath::InterruptData),
            Some((0x0071, 0xa2))
        );
    }

    #[test]
    fn set_report_is_unavailable_until_control_channel_is_ready() {
        let channels = HidL2capState::new();
        assert_eq!(
            hid_report_route(&channels, HidReportPath::ControlSetReport),
            None
        );
    }

    #[test]
    fn successful_hidp_handshake_is_the_only_completion_packet() {
        assert!(crate::is_successful_hidp_handshake(&[
            crate::HIDP_TRANS_HANDSHAKE | crate::HIDP_HSHK_SUCCESSFULL
        ]));
        assert!(!crate::is_successful_hidp_handshake(&[
            crate::HIDP_TRANS_HANDSHAKE | crate::HIDP_HSHK_NOTREADY
        ]));
        assert!(!crate::is_successful_hidp_handshake(&[
            crate::HIDP_TRANS_DATA | crate::HIDP_DATA_RTYPE_INPUT
        ]));
    }

    #[test]
    fn hid_l2cap_mtu_matches_libogc_hidd_payload_plus_header() {
        assert_eq!(HID_L2CAP_MTU, 673);
        assert_eq!(HID_L2CAP_MTU as usize, MAX_HIDP_PAYLOAD_SIZE);
    }

    #[test]
    fn hidp_payload_supports_reports_over_64_bytes_and_full_c_mtu() {
        let over_64 = [0x5a; 65];
        let mut output = [0u8; MAX_HIDP_PAYLOAD_SIZE];
        let used = encode_hidp_payload(&over_64, 0x52, &mut output).unwrap();
        assert_eq!(used, 66);
        assert_eq!(output[0], 0x52);
        assert_eq!(&output[1..used], &over_64);

        let at_limit = [0xa5; MAX_BTE_MESSAGE_SIZE];
        let used = encode_hidp_payload(&at_limit, 0x52, &mut output).unwrap();
        assert_eq!(MAX_BTE_MESSAGE_SIZE, 672);
        assert_eq!(MAX_HIDP_PAYLOAD_SIZE, 673);
        assert_eq!(used, MAX_HIDP_PAYLOAD_SIZE);
        assert_eq!(output[0], 0x52);
        assert_eq!(output[used - 1], 0xa5);

        let caller_preframed = [0xa2, 0x31, 0x32];
        let used = encode_hidp_payload(&caller_preframed, 0xa2, &mut output).unwrap();
        assert_eq!(used, 4);
        assert_eq!(&output[..used], &[0xa2, 0xa2, 0x31, 0x32]);

        let too_large = [0u8; MAX_BTE_MESSAGE_SIZE + 1];
        assert!(encode_hidp_payload(&too_large, 0x52, &mut output).is_none());
    }

    #[test]
    fn remote_disconnect_drains_queued_control_requests_after_both_channels_close() {
        let mut channels = HidL2capState::new();
        channels.control.active = true;
        channels.control.connected = true;
        channels.control.remote_cid = 0x0070;
        channels.interrupt.active = true;
        channels.interrupt.connected = true;
        channels.interrupt.remote_cid = 0x0071;

        let queued = [Some(3u16), Some(8u16), None];
        let queue_cursor = Cell::new(0usize);
        let mut completed = [(CONTROL_NONE, 0); 2];
        let completion_count = Cell::new(0usize);

        let control_local_cid = channels.control.local_cid;
        assert_eq!(
            close_remote_channel(&mut channels, control_local_cid, 0x0070),
            Some(false)
        );
        assert_eq!(
            queue_cursor.get(),
            0,
            "one open channel must keep requests queued"
        );

        let interrupt_local_cid = channels.interrupt.local_cid;
        assert_eq!(
            close_remote_channel(&mut channels, interrupt_local_cid, 0x0071),
            Some(true)
        );
        drain_pending_control_requests(
            || {
                let cursor = queue_cursor.get();
                queue_cursor.set(cursor + 1);
                queued.get(cursor).copied().flatten()
            },
            |slot, result| {
                let count = completion_count.get();
                completed[count] = (slot, result);
                completion_count.set(count + 1);
            },
            crate::ERR_CLSD,
        );

        assert_eq!(completion_count.get(), 2);
        assert_eq!(completed, [(3, crate::ERR_CLSD), (8, crate::ERR_CLSD)]);
    }
}
