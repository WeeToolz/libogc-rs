#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::ffi::{CStr, c_void};
use core::ptr::NonNull;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicUsize, Ordering};

use ogc_core::{mem_physical_to_k0, mem_virtual_to_physical};
#[cfg(not(gamecube))]
use ogc_irq as irq;
use ogc_lwp::sys as lwp_sys;

mod async_completion;
mod critical_lock;
mod dma_buffer;
mod poll_guard;
mod raw_contract;

#[cfg(target_arch = "powerpc")]
core::arch::global_asm!(
    r#"
    .section .text.IOS_IoctlvFormat,"ax",@progbits
    .balign 4
    .globl IOS_IoctlvFormat
    .type IOS_IoctlvFormat,@function
IOS_IoctlvFormat:
    stwu 1,-96(1)
    mflr 0
    stw 0,100(1)
    stw 3,16(1)
    stw 4,20(1)
    stw 5,24(1)
    li 0,0
    stw 0,28(1)
    stw 0,32(1)
    stw 6,36(1)
    stw 7,40(1)
    stw 8,44(1)
    stw 9,48(1)
    stw 10,52(1)
    li 0,4
    stw 0,56(1)
    addi 0,1,104
    stw 0,60(1)
    li 0,0
    stw 0,64(1)
    addi 3,1,16
    bl __ogc_ipc_ioctlv_format_bridge
    lwz 0,100(1)
    mtlr 0
    addi 1,1,96
    blr
    .size IOS_IoctlvFormat,.-IOS_IoctlvFormat

    .section .text.IOS_IoctlvFormatAsync,"ax",@progbits
    .balign 4
    .globl IOS_IoctlvFormatAsync
    .type IOS_IoctlvFormatAsync,@function
IOS_IoctlvFormatAsync:
    stwu 1,-96(1)
    mflr 0
    stw 0,100(1)
    stw 3,16(1)
    stw 4,20(1)
    stw 5,24(1)
    stw 6,28(1)
    stw 7,32(1)
    stw 8,36(1)
    stw 9,40(1)
    stw 10,44(1)
    li 0,0
    stw 0,48(1)
    stw 0,52(1)
    li 0,2
    stw 0,56(1)
    addi 0,1,104
    stw 0,60(1)
    li 0,1
    stw 0,64(1)
    addi 3,1,16
    bl __ogc_ipc_ioctlv_format_bridge
    lwz 0,100(1)
    mtlr 0
    addi 1,1,96
    blr
    .size IOS_IoctlvFormatAsync,.-IOS_IoctlvFormatAsync
"#
);

pub type Ipccallback = Option<async_completion::UserCallback>;
pub type DebugHook = unsafe fn(stage: u32, a: u32, b: u32, c: u32);

pub const HW_IPC_PPCBASE: u32 = 0xcd00_0000;
pub const HW_IPC_PPC_SEND: u32 = 1;
pub const HW_IPC_PPC_MSG_ACK: u32 = 2;
pub const HW_IPC_PPC_CTRL_ACK: u32 = 4;
pub const HW_IPC_PPC_CTRL_REGS: u32 = HW_IPC_PPC_MSG_ACK | HW_IPC_PPC_CTRL_ACK;

pub const IPC_HEAP: i32 = -1;

pub const IPC_OPEN_NONE: u32 = 0;
pub const IPC_OPEN_READ: u32 = 1;
pub const IPC_OPEN_WRITE: u32 = 2;
pub const IPC_OPEN_RW: u32 = IPC_OPEN_READ | IPC_OPEN_WRITE;
pub const IPC_MAXPATH_LEN: u32 = 64;

pub const IPC_OK: i32 = 0;
pub const IPC_EINVAL: i32 = -4;
pub const IPC_ENOHEAP: i32 = -5;
pub const IPC_ENOENT: i32 = -6;
pub const IPC_EQUEUEFULL: i32 = -8;
pub const IPC_ENOMEM: i32 = -22;

const IPC_REQUEST_ALIGN: usize = 32;
const IPC_QUEUE_CAPACITY: usize = 16;
const IPC_DEFERRED_CALLBACK_CAPACITY: u32 = 16;
const IPC_REQUEST_MAGIC_SEED: u32 = 0x4f45_4943;

#[repr(align(32))]
struct AlignedPath([u8; IPC_MAXPATH_LEN as usize + 1]);

#[repr(C, align(32))]
struct OpenAsyncMessage {
    path: AlignedPath,
    cb: Ipccallback,
    userdata: *mut c_void,
}
/// Default heap size used by libogc for IPC request records.
pub const IPC_DEFAULT_HEAP_BYTES: usize = 4096;
/// Legacy Rust staging-size constant retained for API compatibility.
///
/// Staging is now allocated dynamically from the shared LWP workspace and does
/// not reserve this amount from the public IPC arena.
pub const IPC_HEAP_MAX_BYTES: usize = 65_600;
const IPC_NUM_HEAPS: usize = 16;
const IPC_HEAP_ALIGN: usize = 32;
const IPC_HEAP_HEADER: usize = 8;
const IPC_HEAP_NULL: u32 = u32::MAX;
const RELNCH_RELAUNCH: u32 = 1;
const RELNCH_BACKGROUND: u32 = 2;
const IOS_OPEN: u32 = 0x01;
const IOS_CLOSE: u32 = 0x02;
const IOS_READ: u32 = 0x03;
const IOS_WRITE: u32 = 0x04;
const IOS_SEEK: u32 = 0x05;
const IOS_IOCTL: u32 = 0x06;
const IOS_IOCTLV: u32 = 0x07;
const IPC_CONTROL_INIT: u32 = 56;
const IPC_ACK_REPLY_INTERRUPT: u32 = 0x04;
const IPC_ACK_REQUEST_INTERRUPT: u32 = 0x02;
const IPC_ACK_INTERRUPT_COMPLETE: u32 = 0x08;
const IPC_REQUEST_INTERRUPT_MASK: u32 = 0x0022;
const IPC_REPLY_INTERRUPT_MASK: u32 = 0x0014;
const ACR_REGISTER_IRQ_CLEAR: usize = 48;
const ACR_IRQ_CLEAR_VALUE: u32 = 0x4000_0000;
#[cfg(target_arch = "powerpc")]
const MSR_EE: u32 = 0x8000;
#[cfg(target_arch = "powerpc")]
const MSR_FP: u32 = 0x2000;

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
pub struct Ioctlv {
    pub data: *mut c_void,
    pub len: u32,
}

mod ioctl_buffers;
pub use ioctl_buffers::IoctlBuffers;

#[derive(Debug)]
pub enum IoctlvEntry<'a> {
    In(&'a [u8]),
    /// An input vector with a null pointer but a meaningful IOS length.
    InNull(usize),
    Out(&'a mut [u8]),
    /// An omitted output vector, encoded as a null pointer and zero length.
    OutNull,
}

impl<'a> IoctlvEntry<'a> {
    fn as_mut_raw(&mut self) -> (*mut c_void, i32) {
        match self {
            Self::In(buf) => (buf.as_ptr() as *mut u8 as *mut c_void, buf.len() as i32),
            Self::InNull(len) => (core::ptr::null_mut(), (*len).min(i32::MAX as usize) as i32),
            Self::Out(buf) => (buf.as_mut_ptr().cast(), buf.len() as i32),
            Self::OutNull => (core::ptr::null_mut(), 0),
        }
    }
}

#[repr(transparent)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct HeapId(i32);

impl HeapId {
    pub const fn new(raw: i32) -> Self {
        Self(raw)
    }

    pub const fn raw(self) -> i32 {
        self.0
    }

    pub fn create(size: i32) -> Result<Self, i32> {
        if size <= 0 {
            return Err(IPC_EINVAL);
        }
        let _guard = HEAP_LOCK.lock_irq();
        ensure_request_heap_locked()?;
        create_heap_locked(size as usize).map(Self::new)
    }

    /// Returns libogc's default 4096-byte IPC heap, creating it lazily if needed.
    ///
    /// Temporary IOS request metadata should use this heap instead of creating
    /// a new heap for each operation.
    pub fn default_heap() -> Result<Self, i32> {
        request_heap()
    }

    /// The returned pointer is owned by this heap and must be freed with the
    /// same heap id.
    pub fn alloc(self, size: i32) -> Option<NonNull<c_void>> {
        if self.0 < 0 || self.0 as usize >= IPC_NUM_HEAPS || size <= 0 {
            return None;
        }
        unsafe { heap_alloc(self.0 as usize, size as usize) }
    }

    /// # Safety
    ///
    /// `ptr` must have been returned by [`HeapId::alloc`] for this heap id and
    /// must not already be freed.
    pub unsafe fn free(self, ptr: NonNull<c_void>) {
        if self.0 < 0 || self.0 as usize >= IPC_NUM_HEAPS {
            return;
        }
        unsafe { heap_free(self.0 as usize, ptr) }
    }
}

#[repr(transparent)]
#[derive(Debug, Eq, PartialEq)]
pub struct FileDescriptor(i32);

impl FileDescriptor {
    pub const fn new(raw: i32) -> Self {
        Self(raw)
    }

    pub const fn raw(&self) -> i32 {
        self.0
    }

    pub fn close(self) -> Result<(), i32> {
        let rc = with_request(|req| {
            req.cmd = IOS_CLOSE;
            req.fd = self.0;
            unsafe { submit_request(req, || {}) }
        })?;
        if rc == IPC_OK { Ok(()) } else { Err(rc) }
    }

    /// # Safety
    ///
    /// `ipc_cb` and `usrdata` must remain valid until the callback returns.
    pub unsafe fn close_async(self, ipc_cb: Ipccallback, usrdata: *mut c_void) -> i32 {
        if ipc_cb.is_none() {
            return match self.close() {
                Ok(()) => IPC_OK,
                Err(rc) => rc,
            };
        }

        let Ok(req_ptr) = alloc_request() else {
            return IPC_ENOMEM;
        };
        let req = unsafe { &mut *req_ptr.as_ptr() };
        *req = IpcRequest::zeroed();
        req.cmd = IOS_CLOSE;
        req.fd = self.0;
        req.cb = ipc_cb;
        req.usrdata = usrdata;

        let rc = unsafe { submit_async_request(req) };
        if rc < 0 {
            unsafe {
                free_request(req_ptr);
            }
        }
        rc
    }

    /// Queues an IOS read and calls `ipc_cb` when it completes.
    ///
    /// # Safety
    ///
    /// `buf` must remain writable and valid until `ipc_cb` returns. The
    /// callback and `usrdata` must also remain valid until that point.
    pub unsafe fn read_async(
        &self,
        buf: &mut [u8],
        ipc_cb: Ipccallback,
        usrdata: *mut c_void,
    ) -> i32 {
        let Ok(len) = u32::try_from(buf.len()) else {
            return IPC_EINVAL;
        };
        if len == 0 {
            return IPC_EINVAL;
        }

        unsafe { dc_invalidate_range(buf.as_mut_ptr().cast(), len as usize) };
        submit_file_async(self.0, IOS_READ, ipc_cb, usrdata, |req| {
            req.data.read.data = mem_virtual_to_physical(buf.as_mut_ptr() as usize) as *mut c_void;
            req.data.read.len = len;
        })
    }

    /// Queues an IOS write and calls `ipc_cb` when it completes.
    ///
    /// # Safety
    ///
    /// `buf` must remain readable and valid until `ipc_cb` returns. The
    /// callback and `usrdata` must also remain valid until that point.
    pub unsafe fn write_async(&self, buf: &[u8], ipc_cb: Ipccallback, usrdata: *mut c_void) -> i32 {
        let Ok(len) = u32::try_from(buf.len()) else {
            return IPC_EINVAL;
        };
        if len == 0 {
            return IPC_EINVAL;
        }

        unsafe { dc_flush_range(buf.as_ptr().cast(), len as usize) };
        submit_file_async(self.0, IOS_WRITE, ipc_cb, usrdata, |req| {
            req.data.write.data = mem_virtual_to_physical(buf.as_ptr() as usize) as *mut c_void;
            req.data.write.len = len;
        })
    }

    /// Queues an IOS seek and calls `ipc_cb` when it completes.
    ///
    /// # Safety
    ///
    /// The callback and `usrdata` must remain valid until the callback returns.
    pub unsafe fn seek_async(
        &self,
        where_: i32,
        whence: i32,
        ipc_cb: Ipccallback,
        usrdata: *mut c_void,
    ) -> i32 {
        submit_file_async(self.0, IOS_SEEK, ipc_cb, usrdata, |req| {
            req.data.seek.where_ = where_;
            req.data.seek.whence = whence;
        })
    }

    pub fn seek(&self, where_: i32, whence: i32) -> Result<i32, i32> {
        let rc = with_request(|req| {
            req.cmd = IOS_SEEK;
            req.fd = self.0;
            req.data.seek.where_ = where_;
            req.data.seek.whence = whence;
            unsafe { submit_request(req, || {}) }
        })?;
        if rc < 0 { Err(rc) } else { Ok(rc) }
    }

    pub fn read(&self, buf: &mut [u8]) -> Result<usize, i32> {
        let len = buf.len();
        if len == 0 {
            return Ok(0);
        }

        unsafe {
            dc_invalidate_range(buf.as_mut_ptr().cast(), len);
        }

        let rc = with_request(|req| {
            req.cmd = IOS_READ;
            req.fd = self.0;
            req.data.read.data = mem_virtual_to_physical(buf.as_mut_ptr() as usize) as *mut c_void;
            req.data.read.len = len as u32;
            unsafe { submit_request(req, || dc_invalidate_range(buf.as_mut_ptr().cast(), len)) }
        })?;

        if rc < 0 { Err(rc) } else { Ok(rc as usize) }
    }

    pub fn read_exact(&self, buf: &mut [u8]) -> Result<(), i32> {
        let read = self.read(buf)?;
        if read == buf.len() {
            Ok(())
        } else {
            Err(IPC_EINVAL)
        }
    }

    pub fn write(&self, buf: &[u8]) -> Result<usize, i32> {
        let len = buf.len();
        if len == 0 {
            return Ok(0);
        }

        unsafe {
            dc_flush_range(buf.as_ptr().cast(), len);
        }

        let rc = with_request(|req| {
            req.cmd = IOS_WRITE;
            req.fd = self.0;
            req.data.write.data = mem_virtual_to_physical(buf.as_ptr() as usize) as *mut c_void;
            req.data.write.len = len as u32;
            unsafe {
                debug_mark(
                    9,
                    req.fd as u32,
                    req.data.write.len,
                    req.data.write.data as usize as u32,
                );
            }
            unsafe { submit_request(req, || {}) }
        })?;
        if rc < 0 { Err(rc) } else { Ok(rc as usize) }
    }

    pub fn write_exact(&self, buf: &[u8]) -> Result<(), i32> {
        let written = self.write(buf)?;
        if written == buf.len() {
            Ok(())
        } else {
            Err(IPC_EINVAL)
        }
    }

    pub fn ioctl(&self, ioctl: i32, buffers: IoctlBuffers<'_>) -> i32 {
        self.ioctl_result(ioctl, buffers)
            .unwrap_or_else(|error| error)
    }

    /// Preserve submission errors separately from the final IOS result.
    pub fn ioctl_result(&self, ioctl: i32, buffers: IoctlBuffers<'_>) -> Result<i32, i32> {
        if buffers.len_in() < 0 || buffers.len_io() < 0 {
            return Err(IPC_EINVAL);
        }

        let in_ptr = buffers.buffer_in();
        let io_ptr = buffers.buffer_io();
        let in_len = buffers.len_in() as usize;
        let io_len = buffers.len_io() as usize;
        let staged_in = DmaStaging::if_needed(in_ptr, in_len)?;
        let staged_io = DmaStaging::if_needed(io_ptr, io_len)?;
        let transfer_in = staged_in
            .as_ref()
            .map_or(in_ptr, |buffer| buffer.pointer.as_ptr());
        let transfer_io = staged_io
            .as_ref()
            .map_or(io_ptr, |buffer| buffer.pointer.as_ptr());

        unsafe {
            if !in_ptr.is_null() && in_len > 0 {
                dc_flush_range(transfer_in.cast_const(), in_len);
            }
            if !io_ptr.is_null() && io_len > 0 {
                dc_flush_range(transfer_io.cast_const(), io_len);
            }
        }

        with_request(|req| {
            req.cmd = IOS_IOCTL;
            req.fd = self.0;
            req.data.ioctl.ioctl = ioctl as u32;
            req.data.ioctl.buffer_in = mem_virtual_to_physical(transfer_in as usize) as *mut c_void;
            req.data.ioctl.len_in = buffers.len_in() as u32;
            req.data.ioctl.buffer_io = mem_virtual_to_physical(transfer_io as usize) as *mut c_void;
            req.data.ioctl.len_io = buffers.len_io() as u32;

            unsafe {
                submit_request_result(req, || {
                    if !io_ptr.is_null() && io_len > 0 {
                        dc_invalidate_range(transfer_io.cast_const(), io_len);
                        if staged_io.is_some() {
                            core::ptr::copy_nonoverlapping(
                                transfer_io.cast::<u8>(),
                                io_ptr.cast::<u8>(),
                                io_len,
                            );
                        }
                    }
                })
            }
        })?
    }

    /// # Safety
    ///
    /// `buffers` and `ipc_cb`/`usrdata` must remain valid until the callback
    /// returns. The referenced buffers must not be accessed or freed while the
    /// request is pending.
    pub unsafe fn ioctl_async(
        &self,
        ioctl: i32,
        buffers: IoctlBuffers<'_>,
        ipc_cb: Ipccallback,
        usrdata: *mut c_void,
    ) -> i32 {
        if ipc_cb.is_none() {
            return self.ioctl(ioctl, buffers);
        }
        if buffers.len_in() < 0 || buffers.len_io() < 0 {
            return IPC_EINVAL;
        }

        let in_ptr = buffers.buffer_in();
        let io_ptr = buffers.buffer_io();
        let in_len = buffers.len_in() as usize;
        let io_len = buffers.len_io() as usize;

        unsafe {
            if !in_ptr.is_null() && in_len > 0 {
                dc_flush_range(in_ptr.cast_const(), in_len);
            }
            if !io_ptr.is_null() && io_len > 0 {
                dc_flush_range(io_ptr.cast_const(), io_len);
            }
        }

        let Ok(req_ptr) = alloc_request() else {
            return IPC_ENOMEM;
        };
        let req = unsafe { &mut *req_ptr.as_ptr() };
        *req = IpcRequest::zeroed();
        req.cmd = IOS_IOCTL;
        req.fd = self.0;
        req.cb = ipc_cb;
        req.usrdata = usrdata;
        req.data.ioctl.ioctl = ioctl as u32;
        req.data.ioctl.buffer_in = mem_virtual_to_physical(in_ptr as usize) as *mut c_void;
        req.data.ioctl.len_in = buffers.len_in() as u32;
        req.data.ioctl.buffer_io = mem_virtual_to_physical(io_ptr as usize) as *mut c_void;
        req.data.ioctl.len_io = buffers.len_io() as u32;

        let rc = unsafe { submit_async_request(req) };
        if rc < 0 {
            unsafe {
                free_request(req_ptr);
            }
        }
        rc
    }

    /// Submit raw vectors and wait for the final IOS reply.
    ///
    /// Unlike the slice interface, this preserves a zero-length IOS address.
    /// Some IOS protocols use this address as part of the request.
    ///
    /// # Safety
    /// `argv` must point to `cnt_in + cnt_io` writable, 32-byte-aligned vectors.
    /// Non-empty buffers must be cache-line-aligned DMA buffers with writable
    /// capacity rounded up to 32 bytes. The complete rounded span must remain
    /// exclusively owned until this call returns. Zero-length addresses
    /// must be valid for the selected IOS protocol. IOS can interpret them even
    /// though the CPU does not read them.
    pub unsafe fn ioctlv_raw<const N: usize>(
        &self,
        ioctl: i32,
        cnt_in: usize,
        cnt_io: usize,
        vectors: &mut [Ioctlv; N],
    ) -> i32 {
        let argv = vectors.as_mut_ptr();
        let Some((total, raw_len)) = raw_contract::vector_layout(
            cnt_in,
            cnt_io,
            N,
            core::mem::size_of::<Ioctlv>(),
            argv as usize,
        ) else {
            return IPC_EINVAL;
        };
        for entry in vectors.iter().take(total) {
            if !raw_contract::buffer_valid(entry.data as usize, entry.len as usize) {
                return IPC_EINVAL;
            }
        }
        let originals: [*mut c_void; N] = core::array::from_fn(|index| vectors[index].data);
        for entry in vectors.iter_mut().take(total) {
            unsafe {
                dc_flush_range(entry.data.cast_const(), entry.len as usize);
            }
            entry.data = raw_contract::request_address(entry.data as usize, entry.len as usize)
                as *mut c_void;
        }
        let result = with_request(|req| {
            req.cmd = IOS_IOCTLV;
            req.fd = self.0;
            req.data.ioctlv.ioctl = ioctl as u32;
            req.data.ioctlv.argcin = cnt_in as u32;
            req.data.ioctlv.argcio = cnt_io as u32;
            req.data.ioctlv.argv = mem_virtual_to_physical(argv as usize) as *mut Ioctlv;
            unsafe {
                dc_flush_range(argv.cast_const().cast(), raw_len);
                submit_request(req, || {
                    dc_invalidate_range(argv.cast_const().cast(), raw_len);
                    for entry in core::slice::from_raw_parts(argv, total).iter().skip(cnt_in) {
                        if entry.len > 0 {
                            dc_invalidate_range(entry.data.cast_const(), entry.len as usize);
                        }
                    }
                })
            }
        });
        // The reply handler maps non-null vectors to cached aliases. Restore
        // caller addresses also on request-allocation failure and for the IOS
        // zero-length address, which must retain its protocol value.
        for (entry, original) in vectors.iter_mut().zip(originals).take(total) {
            entry.data = original;
        }
        match result {
            Ok(rc) | Err(rc) => rc,
        }
    }

    /// Preserve submission errors separately from the final IOS result.
    pub fn ioctlv_result<'a, const N: usize>(
        &self,
        ioctl: i32,
        cnt_in: usize,
        argv: [IoctlvEntry<'a>; N],
    ) -> Result<i32, i32> {
        if cnt_in > N {
            return Err(IPC_EINVAL);
        }
        ioctlv_common_result(self.0, ioctl, cnt_in, argv, 0, 0)
    }

    pub fn ioctlv<'a, const N: usize>(
        &self,
        ioctl: i32,
        cnt_in: usize,
        argv: [IoctlvEntry<'a>; N],
    ) -> i32 {
        if cnt_in > N {
            return IPC_EINVAL;
        }

        ioctlv_common(self.0, ioctl, cnt_in, argv, 0, 0)
    }

    /// # Safety
    ///
    /// `argv` and every buffer it references must remain valid and cache-safe
    /// until `ipc_cb` returns. The callback is responsible for invalidating any
    /// output buffers before reading them.
    pub unsafe fn ioctlv_async_raw(
        &self,
        ioctl: i32,
        cnt_in: usize,
        cnt_io: usize,
        argv: *mut Ioctlv,
        ipc_cb: Ipccallback,
        usrdata: *mut c_void,
    ) -> i32 {
        unsafe {
            let Some(total) = cnt_in.checked_add(cnt_io) else {
                return IPC_EINVAL;
            };
            if ipc_cb.is_none()
                || total == 0
                || argv.is_null()
                || cnt_in > i32::MAX as usize
                || cnt_io > i32::MAX as usize
            {
                return IPC_EINVAL;
            }
            let Some(raw_len) = total.checked_mul(core::mem::size_of::<Ioctlv>()) else {
                return IPC_EINVAL;
            };
            let mut debug_original = [0u32; 8];
            let mut debug_physical = [0u32; 8];
            let mut debug_len = [0u32; 8];

            {
                let raw = core::slice::from_raw_parts_mut(argv, total);
                for (idx, entry) in raw.iter_mut().enumerate() {
                    let original = entry.data as usize as u32;
                    if !entry.data.is_null() && entry.len > 0 {
                        dc_flush_range(entry.data.cast_const(), entry.len as usize);
                    }
                    if !entry.data.is_null() {
                        entry.data = mem_virtual_to_physical(entry.data as usize) as *mut _;
                    }
                    if idx < debug_original.len() {
                        debug_original[idx] = original;
                        debug_physical[idx] = entry.data as usize as u32;
                        debug_len[idx] = entry.len;
                    }
                }
                dc_flush_range(argv.cast_const().cast(), raw_len);
            }

            let Ok(req_ptr) = alloc_request() else {
                return IPC_ENOMEM;
            };
            let req = { &mut *req_ptr.as_ptr() };
            *req = IpcRequest::zeroed();
            req.cmd = IOS_IOCTLV;
            req.fd = self.0;
            req.cb = ipc_cb;
            req.usrdata = usrdata;
            req.data.ioctlv.ioctl = ioctl as u32;
            req.data.ioctlv.argcin = cnt_in as u32;
            req.data.ioctlv.argcio = cnt_io as u32;
            req.data.ioctlv.argv = mem_virtual_to_physical(argv as usize) as *mut Ioctlv;

            // 30: async IOCTLV submit summary: request ptr, ioctl, packed in/out counts.
            debug_mark(
                30,
                req as *mut IpcRequest as usize as u32,
                ioctl as u32,
                ((cnt_in as u32) << 16) | (cnt_io as u32 & 0xffff),
            );
            // 31: argv summary: virtual argv ptr, physical argv ptr, total entry count.
            debug_mark(
                31,
                argv as usize as u32,
                req.data.ioctlv.argv as usize as u32,
                total as u32,
            );
            for idx in 0..total.min(debug_original.len()) {
                // 32..39: per-entry submit trace: original ptr, physical ptr, len.
                debug_mark(
                    32 + idx as u32,
                    debug_original[idx],
                    debug_physical[idx],
                    debug_len[idx],
                );
            }

            let rc = { submit_async_request(req) };
            if rc < 0 {
                {
                    free_request(req_ptr);
                }
            }
            rc
        }
    }

    pub fn ioctlv_reboot<'a, const N: usize>(
        &self,
        ioctl: i32,
        cnt_in: usize,
        argv: [IoctlvEntry<'a>; N],
    ) -> i32 {
        if cnt_in > N {
            return IPC_EINVAL;
        }

        ioctlv_common(self.0, ioctl, cnt_in, argv, RELNCH_RELAUNCH, 0)
    }

    /// # Safety
    ///
    /// `argv` must remain valid for the duration of the call.
    pub unsafe fn ioctlv_reboot_background<'a, const N: usize>(
        &self,
        ioctl: i32,
        cnt_in: usize,
        argv: [IoctlvEntry<'a>; N],
    ) -> i32 {
        if cnt_in > N {
            return IPC_EINVAL;
        }

        ioctlv_common(
            self.0,
            ioctl,
            cnt_in,
            argv,
            RELNCH_RELAUNCH | RELNCH_BACKGROUND,
            0,
        )
    }
}

fn submit_file_async(
    fd: i32,
    cmd: u32,
    ipc_cb: Ipccallback,
    usrdata: *mut c_void,
    configure: impl FnOnce(&mut IpcRequest),
) -> i32 {
    let Ok(req_ptr) = alloc_request() else {
        return IPC_ENOMEM;
    };
    let req = unsafe { &mut *req_ptr.as_ptr() };
    *req = IpcRequest::zeroed();
    req.cmd = cmd;
    req.fd = fd;
    req.cb = Some(ipc_cb.unwrap_or(async_noop_callback));
    req.usrdata = usrdata;
    configure(req);

    let rc = unsafe { submit_async_request(req) };
    if rc < 0 {
        unsafe { free_request(req_ptr) };
    }
    rc
}

unsafe extern "C" fn async_noop_callback(_result: i32, _usrdata: *mut c_void) -> i32 {
    IPC_OK
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct IpcOpen {
    filepath: *mut c_void,
    mode: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct IpcReadWrite {
    data: *mut c_void,
    len: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct IpcSeek {
    where_: i32,
    whence: i32,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct IpcIoctl {
    ioctl: u32,
    buffer_in: *mut c_void,
    len_in: u32,
    buffer_io: *mut c_void,
    len_io: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct IpcIoctlv {
    ioctl: u32,
    argcin: u32,
    argcio: u32,
    argv: *mut Ioctlv,
}

#[repr(C)]
union IpcRequestData {
    open: IpcOpen,
    read: IpcReadWrite,
    write: IpcReadWrite,
    seek: IpcSeek,
    ioctl: IpcIoctl,
    ioctlv: IpcIoctlv,
    args: [u32; 5],
}

#[repr(C, align(32))]
struct IpcRequest {
    cmd: u32,
    result: i32,
    fd: i32,
    data: IpcRequestData,
    cb: Ipccallback,
    usrdata: *mut c_void,
    relnch: u32,
    syncqueue: u32,
    magic: u32,
    completion_reservation: u32,
    pad1: [u8; 8],
}

const _: () = {
    assert!(core::mem::size_of::<IpcRequest>() == 64);
    assert!(core::mem::offset_of!(IpcRequest, cmd) == 0);
    assert!(core::mem::offset_of!(IpcRequest, result) == 4);
    assert!(core::mem::offset_of!(IpcRequest, fd) == 8);
    assert!(core::mem::offset_of!(IpcRequest, data) == 12);
    assert!(core::mem::offset_of!(IpcRequest, completion_reservation) == 52);
};

impl IpcRequest {
    fn zeroed() -> Self {
        unsafe { core::mem::zeroed() }
    }
}

fn with_request<R>(f: impl FnOnce(&mut IpcRequest) -> R) -> Result<R, i32> {
    let ptr = alloc_request()?;
    let req = unsafe { &mut *ptr.as_ptr() };
    *req = IpcRequest::zeroed();
    let result = f(req);
    unsafe {
        free_request(ptr);
    }
    Ok(result)
}

fn ensure_request_heap_locked() -> Result<i32, i32> {
    let current = IPC_REQUEST_HEAP.load(Ordering::Acquire);
    if current >= 0 {
        return Ok(current);
    }
    let id = create_heap_locked(IPC_DEFAULT_HEAP_BYTES)?;
    IPC_REQUEST_HEAP.store(id, Ordering::Release);
    Ok(id)
}

fn request_heap() -> Result<HeapId, i32> {
    let current = IPC_REQUEST_HEAP.load(Ordering::Acquire);
    if current >= 0 {
        return Ok(HeapId::new(current));
    }
    let _guard = HEAP_LOCK.lock_irq();
    ensure_request_heap_locked().map(HeapId::new)
}

fn alloc_request() -> Result<NonNull<IpcRequest>, i32> {
    let ptr = request_heap()?
        .alloc(core::mem::size_of::<IpcRequest>() as i32)
        .ok_or(IPC_ENOMEM)?;
    Ok(ptr.cast())
}

unsafe fn free_request(ptr: NonNull<IpcRequest>) {
    if let Ok(heap) = request_heap() {
        unsafe { heap.free(ptr.cast()) };
    }
}

unsafe fn free_heap_message<T>(ptr: *mut T) {
    if let (Some(ptr), Ok(heap)) = (NonNull::new(ptr.cast::<c_void>()), request_heap()) {
        unsafe { heap.free(ptr) };
    }
}

unsafe fn deferred_request_callback(argument: *mut c_void) {
    unsafe {
        let req = argument.cast::<IpcRequest>();
        if req.is_null() {
            return;
        }

        ASYNC_CALLBACKS_PENDING.fetch_sub(1, Ordering::AcqRel);
        let result = (*req).result;
        let callback = (*req).cb;
        let user_data = (*req).usrdata;
        async_completion::deliver_reply(result, callback, user_data, || {
            if let Some(ptr) = NonNull::new(req) {
                free_request(ptr);
            }
        });
    }
}

unsafe extern "C" fn open_async_cb(result: i32, usrdata: *mut c_void) -> i32 {
    unsafe {
        let msg = usrdata.cast::<OpenAsyncMessage>();
        if msg.is_null() {
            return IPC_EINVAL;
        }

        let cb = (*msg).cb;
        let user = (*msg).userdata;
        if let Some(cb) = cb {
            cb(result, user);
        }

        free_heap_message(msg);
        IPC_OK
    }
}

#[derive(Copy, Clone, Default)]
struct IoctlvMeta {
    data: *mut c_void,
    len: usize,
}

#[repr(align(32))]
struct AlignedIoctlv<const N: usize>([Ioctlv; N]);

#[repr(C)]
#[derive(Copy, Clone)]
struct HeapBlock {
    size: u32,
    next: u32,
    pad: [u8; 8],
}

impl HeapBlock {
    const fn new(size: u32, next: u32) -> Self {
        Self {
            size,
            next,
            pad: [0; 8],
        }
    }
}

#[derive(Copy, Clone)]
struct HeapState {
    base: usize,
    size: u32,
    free_head: u32,
}

impl HeapState {
    const EMPTY: Self = Self {
        base: 0,
        size: 0,
        free_head: IPC_HEAP_NULL,
    };
}

static HEAP_LOCK: SpinLock = SpinLock::new();
static IPC_REQUEST_HEAP: AtomicI32 = AtomicI32::new(-1);
static IPC_BUFFER_LO: AtomicUsize = AtomicUsize::new(0);
static IPC_BUFFER_HI: AtomicUsize = AtomicUsize::new(0);
static IPC_BUFFER_CURRENT_LO: AtomicUsize = AtomicUsize::new(0);
static IPC_BUFFER_CURRENT_HI: AtomicUsize = AtomicUsize::new(0);
static mut IPC_HEAPS: [HeapState; IPC_NUM_HEAPS] = [HeapState::EMPTY; IPC_NUM_HEAPS];

static IPC_INITIALIZED: AtomicBool = AtomicBool::new(false);
static IPC_MAILBOX_ACK: AtomicU32 = AtomicU32::new(1);
static IPC_QUEUE: [AtomicUsize; IPC_QUEUE_CAPACITY] =
    [const { AtomicUsize::new(0) }; IPC_QUEUE_CAPACITY];
static IPC_QUEUE_WRITE: AtomicU32 = AtomicU32::new(0);
static IPC_QUEUE_SEND: AtomicU32 = AtomicU32::new(0);
static IPC_QUEUE_COUNT: AtomicU32 = AtomicU32::new(0);
static IPC_SENT_COUNT: AtomicU32 = AtomicU32::new(0);
static SYNC_WAIT_REQ: AtomicUsize = AtomicUsize::new(0);
static SYNC_WAIT_DONE: AtomicBool = AtomicBool::new(false);
static ASYNC_CALLBACKS_PENDING: AtomicU32 = AtomicU32::new(0);
static REQUEST_MAGIC: AtomicU32 = AtomicU32::new(0);
static IPC_LOCK: SpinLock = SpinLock::new();
static SYNC_WAIT_MUTEX: AtomicU32 = AtomicU32::new(lwp_sys::LWP_MUTEX_NULL);
static DEBUG_HOOK: AtomicUsize = AtomicUsize::new(0);
#[cfg(not(gamecube))]
static IPC_IRQ_INSTALLED: AtomicBool = AtomicBool::new(false);
static RELAUNCH_REQ: AtomicUsize = AtomicUsize::new(0);

struct SpinLock(AtomicBool);

impl SpinLock {
    const fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    // Heap and request queue sections never sleep. Prevent a high-priority
    // deferred completion from preempting an owner and then waiting above it.
    fn lock_irq(&self) -> SpinLockGuard<'_> {
        let irq_state = critical_lock::acquire(
            || unsafe { lock_by_msr() },
            |state| unsafe { unlock_by_msr(state) },
            || {
                self.0
                    .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
                    .is_ok()
            },
            lwp_sys::yield_thread,
        );
        SpinLockGuard {
            lock: self,
            irq_state,
        }
    }
}

struct SpinLockGuard<'a> {
    lock: &'a SpinLock,
    irq_state: u32,
}

impl Drop for SpinLockGuard<'_> {
    fn drop(&mut self) {
        critical_lock::release(
            self.irq_state,
            || self.lock.0.store(false, Ordering::Release),
            |state| unsafe { unlock_by_msr(state) },
        );
    }
}

struct SyncWaitGuard {
    mutex: lwp_sys::Mutex,
}

impl Drop for SyncWaitGuard {
    fn drop(&mut self) {
        if lwp_sys::mutex_unlock(self.mutex) == lwp_sys::LWP_SUCCESSFUL {
            // The next serialized caller may have donated a higher priority to
            // this thread. Give it an immediate scheduler point after release.
            lwp_sys::yield_thread();
        }
    }
}

fn lock_sync_wait() -> Result<SyncWaitGuard, i32> {
    // A synchronous IOS request sleeps until the reply interrupt wakes its LWP
    // queue. Use an LWP mutex rather than a spin lock so a higher-priority
    // second caller blocks and donates priority to the sleeping owner.
    let mut mutex = SYNC_WAIT_MUTEX.load(Ordering::Acquire);
    if mutex == lwp_sys::LWP_MUTEX_NULL {
        let mut candidate = lwp_sys::LWP_MUTEX_NULL;
        // SAFETY: `candidate` is a valid unique output reference.
        if unsafe { lwp_sys::mutex_init(&mut candidate, 0) } != lwp_sys::LWP_SUCCESSFUL {
            return Err(IPC_ENOMEM);
        }
        match SYNC_WAIT_MUTEX.compare_exchange(
            lwp_sys::LWP_MUTEX_NULL,
            candidate,
            Ordering::AcqRel,
            Ordering::Acquire,
        ) {
            Ok(_) => mutex = candidate,
            Err(existing) => {
                let _ = lwp_sys::mutex_destroy(candidate);
                mutex = existing;
            }
        }
    }
    if lwp_sys::mutex_lock(mutex) != lwp_sys::LWP_SUCCESSFUL {
        return Err(IPC_EQUEUEFULL);
    }
    Ok(SyncWaitGuard { mutex })
}

const fn align_down(value: usize, align: usize) -> usize {
    value & !(align - 1)
}

fn c_heap_block_size(size: usize) -> Option<usize> {
    let page = IPC_HEAP_ALIGN;
    let overhead = core::mem::size_of::<*mut c_void>().checked_mul(2)?;
    let excess = size % page;
    let mut total = size.checked_add(page)?.checked_add(overhead)?;
    if excess != 0 {
        total = total.checked_add(page - excess)?;
    }
    Some(total.max(16)) // sizeof(heap_block) in the 32-bit C implementation.
}

fn heap_block_size(size: usize) -> Option<usize> {
    c_heap_block_size(size)
}

fn request_seed() -> u32 {
    let addr = (&REQUEST_MAGIC as *const AtomicU32 as usize) as u32;
    IPC_REQUEST_MAGIC_SEED ^ addr ^ 0x1357_9bdf
}

fn request_magic() -> u32 {
    let current = REQUEST_MAGIC.load(Ordering::Acquire);
    if current != 0 {
        return current;
    }

    let mut value = request_seed().wrapping_mul(214013).wrapping_add(2531011);
    if value == 0 {
        value = 0x6d6f_6763;
    }

    match REQUEST_MAGIC.compare_exchange(0, value, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => value,
        Err(existing) => existing,
    }
}

fn ensure_client_initialized() {
    if !IPC_INITIALIZED.load(Ordering::Acquire) {
        let _ = request_magic();
        IPC_MAILBOX_ACK.store(1, Ordering::Release);
        unsafe {
            #[cfg(not(gamecube))]
            install_interrupt_handler();
            write_reg(1, IPC_CONTROL_INIT);
        }
        IPC_INITIALIZED.store(true, Ordering::Release);
    }
}

fn reset_request_queue() {
    for slot in &IPC_QUEUE {
        slot.store(0, Ordering::Relaxed);
    }
    IPC_QUEUE_WRITE.store(0, Ordering::Relaxed);
    IPC_QUEUE_SEND.store(0, Ordering::Relaxed);
    IPC_QUEUE_COUNT.store(0, Ordering::Relaxed);
    IPC_SENT_COUNT.store(0, Ordering::Relaxed);
    RELAUNCH_REQ.store(0, Ordering::Release);
}

#[cfg(not(gamecube))]
unsafe extern "C" fn interrupt_handler(_irq: u32, _ctx: *mut c_void) {
    unsafe {
        let status = read_reg(1);
        debug_mark(12, status, 0, 0);
        if (status & IPC_REPLY_INTERRUPT_MASK) == IPC_REPLY_INTERRUPT_MASK {
            reply_handler();
        }

        let status = read_reg(1);
        debug_mark(13, status, 0, 0);
        if (status & IPC_REQUEST_INTERRUPT_MASK) == IPC_REQUEST_INTERRUPT_MASK {
            ackhandler();
        }
    }
}

unsafe fn poll_pending() -> bool {
    unsafe {
        let mut handled = false;
        let status = read_reg(1);
        debug_mark(20, status, read_reg(2), 0);
        if (status & IPC_REPLY_INTERRUPT_MASK) == IPC_REPLY_INTERRUPT_MASK {
            reply_handler();
            handled = true;
        }

        let status = read_reg(1);
        debug_mark(21, status, 0, 0);
        if (status & IPC_REQUEST_INTERRUPT_MASK) == IPC_REQUEST_INTERRUPT_MASK {
            ackhandler();
            handled = true;
        }

        handled
    }
}

#[cfg(not(gamecube))]
unsafe fn install_interrupt_handler() {
    unsafe {
        // The ACR IRQ exists in libogc's Wii mode only.
        if IPC_IRQ_INSTALLED.swap(true, Ordering::AcqRel) {
            return;
        }

        let _ = irq::request(
            irq::IRQ_PI_ACR,
            Some(interrupt_handler),
            core::ptr::null_mut(),
        );
        irq::enable(irq::IM_PI_ACR);
    }
}

pub fn set_debug_hook(hook: Option<DebugHook>) {
    let raw = hook.map_or(0, |hook| hook as usize);
    DEBUG_HOOK.store(raw, Ordering::Release);
}

/// Services pending IPC reply/request status without relying on CPU IRQ dispatch.
///
/// This mirrors the work normally performed by the PI/ACR interrupt handler and
/// remains useful as a diagnostic fallback when explicitly testing without
/// automatic IRQ delivery. It returns `true` when at least one IPC status bit
/// was serviced.
pub fn poll() -> bool {
    ensure_client_initialized();
    // The interrupt handler consumes the same mailbox registers. Keep it out
    // while checking and draining replies, then restore the caller's MSR before
    // yielding to the deferred completion worker.
    let handled = poll_guard::protected_poll(
        || unsafe { lock_by_msr() },
        |state| unsafe { unlock_by_msr(state) },
        || unsafe { poll_pending() },
    );
    if handled {
        // A reply can make the deferred callback worker runnable. `poll` runs
        // in thread context, so give that worker a scheduler point before the
        // caller checks completion state.
        lwp_sys::yield_thread();
    }
    handled
}

/// Reinitializes the PPC-side IPC client state after IOS has been relaunched.
///
/// This keeps the installed IRQ handler and mailbox setup in sync with the new
/// IOS instance before new requests are submitted.
pub fn reinitialize() {
    REQUEST_MAGIC.store(0, Ordering::Release);
    IPC_MAILBOX_ACK.store(1, Ordering::Release);
    reset_request_queue();
    IPC_INITIALIZED.store(false, Ordering::Release);
    ensure_client_initialized();
}

fn debug_mark(stage: u32, a: u32, b: u32, c: u32) {
    let hook = DEBUG_HOOK.load(Ordering::Acquire);
    if hook != 0 {
        let hook: DebugHook = unsafe { core::mem::transmute(hook) };
        unsafe {
            hook(stage, a, b, c);
        }
    }
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn lock_by_msr() -> u32 {
    unsafe {
        let msr = ppc_mfmsr();
        ppc_mtmsr(msr & !(MSR_EE | MSR_FP));
        ogc_sys::msr_lock_state(msr)
    }
}

#[cfg(not(target_arch = "powerpc"))]
#[inline(always)]
unsafe fn lock_by_msr() -> u32 {
    0
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn unlock_by_msr(state: u32) {
    unsafe {
        ppc_mtmsr(ogc_sys::msr_unlock_value(ppc_mfmsr(), state));
    }
}

#[cfg(not(target_arch = "powerpc"))]
#[inline(always)]
unsafe fn unlock_by_msr(_state: u32) {}

fn s32_to_u32(value: i32) -> Option<u32> {
    if value < 0 { None } else { Some(value as u32) }
}

fn reg_ptr(index: usize) -> *mut u32 {
    (HW_IPC_PPCBASE as usize as *mut u32).wrapping_add(index)
}

unsafe fn read_reg(index: usize) -> u32 {
    unsafe { core::ptr::read_volatile(reg_ptr(index)) }
}

unsafe fn write_reg(index: usize, value: u32) {
    unsafe {
        core::ptr::write_volatile(reg_ptr(index), value);
    }
}

unsafe fn acr_write_reg(reg: usize, value: u32) {
    unsafe {
        core::ptr::write_volatile(reg_ptr(reg >> 2), value);
    }
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn ppc_mfmsr() -> u32 {
    let value: u32;
    unsafe {
        core::arch::asm!("mfmsr {0}", out(reg) value, options(nostack));
    }
    value
}

#[cfg(not(target_arch = "powerpc"))]
#[inline(always)]
unsafe fn ppc_mfmsr() -> u32 {
    0
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn ppc_mtmsr(value: u32) {
    unsafe {
        core::arch::asm!("mtmsr {0}", in(reg) value, options(nostack));
    }
}

#[cfg(not(target_arch = "powerpc"))]
#[inline(always)]
unsafe fn ppc_mtmsr(_value: u32) {}

unsafe fn write_request(req: &mut IpcRequest) {
    req.magic = request_magic();
    if req.relnch & RELNCH_RELAUNCH != 0 {
        RELAUNCH_REQ.store(req as *mut IpcRequest as usize, Ordering::Release);
        IPC_MAILBOX_ACK.store(0, Ordering::Release);
    }

    let req_ptr = req as *mut IpcRequest as *mut c_void;
    unsafe {
        dc_flush_range(req_ptr.cast_const(), core::mem::size_of::<IpcRequest>());
        write_reg(0, mem_virtual_to_physical(req_ptr as usize));
        let control = read_reg(1);
        write_reg(1, (control & 0x30) | HW_IPC_PPC_SEND);
    }
    debug_mark(5, req.cmd, req.fd as u32, req.magic);
}

fn queue_request(req: &mut IpcRequest) -> i32 {
    let queued = IPC_QUEUE_COUNT.load(Ordering::Relaxed);
    let sent = IPC_SENT_COUNT.load(Ordering::Relaxed);
    if queued.wrapping_sub(sent) >= IPC_QUEUE_CAPACITY as u32 {
        return IPC_EQUEUEFULL;
    }

    let write = IPC_QUEUE_WRITE.load(Ordering::Relaxed);
    IPC_QUEUE[(write as usize) & (IPC_QUEUE_CAPACITY - 1)]
        .store(req as *mut IpcRequest as usize, Ordering::Relaxed);
    IPC_QUEUE_WRITE.store(write.wrapping_add(1), Ordering::Relaxed);
    IPC_QUEUE_COUNT.store(queued.wrapping_add(1), Ordering::Release);
    IPC_OK
}

unsafe fn send_next_request() {
    unsafe {
        if IPC_MAILBOX_ACK.load(Ordering::Acquire) == 0 {
            return;
        }

        let queued = IPC_QUEUE_COUNT.load(Ordering::Acquire);
        let sent = IPC_SENT_COUNT.load(Ordering::Relaxed);
        if queued.wrapping_sub(sent) == 0 {
            return;
        }

        let send = IPC_QUEUE_SEND.load(Ordering::Relaxed);
        let slot = &IPC_QUEUE[(send as usize) & (IPC_QUEUE_CAPACITY - 1)];
        let req = slot.swap(0, Ordering::Relaxed) as *mut IpcRequest;
        if req.is_null() {
            return;
        }

        IPC_QUEUE_SEND.store(send.wrapping_add(1), Ordering::Relaxed);
        IPC_SENT_COUNT.store(sent.wrapping_add(1), Ordering::Release);
        IPC_MAILBOX_ACK.store(0, Ordering::Release);
        write_request(&mut *req);
    }
}

unsafe fn reply_handler() {
    unsafe {
        let req_ptr = read_reg(2);
        if req_ptr == 0 {
            return;
        }

        let ipc_ack = (read_reg(1) & 0x30) | IPC_ACK_REPLY_INTERRUPT;
        write_reg(1, ipc_ack);
        acr_write_reg(ACR_REGISTER_IRQ_CLEAR, ACR_IRQ_CLEAR_VALUE);

        let req = mem_physical_to_k0(req_ptr) as *mut IpcRequest;
        if req.is_null() {
            return;
        }

        dc_invalidate_range(req.cast_const().cast(), 32);
        // 47: reply header snapshot after the initial 32-byte invalidate.
        debug_mark(47, req_ptr, (*req).cmd, (*req).result as u32);
        dc_invalidate_range(req.cast_const().cast(), core::mem::size_of::<IpcRequest>());
        // 59: post-full-invalidate magic check snapshot: current magic, expected magic, result.
        debug_mark(59, (*req).magic, request_magic(), (*req).result as u32);

        if (*req).magic == request_magic() {
            let request_cmd = (*req).cmd;
            if request_cmd == IOS_IOCTLV {
                // 48: reply full request snapshot: req ptr, result, ioctl.
                debug_mark(48, req_ptr, (*req).result as u32, (*req).data.ioctlv.ioctl);
                // 49: reply IOCTLV metadata: argv ptr, packed in/out counts, callback ptr.
                debug_mark(
                    49,
                    (*req).data.ioctlv.argv as usize as u32,
                    ((*req).data.ioctlv.argcin << 16) | ((*req).data.ioctlv.argcio & 0xffff),
                    (*req).cb.map_or(0, |cb| cb as usize as u32),
                );
                // 50: reply request trailer: usrdata, syncqueue, magic.
                debug_mark(
                    50,
                    (*req).usrdata as usize as u32,
                    (*req).syncqueue,
                    (*req).magic,
                );
            }
            if request_cmd == IOS_READ && !(*req).data.read.data.is_null() && (*req).result > 0 {
                dc_invalidate_range(
                    mem_physical_to_k0((*req).data.read.data as usize as u32) as *const c_void,
                    (*req).result as usize,
                );
            } else if request_cmd == IOS_IOCTL {
                if !(*req).data.ioctl.buffer_in.is_null() {
                    dc_invalidate_range(
                        mem_physical_to_k0((*req).data.ioctl.buffer_in as usize as u32)
                            as *const c_void,
                        (*req).data.ioctl.len_in as usize,
                    );
                }
                if !(*req).data.ioctl.buffer_io.is_null() {
                    dc_invalidate_range(
                        mem_physical_to_k0((*req).data.ioctl.buffer_io as usize as u32)
                            as *const c_void,
                        (*req).data.ioctl.len_io as usize,
                    );
                }
            } else if request_cmd == IOS_IOCTLV && !(*req).data.ioctlv.argv.is_null() {
                let total = ((*req).data.ioctlv.argcin + (*req).data.ioctlv.argcio) as usize;
                let argv_ptr =
                    mem_physical_to_k0((*req).data.ioctlv.argv as usize as u32) as *mut Ioctlv;
                dc_invalidate_range(
                    argv_ptr.cast_const().cast(),
                    total * core::mem::size_of::<Ioctlv>(),
                );
                let raw = core::slice::from_raw_parts_mut(argv_ptr, total);
                for (idx, entry) in raw.iter_mut().enumerate() {
                    if !entry.data.is_null() {
                        entry.data = mem_physical_to_k0(entry.data as usize as u32) as *mut c_void;
                        if entry.len > 0 {
                            dc_invalidate_range(entry.data.cast_const(), entry.len as usize);
                        }
                    }
                    // 51..58: per-entry reply trace: entry ptr, len, idx.
                    if idx < 8 {
                        debug_mark(
                            51 + idx as u32,
                            entry.data as usize as u32,
                            entry.len,
                            idx as u32,
                        );
                    }
                }
            }

            if (*req).cb.is_some() {
                // User callbacks must not execute in the PI/ACR interrupt handler.
                // The LWP deferred worker owns callback execution and request release.
                // SAFETY: the deferred worker owns this request until the callback
                // completes and releases it.
                let deferred = async_completion::enqueue_reply(
                    (*req).completion_reservation,
                    deferred_request_callback,
                    req.cast(),
                    |reservation, callback, argument| {
                        lwp_sys::deferred_call_reserved_from_interrupt(
                            reservation,
                            Some(callback),
                            argument,
                        )
                    },
                    lwp_sys::LWP_SUCCESSFUL,
                );
                if let Err(deferred) = deferred {
                    // Keep the completed request allocated if the reservation
                    // invariant is broken. Freeing it would create a dangling
                    // callback context and lose the accepted operation result.
                    debug_mark(60, req_ptr, deferred as u32, (*req).completion_reservation);
                }
            } else {
                if SYNC_WAIT_REQ.load(Ordering::Acquire) == req as usize {
                    SYNC_WAIT_DONE.store(true, Ordering::Release);
                }
                {
                    lwp_sys::note_ipc_wake((*req).syncqueue);
                    lwp_sys::thread_signal((*req).syncqueue);
                };
            }

            if RELAUNCH_REQ.compare_exchange(req as usize, 0, Ordering::AcqRel, Ordering::Acquire)
                == Ok(req as usize)
            {
                IPC_MAILBOX_ACK.store(1, Ordering::Release);
            }
        }

        let ipc_ack = (read_reg(1) & 0x30) | IPC_ACK_INTERRUPT_COMPLETE;
        write_reg(1, ipc_ack);
    }
}

unsafe fn ackhandler() {
    unsafe {
        let ipc_ack = (read_reg(1) & 0x30) | IPC_ACK_REQUEST_INTERRUPT;
        write_reg(1, ipc_ack);
        acr_write_reg(ACR_REGISTER_IRQ_CLEAR, ACR_IRQ_CLEAR_VALUE);
        IPC_MAILBOX_ACK.store(1, Ordering::Release);

        let relaunch = RELAUNCH_REQ.swap(0, Ordering::AcqRel);
        if relaunch != 0 {
            let req = relaunch as *mut IpcRequest;
            (*req).result = IPC_OK;
            if SYNC_WAIT_REQ.load(Ordering::Acquire) == relaunch {
                SYNC_WAIT_DONE.store(true, Ordering::Release);
                lwp_sys::note_ipc_wake((*req).syncqueue);
                lwp_sys::thread_signal((*req).syncqueue);
            }
        }
        send_next_request();
    }
}

#[cfg(target_arch = "powerpc")]
unsafe fn ppc_sync() {
    unsafe {
        core::arch::asm!("sync", options(nostack));
    }
}

#[cfg(not(target_arch = "powerpc"))]
unsafe fn ppc_sync() {}

#[cfg(target_arch = "powerpc")]
unsafe fn ppc_isync() {
    unsafe {
        core::arch::asm!("isync", options(nostack));
    }
}

#[cfg(not(target_arch = "powerpc"))]
unsafe fn ppc_isync() {}

unsafe fn dc_flush_range(startaddress: *const c_void, len: usize) {
    if startaddress.is_null() || len == 0 {
        return;
    }

    let mut addr = align_down(startaddress as usize, IPC_REQUEST_ALIGN);
    let end = (startaddress as usize)
        .checked_add(len)
        .and_then(|end| end.checked_next_multiple_of(IPC_REQUEST_ALIGN))
        .expect("cache range exceeds the address space");

    #[cfg(target_arch = "powerpc")]
    while addr < end {
        unsafe {
            core::arch::asm!("dcbf 0, {0}", in(reg) addr, options(nostack));
        }
        addr += IPC_REQUEST_ALIGN;
    }

    #[cfg(not(target_arch = "powerpc"))]
    while addr < end {
        addr += IPC_REQUEST_ALIGN;
    }

    unsafe {
        ppc_sync();
    }
}

unsafe fn dc_invalidate_range(startaddress: *const c_void, len: usize) {
    if startaddress.is_null() || len == 0 {
        return;
    }

    let mut addr = align_down(startaddress as usize, IPC_REQUEST_ALIGN);
    let end = (startaddress as usize)
        .checked_add(len)
        .and_then(|end| end.checked_next_multiple_of(IPC_REQUEST_ALIGN))
        .expect("cache range exceeds the address space");

    #[cfg(target_arch = "powerpc")]
    while addr < end {
        unsafe {
            core::arch::asm!("dcbi 0, {0}", in(reg) addr, options(nostack));
        }
        addr += IPC_REQUEST_ALIGN;
    }

    #[cfg(not(target_arch = "powerpc"))]
    while addr < end {
        addr += IPC_REQUEST_ALIGN;
    }

    unsafe {
        ppc_sync();
        ppc_isync();
    }
}

unsafe extern "C" {
    static __ipcbufferLo: u8;
    static __ipcbufferHi: u8;
}

fn initialize_ipc_buffer_locked() {
    if IPC_BUFFER_LO.load(Ordering::Acquire) != 0 {
        return;
    }
    let lo = core::ptr::addr_of!(__ipcbufferLo) as usize;
    let hi = core::ptr::addr_of!(__ipcbufferHi) as usize;
    IPC_BUFFER_LO.store(lo, Ordering::Release);
    IPC_BUFFER_HI.store(hi, Ordering::Release);
    IPC_BUFFER_CURRENT_LO.store(lo, Ordering::Release);
    IPC_BUFFER_CURRENT_HI.store(hi, Ordering::Release);
}

fn valid_buffer_lo(value: usize, base: usize) -> bool {
    value >= base
}

fn valid_buffer_hi(value: usize, limit: usize) -> bool {
    value <= limit
}

pub fn get_buffer_lo() -> *mut c_void {
    let _guard = HEAP_LOCK.lock_irq();
    initialize_ipc_buffer_locked();
    IPC_BUFFER_CURRENT_LO.load(Ordering::Acquire) as *mut c_void
}

pub fn get_buffer_hi() -> *mut c_void {
    let _guard = HEAP_LOCK.lock_irq();
    initialize_ipc_buffer_locked();
    IPC_BUFFER_CURRENT_HI.load(Ordering::Acquire) as *mut c_void
}

/// # Safety
/// `bufferlo` must point inside the linker-provided IPC buffer and must not
/// move the lower boundary below its original value.
pub unsafe fn set_buffer_lo(bufferlo: *mut c_void) {
    let _guard = HEAP_LOCK.lock_irq();
    initialize_ipc_buffer_locked();
    let value = bufferlo as usize;
    let base = IPC_BUFFER_LO.load(Ordering::Acquire);
    if valid_buffer_lo(value, base) {
        IPC_BUFFER_CURRENT_LO.store(value, Ordering::Release);
    }
}

/// # Safety
/// `bufferhi` must point inside the linker-provided IPC buffer and must not
/// move the upper boundary above its original value.
pub unsafe fn set_buffer_hi(bufferhi: *mut c_void) {
    let _guard = HEAP_LOCK.lock_irq();
    initialize_ipc_buffer_locked();
    let value = bufferhi as usize;
    let limit = IPC_BUFFER_HI.load(Ordering::Acquire);
    if valid_buffer_hi(value, limit) {
        IPC_BUFFER_CURRENT_HI.store(value, Ordering::Release);
    }
}

/// Return the current low address of the IOS IPC buffer.
#[unsafe(no_mangle)]
pub extern "C" fn IPC_GetBufferLo() -> *mut c_void {
    get_buffer_lo()
}

/// Return the current high address of the IOS IPC buffer.
#[unsafe(no_mangle)]
pub extern "C" fn IPC_GetBufferHi() -> *mut c_void {
    get_buffer_hi()
}

/// Set the current low address of the IOS IPC buffer.
///
/// # Safety
/// `bufferlo` must be at or above the linker-provided IPC buffer base.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn IPC_SetBufferLo(bufferlo: *mut c_void) {
    unsafe { set_buffer_lo(bufferlo) };
}

/// Set the current high address of the IOS IPC buffer.
///
/// # Safety
/// `bufferhi` must be at or below the linker-provided IPC buffer limit.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn IPC_SetBufferHi(bufferhi: *mut c_void) {
    unsafe { set_buffer_hi(bufferhi) };
}

fn create_heap_locked(size: usize) -> Result<i32, i32> {
    initialize_ipc_buffer_locked();
    if size < IPC_HEAP_HEADER + IPC_HEAP_ALIGN {
        return Err(IPC_ENOMEM);
    }

    let mut heap_index = None;
    // Heap creation runs with IPC serialized. Read through a slice built from
    // the raw static address so this scan does not retain a shared static-mut
    // reference beyond the protected operation.
    let heaps = unsafe {
        core::slice::from_raw_parts(
            core::ptr::addr_of!(IPC_HEAPS).cast::<HeapState>(),
            IPC_NUM_HEAPS,
        )
    };
    for (index, state) in heaps.iter().enumerate() {
        if state.base == 0 {
            heap_index = Some(index);
            break;
        }
    }
    let index = heap_index.ok_or(IPC_ENOHEAP)?;

    let lo = IPC_BUFFER_CURRENT_LO
        .load(Ordering::Acquire)
        .checked_next_multiple_of(IPC_HEAP_ALIGN)
        .ok_or(IPC_ENOMEM)?;
    let hi = IPC_BUFFER_CURRENT_HI.load(Ordering::Acquire);
    let end = lo.checked_add(size).ok_or(IPC_ENOMEM)?;
    if end > hi {
        return Err(IPC_ENOMEM);
    }

    let usable = align_down(size, IPC_HEAP_ALIGN);
    unsafe {
        IPC_HEAPS[index] = HeapState {
            base: lo,
            size: usable as u32,
            free_head: 0,
        };
        heap_write(index, 0, HeapBlock::new(usable as u32, IPC_HEAP_NULL));
    }
    IPC_BUFFER_CURRENT_LO.store(end, Ordering::Release);
    Ok(index as i32)
}

unsafe fn heap_header_ptr(heap: usize, offset: usize) -> *mut HeapBlock {
    let base = unsafe { IPC_HEAPS[heap].base };
    (base + offset) as *mut HeapBlock
}

unsafe fn heap_read(heap: usize, offset: usize) -> HeapBlock {
    unsafe { core::ptr::read(heap_header_ptr(heap, offset)) }
}

unsafe fn heap_write(heap: usize, offset: usize, block: HeapBlock) {
    unsafe { core::ptr::write(heap_header_ptr(heap, offset), block) };
}

unsafe fn heap_alloc(heap: usize, size: usize) -> Option<NonNull<c_void>> {
    let _guard = HEAP_LOCK.lock_irq();
    let state = unsafe { IPC_HEAPS[heap] };
    if state.base == 0 {
        return None;
    }
    let heap_size = state.size as usize;
    let needed = heap_block_size(size).filter(|need| *need <= heap_size)?;

    let mut prev = IPC_HEAP_NULL;
    let mut current = state.free_head;
    while current != IPC_HEAP_NULL {
        let block = unsafe { heap_read(heap, current as usize) };
        if block.size as usize >= needed {
            break;
        }
        prev = current;
        current = block.next;
    }
    if current == IPC_HEAP_NULL {
        return None;
    }

    let block = unsafe { heap_read(heap, current as usize) };
    let remainder = block.size as usize - needed;
    let allocation_offset = if remainder > IPC_HEAP_HEADER + IPC_HEAP_ALIGN {
        let offset = current as usize + remainder;
        unsafe {
            heap_write(
                heap,
                current as usize,
                HeapBlock::new(remainder as u32, block.next),
            );
            heap_write(heap, offset, HeapBlock::new(needed as u32, IPC_HEAP_NULL));
        }
        offset
    } else {
        unsafe {
            heap_write(
                heap,
                current as usize,
                HeapBlock::new(block.size, IPC_HEAP_NULL),
            )
        };
        // Remove a whole-block allocation from the free list.
        if prev == IPC_HEAP_NULL {
            unsafe { IPC_HEAPS[heap].free_head = block.next };
        } else {
            let mut prev_block = unsafe { heap_read(heap, prev as usize) };
            prev_block.next = block.next;
            unsafe { heap_write(heap, prev as usize, prev_block) };
        }
        current as usize
    };
    let user_start = state.base + allocation_offset + IPC_HEAP_HEADER;
    let alignment_offset = IPC_HEAP_ALIGN - (user_start & (IPC_HEAP_ALIGN - 1));
    let user_ptr = user_start + alignment_offset;
    unsafe {
        core::ptr::write(
            (user_ptr - core::mem::size_of::<u32>()) as *mut u32,
            alignment_offset as u32,
        );
    }
    NonNull::new(user_ptr as *mut c_void)
}

unsafe fn heap_free(heap: usize, ptr: NonNull<c_void>) {
    let _guard = HEAP_LOCK.lock_irq();
    let state = unsafe { IPC_HEAPS[heap] };
    if state.base == 0 {
        return;
    }
    let ptr_addr = ptr.as_ptr() as usize;
    let end = state.base + state.size as usize;
    if ptr_addr < state.base + IPC_HEAP_HEADER + core::mem::size_of::<u32>() || ptr_addr >= end {
        return;
    }
    let alignment_offset =
        unsafe { core::ptr::read((ptr_addr - core::mem::size_of::<u32>()) as *const u32) as usize };
    if alignment_offset == 0 || alignment_offset > IPC_HEAP_ALIGN {
        return;
    }
    let Some(block_off) = ptr_addr
        .checked_sub(state.base)
        .and_then(|relative| relative.checked_sub(alignment_offset))
        .and_then(|relative| relative.checked_sub(IPC_HEAP_HEADER))
    else {
        return;
    };
    let block = unsafe { heap_read(heap, block_off) };
    if block.size == 0 || block_off + block.size as usize > state.size as usize {
        return;
    }
    let mut previous_free = IPC_HEAP_NULL;
    let mut next_free = IPC_HEAP_NULL;
    let mut next_free_predecessor = IPC_HEAP_NULL;
    let mut list_predecessor = IPC_HEAP_NULL;
    let mut current = state.free_head;
    while current != IPC_HEAP_NULL {
        if current as usize == block_off {
            return;
        } // already free
        let free = unsafe { heap_read(heap, current as usize) };
        if current as usize + free.size as usize == block_off {
            previous_free = current;
        }
        if block_off + block.size as usize == current as usize {
            next_free = current;
            next_free_predecessor = list_predecessor;
        }
        list_predecessor = current;
        current = free.next;
    }

    if previous_free != IPC_HEAP_NULL {
        let mut combined_size = block.size;
        if next_free != IPC_HEAP_NULL {
            let next = unsafe { heap_read(heap, next_free as usize) };
            combined_size = combined_size.saturating_add(next.size);
            // Memory neighbors need not be neighbors in the free list. Remove
            // the absorbed node from its actual position before rereading the
            // previous memory neighbor, which can also be its list predecessor.
            if next_free_predecessor == IPC_HEAP_NULL {
                unsafe { IPC_HEAPS[heap].free_head = next.next };
            } else {
                let mut before = unsafe { heap_read(heap, next_free_predecessor as usize) };
                before.next = next.next;
                unsafe { heap_write(heap, next_free_predecessor as usize, before) };
            }
            unsafe {
                heap_write(heap, next_free as usize, HeapBlock::new(0, IPC_HEAP_NULL));
            }
        }
        let mut previous = unsafe { heap_read(heap, previous_free as usize) };
        previous.size = previous.size.saturating_add(combined_size);
        unsafe {
            heap_write(heap, previous_free as usize, previous);
        }
    } else if next_free != IPC_HEAP_NULL {
        // Replace the next free node in place, preserving C's free-list order.
        let next = unsafe { heap_read(heap, next_free as usize) };
        let merged = HeapBlock::new(block.size.saturating_add(next.size), next.next);
        unsafe {
            heap_write(heap, block_off, merged);
        }
        if next_free_predecessor == IPC_HEAP_NULL {
            unsafe {
                IPC_HEAPS[heap].free_head = block_off as u32;
            }
        } else {
            let mut before = unsafe { heap_read(heap, next_free_predecessor as usize) };
            before.next = block_off as u32;
            unsafe {
                heap_write(heap, next_free_predecessor as usize, before);
            }
        }
    } else {
        unsafe {
            heap_write(heap, block_off, HeapBlock::new(block.size, state.free_head));
            IPC_HEAPS[heap].free_head = block_off as u32;
        }
    }
}

unsafe fn submit_request<F: FnOnce()>(req: &mut IpcRequest, after_reply: F) -> i32 {
    unsafe { submit_request_result(req, after_reply) }.unwrap_or_else(|error| error)
}

unsafe fn submit_request_result<F>(req: &mut IpcRequest, after_reply: F) -> Result<i32, i32>
where
    F: FnOnce(),
{
    unsafe {
        let _sync_guard = lock_sync_wait()?;
        {
            let _guard = IPC_LOCK.lock_irq();
            ensure_client_initialized();
        }

        let mut syncqueue = lwp_sys::LWP_TQUEUE_NULL;
        if lwp_sys::init_queue(&mut syncqueue) != lwp_sys::LWP_SUCCESSFUL {
            return Err(IPC_ENOMEM);
        }

        debug_mark(0, syncqueue, 0, 0);
        let mut irq_state = lock_by_msr();
        debug_mark(1, req.cmd, req.fd as u32, req.magic);
        req.syncqueue = syncqueue;
        SYNC_WAIT_REQ.store(req as *mut IpcRequest as usize, Ordering::Release);
        SYNC_WAIT_DONE.store(false, Ordering::Release);
        let queued = queue_request(req);
        if queued == IPC_OK {
            send_next_request();
            unlock_by_msr(irq_state);

            // Service replies directly until PI/ACR interrupt delivery is proven
            // reliable on hardware. Sleeping here can otherwise lose the only
            // wakeup and permanently stall every synchronous IOS request.
            while !SYNC_WAIT_DONE.load(Ordering::Acquire) {
                poll();
                lwp_sys::yield_thread();
            }
            irq_state = lock_by_msr();
        }
        let result = raw_contract::submission_result(queued, req.result);
        SYNC_WAIT_REQ.store(0, Ordering::Release);
        SYNC_WAIT_DONE.store(false, Ordering::Release);
        req.syncqueue = lwp_sys::LWP_TQUEUE_NULL;
        unlock_by_msr(irq_state);
        lwp_sys::close_queue(syncqueue);
        if result.is_ok() {
            after_reply();
        }
        debug_mark(8, req.cmd, req.fd as u32, req.result as u32);

        result
    }
}

unsafe fn submit_async_request(req: &mut IpcRequest) -> i32 {
    unsafe {
        // Initialize LWP in caller context before an IOS interrupt can enqueue the
        // completion onto the deferred worker.
        let _ = lwp_sys::get_self();

        let reserves_callback = req.cb.is_some();
        if reserves_callback {
            let reserved = ASYNC_CALLBACKS_PENDING.try_update(
                Ordering::AcqRel,
                Ordering::Acquire,
                |pending| (pending < IPC_DEFERRED_CALLBACK_CAPACITY).then_some(pending + 1),
            );
            if reserved.is_err() {
                return IPC_EQUEUEFULL;
            }
            let Some(reservation) = lwp_sys::deferred_reserve_interrupt_slot() else {
                ASYNC_CALLBACKS_PENDING.fetch_sub(1, Ordering::AcqRel);
                return IPC_EQUEUEFULL;
            };
            req.completion_reservation = reservation;
        }

        let _guard = IPC_LOCK.lock_irq();
        ensure_client_initialized();

        debug_mark(10, req.cmd, req.fd as u32, req.magic);
        let irq_state = lock_by_msr();
        let result = queue_request(req);
        if result == IPC_OK {
            send_next_request();
        } else if reserves_callback {
            let cancelled = lwp_sys::deferred_cancel_interrupt_slot(req.completion_reservation);
            if cancelled != lwp_sys::LWP_SUCCESSFUL {
                debug_mark(62, req.completion_reservation, cancelled as u32, req.magic);
            }
            ASYNC_CALLBACKS_PENDING.fetch_sub(1, Ordering::AcqRel);
        }
        unlock_by_msr(irq_state);
        debug_mark(11, req.cmd, req.fd as u32, req.magic);

        result
    }
}

fn open_request(path: &CStr, mode: u32) -> Result<FileDescriptor, i32> {
    validate_path(path)?;
    let path_bytes = path.to_bytes_with_nul();
    let mut aligned_path = AlignedPath([0; IPC_MAXPATH_LEN as usize + 1]);
    aligned_path.0[..path_bytes.len()].copy_from_slice(path_bytes);

    unsafe {
        dc_flush_range(aligned_path.0.as_ptr().cast(), path_bytes.len());
    }

    let rc = with_request(|req| {
        req.cmd = IOS_OPEN;
        req.data.open.filepath =
            mem_virtual_to_physical(aligned_path.0.as_ptr() as usize) as *mut c_void;
        req.data.open.mode = mode;
        unsafe { submit_request(req, || {}) }
    })?;
    if rc < 0 {
        Err(rc)
    } else {
        Ok(FileDescriptor::new(rc))
    }
}

fn ioctlv_common<'a, const N: usize>(
    fd: i32,
    ioctl: i32,
    cnt_in: usize,
    argv: [IoctlvEntry<'a>; N],
    relnch: u32,
    result: i32,
) -> i32 {
    ioctlv_common_result(fd, ioctl, cnt_in, argv, relnch, result).unwrap_or_else(|error| error)
}

fn ioctlv_common_result<'a, const N: usize>(
    fd: i32,
    ioctl: i32,
    cnt_in: usize,
    mut argv: [IoctlvEntry<'a>; N],
    relnch: u32,
    result: i32,
) -> Result<i32, i32> {
    let Some(cnt_in_u32) = s32_to_u32(cnt_in as i32) else {
        return Err(IPC_EINVAL);
    };
    let Some(cnt_io_u32) = s32_to_u32((N - cnt_in) as i32) else {
        return Err(IPC_EINVAL);
    };

    let mut raw = AlignedIoctlv([Ioctlv::default(); N]);
    let mut metas = [IoctlvMeta::default(); N];
    // IOS writes RAM while the CPU can modify other data in the same cache
    // line. Give partial or unaligned vectors separate, aligned heap storage.
    let mut staged: [Option<DmaStaging>; N] = core::array::from_fn(|_| None);
    for (index, ((slot, meta), entry)) in raw
        .0
        .iter_mut()
        .zip(metas.iter_mut())
        .zip(argv.iter_mut())
        .enumerate()
    {
        let (data, len) = entry.as_mut_raw();
        staged[index] = DmaStaging::if_needed(data, len as usize)?;
        let transfer = staged[index]
            .as_ref()
            .map_or(data, |buffer| buffer.pointer.as_ptr());
        *slot = Ioctlv {
            data: mem_virtual_to_physical(transfer as usize) as *mut c_void,
            len: len as u32,
        };
        *meta = IoctlvMeta {
            data: transfer,
            len: len as usize,
        };
    }
    let raw_len = core::mem::size_of_val(&raw.0);

    with_request(|req| {
        req.cmd = IOS_IOCTLV;
        req.fd = fd;
        req.result = result;
        req.relnch = relnch;
        req.data.ioctlv.ioctl = ioctl as u32;
        req.data.ioctlv.argcin = cnt_in_u32;
        req.data.ioctlv.argcio = cnt_io_u32;
        req.data.ioctlv.argv = mem_virtual_to_physical(raw.0.as_mut_ptr() as usize) as *mut Ioctlv;

        unsafe {
            dc_flush_range(raw.0.as_ptr().cast(), raw_len);
            for meta in &metas {
                if !meta.data.is_null() && meta.len > 0 {
                    dc_flush_range(meta.data.cast_const(), meta.len);
                }
            }

            submit_request_result(req, || {
                dc_invalidate_range(raw.0.as_ptr().cast(), raw_len);
                // Input vectors are read-only. Invalidating them would also
                // discard any stack updates made while waiting for the reply.
                for (index, meta) in metas.iter().enumerate().skip(cnt_in) {
                    if !meta.data.is_null() && meta.len > 0 {
                        dc_invalidate_range(meta.data.cast_const(), meta.len);
                        if staged[index].is_some() {
                            let (destination, _) = argv[index].as_mut_raw();
                            core::ptr::copy_nonoverlapping(
                                meta.data.cast::<u8>(),
                                destination.cast::<u8>(),
                                meta.len,
                            );
                        }
                    }
                }
            })
        }
    })?
}

const IOS_MAXFMT_PARAMS: usize = 32;
const FORMAT_OWNER_HEAP: u8 = 1;
const FORMAT_OWNER_WORKSPACE: u8 = 2;

#[repr(C)]
struct FormatCall {
    h_id: i32,
    fd: i32,
    ioctl: i32,
    user_cb: Ipccallback,
    user_data: *mut c_void,
    format: *const u8,
    reg_words: [u32; 4],
    reg_count: u32,
    overflow: *const u32,
    asynchronous: u32,
}

const _: () = {
    assert!(core::mem::size_of::<FormatCall>() == 52);
    assert!(core::mem::offset_of!(FormatCall, h_id) == 0);
    assert!(core::mem::offset_of!(FormatCall, fd) == 4);
    assert!(core::mem::offset_of!(FormatCall, ioctl) == 8);
    assert!(core::mem::offset_of!(FormatCall, user_cb) == 12);
    assert!(core::mem::offset_of!(FormatCall, user_data) == 16);
    assert!(core::mem::offset_of!(FormatCall, format) == 20);
    assert!(core::mem::offset_of!(FormatCall, reg_words) == 24);
    assert!(core::mem::offset_of!(FormatCall, reg_count) == 40);
    assert!(core::mem::offset_of!(FormatCall, overflow) == 44);
    assert!(core::mem::offset_of!(FormatCall, asynchronous) == 48);
};

#[derive(Copy, Clone)]
struct FormatOwned {
    ipc_buf: *mut c_void,
    io_buf: *mut c_void,
    copy_len: usize,
    allocation_len: usize,
    owner: u8,
}

impl FormatOwned {
    const EMPTY: Self = Self {
        ipc_buf: core::ptr::null_mut(),
        io_buf: core::ptr::null_mut(),
        copy_len: 0,
        allocation_len: 0,
        owner: 0,
    };
}

struct FormatState {
    heap: HeapId,
    user_cb: Ipccallback,
    user_data: *mut c_void,
    num_owned: usize,
    owned: [FormatOwned; IOS_MAXFMT_PARAMS + 1],
}

struct VarWords {
    regs: [u32; 4],
    index: usize,
    count: usize,
    overflow: *const u32,
}

impl VarWords {
    unsafe fn next_u32(&mut self) -> u32 {
        if self.index < self.count {
            let value = self.regs[self.index];
            self.index += 1;
            value
        } else {
            let value = unsafe { self.overflow.read() };
            self.overflow = unsafe { self.overflow.add(1) };
            value
        }
    }

    unsafe fn next_u64(&mut self) -> u64 {
        // r7 for IOS_IoctlvFormat and r9 for IOS_IoctlvFormatAsync both start
        // on an even ABI argument-word slot. A 64-bit variadic value starts on
        // the next even slot, so one GPR can be skipped after a 32-bit value.
        let aligned_index = (self.index + 1) & !1usize;
        if aligned_index + 2 <= self.count {
            let hi = self.regs[aligned_index] as u64;
            let lo = self.regs[aligned_index + 1] as u64;
            self.index = aligned_index + 2;
            return (hi << 32) | lo;
        }

        // The 32-bit PowerPC EABI does not split a 64-bit variadic argument
        // across the last GPR and the overflow area. It places the complete
        // value at the next 8-byte-aligned stack slot.
        self.index = self.count;
        self.overflow = ((self.overflow as usize + 7) & !7usize) as *const u32;
        let hi = unsafe { self.overflow.read() } as u64;
        let lo = unsafe { self.overflow.add(1).read() } as u64;
        self.overflow = unsafe { self.overflow.add(2) };
        (hi << 32) | lo
    }

    unsafe fn next_ptr(&mut self) -> *mut c_void {
        unsafe { self.next_u32() as usize as *mut c_void }
    }
}

unsafe fn format_state_new(
    heap: HeapId,
    user_cb: Ipccallback,
    user_data: *mut c_void,
) -> Result<NonNull<FormatState>, i32> {
    let bytes = core::mem::size_of::<FormatState>();
    let raw = unsafe { lwp_sys::allocate_workspace(bytes) }.cast::<FormatState>();
    let Some(state) = NonNull::new(raw) else {
        return Err(IPC_ENOMEM);
    };
    unsafe {
        state.as_ptr().write(FormatState {
            heap,
            user_cb,
            user_data,
            num_owned: 0,
            owned: [FormatOwned::EMPTY; IOS_MAXFMT_PARAMS + 1],
        });
    }
    Ok(state)
}

unsafe fn format_record(state: &mut FormatState, entry: FormatOwned) -> Result<(), i32> {
    if state.num_owned >= state.owned.len() {
        return Err(IPC_EINVAL);
    }
    state.owned[state.num_owned] = entry;
    state.num_owned += 1;
    Ok(())
}

unsafe fn format_release(
    result: i32,
    state_ptr: *mut FormatState,
    copy_outputs: bool,
    invoke_callback: bool,
) -> i32 {
    if state_ptr.is_null() {
        return IPC_EINVAL;
    }
    let state = unsafe { &mut *state_ptr };
    for entry in state.owned[..state.num_owned].iter().copied() {
        if !entry.ipc_buf.is_null() {
            if copy_outputs && !entry.io_buf.is_null() && entry.copy_len != 0 {
                unsafe {
                    core::ptr::copy_nonoverlapping(
                        entry.ipc_buf.cast::<u8>(),
                        entry.io_buf.cast::<u8>(),
                        entry.copy_len,
                    );
                }
            }
            match entry.owner {
                FORMAT_OWNER_HEAP => {
                    if let Some(ptr) = NonNull::new(entry.ipc_buf) {
                        unsafe { state.heap.free(ptr) };
                    }
                }
                FORMAT_OWNER_WORKSPACE => unsafe {
                    lwp_sys::release_workspace(entry.ipc_buf.cast(), entry.allocation_len);
                },
                _ => {}
            }
        }
    }
    let user_cb = state.user_cb;
    let user_data = state.user_data;
    unsafe {
        lwp_sys::release_workspace(state_ptr.cast(), core::mem::size_of::<FormatState>());
    }
    if invoke_callback && let Some(callback) = user_cb {
        return unsafe { callback(result, user_data) };
    }
    result
}

unsafe extern "C" fn ioctlv_format_callback(result: i32, userdata: *mut c_void) -> i32 {
    unsafe { format_release(result, userdata.cast::<FormatState>(), true, true) }
}

unsafe fn format_heap_alloc(
    state: &mut FormatState,
    size: usize,
    io_buf: *mut c_void,
    copy_len: usize,
) -> Result<*mut c_void, i32> {
    let size_i32 = i32::try_from(size).map_err(|_| IPC_EINVAL)?;
    let ptr = state.heap.alloc(size_i32).ok_or(IPC_ENOMEM)?.as_ptr();
    if let Err(error) = unsafe {
        format_record(
            state,
            FormatOwned {
                ipc_buf: ptr,
                io_buf,
                copy_len,
                allocation_len: size,
                owner: FORMAT_OWNER_HEAP,
            },
        )
    } {
        if let Some(ptr) = NonNull::new(ptr) {
            unsafe { state.heap.free(ptr) };
        }
        return Err(error);
    }
    Ok(ptr)
}

unsafe fn format_vector_data(
    state: &mut FormatState,
    data: *mut c_void,
    len: usize,
    output: bool,
) -> Result<*mut c_void, i32> {
    if data.is_null() || len == 0 || !dma_buffer::needs_staging(data as usize, len) {
        return Ok(data);
    }
    let raw = unsafe { lwp_sys::allocate_workspace(len) }.cast::<c_void>();
    let Some(staged) = NonNull::new(raw) else {
        return Err(IPC_ENOMEM);
    };
    unsafe { core::ptr::copy_nonoverlapping(data.cast::<u8>(), staged.as_ptr().cast::<u8>(), len) };
    if let Err(error) = unsafe {
        format_record(
            state,
            FormatOwned {
                ipc_buf: staged.as_ptr(),
                io_buf: if output { data } else { core::ptr::null_mut() },
                copy_len: if output { len } else { 0 },
                allocation_len: len,
                owner: FORMAT_OWNER_WORKSPACE,
            },
        )
    } {
        unsafe { lwp_sys::release_workspace(staged.as_ptr().cast(), len) };
        return Err(error);
    }
    Ok(staged.as_ptr())
}

unsafe fn submit_format_request(
    fd: i32,
    ioctl: i32,
    cnt_in: usize,
    cnt_io: usize,
    argv: *mut Ioctlv,
    asynchronous: bool,
    state: *mut FormatState,
) -> i32 {
    let Some(total) = cnt_in.checked_add(cnt_io) else {
        return IPC_EINVAL;
    };
    if cnt_in > i32::MAX as usize || cnt_io > i32::MAX as usize || argv.is_null() {
        return IPC_EINVAL;
    }
    let raw_len = match total.checked_mul(core::mem::size_of::<Ioctlv>()) {
        Some(value) => value,
        None => return IPC_EINVAL,
    };
    let raw = unsafe { core::slice::from_raw_parts_mut(argv, total) };
    for entry in raw.iter_mut() {
        if !entry.data.is_null() && entry.len > 0 {
            unsafe { dc_flush_range(entry.data.cast_const(), entry.len as usize) };
            entry.data = mem_virtual_to_physical(entry.data as usize) as *mut c_void;
        }
    }
    if raw_len > 0 {
        unsafe { dc_flush_range(argv.cast_const().cast(), raw_len) };
    }

    if asynchronous {
        let Ok(req_ptr) = alloc_request() else {
            return IPC_ENOMEM;
        };
        let req = unsafe { &mut *req_ptr.as_ptr() };
        *req = IpcRequest::zeroed();
        req.cmd = IOS_IOCTLV;
        req.fd = fd;
        req.cb = Some(ioctlv_format_callback);
        req.usrdata = state.cast();
        req.data.ioctlv.ioctl = ioctl as u32;
        req.data.ioctlv.argcin = cnt_in as u32;
        req.data.ioctlv.argcio = cnt_io as u32;
        req.data.ioctlv.argv = mem_virtual_to_physical(argv as usize) as *mut Ioctlv;
        let rc = unsafe { submit_async_request(req) };
        if rc < 0 {
            unsafe { free_request(req_ptr) };
        }
        rc
    } else {
        match with_request(|req| {
            req.cmd = IOS_IOCTLV;
            req.fd = fd;
            req.data.ioctlv.ioctl = ioctl as u32;
            req.data.ioctlv.argcin = cnt_in as u32;
            req.data.ioctlv.argcio = cnt_io as u32;
            req.data.ioctlv.argv = mem_virtual_to_physical(argv as usize) as *mut Ioctlv;
            unsafe { submit_request(req, || {}) }
        }) {
            Ok(rc) | Err(rc) => rc,
        }
    }
}

unsafe fn parse_format_call(
    call: &FormatCall,
) -> Result<(*mut FormatState, usize, usize, *mut Ioctlv), i32> {
    if call.format.is_null() {
        return Err(IPC_EINVAL);
    }
    let mut len = 0usize;
    while len < IOS_MAXFMT_PARAMS && unsafe { call.format.add(len).read() } != 0 {
        len += 1;
    }
    if len >= IOS_MAXFMT_PARAMS {
        return Err(IPC_EINVAL);
    }
    let heap = if call.h_id == IPC_HEAP {
        request_heap()?
    } else {
        HeapId::new(call.h_id)
    };
    if heap.raw() < 0 || heap.raw() as usize >= IPC_NUM_HEAPS {
        return Err(IPC_EINVAL);
    }
    let state_ptr = unsafe { format_state_new(heap, call.user_cb, call.user_data)? };
    let state = unsafe { &mut *state_ptr.as_ptr() };
    let argv_bytes = (len + 1)
        .checked_mul(core::mem::size_of::<Ioctlv>())
        .ok_or(IPC_EINVAL)?;
    let argv = match unsafe { format_heap_alloc(state, argv_bytes, core::ptr::null_mut(), 0) } {
        Ok(ptr) => ptr.cast::<Ioctlv>(),
        Err(error) => {
            unsafe { format_release(error, state_ptr.as_ptr(), false, false) };
            return Err(error);
        }
    };
    unsafe { core::ptr::write_bytes(argv, 0, len + 1) };
    let mut args = VarWords {
        regs: call.reg_words,
        index: 0,
        count: (call.reg_count as usize).min(4),
        overflow: call.overflow,
    };
    let mut cnt_in = 0usize;
    let mut cnt_io = 0usize;
    let mut output = false;
    let mut vector = 0usize;
    let result = (|| -> Result<(), i32> {
        for pos in 0..len {
            let mut kind = unsafe { call.format.add(pos).read() };
            if kind == b':' {
                if output {
                    return Err(IPC_EINVAL);
                }
                output = true;
                continue;
            }
            kind = kind.to_ascii_lowercase();
            let slot = unsafe { &mut *argv.add(vector) };
            match kind {
                b'b' | b'h' | b'i' | b'q' => {
                    let size = match kind {
                        b'b' => 1,
                        b'h' => 2,
                        b'i' => 4,
                        _ => 8,
                    };
                    if output {
                        let io = unsafe { args.next_ptr() };
                        if io.is_null() {
                            return Err(IPC_EINVAL);
                        }
                        let ptr = unsafe { format_heap_alloc(state, size, io, size)? };
                        unsafe {
                            core::ptr::copy_nonoverlapping(io.cast::<u8>(), ptr.cast::<u8>(), size)
                        };
                        *slot = Ioctlv {
                            data: ptr,
                            len: size as u32,
                        };
                        cnt_io += 1;
                    } else {
                        let ptr =
                            unsafe { format_heap_alloc(state, size, core::ptr::null_mut(), 0)? };
                        match size {
                            1 => unsafe { ptr.cast::<u8>().write(args.next_u32() as u8) },
                            2 => unsafe { ptr.cast::<u16>().write(args.next_u32() as u16) },
                            4 => unsafe { ptr.cast::<u32>().write(args.next_u32()) },
                            _ => unsafe { ptr.cast::<u64>().write(args.next_u64()) },
                        }
                        *slot = Ioctlv {
                            data: ptr,
                            len: size as u32,
                        };
                        cnt_in += 1;
                    }
                }
                b'd' => {
                    let data = unsafe { args.next_ptr() };
                    let data_len = unsafe { args.next_u32() } as usize;
                    let transfer = unsafe { format_vector_data(state, data, data_len, output)? };
                    *slot = Ioctlv {
                        data: transfer,
                        len: data_len as u32,
                    };
                    if output {
                        cnt_io += 1;
                    } else {
                        cnt_in += 1;
                    }
                }
                b's' if !output => {
                    let source = unsafe { args.next_ptr() }.cast::<u8>();
                    if source.is_null() {
                        return Err(IPC_EINVAL);
                    }
                    let mut slen = 0usize;
                    while slen < 256 && unsafe { source.add(slen).read() } != 0 {
                        slen += 1;
                    }
                    if slen >= 256 {
                        return Err(IPC_EINVAL);
                    }
                    let size = slen + 1;
                    let ptr = unsafe { format_heap_alloc(state, size, core::ptr::null_mut(), 0)? };
                    unsafe { core::ptr::copy_nonoverlapping(source, ptr.cast::<u8>(), size) };
                    *slot = Ioctlv {
                        data: ptr,
                        len: size as u32,
                    };
                    cnt_in += 1;
                }
                _ => return Err(IPC_EINVAL),
            }
            vector += 1;
        }
        Ok(())
    })();
    if let Err(error) = result {
        unsafe { format_release(error, state_ptr.as_ptr(), false, false) };
        return Err(error);
    }
    Ok((state_ptr.as_ptr(), cnt_in, cnt_io, argv))
}

/// Internal target-ABI bridge used by the two variadic assembly entry points.
///
/// # Safety
/// `call` must point to a complete [`FormatCall`] record built by one of the
/// assembly shims above.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __ogc_ipc_ioctlv_format_bridge(call: *const c_void) -> i32 {
    if call.is_null() {
        return IPC_EINVAL;
    }
    let call = unsafe { &*call.cast::<FormatCall>() };
    let (state, cnt_in, cnt_io, argv) = match unsafe { parse_format_call(call) } {
        Ok(parsed) => parsed,
        Err(error) => return error,
    };
    let rc = unsafe {
        submit_format_request(
            call.fd,
            call.ioctl,
            cnt_in,
            cnt_io,
            argv,
            call.asynchronous != 0,
            state,
        )
    };
    if call.asynchronous == 0 || rc < 0 {
        unsafe { format_release(rc, state, call.asynchronous == 0, false) }
    } else {
        rc
    }
}

struct DmaStaging {
    pointer: NonNull<c_void>,
    length: usize,
}

impl DmaStaging {
    fn if_needed(source: *mut c_void, length: usize) -> Result<Option<Self>, i32> {
        if source.is_null() || !dma_buffer::needs_staging(source as usize, length) {
            return Ok(None);
        }
        Self::new(source, length).map(Some)
    }

    fn new(source: *mut c_void, length: usize) -> Result<Self, i32> {
        let allocation_length = dma_buffer::staging_length(length).ok_or(IPC_EINVAL)?;
        let allocation = unsafe { lwp_sys::allocate_workspace(allocation_length) }.cast::<c_void>();
        let pointer = NonNull::new(allocation).ok_or(IPC_ENOMEM)?;
        // Reserve the whole cache line so DMA invalidation cannot discard an
        // adjacent allocation header while this request waits for IOS.
        unsafe {
            core::ptr::write_bytes(pointer.as_ptr().cast::<u8>(), 0, allocation_length);
            core::ptr::copy_nonoverlapping(
                source.cast::<u8>(),
                pointer.as_ptr().cast::<u8>(),
                length,
            );
        }
        Ok(Self {
            pointer,
            length: allocation_length,
        })
    }
}

impl Drop for DmaStaging {
    fn drop(&mut self) {
        unsafe { lwp_sys::release_workspace(self.pointer.as_ptr().cast(), self.length) };
    }
}

pub fn validate_path(path: &CStr) -> Result<(), i32> {
    if path.to_bytes_with_nul().len() > IPC_MAXPATH_LEN as usize {
        Err(IPC_EINVAL)
    } else {
        Ok(())
    }
}

pub fn open_file(path: &CStr, mode: u32) -> Result<FileDescriptor, i32> {
    open_request(path, mode)
}

pub fn open_file_async(path: &CStr, mode: u32, cb: Ipccallback, userdata: *mut c_void) -> i32 {
    if cb.is_none() {
        return match open_file(path, mode) {
            Ok(fd) => fd.raw(),
            Err(rc) => rc,
        };
    }

    if let Err(rc) = validate_path(path) {
        return rc;
    }

    let heap = match request_heap() {
        Ok(heap) => heap,
        Err(rc) => return rc,
    };

    let msg_ptr = match heap.alloc(core::mem::size_of::<OpenAsyncMessage>() as i32) {
        Some(ptr) => ptr.as_ptr().cast::<OpenAsyncMessage>(),
        None => return IPC_ENOMEM,
    };

    let path_bytes = path.to_bytes_with_nul();
    unsafe {
        core::ptr::write_bytes(
            msg_ptr.cast::<u8>(),
            0,
            core::mem::size_of::<OpenAsyncMessage>(),
        );
        (&mut (*msg_ptr).path.0)[..path_bytes.len()].copy_from_slice(path_bytes);
        (*msg_ptr).cb = cb;
        (*msg_ptr).userdata = userdata;
        dc_flush_range((*msg_ptr).path.0.as_ptr().cast(), path_bytes.len());
    }

    let req_ptr = match alloc_request() {
        Ok(ptr) => ptr,
        Err(rc) => {
            unsafe {
                free_heap_message(msg_ptr);
            }
            return rc;
        }
    };

    let req = unsafe { &mut *req_ptr.as_ptr() };
    *req = IpcRequest::zeroed();
    req.cmd = IOS_OPEN;
    req.cb = Some(open_async_cb);
    req.usrdata = msg_ptr.cast();
    req.data.open.filepath =
        unsafe { mem_virtual_to_physical((*msg_ptr).path.0.as_ptr() as usize) as *mut c_void };
    req.data.open.mode = mode;

    let rc = unsafe { submit_async_request(req) };
    if rc < 0 {
        unsafe {
            free_request(req_ptr);
            free_heap_message(msg_ptr);
        }
    }
    rc
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;
    use core::ffi::CStr;

    static TEST_HEAP_LOCK: SpinLock = SpinLock::new();
    const TEST_HEAP_INDEX: usize = IPC_NUM_HEAPS - 1;

    #[repr(align(32))]
    struct TestArena([u8; 2048]);

    struct TestHeapGuard {
        _lock: SpinLockGuard<'static>,
        old: HeapState,
    }

    impl TestHeapGuard {
        fn install(arena: &mut TestArena) -> Self {
            let lock = TEST_HEAP_LOCK.lock_irq();
            let old = unsafe { IPC_HEAPS[TEST_HEAP_INDEX] };
            unsafe {
                IPC_HEAPS[TEST_HEAP_INDEX] = HeapState {
                    base: arena.0.as_mut_ptr() as usize,
                    size: arena.0.len() as u32,
                    free_head: 0,
                };
                heap_write(
                    TEST_HEAP_INDEX,
                    0,
                    HeapBlock::new(arena.0.len() as u32, IPC_HEAP_NULL),
                );
            }
            Self { _lock: lock, old }
        }
    }

    impl Drop for TestHeapGuard {
        fn drop(&mut self) {
            unsafe {
                IPC_HEAPS[TEST_HEAP_INDEX] = self.old;
            }
        }
    }

    #[test]
    fn variadic_word_cursor_matches_powerpc_eabi_alignment() {
        #[repr(align(8))]
        struct StackWords([u32; 4]);

        let stack = StackWords([0x1111_1111, 0xaaaa_aaaa, 0x2222_2222, 0x3333_3333]);
        let mut regs = VarWords {
            regs: [0x0101_0101, 0xdead_beef, 0x0202_0202, 0x0303_0303],
            index: 1,
            count: 4,
            overflow: stack.0.as_ptr(),
        };
        assert_eq!(unsafe { regs.next_u64() }, 0x0202_0202_0303_0303);

        let mut overflow = VarWords {
            regs: [1, 2, 3, 4],
            index: 4,
            count: 4,
            overflow: stack.0.as_ptr(),
        };
        assert_eq!(unsafe { overflow.next_u32() }, 0x1111_1111);
        assert_eq!(unsafe { overflow.next_u64() }, 0x2222_2222_3333_3333);
    }

    #[test]
    fn public_layout_matches_c() {
        let payload = core::mem::size_of::<*mut c_void>() + 4;
        let align = core::mem::align_of::<Ioctlv>();
        let expected = payload.next_multiple_of(align);
        assert_eq!(core::mem::size_of::<Ioctlv>(), expected);
        assert_eq!(
            core::mem::align_of::<Ioctlv>(),
            core::mem::align_of::<*mut c_void>()
        );
        assert_eq!(core::mem::size_of::<IpcRequest>(), 64);
    }

    #[test]
    fn typed_helpers_exist_without_backend() {
        let heap = HeapId::new(7);
        assert_eq!(heap.raw(), 7);
        let fd = FileDescriptor::new(9);
        assert_eq!(fd.raw(), 9);
        assert_eq!(IoctlBuffers::empty(), IoctlBuffers::from_slices(None, None));

        let mut input = [1u8, 2, 3];
        let mut io = [4u8, 5];
        let buffers = IoctlBuffers::from_slices(Some(&mut input), Some(&mut io));
        assert_eq!(buffers.len_in(), 3);
        assert_eq!(buffers.len_io(), 2);
        let mut out = [0u8; 2];
        let _ = IoctlvEntry::In(&input);
        let _ = IoctlvEntry::Out(&mut out);
        assert_eq!(
            IoctlvEntry::OutNull.as_mut_raw(),
            (core::ptr::null_mut(), 0)
        );

        let _ = HeapId::create as fn(i32) -> Result<HeapId, i32>;
        let _ = HeapId::alloc as unsafe fn(HeapId, i32) -> Option<core::ptr::NonNull<c_void>>;
        let _ = HeapId::free as unsafe fn(HeapId, core::ptr::NonNull<c_void>);
        let _ = FileDescriptor::close as fn(FileDescriptor) -> Result<(), i32>;
        let _ = FileDescriptor::seek as fn(&FileDescriptor, i32, i32) -> Result<i32, i32>;
        let _ = FileDescriptor::read as fn(&FileDescriptor, &mut [u8]) -> Result<usize, i32>;
        let _ = FileDescriptor::write as fn(&FileDescriptor, &[u8]) -> Result<usize, i32>;
        let _ = FileDescriptor::read_exact as fn(&FileDescriptor, &mut [u8]) -> Result<(), i32>;
        let _ = FileDescriptor::write_exact as fn(&FileDescriptor, &[u8]) -> Result<(), i32>;
        let _ = FileDescriptor::ioctl as fn(&FileDescriptor, i32, IoctlBuffers<'_>) -> i32;
        let _ = FileDescriptor::ioctl_async
            as unsafe fn(&FileDescriptor, i32, IoctlBuffers<'_>, Ipccallback, *mut c_void) -> i32;
        let _ = FileDescriptor::ioctlv::<1>
            as fn(&FileDescriptor, i32, usize, [IoctlvEntry<'_>; 1]) -> i32;
        let _ = FileDescriptor::ioctlv_reboot::<1>
            as fn(&FileDescriptor, i32, usize, [IoctlvEntry<'_>; 1]) -> i32;
        let _ = FileDescriptor::ioctlv_reboot_background::<1>
            as unsafe fn(&FileDescriptor, i32, usize, [IoctlvEntry<'_>; 1]) -> i32;
        let _ = open_file as fn(&CStr, u32) -> Result<FileDescriptor, i32>;
    }

    #[test]
    fn open_file_rejects_oversized_paths_before_backend() {
        let mut bytes = [b'a'; IPC_MAXPATH_LEN as usize + 1];
        bytes[IPC_MAXPATH_LEN as usize] = 0;
        let path = match CStr::from_bytes_with_nul(&bytes) {
            Ok(value) => value,
            Err(_) => return,
        };

        assert_eq!(validate_path(path), Err(IPC_EINVAL));
    }

    #[test]
    fn heap_create_rejects_invalid_sizes() {
        assert_eq!(HeapId::create(0), Err(IPC_EINVAL));
    }

    #[test]
    fn heap_alloc_matches_c_size_formula_and_splits_from_high_end() {
        let mut arena = TestArena([0; 2048]);
        let _guard = TestHeapGuard::install(&mut arena);
        assert_eq!(c_heap_block_size(25), Some(72));
        assert_eq!(heap_block_size(25), Some(72));

        let first = unsafe { heap_alloc(TEST_HEAP_INDEX, 25) }.unwrap();
        let first_offset = first.as_ptr() as usize - arena.0.as_ptr() as usize;
        assert_eq!(first_offset, 2048 - 72 + 40);
        assert_eq!(first_offset % IPC_HEAP_ALIGN, 0);
        let first_alignment =
            unsafe { core::ptr::read((first.as_ptr() as usize - 4) as *const u32) as usize };
        let first_block_offset = first_offset - first_alignment - IPC_HEAP_HEADER;
        let allocated_block = unsafe { heap_read(TEST_HEAP_INDEX, first_block_offset) };
        assert_eq!(allocated_block.size, 72);
        let free_block = unsafe { heap_read(TEST_HEAP_INDEX, 0) };
        assert_eq!(free_block.size, 2048 - 72);

        let second = unsafe { heap_alloc(TEST_HEAP_INDEX, 25) }.unwrap();
        let second_offset = second.as_ptr() as usize - arena.0.as_ptr() as usize;
        assert_eq!(second_offset % IPC_HEAP_ALIGN, 0);
        let second_alignment =
            unsafe { core::ptr::read((second.as_ptr() as usize - 4) as *const u32) as usize };
        let second_block_offset = second_offset - second_alignment - IPC_HEAP_HEADER;
        assert_eq!(
            unsafe { heap_read(TEST_HEAP_INDEX, second_block_offset) }.size,
            72
        );
    }

    #[test]
    fn heap_alloc_uses_free_list_order_for_fragmented_first_fit() {
        let mut arena = TestArena([0; 2048]);
        let _guard = TestHeapGuard::install(&mut arena);
        let a = unsafe { heap_alloc(TEST_HEAP_INDEX, 25) }.unwrap();
        let _b = unsafe { heap_alloc(TEST_HEAP_INDEX, 100) }.unwrap();
        let c = unsafe { heap_alloc(TEST_HEAP_INDEX, 25) }.unwrap();
        let _d = unsafe { heap_alloc(TEST_HEAP_INDEX, 200) }.unwrap();
        unsafe {
            heap_free(TEST_HEAP_INDEX, a);
            heap_free(TEST_HEAP_INDEX, c);
        }
        let reused = unsafe { heap_alloc(TEST_HEAP_INDEX, 25) }.unwrap();
        assert_eq!(reused.as_ptr(), c.as_ptr());
    }

    #[test]
    fn heap_coalescing_removes_the_actual_list_node_and_preserves_other_free_blocks() {
        for absorbed_node_is_head in [true, false] {
            let mut arena = TestArena([0; 2048]);
            let _guard = TestHeapGuard::install(&mut arena);
            let a = unsafe { heap_alloc(TEST_HEAP_INDEX, 64) }.unwrap();
            let b = unsafe { heap_alloc(TEST_HEAP_INDEX, 64) }.unwrap();
            let c = unsafe { heap_alloc(TEST_HEAP_INDEX, 64) }.unwrap();
            let d = unsafe { heap_alloc(TEST_HEAP_INDEX, 64) }.unwrap();
            let e = unsafe { heap_alloc(TEST_HEAP_INDEX, 64) }.unwrap();
            let f = unsafe { heap_alloc(TEST_HEAP_INDEX, 64) }.unwrap();
            unsafe {
                heap_free(TEST_HEAP_INDEX, c);
                if absorbed_node_is_head {
                    heap_free(TEST_HEAP_INDEX, e);
                    heap_free(TEST_HEAP_INDEX, a);
                } else {
                    heap_free(TEST_HEAP_INDEX, a);
                    heap_free(TEST_HEAP_INDEX, e);
                }
                // b has free memory neighbors c and a. Their list positions
                // differ from their positions in memory, and e must survive.
                heap_free(TEST_HEAP_INDEX, b);
                let mut cursor = IPC_HEAPS[TEST_HEAP_INDEX].free_head;
                for _ in 0..3 {
                    assert_ne!(cursor, IPC_HEAP_NULL);
                    let node = heap_read(TEST_HEAP_INDEX, cursor as usize);
                    assert!(node.size > 0, "a removed node must not remain in the list");
                    cursor = node.next;
                }
                assert_eq!(cursor, IPC_HEAP_NULL, "free list must not contain a cycle");
                heap_free(TEST_HEAP_INDEX, d);
                heap_free(TEST_HEAP_INDEX, f);
                assert_eq!(IPC_HEAPS[TEST_HEAP_INDEX].free_head, 0);
                let whole = heap_read(TEST_HEAP_INDEX, 0);
                assert_eq!(whole.size, 2048);
                assert_eq!(whole.next, IPC_HEAP_NULL);
            }
            let large = unsafe { heap_alloc(TEST_HEAP_INDEX, 1900) }
                .expect("all freed space must be available again");
            unsafe { heap_free(TEST_HEAP_INDEX, large) };
            assert_eq!(unsafe { heap_read(TEST_HEAP_INDEX, 0) }.size, 2048);
        }
    }

    #[test]
    fn ipc_heap_model_matches_libogc_limits() {
        assert_eq!(IPC_DEFAULT_HEAP_BYTES, 4096);
        assert_eq!(IPC_NUM_HEAPS, 16);
    }

    #[test]
    fn ipc_buffer_accessors_match_c_abi_and_boundary_rules() {
        let _: extern "C" fn() -> *mut c_void = IPC_GetBufferLo;
        let _: extern "C" fn() -> *mut c_void = IPC_GetBufferHi;
        let _: unsafe extern "C" fn(*mut c_void) = IPC_SetBufferLo;
        let _: unsafe extern "C" fn(*mut c_void) = IPC_SetBufferHi;

        assert!(valid_buffer_lo(0x1000, 0x1000));
        assert!(valid_buffer_lo(0x2000, 0x1000));
        assert!(!valid_buffer_lo(0x0fff, 0x1000));
        assert!(valid_buffer_hi(0x2000, 0x2000));
        assert!(valid_buffer_hi(0x1000, 0x2000));
        assert!(!valid_buffer_hi(0x2001, 0x2000));
    }
}
