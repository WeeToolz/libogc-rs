#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicPtr, AtomicU32, Ordering};

#[cfg(target_arch = "powerpc")]
mod powerpc_backend;

/// Runs the disconnect-response state check used by the Dolphin probe.
#[cfg(all(feature = "dolphin-automation", target_arch = "powerpc"))]
pub fn dolphin_disconnect_response_order_probe() -> bool {
    powerpc_backend::dolphin_disconnect_response_order_probe()
}

pub const ERR_OK: i32 = 0;
pub const ERR_MEM: i32 = -1;
pub const ERR_BUF: i32 = -2;
pub const ERR_ABRT: i32 = -3;
pub const ERR_RST: i32 = -4;
pub const ERR_CLSD: i32 = -5;
pub const ERR_CONN: i32 = -6;
pub const ERR_VAL: i32 = -7;
pub const ERR_ARG: i32 = -8;
pub const ERR_RTE: i32 = -9;
pub const ERR_USE: i32 = -10;
pub const ERR_IF: i32 = -11;
pub const ERR_PKTSIZE: i32 = -17;

pub const HIDP_STATE_READY: u32 = 0x00;
pub const HIDP_STATE_LISTEN: u32 = 0x01;
pub const HIDP_STATE_CONNECTING: u32 = 0x02;
pub const HIDP_STATE_DISCONNECTING: u32 = 0x03;
pub const HIDP_STATE_CONNECTED: u32 = 0x04;

pub const HIDP_CONTROL_CHANNEL: u8 = 0x11;
pub const HIDP_DATA_CHANNEL: u8 = 0x13;

pub const HIDP_HDR_TRANS_MASK: u8 = 0xf0;
pub const HIDP_HDR_PARAM_MASK: u8 = 0x0f;
pub const HIDP_TRANS_HANDSHAKE: u8 = 0x00;
pub const HIDP_TRANS_HIDCONTROL: u8 = 0x10;
pub const HIDP_TRANS_GETREPORT: u8 = 0x40;
pub const HIDP_TRANS_SETREPORT: u8 = 0x50;
pub const HIDP_TRANS_GETPROTOCOL: u8 = 0x60;
pub const HIDP_TRANS_SETPROTOCOL: u8 = 0x70;
pub const HIDP_TRANS_GETIDLE: u8 = 0x80;
pub const HIDP_TRANS_SETIDLE: u8 = 0x90;
pub const HIDP_TRANS_DATA: u8 = 0xa0;
pub const HIDP_TRANS_DATAC: u8 = 0xb0;

pub const HIDP_HSHK_SUCCESSFULL: u8 = 0x00;
pub const HIDP_HSHK_NOTREADY: u8 = 0x01;
pub const HIDP_HSHK_INV_REPORTID: u8 = 0x02;
pub const HIDP_HSHK_NOTSUPPORTED: u8 = 0x03;
pub const HIDP_HSHK_IVALIDPARAM: u8 = 0x04;
pub const HIDP_HSHK_UNKNOWNERROR: u8 = 0x0e;
pub const HIDP_HSHK_FATALERROR: u8 = 0x0f;

pub const HIDP_CTRL_NOP: u8 = 0x00;
pub const HIDP_CTRL_HARDRESET: u8 = 0x01;
pub const HIDP_CTRL_SOFTRESET: u8 = 0x02;
pub const HIDP_CTRL_SUSPEND: u8 = 0x03;
pub const HIDP_CTRL_RESUME: u8 = 0x04;
pub const HIDP_CTRL_VC_UNPLUG: u8 = 0x05;

pub const HIDP_DATA_RTYPE_MASK: u8 = 0x03;
pub const HIDP_DATA_RSRVD_MASK: u8 = 0x0c;
pub const HIDP_DATA_RTYPE_OTHER: u8 = 0x00;
pub const HIDP_DATA_RTYPE_INPUT: u8 = 0x01;
pub const HIDP_DATA_RTYPE_OUPUT: u8 = 0x02;
pub const HIDP_DATA_RTYPE_FEATURE: u8 = 0x03;

pub const HIDP_PROTO_BOOT: u8 = 0x00;
pub const HIDP_PROTO_REPORT: u8 = 0x01;

pub const BD_ADDR_LEN: usize = 6;
pub const BD_LINK_KEY_LEN: usize = 16;
pub const BD_NAME_LEN: usize = 248;
pub const BD_MAX_INQUIRY_DEVS: usize = 255;

pub const PAIR_MODE_NORMAL: u8 = 0;
pub const PAIR_MODE_TEMPORARY: u8 = 1;

pub type LwpQueue = u32;
pub type BteCallback = Option<unsafe extern "C" fn(result: i32, userdata: *mut c_void) -> i32>;
pub type BteRecvCallback =
    Option<unsafe extern "C" fn(arg: *mut c_void, buffer: *mut c_void, len: u16) -> i32>;
pub type BteConnReqCallback = Option<
    unsafe extern "C" fn(
        arg: *mut c_void,
        pcb: *mut BtePcb,
        bdaddr: *mut BdAddr,
        cod: *mut u8,
        link_type: u8,
        err: u8,
    ) -> i32,
>;
pub type BteConnCfmCallback =
    Option<unsafe extern "C" fn(arg: *mut c_void, pcb: *mut BtePcb, err: u8) -> i32>;
pub type BteDisconnectCfmCallback =
    Option<unsafe extern "C" fn(arg: *mut c_void, pcb: *mut BtePcb, err: u8) -> i32>;
pub type BteDisconnectCallback = Option<unsafe extern "C" fn(bdaddr: *mut BdAddr, reason: u8)>;
pub type BteHostSyncButtonCallback = Option<unsafe extern "C" fn(held: u32)>;
pub type BteConnectionRequestCallback = Option<
    unsafe extern "C" fn(arg: *mut c_void, bdaddr: *mut BdAddr, cod: *mut u8, link_type: u8) -> i8,
>;
pub type BteLinkKeyRequestCallback =
    Option<unsafe extern "C" fn(arg: *mut c_void, bdaddr: *mut BdAddr) -> i8>;
pub type BteLinkKeyNotificationCallback =
    Option<unsafe extern "C" fn(arg: *mut c_void, bdaddr: *mut BdAddr, key: *mut u8) -> i8>;

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
pub struct BdAddr {
    pub addr: [u8; BD_ADDR_LEN],
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct InquiryInfo {
    pub bdaddr: BdAddr,
    pub cod: [u8; 3],
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct InquiryInfoEx {
    pub bdaddr: BdAddr,
    pub cod: [u8; 3],
    pub psrm: u8,
    pub psm: u8,
    pub co: u16,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct BteInquiryRes {
    pub count: u8,
    pub info: *mut InquiryInfoEx,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct LinkKeyInfo {
    pub bdaddr: BdAddr,
    pub key: [u8; BD_LINK_KEY_LEN],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct PadNameInfo {
    pub bdaddr: BdAddr,
    pub name: [u8; BD_NAME_LEN],
}

impl Default for PadNameInfo {
    fn default() -> Self {
        Self {
            bdaddr: BdAddr::default(),
            name: [0; BD_NAME_LEN],
        }
    }
}

#[repr(C)]
pub struct L2capPcb {
    _private: [u8; 0],
}

#[repr(C)]
pub struct CtrlReq {
    _private: [u8; 0],
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct BtePcb {
    pub err: u8,
    pub state: u32,
    pub cbarg: *mut c_void,
    pub ctrl_req_head: *mut CtrlReq,
    pub ctrl_req_tail: *mut CtrlReq,
    pub cmdq: LwpQueue,
    pub bdaddr: BdAddr,
    pub ctl_pcb: *mut L2capPcb,
    pub data_pcb: *mut L2capPcb,
    pub recv: BteRecvCallback,
    pub conn_req: BteConnReqCallback,
    pub conn_cfm: BteConnCfmCallback,
    pub disconn_cfm: BteDisconnectCfmCallback,
}

impl Default for BtePcb {
    fn default() -> Self {
        Self {
            err: 0,
            state: HIDP_STATE_READY,
            cbarg: ptr::null_mut(),
            ctrl_req_head: ptr::null_mut(),
            ctrl_req_tail: ptr::null_mut(),
            cmdq: 0,
            bdaddr: BdAddr::default(),
            ctl_pcb: ptr::null_mut(),
            data_pcb: ptr::null_mut(),
            recv: None,
            conn_req: None,
            conn_cfm: None,
            disconn_cfm: None,
        }
    }
}

const PCB_SLOTS: usize = 14;

static BTE_INITED: AtomicBool = AtomicBool::new(false);
static BTE_LAST_STATUS: AtomicI32 = AtomicI32::new(ERR_OK);
static BTE_PAIR_MODE: AtomicU32 = AtomicU32::new(PAIR_MODE_NORMAL as u32);
static HOSTSYNC_HELD: AtomicU32 = AtomicU32::new(0);
static DISCONNECT_CB: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static HOSTSYNC_CB: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static CONN_REQ_CB: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static LINK_KEY_REQ_CB: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static LINK_KEY_NOTIFY_CB: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static STORED_LINK_KEY_VALID: AtomicBool = AtomicBool::new(false);
static PCB_USED: [AtomicBool; PCB_SLOTS] = [const { AtomicBool::new(false) }; PCB_SLOTS];
static mut PCB_STORE: [BtePcb; PCB_SLOTS] = [const {
    BtePcb {
        err: 0,
        state: HIDP_STATE_READY,
        cbarg: ptr::null_mut(),
        ctrl_req_head: ptr::null_mut(),
        ctrl_req_tail: ptr::null_mut(),
        cmdq: 0,
        bdaddr: BdAddr {
            addr: [0; BD_ADDR_LEN],
        },
        ctl_pcb: ptr::null_mut(),
        data_pcb: ptr::null_mut(),
        recv: None,
        conn_req: None,
        conn_cfm: None,
        disconn_cfm: None,
    }
}; PCB_SLOTS];
static mut STORED_LINK_KEY_ADDR: BdAddr = BdAddr {
    addr: [0; BD_ADDR_LEN],
};
static mut STORED_LINK_KEY: [u8; BD_LINK_KEY_LEN] = [0; BD_LINK_KEY_LEN];

#[inline]
const fn lwbt_host_compatibility_enabled() -> bool {
    !cfg!(target_arch = "powerpc")
}

fn deterministic_bdaddr() -> BdAddr {
    BdAddr {
        addr: [0x10, 0x20, 0x30, 0x40, 0x50, 0x60],
    }
}

unsafe fn callback_from_ptr<T>(ptr: *mut c_void) -> T {
    unsafe { core::mem::transmute_copy(&ptr) }
}

fn pcb_slot(pcb: *mut BtePcb) -> Option<usize> {
    unsafe {
        let slots = ptr::addr_of_mut!(PCB_STORE).cast::<BtePcb>();
        (0..PCB_SLOTS).find(|&index| slots.add(index) == pcb)
    }
}

fn valid_pcb(pcb: *mut BtePcb) -> bool {
    pcb_slot(pcb).is_some_and(|idx| PCB_USED[idx].load(Ordering::Acquire))
}

pub(crate) fn pcb_index(pcb: *mut BtePcb) -> Option<usize> {
    pcb_slot(pcb)
}

pub(crate) fn pcb_for_index(index: usize) -> Option<*mut BtePcb> {
    (index < PCB_SLOTS && PCB_USED[index].load(Ordering::Acquire))
        .then(|| unsafe { ptr::addr_of_mut!(PCB_STORE).cast::<BtePcb>().add(index) })
}

pub(crate) unsafe fn deliver_data(pcb: *mut BtePcb, message: &[u8]) {
    if let Some(recv) = unsafe { (*pcb).recv } {
        unsafe {
            recv(
                (*pcb).cbarg,
                message.as_ptr().cast_mut().cast(),
                message.len() as u16,
            )
        };
    }
}

pub(crate) fn is_successful_hidp_handshake(packet: &[u8]) -> bool {
    ogc_bt_sync::parse_hidp_packet(packet).is_ok_and(|hidp| {
        hidp.transaction == ogc_bt_sync::HIDP_TRANS_HANDSHAKE
            && hidp.parameter == HIDP_HSHK_SUCCESSFULL
    })
}

pub fn try_init() -> i32 {
    let result = if cfg!(target_arch = "powerpc") {
        let result = powerpc_backend::init();
        if result == ERR_OK {
            BTE_INITED.store(true, Ordering::Release);
        }
        result
    } else if !lwbt_host_compatibility_enabled() {
        ERR_IF
    } else {
        BTE_INITED.store(true, Ordering::Release);
        ERR_OK
    };
    BTE_LAST_STATUS.store(result, Ordering::Release);
    result
}

pub fn init() {
    try_init();
}

pub fn try_shutdown() -> i32 {
    let result = if cfg!(target_arch = "powerpc") {
        let result = powerpc_backend::shutdown();
        if result != ERR_OK {
            BTE_LAST_STATUS.store(result, Ordering::Release);
            return result;
        }
        ERR_OK
    } else if !lwbt_host_compatibility_enabled() {
        ERR_IF
    } else {
        ERR_OK
    };
    if result == ERR_OK {
        BTE_INITED.store(false, Ordering::Release);
        BTE_PAIR_MODE.store(PAIR_MODE_NORMAL as u32, Ordering::Release);
        STORED_LINK_KEY_VALID.store(false, Ordering::Release);
        for used in PCB_USED.iter() {
            used.store(false, Ordering::Release);
        }
    }
    BTE_LAST_STATUS.store(result, Ordering::Release);
    result
}

pub fn shutdown() {
    try_shutdown();
}

pub fn last_status() -> i32 {
    BTE_LAST_STATUS.load(Ordering::Acquire)
}

pub fn close() {
    shutdown();
}

fn run_init_callback(cb: BteCallback) -> i32 {
    if cfg!(target_arch = "powerpc") {
        return powerpc_backend::run_init_callback(cb);
    }
    if !lwbt_host_compatibility_enabled() {
        return ERR_IF;
    }
    if !BTE_INITED.load(Ordering::Acquire) {
        BTE_INITED.store(true, Ordering::Release);
    }
    unsafe {
        if let Some(cb) = cb {
            cb(ERR_OK, ptr::null_mut());
        }
    }
    ERR_OK
}

pub fn init_core(cb: BteCallback) -> i32 {
    if cfg!(target_arch = "powerpc") {
        return powerpc_backend::init_core(cb);
    }
    run_init_callback(cb)
}

pub fn apply_patch(cb: BteCallback) -> i32 {
    if cfg!(target_arch = "powerpc") {
        return powerpc_backend::apply_patch(cb);
    }
    run_init_callback(cb)
}

pub fn init_sub(cb: BteCallback) -> i32 {
    if cfg!(target_arch = "powerpc") {
        return powerpc_backend::init_sub(cb);
    }
    run_init_callback(cb)
}

/// # Safety
///
/// Raw pointer arguments and callbacks must be valid for the duration of the call.
pub unsafe fn read_stored_link_key(keys: *mut LinkKeyInfo, max_cnt: u8, cb: BteCallback) -> i32 {
    unsafe {
        if cfg!(target_arch = "powerpc") {
            return powerpc_backend::read_stored_link_key(keys, max_cnt, cb);
        }
        if !lwbt_host_compatibility_enabled() {
            let _ = (keys, max_cnt, cb);
            return ERR_IF;
        }
        {
            if !keys.is_null() && max_cnt != 0 {
                if STORED_LINK_KEY_VALID.load(Ordering::Acquire) {
                    (*keys).bdaddr = STORED_LINK_KEY_ADDR;
                    (*keys).key = STORED_LINK_KEY;
                } else {
                    (*keys).bdaddr = deterministic_bdaddr();
                    (*keys).key = [0x11; BD_LINK_KEY_LEN];
                }
            }
            run_init_callback(cb)
        }
    }
}

/// # Safety
///
/// Raw pointer arguments and callbacks must be valid for the duration of the call.
pub unsafe fn read_bd_addr(bdaddr: *mut BdAddr, cb: BteCallback) -> i32 {
    unsafe {
        if cfg!(target_arch = "powerpc") {
            return powerpc_backend::read_bd_addr(bdaddr, cb);
        }
        if !lwbt_host_compatibility_enabled() {
            let _ = (bdaddr, cb);
            return ERR_IF;
        }
        {
            if !bdaddr.is_null() {
                *bdaddr = deterministic_bdaddr();
            }
            run_init_callback(cb)
        }
    }
}

pub fn set_evt_filter(
    filter_type: u8,
    filter_cond_type: u8,
    cond: *mut u8,
    cb: BteCallback,
) -> i32 {
    if cfg!(target_arch = "powerpc") {
        return powerpc_backend::set_evt_filter(filter_type, filter_cond_type, cond, cb);
    }
    if !lwbt_host_compatibility_enabled() {
        let _ = (filter_type, filter_cond_type, cond, cb);
        return ERR_IF;
    }
    run_init_callback(cb)
}

/// # Safety
///
/// Raw pointer arguments and callbacks must be valid for the duration of the call.
pub unsafe fn read_remote_name(padinfo: *mut PadNameInfo, cb: BteCallback) -> i32 {
    unsafe {
        if cfg!(target_arch = "powerpc") {
            return powerpc_backend::read_remote_name(padinfo, cb);
        }
        if !lwbt_host_compatibility_enabled() {
            let _ = (padinfo, cb);
            return ERR_IF;
        }
        {
            if !padinfo.is_null() {
                (*padinfo).bdaddr = deterministic_bdaddr();
                let name = b"Rust lwBT pad\0";
                for (idx, byte) in name.iter().enumerate() {
                    (*padinfo).name[idx] = *byte;
                }
            }
            run_init_callback(cb)
        }
    }
}

pub fn inquiry(_max_cnt: u8, _flush: u8, cb: BteCallback) -> i32 {
    if cfg!(target_arch = "powerpc") {
        return powerpc_backend::inquiry(_max_cnt, _flush, cb);
    }
    if !lwbt_host_compatibility_enabled() {
        let _ = (_max_cnt, _flush, cb);
        return ERR_IF;
    }
    run_init_callback(cb)
}

pub fn periodic_inquiry(_max_cnt: u8, _flush: u8, cb: BteCallback) -> i32 {
    if cfg!(target_arch = "powerpc") {
        return powerpc_backend::periodic_inquiry(_max_cnt, _flush, cb);
    }
    if !lwbt_host_compatibility_enabled() {
        let _ = (_max_cnt, _flush, cb);
        return ERR_IF;
    }
    BTE_PAIR_MODE.store(PAIR_MODE_TEMPORARY as u32, Ordering::Release);
    run_init_callback(cb)
}

pub fn exit_periodic_inquiry() -> i32 {
    if cfg!(target_arch = "powerpc") {
        return powerpc_backend::exit_periodic_inquiry();
    }
    if !lwbt_host_compatibility_enabled() {
        return ERR_IF;
    }
    BTE_PAIR_MODE.store(PAIR_MODE_NORMAL as u32, Ordering::Release);
    ERR_OK
}

/// # Safety
///
/// `_bdaddr` must point to a valid Bluetooth address and `_key` must point to
/// at least `BD_LINK_KEY_LEN` readable bytes for the duration of the call.
pub unsafe fn link_key_request_reply(_bdaddr: *mut BdAddr, _key: *mut u8) -> i32 {
    unsafe {
        if cfg!(target_arch = "powerpc") {
            return powerpc_backend::link_key_request_reply(_bdaddr, _key);
        }
        if !lwbt_host_compatibility_enabled() {
            let _ = (_bdaddr, _key);
            return ERR_IF;
        }
        write_stored_link_key(_bdaddr, _key)
    }
}

pub fn link_key_request_negative_reply(_bdaddr: *mut BdAddr) -> i32 {
    if cfg!(target_arch = "powerpc") {
        return powerpc_backend::link_key_request_negative_reply(_bdaddr);
    }
    if !lwbt_host_compatibility_enabled() {
        let _ = _bdaddr;
        return ERR_IF;
    }
    ERR_OK
}

/// # Safety
///
/// Raw pointer arguments and callbacks must be valid for the duration of the call.
pub unsafe fn set_disconnect_callback(callback: BteDisconnectCallback) -> BteDisconnectCallback {
    let previous = unsafe {
        callback_from_ptr::<BteDisconnectCallback>(DISCONNECT_CB.load(Ordering::Acquire))
    };
    DISCONNECT_CB.store(
        callback.map_or(ptr::null_mut(), |cb| cb as *mut c_void),
        Ordering::Release,
    );
    previous
}

pub fn set_host_sync_button_callback(callback: BteHostSyncButtonCallback) {
    HOSTSYNC_HELD.store(1, Ordering::Release);
    HOSTSYNC_CB.store(
        callback.map_or(ptr::null_mut(), |cb| cb as *mut c_void),
        Ordering::Release,
    );
}

pub fn set_connection_request_callback(callback: BteConnectionRequestCallback) {
    CONN_REQ_CB.store(
        callback.map_or(ptr::null_mut(), |cb| cb as *mut c_void),
        Ordering::Release,
    );
}

pub fn set_link_key_request_callback(callback: BteLinkKeyRequestCallback) {
    LINK_KEY_REQ_CB.store(
        callback.map_or(ptr::null_mut(), |cb| cb as *mut c_void),
        Ordering::Release,
    );
}

pub fn set_link_key_notification_callback(callback: BteLinkKeyNotificationCallback) {
    LINK_KEY_NOTIFY_CB.store(
        callback.map_or(ptr::null_mut(), |cb| cb as *mut c_void),
        Ordering::Release,
    );
}

pub(crate) unsafe fn notify_link_key_request(address: &mut BdAddr) -> i8 {
    unsafe {
        let callback =
            callback_from_ptr::<BteLinkKeyRequestCallback>(LINK_KEY_REQ_CB.load(Ordering::Acquire));
        callback.map_or(-1, |callback| callback(ptr::null_mut(), address))
    }
}

pub(crate) unsafe fn notify_connection_request(
    address: &mut BdAddr,
    cod: &mut [u8; 3],
    link_type: u8,
) -> i8 {
    unsafe {
        let callback =
            callback_from_ptr::<BteConnectionRequestCallback>(CONN_REQ_CB.load(Ordering::Acquire));
        callback.map_or(-1, |callback| {
            callback(ptr::null_mut(), address, cod.as_mut_ptr(), link_type)
        })
    }
}

pub(crate) unsafe fn notify_host_sync_button(held: u32) {
    unsafe {
        let callback =
            callback_from_ptr::<BteHostSyncButtonCallback>(HOSTSYNC_CB.load(Ordering::Acquire));
        if let Some(callback) = callback {
            callback(held);
        }
    }
}

pub(crate) unsafe fn notify_link_key_notification(address: &mut BdAddr, key: &mut [u8; 16]) -> i8 {
    unsafe {
        let callback = callback_from_ptr::<BteLinkKeyNotificationCallback>(
            LINK_KEY_NOTIFY_CB.load(Ordering::Acquire),
        );
        callback.map_or(0, |callback| {
            callback(ptr::null_mut(), address, key.as_mut_ptr())
        })
    }
}

pub(crate) unsafe fn notify_disconnect(address: &mut BdAddr, reason: u8) {
    unsafe {
        let callback =
            callback_from_ptr::<BteDisconnectCallback>(DISCONNECT_CB.load(Ordering::Acquire));
        if let Some(callback) = callback {
            callback(address, reason);
        }
    }
}

pub fn get_pair_mode() -> u8 {
    if cfg!(target_arch = "powerpc") {
        return powerpc_backend::get_pair_mode();
    }
    if !lwbt_host_compatibility_enabled() {
        return PAIR_MODE_NORMAL;
    }
    BTE_PAIR_MODE.load(Ordering::Acquire) as u8
}

/// # Safety
///
/// Raw pointer arguments and callbacks must be valid for the duration of the call.
pub unsafe fn write_stored_link_key(_bdaddr: *mut BdAddr, _key: *mut u8) -> i32 {
    unsafe {
        if cfg!(target_arch = "powerpc") {
            return powerpc_backend::write_stored_link_key(_bdaddr, _key);
        }
        if !lwbt_host_compatibility_enabled() {
            let _ = (_bdaddr, _key);
            return ERR_IF;
        }
        {
            if _bdaddr.is_null() || _key.is_null() {
                return ERR_ARG;
            }
            STORED_LINK_KEY_ADDR = *_bdaddr;
            ptr::copy_nonoverlapping(
                _key,
                ptr::addr_of_mut!(STORED_LINK_KEY).cast::<u8>(),
                BD_LINK_KEY_LEN,
            );
            STORED_LINK_KEY_VALID.store(true, Ordering::Release);
            ERR_OK
        }
    }
}

pub fn clear_stored_link_keys() -> i32 {
    if cfg!(target_arch = "powerpc") {
        return powerpc_backend::clear_stored_link_keys();
    }
    if !lwbt_host_compatibility_enabled() {
        return ERR_IF;
    }
    STORED_LINK_KEY_VALID.store(false, Ordering::Release);
    ERR_OK
}

/// # Safety
///
/// Raw pointer arguments and callbacks must be valid for the duration of the call.
pub unsafe fn delete_stored_link_key(_bdaddr: *mut BdAddr) -> i32 {
    if cfg!(target_arch = "powerpc") {
        return powerpc_backend::delete_stored_link_key(_bdaddr);
    }
    if !lwbt_host_compatibility_enabled() {
        let _ = _bdaddr;
        return ERR_IF;
    }
    unsafe {
        if _bdaddr.is_null() {
            return ERR_ARG;
        }
        if STORED_LINK_KEY_VALID.load(Ordering::Acquire) && STORED_LINK_KEY_ADDR == *_bdaddr {
            STORED_LINK_KEY_VALID.store(false, Ordering::Release);
        }
        ERR_OK
    }
}

/// # Safety
///
/// Raw pointer arguments and callbacks must be valid for the duration of the call.
pub unsafe fn new() -> *mut BtePcb {
    for idx in 0..PCB_SLOTS {
        if !PCB_USED[idx].swap(true, Ordering::AcqRel) {
            unsafe {
                PCB_STORE[idx] = BtePcb::default();
                #[cfg(not(target_arch = "powerpc"))]
                {
                    PCB_STORE[idx].bdaddr = deterministic_bdaddr();
                }
                return ptr::addr_of_mut!(PCB_STORE[idx]);
            }
        }
    }
    ptr::null_mut()
}

/// # Safety
///
/// Raw pointer arguments and callbacks must be valid for the duration of the call.
pub unsafe fn free(pcb: *mut BtePcb) {
    #[cfg(target_arch = "powerpc")]
    powerpc_backend::free_transport(pcb);
    if let Some(idx) = pcb_slot(pcb) {
        PCB_USED[idx].store(false, Ordering::Release);
        unsafe { PCB_STORE[idx] = BtePcb::default() };
    }
}

/// # Safety
///
/// Raw pointer arguments and callbacks must be valid for the duration of the call.
pub unsafe fn arg(pcb: *mut BtePcb, arg: *mut c_void) {
    unsafe {
        if valid_pcb(pcb) {
            (*pcb).cbarg = arg;
        }
    }
}

/// # Safety
///
/// Raw pointer arguments and callbacks must be valid for the duration of the call.
pub unsafe fn received(pcb: *mut BtePcb, recv: BteRecvCallback) {
    unsafe {
        if valid_pcb(pcb) {
            (*pcb).recv = recv;
        }
    }
}

/// # Safety
///
/// Raw pointer arguments and callbacks must be valid for the duration of the call.
pub unsafe fn disconnected(pcb: *mut BtePcb, disconn_cfm: BteDisconnectCfmCallback) {
    unsafe {
        if valid_pcb(pcb) {
            (*pcb).disconn_cfm = disconn_cfm;
        }
    }
}

/// # Safety
///
/// Raw pointer arguments and callbacks must be valid for the duration of the call.
pub unsafe fn disconnect(pcb: *mut BtePcb) -> i32 {
    unsafe {
        if cfg!(target_arch = "powerpc") {
            return powerpc_backend::disconnect(pcb);
        }
        if !lwbt_host_compatibility_enabled() {
            let _ = pcb;
            return ERR_IF;
        }
        {
            if !valid_pcb(pcb) {
                return ERR_ARG;
            }
            (*pcb).state = HIDP_STATE_READY;
            if let Some(cb) = (*pcb).disconn_cfm {
                cb((*pcb).cbarg, pcb, 0);
            }
            let disconnect =
                callback_from_ptr::<BteDisconnectCallback>(DISCONNECT_CB.load(Ordering::Acquire));
            if let Some(cb) = disconnect {
                cb(ptr::addr_of_mut!((*pcb).bdaddr), 0);
            }
            ERR_OK
        }
    }
}

fn connect_pcb(
    pcb: *mut BtePcb,
    bdaddr: *mut BdAddr,
    conn_cfm: BteConnCfmCallback,
    state: u32,
) -> i32 {
    if !lwbt_host_compatibility_enabled() {
        let _ = (pcb, bdaddr, conn_cfm, state);
        return ERR_IF;
    }
    if !valid_pcb(pcb) {
        return ERR_ARG;
    }
    unsafe {
        if !bdaddr.is_null() {
            (*pcb).bdaddr = *bdaddr;
        }
        (*pcb).state = state;
        (*pcb).conn_cfm = conn_cfm;
        if let Some(cb) = conn_cfm {
            cb((*pcb).cbarg, pcb, 0);
        }
    }
    ERR_OK
}

/// Starts listening for an incoming HID connection.
///
/// # Safety
///
/// `pcb` must point to a live PCB returned by [`new`] and remain allocated
/// until `conn_cfm` is called or the connection is cancelled. Do not access the
/// PCB concurrently with the backend while the connection is in progress. If
/// non-null, `bdaddr` must be properly aligned and valid for reads for the
/// duration of the call. If supplied, `conn_cfm` and its callback argument must
/// remain valid until the callback is replaced or called.
pub unsafe fn listenasync(
    pcb: *mut BtePcb,
    bdaddr: *mut BdAddr,
    conn_cfm: BteConnCfmCallback,
) -> i32 {
    if cfg!(target_arch = "powerpc") {
        return unsafe { powerpc_backend::listenasync(pcb, bdaddr, conn_cfm) };
    }
    if !lwbt_host_compatibility_enabled() {
        let _ = (pcb, bdaddr, conn_cfm);
        return ERR_IF;
    }
    connect_pcb(pcb, bdaddr, conn_cfm, HIDP_STATE_LISTEN)
}

/// # Safety
///
/// `pcb` must remain allocated until `conn_cfm` is called or the operation is
/// cancelled. `conn_cfm` and its callback argument must remain valid until the
/// callback is replaced or called.
pub unsafe fn listenasync_step2(pcb: *mut BtePcb, conn_cfm: BteConnCfmCallback) -> i32 {
    unsafe {
        if cfg!(target_arch = "powerpc") {
            return powerpc_backend::listenasync_step2(pcb, conn_cfm);
        }
        if !lwbt_host_compatibility_enabled() {
            let _ = (pcb, conn_cfm);
            return ERR_IF;
        }
        connect_pcb(pcb, ptr::null_mut(), conn_cfm, HIDP_STATE_CONNECTED)
    }
}

/// Starts an outgoing HID connection.
///
/// # Safety
///
/// `pcb` must point to a live PCB returned by [`new`] and remain allocated
/// until `conn_cfm` is called or the connection is cancelled. Do not access the
/// PCB concurrently with the backend while the connection is in progress. If
/// non-null, `bdaddr` must be properly aligned and valid for reads for the
/// duration of the call. If supplied, `conn_cfm` and its callback argument must
/// remain valid until the callback is replaced or called.
pub unsafe fn connectasync(
    pcb: *mut BtePcb,
    bdaddr: *mut BdAddr,
    conn_cfm: BteConnCfmCallback,
) -> i32 {
    if cfg!(target_arch = "powerpc") {
        return unsafe { powerpc_backend::connectasync(pcb, bdaddr, conn_cfm) };
    }
    if !lwbt_host_compatibility_enabled() {
        let _ = (pcb, bdaddr, conn_cfm);
        return ERR_IF;
    }
    connect_pcb(pcb, bdaddr, conn_cfm, HIDP_STATE_CONNECTING)
}

/// # Safety
///
/// `pcb` must remain allocated until `conn_cfm` is called or the operation is
/// cancelled. `conn_cfm` and its callback argument must remain valid until the
/// callback is replaced or called.
pub unsafe fn connectasync_step2(pcb: *mut BtePcb, conn_cfm: BteConnCfmCallback) -> i32 {
    unsafe {
        if cfg!(target_arch = "powerpc") {
            return powerpc_backend::connectasync_step2(pcb, conn_cfm);
        }
        if !lwbt_host_compatibility_enabled() {
            let _ = (pcb, conn_cfm);
            return ERR_IF;
        }
        connect_pcb(pcb, ptr::null_mut(), conn_cfm, HIDP_STATE_CONNECTED)
    }
}

/// # Safety
///
/// Raw pointer arguments and callbacks must be valid for the duration of the call.
pub unsafe fn senddata(pcb: *mut BtePcb, message: *mut c_void, len: u16) -> i32 {
    unsafe {
        if cfg!(target_arch = "powerpc") {
            return powerpc_backend::senddata(pcb, message, len);
        }
        if !lwbt_host_compatibility_enabled() {
            let _ = (pcb, message, len);
            return ERR_IF;
        }
        {
            if !valid_pcb(pcb) || message.is_null() || len == 0 {
                return ERR_ARG;
            }
            if let Some(recv) = (*pcb).recv {
                recv((*pcb).cbarg, message, len);
            }
            ERR_OK
        }
    }
}

/// # Safety
///
/// `pcb` must be a live PCB allocated by this module, and `message` must point
/// to `len` readable bytes for the duration of the call.
pub unsafe fn sendmessage(pcb: *mut BtePcb, message: *mut c_void, len: u16) -> i32 {
    unsafe {
        if cfg!(target_arch = "powerpc") {
            return powerpc_backend::sendmessage(pcb, message, len);
        }
        if !lwbt_host_compatibility_enabled() {
            let _ = (pcb, message, len);
            return ERR_IF;
        }
        senddata(pcb, message, len)
    }
}

/// # Safety
///
/// Raw pointer arguments and callbacks must be valid for the duration of the call.
pub unsafe fn sendmessageasync(
    pcb: *mut BtePcb,
    message: *mut c_void,
    len: u16,
    sent: BteConnCfmCallback,
) -> i32 {
    unsafe {
        if cfg!(target_arch = "powerpc") {
            return powerpc_backend::sendmessageasync(pcb, message, len, sent);
        }
        if !lwbt_host_compatibility_enabled() {
            let _ = (pcb, message, len, sent);
            return ERR_IF;
        }
        {
            let rc = senddata(pcb, message, len);
            if rc == ERR_OK
                && let Some(sent) = sent
            {
                sent((*pcb).cbarg, pcb, 0);
            }
            rc
        }
    }
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    extern crate std;

    use super::*;
    use core::mem::size_of;
    use core::sync::atomic::{AtomicI32, Ordering};
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());
    static CALLBACK_RESULT: AtomicI32 = AtomicI32::new(-99);
    static DISCONNECT_COUNT: AtomicI32 = AtomicI32::new(0);

    #[test]
    fn pcb_pool_matches_fourteen_entry_c_capacity() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let mut allocated = [ptr::null_mut(); PCB_SLOTS];
        for slot in &mut allocated {
            *slot = unsafe { new() };
            assert!(!slot.is_null());
        }
        assert!(unsafe { new() }.is_null());
        for pcb in allocated {
            unsafe { free(pcb) };
        }
    }

    unsafe extern "C" fn init_cb(result: i32, _userdata: *mut c_void) -> i32 {
        CALLBACK_RESULT.store(result, Ordering::Release);
        result
    }

    unsafe extern "C" fn conn_cb(_arg: *mut c_void, _pcb: *mut BtePcb, err: u8) -> i32 {
        CALLBACK_RESULT.store(err as i32, Ordering::Release);
        0
    }

    unsafe extern "C" fn recv_cb(_arg: *mut c_void, _buffer: *mut c_void, len: u16) -> i32 {
        CALLBACK_RESULT.store(len as i32, Ordering::Release);
        0
    }

    unsafe extern "C" fn disconn_cb(_arg: *mut c_void, _pcb: *mut BtePcb, err: u8) -> i32 {
        CALLBACK_RESULT.store(100 + err as i32, Ordering::Release);
        0
    }

    unsafe extern "C" fn global_disconn_cb(_bdaddr: *mut BdAddr, _reason: u8) {
        DISCONNECT_COUNT.fetch_add(1, Ordering::AcqRel);
    }

    #[test]
    fn constants_and_layouts_match_checkpoint() {
        let _guard = match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        assert_eq!(ERR_OK, 0);
        assert_eq!(HIDP_STATE_CONNECTED, 0x04);
        assert_eq!(BD_ADDR_LEN, 6);
        assert!(size_of::<BtePcb>() > 0);
        assert_eq!(size_of::<BdAddr>(), 6);
        let _ = try_init as fn() -> i32;
        let _ = try_shutdown as fn() -> i32;
        let _ = last_status as fn() -> i32;
    }

    #[test]
    fn init_and_pair_mode_callbacks_are_host_compat() {
        let _guard = match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        CALLBACK_RESULT.store(-99, Ordering::Release);
        shutdown();
        assert_eq!(init_core(Some(init_cb)), ERR_OK);
        assert_eq!(CALLBACK_RESULT.load(Ordering::Acquire), ERR_OK);
        assert_eq!(periodic_inquiry(4, 0, Some(init_cb)), ERR_OK);
        assert_eq!(get_pair_mode(), PAIR_MODE_TEMPORARY);
        assert_eq!(exit_periodic_inquiry(), ERR_OK);
        assert_eq!(get_pair_mode(), PAIR_MODE_NORMAL);
    }

    #[test]
    fn pcb_lifecycle_connect_send_and_disconnect_are_host_compat() {
        let _guard = match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        unsafe {
            shutdown();
            let pcb = new();
            assert!(!pcb.is_null());
            received(pcb, Some(recv_cb));
            disconnected(pcb, Some(disconn_cb));
            assert!(set_disconnect_callback(Some(global_disconn_cb)).is_none());
            assert!(set_disconnect_callback(None).is_some());
            set_disconnect_callback(Some(global_disconn_cb));
            let mut addr = deterministic_bdaddr();
            assert_eq!(connectasync(pcb, &mut addr, Some(conn_cb)), ERR_OK);
            assert_eq!((*pcb).state, HIDP_STATE_CONNECTING);
            assert_eq!(connectasync_step2(pcb, Some(conn_cb)), ERR_OK);
            assert_eq!((*pcb).state, HIDP_STATE_CONNECTED);
            let mut payload = [1_u8, 2, 3];
            assert_eq!(
                sendmessage(pcb, payload.as_mut_ptr().cast(), payload.len() as u16),
                ERR_OK
            );
            assert_eq!(
                CALLBACK_RESULT.load(Ordering::Acquire),
                payload.len() as i32
            );
            assert_eq!(disconnect(pcb), ERR_OK);
            assert_eq!(DISCONNECT_COUNT.load(Ordering::Acquire), 1);
            assert_eq!((*pcb).state, HIDP_STATE_READY);
            free(pcb);
        }
    }

    #[test]
    fn read_helpers_fill_deterministic_identity() {
        let _guard = match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        unsafe {
            shutdown();
            let mut addr = BdAddr::default();
            let mut name = PadNameInfo::default();
            let mut key = LinkKeyInfo::default();
            assert_eq!(read_bd_addr(&mut addr, None), ERR_OK);
            assert_eq!(addr, deterministic_bdaddr());
            assert_eq!(read_remote_name(&mut name, None), ERR_OK);
            assert_eq!(name.bdaddr, deterministic_bdaddr());
            assert_eq!(read_stored_link_key(&mut key, 1, None), ERR_OK);
            assert_eq!(key.bdaddr, deterministic_bdaddr());
            assert_eq!(key.key[0], 0x11);
        }
    }

    #[test]
    fn stored_link_key_write_delete_and_clear_are_host_compat() {
        let _guard = match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        unsafe {
            shutdown();
            let mut addr = BdAddr {
                addr: [6, 5, 4, 3, 2, 1],
            };
            let mut key_data = [0x44; BD_LINK_KEY_LEN];
            let mut key = LinkKeyInfo::default();

            assert_eq!(
                write_stored_link_key(&mut addr, key_data.as_mut_ptr()),
                ERR_OK
            );
            assert_eq!(read_stored_link_key(&mut key, 1, None), ERR_OK);
            assert_eq!(key.bdaddr, addr);
            assert_eq!(key.key, key_data);

            assert_eq!(delete_stored_link_key(&mut addr), ERR_OK);
            assert_eq!(read_stored_link_key(&mut key, 1, None), ERR_OK);
            assert_eq!(key.bdaddr, deterministic_bdaddr());

            key_data.fill(0x55);
            assert_eq!(
                link_key_request_reply(&mut addr, key_data.as_mut_ptr()),
                ERR_OK
            );
            assert_eq!(read_stored_link_key(&mut key, 1, None), ERR_OK);
            assert_eq!(key.bdaddr, addr);
            assert_eq!(key.key, key_data);

            assert_eq!(clear_stored_link_keys(), ERR_OK);
            assert_eq!(read_stored_link_key(&mut key, 1, None), ERR_OK);
            assert_eq!(key.bdaddr, deterministic_bdaddr());
        }
    }

    #[test]
    fn invalid_pcb_and_link_key_inputs_return_err_arg() {
        let _guard = match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        unsafe {
            shutdown();
            let mut addr = deterministic_bdaddr();
            let mut payload = [1_u8, 2, 3];
            let mut key_data = [0_u8; BD_LINK_KEY_LEN];
            assert_eq!(disconnect(ptr::null_mut()), ERR_ARG);
            assert_eq!(
                senddata(
                    ptr::null_mut(),
                    payload.as_mut_ptr().cast(),
                    payload.len() as u16
                ),
                ERR_ARG
            );
            assert_eq!(
                write_stored_link_key(ptr::null_mut(), key_data.as_mut_ptr()),
                ERR_ARG
            );
            assert_eq!(write_stored_link_key(&mut addr, ptr::null_mut()), ERR_ARG);
            assert_eq!(delete_stored_link_key(ptr::null_mut()), ERR_ARG);
        }
    }
}
