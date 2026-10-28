#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

#[cfg(target_arch = "powerpc")]
extern crate alloc;

use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicI32, Ordering};

mod ios_socket_flags;
pub use ios_socket_flags::option_to_ios;

macro_rules! powerpc_return {
    ($expr:expr_2021) => {
        if cfg!(target_arch = "powerpc") {
            return $expr;
        }
    };
}

pub type ErrT = i8;
pub type SockLenT = u32;

pub const ERR_OK: ErrT = 0;
pub const ERR_MEM: ErrT = -1;
pub const ERR_BUF: ErrT = -2;
pub const ERR_ABRT: ErrT = -3;
pub const ERR_RST: ErrT = -4;
pub const ERR_CLSD: ErrT = -5;
pub const ERR_CONN: ErrT = -6;
pub const ERR_VAL: ErrT = -7;
pub const ERR_ARG: ErrT = -8;
pub const ERR_RTE: ErrT = -9;
pub const ERR_USE: ErrT = -10;
pub const ERR_IF: ErrT = -11;
pub const ERR_ISCONN: ErrT = -12;

pub const SOCK_STREAM: i32 = 1;
pub const SOCK_DGRAM: i32 = 2;
pub const SOCK_RAW: i32 = 3;
pub const SOCK_CLOEXEC: i32 = 0x1000_0000;
pub const SOCK_NONBLOCK: i32 = 0x2000_0000;
pub const SO_DEBUG: i32 = 0x0001;
pub const SO_ACCEPTCONN: i32 = 0x0002;
pub const SO_REUSEADDR: i32 = 0x0004;
pub const SO_KEEPALIVE: i32 = 0x0008;
pub const SO_DONTROUTE: i32 = 0x0010;
pub const SO_BROADCAST: i32 = 0x0020;
pub const SO_USELOOPBACK: i32 = 0x0040;
pub const SO_LINGER: i32 = 0x0080;
pub const SO_OOBINLINE: i32 = 0x0100;
pub const SO_REUSEPORT: i32 = 0x0200;
pub const SO_DONTLINGER: i32 = !SO_LINGER;
pub const SO_SNDBUF: i32 = 0x1001;
pub const SO_RCVBUF: i32 = 0x1002;
pub const SO_SNDLOWAT: i32 = 0x1003;
pub const SO_RCVLOWAT: i32 = 0x1004;
pub const SO_SNDTIMEO: i32 = 0x1005;
pub const SO_RCVTIMEO: i32 = 0x1006;
pub const SO_ERROR: i32 = 0x1007;
pub const SO_TYPE: i32 = 0x1008;
// Wii IOS uses the BSD socket ABI value. The GameCube lwIP headers use
// 0x0fff, but passing that value to /dev/net/ip/top makes SO_RCVBUF fail.
pub const SOL_SOCKET: i32 = 0xffff;
pub const AF_UNSPEC: i32 = 0;
pub const AF_INET: i32 = 2;
pub const PF_INET: i32 = AF_INET;
pub const IPPROTO_IP: i32 = 0;
pub const IPPROTO_TCP: i32 = 6;
pub const IPPROTO_UDP: i32 = 17;
pub const IP_TOS: i32 = 1;
pub const IP_TTL: i32 = 2;
pub const IPTOS_TOS_MASK: i32 = 0x1e;
pub const IPTOS_LOWDELAY: i32 = 0x10;
pub const IPTOS_THROUGHPUT: i32 = 0x08;
pub const IPTOS_RELIABILITY: i32 = 0x04;
pub const IPTOS_LOWCOST: i32 = 0x02;
pub const IPTOS_MINCOST: i32 = IPTOS_LOWCOST;
pub const IPTOS_PREC_MASK: i32 = 0xe0;
pub const IPTOS_PREC_NETCONTROL: i32 = 0xe0;
pub const IPTOS_PREC_INTERNETCONTROL: i32 = 0xc0;
pub const IPTOS_PREC_CRITIC_ECP: i32 = 0xa0;
pub const IPTOS_PREC_FLASHOVERRIDE: i32 = 0x80;
pub const IPTOS_PREC_FLASH: i32 = 0x60;
pub const IPTOS_PREC_IMMEDIATE: i32 = 0x40;
pub const IPTOS_PREC_PRIORITY: i32 = 0x20;
pub const IPTOS_PREC_ROUTINE: i32 = 0x00;
pub const TCP_NODELAY: i32 = 0x01;
pub const TCP_KEEPALIVE: i32 = 0x02;
pub const INADDR_ANY: u32 = 0;
pub const INADDR_BROADCAST: u32 = 0xffff_ffff;
pub const MSG_DONTWAIT: u32 = 0x40;
pub const FIONREAD: i64 = 0x4004_667f;
pub const FIONBIO: i64 = 0x8004_667e_u64 as i64;
pub const O_NONBLOCK: u32 = 0o4000;
pub const FD_SETSIZE: usize = 16;

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
pub struct InAddr {
    pub s_addr: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct SockAddrIn {
    pub sin_len: u8,
    pub sin_family: u8,
    pub sin_port: u16,
    pub sin_addr: InAddr,
    pub sin_zero: [u8; 8],
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct SockAddr {
    pub sa_len: u8,
    pub sa_family: u8,
    pub sa_data: [u8; 14],
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct Linger {
    pub l_onoff: i32,
    pub l_linger: i32,
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct FdSet {
    pub fd_bits: [u8; FD_SETSIZE.div_ceil(8)],
}

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct TimeVal {
    pub tv_sec: i32,
    pub tv_usec: i32,
}

pub const POLLRDNORM: i16 = 0x0001;
pub const POLLRDBAND: i16 = 0x0002;
pub const POLLIN: i16 = POLLRDNORM | POLLRDBAND;
pub const POLLPRI: i16 = 0x0004;
pub const POLLWRNORM: i16 = 0x0008;
pub const POLLWRBAND: i16 = 0x0010;
pub const POLLOUT: i16 = POLLWRNORM;
pub const POLLERR: i16 = 0x0020;
pub const POLLHUP: i16 = 0x0040;
pub const POLLNVAL: i16 = 0x0080;

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct PollFd {
    pub fd: i32,
    pub events: i16,
    pub revents: i16,
}

const MAX_SOCKETS: usize = 8;
const DEFAULT_RCVBUF: i32 = 32 * 1024;
const DEFAULT_SNDBUF: i32 = 0;
const DEFAULT_LOW_WATER: i32 = 1;
const DEFAULT_IP_TTL: i32 = 255;
const EINVAL: i32 = 22;
const ENXIO: i32 = 6;
const EAFNOSUPPORT: i32 = 97;
const EOVERFLOW: i32 = 75;

static SOCKET_TYPE: [AtomicI32; MAX_SOCKETS] = [const { AtomicI32::new(0) }; MAX_SOCKETS];
static SOCKET_RCVBUF: [AtomicI32; MAX_SOCKETS] =
    [const { AtomicI32::new(DEFAULT_RCVBUF) }; MAX_SOCKETS];
static SOCKET_NONBLOCK: [AtomicBool; MAX_SOCKETS] = [const { AtomicBool::new(false) }; MAX_SOCKETS];
static SOCKET_LISTENING: [AtomicBool; MAX_SOCKETS] =
    [const { AtomicBool::new(false) }; MAX_SOCKETS];
static SOCKET_REUSEADDR: [AtomicBool; MAX_SOCKETS] =
    [const { AtomicBool::new(false) }; MAX_SOCKETS];
static SOCKET_KEEPALIVE: [AtomicBool; MAX_SOCKETS] =
    [const { AtomicBool::new(false) }; MAX_SOCKETS];
static SOCKET_BROADCAST: [AtomicBool; MAX_SOCKETS] =
    [const { AtomicBool::new(false) }; MAX_SOCKETS];
static SOCKET_DEBUG: [AtomicBool; MAX_SOCKETS] = [const { AtomicBool::new(false) }; MAX_SOCKETS];
static SOCKET_DONTROUTE: [AtomicBool; MAX_SOCKETS] =
    [const { AtomicBool::new(false) }; MAX_SOCKETS];
static SOCKET_USELOOPBACK: [AtomicBool; MAX_SOCKETS] =
    [const { AtomicBool::new(false) }; MAX_SOCKETS];
static SOCKET_LINGER_ON: [AtomicBool; MAX_SOCKETS] =
    [const { AtomicBool::new(false) }; MAX_SOCKETS];
static SOCKET_LINGER_SECONDS: [AtomicI32; MAX_SOCKETS] = [const { AtomicI32::new(0) }; MAX_SOCKETS];
static SOCKET_OOBINLINE: [AtomicBool; MAX_SOCKETS] =
    [const { AtomicBool::new(false) }; MAX_SOCKETS];
static SOCKET_REUSEPORT: [AtomicBool; MAX_SOCKETS] =
    [const { AtomicBool::new(false) }; MAX_SOCKETS];
static SOCKET_SNDBUF: [AtomicI32; MAX_SOCKETS] =
    [const { AtomicI32::new(DEFAULT_SNDBUF) }; MAX_SOCKETS];
static SOCKET_SNDLOWAT: [AtomicI32; MAX_SOCKETS] =
    [const { AtomicI32::new(DEFAULT_LOW_WATER) }; MAX_SOCKETS];
static SOCKET_RCVLOWAT: [AtomicI32; MAX_SOCKETS] =
    [const { AtomicI32::new(DEFAULT_LOW_WATER) }; MAX_SOCKETS];
static SOCKET_SNDTIMEO_SEC: [AtomicI32; MAX_SOCKETS] = [const { AtomicI32::new(0) }; MAX_SOCKETS];
static SOCKET_SNDTIMEO_USEC: [AtomicI32; MAX_SOCKETS] = [const { AtomicI32::new(0) }; MAX_SOCKETS];
static SOCKET_RCVTIMEO_SEC: [AtomicI32; MAX_SOCKETS] = [const { AtomicI32::new(0) }; MAX_SOCKETS];
static SOCKET_RCVTIMEO_USEC: [AtomicI32; MAX_SOCKETS] = [const { AtomicI32::new(0) }; MAX_SOCKETS];
static SOCKET_IP_TOS: [AtomicI32; MAX_SOCKETS] = [const { AtomicI32::new(0) }; MAX_SOCKETS];
static SOCKET_IP_TTL: [AtomicI32; MAX_SOCKETS] =
    [const { AtomicI32::new(DEFAULT_IP_TTL) }; MAX_SOCKETS];
static SOCKET_TCP_NODELAY: [AtomicBool; MAX_SOCKETS] =
    [const { AtomicBool::new(false) }; MAX_SOCKETS];
static SOCKET_TCP_KEEPALIVE: [AtomicI32; MAX_SOCKETS] = [const { AtomicI32::new(0) }; MAX_SOCKETS];

#[inline]
fn runtime_unreachable<T>() -> T {
    core::unreachable!()
}

fn reset_tracked_options(idx: usize) {
    SOCKET_RCVBUF[idx].store(DEFAULT_RCVBUF, Ordering::Release);
    SOCKET_NONBLOCK[idx].store(false, Ordering::Release);
    SOCKET_LISTENING[idx].store(false, Ordering::Release);
    SOCKET_REUSEADDR[idx].store(false, Ordering::Release);
    SOCKET_KEEPALIVE[idx].store(false, Ordering::Release);
    SOCKET_BROADCAST[idx].store(false, Ordering::Release);
    SOCKET_DEBUG[idx].store(false, Ordering::Release);
    SOCKET_DONTROUTE[idx].store(false, Ordering::Release);
    SOCKET_USELOOPBACK[idx].store(false, Ordering::Release);
    SOCKET_LINGER_ON[idx].store(false, Ordering::Release);
    SOCKET_LINGER_SECONDS[idx].store(0, Ordering::Release);
    SOCKET_OOBINLINE[idx].store(false, Ordering::Release);
    SOCKET_REUSEPORT[idx].store(false, Ordering::Release);
    SOCKET_SNDBUF[idx].store(DEFAULT_SNDBUF, Ordering::Release);
    SOCKET_SNDLOWAT[idx].store(DEFAULT_LOW_WATER, Ordering::Release);
    SOCKET_RCVLOWAT[idx].store(DEFAULT_LOW_WATER, Ordering::Release);
    SOCKET_SNDTIMEO_SEC[idx].store(0, Ordering::Release);
    SOCKET_SNDTIMEO_USEC[idx].store(0, Ordering::Release);
    SOCKET_RCVTIMEO_SEC[idx].store(0, Ordering::Release);
    SOCKET_RCVTIMEO_USEC[idx].store(0, Ordering::Release);
    SOCKET_IP_TOS[idx].store(0, Ordering::Release);
    SOCKET_IP_TTL[idx].store(DEFAULT_IP_TTL, Ordering::Release);
    SOCKET_TCP_NODELAY[idx].store(false, Ordering::Release);
    SOCKET_TCP_KEEPALIVE[idx].store(0, Ordering::Release);
}

fn pollfd_events_to_pollsd(events: i16) -> u32 {
    (events as u16) as u32
}
fn pollsd_events_to_pollfd(events: u32) -> i16 {
    events as i16
}

#[cfg(target_arch = "powerpc")]
mod wii_backend {
    use super::*;
    use alloc::alloc::{Layout, alloc_zeroed, dealloc};
    use alloc::vec::Vec;
    use core::mem::size_of;
    use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU16, AtomicU32, Ordering};
    use ogc_ipc::{FileDescriptor, IPC_ENOMEM, IoctlBuffers, IoctlvEntry};

    const IOS_O_NONBLOCK: u32 = 0x04;
    const F_GETFL: u32 = 3;
    const F_SETFL: u32 = 4;
    const FLAG_MASK: u32 = (SOCK_NONBLOCK | SOCK_CLOEXEC) as u32;

    pub(super) fn apply_nonblocking_socket_flag(
        socket: i32,
        get_flags: impl FnOnce(i32) -> i32,
        set_flags: impl FnOnce(i32, u32) -> i32,
    ) -> i32 {
        let current_flags = get_flags(socket);
        let _ = set_flags(socket, (current_flags as u32) | IOS_O_NONBLOCK);
        socket
    }

    const IOCTL_SO_ACCEPT: i32 = 1;
    const IOCTL_SO_BIND: i32 = 2;
    const IOCTL_SO_CLOSE: i32 = 3;
    const IOCTL_SO_CONNECT: i32 = 4;
    const IOCTL_SO_FCNTL: i32 = 5;
    const IOCTL_SO_GETSOCKNAME: i32 = 7;
    const IOCTL_SO_SETSOCKOPT: i32 = 9;
    const IOCTL_SO_LISTEN: i32 = 10;
    const IOCTL_SO_POLL: i32 = 11;
    const IOCTLV_SO_RECVFROM: i32 = 12;
    const IOCTLV_SO_SENDTO: i32 = 13;
    const IOCTL_SO_SHUTDOWN: i32 = 14;
    const IOCTL_SO_SOCKET: i32 = 15;

    pub(super) unsafe fn copy_setsockopt_payload(
        dst: &mut [u8; 20],
        optval: *const c_void,
        optlen: SockLenT,
    ) {
        if !optval.is_null() && optlen != 0 {
            unsafe {
                core::ptr::copy_nonoverlapping(
                    optval.cast::<u8>(),
                    dst.as_mut_ptr(),
                    optlen as usize,
                );
            }
        }
    }

    #[repr(C)]
    #[derive(Copy, Clone, Default)]
    struct WiiSockAddrIn {
        sin_len: u8,
        sin_family: u8,
        sin_port: u16,
        sin_addr: u32,
    }

    impl WiiSockAddrIn {
        fn for_accept() -> Self {
            Self {
                sin_len: size_of::<Self>() as u8,
                sin_family: AF_INET as u8,
                sin_port: 0,
                sin_addr: 0,
            }
        }
    }

    #[cfg(test)]
    mod accept_address_tests {
        use super::WiiSockAddrIn;

        #[test]
        fn ios_accept_receives_initialized_ipv4_address() {
            let address = WiiSockAddrIn::for_accept();
            let bytes = unsafe {
                core::slice::from_raw_parts(
                    (&address as *const WiiSockAddrIn).cast::<u8>(),
                    core::mem::size_of::<WiiSockAddrIn>(),
                )
            };
            assert_eq!(bytes, &[8, 2, 0, 0, 0, 0, 0, 0]);
        }
    }

    #[repr(C)]
    #[derive(Copy, Clone, Default)]
    struct BindParams {
        socket: u32,
        has_name: u32,
        name: [u8; 28],
    }

    #[repr(C)]
    #[derive(Copy, Clone, Default)]
    struct ConnectParams {
        socket: u32,
        has_addr: u32,
        addr: [u8; 28],
    }

    #[repr(C)]
    #[derive(Copy, Clone, Default)]
    struct SendToParams {
        socket: u32,
        flags: u32,
        has_destaddr: u32,
        destaddr: [u8; 28],
    }

    #[repr(C)]
    #[derive(Copy, Clone, Default)]
    struct SetSockOptParams {
        socket: u32,
        level: u32,
        optname: u32,
        optlen: u32,
        optval: [u8; 20],
    }

    #[repr(C)]
    #[derive(Copy, Clone, Default)]
    struct SocketFdParam {
        socket: u32,
    }

    #[repr(C)]
    #[derive(Copy, Clone, Default)]
    struct WiiPollSd {
        socket: i32,
        events: u32,
        revents: u32,
    }

    static TOP_FD: AtomicI32 = AtomicI32::new(-1);

    #[repr(align(32))]
    struct Aligned<T>(T);

    const MAX_SEND_BLOCK: usize = 4096;
    const MAX_RECV_BLOCK: usize = 32 * 1024;
    static SOCKET_USED: [AtomicBool; MAX_SOCKETS] = [const { AtomicBool::new(false) }; MAX_SOCKETS];
    static SOCKET_RAW: [AtomicI32; MAX_SOCKETS] = [const { AtomicI32::new(-1) }; MAX_SOCKETS];
    static SOCKET_TYPE: [AtomicI32; MAX_SOCKETS] = [const { AtomicI32::new(0) }; MAX_SOCKETS];
    static SOCKET_BOUND_PORT: [AtomicU16; MAX_SOCKETS] = [const { AtomicU16::new(0) }; MAX_SOCKETS];
    static SOCKET_PEER_PORT: [AtomicU16; MAX_SOCKETS] = [const { AtomicU16::new(0) }; MAX_SOCKETS];
    static SOCKET_BOUND_ADDR: [AtomicU32; MAX_SOCKETS] = [const { AtomicU32::new(0) }; MAX_SOCKETS];
    static SOCKET_PEER_ADDR: [AtomicU32; MAX_SOCKETS] = [const { AtomicU32::new(0) }; MAX_SOCKETS];
    static SOCKET_BOUND_VALID: [AtomicBool; MAX_SOCKETS] =
        [const { AtomicBool::new(false) }; MAX_SOCKETS];
    static SOCKET_PEER_VALID: [AtomicBool; MAX_SOCKETS] =
        [const { AtomicBool::new(false) }; MAX_SOCKETS];
    static SOCKET_REUSEADDR: [AtomicBool; MAX_SOCKETS] =
        [const { AtomicBool::new(false) }; MAX_SOCKETS];
    static SOCKET_KEEPALIVE: [AtomicBool; MAX_SOCKETS] =
        [const { AtomicBool::new(false) }; MAX_SOCKETS];
    static SOCKET_BROADCAST: [AtomicBool; MAX_SOCKETS] =
        [const { AtomicBool::new(false) }; MAX_SOCKETS];
    static SOCKET_NONBLOCK: [AtomicBool; MAX_SOCKETS] =
        [const { AtomicBool::new(false) }; MAX_SOCKETS];

    struct RecvBuffer {
        ptr: *mut u8,
        layout: Layout,
    }

    impl RecvBuffer {
        fn new(size: usize) -> Result<Self, i32> {
            let layout = Layout::from_size_align(size, 32).map_err(|_| IPC_ENOMEM)?;
            let ptr = unsafe { alloc_zeroed(layout) };
            if ptr.is_null() {
                Err(IPC_ENOMEM)
            } else {
                Ok(Self { ptr, layout })
            }
        }

        fn as_mut_slice(&mut self, len: usize) -> &mut [u8] {
            unsafe { core::slice::from_raw_parts_mut(self.ptr, len) }
        }
    }

    impl Drop for RecvBuffer {
        fn drop(&mut self) {
            unsafe { dealloc(self.ptr, self.layout) };
        }
    }

    fn socket_idx(fd: i32) -> Option<usize> {
        if fd < 0 {
            return None;
        }
        (0..MAX_SOCKETS).find(|&idx| {
            SOCKET_USED[idx].load(Ordering::Acquire)
                && SOCKET_RAW[idx].load(Ordering::Acquire) == fd
        })
    }

    fn socket_alloc(raw_socket: i32, type_: i32) -> Option<usize> {
        for idx in 0..MAX_SOCKETS {
            if !SOCKET_USED[idx].swap(true, Ordering::AcqRel) {
                SOCKET_RAW[idx].store(raw_socket, Ordering::Release);
                SOCKET_TYPE[idx].store(type_, Ordering::Release);
                super::SOCKET_TYPE[idx].store(type_, Ordering::Release);
                SOCKET_BOUND_PORT[idx].store(0, Ordering::Release);
                SOCKET_PEER_PORT[idx].store(0, Ordering::Release);
                SOCKET_BOUND_ADDR[idx].store(0, Ordering::Release);
                SOCKET_PEER_ADDR[idx].store(0, Ordering::Release);
                SOCKET_BOUND_VALID[idx].store(false, Ordering::Release);
                SOCKET_PEER_VALID[idx].store(false, Ordering::Release);
                SOCKET_REUSEADDR[idx].store(false, Ordering::Release);
                SOCKET_KEEPALIVE[idx].store(false, Ordering::Release);
                SOCKET_BROADCAST[idx].store(false, Ordering::Release);
                SOCKET_NONBLOCK[idx].store(false, Ordering::Release);
                reset_tracked_options(idx);
                return Some(idx);
            }
        }
        None
    }

    fn socket_store_bind(fd: i32, addr: &WiiSockAddrIn) {
        if let Some(idx) = socket_idx(fd) {
            SOCKET_BOUND_PORT[idx].store(addr.sin_port, Ordering::Release);
            SOCKET_BOUND_ADDR[idx].store(addr.sin_addr, Ordering::Release);
            SOCKET_BOUND_VALID[idx].store(true, Ordering::Release);
        }
    }

    fn socket_store_peer(fd: i32, addr: &WiiSockAddrIn) {
        if let Some(idx) = socket_idx(fd) {
            SOCKET_PEER_PORT[idx].store(addr.sin_port, Ordering::Release);
            SOCKET_PEER_ADDR[idx].store(addr.sin_addr, Ordering::Release);
            SOCKET_PEER_VALID[idx].store(true, Ordering::Release);
        }
    }

    fn socket_clear(fd: i32) {
        if let Some(idx) = socket_idx(fd) {
            SOCKET_USED[idx].store(false, Ordering::Release);
            SOCKET_RAW[idx].store(-1, Ordering::Release);
            SOCKET_TYPE[idx].store(0, Ordering::Release);
            super::SOCKET_TYPE[idx].store(0, Ordering::Release);
            SOCKET_BOUND_PORT[idx].store(0, Ordering::Release);
            SOCKET_PEER_PORT[idx].store(0, Ordering::Release);
            SOCKET_BOUND_ADDR[idx].store(0, Ordering::Release);
            SOCKET_PEER_ADDR[idx].store(0, Ordering::Release);
            SOCKET_BOUND_VALID[idx].store(false, Ordering::Release);
            SOCKET_PEER_VALID[idx].store(false, Ordering::Release);
            SOCKET_REUSEADDR[idx].store(false, Ordering::Release);
            SOCKET_KEEPALIVE[idx].store(false, Ordering::Release);
            SOCKET_BROADCAST[idx].store(false, Ordering::Release);
            SOCKET_NONBLOCK[idx].store(false, Ordering::Release);
            reset_tracked_options(idx);
        }
    }

    fn top_fd() -> Result<FileDescriptor, i32> {
        let raw = TOP_FD.load(Ordering::Acquire);
        if raw < 0 {
            Err(-ENXIO)
        } else {
            Ok(FileDescriptor::new(raw))
        }
    }

    pub fn set_network_top_fd(raw: i32) {
        TOP_FD.store(raw, Ordering::Release);
    }

    fn ioctl_u32s(cmd: i32, params: &mut [u32]) -> i32 {
        if params.len() > 3 {
            return -1;
        }
        let fd = match top_fd() {
            Ok(fd) => fd,
            Err(error) => return error,
        };
        let mut aligned = Aligned([0u32; 3]);
        aligned.0[..params.len()].copy_from_slice(params);
        let bytes = unsafe {
            core::slice::from_raw_parts_mut(
                aligned.0.as_mut_ptr().cast::<u8>(),
                core::mem::size_of_val(params),
            )
        };
        fd.ioctl(cmd, IoctlBuffers::from_slices(Some(bytes), None))
    }

    fn ioctl_struct<T: Copy>(cmd: i32, value: &mut T) -> i32 {
        let fd = match top_fd() {
            Ok(fd) => fd,
            Err(error) => return error,
        };
        let mut aligned = Aligned(*value);
        let bytes = unsafe {
            core::slice::from_raw_parts_mut((&mut aligned.0 as *mut T).cast::<u8>(), size_of::<T>())
        };
        fd.ioctl(cmd, IoctlBuffers::from_slices(Some(bytes), None))
    }

    fn ioctl_struct_inout<TIn: Copy, TOut: Copy>(
        cmd: i32,
        input: &mut TIn,
        output: &mut TOut,
    ) -> i32 {
        let fd = match top_fd() {
            Ok(fd) => fd,
            Err(error) => return error,
        };
        let mut aligned_in = Aligned(*input);
        let mut aligned_out = Aligned(*output);
        let in_bytes = unsafe {
            core::slice::from_raw_parts_mut(
                (&mut aligned_in.0 as *mut TIn).cast::<u8>(),
                size_of::<TIn>(),
            )
        };
        let out_bytes = unsafe {
            core::slice::from_raw_parts_mut(
                (&mut aligned_out.0 as *mut TOut).cast::<u8>(),
                size_of::<TOut>(),
            )
        };
        let result = fd.ioctl(
            cmd,
            IoctlBuffers::from_slices(Some(in_bytes), Some(out_bytes)),
        );
        if result >= 0 {
            *output = aligned_out.0;
        }
        result
    }

    fn ioctlv_sendto(data: *const c_void, len: i32, params: &mut SendToParams) -> i32 {
        let fd = match top_fd() {
            Ok(fd) => fd,
            Err(error) => return error,
        };
        let mut message = Aligned([0u8; MAX_SEND_BLOCK]);
        let aligned_params = Aligned(*params);
        let params_bytes = unsafe {
            core::slice::from_raw_parts(
                (&aligned_params.0 as *const SendToParams).cast::<u8>(),
                size_of::<SendToParams>(),
            )
        };
        let mut sent = 0usize;
        while sent < len as usize {
            let block = ((len as usize) - sent).min(MAX_SEND_BLOCK);
            unsafe {
                core::ptr::copy_nonoverlapping(
                    data.cast::<u8>().add(sent),
                    message.0.as_mut_ptr(),
                    block,
                );
            }
            let result = fd.ioctlv(
                IOCTLV_SO_SENDTO,
                2,
                [
                    IoctlvEntry::In(&message.0[..block]),
                    IoctlvEntry::In(params_bytes),
                ],
            );
            if result <= 0 || result as usize > block {
                return if sent == 0 {
                    result.min(0)
                } else {
                    sent as i32
                };
            }
            sent += result as usize;
        }
        sent as i32
    }

    fn ioctlv_recvfrom_block(
        message: &mut [u8],
        params: &mut [u32; 2],
        from: Option<&mut SockAddrIn>,
    ) -> i32 {
        let fd = match top_fd() {
            Ok(fd) => fd,
            Err(error) => return error,
        };
        let mut from_tmp = WiiSockAddrIn::default();
        let from_bytes = unsafe {
            core::slice::from_raw_parts_mut(
                (&mut from_tmp as *mut WiiSockAddrIn).cast::<u8>(),
                size_of::<WiiSockAddrIn>(),
            )
        };
        // C net_recv supplies a null, zero-length address output. Do not
        // request recvfrom address storage when the caller omitted it.
        let source_vector = if from.is_some() {
            IoctlvEntry::Out(from_bytes)
        } else {
            IoctlvEntry::OutNull
        };
        let argv = [
            IoctlvEntry::In(unsafe {
                core::slice::from_raw_parts_mut(params.as_mut_ptr().cast::<u8>(), 8)
            }),
            IoctlvEntry::Out(message),
            source_vector,
        ];
        let ret = fd.ioctlv(IOCTLV_SO_RECVFROM, 1, argv);
        if ret >= 0
            && let Some(out) = from
        {
            *out = SockAddrIn {
                sin_len: size_of::<SockAddrIn>() as u8,
                sin_family: from_tmp.sin_family,
                sin_port: from_tmp.sin_port,
                sin_addr: InAddr {
                    s_addr: from_tmp.sin_addr,
                },
                sin_zero: [0; 8],
            };
        }
        ret
    }

    pub(super) fn recv_result_after_block(
        total: usize,
        block_len: usize,
        received: i32,
    ) -> Result<usize, i32> {
        if received <= 0 {
            return Err(received);
        }
        if received as usize > block_len {
            return Err(-EOVERFLOW);
        }
        let new_total = total + received as usize;
        if received as usize != block_len {
            Err(new_total as i32)
        } else {
            Ok(new_total)
        }
    }

    fn sockaddr_in_to_wii(name: *mut SockAddr, namelen: SockLenT) -> Option<WiiSockAddrIn> {
        if name.is_null() || namelen < size_of::<WiiSockAddrIn>() as SockLenT {
            return None;
        }
        unsafe {
            let addr = core::ptr::read_unaligned(name.cast::<WiiSockAddrIn>());
            if addr.sin_family as i32 != AF_INET {
                return None;
            }
            Some(WiiSockAddrIn {
                sin_len: size_of::<WiiSockAddrIn>() as u8,
                sin_family: AF_INET as u8,
                sin_port: addr.sin_port,
                sin_addr: addr.sin_addr,
            })
        }
    }

    fn write_canonical_sockaddr_len(name: *mut SockAddr) {
        unsafe {
            core::ptr::addr_of_mut!((*name).sa_len).write(size_of::<WiiSockAddrIn>() as u8);
        }
    }

    fn write_wii_sockaddr(name: *mut SockAddr, namelen: *mut SockLenT, addr: WiiSockAddrIn) -> i32 {
        if name.is_null() || namelen.is_null() {
            return -1;
        }
        let name_len = unsafe { *namelen };
        if name_len < size_of::<WiiSockAddrIn>() as SockLenT {
            return -1;
        }
        unsafe {
            *(name as *mut SockAddrIn) = SockAddrIn {
                sin_len: size_of::<WiiSockAddrIn>() as u8,
                sin_family: addr.sin_family,
                sin_port: addr.sin_port,
                sin_addr: InAddr {
                    s_addr: addr.sin_addr,
                },
                sin_zero: [0; 8],
            };
            *namelen = size_of::<WiiSockAddrIn>() as SockLenT;
        }
        0
    }

    pub fn socket(domain: i32, type_: i32, protocol: i32) -> i32 {
        let mut params = [domain as u32, (type_ as u32) & !FLAG_MASK, protocol as u32];
        let ret = ioctl_u32s(IOCTL_SO_SOCKET, &mut params);
        if ret >= 0 {
            let Some(idx) = socket_alloc(ret, type_) else {
                let close_ret =
                    ioctl_struct(IOCTL_SO_CLOSE, &mut SocketFdParam { socket: ret as u32 });
                return if close_ret < 0 { close_ret } else { -1 };
            };

            if type_ & SOCK_NONBLOCK != 0 {
                apply_nonblocking_socket_flag(
                    ret,
                    |socket| {
                        let mut flags = [socket as u32, F_GETFL, 0];
                        ioctl_u32s(IOCTL_SO_FCNTL, &mut flags)
                    },
                    |socket, flags| {
                        let mut params = [socket as u32, F_SETFL, flags];
                        ioctl_u32s(IOCTL_SO_FCNTL, &mut params)
                    },
                );
                SOCKET_NONBLOCK[idx].store(true, Ordering::Release);
            }

            let window_size = DEFAULT_RCVBUF;
            let _ = unsafe {
                set_socket_option(
                    ret,
                    SOL_SOCKET,
                    SO_RCVBUF,
                    (&window_size as *const i32).cast(),
                    size_of::<i32>() as SockLenT,
                )
            };
        }
        ret
    }

    pub fn close(s: i32) -> i32 {
        let Some(_idx) = socket_idx(s) else {
            return -1;
        };
        let mut param = SocketFdParam { socket: s as u32 };
        let ret = ioctl_struct(IOCTL_SO_CLOSE, &mut param);
        if ret >= 0 {
            socket_clear(s);
        }
        ret
    }

    /// # Safety
    ///
    /// Every non-null raw pointer must satisfy the size, alignment, initialization, mutability, aliasing, and lifetime requirements implied by the other arguments and the underlying Wii/libogc operation.
    pub unsafe fn bind(s: i32, name: *mut SockAddr, namelen: SockLenT) -> i32 {
        if let Err(error) = top_fd() {
            return error;
        }
        if name.is_null() || namelen < 2 {
            return -EINVAL;
        }
        let family = unsafe { core::ptr::addr_of!((*name).sa_family).read() };
        if family as i32 != AF_INET {
            return -EAFNOSUPPORT;
        }
        if namelen < size_of::<WiiSockAddrIn>() as SockLenT {
            return -EINVAL;
        }
        let mut wii_name = unsafe { core::ptr::read_unaligned(name.cast::<WiiSockAddrIn>()) };
        wii_name.sin_len = size_of::<WiiSockAddrIn>() as u8;
        wii_name.sin_family = AF_INET as u8;
        write_canonical_sockaddr_len(name);
        let mut params = BindParams {
            socket: s as u32,
            has_name: 1,
            name: [0; 28],
        };
        unsafe {
            core::ptr::copy_nonoverlapping(
                (&wii_name as *const WiiSockAddrIn).cast::<u8>(),
                params.name.as_mut_ptr(),
                size_of::<WiiSockAddrIn>(),
            );
        }
        let ret = ioctl_struct(IOCTL_SO_BIND, &mut params);
        if ret >= 0 {
            socket_store_bind(s, &wii_name);
        }
        ret
    }

    pub fn shutdown(s: i32, how: u32) -> i32 {
        let mut params = [s as u32, how];
        let ret = ioctl_u32s(IOCTL_SO_SHUTDOWN, &mut params);
        if ret >= 0
            && let Some(idx) = socket_idx(s)
        {
            SOCKET_PEER_PORT[idx].store(0, Ordering::Release);
            SOCKET_PEER_ADDR[idx].store(0, Ordering::Release);
        }
        ret
    }

    /// # Safety
    ///
    /// Every non-null raw pointer must satisfy the size, alignment, initialization, mutability, aliasing, and lifetime requirements implied by the other arguments and the underlying Wii/libogc operation.
    pub unsafe fn get_peer_name(_s: i32, _name: *mut SockAddr, _namelen: *mut SockLenT) -> i32 {
        let Some(idx) = socket_idx(_s) else {
            return -1;
        };
        if !SOCKET_PEER_VALID[idx].load(Ordering::Acquire) {
            return -1;
        }
        let addr = WiiSockAddrIn {
            sin_len: size_of::<WiiSockAddrIn>() as u8,
            sin_family: AF_INET as u8,
            sin_port: SOCKET_PEER_PORT[idx].load(Ordering::Acquire),
            sin_addr: SOCKET_PEER_ADDR[idx].load(Ordering::Acquire),
        };
        write_wii_sockaddr(_name, _namelen, addr)
    }

    /// # Safety
    ///
    /// Every non-null raw pointer must satisfy the size, alignment, initialization, mutability, aliasing, and lifetime requirements implied by the other arguments and the underlying Wii/libogc operation.
    pub unsafe fn get_socket_name(s: i32, name: *mut SockAddr, namelen: *mut SockLenT) -> i32 {
        if name.is_null() || namelen.is_null() {
            return -1;
        }
        if unsafe { *namelen } < size_of::<WiiSockAddrIn>() as SockLenT {
            return -1;
        }
        let mut socket = s as u32;
        let mut out = WiiSockAddrIn::default();
        let ret = ioctl_struct_inout(IOCTL_SO_GETSOCKNAME, &mut socket, &mut out);
        if ret < 0 {
            return ret;
        }
        write_wii_sockaddr(name, namelen, out)
    }

    /// # Safety
    ///
    /// Every non-null raw pointer must satisfy the size, alignment, initialization, mutability, aliasing, and lifetime requirements implied by the other arguments and the underlying Wii/libogc operation.
    pub unsafe fn get_socket_option(
        _s: i32,
        _level: i32,
        _optname: i32,
        _optval: *mut c_void,
        _optlen: *mut SockLenT,
    ) -> i32 {
        // Wii libogc leaves net_getsockopt unimplemented and always reports EINVAL.
        -EINVAL
    }

    /// # Safety
    ///
    /// Every non-null raw pointer must satisfy the size, alignment, initialization, mutability, aliasing, and lifetime requirements implied by the other arguments and the underlying Wii/libogc operation.
    pub unsafe fn set_socket_option(
        s: i32,
        level: i32,
        optname: i32,
        optval: *const c_void,
        optlen: SockLenT,
    ) -> i32 {
        if let Err(error) = top_fd() {
            return error;
        }
        if optlen as usize > 20 {
            return -EINVAL;
        }
        let mut params = SetSockOptParams {
            socket: s as u32,
            level: level as u32,
            optname: crate::option_to_ios(level as u32, optname as u32),
            optlen,
            optval: [0; 20],
        };
        unsafe { copy_setsockopt_payload(&mut params.optval, optval, optlen) };
        ioctl_struct(IOCTL_SO_SETSOCKOPT, &mut params)
    }

    /// # Safety
    ///
    /// Every non-null raw pointer must satisfy the size, alignment, initialization, mutability, aliasing, and lifetime requirements implied by the other arguments and the underlying Wii/libogc operation.
    pub unsafe fn connect(s: i32, name: *mut SockAddr, namelen: SockLenT) -> i32 {
        if let Err(error) = top_fd() {
            return error;
        }
        if name.is_null() || namelen < 2 {
            return -EINVAL;
        }
        let family = unsafe { core::ptr::addr_of!((*name).sa_family).read() };
        if family as i32 != AF_INET {
            return -EAFNOSUPPORT;
        }
        if namelen < size_of::<WiiSockAddrIn>() as SockLenT {
            return -EINVAL;
        }
        write_canonical_sockaddr_len(name);
        let mut wii_name = unsafe { core::ptr::read_unaligned(name.cast::<WiiSockAddrIn>()) };
        wii_name.sin_len = size_of::<WiiSockAddrIn>() as u8;
        wii_name.sin_family = AF_INET as u8;
        let mut params = ConnectParams {
            socket: s as u32,
            has_addr: 1,
            addr: [0; 28],
        };
        unsafe {
            core::ptr::copy_nonoverlapping(
                (&wii_name as *const WiiSockAddrIn).cast::<u8>(),
                params.addr.as_mut_ptr(),
                size_of::<WiiSockAddrIn>(),
            );
        }
        let ret = ioctl_struct(IOCTL_SO_CONNECT, &mut params);
        if ret >= 0 {
            socket_store_peer(s, &wii_name);
        }
        ret
    }

    pub fn listen(s: i32, backlog: i32) -> i32 {
        let mut params = [s as u32, backlog as u32];
        let ret = ioctl_u32s(IOCTL_SO_LISTEN, &mut params);
        if ret >= 0
            && let Some(idx) = socket_idx(s)
        {
            SOCKET_LISTENING[idx].store(true, Ordering::Release);
        }
        ret
    }

    /// # Safety
    ///
    /// Every non-null raw pointer must satisfy the size, alignment, initialization, mutability, aliasing, and lifetime requirements implied by the other arguments and the underlying Wii/libogc operation.
    pub unsafe fn accept(s: i32, addr: *mut SockAddr, addrlen: *mut SockLenT) -> i32 {
        if addr.is_null() || addrlen.is_null() {
            return -1;
        }
        if unsafe { *addrlen } < size_of::<WiiSockAddrIn>() as SockLenT {
            return -1;
        }
        let Some(listener_idx) = socket_idx(s) else {
            return -1;
        };
        let mut socket = s as u32;
        let mut out = WiiSockAddrIn::for_accept();
        let ret = ioctl_struct_inout(IOCTL_SO_ACCEPT, &mut socket, &mut out);
        if ret < 0 {
            return ret;
        }
        let rc = write_wii_sockaddr(addr, addrlen, out);
        if rc < 0 {
            rc
        } else {
            if socket_alloc(ret, SOCKET_TYPE[listener_idx].load(Ordering::Acquire)).is_none() {
                let mut param = SocketFdParam { socket: ret as u32 };
                let close_ret = ioctl_struct(IOCTL_SO_CLOSE, &mut param);
                return if close_ret < 0 { close_ret } else { -1 };
            }
            socket_store_peer(
                ret,
                &WiiSockAddrIn {
                    sin_len: out.sin_len,
                    sin_family: out.sin_family,
                    sin_port: out.sin_port,
                    sin_addr: out.sin_addr,
                },
            );
            ret
        }
    }

    /// # Safety
    ///
    /// Every non-null raw pointer must satisfy the size, alignment, initialization, mutability, aliasing, and lifetime requirements implied by the other arguments and the underlying Wii/libogc operation.
    pub unsafe fn recv(s: i32, mem: *mut c_void, len: i32, flags: u32) -> i32 {
        unsafe {
            receive_from(
                s,
                mem,
                len,
                flags,
                core::ptr::null_mut(),
                core::ptr::null_mut(),
            )
        }
    }

    /// # Safety
    ///
    /// Every non-null raw pointer must satisfy the size, alignment, initialization, mutability, aliasing, and lifetime requirements implied by the other arguments and the underlying Wii/libogc operation.
    pub unsafe fn read(s: i32, mem: *mut c_void, len: i32) -> i32 {
        unsafe { recv(s, mem, len, 0) }
    }

    /// # Safety
    ///
    /// Every non-null raw pointer must satisfy the size, alignment, initialization, mutability, aliasing, and lifetime requirements implied by the other arguments and the underlying Wii/libogc operation.
    pub unsafe fn receive_from(
        s: i32,
        mem: *mut c_void,
        len: i32,
        flags: u32,
        from: *mut SockAddr,
        fromlen: *mut SockLenT,
    ) -> i32 {
        if let Err(error) = top_fd() {
            return error;
        }
        if len <= 0 {
            return -EINVAL;
        }
        if mem.is_null() {
            // Safety deviation: C would later copy through this pointer.
            return -1;
        }

        let total_len = len as usize;
        let first_block = total_len.min(MAX_RECV_BLOCK);
        let mut message = match RecvBuffer::new(first_block) {
            Ok(buffer) => buffer,
            Err(error) => return error,
        };
        let mut params = [s as u32, crate::ios_socket_flags::to_ios(flags)];
        let mut from_out = if from.is_null() {
            None
        } else {
            Some(unsafe { &mut *(from as *mut SockAddrIn) })
        };

        let mut total = 0usize;
        loop {
            let block_len = (total_len - total).min(MAX_RECV_BLOCK);
            let block = message.as_mut_slice(block_len);
            block.fill(0);
            let received = ioctlv_recvfrom_block(block, &mut params, from_out.as_deref_mut());

            if !fromlen.is_null() && !from.is_null() {
                unsafe {
                    *fromlen = (*(from as *const SockAddr)).sa_len as SockLenT;
                }
            }

            match recv_result_after_block(total, block_len, received) {
                Ok(new_total) => {
                    unsafe {
                        core::ptr::copy_nonoverlapping(
                            block.as_ptr(),
                            mem.cast::<u8>().add(total),
                            received as usize,
                        );
                    }
                    total = new_total;
                    if total == total_len {
                        return total as i32;
                    }
                }
                Err(result) => {
                    if received > 0 && received as usize <= block_len {
                        unsafe {
                            core::ptr::copy_nonoverlapping(
                                block.as_ptr(),
                                mem.cast::<u8>().add(total),
                                received as usize,
                            );
                        }
                    }
                    return result;
                }
            }
        }
    }

    /// # Safety
    ///
    /// Every non-null raw pointer must satisfy the size, alignment, initialization, mutability, aliasing, and lifetime requirements implied by the other arguments and the underlying Wii/libogc operation.
    pub unsafe fn send(s: i32, data: *const c_void, size: i32, flags: u32) -> i32 {
        unsafe { send_to(s, data, size, flags, core::ptr::null_mut(), 0) }
    }

    /// # Safety
    ///
    /// Every non-null raw pointer must satisfy the size, alignment, initialization, mutability, aliasing, and lifetime requirements implied by the other arguments and the underlying Wii/libogc operation.
    pub unsafe fn send_to(
        s: i32,
        data: *const c_void,
        size: i32,
        flags: u32,
        to: *mut SockAddr,
        tolen: SockLenT,
    ) -> i32 {
        if data.is_null() || size < 0 {
            return -1;
        }
        if !to.is_null() && tolen < size_of::<WiiSockAddrIn>() as SockLenT {
            return -1;
        }
        let mut params = SendToParams {
            socket: s as u32,
            flags: crate::ios_socket_flags::to_ios(flags),
            has_destaddr: 0,
            destaddr: [0; 28],
        };
        if let Some(wii_to) = sockaddr_in_to_wii(to, tolen) {
            params.has_destaddr = 1;
            unsafe {
                core::ptr::copy_nonoverlapping(
                    (&wii_to as *const WiiSockAddrIn).cast::<u8>(),
                    params.destaddr.as_mut_ptr(),
                    size_of::<WiiSockAddrIn>(),
                );
            }
        }
        ioctlv_sendto(data, size, &mut params)
    }

    /// # Safety
    ///
    /// Every non-null raw pointer must satisfy the size, alignment, initialization, mutability, aliasing, and lifetime requirements implied by the other arguments and the underlying Wii/libogc operation.
    pub unsafe fn write(s: i32, data: *const c_void, size: i32) -> i32 {
        unsafe { send(s, data, size, 0) }
    }

    /// # Safety
    ///
    /// Every non-null raw pointer must satisfy the size, alignment, initialization, mutability, aliasing, and lifetime requirements implied by the other arguments and the underlying Wii/libogc operation.
    pub unsafe fn select(
        maxfdp1: i32,
        readset: *mut FdSet,
        writeset: *mut FdSet,
        exceptset: *mut FdSet,
        timeout: *mut TimeVal,
    ) -> i32 {
        unsafe {
            let mut sds = [PollFd {
                fd: -1,
                events: 0,
                revents: 0,
            }; FD_SETSIZE];
            let mut count = 0usize;

            {
                for fd in 0..maxfdp1.max(0) as usize {
                    let mut events = 0i16;
                    if !readset.is_null() && ((*readset).fd_bits[fd / 8] & (1 << (fd & 7))) != 0 {
                        events |= POLLIN;
                    }
                    if !writeset.is_null() && ((*writeset).fd_bits[fd / 8] & (1 << (fd & 7))) != 0 {
                        events |= POLLOUT;
                    }
                    if !exceptset.is_null() && ((*exceptset).fd_bits[fd / 8] & (1 << (fd & 7))) != 0
                    {
                        events |= POLLPRI;
                    }
                    if events != 0 {
                        sds[count] = PollFd {
                            fd: fd as i32,
                            events,
                            revents: 0,
                        };
                        count += 1;
                    }
                }
            }

            let timeout_ms = if timeout.is_null() {
                -1
            } else {
                (*timeout).tv_sec.saturating_mul(1000) + (*timeout).tv_usec / 1000
            };
            let ret = poll(sds.as_mut_ptr(), count as i32, timeout_ms);
            if ret < 0 {
                return ret;
            }

            {
                if !readset.is_null() {
                    ptr::write_bytes(readset.cast::<u8>(), 0, size_of::<FdSet>());
                }
                if !writeset.is_null() {
                    ptr::write_bytes(writeset.cast::<u8>(), 0, size_of::<FdSet>());
                }
                if !exceptset.is_null() {
                    ptr::write_bytes(exceptset.cast::<u8>(), 0, size_of::<FdSet>());
                }
                for entry in sds.iter().take(count) {
                    if entry.fd < 0 {
                        continue;
                    }
                    let fd = entry.fd as usize;
                    if !readset.is_null() && (entry.revents & POLLIN) != 0 {
                        (*readset).fd_bits[fd / 8] |= 1 << (fd & 7);
                    }
                    if !writeset.is_null() && (entry.revents & POLLOUT) != 0 {
                        (*writeset).fd_bits[fd / 8] |= 1 << (fd & 7);
                    }
                    if !exceptset.is_null() && (entry.revents & (POLLERR | POLLPRI | POLLHUP)) != 0
                    {
                        (*exceptset).fd_bits[fd / 8] |= 1 << (fd & 7);
                    }
                }
            }

            ret
        }
    }

    /// # Safety
    ///
    /// Every non-null raw pointer must satisfy the size, alignment, initialization, mutability, aliasing, and lifetime requirements implied by the other arguments and the underlying Wii/libogc operation.
    pub unsafe fn poll(fds: *mut PollFd, nfds: i32, timeout: i32) -> i32 {
        if fds.is_null() || nfds < 0 {
            return -1;
        }
        let count = nfds as usize;
        let mut backend = Vec::with_capacity(count);

        unsafe {
            let slice = core::slice::from_raw_parts_mut(fds, count);
            for entry in slice.iter_mut() {
                entry.revents = 0;
                backend.push(WiiPollSd {
                    socket: if entry.fd < 0 { -1 } else { entry.fd },
                    events: pollfd_events_to_pollsd(entry.events),
                    revents: 0,
                });
            }
        }

        let mut params = [0u32; 2];
        params[1] = timeout as u32;
        let fd = match top_fd() {
            Ok(fd) => fd,
            Err(error) => return error,
        };
        let bytes = unsafe {
            core::slice::from_raw_parts_mut(
                backend.as_mut_ptr().cast::<u8>(),
                count * size_of::<WiiPollSd>(),
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
            let slice = core::slice::from_raw_parts_mut(fds, count);
            for (idx, entry) in slice.iter_mut().enumerate() {
                entry.revents = pollsd_events_to_pollfd(backend[idx].revents);
            }
        }
        ret
    }

    /// # Safety
    ///
    /// Every non-null raw pointer must satisfy the size, alignment, initialization, mutability, aliasing, and lifetime requirements implied by the other arguments and the underlying Wii/libogc operation.
    pub unsafe fn ioctl(s: i32, cmd: i64, argp: *mut c_void) -> i32 {
        if let Err(error) = top_fd() {
            return error;
        }
        if argp.is_null() {
            return -EINVAL;
        }
        match cmd {
            FIONBIO => {
                let mut flags = [s as u32, F_GETFL, 0];
                let rc = ioctl_u32s(IOCTL_SO_FCNTL, &mut flags);
                if rc < 0 {
                    return rc;
                }
                flags[2] &= !IOS_O_NONBLOCK;
                if unsafe { *(argp as *mut u32) } != 0 {
                    flags[2] |= IOS_O_NONBLOCK;
                }
                flags[1] = F_SETFL;
                let ret = ioctl_u32s(IOCTL_SO_FCNTL, &mut flags);
                if ret >= 0
                    && let Some(idx) = socket_idx(s)
                {
                    SOCKET_NONBLOCK[idx]
                        .store(unsafe { *(argp as *mut u32) } != 0, Ordering::Release);
                }
                ret
            }
            _ => -EINVAL,
        }
    }
}

#[cfg(not(target_arch = "powerpc"))]
mod wii_backend {
    use super::*;

    pub fn socket(_domain: i32, _type_: i32, _protocol: i32) -> i32 {
        -1
    }
    pub fn close(_s: i32) -> i32 {
        -1
    }
    pub fn bind(_s: i32, _name: *mut SockAddr, _namelen: SockLenT) -> i32 {
        -1
    }
    pub fn shutdown(_s: i32, _how: u32) -> i32 {
        -1
    }
    pub fn get_peer_name(_s: i32, _name: *mut SockAddr, _namelen: *mut SockLenT) -> i32 {
        -1
    }
    pub fn get_socket_name(_s: i32, _name: *mut SockAddr, _namelen: *mut SockLenT) -> i32 {
        -1
    }
    pub fn get_socket_option(
        _s: i32,
        _level: i32,
        _optname: i32,
        _optval: *mut c_void,
        _optlen: *mut SockLenT,
    ) -> i32 {
        -1
    }
    pub fn set_socket_option(
        _s: i32,
        _level: i32,
        _optname: i32,
        _optval: *const c_void,
        _optlen: SockLenT,
    ) -> i32 {
        -1
    }
    pub fn connect(_s: i32, _name: *mut SockAddr, _namelen: SockLenT) -> i32 {
        -1
    }
    pub fn listen(_s: i32, _backlog: i32) -> i32 {
        -1
    }
    pub fn accept(_s: i32, _addr: *mut SockAddr, _addrlen: *mut SockLenT) -> i32 {
        -1
    }
    pub fn recv(_s: i32, _mem: *mut c_void, _len: i32, _flags: u32) -> i32 {
        -1
    }
    pub fn read(_s: i32, _mem: *mut c_void, _len: i32) -> i32 {
        -1
    }
    pub fn receive_from(
        _s: i32,
        _mem: *mut c_void,
        _len: i32,
        _flags: u32,
        _from: *mut SockAddr,
        _fromlen: *mut SockLenT,
    ) -> i32 {
        -1
    }
    pub fn send(_s: i32, _dataptr: *const c_void, _size: i32, _flags: u32) -> i32 {
        -1
    }
    pub fn send_to(
        _s: i32,
        _dataptr: *const c_void,
        _size: i32,
        _flags: u32,
        _to: *mut SockAddr,
        _tolen: SockLenT,
    ) -> i32 {
        -1
    }
    pub fn write(_s: i32, _dataptr: *const c_void, _size: i32) -> i32 {
        -1
    }
    pub fn select(
        _maxfdp1: i32,
        _readset: *mut FdSet,
        _writeset: *mut FdSet,
        _exceptset: *mut FdSet,
        _timeout: *mut TimeVal,
    ) -> i32 {
        0
    }
    pub fn poll(_fds: *mut PollFd, _nfds: i32, _timeout: i32) -> i32 {
        -1
    }
    pub fn ioctl(_s: i32, _cmd: i64, _argp: *mut c_void) -> i32 {
        -1
    }
}

/// Shares the IOS socket service handle created by `ogc-network-sys::net_init`.
/// The network owner closes the handle after socket shutdown.
#[cfg(target_arch = "powerpc")]
pub fn set_network_top_fd(raw: i32) {
    wii_backend::set_network_top_fd(raw);
}

pub fn socket(domain: i32, type_: i32, protocol: i32) -> i32 {
    powerpc_return!(wii_backend::socket(domain, type_, protocol));
    runtime_unreachable()
}

pub fn close(s: i32) -> i32 {
    powerpc_return!(wii_backend::close(s));
    runtime_unreachable()
}

/// # Safety
///
/// Every non-null pointer must be properly aligned and valid for all reads or
/// writes implied by the arguments and the Wii socket operation. Referenced
/// memory must remain valid for the duration of the call, and Rust aliasing
/// requirements must be upheld.
pub unsafe fn bind(s: i32, name: *mut SockAddr, namelen: SockLenT) -> i32 {
    powerpc_return!(unsafe { wii_backend::bind(s, name, namelen) });
    runtime_unreachable()
}

pub fn shutdown(s: i32, _how: i32) -> i32 {
    powerpc_return!(wii_backend::shutdown(s, _how as u32));
    runtime_unreachable()
}

/// # Safety
///
/// Every non-null pointer must be properly aligned and valid for all reads or
/// writes implied by the arguments and the Wii socket operation. Referenced
/// memory must remain valid for the duration of the call, and Rust aliasing
/// requirements must be upheld.
pub unsafe fn get_peer_name(s: i32, name: *mut SockAddr, namelen: *mut SockLenT) -> i32 {
    powerpc_return!(unsafe { wii_backend::get_peer_name(s, name, namelen) });
    runtime_unreachable()
}

/// # Safety
///
/// Every non-null pointer must be properly aligned and valid for all reads or
/// writes implied by the arguments and the Wii socket operation. Referenced
/// memory must remain valid for the duration of the call, and Rust aliasing
/// requirements must be upheld.
pub unsafe fn get_socket_name(s: i32, name: *mut SockAddr, namelen: *mut SockLenT) -> i32 {
    powerpc_return!(unsafe { wii_backend::get_socket_name(s, name, namelen) });
    runtime_unreachable()
}

/// # Safety
///
/// Every non-null pointer must be properly aligned and valid for all reads or
/// writes implied by the arguments and the Wii socket operation. Referenced
/// memory must remain valid for the duration of the call, and Rust aliasing
/// requirements must be upheld.
pub unsafe fn get_socket_option(
    s: i32,
    level: i32,
    optname: i32,
    optval: *mut c_void,
    optlen: *mut SockLenT,
) -> i32 {
    powerpc_return!(unsafe { wii_backend::get_socket_option(s, level, optname, optval, optlen) });
    runtime_unreachable()
}

/// # Safety
///
/// Every non-null pointer must be properly aligned and valid for all reads or
/// writes implied by the arguments and the Wii socket operation. Referenced
/// memory must remain valid for the duration of the call, and Rust aliasing
/// requirements must be upheld.
pub unsafe fn set_socket_option(
    s: i32,
    level: i32,
    optname: i32,
    optval: *const c_void,
    optlen: SockLenT,
) -> i32 {
    powerpc_return!(unsafe { wii_backend::set_socket_option(s, level, optname, optval, optlen) });
    runtime_unreachable()
}

/// # Safety
///
/// Every non-null pointer must be properly aligned and valid for all reads or
/// writes implied by the arguments and the Wii socket operation. Referenced
/// memory must remain valid for the duration of the call, and Rust aliasing
/// requirements must be upheld.
pub unsafe fn connect(s: i32, name: *mut SockAddr, namelen: SockLenT) -> i32 {
    powerpc_return!(unsafe { wii_backend::connect(s, name, namelen) });
    runtime_unreachable()
}

pub fn listen(s: i32, _backlog: i32) -> i32 {
    powerpc_return!(wii_backend::listen(s, _backlog));
    runtime_unreachable()
}

/// # Safety
///
/// Every non-null pointer must be properly aligned and valid for all reads or
/// writes implied by the arguments and the Wii socket operation. Referenced
/// memory must remain valid for the duration of the call, and Rust aliasing
/// requirements must be upheld.
pub unsafe fn accept(s: i32, addr: *mut SockAddr, addrlen: *mut SockLenT) -> i32 {
    powerpc_return!(unsafe { wii_backend::accept(s, addr, addrlen) });
    runtime_unreachable()
}

/// # Safety
///
/// Every non-null pointer must be properly aligned and valid for all reads or
/// writes implied by the arguments and the Wii socket operation. Referenced
/// memory must remain valid for the duration of the call, and Rust aliasing
/// requirements must be upheld.
pub unsafe fn recv(s: i32, mem: *mut c_void, len: i32, _flags: u32) -> i32 {
    powerpc_return!(unsafe { wii_backend::recv(s, mem, len, _flags) });
    runtime_unreachable()
}

/// # Safety
///
/// Every non-null pointer must be properly aligned and valid for all reads or
/// writes implied by the arguments and the Wii socket operation. Referenced
/// memory must remain valid for the duration of the call, and Rust aliasing
/// requirements must be upheld.
pub unsafe fn read(s: i32, mem: *mut c_void, len: i32) -> i32 {
    powerpc_return!(unsafe { wii_backend::read(s, mem, len) });
    runtime_unreachable()
}

/// # Safety
///
/// Every non-null pointer must be properly aligned and valid for all reads or
/// writes implied by the arguments and the Wii socket operation. Referenced
/// memory must remain valid for the duration of the call, and Rust aliasing
/// requirements must be upheld.
pub unsafe fn receive_from(
    s: i32,
    mem: *mut c_void,
    len: i32,
    flags: u32,
    from: *mut SockAddr,
    fromlen: *mut SockLenT,
) -> i32 {
    powerpc_return!(unsafe { wii_backend::receive_from(s, mem, len, flags, from, fromlen) });
    runtime_unreachable()
}

/// # Safety
///
/// Every non-null pointer must be properly aligned and valid for all reads or
/// writes implied by the arguments and the Wii socket operation. Referenced
/// memory must remain valid for the duration of the call, and Rust aliasing
/// requirements must be upheld.
pub unsafe fn send(s: i32, dataptr: *const c_void, size: i32, _flags: u32) -> i32 {
    powerpc_return!(unsafe { wii_backend::send(s, dataptr, size, _flags) });
    runtime_unreachable()
}

/// # Safety
///
/// Every non-null pointer must be properly aligned and valid for all reads or
/// writes implied by the arguments and the Wii socket operation. Referenced
/// memory must remain valid for the duration of the call, and Rust aliasing
/// requirements must be upheld.
pub unsafe fn send_to(
    s: i32,
    dataptr: *const c_void,
    size: i32,
    _flags: u32,
    _to: *mut SockAddr,
    _tolen: SockLenT,
) -> i32 {
    powerpc_return!(unsafe { wii_backend::send_to(s, dataptr, size, _flags, _to, _tolen) });
    runtime_unreachable()
}

/// # Safety
///
/// Every non-null pointer must be properly aligned and valid for all reads or
/// writes implied by the arguments and the Wii socket operation. Referenced
/// memory must remain valid for the duration of the call, and Rust aliasing
/// requirements must be upheld.
pub unsafe fn write(s: i32, dataptr: *const c_void, size: i32) -> i32 {
    powerpc_return!(unsafe { wii_backend::write(s, dataptr, size) });
    runtime_unreachable()
}

/// # Safety
///
/// Every non-null pointer must be properly aligned and valid for all reads or
/// writes implied by the arguments and the Wii socket operation. Referenced
/// memory must remain valid for the duration of the call, and Rust aliasing
/// requirements must be upheld.
pub unsafe fn select(
    maxfdp1: i32,
    readset: *mut FdSet,
    _writeset: *mut FdSet,
    _exceptset: *mut FdSet,
    _timeout: *mut TimeVal,
) -> i32 {
    powerpc_return!(unsafe {
        wii_backend::select(maxfdp1, readset, _writeset, _exceptset, _timeout)
    });
    runtime_unreachable()
}

/// # Safety
///
/// Every non-null pointer must be properly aligned and valid for all reads or
/// writes implied by the arguments and the Wii socket operation. Referenced
/// memory must remain valid for the duration of the call, and Rust aliasing
/// requirements must be upheld.
pub unsafe fn poll(fds: *mut PollFd, nfds: i32, timeout: i32) -> i32 {
    powerpc_return!(unsafe { wii_backend::poll(fds, nfds, timeout) });
    runtime_unreachable()
}

/// # Safety
///
/// Every non-null pointer must be properly aligned and valid for all reads or
/// writes implied by the arguments and the Wii socket operation. Referenced
/// memory must remain valid for the duration of the call, and Rust aliasing
/// requirements must be upheld.
pub unsafe fn ioctl(s: i32, cmd: i64, argp: *mut c_void) -> i32 {
    powerpc_return!(unsafe { wii_backend::ioctl(s, cmd, argp) });
    runtime_unreachable()
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    extern crate std;

    use super::*;
    use core::mem::size_of;
    use std::sync::Mutex;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn constants_and_layouts_match_checkpoint() {
        let _guard = match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        assert_eq!(ERR_ISCONN, -12);
        assert_eq!(SOCK_STREAM, 1);
        assert_eq!(AF_INET, 2);
        assert_eq!(FD_SETSIZE, 16);
        assert_eq!(size_of::<InAddr>(), 4);
        assert_eq!(size_of::<FdSet>(), 2);
        assert_eq!(size_of::<TimeVal>(), 8);
    }

    #[repr(align(4))]
    struct ShortSockAddr([u8; 2]);

    #[test]
    fn recv_chunk_result_matches_c_precedence() {
        assert_eq!(
            wii_backend::recv_result_after_block(0, 32 * 1024, 32 * 1024),
            Ok(32 * 1024)
        );
        assert_eq!(
            wii_backend::recv_result_after_block(32 * 1024, 100, 40),
            Err((32 * 1024 + 40) as i32)
        );
        assert_eq!(
            wii_backend::recv_result_after_block(32 * 1024, 100, 0),
            Err(0)
        );
        assert_eq!(
            wii_backend::recv_result_after_block(32 * 1024, 100, -5),
            Err(-5)
        );
        assert_eq!(
            wii_backend::recv_result_after_block(32 * 1024, 100, 101),
            Err(-EOVERFLOW)
        );
    }

    #[test]
    fn recv_checks_descriptor_before_length() {
        let _guard = match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        super::set_network_top_fd(-1);
        let result = unsafe { recv(0, ptr::null_mut(), 0, 0) };
        super::set_network_top_fd(0);
        assert_eq!(result, -ENXIO);

        let result = unsafe { recv(0, ptr::null_mut(), 0, 0) };
        super::set_network_top_fd(-1);
        assert_eq!(result, -EINVAL);
    }

    #[test]
    fn ioctl_without_network_fd_precedes_null_argument_error() {
        let _guard = match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        super::set_network_top_fd(-1);
        let result = unsafe { ioctl(0, FIONBIO, ptr::null_mut()) };
        super::set_network_top_fd(0);
        assert_eq!(result, -ENXIO);
    }

    #[test]
    fn connect_rejects_short_address_after_reading_only_family() {
        let _guard = match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        super::set_network_top_fd(0);
        let mut short = ShortSockAddr([2, AF_INET as u8]);
        let result = unsafe { connect(0, short.0.as_mut_ptr().cast(), short.0.len() as SockLenT) };
        super::set_network_top_fd(-1);
        assert_eq!(result, -EINVAL);
        assert_eq!(short.0[0], 2);
    }

    #[test]
    fn socket_nonblock_calls_setfl_even_when_getfl_fails() {
        let socket = 7;
        let setfl_called = core::cell::Cell::new(false);
        let setfl_args = core::cell::Cell::new((0, 0));
        let result = wii_backend::apply_nonblocking_socket_flag(
            socket,
            |_| -5,
            |called_socket, flags| {
                setfl_called.set(true);
                setfl_args.set((called_socket, flags));
                -1
            },
        );
        assert!(setfl_called.get());
        assert_eq!(result, socket);
        let (set_socket, flags) = setfl_args.get();
        assert_eq!(set_socket, socket);
        assert_eq!(flags, (-5i32 as u32) | 0x04);
    }

    #[test]
    fn getsockopt_matches_wii_unimplemented_result() {
        assert_eq!(
            unsafe { get_socket_option(7, SOL_SOCKET, SO_TYPE, ptr::null_mut(), ptr::null_mut()) },
            -EINVAL
        );
    }

    #[test]
    fn setsockopt_null_payload_keeps_zero_filled_ios_buffer() {
        let mut payload = [0_u8; 20];
        unsafe {
            wii_backend::copy_setsockopt_payload(
                &mut payload,
                ptr::null(),
                size_of::<i32>() as SockLenT,
            )
        };
        assert_eq!(payload, [0; 20]);

        let value = 0x1234_5678_i32;
        unsafe {
            wii_backend::copy_setsockopt_payload(
                &mut payload,
                (&value as *const i32).cast(),
                size_of::<i32>() as SockLenT,
            )
        };
        assert_eq!(&payload[..4], &value.to_ne_bytes());
    }

    #[test]
    fn socket_bind_connect_send_recv_and_close_are_host_compat() {
        let _guard = match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        unsafe {
            let fd = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
            assert!(fd >= 0);
            let addr = SockAddrIn {
                sin_len: size_of::<SockAddrIn>() as u8,
                sin_family: AF_INET as u8,
                sin_port: 8080,
                sin_addr: InAddr { s_addr: INADDR_ANY },
                sin_zero: [0; 8],
            };
            let mut addr_copy = addr;
            addr_copy.sin_len = 1;
            assert_eq!(
                bind(
                    fd,
                    (&mut addr_copy as *mut SockAddrIn).cast(),
                    size_of::<SockAddrIn>() as SockLenT
                ),
                0
            );
            assert_eq!(addr_copy.sin_len, 8);
            addr_copy.sin_len = 2;
            assert_eq!(
                connect(
                    fd,
                    (&mut addr_copy as *mut SockAddrIn).cast(),
                    size_of::<SockAddrIn>() as SockLenT
                ),
                0
            );
            assert_eq!(addr_copy.sin_len, 8);
            let mut rcvbuf = 0_i32;
            let mut rcvbuf_len = size_of::<i32>() as SockLenT;
            assert_eq!(
                get_socket_option(
                    fd,
                    SOL_SOCKET,
                    SO_RCVBUF,
                    (&mut rcvbuf as *mut i32).cast(),
                    &mut rcvbuf_len
                ),
                -EINVAL
            );
            assert_eq!(rcvbuf, 0);
            let mut pending = 0x1234_5678_u32;
            assert_eq!(
                ioctl(fd, FIONREAD, (&mut pending as *mut u32).cast()),
                -EINVAL
            );
            assert_eq!(pending, 0x1234_5678);
            let mut recv_buf = [1_u8; 8];
            assert_eq!(recv(fd, recv_buf.as_mut_ptr().cast(), 8, 0), 4);
            assert_eq!(&recv_buf[..4], &[0, 0, 0, 0]);
            let send_buf = [9_u8; 3];
            assert_eq!(
                send(fd, send_buf.as_ptr().cast(), send_buf.len() as i32, 0),
                3
            );
            assert_eq!(close(fd), 0);
        }
    }

    #[test]
    fn socket_options_accept_and_select_are_host_compat() {
        let _guard = match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        unsafe {
            let fd = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
            assert!(fd >= 0);
            let reuse = 1_i32;
            assert_eq!(
                set_socket_option(
                    fd,
                    SOL_SOCKET,
                    SO_REUSEADDR,
                    (&reuse as *const i32).cast(),
                    size_of::<i32>() as SockLenT
                ),
                0
            );
            let mut acceptconn = -1_i32;
            let mut acceptconn_len = size_of::<i32>() as SockLenT;
            assert_eq!(
                get_socket_option(
                    fd,
                    SOL_SOCKET,
                    SO_ACCEPTCONN,
                    (&mut acceptconn as *mut i32).cast(),
                    &mut acceptconn_len,
                ),
                -EINVAL
            );
            assert_eq!(acceptconn, -1);
            let mut ty = -1_i32;
            let mut ty_len = size_of::<i32>() as SockLenT;
            assert_eq!(
                get_socket_option(
                    fd,
                    SOL_SOCKET,
                    SO_TYPE,
                    (&mut ty as *mut i32).cast(),
                    &mut ty_len
                ),
                -EINVAL
            );
            assert_eq!(ty, -1);

            assert_eq!(listen(fd, 1), 0);
            acceptconn_len = size_of::<i32>() as SockLenT;
            assert_eq!(
                get_socket_option(
                    fd,
                    SOL_SOCKET,
                    SO_ACCEPTCONN,
                    (&mut acceptconn as *mut i32).cast(),
                    &mut acceptconn_len,
                ),
                -EINVAL
            );
            assert_eq!(acceptconn, -1);
            let mut peer = SockAddrIn::default();
            let mut peer_len = size_of::<SockAddrIn>() as SockLenT;
            let accepted = accept(fd, (&mut peer as *mut SockAddrIn).cast(), &mut peer_len);
            assert!(accepted >= 0);

            let mut set = FdSet::default();
            let ready = select(
                accepted + 1,
                &mut set,
                ptr::null_mut(),
                ptr::null_mut(),
                ptr::null_mut(),
            );
            assert_eq!(ready, 0);

            let mut nonblock = 1_u32;
            assert_eq!(
                ioctl(accepted, FIONBIO, (&mut nonblock as *mut u32).cast()),
                0
            );
            assert_eq!(close(accepted), 0);
            assert_eq!(close(fd), 0);
        }
    }

    #[test]
    fn socket_names_recvfrom_and_shutdown_are_host_compat() {
        let _guard = match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        unsafe {
            let fd = socket(AF_INET, SOCK_DGRAM, IPPROTO_UDP);
            assert!(fd >= 0);
            let addr = SockAddrIn {
                sin_len: size_of::<SockAddrIn>() as u8,
                sin_family: AF_INET as u8,
                sin_port: 9001,
                sin_addr: InAddr { s_addr: INADDR_ANY },
                sin_zero: [0; 8],
            };
            let mut addr_copy = addr;
            assert_eq!(
                bind(
                    fd,
                    (&mut addr_copy as *mut SockAddrIn).cast(),
                    size_of::<SockAddrIn>() as SockLenT,
                ),
                0
            );
            assert_eq!(
                connect(
                    fd,
                    (&mut addr_copy as *mut SockAddrIn).cast(),
                    size_of::<SockAddrIn>() as SockLenT,
                ),
                0
            );

            let mut local = SockAddrIn::default();
            let mut local_len = size_of::<SockAddrIn>() as SockLenT;
            assert_eq!(
                get_socket_name(fd, (&mut local as *mut SockAddrIn).cast(), &mut local_len),
                0
            );
            assert_eq!(local.sin_port, 9001);

            let mut peer = SockAddrIn::default();
            let mut peer_len = size_of::<SockAddrIn>() as SockLenT;
            assert_eq!(
                get_peer_name(fd, (&mut peer as *mut SockAddrIn).cast(), &mut peer_len),
                0
            );
            assert_eq!(peer.sin_port, 9001);

            let mut recv = [0xaa_u8; 6];
            let mut from = SockAddrIn::default();
            let mut from_len = size_of::<SockAddrIn>() as SockLenT;
            assert_eq!(
                receive_from(
                    fd,
                    recv.as_mut_ptr().cast(),
                    recv.len() as i32,
                    MSG_DONTWAIT,
                    (&mut from as *mut SockAddrIn).cast(),
                    &mut from_len,
                ),
                4
            );
            assert_eq!(&recv[..4], &[0, 0, 0, 0]);
            assert_eq!(from.sin_port, 0x2233);

            assert_eq!(shutdown(fd, 0), 0);
            peer_len = size_of::<SockAddrIn>() as SockLenT;
            assert_eq!(
                get_peer_name(fd, (&mut peer as *mut SockAddrIn).cast(), &mut peer_len),
                0
            );
            assert_eq!(peer.sin_port, 0);
            assert_eq!(close(fd), 0);
        }
    }

    #[test]
    fn invalid_inputs_and_boolean_sockopts_are_host_compat() {
        let _guard = match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        unsafe {
            assert_eq!(socket(AF_UNSPEC, SOCK_STREAM, IPPROTO_TCP), -1);
            let fd = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
            assert!(fd >= 0);

            let enabled = 1_i32;
            let mut value = 0_i32;
            let mut value_len = size_of::<i32>() as SockLenT;
            assert_eq!(
                set_socket_option(
                    fd,
                    SOL_SOCKET,
                    SO_KEEPALIVE,
                    (&enabled as *const i32).cast(),
                    size_of::<i32>() as SockLenT,
                ),
                0
            );
            assert_eq!(
                get_socket_option(
                    fd,
                    SOL_SOCKET,
                    SO_KEEPALIVE,
                    (&mut value as *mut i32).cast(),
                    &mut value_len,
                ),
                -EINVAL
            );
            assert_eq!(value, 0);

            assert_eq!(
                bind(fd, ptr::null_mut(), size_of::<SockAddrIn>() as SockLenT),
                -1
            );
            assert_eq!(
                connect(fd, ptr::null_mut(), size_of::<SockAddrIn>() as SockLenT),
                -1
            );
            assert_eq!(ioctl(fd, FIONREAD, ptr::null_mut()), -EINVAL);
            assert_eq!(close(fd), 0);
            assert_eq!(close(fd), -1);
        }
    }

    #[test]
    fn extended_socket_options_submit_and_getsockopt_stays_unimplemented() {
        let _guard = match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        unsafe {
            let fd = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
            assert!(fd >= 0);

            for opt in [
                SO_DEBUG,
                SO_DONTROUTE,
                SO_BROADCAST,
                SO_USELOOPBACK,
                SO_OOBINLINE,
                SO_REUSEPORT,
            ] {
                let enabled = 1_i32;
                assert_eq!(
                    set_socket_option(
                        fd,
                        SOL_SOCKET,
                        opt,
                        (&enabled as *const i32).cast(),
                        size_of::<i32>() as SockLenT,
                    ),
                    0
                );
            }

            let sndbuf = 8192_i32;
            assert_eq!(
                set_socket_option(
                    fd,
                    SOL_SOCKET,
                    SO_SNDBUF,
                    (&sndbuf as *const i32).cast(),
                    size_of::<i32>() as SockLenT,
                ),
                0
            );

            let sndlowat = 4_i32;
            let rcvlowat = 8_i32;
            assert_eq!(
                set_socket_option(
                    fd,
                    SOL_SOCKET,
                    SO_SNDLOWAT,
                    (&sndlowat as *const i32).cast(),
                    size_of::<i32>() as SockLenT,
                ),
                0
            );
            assert_eq!(
                set_socket_option(
                    fd,
                    SOL_SOCKET,
                    SO_RCVLOWAT,
                    (&rcvlowat as *const i32).cast(),
                    size_of::<i32>() as SockLenT,
                ),
                0
            );

            let linger = Linger {
                l_onoff: 1,
                l_linger: 7,
            };
            assert_eq!(
                set_socket_option(
                    fd,
                    SOL_SOCKET,
                    SO_LINGER,
                    (&linger as *const Linger).cast(),
                    size_of::<Linger>() as SockLenT,
                ),
                0
            );

            let timeout = TimeVal {
                tv_sec: 2,
                tv_usec: 25_000,
            };
            assert_eq!(
                set_socket_option(
                    fd,
                    SOL_SOCKET,
                    SO_RCVTIMEO,
                    (&timeout as *const TimeVal).cast(),
                    size_of::<TimeVal>() as SockLenT,
                ),
                0
            );

            let mut out = 0_i32;
            let mut out_len = size_of::<i32>() as SockLenT;
            assert_eq!(
                get_socket_option(
                    fd,
                    SOL_SOCKET,
                    SO_SNDBUF,
                    (&mut out as *mut i32).cast(),
                    &mut out_len,
                ),
                -EINVAL
            );
            assert_eq!(out, 0);
            assert_eq!(close(fd), 0);
        }
    }

    #[test]
    fn ip_and_tcp_socket_options_submit_while_getsockopt_is_unimplemented() {
        let _guard = match TEST_LOCK.lock() {
            Ok(guard) => guard,
            Err(poisoned) => poisoned.into_inner(),
        };
        unsafe {
            let fd = socket(AF_INET, SOCK_STREAM, IPPROTO_TCP);
            assert!(fd >= 0);

            let tos = IPTOS_LOWDELAY | IPTOS_RELIABILITY;
            let ttl = 64_i32;
            let nodelay = 1_i32;
            let keepalive = 30_i32;
            for (level, opt, value) in [
                (IPPROTO_IP, IP_TOS, tos),
                (IPPROTO_IP, IP_TTL, ttl),
                (IPPROTO_TCP, TCP_NODELAY, nodelay),
                (IPPROTO_TCP, TCP_KEEPALIVE, keepalive),
            ] {
                assert_eq!(
                    set_socket_option(
                        fd,
                        level,
                        opt,
                        (&value as *const i32).cast(),
                        size_of::<i32>() as SockLenT,
                    ),
                    0
                );
            }

            let mut out = 0_i32;
            let mut out_len = size_of::<i32>() as SockLenT;
            assert_eq!(
                get_socket_option(
                    fd,
                    IPPROTO_TCP,
                    TCP_NODELAY,
                    (&mut out as *mut i32).cast(),
                    &mut out_len,
                ),
                -EINVAL
            );
            assert_eq!(out, 0);
            assert_eq!(close(fd), 0);
        }
    }
}
