#![no_std]
#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

#[cfg(target_arch = "powerpc")]
extern crate alloc;

use core::ffi::{CStr, c_char, c_void};

mod ipc_cleanup;
#[cfg(not(gamecube))]
mod socket_rules;
#[cfg(all(target_arch = "powerpc", not(gamecube)))]
pub mod std_net;
pub use ipc_cleanup::{initialization_close_failures, last_initialization_close_error};

#[cfg(target_arch = "powerpc")]
use ogc_core::mem_physical_to_k0;
pub use ogc_sys::Bool;

pub const INVALID_SOCKET: u32 = !0;
pub const SOCKET_ERROR: i32 = -1;

mod startup_ip;
#[cfg(test)]
use startup_ip::{MAX_NETWORK_IP_RETRIES, retry_network_ip};
use startup_ip::{active_attach_ip, startup_network_ip};

pub const SOCK_STREAM: u32 = 1;
pub const SOCK_DGRAM: u32 = 2;
pub const SOCK_RAW: u32 = 3;

pub const SO_DEBUG: u32 = 0x0001;
pub const SO_ACCEPTCONN: u32 = 0x0002;
pub const SO_REUSEADDR: u32 = 0x0004;
pub const SO_KEEPALIVE: u32 = 0x0008;
pub const SO_DONTROUTE: u32 = 0x0010;
pub const SO_BROADCAST: u32 = 0x0020;
pub const SO_USELOOPBACK: u32 = 0x0040;
pub const SO_LINGER: u32 = 0x0080;
pub const SO_OOBINLINE: u32 = 0x0100;
pub const SO_REUSEPORT: u32 = 0x0200;
pub const SO_DONTLINGER: i32 = !(SO_LINGER as i32);

pub const SO_SNDBUF: u32 = 0x1001;
pub const SO_RCVBUF: u32 = 0x1002;
pub const SO_SNDLOWAT: u32 = 0x1003;
pub const SO_RCVLOWAT: u32 = 0x1004;
pub const SO_SNDTIMEO: u32 = 0x1005;
pub const SO_RCVTIMEO: u32 = 0x1006;
pub const SO_ERROR: u32 = 0x1007;
pub const SO_TYPE: u32 = 0x1008;

pub const SOL_SOCKET: u32 = 0xffff;

pub const AF_UNSPEC: u32 = 0;
pub const AF_INET: u32 = 2;
pub const PF_INET: u32 = AF_INET;
pub const PF_UNSPEC: u32 = AF_UNSPEC;

pub const IPPROTO_IP: u32 = 0;
pub const IPPROTO_TCP: u32 = 6;
pub const IPPROTO_UDP: u32 = 17;

pub const INADDR_ANY: u32 = 0;
pub const INADDR_BROADCAST: u32 = 0xffff_ffff;

pub const MSG_DONTWAIT: u32 = 0x40;

pub const IP_TOS: u32 = 1;
pub const IP_TTL: u32 = 2;

pub const IPTOS_TOS_MASK: u32 = 0x1e;
pub const IPTOS_LOWDELAY: u32 = 0x10;
pub const IPTOS_THROUGHPUT: u32 = 0x08;
pub const IPTOS_RELIABILITY: u32 = 0x04;
pub const IPTOS_LOWCOST: u32 = 0x02;
pub const IPTOS_MINCOST: u32 = IPTOS_LOWCOST;

pub const IPTOS_PREC_MASK: u32 = 0xe0;
pub const IPTOS_PREC_NETCONTROL: u32 = 0xe0;
pub const IPTOS_PREC_INTERNETCONTROL: u32 = 0xc0;
pub const IPTOS_PREC_CRITIC_ECP: u32 = 0xa0;
pub const IPTOS_PREC_FLASHOVERRIDE: u32 = 0x80;
pub const IPTOS_PREC_FLASH: u32 = 0x60;
pub const IPTOS_PREC_IMMEDIATE: u32 = 0x40;
pub const IPTOS_PREC_PRIORITY: u32 = 0x20;
pub const IPTOS_PREC_ROUTINE: u32 = 0x00;

pub const O_NONBLOCK: u32 = 0o4000;
pub const F_GETFL: u32 = 3;
pub const F_SETFL: u32 = 4;
pub const FD_SETSIZE: usize = 16;

pub const TCP_NODELAY: u32 = 0x01;
pub const TCP_KEEPALIVE: u32 = 0x02;

pub const FIONREAD: u32 = 0x4004_667f;
pub const FIONBIO: u32 = 0x8004_667e;

pub type SockLenT = u32;

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
pub struct InAddr {
    pub s_addr: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub struct SockaddrIn {
    pub sin_len: u8,
    pub sin_family: u8,
    pub sin_port: u16,
    pub sin_addr: InAddr,
    pub sin_zero: [u8; 8],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq, Default)]
pub struct Sockaddr {
    pub sa_len: u8,
    pub sa_family: u8,
    pub sa_data: [u8; 14],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct FdSet {
    pub fd_bits: [u8; FD_SETSIZE.div_ceil(8)],
}

impl Default for FdSet {
    fn default() -> Self {
        Self {
            fd_bits: [0; FD_SETSIZE.div_ceil(8)],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
pub struct Timeval {
    pub tv_sec: i32,
    pub tv_usec: i32,
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
pub struct PollSd {
    pub socket: i32,
    pub events: u32,
    pub revents: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct HostEnt {
    pub h_name: *mut c_char,
    pub h_aliases: *mut *mut c_char,
    pub h_addrtype: i16,
    pub h_length: i16,
    pub h_addr_list: *mut *mut c_char,
}

pub type Netcallback = Option<unsafe extern "C" fn(result: i32, usrdata: *mut c_void) -> i32>;

#[inline(always)]
fn runtime_unreachable<T>() -> T {
    core::unreachable!()
}

macro_rules! powerpc_return {
    ($expr:expr_2021) => {
        if cfg!(target_arch = "powerpc") {
            return $expr;
        }
    };
}

fn parse_ipv4(cp: *const c_char) -> u32 {
    if cp.is_null() {
        return 0xffff_ffff;
    }
    let bytes = unsafe { CStr::from_ptr(cp) }.to_bytes();
    if bytes.is_empty() {
        return 0xffff_ffff;
    }

    let mut parts = [0u32; 4];
    let mut num_parts = 0usize;
    let mut idx = 0usize;
    let mut c = bytes[idx];
    let mut value: u32;

    loop {
        if !c.is_ascii_digit() {
            return 0xffff_ffff;
        }

        value = 0;
        let mut base = 10u32;
        if c == b'0' {
            idx += 1;
            c = *bytes.get(idx).unwrap_or(&0);
            if c == b'x' || c == b'X' {
                base = 16;
                idx += 1;
                c = *bytes.get(idx).unwrap_or(&0);
            } else {
                base = 8;
            }
        }

        loop {
            if c.is_ascii_digit() {
                if base == 8 && c >= b'8' {
                    return 0xffff_ffff;
                }
                let Some(next) = value
                    .checked_mul(base)
                    .and_then(|value| value.checked_add((c - b'0') as u32))
                else {
                    return 0xffff_ffff;
                };
                value = next;
                idx += 1;
                c = *bytes.get(idx).unwrap_or(&0);
            } else if base == 16 && c.is_ascii_hexdigit() {
                let digit = if c.is_ascii_digit() {
                    c - b'0'
                } else if c.is_ascii_lowercase() {
                    c - b'a' + 10
                } else {
                    c - b'A' + 10
                };
                let Some(next) = value
                    .checked_mul(base)
                    .and_then(|value| value.checked_add(digit as u32))
                else {
                    return 0xffff_ffff;
                };
                value = next;
                idx += 1;
                c = *bytes.get(idx).unwrap_or(&0);
            } else {
                break;
            }
        }

        if c == b'.' {
            if num_parts > 3 || value > 0xff {
                return 0xffff_ffff;
            }
            parts[num_parts] = value;
            num_parts += 1;
            idx += 1;
            c = *bytes.get(idx).unwrap_or(&0);
        } else {
            break;
        }
    }

    if c != 0 && !c.is_ascii_whitespace() {
        return 0xffff_ffff;
    }
    if c.is_ascii_whitespace() && bytes[idx..].iter().any(|byte| !byte.is_ascii_whitespace()) {
        return 0xffff_ffff;
    }

    match num_parts {
        0 => {}
        1 => {
            if value > 0x00ff_ffff {
                return 0xffff_ffff;
            }
            value |= parts[0] << 24;
        }
        2 => {
            if value > 0x0000_ffff {
                return 0xffff_ffff;
            }
            value |= parts[0] << 24;
            value |= parts[1] << 16;
        }
        3 => {
            if value > 0x0000_00ff {
                return 0xffff_ffff;
            }
            value |= parts[0] << 24;
            value |= parts[1] << 16;
            value |= parts[2] << 8;
        }
        _ => return 0xffff_ffff,
    }

    value
}

#[cfg(target_arch = "powerpc")]
fn write_ipv4_string(dst: *mut c_char, addr: u32) {
    if dst.is_null() {
        return;
    }
    let octets = [
        ip4_addr1(addr),
        ip4_addr2(addr),
        ip4_addr3(addr),
        ip4_addr4(addr),
    ];
    let mut out = [0u8; 16];
    let mut idx = 0usize;
    for (octet_idx, octet) in octets.iter().enumerate() {
        if octet_idx != 0 {
            out[idx] = b'.';
            idx += 1;
        }
        idx += if *octet >= 100 {
            out[idx] = b'0' + (*octet / 100) as u8;
            out[idx + 1] = b'0' + ((*octet / 10) % 10) as u8;
            out[idx + 2] = b'0' + (*octet % 10) as u8;
            3
        } else if *octet >= 10 {
            out[idx] = b'0' + (*octet / 10) as u8;
            out[idx + 1] = b'0' + (*octet % 10) as u8;
            2
        } else {
            out[idx] = b'0' + *octet as u8;
            1
        };
    }
    out[idx] = 0;
    for (offset, byte) in out[..=idx].iter().copied().enumerate() {
        unsafe {
            *dst.add(offset) = byte as c_char;
        }
    }
}

#[cfg(target_arch = "powerpc")]
mod wii_backend {
    use super::*;
    use alloc::boxed::Box;
    use alloc::vec::Vec;
    use core::arch::asm;
    use core::mem::size_of;
    use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, Ordering};
    use lwip_sys as lwip;
    use ogc_ipc::{FileDescriptor, IoctlBuffers, IoctlvEntry, open_file};
    use ogc_lwp_driver as lwp;

    static NET_STATUS: AtomicI32 = AtomicI32::new(INITIAL_NET_STATUS);
    static NET_IP_TOP_FD: AtomicI32 = AtomicI32::new(-1);
    static NET_INIT_LOCK: AtomicBool = AtomicBool::new(false);
    static NET_INIT_ABORT: AtomicBool = AtomicBool::new(false);
    static NET_DEINIT_LOCK: AtomicBool = AtomicBool::new(false);
    static NET_INIT_ENTRY_LOCK: AtomicBool = AtomicBool::new(false);
    static NETWORK_LEASES: AtomicU32 = AtomicU32::new(0);
    static NET_ASYNC_THREAD: AtomicU32 = AtomicU32::new(u32::MAX);
    static NET_ASYNC_REAP_PENDING: AtomicU32 = AtomicU32::new(u32::MAX);
    const NET_ASYNC_STACK_SIZE: u32 = 8 * 1024;
    const NET_ASYNC_PRIORITY: u8 = 105;
    const MANAGE_PATH: &[u8] = b"/dev/net/ncd/manage\0";
    const IP_TOP_PATH: &[u8] = b"/dev/net/ip/top\0";
    const WC24_REQUEST_PATH: &[u8] = b"/dev/net/kd/request\0";
    const IOCTL_NWC24_STARTUP: i32 = 0x06;
    const IOCTL_NCD_GETLINKSTATUS: i32 = 0x07;
    const IOCTLV_SO_GETINTERFACEOPT: i32 = 0x1c;
    const SO_GETINTERFACEOPT_LEVEL: u32 = 0x0000_fffe;
    const SO_GETINTERFACEOPT_IPCONFIG: u32 = 0x4003;
    const SO_GETINTERFACEOPT_ROUTING_SIZE: u32 = 0x4005;
    const SO_GETINTERFACEOPT_ROUTING: u32 = 0x4006;
    const IOCTL_SO_GETHOSTID: i32 = 0x10;
    const IOCTL_SO_STARTUP: i32 = 0x1f;
    const IOCTL_SO_POLL: i32 = 0x0b;
    const GETHOSTBYNAME_IOCTL: i32 = 0x11;
    const WC24_CLEANUP_PATH: &[u8] = b"/dev/net/kd/request\0";
    const WC24_CLEANUP_IOCTL: i32 = 7;
    const MAX_TRACKED_SOCKETS: usize = 128;
    const MAX_INIT_RETRIES: usize = 32;
    const NET_INIT_RETRY_DELAY_MS: u64 = 50;
    const NET_STARTUP_RETRY_DELAY_MS: u64 = 100;
    const MAX_NWC24_STARTUP_RETRIES: usize = 32;
    const TB_TICKS_PER_MILLISECOND: u64 = 60_750;
    const ENXIO: i32 = 6;
    const EAGAIN: i32 = 11;
    const EBUSY: i32 = 16;
    const ENETDOWN: i32 = 100;
    pub(super) const INITIAL_NET_STATUS: i32 = -ENETDOWN;
    const ETIMEDOUT: i32 = 110;
    static SOCKET_NONBLOCK: [AtomicBool; MAX_TRACKED_SOCKETS] =
        [const { AtomicBool::new(false) }; MAX_TRACKED_SOCKETS];
    static WC24_CLEANUP_LOCK: AtomicBool = AtomicBool::new(false);

    #[repr(align(32))]
    struct HostEntBuffer([u8; 0x460]);

    #[repr(align(32))]
    struct Wc24CleanupBuffer([u8; 0x20]);

    #[repr(align(32))]
    struct NetworkStartupBuffer([u8; 0x20]);

    struct AsyncInitContext {
        callback: Netcallback,
        usrdata: *mut c_void,
    }

    #[repr(C)]
    #[derive(Copy, Clone, Default)]
    struct GetInterfaceOptParams {
        magic: u32,
        option: u32,
    }

    #[repr(C)]
    #[derive(Copy, Clone, Default)]
    struct GetInterfaceOptIpConfig {
        inet: u32,
        subnet_mask: u32,
        broadcast_ip: u32,
    }

    #[repr(C)]
    #[derive(Copy, Clone, Default)]
    struct GetInterfaceOptIpRoute {
        destination: u32,
        netmask: u32,
        gateway: u32,
        flags: u32,
        ticks: u64,
    }

    #[repr(align(32))]
    struct AlignedU32(u32);

    #[repr(align(32))]
    struct Aligned<T>(T);

    const _: () = assert!(size_of::<GetInterfaceOptParams>() == 8);
    const _: () = assert!(size_of::<GetInterfaceOptIpConfig>() == 12);
    const _: () = assert!(size_of::<GetInterfaceOptIpRoute>() == 24);

    static mut HOSTENT_BUFFER: HostEntBuffer = HostEntBuffer([0; 0x460]);
    static mut WC24_CLEANUP_BUFFER: Wc24CleanupBuffer = Wc24CleanupBuffer([0; 0x20]);

    fn wait_network_startup_retry() {
        let delay = NET_STARTUP_RETRY_DELAY_MS * TB_TICKS_PER_MILLISECOND;
        lwp::sleep_ticks(delay);
    }

    pub(super) fn net_gethostip_with(
        fd: i32,
        mut query: impl FnMut(i32) -> i32,
        mut wait: impl FnMut(),
    ) -> u32 {
        if fd < 0 {
            return 0;
        }
        let mut ip_addr = 0;
        for _ in 0..5 {
            ip_addr = query(fd) as u32;
            if ip_addr != 0 {
                break;
            }
            wait();
        }
        ip_addr
    }

    struct CleanupGuard;

    struct NetDeinitGuard;

    struct NetInitEntryGuard;

    impl NetInitEntryGuard {
        fn lock() -> Self {
            while NET_INIT_ENTRY_LOCK
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                core::hint::spin_loop();
            }
            Self
        }
    }

    impl Drop for NetInitEntryGuard {
        fn drop(&mut self) {
            NET_INIT_ENTRY_LOCK.store(false, Ordering::Release);
        }
    }

    impl NetDeinitGuard {
        fn lock() -> Self {
            loop {
                let entry_guard = NetInitEntryGuard::lock();
                if !NET_DEINIT_LOCK.load(Ordering::Acquire) {
                    NET_DEINIT_LOCK.store(true, Ordering::Release);
                    drop(entry_guard);
                    return Self;
                }
                drop(entry_guard);
                core::hint::spin_loop();
            }
        }
    }

    impl Drop for NetDeinitGuard {
        fn drop(&mut self) {
            NET_DEINIT_LOCK.store(false, Ordering::Release);
        }
    }

    enum NetInitStart {
        AlreadyInitialized,
        Started,
    }

    fn begin_net_init() -> Result<NetInitStart, ()> {
        let _entry_guard = NetInitEntryGuard::lock();
        if NET_DEINIT_LOCK.load(Ordering::Acquire) {
            return Err(());
        }
        if NET_IP_TOP_FD.load(Ordering::Acquire) >= 0 {
            return Ok(NetInitStart::AlreadyInitialized);
        }
        if NET_INIT_LOCK
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(());
        }
        NET_INIT_ABORT.store(false, Ordering::Release);
        Ok(NetInitStart::Started)
    }

    fn wait_for_init_completion(
        is_busy: impl Fn() -> bool,
        request_abort: impl FnOnce(),
        mut wait: impl FnMut(),
    ) {
        if is_busy() {
            request_abort();
            while is_busy() {
                wait();
            }
        }
    }

    fn run_cancellable_init_step<T>(
        is_aborted: impl Fn() -> bool,
        operation: impl FnOnce() -> Result<T, i32>,
        cleanup_cancelled: impl FnOnce(T),
    ) -> Result<T, i32> {
        if is_aborted() {
            return Err(-ENETDOWN);
        }
        match operation() {
            Ok(value) if is_aborted() => {
                cleanup_cancelled(value);
                Err(-ENETDOWN)
            }
            Err(_) if is_aborted() => Err(-ENETDOWN),
            result => result,
        }
    }

    impl CleanupGuard {
        fn lock() -> Self {
            while WC24_CLEANUP_LOCK
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_err()
            {
                core::hint::spin_loop();
            }
            Self
        }
    }

    impl Drop for CleanupGuard {
        fn drop(&mut self) {
            WC24_CLEANUP_LOCK.store(false, Ordering::Release);
        }
    }

    unsafe fn bytes_of<T>(value: &T) -> &[u8] {
        unsafe { core::slice::from_raw_parts((value as *const T).cast::<u8>(), size_of::<T>()) }
    }

    unsafe fn bytes_of_mut<T>(value: &mut T) -> &mut [u8] {
        unsafe { core::slice::from_raw_parts_mut((value as *mut T).cast::<u8>(), size_of::<T>()) }
    }

    #[inline]
    fn read_timebase() -> u64 {
        loop {
            let upper1: u32;
            let lower: u32;
            let upper2: u32;
            unsafe {
                asm!(
                    "mftbu {upper1}",
                    "mftb {lower}",
                    "mftbu {upper2}",
                    upper1 = out(reg) upper1,
                    lower = out(reg) lower,
                    upper2 = out(reg) upper2,
                    options(nostack, preserves_flags)
                );
            }
            if upper1 == upper2 {
                return ((upper1 as u64) << 32) | lower as u64;
            }
        }
    }

    fn wait_before_retry() {
        let start = read_timebase();
        let delay = NET_INIT_RETRY_DELAY_MS * TB_TICKS_PER_MILLISECOND;
        while read_timebase().wrapping_sub(start) < delay {
            core::hint::spin_loop();
        }
    }

    fn retry_net_init(
        _max_retries: i32,
        mut initialize: impl FnMut() -> i32,
        mut delay: impl FnMut(),
    ) -> i32 {
        // BUG FIX: C ignores max_retries, uses 32 attempts, and delays after
        // every retryable result, including the result from the final attempt.
        let mut result = SOCKET_ERROR;
        for _ in 0..MAX_INIT_RETRIES {
            result = initialize();
            if result != -EAGAIN && result != -ETIMEDOUT {
                break;
            }
            delay();
        }
        result
    }

    fn track_idx(socket: i32) -> Option<usize> {
        if socket < 0 {
            return None;
        }
        let idx = socket as usize;
        (idx < MAX_TRACKED_SOCKETS).then_some(idx)
    }

    fn finish_file_operation(fd: FileDescriptor, operation_result: i32) -> i32 {
        super::ipc_cleanup::finish(operation_result, fd.close())
    }

    pub fn net_init() -> i32 {
        match begin_net_init() {
            Ok(NetInitStart::AlreadyInitialized) => return 0,
            Ok(NetInitStart::Started) => {}
            Err(()) => return -EAGAIN,
        }
        reap_pending_async_init_thread();

        let result = net_init_locked();
        NET_INIT_LOCK.store(false, Ordering::Release);
        result
    }

    fn net_init_locked() -> i32 {
        if NET_IP_TOP_FD.load(Ordering::Acquire) >= 0 {
            NET_STATUS.store(0, Ordering::Release);
            return 0;
        }
        let result = net_init_sequence();
        NET_STATUS.store(result, Ordering::Release);
        result
    }

    /// Attach this process to an IOS network stack that is already running.
    /// This skips NWC24 startup, which can block or fail when another HBC
    /// process started the same service before launching this DOL.
    pub fn net_attach_active() -> i32 {
        match begin_net_init() {
            Ok(NetInitStart::AlreadyInitialized) => return 0,
            Ok(NetInitStart::Started) => {}
            Err(()) => return -EAGAIN,
        }
        reap_pending_async_init_thread();
        let result = net_attach_active_sequence();
        NET_STATUS.store(result, Ordering::Release);
        NET_INIT_LOCK.store(false, Ordering::Release);
        result
    }

    fn net_attach_active_sequence() -> i32 {
        let Ok(path) = CStr::from_bytes_with_nul(IP_TOP_PATH) else {
            return SOCKET_ERROR;
        };
        let ip_top = match run_cancellable_init_step(
            || NET_INIT_ABORT.load(Ordering::Acquire),
            || open_file(path, 0),
            |fd| {
                super::ipc_cleanup::after_failure(fd.close());
            },
        ) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };
        // An HBC launch does not guarantee that the new IOS descriptor already
        // has an address. Start the socket interface only when its host ID is zero.
        let ready = run_cancellable_init_step(
            || NET_INIT_ABORT.load(Ordering::Acquire),
            || Ok(ip_top.ioctl(IOCTL_SO_GETHOSTID, IoctlBuffers::empty())),
            |_| {},
        )
        .and_then(|host_id| {
            active_attach_ip(host_id, -ETIMEDOUT, || startup_interface_ip(&ip_top))
        });
        if let Err(error) = ready {
            return finish_file_operation(ip_top, error);
        }
        if NET_INIT_ABORT.load(Ordering::Acquire) {
            return finish_file_operation(ip_top, -ENETDOWN);
        }
        let raw_fd = ip_top.raw();
        lwip::set_network_top_fd(raw_fd);
        NET_IP_TOP_FD.store(raw_fd, Ordering::Release);
        0
    }

    fn startup_interface_ip(ip_top: &FileDescriptor) -> Result<i32, i32> {
        // C repeats SO_STARTUP before every host-ID query while DHCP is pending.
        startup_network_ip(
            -ETIMEDOUT,
            || {
                run_cancellable_init_step(
                    || NET_INIT_ABORT.load(Ordering::Acquire),
                    || Ok(ip_top.ioctl(IOCTL_SO_STARTUP, IoctlBuffers::empty())),
                    |_| {},
                )
            },
            || {
                run_cancellable_init_step(
                    || NET_INIT_ABORT.load(Ordering::Acquire),
                    || Ok(ip_top.ioctl(IOCTL_SO_GETHOSTID, IoctlBuffers::empty())),
                    |_| {},
                )
            },
            wait_network_startup_retry,
        )
    }

    fn net_init_sequence() -> i32 {
        let Ok(manage_path) = CStr::from_bytes_with_nul(MANAGE_PATH) else {
            return SOCKET_ERROR;
        };
        let manage = loop {
            match run_cancellable_init_step(
                || NET_INIT_ABORT.load(Ordering::Acquire),
                || open_file(manage_path, 0),
                |fd| {
                    super::ipc_cleanup::after_failure(fd.close());
                },
            ) {
                Ok(fd) => break fd,
                Err(ogc_ipc::IPC_ENOENT) => wait_network_startup_retry(),
                Err(rc) => return rc,
            }
        };
        let mut link_status = NetworkStartupBuffer([0; 0x20]);
        let link = match run_cancellable_init_step(
            || NET_INIT_ABORT.load(Ordering::Acquire),
            || {
                Ok(manage.ioctlv(
                    IOCTL_NCD_GETLINKSTATUS,
                    0,
                    [IoctlvEntry::Out(&mut link_status.0)],
                ))
            },
            |_| {},
        ) {
            Ok(result) => finish_file_operation(manage, result),
            Err(rc) => {
                super::ipc_cleanup::after_failure(manage.close());
                return rc;
            }
        };
        if link < 0 {
            return link;
        }

        let Ok(ip_top_path) = CStr::from_bytes_with_nul(IP_TOP_PATH) else {
            return SOCKET_ERROR;
        };
        let ip_top = match run_cancellable_init_step(
            || NET_INIT_ABORT.load(Ordering::Acquire),
            || open_file(ip_top_path, 0),
            |fd| {
                super::ipc_cleanup::after_failure(fd.close());
            },
        ) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };
        let Ok(wc24_path) = CStr::from_bytes_with_nul(WC24_REQUEST_PATH) else {
            super::ipc_cleanup::after_failure(ip_top.close());
            return SOCKET_ERROR;
        };
        let wc24 = match run_cancellable_init_step(
            || NET_INIT_ABORT.load(Ordering::Acquire),
            || open_file(wc24_path, 0),
            |fd| {
                super::ipc_cleanup::after_failure(fd.close());
            },
        ) {
            Ok(fd) => fd,
            Err(rc) => {
                super::ipc_cleanup::after_failure(ip_top.close());
                return rc;
            }
        };

        let mut wc24_result = SOCKET_ERROR;
        for attempt in 0..MAX_NWC24_STARTUP_RETRIES {
            let mut response = NetworkStartupBuffer([0; 0x20]);
            let rc = match run_cancellable_init_step(
                || NET_INIT_ABORT.load(Ordering::Acquire),
                || {
                    Ok(wc24.ioctl(
                        IOCTL_NWC24_STARTUP,
                        IoctlBuffers::from_slices(None, Some(&mut response.0)),
                    ))
                },
                |_| {},
            ) {
                Ok(rc) => rc,
                Err(rc) => {
                    super::ipc_cleanup::after_failure(wc24.close());
                    super::ipc_cleanup::after_failure(ip_top.close());
                    return rc;
                }
            };
            if rc < 0 {
                wc24_result = rc;
                break;
            }
            wc24_result = i32::from_be_bytes(response.0[..4].try_into().unwrap_or([0; 4]));
            if wc24_result == -15 {
                wc24_result = 0;
            }
            if wc24_result != -29 || attempt + 1 == MAX_NWC24_STARTUP_RETRIES {
                break;
            }
            wait_network_startup_retry();
        }
        let wc24_result = finish_file_operation(wc24, wc24_result);
        if NET_INIT_ABORT.load(Ordering::Acquire) {
            super::ipc_cleanup::after_failure(ip_top.close());
            return -ENETDOWN;
        }
        if wc24_result < 0 {
            super::ipc_cleanup::after_failure(ip_top.close());
            return wc24_result;
        }

        if let Err(rc) = startup_interface_ip(&ip_top) {
            return finish_file_operation(ip_top, rc);
        }

        if NET_INIT_ABORT.load(Ordering::Acquire) {
            super::ipc_cleanup::after_failure(ip_top.close());
            return -ENETDOWN;
        }

        let raw_fd = ip_top.raw();
        lwip::set_network_top_fd(raw_fd);
        NET_IP_TOP_FD.store(raw_fd, Ordering::Release);
        0
    }

    unsafe fn deliver_async_init_callback(
        callback: Netcallback,
        result: i32,
        usrdata: *mut c_void,
    ) {
        if let Some(callback) = callback {
            unsafe {
                let _ = callback(result, usrdata);
            }
        }
    }

    #[cfg(test)]
    mod async_init_tests {
        use super::*;

        struct CallbackCapture {
            result: i32,
            userdata_seen: bool,
        }

        unsafe extern "C" fn capture_callback(result: i32, usrdata: *mut c_void) -> i32 {
            let capture = unsafe { &mut *usrdata.cast::<CallbackCapture>() };
            capture.result = result;
            capture.userdata_seen = true;
            99
        }

        #[test]
        fn async_completion_forwards_result_and_userdata_and_ignores_callback_return() {
            let mut capture = CallbackCapture {
                result: 0,
                userdata_seen: false,
            };
            unsafe {
                deliver_async_init_callback(
                    Some(capture_callback),
                    -110,
                    (&mut capture as *mut CallbackCapture).cast(),
                );
            }
            assert_eq!(capture.result, -110);
            assert!(capture.userdata_seen);
        }

        #[test]
        fn cancellation_after_an_init_step_cleans_its_result_and_skips_later_steps() {
            let aborted = core::cell::Cell::new(false);
            let calls = core::cell::Cell::new(0);
            let cleaned = core::cell::Cell::new(None);

            let first = run_cancellable_init_step(
                || aborted.get(),
                || {
                    calls.set(calls.get() + 1);
                    aborted.set(true);
                    Ok(17)
                },
                |resource| cleaned.set(Some(resource)),
            );
            assert_eq!(first, Err(-ENETDOWN));
            assert_eq!(cleaned.get(), Some(17));

            let later = run_cancellable_init_step(
                || aborted.get(),
                || {
                    calls.set(calls.get() + 1);
                    Ok(())
                },
                |_| {},
            );
            assert_eq!(later, Err(-ENETDOWN));
            assert_eq!(calls.get(), 1);
        }

        #[test]
        fn deinit_requests_abort_and_waits_until_init_exits() {
            let busy = core::cell::Cell::new(true);
            let abort_requested = core::cell::Cell::new(false);
            let waits = core::cell::Cell::new(0);

            wait_for_init_completion(
                || busy.get(),
                || abort_requested.set(true),
                || {
                    assert!(abort_requested.get());
                    waits.set(waits.get() + 1);
                    busy.set(false);
                },
            );

            assert!(abort_requested.get());
            assert_eq!(waits.get(), 1);
            assert!(!busy.get());
        }
    }

    unsafe fn reap_async_init_thread(arg: *mut c_void) {
        let thread = arg as usize as u32;
        let mut result = core::ptr::null_mut();
        let _ = unsafe { lwp::join_thread(thread, &mut result) };
    }

    fn reap_pending_async_init_thread() {
        let thread = NET_ASYNC_REAP_PENDING.swap(u32::MAX, Ordering::AcqRel);
        if thread != u32::MAX {
            let mut result = core::ptr::null_mut();
            let _ = unsafe { lwp::join_thread(thread, &mut result) };
        }
    }

    unsafe extern "C" fn net_init_async_worker(arg: *mut c_void) -> *mut c_void {
        let context = unsafe { Box::from_raw(arg.cast::<AsyncInitContext>()) };
        let result = net_init_locked();
        unsafe { deliver_async_init_callback(context.callback, result, context.usrdata) };

        let thread = loop {
            let thread = NET_ASYNC_THREAD.load(Ordering::Acquire);
            if thread != u32::MAX {
                break thread;
            }
            lwp::yield_thread();
        };
        drop(context);
        let enqueue_result = unsafe {
            lwp::deferred_enqueue(Some(reap_async_init_thread), thread as usize as *mut c_void)
        };
        if enqueue_result != lwp::LWP_SUCCESSFUL {
            NET_ASYNC_REAP_PENDING.store(thread, Ordering::Release);
        }
        NET_INIT_LOCK.store(false, Ordering::Release);
        core::ptr::null_mut()
    }

    /// # Safety
    ///
    /// `usrdata` must be null where permitted, or properly aligned and valid for all required reads and writes.
    /// Buffer extents must cover the supplied lengths, counts, and offsets,
    /// and the referenced memory must obey Rust's aliasing requirements.
    /// All referenced objects must remain valid and unmoved until the
    /// asynchronous operation and its callback have completed.
    pub unsafe fn net_init_async(cb: Netcallback, usrdata: *mut c_void) -> i32 {
        match begin_net_init() {
            Ok(NetInitStart::AlreadyInitialized) => return 0,
            Ok(NetInitStart::Started) => {}
            Err(()) => return -EBUSY,
        }
        reap_pending_async_init_thread();

        NET_STATUS.store(-EBUSY, Ordering::Release);
        let context = Box::new(AsyncInitContext {
            callback: cb,
            usrdata,
        });
        let context_ptr = Box::into_raw(context).cast::<c_void>();
        NET_ASYNC_THREAD.store(u32::MAX, Ordering::Release);
        let mut thread = lwp::LWP_THREAD_NULL;
        let result = unsafe {
            lwp::create_thread(
                &mut thread,
                Some(net_init_async_worker),
                context_ptr,
                core::ptr::null_mut(),
                NET_ASYNC_STACK_SIZE,
                NET_ASYNC_PRIORITY,
            )
        };
        if result != lwp::LWP_SUCCESSFUL {
            unsafe {
                drop(Box::from_raw(context_ptr.cast::<AsyncInitContext>()));
            }
            NET_STATUS.store(result, Ordering::Release);
            NET_INIT_LOCK.store(false, Ordering::Release);
            return result;
        }
        NET_ASYNC_THREAD.store(thread, Ordering::Release);
        0
    }

    pub fn net_get_status() -> i32 {
        NET_STATUS.load(Ordering::Acquire)
    }

    pub fn net_wc24cleanup() -> i32 {
        let _guard = CleanupGuard::lock();
        let Ok(path) = core::ffi::CStr::from_bytes_with_nul(WC24_CLEANUP_PATH) else {
            return SOCKET_ERROR;
        };
        let fd = match open_file(path, 0) {
            Ok(fd) => fd,
            Err(rc) => return rc,
        };
        let cleanup = unsafe {
            core::slice::from_raw_parts_mut(
                core::ptr::addr_of_mut!(WC24_CLEANUP_BUFFER.0).cast::<u8>(),
                0x20,
            )
        };
        cleanup.fill(0);
        let result = fd.ioctl(
            WC24_CLEANUP_IOCTL,
            IoctlBuffers::from_slices(None, Some(cleanup)),
        );
        finish_file_operation(fd, result)
    }

    /// # Safety
    ///
    /// `mac_buf` must be null where permitted, or properly aligned and valid for all required reads and writes.
    /// Buffer extents must cover the supplied lengths, counts, and offsets,
    /// and the referenced memory must obey Rust's aliasing requirements.
    /// The pointers must remain valid for the duration of the call.
    pub unsafe fn net_get_mac_address(mac_buf: *mut c_void) -> i32 {
        unsafe {
            if mac_buf.is_null() {
                return SOCKET_ERROR;
            }
            let Ok(path) = core::ffi::CStr::from_bytes_with_nul(MANAGE_PATH) else {
                return SOCKET_ERROR;
            };
            let fd = match open_file(path, 0) {
                Ok(fd) => fd,
                Err(rc) => return rc,
            };
            let mut manage_buf = [0u32; 8];
            let manage_bytes = core::slice::from_raw_parts_mut(
                manage_buf.as_mut_ptr().cast::<u8>(),
                core::mem::size_of_val(&manage_buf),
            );
            let mut mac = [0u8; 6];
            let result = fd.ioctlv(
                0x08,
                0,
                [IoctlvEntry::Out(manage_bytes), IoctlvEntry::Out(&mut mac)],
            );
            let result = finish_file_operation(fd, result);
            if result < 0 {
                return result;
            }
            if manage_buf[0] != 0 {
                return manage_buf[0] as i32;
            }
            core::ptr::copy_nonoverlapping(mac.as_ptr(), mac_buf.cast::<u8>(), mac.len());
            result
        }
    }

    pub fn net_deinit() -> i32 {
        let _deinit_guard = NetDeinitGuard::lock();
        if NETWORK_LEASES.load(Ordering::Acquire) != 0 {
            return -EBUSY;
        }
        wait_for_init_completion(
            || NET_INIT_LOCK.load(Ordering::Acquire),
            || NET_INIT_ABORT.store(true, Ordering::Release),
            lwp::yield_thread,
        );
        reap_pending_async_init_thread();

        let top_fd = NET_IP_TOP_FD.swap(-1, Ordering::AcqRel);
        lwip::set_network_top_fd(-1);
        let close_result = if top_fd >= 0 {
            finish_file_operation(FileDescriptor::new(top_fd), 0)
        } else {
            0
        };
        NET_STATUS.store(-ENETDOWN, Ordering::Release);
        close_result
    }

    pub fn _net_convert_error(ios_retval: i32) -> i32 {
        ios_retval
    }

    #[cfg(not(gamecube))]
    pub(super) fn lease_acquire() -> Result<FileDescriptor, i32> {
        lwp::current_thread_checked().map_err(|_| -EBUSY)?;
        let _guard = NetInitEntryGuard::lock();
        if NET_DEINIT_LOCK.load(Ordering::Acquire) {
            return Err(-EBUSY);
        }
        let fd = NET_IP_TOP_FD.load(Ordering::Acquire);
        if fd < 0 {
            return Err(-ENETDOWN);
        }
        let count = NETWORK_LEASES.load(Ordering::Relaxed);
        let next = count.checked_add(1).ok_or(-EBUSY)?;
        NETWORK_LEASES.store(next, Ordering::Release);
        Ok(FileDescriptor::new(fd))
    }

    #[cfg(not(gamecube))]
    pub(super) fn lease_release() {
        let _guard = NetInitEntryGuard::lock();
        let count = NETWORK_LEASES.load(Ordering::Relaxed);
        assert!(count > 0, "network lease underflow");
        NETWORK_LEASES.store(count - 1, Ordering::Release);
    }

    pub fn net_gethostip() -> u32 {
        let fd = NET_IP_TOP_FD.load(Ordering::Acquire);
        net_gethostip_with(
            fd,
            |fd| FileDescriptor::new(fd).ioctl(IOCTL_SO_GETHOSTID, IoctlBuffers::empty()),
            wait_network_startup_retry,
        )
    }

    /// # Safety
    ///
    /// `_addr_string` must be null or point to a readable NUL-terminated C string.
    /// Buffer extents must cover the supplied lengths, counts, and offsets,
    /// and the referenced memory must obey Rust's aliasing requirements.
    /// The pointers must remain valid for the duration of the call.
    pub unsafe fn net_gethostbyname(_addr_string: *const c_char) -> *mut HostEnt {
        unsafe {
            if _addr_string.is_null() {
                return core::ptr::null_mut();
            }
            let bytes = CStr::from_ptr(_addr_string).to_bytes();
            if bytes.is_empty() || bytes.len() + 1 > 256 {
                return core::ptr::null_mut();
            }
            let Ok(path) = CStr::from_bytes_with_nul(IP_TOP_PATH) else {
                return core::ptr::null_mut();
            };
            let Ok(fd) = open_file(path, 0) else {
                return core::ptr::null_mut();
            };
            let mut request = [0u8; 256];
            request[..bytes.len()].copy_from_slice(bytes);
            request[bytes.len()] = 0;
            let response_ptr = core::ptr::addr_of_mut!(HOSTENT_BUFFER.0).cast::<u8>();
            let response = core::slice::from_raw_parts_mut(response_ptr, 0x460);
            response.fill(0);
            let rc = fd.ioctl(
                GETHOSTBYNAME_IOCTL,
                IoctlBuffers::from_slices(Some(&mut request[..=bytes.len()]), Some(&mut *response)),
            );
            let rc = finish_file_operation(fd, rc);
            if rc < 0 {
                return core::ptr::null_mut();
            }

            let ip_data = response.as_mut_ptr().cast::<HostEnt>();
            let base = response.as_mut_ptr() as usize;
            let end = base + response.len();
            let name_ptr = (*ip_data).h_name;
            if name_ptr.is_null() {
                return core::ptr::null_mut();
            }
            let Some(addr_offset) =
                (mem_physical_to_k0(name_ptr as usize as u32) as usize).checked_sub(base + 0x10)
            else {
                return core::ptr::null_mut();
            };
            let relocate = |pointer: usize| -> Option<usize> {
                if pointer == 0 {
                    return Some(0);
                }
                let relocated =
                    (mem_physical_to_k0(pointer as u32) as usize).checked_sub(addr_offset)?;
                (relocated >= base && relocated < end).then_some(relocated)
            };
            {
                let Some(name) = relocate((*ip_data).h_name as usize) else {
                    return core::ptr::null_mut();
                };
                (*ip_data).h_name = name as *mut c_char;

                let Some(aliases) = relocate((*ip_data).h_aliases as usize) else {
                    return core::ptr::null_mut();
                };
                (*ip_data).h_aliases = aliases as *mut *mut c_char;
                let mut idx = 0usize;
                while !(*ip_data).h_aliases.is_null() && idx < 0x40 {
                    if ((*ip_data).h_aliases as usize)
                        .checked_add((idx + 1) * core::mem::size_of::<*mut c_char>())
                        .is_none_or(|slot_end| slot_end > end)
                    {
                        return core::ptr::null_mut();
                    }
                    let alias = *(*ip_data).h_aliases.add(idx);
                    if alias.is_null() {
                        break;
                    }
                    let Some(alias) = relocate(alias as usize) else {
                        return core::ptr::null_mut();
                    };
                    *(*ip_data).h_aliases.add(idx) = alias as *mut c_char;
                    idx += 1;
                }

                let Some(addresses) = relocate((*ip_data).h_addr_list as usize) else {
                    return core::ptr::null_mut();
                };
                (*ip_data).h_addr_list = addresses as *mut *mut c_char;
                idx = 0;
                while !(*ip_data).h_addr_list.is_null() && idx < 0x40 {
                    if ((*ip_data).h_addr_list as usize)
                        .checked_add((idx + 1) * core::mem::size_of::<*mut c_char>())
                        .is_none_or(|slot_end| slot_end > end)
                    {
                        return core::ptr::null_mut();
                    }
                    let addr = *(*ip_data).h_addr_list.add(idx);
                    if addr.is_null() {
                        break;
                    }
                    let Some(addr) = relocate(addr as usize) else {
                        return core::ptr::null_mut();
                    };
                    if (*ip_data).h_length < 0
                        || addr
                            .checked_add((*ip_data).h_length as usize)
                            .is_none_or(|addr_end| addr_end > end)
                    {
                        return core::ptr::null_mut();
                    }
                    *(*ip_data).h_addr_list.add(idx) = addr as *mut c_char;
                    idx += 1;
                }
            }
            ip_data
        }
    }

    /// # Safety
    ///
    /// `cp` must be null where permitted, or properly aligned and valid for all required reads.
    /// Buffer extents must cover the supplied lengths, counts, and offsets,
    /// and the referenced memory must obey Rust's aliasing requirements.
    /// The pointers must remain valid for the duration of the call.
    pub unsafe fn inet_addr(cp: *const c_char) -> u32 {
        parse_ipv4(cp)
    }

    /// # Safety
    ///
    /// `name` must be null where permitted, or properly aligned and valid for all required reads and writes.
    /// Buffer extents must cover the supplied lengths, counts, and offsets,
    /// and the referenced memory must obey Rust's aliasing requirements.
    /// The pointers must remain valid for the duration of the call.
    pub unsafe fn net_bind(s: i32, name: *mut Sockaddr, namelen: SockLenT) -> i32 {
        unsafe { lwip::bind(s, name.cast(), namelen) }
    }

    pub fn net_listen(s: i32, backlog: u32) -> i32 {
        lwip::listen(s, backlog as i32)
    }

    /// # Safety
    ///
    /// `addr` must be null where permitted, or properly aligned and valid for all required reads and writes.
    /// Additionally, `addrlen` must be null where permitted, or properly aligned and valid for all required reads and writes.
    /// Buffer extents must cover the supplied lengths, counts, and offsets,
    /// and the referenced memory must obey Rust's aliasing requirements.
    /// The pointers must remain valid for the duration of the call.
    pub unsafe fn net_accept(s: i32, addr: *mut Sockaddr, addrlen: *mut SockLenT) -> i32 {
        unsafe { lwip::accept(s, addr.cast(), addrlen) }
    }

    /// # Safety
    ///
    /// `addr` must be null where permitted, or properly aligned and valid for all required reads and writes.
    /// Buffer extents must cover the supplied lengths, counts, and offsets,
    /// and the referenced memory must obey Rust's aliasing requirements.
    /// The pointers must remain valid for the duration of the call.
    pub unsafe fn net_connect(s: i32, addr: *mut Sockaddr, addrlen: SockLenT) -> i32 {
        unsafe { lwip::connect(s, addr.cast(), addrlen) }
    }

    /// # Safety
    ///
    /// `data` must be null where permitted, or properly aligned and valid for all required reads.
    /// Buffer extents must cover the supplied lengths, counts, and offsets,
    /// and the referenced memory must obey Rust's aliasing requirements.
    /// The pointers must remain valid for the duration of the call.
    pub unsafe fn net_write(s: i32, data: *const c_void, size: i32) -> i32 {
        unsafe { lwip::write(s, data, size) }
    }

    /// # Safety
    ///
    /// `data` must be null where permitted, or properly aligned and valid for all required reads.
    /// Buffer extents must cover the supplied lengths, counts, and offsets,
    /// and the referenced memory must obey Rust's aliasing requirements.
    /// The pointers must remain valid for the duration of the call.
    pub unsafe fn net_send(s: i32, data: *const c_void, size: i32, flags: u32) -> i32 {
        unsafe { lwip::send(s, data, size, flags) }
    }

    /// # Safety
    ///
    /// `data` must be null where permitted, or properly aligned and valid for all required reads.
    /// Additionally, `to` must be null where permitted, or properly aligned and valid for all required reads and writes.
    /// Buffer extents must cover the supplied lengths, counts, and offsets,
    /// and the referenced memory must obey Rust's aliasing requirements.
    /// The pointers must remain valid for the duration of the call.
    pub unsafe fn net_sendto(
        s: i32,
        data: *const c_void,
        len: i32,
        flags: u32,
        to: *mut Sockaddr,
        tolen: SockLenT,
    ) -> i32 {
        unsafe { lwip::send_to(s, data, len, flags, to.cast(), tolen) }
    }

    /// # Safety
    ///
    /// `mem` must be null where permitted, or properly aligned and valid for all required reads and writes.
    /// Buffer extents must cover the supplied lengths, counts, and offsets,
    /// and the referenced memory must obey Rust's aliasing requirements.
    /// The pointers must remain valid for the duration of the call.
    pub unsafe fn net_recv(s: i32, mem: *mut c_void, len: i32, flags: u32) -> i32 {
        unsafe { lwip::recv(s, mem, len, flags) }
    }

    /// # Safety
    ///
    /// `mem` must be null where permitted, or properly aligned and valid for all required reads and writes.
    /// Additionally, `from` must be null where permitted, or properly aligned and valid for all required reads and writes.
    /// Additionally, `fromlen` must be null where permitted, or properly aligned and valid for all required reads and writes.
    /// Buffer extents must cover the supplied lengths, counts, and offsets,
    /// and the referenced memory must obey Rust's aliasing requirements.
    /// The pointers must remain valid for the duration of the call.
    pub unsafe fn net_recvfrom(
        s: i32,
        mem: *mut c_void,
        len: i32,
        flags: u32,
        from: *mut Sockaddr,
        fromlen: *mut SockLenT,
    ) -> i32 {
        unsafe { lwip::receive_from(s, mem, len, flags, from.cast(), fromlen) }
    }

    /// # Safety
    ///
    /// `mem` must be null where permitted, or properly aligned and valid for all required reads and writes.
    /// Buffer extents must cover the supplied lengths, counts, and offsets,
    /// and the referenced memory must obey Rust's aliasing requirements.
    /// The pointers must remain valid for the duration of the call.
    pub unsafe fn net_read(s: i32, mem: *mut c_void, len: i32) -> i32 {
        unsafe { lwip::read(s, mem, len) }
    }

    /// # Safety
    ///
    /// `optval` must be null where permitted, or properly aligned and valid for all required reads.
    /// Additionally, `optlen` must be null where permitted, or properly aligned and valid for all required reads and writes.
    /// Buffer extents must cover the supplied lengths, counts, and offsets,
    /// and the referenced memory must obey Rust's aliasing requirements.
    /// The pointers must remain valid for the duration of the call.
    pub unsafe fn net_getsockopt(
        s: i32,
        level: u32,
        optname: u32,
        optval: *const c_void,
        optlen: *mut SockLenT,
    ) -> i32 {
        unsafe {
            lwip::get_socket_option(
                s,
                level as i32,
                optname as i32,
                optval as *mut c_void,
                optlen,
            )
        }
    }

    /// # Safety
    ///
    /// `optval` must be null where permitted, or properly aligned and valid for all required reads.
    /// Buffer extents must cover the supplied lengths, counts, and offsets,
    /// and the referenced memory must obey Rust's aliasing requirements.
    /// The pointers must remain valid for the duration of the call.
    pub unsafe fn net_setsockopt(
        s: i32,
        level: u32,
        optname: u32,
        optval: *const c_void,
        optlen: SockLenT,
    ) -> i32 {
        unsafe { lwip::set_socket_option(s, level as i32, optname as i32, optval, optlen) }
    }

    /// # Safety
    ///
    /// `addr` must be null where permitted, or properly aligned and valid for all required reads and writes.
    /// Additionally, `addrlen` must be null where permitted, or properly aligned and valid for all required reads and writes.
    /// Buffer extents must cover the supplied lengths, counts, and offsets,
    /// and the referenced memory must obey Rust's aliasing requirements.
    /// The pointers must remain valid for the duration of the call.
    pub unsafe fn net_getsockname(s: i32, addr: *mut Sockaddr, addrlen: *mut SockLenT) -> i32 {
        // SAFETY: Both length pointer types are 32-bit integers with all bit patterns valid.
        unsafe { lwip::get_socket_name(s, addr.cast(), addrlen) }
    }

    /// # Safety
    ///
    /// `argp` must be null where permitted, or properly aligned and valid for all required reads and writes.
    /// Buffer extents must cover the supplied lengths, counts, and offsets,
    /// and the referenced memory must obey Rust's aliasing requirements.
    /// The pointers must remain valid for the duration of the call.
    pub unsafe fn net_ioctl(s: i32, cmd: u32, argp: *mut c_void) -> i32 {
        unsafe { lwip::ioctl(s, cmd as i64, argp) }
    }

    pub fn net_shutdown(s: i32, how: u32) -> i32 {
        lwip::shutdown(s, how as i32)
    }

    pub fn net_socket(domain: u32, type_: u32, protocol: u32) -> i32 {
        lwip::socket(domain as i32, type_ as i32, protocol as i32)
    }

    pub fn net_close(s: i32) -> i32 {
        if let Some(idx) = track_idx(s) {
            SOCKET_NONBLOCK[idx].store(false, Ordering::Release);
        }
        lwip::close(s)
    }

    /// # Safety
    ///
    /// `readset` must be null where permitted, or properly aligned and valid for all required reads and writes.
    /// Additionally, `writeset` must be null where permitted, or properly aligned and valid for all required reads and writes.
    /// Additionally, `exceptset` must be null where permitted, or properly aligned and valid for all required reads and writes.
    /// Additionally, `timeout` must be null where permitted, or properly aligned and valid for all required reads and writes.
    /// Buffer extents must cover the supplied lengths, counts, and offsets,
    /// and the referenced memory must obey Rust's aliasing requirements.
    /// The pointers must remain valid for the duration of the call.
    pub unsafe fn net_select(
        maxfdp1: i32,
        readset: *mut FdSet,
        writeset: *mut FdSet,
        exceptset: *mut FdSet,
        timeout: *mut Timeval,
    ) -> i32 {
        let mut lwip_timeout = if timeout.is_null() {
            lwip::TimeVal::default()
        } else {
            unsafe {
                lwip::TimeVal {
                    tv_sec: (*timeout).tv_sec,
                    tv_usec: (*timeout).tv_usec,
                }
            }
        };
        let mut lwip_read = lwip::FdSet::default();
        let mut lwip_write = lwip::FdSet::default();
        let mut lwip_except = lwip::FdSet::default();
        if !readset.is_null() {
            lwip_read
                .fd_bits
                .copy_from_slice(unsafe { &(&(*readset).fd_bits)[..lwip::FD_SETSIZE.div_ceil(8)] });
        }
        if !writeset.is_null() {
            lwip_write.fd_bits.copy_from_slice(unsafe {
                &(&(*writeset).fd_bits)[..lwip::FD_SETSIZE.div_ceil(8)]
            });
        }
        if !exceptset.is_null() {
            lwip_except.fd_bits.copy_from_slice(unsafe {
                &(&(*exceptset).fd_bits)[..lwip::FD_SETSIZE.div_ceil(8)]
            });
        }
        let ret = unsafe {
            lwip::select(
                maxfdp1,
                if readset.is_null() {
                    core::ptr::null_mut()
                } else {
                    &mut lwip_read
                },
                if writeset.is_null() {
                    core::ptr::null_mut()
                } else {
                    &mut lwip_write
                },
                if exceptset.is_null() {
                    core::ptr::null_mut()
                } else {
                    &mut lwip_except
                },
                if timeout.is_null() {
                    core::ptr::null_mut()
                } else {
                    &mut lwip_timeout
                },
            )
        };
        if ret >= 0 {
            if !readset.is_null() {
                unsafe {
                    (&mut (*readset).fd_bits)[..lwip::FD_SETSIZE.div_ceil(8)]
                        .copy_from_slice(&lwip_read.fd_bits);
                }
            }
            if !writeset.is_null() {
                unsafe {
                    (&mut (*writeset).fd_bits)[..lwip::FD_SETSIZE.div_ceil(8)]
                        .copy_from_slice(&lwip_write.fd_bits);
                }
            }
            if !exceptset.is_null() {
                unsafe {
                    (&mut (*exceptset).fd_bits)[..lwip::FD_SETSIZE.div_ceil(8)]
                        .copy_from_slice(&lwip_except.fd_bits);
                }
            }
            if !timeout.is_null() {
                unsafe {
                    (*timeout).tv_sec = lwip_timeout.tv_sec;
                    (*timeout).tv_usec = lwip_timeout.tv_usec;
                }
            }
        }
        ret
    }

    /// # Safety
    ///
    /// `sds` must be null where permitted, or properly aligned and valid for all required reads and writes.
    /// Buffer extents must cover the supplied lengths, counts, and offsets,
    /// and the referenced memory must obey Rust's aliasing requirements.
    /// The pointers must remain valid for the duration of the call.
    pub unsafe fn net_poll(sds: *mut PollSd, nsds: i32, timeout: i32) -> i32 {
        if sds.is_null() || nsds <= 0 {
            return -22;
        }
        // Poll belongs to the initialized network session. Opening and closing
        // another top descriptor can deactivate socket processing while the
        // original descriptor still has live sockets and pending requests.
        let raw_fd = NET_IP_TOP_FD.load(Ordering::Acquire);
        if raw_fd < 0 {
            return -ENXIO;
        }
        let fd = FileDescriptor::new(raw_fd);
        let count = nsds as usize;
        let mut backend = Vec::with_capacity(count);
        unsafe {
            let slice = core::slice::from_raw_parts_mut(sds, count);
            backend.extend_from_slice(slice);
        }
        let mut params = [0u32; 2];
        params[1] = timeout as u32;
        let bytes = unsafe {
            core::slice::from_raw_parts_mut(
                backend.as_mut_ptr().cast::<u8>(),
                count * core::mem::size_of::<PollSd>(),
            )
        };
        let ret = fd.ioctl(
            IOCTL_SO_POLL,
            IoctlBuffers::from_slices(
                Some(unsafe {
                    core::slice::from_raw_parts_mut(params.as_mut_ptr().cast::<u8>(), 8)
                }),
                Some(bytes),
            ),
        );
        if ret < 0 {
            return ret;
        }
        unsafe {
            let slice = core::slice::from_raw_parts_mut(sds, count);
            slice.copy_from_slice(&backend);
        }
        ret
    }

    /// # Safety
    ///
    /// `local_ip` must be null where permitted, or properly aligned and valid for all required reads and writes.
    /// Additionally, `netmask` must be null where permitted, or properly aligned and valid for all required reads and writes.
    /// Additionally, `gateway` must be null where permitted, or properly aligned and valid for all required reads and writes.
    /// Buffer extents must cover the supplied lengths, counts, and offsets,
    /// and the referenced memory must obey Rust's aliasing requirements.
    /// The pointers must remain valid for the duration of the call.
    pub unsafe fn if_config(
        local_ip: *mut c_char,
        netmask: *mut c_char,
        gateway: *mut c_char,
        use_dhcp: bool,
        max_retries: i32,
    ) -> i32 {
        unsafe {
            let mut local = InAddr::default();
            let mut mask = InAddr::default();
            let mut route = InAddr::default();
            let ret = if_configex(&mut local, &mut mask, &mut route, use_dhcp, max_retries);
            if ret < 0 {
                return ret;
            }
            if !local_ip.is_null() && local.s_addr != 0 {
                write_ipv4_string(local_ip, local.s_addr);
            }
            if !netmask.is_null() && mask.s_addr != 0 {
                write_ipv4_string(netmask, mask.s_addr);
            }
            if !gateway.is_null() && route.s_addr != 0 {
                write_ipv4_string(gateway, route.s_addr);
            }
            ret
        }
    }

    /// # Safety
    ///
    /// `local_ip` must be null where permitted, or properly aligned and valid for all required reads and writes.
    /// Additionally, `netmask` must be null where permitted, or properly aligned and valid for all required reads and writes.
    /// Additionally, `gateway` must be null where permitted, or properly aligned and valid for all required reads and writes.
    /// Buffer extents must cover the supplied lengths, counts, and offsets,
    /// and the referenced memory must obey Rust's aliasing requirements.
    /// The pointers must remain valid for the duration of the call.
    pub unsafe fn if_configex(
        local_ip: *mut InAddr,
        netmask: *mut InAddr,
        gateway: *mut InAddr,
        use_dhcp: bool,
        max_retries: i32,
    ) -> i32 {
        if !use_dhcp {
            return -22;
        }
        let init = retry_net_init(max_retries, net_init, wait_before_retry);
        if init < 0 {
            return init;
        }
        let raw_fd = NET_IP_TOP_FD.load(Ordering::Acquire);
        if raw_fd < 0 {
            return SOCKET_ERROR;
        }
        let fd = FileDescriptor::new(raw_fd);
        let mut params = Aligned(GetInterfaceOptParams {
            magic: SO_GETINTERFACEOPT_LEVEL,
            option: SO_GETINTERFACEOPT_IPCONFIG,
        });
        let mut response = AlignedU32(size_of::<GetInterfaceOptIpConfig>() as u32);
        let mut ipconfig = Aligned(GetInterfaceOptIpConfig::default());
        let rc = fd.ioctlv(
            IOCTLV_SO_GETINTERFACEOPT,
            1,
            [
                IoctlvEntry::In(unsafe { bytes_of(&params.0) }),
                IoctlvEntry::Out(unsafe { bytes_of_mut(&mut ipconfig.0) }),
                IoctlvEntry::Out(unsafe { bytes_of_mut(&mut response.0) }),
            ],
        );
        if rc < 0 {
            return rc;
        }
        if gateway.is_null() {
            if !local_ip.is_null() {
                unsafe { (*local_ip).s_addr = ipconfig.0.inet };
            }
            if !netmask.is_null() {
                unsafe { (*netmask).s_addr = ipconfig.0.subnet_mask };
            }
            return 0;
        }
        let mut route_count = AlignedU32(0);
        let mut route_response = AlignedU32(size_of::<u32>() as u32);
        params.0.option = SO_GETINTERFACEOPT_ROUTING_SIZE;
        let rc = fd.ioctlv(
            IOCTLV_SO_GETINTERFACEOPT,
            1,
            [
                IoctlvEntry::In(unsafe { bytes_of(&params.0) }),
                IoctlvEntry::Out(unsafe { bytes_of_mut(&mut route_count.0) }),
                IoctlvEntry::Out(unsafe { bytes_of_mut(&mut route_response.0) }),
            ],
        );
        if rc < 0 {
            return rc;
        }
        if route_count.0 == 0 {
            return SOCKET_ERROR;
        }
        let Some(route_bytes) =
            (route_count.0 as usize).checked_mul(size_of::<GetInterfaceOptIpRoute>())
        else {
            return SOCKET_ERROR;
        };
        if route_bytes > 64 * 1024 {
            return SOCKET_ERROR;
        }
        let mut storage = Vec::new();
        if storage
            .try_reserve_exact(route_bytes.saturating_add(31))
            .is_err()
        {
            return SOCKET_ERROR;
        }
        storage.resize(route_bytes + 31, 0u8);
        let offset = (32 - (storage.as_ptr() as usize & 31)) & 31;
        let routes = &mut storage[offset..offset + route_bytes];
        let mut route_response = AlignedU32(route_bytes as u32);
        params.0.option = SO_GETINTERFACEOPT_ROUTING;
        let rc = fd.ioctlv(
            IOCTLV_SO_GETINTERFACEOPT,
            1,
            [
                IoctlvEntry::In(unsafe { bytes_of(&params.0) }),
                IoctlvEntry::Out(routes),
                IoctlvEntry::Out(unsafe { bytes_of_mut(&mut route_response.0) }),
            ],
        );
        if rc < 0 {
            return rc;
        }
        let mut route_gateway = None;
        for chunk in routes
            .as_chunks::<{ size_of::<GetInterfaceOptIpRoute>() }>()
            .0
        {
            let route = unsafe {
                core::ptr::read_unaligned(chunk.as_ptr().cast::<GetInterfaceOptIpRoute>())
            };
            if route.destination == 0 && route.netmask == 0 && (route.flags & 1) == 1 {
                route_gateway = Some(route.gateway);
                break;
            }
        }
        let operation_result = if route_gateway.is_some() {
            0
        } else {
            SOCKET_ERROR
        };
        if operation_result >= 0 {
            if !local_ip.is_null() {
                unsafe { (*local_ip).s_addr = ipconfig.0.inet };
            }
            if !netmask.is_null() {
                unsafe { (*netmask).s_addr = ipconfig.0.subnet_mask };
            }
            unsafe { (*gateway).s_addr = route_gateway.unwrap_or(0) };
        }
        operation_result
    }

    pub fn net_fcntl(s: i32, cmd: u32, flags: u32) -> i32 {
        let Some(idx) = track_idx(s) else {
            return SOCKET_ERROR;
        };
        match cmd {
            F_GETFL => {
                if SOCKET_NONBLOCK[idx].load(Ordering::Acquire) {
                    O_NONBLOCK as i32
                } else {
                    0
                }
            }
            F_SETFL => {
                let mut value = u32::from((flags & O_NONBLOCK) != 0);
                let ret = unsafe { lwip::ioctl(s, lwip::FIONBIO, (&mut value as *mut u32).cast()) };
                if ret >= 0 {
                    SOCKET_NONBLOCK[idx].store(value != 0, Ordering::Release);
                }
                ret
            }
            _ => SOCKET_ERROR,
        }
    }

    #[cfg(test)]
    mod socklen_type_tests {
        use super::SockLenT;

        #[test]
        fn public_socket_length_is_unsigned_u32() {
            let largest: SockLenT = u32::MAX;
            assert_eq!(largest, u32::MAX);
        }
    }

    #[cfg(test)]
    mod ifconfig_retry_tests {
        use super::{EAGAIN, ETIMEDOUT, MAX_INIT_RETRIES, retry_net_init};
        use core::cell::Cell;

        #[test]
        fn if_configex_ignores_max_retries_and_uses_c_attempt_limit() {
            for max_retries in [i32::MIN, -1, 0, 31, i32::MAX] {
                let init_calls = Cell::new(0);
                let delays = Cell::new(0);
                let result = retry_net_init(
                    max_retries,
                    || {
                        init_calls.set(init_calls.get() + 1);
                        -EAGAIN
                    },
                    || delays.set(delays.get() + 1),
                );

                assert_eq!(result, -EAGAIN);
                assert_eq!(init_calls.get(), MAX_INIT_RETRIES);
                assert_eq!(delays.get(), MAX_INIT_RETRIES);
            }
        }

        #[test]
        fn if_configex_stops_after_non_retryable_init_result() {
            let init_calls = Cell::new(0);
            let delays = Cell::new(0);
            let result = retry_net_init(
                -7,
                || {
                    let call = init_calls.get();
                    init_calls.set(call + 1);
                    match call {
                        0 => -EAGAIN,
                        1 => -ETIMEDOUT,
                        _ => 0,
                    }
                },
                || delays.set(delays.get() + 1),
            );

            assert_eq!(result, 0);
            assert_eq!(init_calls.get(), 3);
            assert_eq!(delays.get(), 2);
        }
    }
}

pub fn iptost(tos: u32) -> u32 {
    tos & IPTOS_TOS_MASK
}

pub fn iptos_prec(tos: u32) -> u32 {
    tos & IPTOS_PREC_MASK
}

pub fn ip4_addr(a: u8, b: u8, c: u8, d: u8) -> u32 {
    ((a as u32) << 24) | ((b as u32) << 16) | ((c as u32) << 8) | (d as u32)
}

pub fn ip4_addr1(addr: u32) -> u32 {
    (addr >> 24) & 0xff
}

pub fn ip4_addr2(addr: u32) -> u32 {
    (addr >> 16) & 0xff
}

pub fn ip4_addr3(addr: u32) -> u32 {
    (addr >> 8) & 0xff
}

pub fn ip4_addr4(addr: u32) -> u32 {
    addr & 0xff
}

pub fn net_init() -> i32 {
    powerpc_return!(wii_backend::net_init());
    runtime_unreachable()
}

pub fn net_attach_active() -> i32 {
    powerpc_return!(wii_backend::net_attach_active());
    runtime_unreachable()
}

/// # Safety
///
/// Raw pointer arguments and callbacks must be valid for the duration of the call.
pub unsafe fn net_init_async(cb: Netcallback, usrdata: *mut c_void) -> i32 {
    unsafe {
        powerpc_return!(wii_backend::net_init_async(cb, usrdata));
        runtime_unreachable()
    }
}

pub fn net_get_status() -> i32 {
    powerpc_return!(wii_backend::net_get_status());
    runtime_unreachable()
}

pub fn net_wc24cleanup() -> i32 {
    powerpc_return!(wii_backend::net_wc24cleanup());
    runtime_unreachable()
}

/// # Safety
///
/// Raw pointer arguments and callbacks must be valid for the duration of the call.
pub unsafe fn net_get_mac_address(mac_buf: *mut c_void) -> i32 {
    unsafe {
        powerpc_return!(wii_backend::net_get_mac_address(mac_buf));
        runtime_unreachable()
    }
}

pub fn net_deinit() -> i32 {
    powerpc_return!(wii_backend::net_deinit());
    runtime_unreachable()
}

pub fn _net_convert_error(ios_retval: i32) -> i32 {
    powerpc_return!(wii_backend::_net_convert_error(ios_retval));
    runtime_unreachable()
}

pub fn net_gethostip() -> u32 {
    powerpc_return!(wii_backend::net_gethostip());
    runtime_unreachable()
}

/// # Safety
///
/// Raw pointer arguments and callbacks must be valid for the duration of the call.
pub unsafe fn net_gethostbyname(_addr_string: *const c_char) -> *mut HostEnt {
    unsafe {
        powerpc_return!(wii_backend::net_gethostbyname(_addr_string));
        runtime_unreachable()
    }
}

/// # Safety
///
/// `cp` must be null where permitted, or properly aligned and valid for all required reads.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn inet_addr(cp: *const c_char) -> u32 {
    unsafe {
        powerpc_return!(wii_backend::inet_addr(cp));
        runtime_unreachable()
    }
}

/// # Safety
///
/// `name` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn net_bind(s: i32, name: *mut Sockaddr, namelen: SockLenT) -> i32 {
    unsafe {
        powerpc_return!(wii_backend::net_bind(s, name, namelen));
        runtime_unreachable()
    }
}

pub fn net_listen(s: i32, backlog: u32) -> i32 {
    powerpc_return!(wii_backend::net_listen(s, backlog));
    runtime_unreachable()
}

/// # Safety
///
/// `addr` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Additionally, `addrlen` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn net_accept(s: i32, addr: *mut Sockaddr, addrlen: *mut SockLenT) -> i32 {
    unsafe {
        powerpc_return!(wii_backend::net_accept(s, addr, addrlen));
        runtime_unreachable()
    }
}

/// # Safety
///
/// `addr` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn net_connect(s: i32, addr: *mut Sockaddr, addrlen: SockLenT) -> i32 {
    unsafe {
        powerpc_return!(wii_backend::net_connect(s, addr, addrlen));
        runtime_unreachable()
    }
}

/// # Safety
///
/// `data` must be null where permitted, or properly aligned and valid for all required reads.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn net_write(s: i32, data: *const c_void, size: i32) -> i32 {
    unsafe {
        powerpc_return!(wii_backend::net_write(s, data, size));
        runtime_unreachable()
    }
}

/// # Safety
///
/// `data` must be null where permitted, or properly aligned and valid for all required reads.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn net_send(s: i32, data: *const c_void, size: i32, flags: u32) -> i32 {
    unsafe {
        powerpc_return!(wii_backend::net_send(s, data, size, flags));
        runtime_unreachable()
    }
}

/// # Safety
///
/// `data` must be null where permitted, or properly aligned and valid for all required reads.
/// Additionally, `to` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn net_sendto(
    s: i32,
    data: *const c_void,
    len: i32,
    flags: u32,
    to: *mut Sockaddr,
    tolen: SockLenT,
) -> i32 {
    unsafe {
        powerpc_return!(wii_backend::net_sendto(s, data, len, flags, to, tolen));
        runtime_unreachable()
    }
}

/// # Safety
///
/// `mem` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn net_recv(s: i32, mem: *mut c_void, len: i32, flags: u32) -> i32 {
    unsafe {
        powerpc_return!(wii_backend::net_recv(s, mem, len, flags));
        runtime_unreachable()
    }
}

/// # Safety
///
/// `mem` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Additionally, `from` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Additionally, `fromlen` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn net_recvfrom(
    s: i32,
    mem: *mut c_void,
    len: i32,
    flags: u32,
    from: *mut Sockaddr,
    fromlen: *mut SockLenT,
) -> i32 {
    unsafe {
        powerpc_return!(wii_backend::net_recvfrom(s, mem, len, flags, from, fromlen));
        runtime_unreachable()
    }
}

/// # Safety
///
/// `mem` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn net_read(s: i32, mem: *mut c_void, len: i32) -> i32 {
    unsafe {
        powerpc_return!(wii_backend::net_read(s, mem, len));
        runtime_unreachable()
    }
}

/// # Safety
///
/// `optval` must be null where permitted, or properly aligned and valid for all required reads.
/// Additionally, `optlen` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn net_getsockopt(
    s: i32,
    level: u32,
    optname: u32,
    optval: *const c_void,
    optlen: *mut SockLenT,
) -> i32 {
    unsafe {
        powerpc_return!(wii_backend::net_getsockopt(
            s, level, optname, optval, optlen
        ));
        runtime_unreachable()
    }
}

/// # Safety
///
/// `optval` must be null where permitted, or properly aligned and valid for all required reads.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn net_setsockopt(
    s: i32,
    level: u32,
    optname: u32,
    optval: *const c_void,
    optlen: SockLenT,
) -> i32 {
    unsafe {
        powerpc_return!(wii_backend::net_setsockopt(
            s, level, optname, optval, optlen
        ));
        runtime_unreachable()
    }
}

/// # Safety
///
/// `addr` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Additionally, `addrlen` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn net_getsockname(s: i32, addr: *mut Sockaddr, addrlen: *mut SockLenT) -> i32 {
    unsafe {
        powerpc_return!(wii_backend::net_getsockname(s, addr, addrlen));
        runtime_unreachable()
    }
}

/// # Safety
///
/// `argp` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn net_ioctl(s: i32, cmd: u32, argp: *mut c_void) -> i32 {
    unsafe {
        powerpc_return!(wii_backend::net_ioctl(s, cmd, argp));
        runtime_unreachable()
    }
}

pub fn net_fcntl(s: i32, cmd: u32, flags: u32) -> i32 {
    powerpc_return!(wii_backend::net_fcntl(s, cmd, flags));
    runtime_unreachable()
}

pub fn net_shutdown(s: i32, how: u32) -> i32 {
    powerpc_return!(wii_backend::net_shutdown(s, how));
    runtime_unreachable()
}

pub fn net_socket(domain: u32, type_: u32, protocol: u32) -> i32 {
    powerpc_return!(wii_backend::net_socket(domain, type_, protocol));
    runtime_unreachable()
}

pub fn net_close(s: i32) -> i32 {
    powerpc_return!(wii_backend::net_close(s));
    runtime_unreachable()
}

/// # Safety
///
/// Every non-null set and timeout pointer must be valid for the duration of
/// the call and uniquely borrowed when written.
pub unsafe fn net_select(
    maxfdp1: i32,
    readset: *mut FdSet,
    writeset: *mut FdSet,
    exceptset: *mut FdSet,
    timeout: *mut Timeval,
) -> i32 {
    powerpc_return!(unsafe {
        wii_backend::net_select(maxfdp1, readset, writeset, exceptset, timeout)
    });
    runtime_unreachable()
}

/// # Safety
///
/// `sds` must be null where permitted, or properly aligned and valid for all required reads and writes.
/// Buffer extents must cover the supplied lengths, counts, and offsets,
/// and the referenced memory must obey Rust's aliasing requirements.
/// The pointers must remain valid for the duration of the call.
pub unsafe fn net_poll(sds: *mut PollSd, nsds: i32, timeout: i32) -> i32 {
    unsafe {
        powerpc_return!(wii_backend::net_poll(sds, nsds, timeout));
        runtime_unreachable()
    }
}

/// # Safety
///
/// Raw pointer arguments and callbacks must be valid for the duration of the call.
pub unsafe fn if_config(
    local_ip: *mut c_char,
    netmask: *mut c_char,
    gateway: *mut c_char,
    use_dhcp: bool,
    max_retries: i32,
) -> i32 {
    unsafe {
        powerpc_return!(wii_backend::if_config(
            local_ip,
            netmask,
            gateway,
            use_dhcp,
            max_retries
        ));
        runtime_unreachable()
    }
}

/// # Safety
///
/// Raw pointer arguments and callbacks must be valid for the duration of the call.
pub unsafe fn if_configex(
    local_ip: *mut InAddr,
    netmask: *mut InAddr,
    gateway: *mut InAddr,
    use_dhcp: bool,
    _max_retries: i32,
) -> i32 {
    unsafe {
        powerpc_return!(wii_backend::if_configex(
            local_ip,
            netmask,
            gateway,
            use_dhcp,
            _max_retries
        ));
        runtime_unreachable()
    }
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    extern crate std;

    use super::*;
    use core::mem::size_of;
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    unsafe extern "C" fn cb(result: i32, usrdata: *mut c_void) -> i32 {
        let slot = usrdata as *mut i32;
        if !slot.is_null() {
            // SAFETY: The callback test supplies a live, writable i32.
            unsafe {
                *slot = result;
            }
        }
        result
    }

    #[test]
    fn ipv4_helpers_and_basic_constants_match() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        assert_eq!(ip4_addr(1, 2, 3, 4), 0x0102_0304);
        assert_eq!(ip4_addr1(0x0102_0304), 1);
        assert_eq!(ip4_addr2(0x0102_0304), 2);
        assert_eq!(ip4_addr3(0x0102_0304), 3);
        assert_eq!(ip4_addr4(0x0102_0304), 4);
        assert_eq!(SOCK_STREAM, 1);
        assert_eq!(SOL_SOCKET, 0x0fff);
        assert_eq!(FD_SETSIZE, 16);
    }

    #[test]
    fn initial_network_status_matches_c() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        assert_eq!(wii_backend::INITIAL_NET_STATUS, -100);
    }

    #[test]
    fn network_startup_repeats_before_each_host_id_query() {
        let events = std::cell::RefCell::new(std::vec::Vec::new());
        let queries = std::cell::Cell::new(0);
        let result = startup_network_ip(
            -110,
            || {
                events.borrow_mut().push("startup");
                Ok(0)
            },
            || {
                events.borrow_mut().push("query");
                queries.set(queries.get() + 1);
                Ok(if queries.get() == 3 { 0x0a00_00f2 } else { 0 })
            },
            || events.borrow_mut().push("wait"),
        );
        assert_eq!(result, Ok(0x0a00_00f2));
        assert_eq!(
            *events.borrow(),
            [
                "startup", "query", "wait", "startup", "query", "wait", "startup", "query"
            ]
        );
    }

    #[test]
    fn network_startup_failure_stops_before_host_id_query() {
        for failure in [Ok(-4), Err(-100)] {
            assert_eq!(
                startup_network_ip(
                    -110,
                    || failure,
                    || panic!("query after startup failure"),
                    || panic!("wait after startup failure")
                ),
                Err(failure.unwrap_or(-100))
            );
        }
        assert_eq!(
            startup_network_ip(
                -110,
                || Ok(0),
                || Err(-4),
                || panic!("wait after query failure")
            ),
            Err(-4)
        );
    }

    #[test]
    fn network_host_id_retries_match_c_attempt_and_wait_counts() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let mut attempts = 0;
        let mut waits = 0;
        let result = retry_network_ip(
            -110,
            || {
                attempts += 1;
                Ok(0)
            },
            || waits += 1,
        );
        assert_eq!(result, Err(-110));
        assert_eq!(attempts, 101);
        assert_eq!(waits, 100);

        attempts = 0;
        waits = 0;
        assert_eq!(
            retry_network_ip(
                -110,
                || {
                    attempts += 1;
                    Ok(if attempts == 3 { 0x0102_0304 } else { 0 })
                },
                || waits += 1,
            ),
            Ok(0x0102_0304)
        );
        assert_eq!(attempts, 3);
        assert_eq!(waits, 2);
    }

    #[test]
    fn active_attach_reuses_an_address_without_startup() {
        assert_eq!(
            active_attach_ip(0x0a00_00f2, -110, || panic!("unneeded startup")),
            Ok(0x0a00_00f2)
        );
        assert_eq!(
            active_attach_ip(-6, -110, || panic!("startup after query failure")),
            Err(-6)
        );
    }

    #[test]
    fn active_attach_starts_an_interface_with_no_address() {
        let queries = core::cell::Cell::new(0);
        let startups = core::cell::Cell::new(0);
        let waits = core::cell::Cell::new(0);
        let result = active_attach_ip(0, -110, || {
            startup_network_ip(
                -110,
                || {
                    startups.set(startups.get() + 1);
                    Ok(0)
                },
                || {
                    queries.set(queries.get() + 1);
                    Ok(if queries.get() == 6 { 0x0a00_00f2 } else { 0 })
                },
                || waits.set(waits.get() + 1),
            )
        });
        assert_eq!(result, Ok(0x0a00_00f2));
        assert_eq!(startups.get(), 6);
        assert_eq!(queries.get(), 6);
        assert_eq!(waits.get(), 5);
    }

    #[test]
    fn active_attach_returns_startup_timeout_and_failures() {
        for (startup_result, expected) in [(Err(-100), -100), (Ok(-4), -4), (Ok(0), -110)] {
            assert_eq!(active_attach_ip(0, -110, || startup_result), Err(expected));
        }
    }

    #[test]
    fn gethostip_uses_only_the_initialized_descriptor_and_retries_zero() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let calls = core::cell::Cell::new(0);
        let waits = core::cell::Cell::new(0);
        assert_eq!(
            wii_backend::net_gethostip_with(
                -1,
                |_| {
                    calls.set(calls.get() + 1);
                    0x0102_0304
                },
                || waits.set(waits.get() + 1)
            ),
            0
        );
        assert_eq!(calls.get(), 0);
        assert_eq!(waits.get(), 0);

        assert_eq!(
            wii_backend::net_gethostip_with(
                7,
                |_| {
                    calls.set(calls.get() + 1);
                    if calls.get() == 1 { 0 } else { 0x0102_0304 }
                },
                || waits.set(waits.get() + 1)
            ),
            0x0102_0304
        );
        assert_eq!(calls.get(), 2);
        assert_eq!(waits.get(), 1);

        calls.set(0);
        waits.set(0);
        assert_eq!(
            wii_backend::net_gethostip_with(
                7,
                |_| {
                    calls.set(calls.get() + 1);
                    0
                },
                || waits.set(waits.get() + 1),
            ),
            0
        );
        assert_eq!(calls.get(), 5);
        assert_eq!(waits.get(), 5);

        assert_eq!(
            wii_backend::net_gethostip_with(7, |_| -6, || panic!("nonzero result must not retry")),
            (-6_i32) as u32
        );
    }

    #[test]
    fn init_and_async_callback_are_host_compat() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let mut result = -1;
        assert_eq!(
            unsafe { net_init_async(Some(cb), &mut result as *mut i32 as *mut c_void) },
            0
        );
        assert_eq!(result, 0);
        assert_eq!(net_init(), 0);
        assert_eq!(net_get_status(), 0);
        assert_eq!(net_gethostip(), 0);
        assert_eq!(_net_convert_error(-12), -12);
        let mut mac = [0xffu8; 6];
        assert_eq!(unsafe { net_get_mac_address(mac.as_mut_ptr().cast()) }, 0);
        assert_eq!(mac, [0; 6]);
        net_wc24cleanup();
        net_deinit();
        assert_eq!(net_get_status(), -100);
    }

    #[test]
    fn socket_entrypoints_and_ifconfig_follow_host_contract() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let s = net_socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
        assert!(s >= 0);
        let mut sockaddr = SockaddrIn {
            sin_len: size_of::<SockaddrIn>() as u8,
            sin_family: AF_INET as u8,
            sin_port: 0x1234,
            sin_addr: InAddr {
                s_addr: ip4_addr(10, 0, 0, 1),
            },
            sin_zero: [0; 8],
        };
        assert_eq!(
            unsafe {
                net_bind(
                    s,
                    (&mut sockaddr as *mut SockaddrIn).cast(),
                    size_of::<SockaddrIn>() as SockLenT,
                )
            },
            0
        );
        assert_eq!(net_listen(s, 4), 0);
        let opt = 1_i32;
        assert_eq!(
            unsafe {
                net_setsockopt(
                    s,
                    SOL_SOCKET,
                    SO_KEEPALIVE,
                    (&opt as *const i32).cast(),
                    size_of::<i32>() as SockLenT,
                )
            },
            0
        );
        let mut out = 0x55_i32;
        let mut out_len = size_of::<i32>() as SockLenT;
        assert_eq!(
            unsafe {
                net_getsockopt(
                    s,
                    SOL_SOCKET,
                    SO_KEEPALIVE,
                    (&mut out as *mut i32).cast(),
                    &mut out_len,
                )
            },
            0
        );
        assert_eq!(out, 0x55);

        let mut local = InAddr::default();
        let mut mask = InAddr::default();
        let mut gateway = InAddr::default();
        assert_eq!(
            unsafe { if_configex(&mut local, &mut mask, &mut gateway, true, 0) },
            0
        );
        assert_eq!(local.s_addr, 0);
        assert_eq!(mask.s_addr, 0);
        assert_eq!(gateway.s_addr, 0);
        assert_eq!(
            unsafe {
                if_config(
                    core::ptr::null_mut(),
                    core::ptr::null_mut(),
                    core::ptr::null_mut(),
                    false,
                    0,
                )
            },
            -22
        );
        assert_eq!(net_close(s), 0);
    }
}
