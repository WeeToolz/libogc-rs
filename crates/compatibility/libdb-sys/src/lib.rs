#![no_std]

extern crate alloc;

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::arch::global_asm;
use core::cell::UnsafeCell;
use core::ffi::{c_char, c_void};
use core::mem::size_of;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicPtr, AtomicU16, AtomicU32, Ordering};

pub use ogc_sys::Bool;
use ogc_usbgecko as usb;
use tuxedo_sys as tuxedo;

mod bba;
mod private_net;
#[cfg(feature = "wii-network")]
pub mod wii_network;

use private_net::{Ipv4Config, NetError, PrivateNet};

const ERRNO_EADDRINUSE: i32 = -98;
const UIP_ERR_USE: i32 = -10;

pub const GDBSTUB_DEVICE_USB: i32 = 0;
pub const GDBSTUB_DEVICE_TCP: i32 = 1;
pub const GDBSTUB_DEF_CHANNEL: i32 = 0;
pub const GDBSTUB_DEF_TCPPORT: i32 = 2828;

pub type DbgOpenFn = Option<unsafe extern "C" fn(device: *mut DbgInterface) -> i32>;
pub type DbgCloseFn = Option<unsafe extern "C" fn(device: *mut DbgInterface) -> i32>;
pub type DbgWaitFn = Option<unsafe extern "C" fn(device: *mut DbgInterface) -> i32>;
pub type DbgReadFn =
    Option<unsafe extern "C" fn(device: *mut DbgInterface, buffer: *mut c_void, size: i32) -> i32>;
pub type DbgWriteFn = Option<
    unsafe extern "C" fn(device: *mut DbgInterface, buffer: *const c_void, size: i32) -> i32,
>;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct DbgInterface {
    pub fhndl: i32,
    pub open: DbgOpenFn,
    pub close: DbgCloseFn,
    pub wait: DbgWaitFn,
    pub read: DbgReadFn,
    pub write: DbgWriteFn,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct UipIpAddr {
    pub addr: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
struct SockaddrIn {
    sin_len: u8,
    sin_family: u8,
    sin_port: u16,
    sin_addr: UipIpAddr,
    sin_zero: [u8; 8],
}

struct PrivateNetCell(UnsafeCell<Option<PrivateNet>>);

// Access is serialized by PRIVATE_NET_LOCK. The target is single-core, but the
// lock also prevents Rust aliasing when debugger callbacks are invoked from
// different execution contexts.
unsafe impl Sync for PrivateNetCell {}

static PRIVATE_NET: PrivateNetCell = PrivateNetCell(UnsafeCell::new(None));
static PRIVATE_NET_LOCK: AtomicBool = AtomicBool::new(false);
static TCP_SOCKET_MASK: AtomicU32 = AtomicU32::new(0);
static TCP_BOUND_SOCKET: AtomicI32 = AtomicI32::new(-1);

struct PrivateNetGuard;

impl PrivateNetGuard {
    fn acquire() -> Self {
        while PRIVATE_NET_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        Self
    }
}

impl Drop for PrivateNetGuard {
    fn drop(&mut self) {
        PRIVATE_NET_LOCK.store(false, Ordering::Release);
    }
}

fn with_private_net<R>(f: impl FnOnce(&mut PrivateNet) -> R) -> Option<R> {
    let _guard = PrivateNetGuard::acquire();
    let slot = unsafe { &mut *PRIVATE_NET.0.get() };
    slot.as_mut().map(f)
}

fn replace_private_net(value: Option<PrivateNet>) {
    let _guard = PrivateNetGuard::acquire();
    unsafe { *PRIVATE_NET.0.get() = value };
}

fn socket_is_allocated(socket: i32) -> bool {
    (0..32).contains(&socket)
        && TCP_SOCKET_MASK.load(Ordering::Acquire) & (1u32 << socket as u32) != 0
}

fn allocate_socket_handle() -> i32 {
    loop {
        let current = TCP_SOCKET_MASK.load(Ordering::Acquire);
        let free = !current;
        if free == 0 {
            return -1;
        }
        let socket = free.trailing_zeros();
        let bit = 1u32 << socket;
        if TCP_SOCKET_MASK
            .compare_exchange(current, current | bit, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
        {
            return socket as i32;
        }
    }
}

fn release_socket_handle(socket: i32) {
    if (0..32).contains(&socket) {
        TCP_SOCKET_MASK.fetch_and(!(1u32 << socket as u32), Ordering::AcqRel);
    }
}

/// Validates a signed TCP debugger callback length before converting it to the
/// unsigned length used by the network backend.
#[doc(hidden)]
pub fn validate_tcp_read_size(size: i32) -> Option<i32> {
    (size >= 0).then_some(size)
}

/// Preserves USB debugger transfer byte counts, including zero-byte results.
#[doc(hidden)]
pub fn usb_transfer_callback_result(transferred: i32) -> i32 {
    if transferred < 0 { -1 } else { transferred }
}

static DEBUG_DEVICE: AtomicI32 = AtomicI32::new(GDBSTUB_DEVICE_USB);
static DEBUG_PORT: AtomicI32 = AtomicI32::new(GDBSTUB_DEF_CHANNEL);
static DEBUG_BREAKS: AtomicI32 = AtomicI32::new(0);
static DEBUG_ACTIVE: AtomicBool = AtomicBool::new(false);
static DEBUG_INSTEP: AtomicBool = AtomicBool::new(false);
static DEBUG_INITIALIZED: AtomicBool = AtomicBool::new(false);
static TCP_OPEN: AtomicBool = AtomicBool::new(false);
static TCP_TIMER: AtomicBool = AtomicBool::new(false);
static TCP_BOUND: AtomicBool = AtomicBool::new(false);
static TCP_LISTENING: AtomicBool = AtomicBool::new(false);
static TCP_PORT: AtomicU16 = AtomicU16::new(GDBSTUB_DEF_TCPPORT as u16);
static TCP_LISTEN_SOCKET: AtomicI32 = AtomicI32::new(-1);
static TCP_ACTIVE_SOCKET: AtomicI32 = AtomicI32::new(-1);
static TCP_TIMER_SOCKET: AtomicI32 = AtomicI32::new(-1);
static CURRENT_DEVICE: AtomicPtr<DbgInterface> = AtomicPtr::new(ptr::null_mut());
static USB_FLUSH_COUNT: AtomicI32 = AtomicI32::new(0);

global_asm!(
    r#"
    .section .rodata
.Llibdb_empty_ip:
    .asciz ""

    .section .data
    .weak tcp_localip
    .type tcp_localip, @object
tcp_localip:
    .long .Llibdb_empty_ip
    .size tcp_localip, 4

    .weak tcp_netmask
    .type tcp_netmask, @object
tcp_netmask:
    .long .Llibdb_empty_ip
    .size tcp_netmask, 4

    .weak tcp_gateway
    .type tcp_gateway, @object
tcp_gateway:
    .long .Llibdb_empty_ip
    .size tcp_gateway, 4
    "#
);

unsafe extern "C" {
    static tcp_localip: *const c_char;
    static tcp_netmask: *const c_char;
    static tcp_gateway: *const c_char;
}

global_asm!(
    r#"
    .section .text.__breakinst,"ax",@progbits
    .balign 4
    .globl __breakinst
    .type __breakinst, @function
__breakinst:
    .long 0x7d821008
    blr
    .size __breakinst, . - __breakinst

    .globl __libdb_load_u8
    .type __libdb_load_u8, @function
__libdb_load_u8:
    lbz 3, 0(3)
    blr
    .size __libdb_load_u8, . - __libdb_load_u8

    .globl __libdb_load_u32
    .type __libdb_load_u32, @function
__libdb_load_u32:
    lwz 3, 0(3)
    blr
    .size __libdb_load_u32, . - __libdb_load_u32

    .globl __libdb_store_u32
    .type __libdb_store_u32, @function
__libdb_store_u32:
    stw 4, 0(3)
    blr
    .size __libdb_store_u32, . - __libdb_store_u32
"#
);

unsafe extern "C" {
    fn __breakinst();
    fn __libdb_load_u8(address: u32) -> u8;
    fn __libdb_load_u32(address: u32) -> u32;
    fn __libdb_store_u32(address: u32, value: u32);
}

const BUFMAX: usize = 2048;
const MSR_SE: u32 = 1 << 10;
const MSR_FP: u32 = 1 << 13;
const MSR_RI: u32 = 1 << 1;
const SP_REGNUM: u8 = 1;
const PC_REGNUM: u8 = 64;

const PPC_EXCPT_MCHK: u32 = 2;
const PPC_EXCPT_DSI: u32 = 3;
const PPC_EXCPT_ISI: u32 = 4;
const PPC_EXCPT_ALIGN: u32 = 6;
const PPC_EXCPT_UNDEF: u32 = 7;
const PPC_EXCPT_SYSCALL: u32 = 12;
const PPC_EXCPT_TRACE: u32 = 13;
const PPC_EXCPT_BKPT: u32 = 19;

const SIG_HUP: u8 = 1;
const SIG_BUS: u8 = 10;
const SIG_FPE: u8 = 8;
const SIG_SEGV: u8 = 11;
const SIG_SYS: u8 = 12;
const SIG_TRAP: u8 = 5;

const HEX: &[u8; 16] = b"0123456789abcdef";
const BPCODE: u32 = 0x7d82_1008;
const MAX_BREAKPOINTS: usize = 256;

#[derive(Copy, Clone)]
struct Breakpoint {
    address: u32,
    instruction: u32,
    next: i16,
}

const EMPTY_BREAKPOINT: Breakpoint = Breakpoint {
    address: 0,
    instruction: u32::MAX,
    next: -1,
};

static mut BREAKPOINTS: [Breakpoint; MAX_BREAKPOINTS] = [EMPTY_BREAKPOINT; MAX_BREAKPOINTS];
static mut BREAKPOINT_HEAD: i16 = -1;

#[inline]
fn frame_word(frame: *mut u32, index: usize) -> *mut u32 {
    unsafe { frame.add(index) }
}

#[inline]
fn frame_pc(frame: *mut u32) -> *mut u32 {
    frame_word(frame, 0)
}

#[inline]
fn frame_msr(frame: *mut u32) -> *mut u32 {
    frame_word(frame, 1)
}

#[inline]
fn frame_gpr(frame: *mut u32, reg: usize) -> *mut u32 {
    frame_word(frame, 6 + reg)
}

fn compute_signal(exid: u32) -> u8 {
    match exid {
        PPC_EXCPT_MCHK | PPC_EXCPT_DSI => SIG_SEGV,
        PPC_EXCPT_ISI | PPC_EXCPT_ALIGN => SIG_BUS,
        PPC_EXCPT_UNDEF | PPC_EXCPT_TRACE | PPC_EXCPT_BKPT => SIG_TRAP,
        PPC_EXCPT_SYSCALL => SIG_SYS,
        8 => SIG_FPE,
        _ => SIG_HUP,
    }
}

unsafe fn device_read_char(device: *mut DbgInterface) -> u8 {
    let Some(read) = (unsafe { (*device).read }) else {
        return 0;
    };
    let mut ch = 0u8;
    let len = unsafe { read(device, (&mut ch as *mut u8).cast(), 1) };
    if len > 0 { ch } else { 0 }
}

unsafe fn device_write_all(device: *mut DbgInterface, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    let Some(write) = (unsafe { (*device).write }) else {
        return;
    };
    let _ = unsafe { write(device, bytes.as_ptr().cast(), bytes.len() as i32) };
}

unsafe fn put_packet(device: *mut DbgInterface, payload: &[u8]) {
    let mut packet = [0u8; BUFMAX + 4];
    let len = payload.len().min(BUFMAX);
    let mut sum = 0u8;
    packet[0] = b'$';
    for (i, &byte) in payload[..len].iter().enumerate() {
        packet[1 + i] = byte;
        sum = sum.wrapping_add(byte);
    }
    packet[1 + len] = b'#';
    packet[2 + len] = HEX[(sum >> 4) as usize];
    packet[3 + len] = HEX[(sum & 0x0f) as usize];
    loop {
        unsafe { device_write_all(device, &packet[..len + 4]) };
        if unsafe { device_read_char(device) } & 0x7f == b'+' {
            return;
        }
    }
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

unsafe fn get_packet(device: *mut DbgInterface, buffer: &mut [u8; BUFMAX]) -> usize {
    loop {
        while unsafe { device_read_char(device) } & 0x7f != b'$' {}

        let mut count = 0usize;
        let mut sum = 0u8;
        let mut overflow = false;
        loop {
            let ch = unsafe { device_read_char(device) } & 0x7f;
            if ch == b'#' {
                break;
            }
            if count >= BUFMAX {
                overflow = true;
                continue;
            }
            buffer[count] = ch;
            count += 1;
            sum = sum.wrapping_add(ch);
        }
        let hi = hex_nibble(unsafe { device_read_char(device) } & 0x7f);
        let lo = hex_nibble(unsafe { device_read_char(device) } & 0x7f);
        let Some(sent) = hi.zip(lo).map(|(hi, lo)| (hi << 4) | lo) else {
            unsafe { device_write_all(device, b"-") };
            continue;
        };
        if overflow || sum != sent {
            unsafe { device_write_all(device, b"-") };
            continue;
        }

        unsafe { device_write_all(device, b"+") };
        if count >= 3 && buffer[2] == b':' {
            unsafe { device_write_all(device, &buffer[..2]) };
            buffer.copy_within(3..count, 0);
            count -= 3;
        }
        return count;
    }
}

fn push_hex_byte(out: &mut [u8], pos: &mut usize, byte: u8) {
    if *pos + 2 > out.len() {
        return;
    }
    out[*pos] = HEX[(byte >> 4) as usize];
    out[*pos + 1] = HEX[(byte & 0x0f) as usize];
    *pos += 2;
}

unsafe fn push_word_bytes(out: &mut [u8], pos: &mut usize, word: *const u32) {
    let bytes = word.cast::<u8>();
    for i in 0..4 {
        push_hex_byte(out, pos, unsafe { ptr::read(bytes.add(i)) });
    }
}

unsafe fn push_raw_bytes(out: &mut [u8], pos: &mut usize, bytes: *const u8, count: usize) {
    for i in 0..count {
        push_hex_byte(out, pos, unsafe { ptr::read(bytes.add(i)) });
    }
}

fn push_ascii(out: &mut [u8], pos: &mut usize, bytes: &[u8]) {
    let room = out.len().saturating_sub(*pos);
    let count = bytes.len().min(room);
    out[*pos..*pos + count].copy_from_slice(&bytes[..count]);
    *pos += count;
}

fn push_thread_id(out: &mut [u8], pos: &mut usize, thread: *mut tuxedo::KThread) {
    let value = (thread as usize as u32) & 0x3fff_ffff;
    if value == 0 {
        if *pos < out.len() {
            out[*pos] = b'0';
            *pos += 1;
        }
        return;
    }
    let mut started = false;
    for shift in (0..=28).rev().step_by(4) {
        let nibble = ((value >> shift) & 0xf) as usize;
        if nibble != 0 || started {
            if *pos >= out.len() {
                return;
            }
            out[*pos] = HEX[nibble];
            *pos += 1;
            started = true;
        }
    }
}

fn parse_hex_u32(bytes: &[u8]) -> Option<(u32, usize)> {
    let mut value = 0u32;
    let mut count = 0usize;
    for &byte in bytes {
        let Some(nibble) = hex_nibble(byte) else {
            break;
        };
        value = value.checked_shl(4)?.checked_add(nibble as u32)?;
        count += 1;
    }
    (count != 0).then_some((value, count))
}

fn thread_from_id(id: u32, current: *mut tuxedo::KThread) -> *mut tuxedo::KThread {
    if id == 0 {
        current
    } else {
        tuxedo::k_thread_find_physical(id)
    }
}

fn push_fixed_u32(out: &mut [u8], pos: &mut usize, value: u32) {
    for shift in (0..=28).rev().step_by(4) {
        if *pos >= out.len() {
            return;
        }
        out[*pos] = HEX[((value >> shift) & 0xf) as usize];
        *pos += 1;
    }
}

fn push_fixed_thread_id(out: &mut [u8], pos: &mut usize, thread: *mut tuxedo::KThread) {
    push_ascii(out, pos, b"00000000");
    push_fixed_u32(out, pos, (thread as usize as u32) & 0x3fff_ffff);
}

fn parse_fixed_u32(bytes: &[u8]) -> Option<u32> {
    if bytes.len() < 8 {
        return None;
    }
    let mut value = 0u32;
    for &byte in &bytes[..8] {
        value = (value << 4) | hex_nibble(byte)? as u32;
    }
    Some(value)
}

fn parse_fixed_thread_id(bytes: &[u8]) -> Option<u32> {
    if bytes.len() < 16 || bytes[..8] != *b"00000000" {
        return None;
    }
    parse_fixed_u32(&bytes[8..16])
}

fn valid_memory_address(address: u32) -> bool {
    matches!(address & 0xc000_0000, 0x8000_0000 | 0xc000_0000)
}

unsafe fn encode_memory(address: u32, len: usize, out: &mut [u8; BUFMAX]) -> Option<usize> {
    if !valid_memory_address(address) || len > (BUFMAX - 4) / 2 {
        return None;
    }
    let mut pos = 0usize;
    for offset in 0..len {
        let byte = unsafe { __libdb_load_u8(address.wrapping_add(offset as u32)) };
        push_hex_byte(out, &mut pos, byte);
    }
    Some(pos)
}

unsafe fn reset_breakpoints() {
    let base = core::ptr::addr_of_mut!(BREAKPOINTS).cast::<Breakpoint>();
    for index in 0..MAX_BREAKPOINTS {
        unsafe { *base.add(index) = EMPTY_BREAKPOINT };
    }
    unsafe { BREAKPOINT_HEAD = -1 };
}

unsafe fn insert_breakpoint(address: u32) -> bool {
    let base = core::ptr::addr_of_mut!(BREAKPOINTS).cast::<Breakpoint>();
    for index in 0..MAX_BREAKPOINTS {
        let entry = unsafe { base.add(index) };
        if unsafe { (*entry).address } == 0 {
            let instruction = unsafe { __libdb_load_u32(address) };
            unsafe {
                (*entry).address = address;
                (*entry).instruction = instruction;
                (*entry).next = BREAKPOINT_HEAD;
                BREAKPOINT_HEAD = index as i16;
                __libdb_store_u32(address, BPCODE);
                ogc_cache::dc_store_range_no_sync((address & !0x1f) as *const c_void, 32);
                ogc_cache::ic_invalidate_range((address & !0x1f) as *const c_void, 32);
            }
            ogc_cache::sync();
            return true;
        }
    }
    false
}

unsafe fn remove_breakpoint(address: u32) -> bool {
    let base = core::ptr::addr_of_mut!(BREAKPOINTS).cast::<Breakpoint>();
    let mut previous: i16 = -1;
    let mut current = unsafe { BREAKPOINT_HEAD };
    while current >= 0 {
        let entry = unsafe { base.add(current as usize) };
        if unsafe { (*entry).address } == address {
            let next = unsafe { (*entry).next };
            if previous < 0 {
                unsafe { BREAKPOINT_HEAD = next };
            } else {
                unsafe { (*base.add(previous as usize)).next = next };
            }
            let instruction = unsafe { (*entry).instruction };
            unsafe {
                __libdb_store_u32(address, instruction);
                *entry = EMPTY_BREAKPOINT;
                ogc_cache::dc_store_range_no_sync((address & !0x1f) as *const c_void, 32);
                ogc_cache::ic_invalidate_range((address & !0x1f) as *const c_void, 32);
            }
            ogc_cache::sync();
            return true;
        }
        previous = current;
        current = unsafe { (*entry).next };
    }
    false
}

fn parse_breakpoint_packet(bytes: &[u8]) -> Option<(u8, u32, u32)> {
    if bytes.len() < 6 {
        return None;
    }
    let kind = hex_nibble(bytes[1])?;
    if bytes[2] != b',' {
        return None;
    }
    let (address, address_len) = parse_hex_u32(&bytes[3..])?;
    let comma = 3 + address_len;
    if bytes.get(comma).copied() != Some(b',') {
        return None;
    }
    let (len, len_len) = parse_hex_u32(&bytes[comma + 1..])?;
    if comma + 1 + len_len != bytes.len() || len == 0 {
        return None;
    }
    Some((kind, address, len))
}

unsafe fn encode_registers(
    frame: *mut u32,
    stopped_thread: *mut tuxedo::KThread,
    selected_thread: *mut tuxedo::KThread,
    out: &mut [u8; BUFMAX],
) -> usize {
    if selected_thread.is_null() {
        return 0;
    }
    let thread_ctx = unsafe { &(*selected_thread).ctx };
    let current = selected_thread == stopped_thread;
    let frame_ctx = frame.cast::<tuxedo::PpcContext>();
    let mut pos = 0usize;

    if current {
        unsafe { push_raw_bytes(out, &mut pos, (*frame_ctx).gpr.as_ptr().cast(), 32 * 4) };
    } else {
        unsafe { push_raw_bytes(out, &mut pos, thread_ctx.gpr.as_ptr().cast(), 32 * 4) };
    }
    unsafe { push_raw_bytes(out, &mut pos, thread_ctx.fpr.as_ptr().cast(), 32 * 8) };

    let scalar = if current {
        unsafe { &*frame_ctx }
    } else {
        thread_ctx
    };
    for value in [
        scalar.pc, scalar.msr, scalar.cr, scalar.lr, scalar.ctr, scalar.xer,
    ] {
        unsafe { push_raw_bytes(out, &mut pos, (&value as *const u32).cast(), 4) };
    }
    unsafe { push_raw_bytes(out, &mut pos, (&thread_ctx.fpscr as *const u64).cast(), 4) };
    pos
}

unsafe fn report_exception(
    exid: u32,
    frame: *mut u32,
    thread: *mut tuxedo::KThread,
    out: &mut [u8; BUFMAX],
) -> usize {
    let mut pos = 0usize;
    out[pos] = b'T';
    pos += 1;
    push_hex_byte(out, &mut pos, compute_signal(exid));
    push_hex_byte(out, &mut pos, SP_REGNUM);
    out[pos] = b':';
    pos += 1;
    unsafe { push_word_bytes(out, &mut pos, frame_gpr(frame, 1)) };
    out[pos] = b';';
    pos += 1;
    push_hex_byte(out, &mut pos, PC_REGNUM);
    out[pos] = b':';
    pos += 1;
    unsafe { push_word_bytes(out, &mut pos, frame_pc(frame)) };
    out[pos] = b';';
    pos += 1;
    push_ascii(out, &mut pos, b"thread:");
    push_thread_id(out, &mut pos, thread);
    if pos < out.len() {
        out[pos] = b';';
        pos += 1;
    }
    pos
}

unsafe extern "C" fn debug_exception_handler(exid: u32, frame: *mut u32) {
    if frame.is_null() {
        return;
    }
    let device = CURRENT_DEVICE.load(Ordering::Acquire);
    if device.is_null() {
        return;
    }
    let Some(open) = (unsafe { (*device).open }) else {
        return;
    };
    if unsafe { open(device) } < 0 {
        return;
    }

    let stopped_thread = tuxedo::k_thread_get_self();
    let mut current_thread = stopped_thread;
    let mut input = [0u8; BUFMAX];
    let mut output = [0u8; BUFMAX];

    if DEBUG_ACTIVE.load(Ordering::Acquire) {
        let len = unsafe { report_exception(exid, frame, stopped_thread, &mut output) };
        unsafe { put_packet(device, &output[..len]) };
    }

    let break_pc = __breakinst as *const () as usize as u32;
    if unsafe { ptr::read(frame_pc(frame)) } == break_pc {
        unsafe { ptr::write(frame_pc(frame), break_pc.wrapping_add(4)) };
    }

    let mut close_on_exit = false;
    loop {
        let input_len = unsafe { get_packet(device, &mut input) };
        let command = input.first().copied().unwrap_or(0);
        let output_len = match command {
            b'?' => unsafe { report_exception(exid, frame, stopped_thread, &mut output) },
            b'g' => unsafe { encode_registers(frame, stopped_thread, current_thread, &mut output) },
            b'm' => {
                let parsed = parse_hex_u32(&input[1..input_len]).and_then(|(address, used)| {
                    let comma = 1 + used;
                    if input.get(comma).copied() != Some(b',') {
                        return None;
                    }
                    let (len, len_used) = parse_hex_u32(&input[comma + 1..input_len])?;
                    (comma + 1 + len_used == input_len).then_some((address, len as usize))
                });
                if let Some((address, len)) = parsed {
                    unsafe { encode_memory(address, len, &mut output) }.unwrap_or_else(|| {
                        output[..3].copy_from_slice(b"E00");
                        3
                    })
                } else {
                    output[..3].copy_from_slice(b"E00");
                    3
                }
            }
            b'Z' | b'z' => {
                if let Some((kind, address, len)) = parse_breakpoint_packet(&input[..input_len]) {
                    if kind != 0 {
                        if command == b'Z' {
                            output[..3].copy_from_slice(b"E02");
                            3
                        } else {
                            0
                        }
                    } else if len < 4 {
                        let code = if command == b'Z' { b"E03" } else { b"E02" };
                        output[..3].copy_from_slice(code);
                        3
                    } else {
                        let ok = if command == b'Z' {
                            unsafe { insert_breakpoint(address) }
                        } else {
                            unsafe { remove_breakpoint(address) }
                        };
                        if ok {
                            output[..2].copy_from_slice(b"OK");
                            2
                        } else {
                            let code = if command == b'Z' { b"E04" } else { b"E03" };
                            output[..3].copy_from_slice(code);
                            3
                        }
                    }
                } else {
                    output[..3].copy_from_slice(b"E01");
                    3
                }
            }
            b'H' => {
                if input_len < 3 || input[1] != b'g' {
                    output[..3].copy_from_slice(b"E01");
                    3
                } else if let Some((id, used)) = parse_hex_u32(&input[2..input_len]) {
                    if 2 + used != input_len {
                        output[..3].copy_from_slice(b"E01");
                        3
                    } else {
                        let selected = thread_from_id(id, stopped_thread);
                        if selected.is_null() {
                            output[..3].copy_from_slice(b"E02");
                            3
                        } else {
                            current_thread = selected;
                            output[..2].copy_from_slice(b"OK");
                            2
                        }
                    }
                } else {
                    output[..3].copy_from_slice(b"E01");
                    3
                }
            }
            b'T' => {
                if let Some((id, used)) = parse_hex_u32(&input[1..input_len]) {
                    if 1 + used != input_len {
                        output[..3].copy_from_slice(b"E01");
                        3
                    } else if tuxedo::k_thread_find_physical(id).is_null() {
                        output[..3].copy_from_slice(b"E02");
                        3
                    } else {
                        output[..2].copy_from_slice(b"OK");
                        2
                    }
                } else {
                    output[..3].copy_from_slice(b"E01");
                    3
                }
            }
            b'q' if input_len >= 2 && input[1] == b'C' => {
                let mut pos = 0usize;
                push_ascii(&mut output, &mut pos, b"QC");
                push_thread_id(&mut output, &mut pos, stopped_thread);
                pos
            }
            b'q' if input_len >= 26 && input[1] == b'P' => {
                let mask = parse_fixed_u32(&input[2..10]);
                let id = parse_fixed_thread_id(&input[10..26]);
                match (mask, id) {
                    (Some(mask), Some(id)) if mask & !0x1f == 0 => {
                        let thread = tuxedo::k_thread_find_physical(id);
                        if thread.is_null() {
                            output[..3].copy_from_slice(b"E02");
                            3
                        } else {
                            let mut pos = 0usize;
                            push_ascii(&mut output, &mut pos, b"qQ");
                            push_fixed_u32(&mut output, &mut pos, mask);
                            push_fixed_thread_id(&mut output, &mut pos, thread);
                            if mask & 0x01 != 0 {
                                push_ascii(&mut output, &mut pos, b"0000000110");
                                push_fixed_thread_id(&mut output, &mut pos, thread);
                            }
                            if mask & 0x02 != 0 {
                                push_ascii(&mut output, &mut pos, b"00000002011");
                            }
                            if mask & 0x04 != 0 {
                                let mut display = [0u8; 64];
                                let mut dpos = 0usize;
                                push_ascii(
                                    &mut display,
                                    &mut dpos,
                                    b"libogc task:   control at: 0x",
                                );
                                push_fixed_u32(&mut display, &mut dpos, thread as usize as u32);
                                push_ascii(&mut output, &mut pos, b"00000004");
                                push_hex_byte(&mut output, &mut pos, dpos as u8);
                                push_ascii(&mut output, &mut pos, &display[..dpos]);
                            }
                            if mask & 0x08 != 0 {
                                push_ascii(&mut output, &mut pos, b"0000000800");
                            }
                            if mask & 0x10 != 0 {
                                push_ascii(&mut output, &mut pos, b"0000001000");
                            }
                            pos
                        }
                    }
                    _ => {
                        output[..3].copy_from_slice(b"E01");
                        3
                    }
                }
            }
            b'q' if input_len >= 21 && input[1] == b'L' => {
                let first = hex_nibble(input[2]);
                let max_hi = hex_nibble(input[3]);
                let max_lo = hex_nibble(input[4]);
                let anchor_id = parse_fixed_thread_id(&input[5..21]);
                match (first, max_hi, max_lo, anchor_id) {
                    (Some(first), Some(hi), Some(lo), Some(anchor_id)) if ((hi << 4) | lo) != 0 => {
                        let max_count = usize::from((hi << 4) | lo).min(32);
                        let anchor = tuxedo::k_thread_find_physical(anchor_id);
                        let mut thread = if first != 0 || anchor_id == 0 {
                            tuxedo::k_thread_first()
                        } else if anchor.is_null() {
                            ptr::null_mut()
                        } else {
                            unsafe { tuxedo::k_thread_next(anchor) }
                        };
                        let mut ids = [ptr::null_mut(); 32];
                        let mut count = 0usize;
                        while count < max_count && !thread.is_null() {
                            ids[count] = thread;
                            count += 1;
                            thread = unsafe { tuxedo::k_thread_next(thread) };
                        }
                        let done = thread.is_null();
                        let mut pos = 0usize;
                        push_ascii(&mut output, &mut pos, b"qM");
                        push_hex_byte(&mut output, &mut pos, count as u8);
                        output[pos] = if done { b'1' } else { b'0' };
                        pos += 1;
                        push_fixed_thread_id(
                            &mut output,
                            &mut pos,
                            tuxedo::k_thread_find_physical(anchor_id),
                        );
                        for &id_thread in &ids[..count] {
                            push_fixed_thread_id(&mut output, &mut pos, id_thread);
                        }
                        pos
                    }
                    _ => {
                        output[..3].copy_from_slice(b"E02");
                        3
                    }
                }
            }
            b'q' if input_len >= 12 && &input[1..12] == b"fThreadInfo" => {
                let mut pos = 0usize;
                output[pos] = b'm';
                pos += 1;
                let mut thread = tuxedo::k_thread_first();
                let mut first = true;
                while !thread.is_null() {
                    if !first {
                        output[pos] = b',';
                        pos += 1;
                    }
                    push_thread_id(&mut output, &mut pos, thread);
                    first = false;
                    thread = unsafe { tuxedo::k_thread_next(thread) };
                }
                pos
            }
            b'q' if input_len >= 12 && &input[1..12] == b"sThreadInfo" => {
                output[0] = b'l';
                1
            }
            b'D' => {
                DEBUG_INSTEP.store(false, Ordering::Release);
                DEBUG_ACTIVE.store(false, Ordering::Release);
                let msr = unsafe { ptr::read(frame_msr(frame)) } & !MSR_SE;
                unsafe { ptr::write(frame_msr(frame), msr) };
                output[..2].copy_from_slice(b"OK");
                close_on_exit = true;
                unsafe { put_packet(device, &output[..2]) };
                break;
            }
            b'k' => {
                DEBUG_INSTEP.store(false, Ordering::Release);
                DEBUG_ACTIVE.store(false, Ordering::Release);
                let msr = unsafe { ptr::read(frame_msr(frame)) } & !MSR_SE;
                unsafe {
                    ptr::write(frame_msr(frame), msr);
                    ptr::write(frame_pc(frame), 0x8000_1800);
                }
                break;
            }
            b'c' => {
                DEBUG_INSTEP.store(false, Ordering::Release);
                DEBUG_ACTIVE.store(true, Ordering::Release);
                let msr = unsafe { ptr::read(frame_msr(frame)) } & !MSR_SE;
                unsafe { ptr::write(frame_msr(frame), msr) };
                if let Some(wait) = unsafe { (*device).wait } {
                    let _ = unsafe { wait(device) };
                }
                break;
            }
            b's' => {
                DEBUG_INSTEP.store(true, Ordering::Release);
                DEBUG_ACTIVE.store(true, Ordering::Release);
                let msr = unsafe { ptr::read(frame_msr(frame)) } | MSR_SE;
                unsafe { ptr::write(frame_msr(frame), msr) };
                if let Some(wait) = unsafe { (*device).wait } {
                    let _ = unsafe { wait(device) };
                }
                break;
            }
            _ => {
                let _ = input_len;
                0
            }
        };
        unsafe { put_packet(device, &output[..output_len]) };
    }

    if close_on_exit && let Some(close) = unsafe { (*device).close } {
        let _ = unsafe { close(device) };
    }

    // Match C: relinquish FPU access when returning to a recoverable context.
    let msr = unsafe { ptr::read(frame_msr(frame)) };
    if msr & MSR_RI != 0 {
        unsafe { ptr::write(frame_msr(frame), msr & !MSR_FP) };
    }
}

unsafe fn current_device_is(ptr_: *mut DbgInterface) -> bool {
    CURRENT_DEVICE.load(Ordering::Acquire) == ptr_
}

fn ipv4_addr_value(bytes: [u8; 4]) -> u32 {
    u32::from_be_bytes(bytes)
}

fn configured_ip(pointer: *mut UipIpAddr) -> [u8; 4] {
    if pointer.is_null() {
        // C requires a valid address object here. Avoid its null dereference and
        // use INADDR_NONE as the narrowly scoped safety divergence.
        return [0xff; 4];
    }
    unsafe { (*pointer).addr.to_be_bytes() }
}

fn parse_ipv4_c_string(pointer: *const c_char) -> Option<[u8; 4]> {
    if pointer.is_null() {
        return None;
    }
    let mut bytes = [0u8; 64];
    let mut len = 0usize;
    while len < bytes.len() {
        let byte = unsafe { *(pointer.cast::<u8>().add(len)) };
        if byte == 0 {
            break;
        }
        bytes[len] = byte;
        len += 1;
    }
    if len == bytes.len() || len == 0 {
        return None;
    }
    parse_ipv4_text(&bytes[..len])
}

fn parse_ipv4_text(bytes: &[u8]) -> Option<[u8; 4]> {
    if bytes.is_empty() || !bytes[0].is_ascii_digit() {
        return None;
    }

    let mut parts = [0u32; 4];
    let mut stored = 0usize;
    let mut index = 0usize;
    let mut value;

    loop {
        let mut base = 10u32;
        value = 0u32;
        let mut current = bytes.get(index).copied().unwrap_or(0);
        if !current.is_ascii_digit() {
            return None;
        }
        if current == b'0' {
            index += 1;
            current = bytes.get(index).copied().unwrap_or(0);
            if matches!(current, b'x' | b'X') {
                base = 16;
                index += 1;
                current = bytes.get(index).copied().unwrap_or(0);
            } else {
                base = 8;
            }
        }

        loop {
            if current.is_ascii_digit() {
                // Match uip_ipaton's unsigned arithmetic, including its
                // acceptance of 8 and 9 while parsing an octal component.
                value = value
                    .wrapping_mul(base)
                    .wrapping_add(u32::from(current - b'0'));
                index += 1;
                current = bytes.get(index).copied().unwrap_or(0);
            } else if base == 16 && current.is_ascii_hexdigit() {
                let nibble = match current {
                    b'a'..=b'f' => current - b'a' + 10,
                    b'A'..=b'F' => current - b'A' + 10,
                    _ => unreachable!(),
                };
                value = value.wrapping_shl(4) | u32::from(nibble);
                index += 1;
                current = bytes.get(index).copied().unwrap_or(0);
            } else {
                break;
            }
        }

        if current == b'.' {
            if stored >= 3 {
                return None;
            }
            parts[stored] = value;
            stored += 1;
            index += 1;
            continue;
        }

        // This intentionally preserves uip_ipaton's trailing-character test:
        // ASCII non-whitespace terminators are accepted, while whitespace and
        // non-ASCII terminators fail.
        if current != 0 && (!current.is_ascii() || current.is_ascii_whitespace()) {
            return None;
        }
        break;
    }

    let count = stored + 1;
    let value = match count {
        1 => value,
        2 if value <= 0x00ff_ffff => value | parts[0].wrapping_shl(24),
        3 if value <= 0x0000_ffff => value | parts[0].wrapping_shl(24) | parts[1].wrapping_shl(16),
        4 if value <= 0x0000_00ff => {
            value | parts[0].wrapping_shl(24) | parts[1].wrapping_shl(16) | parts[2].wrapping_shl(8)
        }
        _ => return None,
    };
    Some(value.to_be_bytes())
}

fn reset_tcp_state() {
    replace_private_net(None);
    TCP_SOCKET_MASK.store(0, Ordering::Release);
    TCP_ACTIVE_SOCKET.store(-1, Ordering::Release);
    TCP_LISTEN_SOCKET.store(-1, Ordering::Release);
    TCP_BOUND_SOCKET.store(-1, Ordering::Release);
    TCP_OPEN.store(false, Ordering::Release);
    TCP_TIMER.store(false, Ordering::Release);
    TCP_BOUND.store(false, Ordering::Release);
    TCP_LISTENING.store(false, Ordering::Release);
    TCP_TIMER_SOCKET.store(-1, Ordering::Release);

    unsafe {
        if current_device_is(core::ptr::addr_of_mut!(TCP_IFACE)) {
            TCP_IFACE.fhndl = -1;
        }
    }
}

fn reset_debug_state() {
    reset_tcp_state();
    CURRENT_DEVICE.store(ptr::null_mut(), Ordering::Release);
    DEBUG_ACTIVE.store(false, Ordering::Release);
    DEBUG_INSTEP.store(false, Ordering::Release);
    DEBUG_INITIALIZED.store(false, Ordering::Release);
}

unsafe extern "C" fn iface_open(device: *mut DbgInterface) -> i32 {
    unsafe {
        if device.is_null() {
            return -1;
        }

        if device == core::ptr::addr_of_mut!(TCP_IFACE) {
            let listen = TCP_LISTEN_SOCKET.load(Ordering::Acquire);
            if (*device).fhndl < 0 {
                let accepted = tcpip_accept(listen);
                if accepted < 0 {
                    return -1;
                }
                (*device).fhndl = accepted;
            }
            if (*device).fhndl < 0 {
                return -1;
            }
            tcpip_starttimer((*device).fhndl);
        } else if device == core::ptr::addr_of_mut!(USB_IFACE) {
            if usb::is_gecko_alive((*device).fhndl) == 0 {
                return -1;
            }
        } else {
            return -1;
        }

        CURRENT_DEVICE.store(device, Ordering::Release);
        DEBUG_ACTIVE.store(true, Ordering::Release);
        0
    }
}

unsafe extern "C" fn iface_close(device: *mut DbgInterface) -> i32 {
    unsafe {
        if device.is_null() {
            return -1;
        }

        if device == core::ptr::addr_of_mut!(TCP_IFACE) {
            tcpip_stoptimer((*device).fhndl);
            tcpip_close((*device).fhndl);
            (*device).fhndl = -1;
        }

        if current_device_is(device) {
            CURRENT_DEVICE.store(ptr::null_mut(), Ordering::Release);
        }

        0
    }
}

unsafe extern "C" fn iface_wait(device: *mut DbgInterface) -> i32 {
    unsafe {
        if device.is_null() {
            return -1;
        }

        if device == core::ptr::addr_of_mut!(TCP_IFACE) {
            tcpip_stoptimer((*device).fhndl);
        }

        0
    }
}

unsafe extern "C" fn iface_read(device: *mut DbgInterface, buffer: *mut c_void, size: i32) -> i32 {
    unsafe {
        if device.is_null() {
            return -1;
        }
        // This is the TCP callback. C checks fhndl and does not depend on the
        // interface object's address.
        if (*device).fhndl < 0 {
            return 0;
        }
        let Some(size) = validate_tcp_read_size(size) else {
            return -1;
        };
        if buffer.is_null() {
            return -1;
        }
        if size == 0 {
            return 0;
        }

        tcpip_read((*device).fhndl, buffer, size as u32)
    }
}

unsafe extern "C" fn usb_iface_read(
    device: *mut DbgInterface,
    buffer: *mut c_void,
    size: i32,
) -> i32 {
    unsafe {
        if device.is_null() || buffer.is_null() || size < 0 {
            return -1;
        }
        if size == 0 {
            return 0;
        }
        let count = usb::receive_buffer((*device).fhndl, buffer, size);
        usb_transfer_callback_result(count)
    }
}

unsafe extern "C" fn iface_write(
    device: *mut DbgInterface,
    buffer: *const c_void,
    size: i32,
) -> i32 {
    unsafe {
        if device.is_null() {
            return -1;
        }
        // This is the TCP callback. C checks fhndl and does not depend on the
        // interface object's address.
        if (*device).fhndl < 0 {
            return 0;
        }
        if size < 0 {
            return -1;
        }
        if size == 0 {
            return 0;
        }
        if buffer.is_null() {
            return -1;
        }

        tcpip_write((*device).fhndl, buffer, size as u32)
    }
}

unsafe extern "C" fn usb_iface_write(
    device: *mut DbgInterface,
    buffer: *const c_void,
    size: i32,
) -> i32 {
    unsafe {
        if device.is_null() || size < 0 || (size > 0 && buffer.is_null()) {
            return -1;
        }
        if size == 0 {
            return 0;
        }
        let count = usb::send_buffer((*device).fhndl, buffer, size);
        usb_transfer_callback_result(count)
    }
}

static mut USB_IFACE: DbgInterface = DbgInterface {
    fhndl: 0,
    open: Some(iface_open),
    close: Some(iface_close),
    wait: Some(iface_wait),
    read: Some(usb_iface_read),
    write: Some(usb_iface_write),
};

static mut TCP_IFACE: DbgInterface = DbgInterface {
    fhndl: -1,
    open: Some(iface_open),
    close: Some(iface_close),
    wait: Some(iface_wait),
    read: Some(iface_read),
    write: Some(iface_write),
};

fn usb_alive(channel: i32) -> i32 {
    usb::is_gecko_alive(channel)
}

fn flush(channel: i32) {
    if channel < 0 {
        return;
    }
    USB_FLUSH_COUNT.fetch_add(1, Ordering::AcqRel);
    usb::flush(channel);
}

#[unsafe(no_mangle)]
pub extern "C" fn _break() {
    if !DEBUG_INITIALIZED.load(Ordering::Acquire) {
        return;
    }
    DEBUG_BREAKS.fetch_add(1, Ordering::AcqRel);

    // C exposes `__breakinst` as the exact instruction address that the
    // debugger handler recognizes and advances past after handling the trap.
    // Keep the trap in a separate assembly symbol instead of using inline asm
    // here, so the public PC value matches the C reference.
    unsafe { __breakinst() };
}

pub fn debug_init(device_type: i32, channel_port: i32) {
    DEBUG_DEVICE.store(device_type, Ordering::Release);
    DEBUG_PORT.store(channel_port, Ordering::Release);
    reset_debug_state();
    unsafe { reset_breakpoints() };

    let device = if device_type == GDBSTUB_DEVICE_USB {
        unsafe { init(channel_port) }
    } else {
        // Match C DEBUG_Init: parse the three weak, application-overridable
        // address strings, then pass their binary values into tcpip_init.
        let mut localip = UipIpAddr {
            addr: ipv4_addr_value(parse_ipv4_c_string(unsafe { tcp_localip }).unwrap_or([0xff; 4])),
        };
        let mut netmask = UipIpAddr {
            addr: ipv4_addr_value(parse_ipv4_c_string(unsafe { tcp_netmask }).unwrap_or([0xff; 4])),
        };
        let mut gateway = UipIpAddr {
            addr: ipv4_addr_value(parse_ipv4_c_string(unsafe { tcp_gateway }).unwrap_or([0xff; 4])),
        };
        unsafe {
            tcpip_init(
                &mut localip,
                &mut netmask,
                &mut gateway,
                channel_port as u16,
            )
        }
    };

    if device.is_null() {
        return;
    }

    CURRENT_DEVICE.store(device, Ordering::Release);
    DEBUG_ACTIVE.store(false, Ordering::Release);
    DEBUG_INSTEP.store(false, Ordering::Release);
    ogc_sys::set_general_exception_callback(Some(debug_exception_handler));
    for exid in [
        PPC_EXCPT_MCHK,
        PPC_EXCPT_DSI,
        PPC_EXCPT_ISI,
        PPC_EXCPT_ALIGN,
        PPC_EXCPT_UNDEF,
        PPC_EXCPT_SYSCALL,
        PPC_EXCPT_TRACE,
        PPC_EXCPT_BKPT,
    ] {
        let _ = ogc_sys::install_general_exception_vector(exid);
    }
    DEBUG_INITIALIZED.store(true, Ordering::Release);
}

/// C ABI entry point for `DEBUG_Init`.
#[unsafe(no_mangle)]
pub extern "C" fn DEBUG_Init(device_type: i32, channel_port: i32) {
    debug_init(device_type, channel_port);
}

/// # Safety
///
/// Raw pointer arguments and callbacks must be valid for the duration of the call.
pub unsafe fn init(channel: i32) -> *mut DbgInterface {
    unsafe {
        USB_IFACE.fhndl = channel;
    }
    if usb_alive(channel) != 0 {
        flush(channel);
    }
    core::ptr::addr_of_mut!(USB_IFACE)
}

/// # Safety
///
/// Raw pointer arguments and callbacks must be valid for the duration of the call.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcpip_init(
    localip: *mut UipIpAddr,
    netmask: *mut UipIpAddr,
    gateway: *mut UipIpAddr,
    port: u16,
) -> *mut DbgInterface {
    reset_tcp_state();

    let config = Ipv4Config {
        local: configured_ip(localip),
        netmask: configured_ip(netmask),
        gateway: configured_ip(gateway),
    };
    let stack = match PrivateNet::initialize(config) {
        Ok(stack) => stack,
        Err(_) => return ptr::null_mut(),
    };
    replace_private_net(Some(stack));
    TCP_PORT.store(port, Ordering::Release);

    let listen_socket = tcpip_socket();
    if listen_socket < 0 {
        reset_tcp_state();
        return ptr::null_mut();
    }

    let mut address = SockaddrIn {
        sin_len: size_of::<SockaddrIn>() as u8,
        sin_family: 2,
        sin_port: port.to_be(),
        sin_addr: UipIpAddr { addr: 0 },
        sin_zero: [0; 8],
    };
    let mut address_len = size_of::<SockaddrIn>() as u32;
    if unsafe {
        tcpip_bind(
            listen_socket,
            (&mut address as *mut SockaddrIn).cast(),
            &mut address_len,
        )
    } < 0
        || tcpip_listen(listen_socket, 1) < 0
    {
        tcpip_close(listen_socket);
        reset_tcp_state();
        return ptr::null_mut();
    }

    unsafe { TCP_IFACE.fhndl = -1 };
    TCP_LISTEN_SOCKET.store(listen_socket, Ordering::Release);
    core::ptr::addr_of_mut!(TCP_IFACE)
}

#[unsafe(no_mangle)]
pub extern "C" fn tcpip_close(s: i32) {
    if !socket_is_allocated(s) {
        return;
    }

    if TCP_ACTIVE_SOCKET.load(Ordering::Acquire) == s {
        let _ = with_private_net(|stack| stack.close());
        TCP_ACTIVE_SOCKET.store(-1, Ordering::Release);
        TCP_OPEN.store(false, Ordering::Release);
        TCP_TIMER.store(false, Ordering::Release);
        TCP_TIMER_SOCKET.store(-1, Ordering::Release);
        unsafe {
            if TCP_IFACE.fhndl == s {
                TCP_IFACE.fhndl = -1;
            }
        }
    }

    if TCP_LISTEN_SOCKET.load(Ordering::Acquire) == s {
        let _ = with_private_net(|stack| stack.stop_listen());
        TCP_LISTEN_SOCKET.store(-1, Ordering::Release);
        TCP_LISTENING.store(false, Ordering::Release);
        TCP_BOUND.store(false, Ordering::Release);
        TCP_BOUND_SOCKET.store(-1, Ordering::Release);
    }
    release_socket_handle(s);
}

#[unsafe(no_mangle)]
pub extern "C" fn tcpip_starttimer(s: i32) {
    if !socket_is_allocated(s) || TCP_ACTIVE_SOCKET.load(Ordering::Acquire) != s {
        return;
    }
    TCP_TIMER.store(true, Ordering::Release);
    TCP_TIMER_SOCKET.store(s, Ordering::Release);
}

#[unsafe(no_mangle)]
pub extern "C" fn tcpip_stoptimer(s: i32) {
    if !socket_is_allocated(s) {
        return;
    }
    if TCP_TIMER_SOCKET.load(Ordering::Acquire) == s {
        TCP_TIMER.store(false, Ordering::Release);
        TCP_TIMER_SOCKET.store(-1, Ordering::Release);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tcpip_socket() -> i32 {
    allocate_socket_handle()
}

#[unsafe(no_mangle)]
pub extern "C" fn tcpip_listen(s: i32, _backlog: u32) -> i32 {
    if !socket_is_allocated(s)
        || !TCP_BOUND.load(Ordering::Acquire)
        || TCP_BOUND_SOCKET.load(Ordering::Acquire) != s
    {
        return -1;
    }
    let port = TCP_PORT.load(Ordering::Acquire);
    if with_private_net(|stack| stack.listen(port)).is_none() {
        return -1;
    }
    TCP_LISTENING.store(true, Ordering::Release);
    TCP_LISTEN_SOCKET.store(s, Ordering::Release);
    0
}

/// Binds a debugger TCP socket to an address.
///
/// # Safety
///
/// `name` must be properly aligned and point to an initialized C-compatible
/// IPv4 `sockaddr_in`. A null pointer is rejected instead of reproducing C's
/// undefined null dereference. `namelen` is accepted for ABI parity and is not
/// dereferenced, matching the C implementation.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcpip_bind(s: i32, name: *mut c_void, _namelen: *mut u32) -> i32 {
    if !socket_is_allocated(s) || name.is_null() {
        return -1;
    }
    let address = unsafe { ptr::read(name.cast::<SockaddrIn>()) };
    let mut port = u16::from_be(address.sin_port);
    if port == 0 {
        // uIP chooses from 4096..32767. This server-only backend has no active
        // connect path, so use the first ephemeral port deterministically.
        port = 4097;
    }

    if TCP_BOUND.load(Ordering::Acquire) {
        let current = TCP_BOUND_SOCKET.load(Ordering::Acquire);
        if current != s && TCP_PORT.load(Ordering::Acquire) == port {
            return finish_tcpip_bind(UIP_ERR_USE);
        }
    }
    TCP_PORT.store(port, Ordering::Release);
    TCP_BOUND_SOCKET.store(s, Ordering::Release);
    finish_tcpip_bind(0)
}

fn finish_tcpip_bind(result: i32) -> i32 {
    match result {
        0 => {
            TCP_BOUND.store(true, Ordering::Release);
            0
        }
        ERRNO_EADDRINUSE => UIP_ERR_USE,
        other => other,
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn tcpip_accept(s: i32) -> i32 {
    if !socket_is_allocated(s)
        || !TCP_LISTENING.load(Ordering::Acquire)
        || TCP_LISTEN_SOCKET.load(Ordering::Acquire) != s
    {
        return -1;
    }
    let existing = TCP_ACTIVE_SOCKET.load(Ordering::Acquire);
    if existing >= 0 {
        return existing;
    }

    loop {
        let status = with_private_net(|stack| {
            let result = stack.poll();
            (result, stack.is_accepted())
        });
        match status {
            Some((Ok(()), true)) => {
                let accepted = allocate_socket_handle();
                if accepted < 0 {
                    return -1;
                }
                TCP_ACTIVE_SOCKET.store(accepted, Ordering::Release);
                TCP_OPEN.store(true, Ordering::Release);
                unsafe { TCP_IFACE.fhndl = accepted };
                return accepted;
            }
            Some((Ok(()), false)) | Some((Err(NetError::ConnectionReset), false)) => {}
            Some((Err(_), _)) | None => return -1,
        }
    }
}

/// # Safety
///
/// For nonzero `len`, `buffer` must point to writable storage for `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcpip_read(s: i32, buffer: *mut c_void, len: u32) -> i32 {
    if !socket_is_allocated(s)
        || TCP_ACTIVE_SOCKET.load(Ordering::Acquire) != s
        || !TCP_OPEN.load(Ordering::Acquire)
    {
        return -1;
    }
    if len == 0 {
        loop {
            let result = with_private_net(|stack| {
                stack.poll()?;
                Ok::<bool, NetError>(stack.has_receive_data())
            });
            match result {
                Some(Ok(true)) => return 0,
                Some(Ok(false)) | Some(Err(NetError::WouldBlock)) => {}
                Some(Err(_)) | None => return -1,
            }
        }
    }
    if buffer.is_null() {
        return -1;
    }
    let output = unsafe { core::slice::from_raw_parts_mut(buffer.cast::<u8>(), len as usize) };
    loop {
        let result = with_private_net(|stack| {
            stack.poll()?;
            stack.read(output)
        });
        match result {
            Some(Ok(count)) => return count as i32,
            Some(Err(NetError::WouldBlock)) => {}
            Some(Err(_)) | None => return -1,
        }
    }
}

/// Writes all bytes to the active debugger TCP connection.
///
/// Return zero after the full buffer is queued, matching C `tcpip_write`.
///
/// # Safety
///
/// For nonzero `len`, `buffer` must point to readable storage for `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn tcpip_write(s: i32, buffer: *const c_void, len: u32) -> i32 {
    if !socket_is_allocated(s)
        || TCP_ACTIVE_SOCKET.load(Ordering::Acquire) != s
        || !TCP_OPEN.load(Ordering::Acquire)
    {
        return -1;
    }
    if len == 0 {
        return 0;
    }
    if buffer.is_null() {
        return -1;
    }
    let input = unsafe { core::slice::from_raw_parts(buffer.cast::<u8>(), len as usize) };
    let mut written = 0usize;
    while written < input.len() {
        let result = with_private_net(|stack| {
            stack.poll()?;
            stack.write(&input[written..])
        });
        match result {
            Some(Ok(0)) => return 0,
            Some(Ok(count)) => written += count,
            Some(Err(NetError::WouldBlock)) => {}
            // C advances through the input and ultimately returns UIP_ERR_OK
            // even when its TCP/output path reports an error for a chunk.
            Some(Err(_)) | None => return 0,
        }
    }
    0
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    extern crate std;

    use super::*;
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
    fn constants_and_layouts_match_checkpoint() {
        let _guard = test_guard();
        assert_eq!(GDBSTUB_DEVICE_USB, 0);
        assert_eq!(GDBSTUB_DEVICE_TCP, 1);
        assert_eq!(GDBSTUB_DEF_TCPPORT, 2828);
        assert!(size_of::<DbgInterface>() >= 24);
        assert_eq!(size_of::<UipIpAddr>(), 4);
    }

    #[test]
    fn debugger_and_tcp_entry_points_use_c_abi_signatures() {
        let _guard = test_guard();
        let _: extern "C" fn() = _break;
        let _: extern "C" fn(i32, i32) = DEBUG_Init;
        let _: unsafe extern "C" fn(
            *mut UipIpAddr,
            *mut UipIpAddr,
            *mut UipIpAddr,
            u16,
        ) -> *mut DbgInterface = tcpip_init;
        let _: extern "C" fn(i32) = tcpip_close;
        let _: extern "C" fn(i32) = tcpip_starttimer;
        let _: extern "C" fn(i32) = tcpip_stoptimer;
        let _: extern "C" fn() -> i32 = tcpip_socket;
        let _: extern "C" fn(i32, u32) -> i32 = tcpip_listen;
        let _: unsafe extern "C" fn(i32, *mut c_void, *mut u32) -> i32 = tcpip_bind;
        let _: extern "C" fn(i32) -> i32 = tcpip_accept;
        let _: unsafe extern "C" fn(i32, *mut c_void, u32) -> i32 = tcpip_read;
        let _: unsafe extern "C" fn(i32, *const c_void, u32) -> i32 = tcpip_write;
    }

    #[test]
    fn usb_debug_init_publishes_interface() {
        let _guard = test_guard();
        debug_init(GDBSTUB_DEVICE_USB, 3000);
        assert_eq!(DEBUG_DEVICE.load(Ordering::Acquire), GDBSTUB_DEVICE_USB);
        assert_eq!(DEBUG_PORT.load(Ordering::Acquire), 3000);
        assert!(!CURRENT_DEVICE.load(Ordering::Acquire).is_null());
        assert!(!DEBUG_ACTIVE.load(Ordering::Acquire));
        assert!(DEBUG_INITIALIZED.load(Ordering::Acquire));
    }

    #[test]
    fn invalid_tcp_helpers_are_rejected() {
        let _guard = test_guard();
        let mut buf = [0_u8; 4];
        assert_eq!(
            unsafe { tcpip_bind(-1, ptr::null_mut(), ptr::null_mut()) },
            -1
        );
        assert_eq!(tcpip_listen(-1, 1), -1);
        assert_eq!(tcpip_accept(-1), -1);
        assert_eq!(
            unsafe { tcpip_read(-1, buf.as_mut_ptr().cast(), buf.len() as u32) },
            -1
        );
        assert_eq!(
            unsafe { tcpip_write(-1, buf.as_ptr().cast(), buf.len() as u32) },
            -1
        );
    }

    #[test]
    fn tcp_bind_maps_address_conflict_and_only_marks_success_bound() {
        let _guard = test_guard();
        let original_bound = TCP_BOUND.swap(false, Ordering::AcqRel);
        assert_eq!(finish_tcpip_bind(-98), -10);
        assert_eq!(finish_tcpip_bind(-10), -10);
        assert!(!TCP_BOUND.load(Ordering::Acquire));

        assert_eq!(finish_tcpip_bind(0), 0);
        assert!(TCP_BOUND.load(Ordering::Acquire));
        TCP_BOUND.store(original_bound, Ordering::Release);
    }

    #[test]
    fn tcp_callbacks_return_zero_when_no_client_handle_is_open() {
        let _guard = test_guard();
        let original_handle = unsafe { TCP_IFACE.fhndl };
        unsafe { TCP_IFACE.fhndl = -1 };
        let mut buffer = [0_u8; 4];
        let result = unsafe {
            let device = core::ptr::addr_of_mut!(TCP_IFACE);
            let read =
                TCP_IFACE.read.unwrap()(device, buffer.as_mut_ptr().cast(), buffer.len() as i32);
            let write =
                TCP_IFACE.write.unwrap()(device, buffer.as_ptr().cast(), buffer.len() as i32);
            (read, write)
        };
        unsafe { TCP_IFACE.fhndl = original_handle };
        assert_eq!(result, (0, 0));
    }

    #[test]
    fn copied_tcp_callbacks_return_zero_when_no_client_handle_is_open() {
        let _guard = test_guard();
        let mut copied_iface = unsafe { TCP_IFACE };
        copied_iface.fhndl = -1;
        let mut buffer = [0_u8; 4];
        let read = unsafe {
            copied_iface.read.unwrap()(
                &mut copied_iface,
                buffer.as_mut_ptr().cast(),
                buffer.len() as i32,
            )
        };
        let write = unsafe {
            copied_iface.write.unwrap()(
                &mut copied_iface,
                buffer.as_ptr().cast(),
                buffer.len() as i32,
            )
        };
        assert_eq!((read, write), (0, 0));
    }

    #[test]
    fn tcp_read_rejects_negative_size_before_transport_io() {
        let _guard = test_guard();
        assert_eq!(validate_tcp_read_size(-1), None);
        assert_eq!(validate_tcp_read_size(0), Some(0));
        assert_eq!(validate_tcp_read_size(4), Some(4));
    }

    #[test]
    fn usb_callback_result_preserves_zero_and_transferred_lengths() {
        let _guard = test_guard();
        assert_eq!(usb_transfer_callback_result(-1), -1);
        assert_eq!(usb_transfer_callback_result(0), 0);
        assert_eq!(usb_transfer_callback_result(4), 4);
    }

    #[test]
    fn copied_usb_callbacks_do_not_use_tcp_no_handle_result() {
        let _guard = test_guard();
        let mut copied_iface = unsafe { USB_IFACE };
        copied_iface.fhndl = -1;
        let mut buffer = [0_u8; 1];
        let read = unsafe {
            copied_iface.read.unwrap()(
                &mut copied_iface,
                buffer.as_mut_ptr().cast(),
                buffer.len() as i32,
            )
        };
        let write = unsafe {
            copied_iface.write.unwrap()(
                &mut copied_iface,
                buffer.as_ptr().cast(),
                buffer.len() as i32,
            )
        };
        assert_eq!((read, write), (-1, -1));
    }

    #[test]
    fn ipv4_text_parser_matches_uip_forms() {
        let _guard = test_guard();
        assert_eq!(parse_ipv4_text(b"192.168.1.7"), Some([192, 168, 1, 7]));
        assert_eq!(parse_ipv4_text(b"127.1"), Some([127, 0, 0, 1]));
        assert_eq!(parse_ipv4_text(b"0x7f000001"), Some([127, 0, 0, 1]));
        assert_eq!(parse_ipv4_text(b"0300.0250.01.07"), Some([192, 168, 1, 7]));
        assert_eq!(parse_ipv4_text(b""), None);
        assert_eq!(parse_ipv4_text(b"300.1.1.1"), Some([44, 1, 1, 1]));
        assert_eq!(parse_ipv4_text(b"1x"), Some([0, 0, 0, 1]));
    }

    #[test]
    fn debug_break_counter_tracks_requests() {
        let _guard = test_guard();
        DEBUG_BREAKS.store(0, Ordering::Release);
        debug_init(GDBSTUB_DEVICE_USB, 1);
        _break();
        assert_eq!(DEBUG_BREAKS.load(Ordering::Acquire), 1);
    }
}
