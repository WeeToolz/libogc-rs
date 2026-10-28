use core::ffi::{CStr, c_void};
use core::hint::spin_loop;
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, Ordering};

use super::*;

use ogc_ipc::{FileDescriptor, HeapId, IPC_OPEN_NONE, IoctlvEntry, open_file};

pub const STD_SIGNED_TIK_SIZE: usize =
    core::mem::size_of::<Tik>() + core::mem::size_of::<SigRsa2048>();
pub const MAX_TMD_SIZE: usize =
    core::mem::size_of::<Tmd>() + MAX_NUM_TMD_CONTENTS * core::mem::size_of::<TmdContent>();
pub const MAX_SIGNED_TMD_SIZE: usize = MAX_TMD_SIZE + core::mem::size_of::<SigRsa2048>();

const ES_PATH: &CStr = c"/dev/es";
const ES_HEAP_SIZE: i32 = 0x800;

#[path = "imp/filesystem.rs"]
mod filesystem;

const IOCTL_ES_ADDTICKET: i32 = 0x01;
const IOCTL_ES_ADDTITLESTART: i32 = 0x02;
const IOCTL_ES_ADDCONTENTSTART: i32 = 0x03;
const IOCTL_ES_ADDCONTENTDATA: i32 = 0x04;
const IOCTL_ES_ADDCONTENTFINISH: i32 = 0x05;
const IOCTL_ES_ADDTITLEFINISH: i32 = 0x06;
const IOCTL_ES_GETDEVICEID: i32 = 0x07;
const IOCTL_ES_LAUNCH: i32 = 0x08;
const IOCTL_ES_OPENCONTENT: i32 = 0x09;
const IOCTL_ES_READCONTENT: i32 = 0x0A;
const IOCTL_ES_CLOSECONTENT: i32 = 0x0B;
const IOCTL_ES_GETOWNEDTITLECNT: i32 = 0x0C;
const IOCTL_ES_GETOWNEDTITLES: i32 = 0x0D;
const IOCTL_ES_GETTITLECNT: i32 = 0x0E;
const IOCTL_ES_GETTITLES: i32 = 0x0F;
const IOCTL_ES_GETTITLECONTENTSCNT: i32 = 0x10;
const IOCTL_ES_GETTITLECONTENTS: i32 = 0x11;
const IOCTL_ES_GETVIEWCNT: i32 = 0x12;
const IOCTL_ES_GETVIEWS: i32 = 0x13;
const IOCTL_ES_GETTMDVIEWCNT: i32 = 0x14;
const IOCTL_ES_GETTMDVIEWS: i32 = 0x15;
const IOCTL_ES_GETCONSUMPTION: i32 = 0x16;
const IOCTL_ES_DELETETITLE: i32 = 0x17;
const IOCTL_ES_DELETETICKET: i32 = 0x18;
const IOCTL_ES_DIGETTMDVIEWSIZE: i32 = 0x19;
const IOCTL_ES_DIGETTMDVIEW: i32 = 0x1A;
const IOCTL_ES_DIGETTICKETVIEW: i32 = 0x1B;
const IOCTL_ES_DIVERIFY: i32 = 0x1C;
const IOCTL_ES_GETTITLEDIR: i32 = 0x1D;
const IOCTL_ES_GETDEVICECERT: i32 = 0x1E;
const IOCTL_ES_IMPORTBOOT: i32 = 0x1F;
const IOCTL_ES_GETTITLEID: i32 = 0x20;
const IOCTL_ES_SETUID: i32 = 0x21;
const IOCTL_ES_DELETETITLECONTENT: i32 = 0x22;
const IOCTL_ES_SEEKCONTENT: i32 = 0x23;
const IOCTL_ES_OPENTITLECONTENT: i32 = 0x24;
const IOCTL_ES_EXPORTTITLEINIT: i32 = 0x26;
const IOCTL_ES_EXPORTCONTENTBEGIN: i32 = 0x27;
const IOCTL_ES_EXPORTCONTENTDATA: i32 = 0x28;
const IOCTL_ES_EXPORTCONTENTEND: i32 = 0x29;
const IOCTL_ES_EXPORTTITLEDONE: i32 = 0x2A;
const IOCTL_ES_ADDTMD: i32 = 0x2B;
const IOCTL_ES_ENCRYPT: i32 = 0x2C;
const IOCTL_ES_DECRYPT: i32 = 0x2D;
const IOCTL_ES_GETBOOT2VERSION: i32 = 0x2E;
const IOCTL_ES_ADDTITLECANCEL: i32 = 0x2F;
const IOCTL_ES_SIGN: i32 = 0x30;
const IOCTL_ES_VERIFYSIGN: i32 = 0x31;
const IOCTL_ES_GETSTOREDCONTENTCNT: i32 = 0x32;
const IOCTL_ES_GETSTOREDCONTENTS: i32 = 0x33;
const IOCTL_ES_GETSTOREDTMDSIZE: i32 = 0x34;
const IOCTL_ES_GETSTOREDTMD: i32 = 0x35;
const IOCTL_ES_GETSHAREDCONTENTCNT: i32 = 0x36;
const IOCTL_ES_GETSHAREDCONTENTS: i32 = 0x37;
const IOCTL_ES_DIVERIFYWITHTIKVIEW: i32 = 0x3B;
const IOCTL_ES_CHECKHASKOREANKEY: i32 = 0x45;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ImportBootParams {
    pub tik: *const SignedBlob,
    pub tik_size: u32,
    pub tik_certs: *const SignedBlob,
    pub tik_certs_size: u32,
    pub tmd: *const SignedBlob,
    pub tmd_size: u32,
    pub tmd_certs: *const SignedBlob,
    pub tmd_certs_size: u32,
    pub content: *const u8,
    pub content_size: u32,
}

struct EsState {
    fd: Option<FileDescriptor>,
    heap: Option<HeapId>,
    filesystem_registered: bool,
}

impl EsState {
    const fn new() -> Self {
        Self {
            fd: None,
            heap: None,
            filesystem_registered: false,
        }
    }
}

struct SpinLock<T> {
    locked: AtomicBool,
    value: core::cell::UnsafeCell<T>,
}

unsafe impl<T: Send> Sync for SpinLock<T> {}

impl<T> SpinLock<T> {
    const fn new(value: T) -> Self {
        Self {
            locked: AtomicBool::new(false),
            value: core::cell::UnsafeCell::new(value),
        }
    }

    fn lock(&self) -> SpinGuard<'_, T> {
        while self.locked.swap(true, Ordering::Acquire) {
            spin_loop();
        }
        SpinGuard { lock: self }
    }
}

struct SpinGuard<'a, T> {
    lock: &'a SpinLock<T>,
}

impl<T> core::ops::Deref for SpinGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        unsafe { &*self.lock.value.get() }
    }
}

impl<T> core::ops::DerefMut for SpinGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.lock.value.get() }
    }
}

impl<T> Drop for SpinGuard<'_, T> {
    fn drop(&mut self) {
        self.lock.locked.store(false, Ordering::Release);
    }
}

struct HeapAlloc {
    heap: HeapId,
    ptr: NonNull<c_void>,
}

impl HeapAlloc {
    fn new(heap: HeapId, size: usize) -> Result<Self, i32> {
        let alloc_size = size.max(1) as i32;
        let ptr = heap.alloc(alloc_size).ok_or(ES_ENOMEM)?;
        Ok(Self { heap, ptr })
    }

    fn as_mut_ptr<T>(&self) -> *mut T {
        self.ptr.as_ptr().cast()
    }
}

impl Drop for HeapAlloc {
    fn drop(&mut self) {
        unsafe { self.heap.free(self.ptr) };
    }
}

static STATE: SpinLock<EsState> = SpinLock::new(EsState::new());

fn with_state<R>(f: impl FnOnce(&mut EsState) -> R) -> R {
    let mut guard = STATE.lock();
    f(&mut guard)
}

fn ensure_open(state: &mut EsState) -> Result<(), i32> {
    if state.heap.is_none() {
        state.heap = Some(HeapId::create(ES_HEAP_SIZE)?);
    }
    if state.fd.is_none() {
        state.fd = Some(open_file(ES_PATH, IPC_OPEN_NONE)?);
    }
    Ok(())
}

fn require_initialized(state: &EsState) -> Result<(), i32> {
    current_fd(state).map(|_| ())
}

fn current_fd(state: &EsState) -> Result<&FileDescriptor, i32> {
    state.fd.as_ref().ok_or(ES_ENOTINIT)
}

fn current_heap(state: &EsState) -> Result<HeapId, i32> {
    state.heap.ok_or(ES_ENOTINIT)
}

fn reopen(state: &mut EsState) -> Result<(), i32> {
    if let Some(fd) = state.fd.take() {
        let raw = fd.raw();
        if let Err(error) = fd.close() {
            state.fd = Some(FileDescriptor::new(raw));
            return Err(error);
        }
    }
    state.fd = Some(open_file(ES_PATH, IPC_OPEN_NONE)?);
    Ok(())
}

fn bytes_of<T>(value: &T) -> &[u8] {
    unsafe {
        core::slice::from_raw_parts((value as *const T).cast::<u8>(), core::mem::size_of::<T>())
    }
}

fn bytes_of_mut<T>(value: &mut T) -> &mut [u8] {
    unsafe {
        core::slice::from_raw_parts_mut((value as *mut T).cast::<u8>(), core::mem::size_of::<T>())
    }
}

fn is_aligned(ptr: *const c_void, align: usize) -> bool {
    ((ptr as usize) & (align - 1)) == 0
}

fn is_aligned_value(value: usize, align: usize) -> bool {
    (value & (align - 1)) == 0
}

fn is_valid_signature_type(signature_type: Sigtype) -> bool {
    matches!(
        signature_type,
        ES_SIG_RSA2048 | ES_SIG_RSA4096 | ES_SIG_ECDSA
    )
}

fn signature_size_inner(signature_type: Sigtype) -> usize {
    match signature_type {
        ES_SIG_RSA2048 => core::mem::size_of::<SigRsa2048>(),
        ES_SIG_RSA4096 => core::mem::size_of::<SigRsa4096>(),
        ES_SIG_ECDSA => core::mem::size_of::<SigEcdsa>(),
        _ => 0,
    }
}

fn certificate_size_inner(cert_type: u32) -> usize {
    match cert_type {
        ES_CERT_RSA2048 => core::mem::size_of::<CertRsa2048>(),
        ES_CERT_RSA4096 => core::mem::size_of::<CertRsa4096>(),
        ES_CERT_ECDSA => core::mem::size_of::<CertEcdsa>(),
        _ => 0,
    }
}

fn read_u32_unaligned(ptr: *const u32) -> u32 {
    unsafe { core::ptr::read_unaligned(ptr) }
}

fn read_u16_unaligned(ptr: *const u16) -> u16 {
    unsafe { core::ptr::read_unaligned(ptr) }
}

fn signed_blob_signature_type(blob: *const SignedBlob) -> Option<Sigtype> {
    if blob.is_null() {
        return None;
    }
    let signature_type = read_u32_unaligned(blob.cast::<u32>());
    if is_valid_signature_type(signature_type) {
        Some(signature_type)
    } else {
        None
    }
}

fn signature_payload(blob: *const SignedBlob) -> *const u8 {
    let signature_type = signed_blob_signature_type(blob).unwrap_or(0);
    unsafe { blob.cast::<u8>().add(signature_size_inner(signature_type)) }
}

fn signed_tmd_size_checked(blob: *const SignedBlob, available: u32) -> Option<u32> {
    if blob.is_null() || available < core::mem::size_of::<u32>() as u32 {
        return None;
    }
    let signature_type = signed_blob_signature_type(blob)?;
    let payload_offset = signature_size_inner(signature_type);
    let header_end = payload_offset.checked_add(core::mem::size_of::<Tmd>())?;
    if header_end > available as usize {
        return None;
    }
    let payload = signature_payload(blob).cast::<Tmd>();
    let num_contents = read_u16_unaligned(unsafe { core::ptr::addr_of!((*payload).num_contents) });
    let contents_size = usize::from(num_contents).checked_mul(core::mem::size_of::<TmdContent>())?;
    let total = header_end.checked_add(contents_size)?;
    u32::try_from(total).ok()
}

fn certificate_size_from_blob(blob: *const SignedBlob) -> usize {
    let signature_type = match signed_blob_signature_type(blob) {
        Some(value) => value,
        None => return 0,
    };
    let header =
        unsafe { blob.cast::<u8>().add(signature_size_inner(signature_type)) }.cast::<CertHeader>();
    let cert_type = read_u32_unaligned(unsafe { core::ptr::addr_of!((*header).cert_type) });
    signature_size_inner(signature_type) + certificate_size_inner(cert_type)
}

fn next_cert_inner(certs: *const SignedBlob) -> *const SignedBlob {
    let cert_size = certificate_size_from_blob(certs);
    if cert_size == 0 {
        core::ptr::null()
    } else {
        unsafe { certs.cast::<u8>().add(cert_size).cast::<SignedBlob>() }
    }
}

fn sanity_check_certlist(certs: *const SignedBlob, certsize: u32) -> bool {
    if certs.is_null() || certsize == 0 {
        return false;
    }

    let bytes = certs.cast::<u8>();
    let mut offset = 0usize;
    let total = certsize as usize;
    while offset < total {
        let remaining = total - offset;
        if remaining < core::mem::size_of::<u32>() {
            return false;
        }
        let cursor = unsafe { bytes.add(offset).cast::<SignedBlob>() };
        let Some(signature_type) = signed_blob_signature_type(cursor) else {
            return false;
        };
        let signature_size = signature_size_inner(signature_type);
        let Some(header_end) = signature_size.checked_add(core::mem::size_of::<CertHeader>())
        else {
            return false;
        };
        if header_end > remaining {
            return false;
        }
        let header = unsafe { bytes.add(offset + signature_size).cast::<CertHeader>() };
        let cert_type = read_u32_unaligned(unsafe { core::ptr::addr_of!((*header).cert_type) });
        let certificate_size = certificate_size_inner(cert_type);
        let Some(entry_size) = signature_size.checked_add(certificate_size) else {
            return false;
        };
        if certificate_size == 0 || entry_size > remaining {
            return false;
        }
        offset += entry_size;
    }
    offset == total
}

unsafe fn slice_from_const<'a>(ptr: *const u8, len: usize) -> &'a [u8] {
    unsafe { core::slice::from_raw_parts(ptr, len) }
}

unsafe fn slice_from_mut<'a>(ptr: *mut u8, len: usize) -> &'a mut [u8] {
    unsafe { core::slice::from_raw_parts_mut(ptr, len) }
}

fn ioctlv_scalar_out<T: Default + Copy>(fd: &FileDescriptor, ioctl: i32) -> Result<(T, i32), i32> {
    let mut out = T::default();
    let rc = fd.ioctlv(ioctl, 0, [IoctlvEntry::Out(bytes_of_mut(&mut out))]);
    if rc < 0 { Err(rc) } else { Ok((out, rc)) }
}

pub fn is_valid_signature(signature_type: Sigtype) -> bool {
    is_valid_signature_type(signature_type)
}

pub fn signature_size(signature_type: Sigtype) -> usize {
    signature_size_inner(signature_type)
}

pub fn is_valid_cert(cert_type: u32) -> bool {
    matches!(cert_type, ES_CERT_RSA2048 | ES_CERT_RSA4096 | ES_CERT_ECDSA)
}

pub fn certificate_size(cert_type: u32) -> usize {
    certificate_size_inner(cert_type)
}

pub fn tmd_size(num_contents: u16) -> usize {
    core::mem::size_of::<Tmd>() + usize::from(num_contents) * core::mem::size_of::<TmdContent>()
}

pub fn signed_tmd_size(signature_type: Sigtype, num_contents: u16) -> usize {
    tmd_size(num_contents) + signature_size_inner(signature_type)
}

pub fn signed_tik_size(signature_type: Sigtype) -> usize {
    core::mem::size_of::<Tik>() + signature_size_inner(signature_type)
}

pub fn signed_cert_size(signature_type: Sigtype, cert_type: u32) -> usize {
    certificate_size_inner(cert_type) + signature_size_inner(signature_type)
}

pub fn init() -> i32 {
    with_state(|state| {
        if let Err(rc) = ensure_open(state) {
            return rc;
        }
        if state.filesystem_registered {
            return 0;
        }
        let rc = filesystem::init_fs();
        if rc < 0 {
            return rc;
        }
        state.filesystem_registered = true;
        0
    })
}

pub fn close() -> i32 {
    with_state(|state| {
        if state.filesystem_registered {
            let rc = filesystem::deinit_fs();
            if rc < 0 {
                return rc;
            }
            state.filesystem_registered = false;
        }
        if let Some(fd) = state.fd.take() {
            return match fd.close() {
                Ok(()) => 0,
                Err(rc) => rc,
            };
        }
        0
    })
}

pub fn reset() -> i32 {
    with_state(|state| {
        state.fd = None;
        0
    })
}

/// # Safety
///
/// Output pointers and buffers must satisfy libogc's documented alignment and
/// size requirements for each ES request.
pub unsafe fn get_title_id(title_id: *mut u64) -> i32 {
    with_state(|state| {
        if let Err(rc) = require_initialized(state) {
            return rc;
        }
        if title_id.is_null() {
            return ES_EINVAL;
        }
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };
        match ioctlv_scalar_out::<u64>(fd, IOCTL_ES_GETTITLEID) {
            Ok((value, rc)) => {
                unsafe { title_id.write(value) };
                rc
            }
            Err(rc) => rc,
        }
    })
}

pub fn set_uid(uid: u64) -> i32 {
    with_state(|state| {
        if let Err(rc) = require_initialized(state) {
            return rc;
        }
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };

        fd.ioctlv(IOCTL_ES_SETUID, 1, [IoctlvEntry::In(bytes_of(&uid))])
    })
}

/// # Safety
///
/// `filepath` must point to a writable 30-byte title-directory buffer.
pub unsafe fn get_data_dir(title_id: u64, filepath: *mut u8) -> i32 {
    unsafe {
        with_state(|state| {
            if filepath.is_null() {
                return ES_EINVAL;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let out = slice_from_mut(filepath, 30);

            fd.ioctlv(
                IOCTL_ES_GETTITLEDIR,
                1,
                [IoctlvEntry::In(bytes_of(&title_id)), IoctlvEntry::Out(out)],
            )
        })
    }
}

/// # Safety
///
/// `cnt` must be valid for writes.
pub unsafe fn get_num_ticket_views(title_id: u64, cnt: *mut u32) -> i32 {
    with_state(|state| {
        if cnt.is_null() {
            return ES_EINVAL;
        }
        if let Err(rc) = require_initialized(state) {
            return rc;
        }
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };
        let mut out = 0u32;
        let rc = fd.ioctlv(
            IOCTL_ES_GETVIEWCNT,
            1,
            [
                IoctlvEntry::In(bytes_of(&title_id)),
                IoctlvEntry::Out(bytes_of_mut(&mut out)),
            ],
        );
        if rc < 0 {
            rc
        } else {
            unsafe { cnt.write(out) };
            rc
        }
    })
}

/// # Safety
///
/// `views` must be 4-byte aligned and valid for `cnt` entries.
pub unsafe fn get_ticket_views(title_id: u64, views: *mut Tikview, cnt: u32) -> i32 {
    unsafe {
        with_state(|state| {
            if cnt == 0 || views.is_null() {
                return ES_EINVAL;
            }
            if !is_aligned(views.cast(), 4) {
                return ES_EALIGN;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let out = slice_from_mut(
                views.cast::<u8>(),
                core::mem::size_of::<Tikview>() * cnt as usize,
            );

            fd.ioctlv(
                IOCTL_ES_GETVIEWS,
                2,
                [
                    IoctlvEntry::In(bytes_of(&title_id)),
                    IoctlvEntry::In(bytes_of(&cnt)),
                    IoctlvEntry::Out(out),
                ],
            )
        })
    }
}

/// # Safety
///
/// `cnt` must be valid for writes.
pub unsafe fn get_num_owned_titles(cnt: *mut u32) -> i32 {
    with_state(|state| {
        if cnt.is_null() {
            return ES_EINVAL;
        }
        if let Err(rc) = require_initialized(state) {
            return rc;
        }
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };
        match ioctlv_scalar_out::<u32>(fd, IOCTL_ES_GETOWNEDTITLECNT) {
            Ok((value, rc)) => {
                unsafe { cnt.write(value) };
                rc
            }
            Err(rc) => rc,
        }
    })
}

/// # Safety
///
/// `titles` must be 4-byte aligned and valid for `cnt` entries.
pub unsafe fn get_owned_titles(titles: *mut u64, cnt: u32) -> i32 {
    unsafe {
        with_state(|state| {
            if cnt == 0 || titles.is_null() {
                return ES_EINVAL;
            }
            if !is_aligned(titles.cast(), 4) {
                return ES_EALIGN;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let out = slice_from_mut(
                titles.cast::<u8>(),
                core::mem::size_of::<u64>() * cnt as usize,
            );

            fd.ioctlv(
                IOCTL_ES_GETOWNEDTITLES,
                1,
                [IoctlvEntry::In(bytes_of(&cnt)), IoctlvEntry::Out(out)],
            )
        })
    }
}

/// # Safety
///
/// `cnt` must be valid for writes.
pub unsafe fn get_num_titles(cnt: *mut u32) -> i32 {
    with_state(|state| {
        if cnt.is_null() {
            return ES_EINVAL;
        }
        if let Err(rc) = require_initialized(state) {
            return rc;
        }
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };
        match ioctlv_scalar_out::<u32>(fd, IOCTL_ES_GETTITLECNT) {
            Ok((value, rc)) => {
                unsafe { cnt.write(value) };
                rc
            }
            Err(rc) => rc,
        }
    })
}

/// # Safety
///
/// `titles` must be 4-byte aligned and valid for `cnt` entries.
pub unsafe fn get_titles(titles: *mut u64, cnt: u32) -> i32 {
    unsafe {
        with_state(|state| {
            if cnt == 0 || titles.is_null() {
                return ES_EINVAL;
            }
            if !is_aligned(titles.cast(), 4) {
                return ES_EALIGN;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let out = slice_from_mut(
                titles.cast::<u8>(),
                core::mem::size_of::<u64>() * cnt as usize,
            );

            fd.ioctlv(
                IOCTL_ES_GETTITLES,
                1,
                [IoctlvEntry::In(bytes_of(&cnt)), IoctlvEntry::Out(out)],
            )
        })
    }
}

/// # Safety
///
/// `stmd` and `cnt` must satisfy libogc's alignment and size requirements.
pub unsafe fn get_num_stored_tmd_contents(
    stmd: *const SignedBlob,
    tmd_size: u32,
    cnt: *mut u32,
) -> i32 {
    unsafe {
        with_state(|state| {
            if cnt.is_null() {
                return ES_EINVAL;
            }
            if stmd.is_null() || !is_valid_signature_type(read_u32_unaligned(stmd.cast::<u32>())) {
                return ES_EINVAL;
            }
            if !is_aligned(stmd.cast(), 64) {
                return ES_EALIGN;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let mut out = 0u32;
            let stmd_bytes = slice_from_const(stmd.cast::<u8>(), tmd_size as usize);
            let rc = fd.ioctlv(
                IOCTL_ES_GETSTOREDCONTENTCNT,
                1,
                [
                    IoctlvEntry::In(stmd_bytes),
                    IoctlvEntry::Out(bytes_of_mut(&mut out)),
                ],
            );
            if rc < 0 {
                rc
            } else {
                cnt.write(out);
                rc
            }
        })
    }
}

/// # Safety
///
/// Pointers must satisfy libogc's alignment and size requirements.
pub unsafe fn get_stored_tmd_contents(
    stmd: *const SignedBlob,
    tmd_size: u32,
    contents: *mut u32,
    cnt: u32,
) -> i32 {
    unsafe {
        with_state(|state| {
            if cnt == 0 || contents.is_null() {
                return ES_EINVAL;
            }
            if stmd.is_null() || !is_valid_signature_type(read_u32_unaligned(stmd.cast::<u32>())) {
                return ES_EINVAL;
            }
            if !is_aligned(stmd.cast(), 64) || !is_aligned(contents.cast(), 4) {
                return ES_EALIGN;
            }
            if tmd_size as usize > MAX_SIGNED_TMD_SIZE {
                return ES_EINVAL;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let stmd_bytes = slice_from_const(stmd.cast::<u8>(), tmd_size as usize);
            let out = slice_from_mut(
                contents.cast::<u8>(),
                core::mem::size_of::<u32>() * cnt as usize,
            );

            fd.ioctlv(
                IOCTL_ES_GETSTOREDCONTENTS,
                2,
                [
                    IoctlvEntry::In(stmd_bytes),
                    IoctlvEntry::In(bytes_of(&cnt)),
                    IoctlvEntry::Out(out),
                ],
            )
        })
    }
}

/// # Safety
///
/// `size` must be valid for writes.
pub unsafe fn get_stored_tmd_size(title_id: u64, size: *mut u32) -> i32 {
    with_state(|state| {
        if size.is_null() {
            return ES_EINVAL;
        }
        if let Err(rc) = require_initialized(state) {
            return rc;
        }
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };
        let mut out = 0u32;
        let rc = fd.ioctlv(
            IOCTL_ES_GETSTOREDTMDSIZE,
            1,
            [
                IoctlvEntry::In(bytes_of(&title_id)),
                IoctlvEntry::Out(bytes_of_mut(&mut out)),
            ],
        );
        if rc < 0 {
            rc
        } else {
            unsafe { size.write(out) };
            rc
        }
    })
}

/// # Safety
///
/// `stmd` must satisfy libogc's alignment and output-size requirements.
pub unsafe fn get_stored_tmd(title_id: u64, stmd: *mut SignedBlob, size: u32) -> i32 {
    unsafe {
        with_state(|state| {
            if size == 0 || stmd.is_null() {
                return ES_EINVAL;
            }
            if !is_aligned(stmd.cast(), 4) {
                return ES_EALIGN;
            }
            if size as usize > MAX_SIGNED_TMD_SIZE {
                return ES_EINVAL;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let out = slice_from_mut(stmd.cast::<u8>(), size as usize);

            fd.ioctlv(
                IOCTL_ES_GETSTOREDTMD,
                2,
                [
                    IoctlvEntry::In(bytes_of(&title_id)),
                    IoctlvEntry::In(bytes_of(&size)),
                    IoctlvEntry::Out(out),
                ],
            )
        })
    }
}

/// # Safety
///
/// `num` must be valid for writes.
pub unsafe fn get_title_contents_count(title_id: u64, num: *mut u32) -> i32 {
    with_state(|state| {
        if num.is_null() {
            return ES_EINVAL;
        }
        if let Err(rc) = require_initialized(state) {
            return rc;
        }
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };
        let mut out = 0u32;
        let rc = fd.ioctlv(
            IOCTL_ES_GETTITLECONTENTSCNT,
            1,
            [
                IoctlvEntry::In(bytes_of(&title_id)),
                IoctlvEntry::Out(bytes_of_mut(&mut out)),
            ],
        );
        if rc < 0 {
            rc
        } else {
            unsafe { num.write(out) };
            rc
        }
    })
}

/// # Safety
///
/// `contents` must satisfy libogc's alignment and output-size requirements.
pub unsafe fn get_title_contents(title_id: u64, contents: *mut u32, num: u32) -> i32 {
    unsafe {
        with_state(|state| {
            if num == 0 || contents.is_null() {
                return ES_EINVAL;
            }
            if !is_aligned(contents.cast(), 4) {
                return ES_EALIGN;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let out = slice_from_mut(
                contents.cast::<u8>(),
                core::mem::size_of::<u32>() * num as usize,
            );

            fd.ioctlv(
                IOCTL_ES_GETTITLECONTENTS,
                2,
                [
                    IoctlvEntry::In(bytes_of(&title_id)),
                    IoctlvEntry::In(bytes_of(&num)),
                    IoctlvEntry::Out(out),
                ],
            )
        })
    }
}

/// # Safety
///
/// `size` must be valid for writes.
pub unsafe fn get_tmd_view_size(title_id: u64, size: *mut u32) -> i32 {
    with_state(|state| {
        if size.is_null() {
            return ES_EINVAL;
        }
        if let Err(rc) = require_initialized(state) {
            return rc;
        }
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };
        let mut out = 0u32;
        let rc = fd.ioctlv(
            IOCTL_ES_GETTMDVIEWCNT,
            1,
            [
                IoctlvEntry::In(bytes_of(&title_id)),
                IoctlvEntry::Out(bytes_of_mut(&mut out)),
            ],
        );
        if rc < 0 {
            rc
        } else {
            unsafe { size.write(out) };
            rc
        }
    })
}

/// # Safety
///
/// `view` must satisfy libogc's alignment and output-size requirements.
pub unsafe fn get_tmd_view(title_id: u64, view: *mut TmdView, size: u32) -> i32 {
    unsafe {
        with_state(|state| {
            if size == 0 || view.is_null() {
                return ES_EINVAL;
            }
            if !is_aligned(view.cast(), 4) {
                return ES_EALIGN;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let out = slice_from_mut(view.cast::<u8>(), size as usize);

            fd.ioctlv(
                IOCTL_ES_GETTMDVIEWS,
                2,
                [
                    IoctlvEntry::In(bytes_of(&title_id)),
                    IoctlvEntry::In(bytes_of(&size)),
                    IoctlvEntry::Out(out),
                ],
            )
        })
    }
}

/// # Safety
///
/// `n_limits` must be valid for writes.
pub unsafe fn get_consumption_count(ticket_id: u64, n_limits: *mut u32) -> i32 {
    with_state(|state| {
        if n_limits.is_null() {
            return ES_EINVAL;
        }
        if let Err(rc) = require_initialized(state) {
            return rc;
        }
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };
        let mut out = 0u32;
        let empty: &mut [u8] = &mut [];
        let rc = fd.ioctlv(
            IOCTL_ES_GETCONSUMPTION,
            1,
            [
                IoctlvEntry::In(bytes_of(&ticket_id)),
                IoctlvEntry::Out(empty),
                IoctlvEntry::Out(bytes_of_mut(&mut out)),
            ],
        );
        if rc < 0 {
            rc
        } else {
            unsafe { n_limits.write(out) };
            rc
        }
    })
}

/// # Safety
///
/// `limits` must satisfy libogc's alignment and output-size requirements.
pub unsafe fn get_consumption(ticket_id: u64, limits: *mut Tiklimit, n_limits: u32) -> i32 {
    unsafe {
        with_state(|state| {
            if n_limits == 0 || limits.is_null() {
                return ES_EINVAL;
            }
            if !is_aligned(limits.cast(), 4) {
                return ES_EALIGN;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let out = slice_from_mut(
                limits.cast::<u8>(),
                core::mem::size_of::<Tiklimit>() * n_limits as usize,
            );
            let mut count = n_limits;

            fd.ioctlv(
                IOCTL_ES_GETCONSUMPTION,
                1,
                [
                    IoctlvEntry::In(bytes_of(&ticket_id)),
                    IoctlvEntry::Out(out),
                    IoctlvEntry::Out(bytes_of_mut(&mut count)),
                ],
            )
        })
    }
}

/// # Safety
///
/// Pointers must satisfy libogc's alignment and size requirements.
pub unsafe fn di_get_tmd_view_size(
    s_tmd: *const SignedBlob,
    tmd_size: u32,
    view_size: *mut u32,
) -> i32 {
    unsafe {
        with_state(|state| {
            if view_size.is_null() {
                return ES_EINVAL;
            }
            if !is_aligned(s_tmd.cast(), 4) {
                return ES_EALIGN;
            }
            if !s_tmd.is_null() && signed_tmd_size_checked(s_tmd, tmd_size) != Some(tmd_size) {
                return ES_EINVAL;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let stmd_bytes = if s_tmd.is_null() {
                &[][..]
            } else {
                slice_from_const(s_tmd.cast::<u8>(), tmd_size as usize)
            };
            let mut out = 0u32;
            let rc = fd.ioctlv(
                IOCTL_ES_DIGETTMDVIEWSIZE,
                1,
                [
                    IoctlvEntry::In(stmd_bytes),
                    IoctlvEntry::Out(bytes_of_mut(&mut out)),
                ],
            );
            if rc < 0 {
                rc
            } else {
                view_size.write(out);
                rc
            }
        })
    }
}

/// # Safety
///
/// Pointers must satisfy libogc's alignment and size requirements.
pub unsafe fn di_get_tmd_view(
    s_tmd: *const SignedBlob,
    tmd_size: u32,
    view: *mut TmdView,
    view_size: u32,
) -> i32 {
    unsafe {
        with_state(|state| {
            if view_size < core::mem::size_of::<TmdView>() as u32 {
                return ES_EINVAL;
            }
            if !is_aligned(s_tmd.cast(), 4) || !is_aligned(view.cast(), 4) {
                return ES_EALIGN;
            }
            if !s_tmd.is_null() && signed_tmd_size_checked(s_tmd, tmd_size) != Some(tmd_size) {
                return ES_EINVAL;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let stmd_bytes = if s_tmd.is_null() {
                &[][..]
            } else {
                slice_from_const(s_tmd.cast::<u8>(), tmd_size as usize)
            };
            let out = slice_from_mut(view.cast::<u8>(), view_size as usize);
            let size_in = if s_tmd.is_null() { 0 } else { tmd_size };

            fd.ioctlv(
                IOCTL_ES_DIGETTMDVIEW,
                2,
                [
                    IoctlvEntry::In(stmd_bytes),
                    IoctlvEntry::In(bytes_of(&size_in)),
                    IoctlvEntry::Out(out),
                ],
            )
        })
    }
}

/// # Safety
///
/// `s_tik` and `view` must satisfy libogc's alignment requirements.
pub unsafe fn get_ticket_view(s_tik: *const SignedBlob, view: *mut Tikview) -> i32 {
    unsafe {
        with_state(|state| {
            if view.is_null() {
                return ES_EINVAL;
            }
            if !is_aligned(s_tik.cast(), 4) || !is_aligned(view.cast(), 4) {
                return ES_EALIGN;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let in_len = if s_tik.is_null() {
                0
            } else {
                STD_SIGNED_TIK_SIZE
            };
            let tik_bytes = if s_tik.is_null() {
                &[][..]
            } else {
                slice_from_const(s_tik.cast::<u8>(), in_len)
            };
            let out = slice_from_mut(view.cast::<u8>(), core::mem::size_of::<Tikview>());

            fd.ioctlv(
                IOCTL_ES_DIGETTICKETVIEW,
                1,
                [IoctlvEntry::In(tik_bytes), IoctlvEntry::Out(out)],
            )
        })
    }
}

/// # Safety
///
/// `cnt` must be valid for writes.
pub unsafe fn get_num_shared_contents(cnt: *mut u32) -> i32 {
    with_state(|state| {
        if cnt.is_null() {
            return ES_EINVAL;
        }
        if let Err(rc) = require_initialized(state) {
            return rc;
        }
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };
        match ioctlv_scalar_out::<u32>(fd, IOCTL_ES_GETSHAREDCONTENTCNT) {
            Ok((value, rc)) => {
                unsafe { cnt.write(value) };
                rc
            }
            Err(rc) => rc,
        }
    })
}

/// # Safety
///
/// `contents` must satisfy libogc's alignment and output-size requirements.
pub unsafe fn get_shared_contents(contents: *mut Sha1, cnt: u32) -> i32 {
    unsafe {
        with_state(|state| {
            if cnt == 0 || contents.is_null() {
                return ES_EINVAL;
            }
            if !is_aligned(contents.cast(), 4) {
                return ES_EALIGN;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let out = slice_from_mut(
                contents.cast::<u8>(),
                core::mem::size_of::<Sha1>() * cnt as usize,
            );

            fd.ioctlv(
                IOCTL_ES_GETSHAREDCONTENTS,
                1,
                [IoctlvEntry::In(bytes_of(&cnt)), IoctlvEntry::Out(out)],
            )
        })
    }
}

/// # Safety
///
/// `view` must satisfy libogc's alignment requirements.
pub unsafe fn launch_title(title_id: u64, view: *const Tikview) -> i32 {
    unsafe {
        with_state(|state| {
            if view.is_null() {
                return ES_EINVAL;
            }
            if !is_aligned(view.cast(), 4) {
                return ES_EALIGN;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let view_bytes = slice_from_const(view.cast::<u8>(), core::mem::size_of::<Tikview>());
            fd.ioctlv_reboot(
                IOCTL_ES_LAUNCH,
                2,
                [
                    IoctlvEntry::In(bytes_of(&title_id)),
                    IoctlvEntry::In(view_bytes),
                ],
            )
        })
    }
}

/// # Safety
///
/// `view` must satisfy libogc's alignment requirements.
pub unsafe fn launch_title_background(title_id: u64, view: *const Tikview) -> i32 {
    unsafe {
        with_state(|state| {
            if view.is_null() {
                return ES_EINVAL;
            }
            if !is_aligned(view.cast(), 4) {
                return ES_EALIGN;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let view_bytes = slice_from_const(view.cast::<u8>(), core::mem::size_of::<Tikview>());
            {
                fd.ioctlv_reboot_background(
                    IOCTL_ES_LAUNCH,
                    2,
                    [
                        IoctlvEntry::In(bytes_of(&title_id)),
                        IoctlvEntry::In(view_bytes),
                    ],
                )
            }
        })
    }
}

/// # Safety
///
/// All pointers must satisfy libogc's alignment, size, and ownership rules for
/// the chosen ES request.
pub unsafe fn identify(
    certificates: *const SignedBlob,
    certificates_size: u32,
    tmd: *const SignedBlob,
    tmd_size: u32,
    ticket: *const SignedBlob,
    ticket_size: u32,
    keyid: *mut u32,
) -> i32 {
    unsafe {
        with_state(|state| {
            if ticket_size as usize != STD_SIGNED_TIK_SIZE {
                return ES_EINVAL;
            }
            if tmd.is_null()
                || tmd_size == 0
                || !is_valid_signature_type(read_u32_unaligned(tmd.cast::<u32>()))
            {
                return ES_EINVAL;
            }
            if ticket.is_null()
                || !is_valid_signature_type(read_u32_unaligned(ticket.cast::<u32>()))
            {
                return ES_EINVAL;
            }
            if !sanity_check_certlist(certificates, certificates_size) {
                return ES_EINVAL;
            }
            if !is_aligned(certificates.cast(), 4)
                || !is_aligned(tmd.cast(), 4)
                || !is_aligned(ticket.cast(), 4)
            {
                return ES_EALIGN;
            }
            if tmd_size as usize > MAX_SIGNED_TMD_SIZE {
                return ES_EINVAL;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let heap = match current_heap(state) {
                Ok(heap) => heap,
                Err(rc) => return rc,
            };
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let payload = signature_payload(tmd).cast::<Tmd>();
            let num_contents =
                read_u16_unaligned({ core::ptr::addr_of!((*payload).num_contents) }) as usize;
            let keyid_buf = match HeapAlloc::new(heap, core::mem::size_of::<u32>()) {
                Ok(value) => value,
                Err(rc) => return rc,
            };
            let hashes = match HeapAlloc::new(heap, num_contents * core::mem::size_of::<Sha1>()) {
                Ok(value) => value,
                Err(rc) => return rc,
            };
            let cert_bytes =
                slice_from_const(certificates.cast::<u8>(), certificates_size as usize);
            let ticket_bytes = slice_from_const(ticket.cast::<u8>(), ticket_size as usize);
            let tmd_bytes = slice_from_const(tmd.cast::<u8>(), tmd_size as usize);
            let keyid_out =
                slice_from_mut(keyid_buf.as_mut_ptr::<u8>(), core::mem::size_of::<u32>());
            let hashes_out = {
                slice_from_mut(
                    hashes.as_mut_ptr::<u8>(),
                    num_contents * core::mem::size_of::<Sha1>(),
                )
            };
            let empty: &mut [u8] = &mut [];
            let rc = fd.ioctlv(
                IOCTL_ES_DIVERIFY,
                4,
                [
                    IoctlvEntry::In(cert_bytes),
                    IoctlvEntry::In(empty),
                    IoctlvEntry::In(ticket_bytes),
                    IoctlvEntry::In(tmd_bytes),
                    IoctlvEntry::Out(keyid_out),
                    IoctlvEntry::Out(hashes_out),
                ],
            );
            if rc >= 0 {
                if !keyid.is_null() {
                    keyid.write(keyid_buf.as_mut_ptr::<u32>().read());
                }
                match reopen(state) {
                    Ok(()) => 0,
                    Err(error) => error,
                }
            } else {
                rc
            }
        })
    }
}

/// # Safety
///
/// All pointers must satisfy libogc's alignment, size, and ownership rules for
/// the chosen ES request.
pub unsafe fn verify_with_ticket_view(
    certificates: *const SignedBlob,
    certificates_size: u32,
    s_tmd: *const SignedBlob,
    tmd_size: u32,
    ticket_view: *const Tikview,
    keynum: *mut u32,
) -> i32 {
    unsafe {
        with_state(|state| {
            if s_tmd.is_null() || signed_tmd_size_checked(s_tmd, tmd_size) != Some(tmd_size) {
                return ES_EINVAL;
            }
            if certificates.is_null()
                || (certificates_size != 0
                    && !sanity_check_certlist(certificates, certificates_size))
                || !is_aligned_value(certificates_size as usize, 64)
            {
                return ES_EINVAL;
            }
            if !is_aligned(certificates.cast(), 4)
                || !is_aligned(s_tmd.cast(), 4)
                || !is_aligned(ticket_view.cast(), 4)
                || !keynum.is_null() && !is_aligned(keynum.cast(), 4)
            {
                return ES_EALIGN;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let heap = match current_heap(state) {
                Ok(heap) => heap,
                Err(rc) => return rc,
            };
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let payload = signature_payload(s_tmd).cast::<Tmd>();
            let num_contents =
                read_u16_unaligned({ core::ptr::addr_of!((*payload).num_contents) }) as usize;
            let hashes = match HeapAlloc::new(heap, num_contents * core::mem::size_of::<Sha1>()) {
                Ok(value) => value,
                Err(rc) => return rc,
            };
            let mut spare = 0u32;
            let key_ptr = if keynum.is_null() {
                &mut spare as *mut u32
            } else {
                keynum
            };
            let cert_bytes =
                slice_from_const(certificates.cast::<u8>(), certificates_size as usize);
            let empty: &mut [u8] = &mut [];
            let view_bytes =
                slice_from_const(ticket_view.cast::<u8>(), core::mem::size_of::<Tikview>());
            let tmd_bytes = slice_from_const(s_tmd.cast::<u8>(), tmd_size as usize);
            let key_out = slice_from_mut(key_ptr.cast::<u8>(), core::mem::size_of::<u32>());
            let hashes_out = slice_from_mut(
                hashes.as_mut_ptr::<u8>(),
                num_contents * core::mem::size_of::<Sha1>(),
            );
            let rc = fd.ioctlv(
                IOCTL_ES_DIVERIFYWITHTIKVIEW,
                4,
                [
                    IoctlvEntry::In(cert_bytes),
                    IoctlvEntry::In(empty),
                    IoctlvEntry::In(view_bytes),
                    IoctlvEntry::In(tmd_bytes),
                    IoctlvEntry::Out(key_out),
                    IoctlvEntry::Out(hashes_out),
                ],
            );
            if rc >= 0 {
                match reopen(state) {
                    Ok(()) => 0,
                    Err(error) => error,
                }
            } else {
                rc
            }
        })
    }
}

/// # Safety
///
/// All pointers must satisfy libogc's alignment, size, and ownership rules for
/// the chosen ES request.
pub unsafe fn di_verify_with_ticket_view(
    certificates: *const SignedBlob,
    certificates_size: u32,
    s_tmd: *const SignedBlob,
    tmd_size: u32,
    ticket_view: *const Tikview,
    keynum: *mut u32,
) -> i32 {
    unsafe {
        verify_with_ticket_view(
            certificates,
            certificates_size,
            s_tmd,
            tmd_size,
            ticket_view,
            keynum,
        )
    }
}

/// # Safety
///
/// All pointers must satisfy libogc's alignment, size, and ownership rules.
pub unsafe fn add_ticket(
    tik: *const SignedBlob,
    tik_size: u32,
    certificates: *const SignedBlob,
    certificates_size: u32,
    crl: *const SignedBlob,
    crl_size: u32,
) -> i32 {
    unsafe {
        with_state(|state| {
            if tik_size as usize != STD_SIGNED_TIK_SIZE {
                return ES_EINVAL;
            }
            if certificates.is_null()
                || tik.is_null()
                || !is_valid_signature_type(read_u32_unaligned(tik.cast::<u32>()))
            {
                return ES_EINVAL;
            }
            if crl_size != 0
                && (crl.is_null()
                    || !is_valid_signature_type(read_u32_unaligned(crl.cast::<u32>())))
            {
                return ES_EINVAL;
            }
            if !sanity_check_certlist(certificates, certificates_size) {
                return ES_EINVAL;
            }
            if !is_aligned(certificates.cast(), 4)
                || !is_aligned(tik.cast(), 4)
                || !is_aligned(crl.cast(), 4)
            {
                return ES_EALIGN;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let tik_bytes = slice_from_const(tik.cast::<u8>(), tik_size as usize);
            let cert_bytes =
                slice_from_const(certificates.cast::<u8>(), certificates_size as usize);
            let crl_bytes = if crl_size == 0 {
                &[][..]
            } else {
                slice_from_const(crl.cast::<u8>(), crl_size as usize)
            };

            fd.ioctlv(
                IOCTL_ES_ADDTICKET,
                3,
                [
                    IoctlvEntry::In(tik_bytes),
                    IoctlvEntry::In(cert_bytes),
                    IoctlvEntry::In(crl_bytes),
                ],
            )
        })
    }
}

/// # Safety
///
/// `view` must satisfy libogc's alignment requirements.
pub unsafe fn delete_ticket(view: *const Tikview) -> i32 {
    unsafe {
        with_state(|state| {
            if view.is_null() {
                return ES_EINVAL;
            }
            if !is_aligned(view.cast(), 4) {
                return ES_EALIGN;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let view_bytes = slice_from_const(view.cast::<u8>(), core::mem::size_of::<Tikview>());

            fd.ioctlv(IOCTL_ES_DELETETICKET, 1, [IoctlvEntry::In(view_bytes)])
        })
    }
}

/// # Safety
///
/// Output buffers must satisfy libogc's alignment and size requirements.
pub unsafe fn export_title_init(title_id: u64, tmd_out: *mut SignedBlob, tmd_size: u32) -> i32 {
    unsafe {
        with_state(|state| {
            if tmd_out.is_null() || tmd_size == 0 || tmd_size as usize > MAX_SIGNED_TMD_SIZE {
                return ES_EINVAL;
            }
            if !is_aligned(tmd_out.cast(), 4) {
                return ES_EALIGN;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let out = slice_from_mut(tmd_out.cast::<u8>(), tmd_size as usize);

            fd.ioctlv(
                IOCTL_ES_EXPORTTITLEINIT,
                1,
                [IoctlvEntry::In(bytes_of(&title_id)), IoctlvEntry::Out(out)],
            )
        })
    }
}

pub fn export_content_begin(title_id: u64, cid: u32) -> i32 {
    with_state(|state| {
        if let Err(rc) = require_initialized(state) {
            return rc;
        }
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };

        fd.ioctlv(
            IOCTL_ES_EXPORTCONTENTBEGIN,
            2,
            [
                IoctlvEntry::In(bytes_of(&title_id)),
                IoctlvEntry::In(bytes_of(&cid)),
            ],
        )
    })
}

/// # Safety
///
/// `data` must satisfy libogc's alignment and size requirements.
pub unsafe fn export_content_data(cfd: i32, data: *mut u8, data_size: u32) -> i32 {
    unsafe {
        with_state(|state| {
            if data.is_null() || data_size == 0 {
                return ES_EINVAL;
            }
            if !is_aligned(data.cast(), 32) || !is_aligned_value(data_size as usize, 32) {
                return ES_EALIGN;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let out = slice_from_mut(data, data_size as usize);

            fd.ioctlv(
                IOCTL_ES_EXPORTCONTENTDATA,
                1,
                [IoctlvEntry::In(bytes_of(&cfd)), IoctlvEntry::Out(out)],
            )
        })
    }
}

pub fn export_content_end(cfd: i32) -> i32 {
    with_state(|state| {
        if let Err(rc) = require_initialized(state) {
            return rc;
        }
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };

        fd.ioctlv(
            IOCTL_ES_EXPORTCONTENTEND,
            1,
            [IoctlvEntry::In(bytes_of(&cfd))],
        )
    })
}

pub fn export_title_done() -> i32 {
    with_state(|state| {
        if let Err(rc) = require_initialized(state) {
            return rc;
        }
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };

        fd.ioctlv::<0>(IOCTL_ES_EXPORTTITLEDONE, 0, [])
    })
}

/// # Safety
///
/// `tmd` must satisfy libogc's alignment and size requirements.
pub unsafe fn reimport_title_init(tmd: *const SignedBlob, tmd_size: u32) -> i32 {
    unsafe {
        with_state(|state| {
            if tmd.is_null() || !is_valid_signature_type(read_u32_unaligned(tmd.cast::<u32>())) {
                return ES_EINVAL;
            }
            if signed_tmd_size_checked(tmd, tmd_size) != Some(tmd_size) {
                return ES_EINVAL;
            }
            if !is_aligned(tmd.cast(), 4) {
                return ES_EALIGN;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let tmd_bytes = slice_from_const(tmd.cast::<u8>(), tmd_size as usize);

            fd.ioctlv(IOCTL_ES_ADDTMD, 1, [IoctlvEntry::In(tmd_bytes)])
        })
    }
}

/// # Safety
///
/// All pointers must satisfy libogc's alignment, size, and ownership rules.
pub unsafe fn add_title_start(
    tmd: *const SignedBlob,
    tmd_size: u32,
    certificates: *const SignedBlob,
    certificates_size: u32,
    crl: *const SignedBlob,
    crl_size: u32,
) -> i32 {
    unsafe {
        with_state(|state| {
            if certificates.is_null()
                || tmd.is_null()
                || !is_valid_signature_type(read_u32_unaligned(tmd.cast::<u32>()))
            {
                return ES_EINVAL;
            }
            if signed_tmd_size_checked(tmd, tmd_size) != Some(tmd_size) {
                return ES_EINVAL;
            }
            if crl_size != 0
                && (crl.is_null()
                    || !is_valid_signature_type(read_u32_unaligned(crl.cast::<u32>())))
            {
                return ES_EINVAL;
            }
            if !sanity_check_certlist(certificates, certificates_size) {
                return ES_EINVAL;
            }
            if !is_aligned(certificates.cast(), 4)
                || !is_aligned(tmd.cast(), 4)
                || !is_aligned(crl.cast(), 4)
            {
                return ES_EALIGN;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let tmd_bytes = slice_from_const(tmd.cast::<u8>(), tmd_size as usize);
            let cert_bytes =
                slice_from_const(certificates.cast::<u8>(), certificates_size as usize);
            let crl_bytes = if crl_size == 0 {
                &[][..]
            } else {
                slice_from_const(crl.cast::<u8>(), crl_size as usize)
            };
            let one = 1u32;

            fd.ioctlv(
                IOCTL_ES_ADDTITLESTART,
                4,
                [
                    IoctlvEntry::In(tmd_bytes),
                    IoctlvEntry::In(cert_bytes),
                    IoctlvEntry::In(crl_bytes),
                    IoctlvEntry::In(bytes_of(&one)),
                ],
            )
        })
    }
}

pub fn add_content_start(title_id: u64, cid: u32) -> i32 {
    with_state(|state| {
        if let Err(rc) = require_initialized(state) {
            return rc;
        }
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };

        fd.ioctlv(
            IOCTL_ES_ADDCONTENTSTART,
            2,
            [
                IoctlvEntry::In(bytes_of(&title_id)),
                IoctlvEntry::In(bytes_of(&cid)),
            ],
        )
    })
}

/// # Safety
///
/// `data` must satisfy libogc's alignment and size requirements.
pub unsafe fn add_content_data(cid: i32, data: *const u8, data_size: u32) -> i32 {
    unsafe {
        with_state(|state| {
            if cid < 0 || data.is_null() || data_size == 0 {
                return ES_EINVAL;
            }
            if !is_aligned(data.cast(), 4) {
                return ES_EALIGN;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let data_bytes = slice_from_const(data, data_size as usize);

            fd.ioctlv(
                IOCTL_ES_ADDCONTENTDATA,
                2,
                [IoctlvEntry::In(bytes_of(&cid)), IoctlvEntry::In(data_bytes)],
            )
        })
    }
}

pub fn add_content_finish(cid: u32) -> i32 {
    with_state(|state| {
        if let Err(rc) = require_initialized(state) {
            return rc;
        }
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };

        fd.ioctlv(
            IOCTL_ES_ADDCONTENTFINISH,
            1,
            [IoctlvEntry::In(bytes_of(&cid))],
        )
    })
}

pub fn add_title_finish() -> i32 {
    with_state(|state| {
        if let Err(rc) = require_initialized(state) {
            return rc;
        }
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };

        fd.ioctlv::<0>(IOCTL_ES_ADDTITLEFINISH, 0, [])
    })
}

pub fn add_title_cancel() -> i32 {
    with_state(|state| {
        if let Err(rc) = require_initialized(state) {
            return rc;
        }
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };

        fd.ioctlv::<0>(IOCTL_ES_ADDTITLECANCEL, 0, [])
    })
}

/// # Safety
///
/// All pointers must satisfy libogc's alignment, size, and ownership rules.
pub unsafe fn import_boot(params: ImportBootParams) -> i32 {
    unsafe {
        with_state(|state| {
            if params.tik.is_null() || params.tik_size == 0 {
                return ES_EINVAL;
            }
            if params.tik_certs.is_null() || params.tik_certs_size == 0 {
                return ES_EINVAL;
            }
            if params.tmd.is_null() || params.tmd_size == 0 {
                return ES_EINVAL;
            }
            if params.tmd_certs.is_null() || params.tmd_certs_size == 0 {
                return ES_EINVAL;
            }
            if params.content.is_null() || params.content_size == 0 {
                return ES_EINVAL;
            }
            if !is_aligned(params.tik.cast(), 4)
                || !is_aligned(params.tmd.cast(), 4)
                || !is_aligned(params.tik_certs.cast(), 4)
                || !is_aligned(params.tmd_certs.cast(), 4)
                || !is_aligned(params.content.cast(), 4)
            {
                return ES_EALIGN;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let tik_bytes = slice_from_const(params.tik.cast::<u8>(), params.tik_size as usize);
            let tik_certs = slice_from_const(
                params.tik_certs.cast::<u8>(),
                params.tik_certs_size as usize,
            );
            let tmd_bytes = slice_from_const(params.tmd.cast::<u8>(), params.tmd_size as usize);
            let tmd_certs = slice_from_const(
                params.tmd_certs.cast::<u8>(),
                params.tmd_certs_size as usize,
            );
            let empty: &[u8] = &[];
            let content = slice_from_const(params.content, params.content_size as usize);

            fd.ioctlv(
                IOCTL_ES_IMPORTBOOT,
                6,
                [
                    IoctlvEntry::In(tik_bytes),
                    IoctlvEntry::In(tik_certs),
                    IoctlvEntry::In(tmd_bytes),
                    IoctlvEntry::In(tmd_certs),
                    IoctlvEntry::In(empty),
                    IoctlvEntry::In(content),
                ],
            )
        })
    }
}

pub fn open_content(index: u16) -> i32 {
    with_state(|state| {
        if let Err(rc) = require_initialized(state) {
            return rc;
        }
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };
        fd.ioctlv(
            IOCTL_ES_OPENCONTENT,
            1,
            [IoctlvEntry::In(bytes_of(&(index as u32)))],
        )
    })
}

/// # Safety
///
/// `views` must satisfy libogc's alignment requirements.
pub unsafe fn open_title_content(title_id: u64, views: *const Tikview, index: u16) -> i32 {
    unsafe {
        with_state(|state| {
            if !is_aligned(views.cast(), 4) {
                return ES_EALIGN;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let view_bytes = slice_from_const(views.cast::<u8>(), core::mem::size_of::<Tikview>());
            fd.ioctlv(
                IOCTL_ES_OPENTITLECONTENT,
                3,
                [
                    IoctlvEntry::In(bytes_of(&title_id)),
                    IoctlvEntry::In(view_bytes),
                    IoctlvEntry::In(bytes_of(&(index as u32))),
                ],
            )
        })
    }
}

/// # Safety
///
/// `data` must satisfy libogc's alignment and size requirements.
pub unsafe fn read_content(cfd: i32, data: *mut u8, data_size: u32) -> i32 {
    unsafe {
        with_state(|state| {
            if cfd < 0 || data.is_null() || data_size == 0 {
                return ES_EINVAL;
            }
            if !is_aligned(data.cast(), 32) {
                return ES_EALIGN;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let out = slice_from_mut(data, data_size as usize);

            fd.ioctlv(
                IOCTL_ES_READCONTENT,
                1,
                [IoctlvEntry::In(bytes_of(&cfd)), IoctlvEntry::Out(out)],
            )
        })
    }
}

pub fn seek_content(cfd: i32, where_: i32, whence: i32) -> i32 {
    with_state(|state| {
        if cfd < 0 {
            return ES_EINVAL;
        }
        if let Err(rc) = require_initialized(state) {
            return rc;
        }
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };
        fd.ioctlv(
            IOCTL_ES_SEEKCONTENT,
            3,
            [
                IoctlvEntry::In(bytes_of(&cfd)),
                IoctlvEntry::In(bytes_of(&where_)),
                IoctlvEntry::In(bytes_of(&whence)),
            ],
        )
    })
}

pub fn close_content(cfd: i32) -> i32 {
    with_state(|state| {
        if cfd < 0 {
            return ES_EINVAL;
        }
        if let Err(rc) = require_initialized(state) {
            return rc;
        }
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };
        fd.ioctlv(IOCTL_ES_CLOSECONTENT, 1, [IoctlvEntry::In(bytes_of(&cfd))])
    })
}

pub fn delete_title(title_id: u64) -> i32 {
    with_state(|state| {
        if let Err(rc) = require_initialized(state) {
            return rc;
        }
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };
        fd.ioctlv(
            IOCTL_ES_DELETETITLE,
            1,
            [IoctlvEntry::In(bytes_of(&title_id))],
        )
    })
}

pub fn delete_title_content(title_id: u64) -> i32 {
    with_state(|state| {
        if let Err(rc) = require_initialized(state) {
            return rc;
        }
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };
        fd.ioctlv(
            IOCTL_ES_DELETETITLECONTENT,
            1,
            [IoctlvEntry::In(bytes_of(&title_id))],
        )
    })
}

/// # Safety
///
/// `iv`, `source`, and `dest` must satisfy libogc's alignment and size rules.
pub unsafe fn encrypt(
    keynum: u32,
    iv: *mut u32,
    source: *const u8,
    size: u32,
    dest: *mut u8,
) -> i32 {
    unsafe {
        with_state(|state| {
            if iv.is_null() || source.is_null() || size == 0 || dest.is_null() {
                return ES_EINVAL;
            }
            if !is_aligned(iv.cast(), 4)
                || !is_aligned(source.cast(), 32)
                || !is_aligned(dest.cast(), 32)
                || !is_aligned_value(size as usize, 16)
            {
                return ES_EALIGN;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let iv_in = slice_from_const(iv.cast::<u8>(), 0x10);
            let source_in = slice_from_const(source, size as usize);
            let iv_out = slice_from_mut(iv.cast::<u8>(), 0x10);
            let dest_out = slice_from_mut(dest, size as usize);

            fd.ioctlv(
                IOCTL_ES_ENCRYPT,
                3,
                [
                    IoctlvEntry::In(bytes_of(&keynum)),
                    IoctlvEntry::In(iv_in),
                    IoctlvEntry::In(source_in),
                    IoctlvEntry::Out(iv_out),
                    IoctlvEntry::Out(dest_out),
                ],
            )
        })
    }
}

/// # Safety
///
/// `iv`, `source`, and `dest` must satisfy libogc's alignment and size rules.
pub unsafe fn decrypt(
    keynum: u32,
    iv: *mut u32,
    source: *const u8,
    size: u32,
    dest: *mut u8,
) -> i32 {
    unsafe {
        with_state(|state| {
            if iv.is_null() || source.is_null() || size == 0 || dest.is_null() {
                return ES_EINVAL;
            }
            if !is_aligned(iv.cast(), 4)
                || !is_aligned(source.cast(), 32)
                || !is_aligned(dest.cast(), 32)
                || !is_aligned_value(size as usize, 16)
            {
                return ES_EALIGN;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let iv_in = slice_from_const(iv.cast::<u8>(), 0x10);
            let source_in = slice_from_const(source, size as usize);
            let iv_out = slice_from_mut(iv.cast::<u8>(), 0x10);
            let dest_out = slice_from_mut(dest, size as usize);

            fd.ioctlv(
                IOCTL_ES_DECRYPT,
                3,
                [
                    IoctlvEntry::In(bytes_of(&keynum)),
                    IoctlvEntry::In(iv_in),
                    IoctlvEntry::In(source_in),
                    IoctlvEntry::Out(iv_out),
                    IoctlvEntry::Out(dest_out),
                ],
            )
        })
    }
}

/// # Safety
///
/// Output buffers must satisfy libogc's alignment and size rules.
pub unsafe fn sign(
    source: *const u8,
    size: u32,
    ap_signature: *mut u8,
    ap_certificate: *mut SignedBlob,
) -> i32 {
    unsafe {
        with_state(|state| {
            if source.is_null() || size == 0 || ap_signature.is_null() || ap_certificate.is_null() {
                return ES_EINVAL;
            }
            if !is_aligned(source.cast(), 64)
                || !is_aligned(ap_signature.cast(), 4)
                || !is_aligned(ap_certificate.cast(), 4)
            {
                return ES_EALIGN;
            }
            if size > 0x10000 {
                return ES_EINVAL;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let source_in = slice_from_const(source, size as usize);
            let sig_out = slice_from_mut(ap_signature, 0x3c);
            let cert_out = slice_from_mut(ap_certificate.cast::<u8>(), 0x180);

            fd.ioctlv(
                IOCTL_ES_SIGN,
                1,
                [
                    IoctlvEntry::In(source_in),
                    IoctlvEntry::Out(sig_out),
                    IoctlvEntry::Out(cert_out),
                ],
            )
        })
    }
}

/// # Safety
///
/// Input buffers must satisfy libogc's alignment and size rules.
pub unsafe fn verify_sign(
    source: *const u8,
    size: u32,
    ap_signature: *const u8,
    certificates: *const SignedBlob,
    certificates_size: u32,
) -> i32 {
    unsafe {
        with_state(|state| {
            if source.is_null()
                || size == 0
                || ap_signature.is_null()
                || certificates.is_null()
                || certificates_size == 0
            {
                return ES_EINVAL;
            }
            if !sanity_check_certlist(certificates, certificates_size) {
                return ES_EINVAL;
            }
            if !is_aligned(source.cast(), 64)
                || !is_aligned(ap_signature.cast(), 4)
                || !is_aligned(certificates.cast(), 4)
            {
                return ES_EALIGN;
            }
            if size > 0x10000 {
                return ES_EINVAL;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let source_in = slice_from_const(source, size as usize);
            let sig_in = slice_from_const(ap_signature, 0x3c);
            let certs_in = slice_from_const(certificates.cast::<u8>(), certificates_size as usize);

            fd.ioctlv(
                IOCTL_ES_VERIFYSIGN,
                3,
                [
                    IoctlvEntry::In(source_in),
                    IoctlvEntry::In(sig_in),
                    IoctlvEntry::In(certs_in),
                ],
            )
        })
    }
}

/// # Safety
///
/// `outbuf` must point to a writable device-certificate buffer.
pub unsafe fn get_device_cert(outbuf: *mut u8) -> i32 {
    unsafe {
        with_state(|state| {
            if outbuf.is_null() {
                return ES_EINVAL;
            }
            if !is_aligned(outbuf.cast(), 4) {
                return ES_EALIGN;
            }
            if let Err(rc) = require_initialized(state) {
                return rc;
            }
            let fd = match current_fd(state) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let out = slice_from_mut(outbuf, 0x180);

            fd.ioctlv(IOCTL_ES_GETDEVICECERT, 0, [IoctlvEntry::Out(out)])
        })
    }
}

/// # Safety
///
/// `device_id` must be valid for writes.
pub unsafe fn get_device_id(device_id: *mut u32) -> i32 {
    with_state(|state| {
        if device_id.is_null() {
            return ES_EINVAL;
        }
        if let Err(rc) = require_initialized(state) {
            return rc;
        }
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };
        match ioctlv_scalar_out::<u32>(fd, IOCTL_ES_GETDEVICEID) {
            Ok((value, rc)) => {
                unsafe { device_id.write(value) };
                rc
            }
            Err(rc) => rc,
        }
    })
}

/// # Safety
///
/// `version` must be valid for writes.
pub unsafe fn get_boot2_version(version: *mut u32) -> i32 {
    with_state(|state| {
        if version.is_null() {
            return ES_EINVAL;
        }
        if let Err(rc) = require_initialized(state) {
            return rc;
        }
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };
        match ioctlv_scalar_out::<u32>(fd, IOCTL_ES_GETBOOT2VERSION) {
            Ok((value, rc)) => {
                unsafe { version.write(value) };
                rc
            }
            Err(rc) => rc,
        }
    })
}

pub fn check_has_korean_key() -> i32 {
    with_state(|state| {
        if let Err(rc) = require_initialized(state) {
            return rc;
        }
        let fd = match current_fd(state) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };
        fd.ioctlv::<0>(IOCTL_ES_CHECKHASKOREANKEY, 0, [])
    })
}

/// # Safety
///
/// `certs` must point to a valid signed certificate blob chain.
pub unsafe fn next_cert(certs: *const SignedBlob) -> *const SignedBlob {
    next_cert_inner(certs)
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    #[test]
    fn public_layout_matches_c() {
        assert_eq!(core::mem::size_of::<Tiklimit>(), 8);
        assert_eq!(core::mem::size_of::<Tikview>(), 216);
        assert_eq!(core::mem::size_of::<SigRsa2048>(), 320);
        assert_eq!(core::mem::size_of::<SigRsa4096>(), 576);
        assert_eq!(core::mem::size_of::<SigEcdsa>(), 128);
        assert_eq!(core::mem::size_of::<Tik>(), 356);
        assert_eq!(core::mem::size_of::<TmdContent>(), 36);
        assert_eq!(core::mem::size_of::<Tmd>(), 164);
        assert_eq!(core::mem::size_of::<TmdViewContent>(), 16);
        assert_eq!(core::mem::size_of::<TmdView>(), 92);
    }

    #[test]
    fn helper_functions_exist_without_backend() {
        let _ = init as fn() -> i32;
        let _ = close as fn() -> i32;
        let _ = reset as fn() -> i32;
        let _ = is_valid_signature as fn(Sigtype) -> bool;
        let _ = signature_size as fn(Sigtype) -> usize;
        let _ = is_valid_cert as fn(u32) -> bool;
        let _ = certificate_size as fn(u32) -> usize;
        let _ = tmd_size as fn(u16) -> usize;
        let _ = signed_tmd_size as fn(Sigtype, u16) -> usize;
        let _ = signed_tik_size as fn(Sigtype) -> usize;
        let _ = signed_cert_size as fn(Sigtype, u32) -> usize;
        let _ = get_title_id as unsafe fn(*mut u64) -> i32;
        let _ = launch_title as unsafe fn(u64, *const Tikview) -> i32;
        let _ = get_device_id as unsafe fn(*mut u32) -> i32;
        let _ = import_boot as unsafe fn(ImportBootParams) -> i32;
        let _ = next_cert as unsafe fn(*const SignedBlob) -> *const SignedBlob;
    }

    #[test]
    fn public_calls_require_explicit_initialization() {
        let mut title_id = 0;
        assert_eq!(unsafe { get_title_id(&mut title_id) }, ES_ENOTINIT);
    }

    #[test]
    fn get_title_id_checks_initialization_before_null_output() {
        assert_eq!(unsafe { get_title_id(core::ptr::null_mut()) }, ES_ENOTINIT);
    }
}
