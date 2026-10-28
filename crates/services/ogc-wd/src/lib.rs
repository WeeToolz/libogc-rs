#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

extern crate alloc;

use core::mem::size_of;

pub use ogc_sys::{ErrorCode, Result};

pub const SSID_LENGTH: usize = 32;
pub const BSSID_LENGTH: usize = 6;

pub const IOCTLV_WD_INVALID: u32 = 0x1000;
pub const IOCTLV_WD_GET_MODE: u32 = 0x1001;
pub const IOCTLV_WD_SET_LINKSTATE: u32 = 0x1002;
pub const IOCTLV_WD_GET_LINKSTATE: u32 = 0x1003;
pub const IOCTLV_WD_SET_CONFIG: u32 = 0x1004;
pub const IOCTLV_WD_GET_CONFIG: u32 = 0x1005;
pub const IOCTLV_WD_CHANGE_BEACON: u32 = 0x1006;
pub const IOCTLV_WD_DISASSOC: u32 = 0x1007;
pub const IOCTLV_WD_MP_SEND_FRAME: u32 = 0x1008;
pub const IOCTLV_WD_SEND_FRAME: u32 = 0x1009;
pub const IOCTLV_WD_SCAN: u32 = 0x100A;
pub const IOCTLV_WD_MEASURE_CHANNEL: u32 = 0x100B;
pub const IOCTLV_WD_CALL_WL: u32 = 0x100C;
pub const IOCTLV_WD_GET_LASTERROR: u32 = 0x100D;
pub const IOCTLV_WD_GET_INFO: u32 = 0x100E;
pub const IOCTLV_WD_CHANGE_GAMEINFO: u32 = 0x100F;
pub const IOCTLV_WD_CHANGE_VTSF: u32 = 0x1010;
pub const IOCTLV_WD_RECV_FRAME: u32 = 0x8000;
pub const IOCTLV_WD_RECV_NOTIFICATION: u32 = 0x8001;

pub const WD_SIGNAL_STRONG: u8 = 3;
pub const WD_SIGNAL_NORMAL: u8 = 2;
pub const WD_SIGNAL_FAIR: u8 = 1;
pub const WD_SIGNAL_WEAK: u8 = 0;

pub const UNINITIALIZED: u32 = 0;
pub const DSCOMMUNICATIONS: u32 = 1;
pub const AOSSAP_SCAN: u32 = 3;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ScanStage {
    Platform,
    NetworkDriverLock,
    DriverOpen,
    Scan,
    DriverClose,
    NetworkDriverUnlock,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ScanFailure {
    pub stage: ScanStage,
    pub code: i32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct WdInfo {
    pub mac: [u8; 6],
    pub enable_channels_mask: u16,
    pub ntrallowed_channels_mask: u16,
    pub country_code: [u8; 4],
    pub channel: u8,
    pub initialized: u8,
    pub version: [u8; 80],
    pub unknown: [u8; 48],
}

impl Default for WdInfo {
    fn default() -> Self {
        Self {
            mac: [0; 6],
            enable_channels_mask: 0,
            ntrallowed_channels_mask: 0,
            country_code: [0; 4],
            channel: 0,
            initialized: 0,
            version: [0; 80],
            unknown: [0; 48],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ScanParameters {
    pub channel_bitmap: u16,
    pub max_channel_time: u16,
    pub bssid: [u8; BSSID_LENGTH],
    pub scan_type: u16,
    pub ssid_length: u16,
    pub ssid: [u8; SSID_LENGTH],
    pub ssid_match_mask: [u8; 6],
}

impl Default for ScanParameters {
    fn default() -> Self {
        Self {
            channel_bitmap: 0,
            max_channel_time: 0,
            bssid: [0; BSSID_LENGTH],
            scan_type: 0,
            ssid_length: 0,
            ssid: [0; SSID_LENGTH],
            ssid_match_mask: [0; 6],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct BssDescriptor {
    pub length: u16,
    pub rssi: u16,
    pub bssid: [u8; 6],
    pub ssid_length: u16,
    pub ssid: [u8; 32],
    pub capabilities: u16,
    pub rate_set_basic: u16,
    pub rate_set_support: u16,
    pub beacon_period: u16,
    pub dtim_period: u16,
    pub channel: u16,
    pub cf_period: u16,
    pub cf_max_duration: u16,
    pub ies_length: u16,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct IeHdr {
    pub id: u8,
    pub len: u8,
}

#[cfg(target_arch = "powerpc")]
mod runtime {
    use super::*;
    use core::ptr::{addr_of_mut, copy_nonoverlapping};
    use core::sync::atomic::{AtomicI32, Ordering};

    use ogc_ipc::{FileDescriptor, IoctlvEntry, open_file};
    use ogc_system_sys::sys_time;

    const IOCTLV_NCD_LOCK: i32 = 0x01;
    const IOCTLV_NCD_UNLOCK: i32 = 0x02;
    const WD_OPEN_MODE_BASE: u32 = 0x10000;
    const WD_CLOSING: i32 = -2;

    #[inline]
    fn supports_wii_wireless_driver() -> bool {
        !cfg!(gamecube)
    }

    #[repr(C, align(32))]
    struct AlignedBytes<const N: usize>([u8; N]);

    pub(super) struct ScanBuffer {
        pointer: core::ptr::NonNull<u8>,
        layout: alloc::alloc::Layout,
    }

    impl ScanBuffer {
        pub(super) fn new(buffsize: u16) -> core::result::Result<Self, i32> {
            let layout = alloc::alloc::Layout::from_size_align(usize::from(buffsize) + 2, 32)
                .map_err(|_| ogc_ipc::IPC_EINVAL)?;
            // SAFETY: The layout has nonzero size and valid alignment.
            let pointer = core::ptr::NonNull::new(unsafe { alloc::alloc::alloc_zeroed(layout) })
                .ok_or(ogc_ipc::IPC_ENOMEM)?;
            Ok(Self { pointer, layout })
        }

        pub(super) fn bytes(&mut self) -> &mut [u8] {
            // SAFETY: This allocation owns layout.size() initialized bytes.
            unsafe { core::slice::from_raw_parts_mut(self.pointer.as_ptr(), self.layout.size()) }
        }
    }

    impl Drop for ScanBuffer {
        fn drop(&mut self) {
            // SAFETY: The allocation is released once with its original layout.
            unsafe { alloc::alloc::dealloc(self.pointer.as_ptr(), self.layout) };
        }
    }

    #[repr(C, align(32))]
    struct AlignedWdInfo(WdInfo);

    static WD_FD: AtomicI32 = AtomicI32::new(-1);
    static mut NCD_COMMON_BUFFER: AlignedBytes<0x20> = AlignedBytes([0; 0x20]);

    #[inline]
    fn wd_fd() -> Option<FileDescriptor> {
        let raw = WD_FD.load(Ordering::Acquire);
        (raw >= 0).then(|| FileDescriptor::new(raw))
    }

    fn finish_file_operation(fd: FileDescriptor, operation_result: i32) -> i32 {
        match fd.close() {
            Ok(()) => operation_result,
            Err(_) if operation_result < 0 => operation_result,
            Err(close_error) => close_error,
        }
    }

    fn unlock_status(result: u32) -> i32 {
        if result == 0 {
            0
        } else {
            i32::try_from(result).unwrap_or(-1)
        }
    }

    fn first_error(primary: i32, cleanup: i32) -> i32 {
        if primary < 0 { primary } else { cleanup }
    }

    #[inline]
    unsafe fn bytes_of_mut<T>(value: &mut T) -> &mut [u8] {
        unsafe {
            core::slice::from_raw_parts_mut(
                (value as *mut T).cast::<u8>(),
                core::mem::size_of::<T>(),
            )
        }
    }

    #[inline]
    unsafe fn static_bytes<const N: usize>(ptr: *const AlignedBytes<N>) -> &'static [u8] {
        unsafe { core::slice::from_raw_parts((*ptr).0.as_ptr(), N) }
    }

    #[inline]
    unsafe fn static_bytes_mut<const N: usize>(ptr: *mut AlignedBytes<N>) -> &'static mut [u8] {
        unsafe { core::slice::from_raw_parts_mut((*ptr).0.as_mut_ptr(), N) }
    }

    #[inline]
    fn scan_settle_delay() {
        const TB_TICKS_PER_MILLISECOND: u64 = 60_750;
        let start = unsafe { sys_time() };
        let delay = 100 * TB_TICKS_PER_MILLISECOND;
        while (unsafe { sys_time() }).wrapping_sub(start) < delay {
            core::hint::spin_loop();
        }
    }

    pub fn ncd_lock_wireless_driver() -> i32 {
        if !supports_wii_wireless_driver() {
            return -1;
        }
        let Ok(fd) = open_file(c"/dev/net/ncd/manage", 0) else {
            return -1;
        };
        let common = unsafe { static_bytes_mut(addr_of_mut!(NCD_COMMON_BUFFER)) };
        common.fill(0);
        let rc = fd.ioctlv(IOCTLV_NCD_LOCK, 0, [IoctlvEntry::Out(common)]);
        let lockid = i32::from_ne_bytes(common[..4].try_into().unwrap());
        let result = finish_file_operation(fd, rc);
        if result < 0 {
            return result;
        }
        lockid
    }

    pub fn ncd_unlock_wireless_driver(lockid: i32) -> u32 {
        if !supports_wii_wireless_driver() {
            return u32::MAX;
        }
        let Ok(fd) = open_file(c"/dev/net/ncd/manage", 0) else {
            return u32::MAX;
        };
        let common = unsafe { static_bytes(addr_of_mut!(NCD_COMMON_BUFFER).cast_const()) };
        let mut reply = AlignedBytes([0u8; 4]);
        reply.0.copy_from_slice(&lockid.to_ne_bytes());
        let rc = fd.ioctlv(
            IOCTLV_NCD_UNLOCK,
            1,
            [IoctlvEntry::In(common), IoctlvEntry::Out(&mut reply.0)],
        );
        let result = u32::from_ne_bytes(reply.0);
        let rc = finish_file_operation(fd, rc);
        if rc < 0 { u32::MAX } else { result }
    }

    pub fn init(mode: u8) -> i32 {
        if !supports_wii_wireless_driver() {
            return -1;
        }
        let state = WD_FD.load(Ordering::Acquire);
        if state >= 0 {
            return 0;
        }
        if state == WD_CLOSING {
            return -1;
        }
        match open_file(c"/dev/net/wd/command", WD_OPEN_MODE_BASE | mode as u32) {
            Ok(fd) => {
                let raw = fd.raw();
                WD_FD.store(raw, Ordering::Release);
                0
            }
            Err(rc) => rc,
        }
    }

    pub fn deinit() -> i32 {
        if !supports_wii_wireless_driver() {
            return -1;
        }
        let raw = WD_FD.load(Ordering::Acquire);
        if raw < 0 {
            return 0;
        }
        if WD_FD
            .compare_exchange(raw, WD_CLOSING, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return -1;
        }
        match FileDescriptor::new(raw).close() {
            Ok(()) => {
                WD_FD.store(-1, Ordering::Release);
                0
            }
            Err(rc) => {
                WD_FD.store(raw, Ordering::Release);
                rc
            }
        }
    }

    /// # Safety
    /// `info` must be null or writable for one properly aligned [`WdInfo`].
    pub unsafe fn get_info(info: *mut WdInfo) -> i32 {
        if !supports_wii_wireless_driver() {
            return -1;
        }
        if info.is_null() {
            return -1;
        }

        let lockid = ncd_lock_wireless_driver();
        if lockid < 0 {
            return lockid;
        }

        let init_rc = init(AOSSAP_SCAN as u8);
        if init_rc < 0 {
            let unlock_rc = unlock_status(ncd_unlock_wireless_driver(lockid));
            return first_error(init_rc, unlock_rc);
        }

        let mut wd_info = AlignedWdInfo(WdInfo::default());
        let rc = match wd_fd() {
            Some(fd) => fd.ioctlv(
                IOCTLV_WD_GET_INFO as i32,
                0,
                [IoctlvEntry::Out(unsafe { bytes_of_mut(&mut wd_info.0) })],
            ),
            None => -1,
        };

        let deinit_rc = deinit();
        let unlock_rc = unlock_status(ncd_unlock_wireless_driver(lockid));

        if rc < 0 {
            return rc;
        }
        if deinit_rc < 0 {
            return deinit_rc;
        }
        if unlock_rc < 0 {
            return unlock_rc;
        }
        unsafe {
            *info = wd_info.0;
        }
        0
    }

    /// # Safety
    ///
    /// `settings` must reference a scan configuration and `buff` must be
    /// writable for `buffsize` bytes.
    pub unsafe fn scan(settings: *mut ScanParameters, buff: *mut u8, buffsize: u16) -> i32 {
        if !supports_wii_wireless_driver() {
            return -1;
        }
        if settings.is_null() || buff.is_null() {
            return -1;
        }
        let Some(fd) = wd_fd() else {
            return -1;
        };

        let mut settings_buf = AlignedBytes([0u8; 0x4e]);
        unsafe {
            let settings_src = core::slice::from_raw_parts(settings.cast::<u8>(), 0x1a);
            settings_buf.0[..0x1a].copy_from_slice(settings_src);
        }

        let mut scan_buffer = match ScanBuffer::new(buffsize) {
            Ok(buffer) => buffer,
            Err(rc) => return rc,
        };

        {
            let scan_slice = scan_buffer.bytes();
            let rc = fd.ioctlv(
                IOCTLV_WD_SCAN as i32,
                1,
                [
                    IoctlvEntry::In(&settings_buf.0),
                    IoctlvEntry::Out(&mut scan_slice[..buffsize as usize]),
                ],
            );
            if rc < 0 {
                return rc;
            }
            scan_settle_delay();
            unsafe {
                copy_nonoverlapping(scan_slice.as_ptr(), buff, buffsize as usize);
            }
            0
        }
    }

    /// # Safety
    ///
    /// `settings` must reference a scan configuration and `buff` must be
    /// writable for `buffsize` bytes.
    pub unsafe fn scan_once_detailed(
        settings: *mut ScanParameters,
        buff: *mut u8,
        buffsize: u16,
    ) -> core::result::Result<(), ScanFailure> {
        unsafe {
            if !supports_wii_wireless_driver() {
                return Err(ScanFailure {
                    stage: ScanStage::Platform,
                    code: -1,
                });
            }
            let lockid = ncd_lock_wireless_driver();
            if lockid < 0 {
                return Err(ScanFailure {
                    stage: ScanStage::NetworkDriverLock,
                    code: lockid,
                });
            }

            let init_rc = init(AOSSAP_SCAN as u8);
            if init_rc < 0 {
                let _unlock_rc = unlock_status(ncd_unlock_wireless_driver(lockid));
                return Err(ScanFailure {
                    stage: ScanStage::DriverOpen,
                    code: init_rc,
                });
            }

            let scan_rc = scan(settings, buff, buffsize);
            let deinit_rc = deinit();
            let unlock_rc = unlock_status(ncd_unlock_wireless_driver(lockid));
            if scan_rc < 0 {
                Err(ScanFailure {
                    stage: ScanStage::Scan,
                    code: scan_rc,
                })
            } else if deinit_rc < 0 {
                Err(ScanFailure {
                    stage: ScanStage::DriverClose,
                    code: deinit_rc,
                })
            } else if unlock_rc < 0 {
                Err(ScanFailure {
                    stage: ScanStage::NetworkDriverUnlock,
                    code: unlock_rc,
                })
            } else {
                Ok(())
            }
        }
    }

    /// # Safety
    /// The pointers must follow the contract of [`scan_once_detailed`].
    pub unsafe fn scan_once(settings: *mut ScanParameters, buff: *mut u8, buffsize: u16) -> i32 {
        unsafe { scan_once_detailed(settings, buff, buffsize) }
            .map_or_else(|failure| failure.code, |()| 0)
    }
}

#[cfg(not(target_arch = "powerpc"))]
mod runtime {
    use super::*;

    pub fn ncd_lock_wireless_driver() -> i32 {
        -1
    }

    pub fn ncd_unlock_wireless_driver(_lockid: i32) -> u32 {
        u32::MAX
    }

    pub fn init(_mode: u8) -> i32 {
        -1
    }

    pub fn deinit() -> i32 {
        -1
    }

    /// # Safety
    /// A non-null `_info` must be writable for one properly aligned [`WdInfo`].
    pub unsafe fn get_info(_info: *mut WdInfo) -> i32 {
        -1
    }

    pub fn scan(_settings: *mut ScanParameters, _buff: *mut u8, _buffsize: u16) -> i32 {
        -1
    }

    pub fn scan_once(_settings: *mut ScanParameters, _buff: *mut u8, _buffsize: u16) -> i32 {
        -1
    }

    /// # Safety
    /// A non-null `_settings` must point to `ScanParameters`; `_buff` must
    /// point to at least `_buffsize` writable bytes.
    pub unsafe fn scan_once_detailed(
        _settings: *mut ScanParameters,
        _buff: *mut u8,
        _buffsize: u16,
    ) -> core::result::Result<(), ScanFailure> {
        Err(ScanFailure {
            stage: ScanStage::Platform,
            code: -1,
        })
    }
}

pub use runtime::{
    deinit, get_info, init, ncd_lock_wireless_driver, ncd_unlock_wireless_driver, scan, scan_once,
    scan_once_detailed,
};

pub fn try_wd_init(mode: u8) -> Result<()> {
    ogc_sys::unit_from_status(init(mode))
}

pub fn try_wd_deinit() -> Result<()> {
    ogc_sys::unit_from_status(deinit())
}

pub fn get_info_result() -> Result<WdInfo> {
    let mut info = WdInfo::default();
    // SAFETY: `info` is a valid unique output reference.
    ogc_sys::unit_from_status(unsafe { get_info(&mut info) }).map(|()| info)
}

/// Returns a coarse signal bucket derived from the descriptor RSSI.
///
/// # Safety
///
/// `bss` must be either null or a valid pointer to a `BssDescriptor`.
pub unsafe fn get_radio_level(bss: *const BssDescriptor) -> u8 {
    if bss.is_null() {
        return WD_SIGNAL_WEAK;
    }
    unsafe {
        match (*bss).rssi {
            0xc4..=u16::MAX => WD_SIGNAL_STRONG,
            0xb5..=0xc3 => WD_SIGNAL_NORMAL,
            0xab..=0xb4 => WD_SIGNAL_FAIR,
            _ => WD_SIGNAL_WEAK,
        }
    }
}

/// Fills the scan parameter block with libogc-style defaults.
///
/// # Safety
///
/// `set` must be either null or a valid, writable pointer to `ScanParameters`.
pub unsafe fn set_default_scan_parameters(set: *mut ScanParameters) {
    if set.is_null() {
        return;
    }
    unsafe {
        (*set).channel_bitmap = 0xfffe;
        (*set).max_channel_time = 100;
        (*set).bssid = [0xff; BSSID_LENGTH];
        (*set).scan_type = 0;
        (*set).ssid_length = 0;
        (*set).ssid = [0; SSID_LENGTH];
        (*set).ssid_match_mask = [0xff; 6];
    }
}

pub fn default_scan_parameters() -> ScanParameters {
    let mut params = ScanParameters::default();
    unsafe { set_default_scan_parameters(&mut params) };
    params
}

pub fn scan_result(settings: &mut ScanParameters, buffer: &mut [u8]) -> Result<()> {
    ogc_sys::unit_from_status(unsafe {
        scan(
            settings,
            buffer.as_mut_ptr(),
            buffer.len().min(u16::MAX as usize) as u16,
        )
    })
}

pub fn scan_once_result(settings: &mut ScanParameters, buffer: &mut [u8]) -> Result<()> {
    ogc_sys::unit_from_status(unsafe {
        scan_once(
            settings,
            buffer.as_mut_ptr(),
            buffer.len().min(u16::MAX as usize) as u16,
        )
    })
}

pub fn scan_once_detailed_result(
    settings: &mut ScanParameters,
    buffer: &mut [u8],
) -> core::result::Result<(), ScanFailure> {
    unsafe {
        scan_once_detailed(
            settings,
            buffer.as_mut_ptr(),
            buffer.len().min(u16::MAX as usize) as u16,
        )
    }
}

unsafe fn ie_bytes<'a>(bss: *const BssDescriptor, backing_len: usize) -> Option<&'a [u8]> {
    unsafe {
        if bss.is_null() || backing_len < size_of::<BssDescriptor>() {
            return None;
        }
        let ies_len = core::ptr::read_unaligned(core::ptr::addr_of!((*bss).ies_length)) as usize;
        let total = size_of::<BssDescriptor>().checked_add(ies_len)?;
        if total > backing_len {
            return None;
        }
        Some(core::slice::from_raw_parts(
            bss.cast::<u8>().add(size_of::<BssDescriptor>()),
            ies_len,
        ))
    }
}

fn find_ie(ies: &[u8], id: u8) -> Option<&[u8]> {
    let mut offset = 0usize;
    while offset < ies.len() {
        let header_end = offset.checked_add(size_of::<IeHdr>())?;
        if header_end > ies.len() {
            return None;
        }
        let len = ies[offset + 1] as usize;
        let end = header_end.checked_add(len)?;
        if end > ies.len() {
            return None;
        }
        if ies[offset] == id {
            return Some(&ies[header_end..end]);
        }
        if len == 0 {
            break;
        }
        offset = end;
    }
    None
}

/// Returns the number of complete information elements in the descriptor.
///
/// # Safety
///
/// `bss` must point to `backing_len` readable bytes.
pub unsafe fn get_number_of_ies(bss: *const BssDescriptor, backing_len: usize) -> u8 {
    unsafe {
        let Some(ies) = ie_bytes(bss, backing_len) else {
            return 0;
        };
        if ies.len() < size_of::<IeHdr>() {
            return 0;
        }
        let mut count = 0u8;
        let mut offset = 0usize;
        while offset < ies.len() {
            let Some(header_end) = offset.checked_add(size_of::<IeHdr>()) else {
                break;
            };
            if header_end > ies.len() {
                break;
            }
            let len = ies[offset + 1] as usize;
            if len == 0 {
                break;
            }
            let Some(end) = header_end.checked_add(len) else {
                break;
            };
            if end > ies.len() {
                break;
            }
            count = count.wrapping_add(1);
            offset = end;
        }
        count
    }
}

pub fn number_of_ies(bss: &[u8]) -> u8 {
    unsafe { get_number_of_ies(bss.as_ptr().cast(), bss.len()) }
}

/// Returns the length of a specific information element.
///
/// # Safety
///
/// `bss` must point to `backing_len` readable bytes.
pub unsafe fn get_ie_length(bss: *const BssDescriptor, backing_len: usize, id: u8) -> i32 {
    unsafe {
        ie_bytes(bss, backing_len)
            .and_then(|ies| find_ie(ies, id))
            .map_or(-1, |ie| ie.len() as i32)
    }
}

pub fn ie_length(bss: &[u8], id: u8) -> Result<usize> {
    ogc_sys::result_from_status(unsafe { get_ie_length(bss.as_ptr().cast(), bss.len(), id) })
        .map(|len| len as usize)
}

/// Copies a specific information element into the caller-provided buffer.
///
/// # Safety
///
/// `bss` must point to `backing_len` readable bytes and `buff` must point to
/// `buffsize` writable bytes.
pub unsafe fn get_ie(
    bss: *const BssDescriptor,
    backing_len: usize,
    id: u8,
    buff: *mut u8,
    buffsize: usize,
) -> i32 {
    unsafe {
        if buff.is_null() {
            return -2;
        }
        let Some(ie) = ie_bytes(bss, backing_len).and_then(|ies| find_ie(ies, id)) else {
            return -1;
        };
        if ie.len() > buffsize {
            return -2;
        }
        core::ptr::write_bytes(buff, 0, buffsize);
        core::ptr::copy_nonoverlapping(ie.as_ptr(), buff, ie.len());
        0
    }
}

pub fn copy_ie(bss: &[u8], id: u8, buffer: &mut [u8]) -> Result<()> {
    ogc_sys::unit_from_status(unsafe {
        get_ie(
            bss.as_ptr().cast(),
            bss.len(),
            id,
            buffer.as_mut_ptr(),
            buffer.len(),
        )
    })
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;
    use core::mem::{offset_of, size_of};
    use core::sync::atomic::{AtomicBool, Ordering};

    static TEST_LOCK: AtomicBool = AtomicBool::new(false);

    fn test_guard() -> TestGuard<'static> {
        while TEST_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        TestGuard { lock: &TEST_LOCK }
    }

    struct TestGuard<'a> {
        lock: &'a AtomicBool,
    }

    impl Drop for TestGuard<'_> {
        fn drop(&mut self) {
            self.lock.store(false, Ordering::Release);
        }
    }

    #[test]
    fn helpers_exist() {
        let _guard = test_guard();
        let _ = ncd_lock_wireless_driver as fn() -> i32;
        let _ = ncd_unlock_wireless_driver as fn(i32) -> u32;
        let _ = init as fn(u8) -> i32;
        let _ = deinit as fn() -> i32;
        let _ = get_info as unsafe fn(*mut WdInfo) -> i32;
        let _ = get_radio_level as unsafe fn(*const BssDescriptor) -> u8;
        let _ = set_default_scan_parameters as unsafe fn(*mut ScanParameters);
        let _ = scan as unsafe fn(*mut ScanParameters, *mut u8, u16) -> i32;
        let _ = scan_once as unsafe fn(*mut ScanParameters, *mut u8, u16) -> i32;
        let _ = get_number_of_ies as unsafe fn(*const BssDescriptor, usize) -> u8;
        let _ = get_ie_length as unsafe fn(*const BssDescriptor, usize, u8) -> i32;
        let _ = get_ie as unsafe fn(*const BssDescriptor, usize, u8, *mut u8, usize) -> i32;
    }

    #[test]
    fn default_scan_parameters_match_c_values() {
        let _guard = test_guard();
        let mut params = ScanParameters::default();
        set_default_scan_parameters(&mut params);
        assert_eq!(params.channel_bitmap, 0xfffe);
        assert_eq!(params.max_channel_time, 100);
        assert_eq!(params.bssid, [0xff; BSSID_LENGTH]);
        assert_eq!(params.scan_type, 0);
        assert_eq!(params.ssid_length, 0);
        assert_eq!(params.ssid, [0; SSID_LENGTH]);
        assert_eq!(params.ssid_match_mask, [0xff; 6]);
    }

    #[test]
    fn scan_buffers_support_repeated_aligned_full_size_allocations() {
        for size in [16_384, u16::MAX, 16_384, 0] {
            let mut buffer = runtime::ScanBuffer::new(size).unwrap();
            let bytes = buffer.bytes();
            assert_eq!(bytes.len(), usize::from(size) + 2);
            assert_eq!(bytes.as_ptr() as usize % 32, 0);
            assert!(bytes.iter().all(|byte| *byte == 0));
            bytes.fill(0x5a);
        }
    }

    #[test]
    fn ie_helpers_match_c_traversal_rules() {
        let _guard = test_guard();
        #[repr(align(4))]
        struct AlignedBuf([u8; size_of::<BssDescriptor>() + 9]);

        let mut bytes = AlignedBuf([0u8; size_of::<BssDescriptor>() + 9]);
        let ies_len_off = offset_of!(BssDescriptor, ies_length);
        let ies_len_ptr = bytes.0.as_mut_ptr().wrapping_add(ies_len_off).cast::<u16>();
        unsafe {
            *ies_len_ptr = 9;
        }
        bytes.0[size_of::<BssDescriptor>()] = 0x01;
        bytes.0[size_of::<BssDescriptor>() + 1] = 2;
        bytes.0[size_of::<BssDescriptor>() + 2] = 0xaa;
        bytes.0[size_of::<BssDescriptor>() + 3] = 0xbb;
        bytes.0[size_of::<BssDescriptor>() + 4] = 0x05;
        bytes.0[size_of::<BssDescriptor>() + 5] = 1;
        bytes.0[size_of::<BssDescriptor>() + 6] = 0xcc;
        bytes.0[size_of::<BssDescriptor>() + 7] = 0x09;
        bytes.0[size_of::<BssDescriptor>() + 8] = 0;

        let ptr = bytes.0.as_mut_ptr().cast::<BssDescriptor>();
        unsafe {
            assert_eq!(get_number_of_ies(ptr, bytes.0.len()), 2);
            assert_eq!(get_ie_length(ptr, bytes.0.len(), 0x01), 2);
            assert_eq!(get_ie_length(ptr, bytes.0.len(), 0x05), 1);
            assert_eq!(get_ie_length(ptr, bytes.0.len(), 0x09), 0);

            let mut out = [0u8; 4];
            assert_eq!(
                get_ie(ptr, bytes.0.len(), 0x05, out.as_mut_ptr(), out.len()),
                0
            );
            assert_eq!(&out[..], &[0xcc, 0, 0, 0]);
            assert_eq!(
                get_ie(ptr, bytes.0.len(), 0x09, out.as_mut_ptr(), out.len()),
                0
            );
            assert_eq!(
                get_ie(ptr, bytes.0.len(), 0x01, core::ptr::null_mut(), out.len(),),
                -2
            );
            assert_eq!(get_ie(ptr, bytes.0.len(), 0x01, out.as_mut_ptr(), 1), -2);
            assert_eq!(get_ie_length(ptr, size_of::<BssDescriptor>(), 0x01), -1);
        }
        assert_eq!(number_of_ies(&bytes.0), 2);
        assert_eq!(ie_length(&bytes.0, 0x05), Ok(1));
        let mut out = [0u8; 1];
        assert_eq!(copy_ie(&bytes.0, 0x05, &mut out), Ok(()));
        assert_eq!(out, [0xcc]);
    }

    #[test]
    fn ie_helpers_treat_leading_zero_length_ie_as_terminator() {
        let _guard = test_guard();
        #[repr(align(4))]
        struct AlignedBuf([u8; size_of::<BssDescriptor>() + 4]);

        let mut bytes = AlignedBuf([0u8; size_of::<BssDescriptor>() + 4]);
        let ies_len_off = offset_of!(BssDescriptor, ies_length);
        let ies_len_ptr = bytes.0.as_mut_ptr().wrapping_add(ies_len_off).cast::<u16>();
        unsafe {
            *ies_len_ptr = 4;
        }
        bytes.0[size_of::<BssDescriptor>()] = 0x01;
        bytes.0[size_of::<BssDescriptor>() + 1] = 0;
        bytes.0[size_of::<BssDescriptor>() + 2] = 0x05;
        bytes.0[size_of::<BssDescriptor>() + 3] = 1;

        let ptr = bytes.0.as_mut_ptr().cast::<BssDescriptor>();
        unsafe {
            assert_eq!(get_number_of_ies(ptr, bytes.0.len()), 0);
            assert_eq!(get_ie_length(ptr, bytes.0.len(), 0x05), -1);
        }
        assert_eq!(number_of_ies(&bytes.0), 0);
        assert_eq!(ie_length(&bytes.0, 0x05), Err(ErrorCode::new(-1)));
    }

    #[test]
    fn runtime_helpers_fail_closed_without_ios_backend() {
        let _guard = test_guard();
        let mut info = WdInfo::default();
        let mut params = ScanParameters::default();
        let mut out = [0xaa; 8];

        assert_eq!(ncd_lock_wireless_driver(), -1);
        assert_eq!(ncd_unlock_wireless_driver(1), u32::MAX);
        assert_eq!(init(DSCOMMUNICATIONS as u8), -1);
        unsafe {
            assert_eq!(unsafe { get_info(&mut info) }, -1);
            assert_eq!(scan(&mut params, out.as_mut_ptr(), out.len() as u16), -1);
            assert_eq!(
                scan_once(&mut params, out.as_mut_ptr(), out.len() as u16),
                -1
            );
        }
        assert_eq!(out, [0xaa; 8]);
    }

    #[test]
    fn result_helpers_wrap_pointer_apis() {
        let _guard = test_guard();
        let mut params = default_scan_parameters();
        let mut out = [0xaa; 4];

        assert_eq!(try_wd_init(DSCOMMUNICATIONS as u8), Err(ErrorCode::new(-1)));
        assert_eq!(scan_result(&mut params, &mut out), Err(ErrorCode::new(-1)));
        assert_eq!(
            scan_once_result(&mut params, &mut out),
            Err(ErrorCode::new(-1))
        );
        assert!(matches!(get_info_result(), Err(err) if err == ErrorCode::new(-1)));
        assert_eq!(out, [0xaa; 4]);
    }
}
