#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::ffi::{c_char, c_void};
use core::sync::atomic::{AtomicBool, Ordering};

pub use ogc_network_sys as sys;
pub use sys::{
    AF_INET, AF_UNSPEC, Bool, FD_SETSIZE, FIONBIO, FIONREAD, FdSet, HostEnt, INADDR_ANY,
    INADDR_BROADCAST, INVALID_SOCKET, IPPROTO_IP, IPPROTO_TCP, IPPROTO_UDP, IPTOS_LOWCOST,
    IPTOS_LOWDELAY, IPTOS_MINCOST, IPTOS_PREC_CRITIC_ECP, IPTOS_PREC_FLASH,
    IPTOS_PREC_FLASHOVERRIDE, IPTOS_PREC_IMMEDIATE, IPTOS_PREC_INTERNETCONTROL, IPTOS_PREC_MASK,
    IPTOS_PREC_NETCONTROL, IPTOS_PREC_PRIORITY, IPTOS_PREC_ROUTINE, IPTOS_RELIABILITY,
    IPTOS_THROUGHPUT, IPTOS_TOS_MASK, InAddr, MSG_DONTWAIT, Netcallback, O_NONBLOCK, PF_INET,
    PF_UNSPEC, PollSd, SO_ACCEPTCONN, SO_BROADCAST, SO_DEBUG, SO_DONTLINGER, SO_DONTROUTE,
    SO_ERROR, SO_KEEPALIVE, SO_LINGER, SO_OOBINLINE, SO_RCVBUF, SO_RCVLOWAT, SO_RCVTIMEO,
    SO_REUSEADDR, SO_REUSEPORT, SO_SNDBUF, SO_SNDLOWAT, SO_SNDTIMEO, SO_TYPE, SO_USELOOPBACK,
    SOCK_DGRAM, SOCK_RAW, SOCK_STREAM, SOCKET_ERROR, SOL_SOCKET, SockLenT, Sockaddr, SockaddrIn,
    TCP_KEEPALIVE, TCP_NODELAY, Timeval,
};

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

/// Builds a mutable descriptor slice, including the empty null-pointer case.
///
/// # Safety
///
/// When `nfds` is nonzero, `fds` must point to `nfds` writable descriptors.
unsafe fn poll_descriptors<'a>(fds: *mut PollFd, nfds: usize) -> &'a mut [PollFd] {
    // SAFETY: A zero-length slice still needs a non-null, aligned pointer.
    let descriptors = if nfds == 0 {
        core::ptr::NonNull::<PollFd>::dangling().as_ptr()
    } else {
        fds
    };
    // SAFETY: The caller of this helper guarantees storage for `nfds` values;
    // for zero values, `descriptors` is a valid aligned dangling pointer.
    unsafe { core::slice::from_raw_parts_mut(descriptors, nfds) }
}

pub fn init() -> i32 {
    let ret = sys::net_init();
    if ret >= 0 {
        let soc_ret = soc_init();
        if soc_ret < 0 {
            let cleanup_ret = sys::net_deinit();
            return if cleanup_ret < 0 {
                cleanup_ret
            } else {
                soc_ret
            };
        }
    }
    ret
}

/// Attach to the Wii IOS network service without starting NWC24 again.
/// Use this when an app was launched by an existing network client.
pub fn attach_active() -> i32 {
    let ret = sys::net_attach_active();
    if ret < 0 {
        return ret;
    }
    soc_init()
}

/// # Safety
///
/// `usrdata` must remain valid for the callback contract if `cb` is provided.
pub unsafe fn init_async(cb: Netcallback, usrdata: *mut c_void) -> i32 {
    unsafe { sys::net_init_async(cb, usrdata) }
}

pub fn get_status() -> i32 {
    sys::net_get_status()
}

pub fn wc24_cleanup() -> i32 {
    sys::net_wc24cleanup()
}

/// # Safety
///
/// `mac_buf` must point to a writable 6-byte buffer.
pub unsafe fn get_mac_address(mac_buf: *mut c_void) -> i32 {
    unsafe { sys::net_get_mac_address(mac_buf) }
}

pub fn deinit() -> i32 {
    let ret = soc_exit();
    if ret < 0 {
        return ret;
    }
    sys::net_deinit()
}

pub fn convert_error(ios_retval: i32) -> i32 {
    sys::_net_convert_error(ios_retval)
}

pub fn get_host_ip() -> u32 {
    sys::net_gethostip()
}

/// # Safety
///
/// `addr_string` must point to a valid, NUL-terminated C string for the duration
/// of the call.
pub unsafe fn get_host_by_name(addr_string: *const c_char) -> *mut HostEnt {
    unsafe { sys::net_gethostbyname(addr_string) }
}

/// # Safety
///
/// `cp` must point to a valid, NUL-terminated C string for the duration of the call.
pub unsafe fn inet_addr(cp: *const c_char) -> u32 {
    unsafe { sys::inet_addr(cp) }
}

pub use get_host_by_name as gethostbyname;

pub fn gai_strerror(code: i32) -> *const c_char {
    const UNKNOWN: &[u8] = b"unknown error\0";
    const MAPPINGS: &[(i32, &[u8])] = &[
        (-9, b"address family for hostname not supported\0"),
        (-3, b"temporary failure in name resolution\0"),
        (-1, b"invalid value for ai_flags\0"),
        (-4, b"non-recoverable failure in name resolution\0"),
        (-6, b"ai_family not supported\0"),
        (-10, b"memory allocation failure\0"),
        (-5, b"no address associated with hostname\0"),
        (-2, b"hostname nor servname provided, or not known\0"),
        (-8, b"servname not supported for ai_socktype\0"),
        (-7, b"ai_socktype not supported\0"),
        (-11, b"system error returned in errno\0"),
        (-12, b"invalid value for hints\0"),
        (-13, b"resolved protocol is unknown\0"),
        (-14, b"argument buffer overflow\0"),
    ];

    for (expected, msg) in MAPPINGS {
        if *expected == code {
            return msg.as_ptr().cast::<c_char>();
        }
    }

    UNKNOWN.as_ptr().cast()
}

pub fn ip4_addr(a: u8, b: u8, c: u8, d: u8) -> u32 {
    sys::ip4_addr(a, b, c, d)
}

pub fn ip4_addr1(addr: u32) -> u32 {
    sys::ip4_addr1(addr)
}

pub fn ip4_addr2(addr: u32) -> u32 {
    sys::ip4_addr2(addr)
}

pub fn ip4_addr3(addr: u32) -> u32 {
    sys::ip4_addr3(addr)
}

pub fn ip4_addr4(addr: u32) -> u32 {
    sys::ip4_addr4(addr)
}

pub fn iptost(tos: u32) -> u32 {
    sys::iptost(tos)
}

pub fn iptos_prec(tos: u32) -> u32 {
    sys::iptos_prec(tos)
}

pub fn fd_zero(set: &mut FdSet) {
    set.fd_bits.fill(0);
}

pub fn fd_set(set: &mut FdSet, socket: usize) {
    set.fd_bits[socket / 8] |= 1 << (socket & 7);
}

pub fn fd_clear(set: &mut FdSet, socket: usize) {
    set.fd_bits[socket / 8] &= !(1 << (socket & 7));
}

pub fn fd_is_set(set: &FdSet, socket: usize) -> bool {
    (set.fd_bits[socket / 8] & (1 << (socket & 7))) != 0
}

fn pollfd_events_to_pollsd(events: i16) -> u32 {
    (events as u16) as u32
}

#[cfg(any(target_arch = "powerpc", test))]
fn pollsd_events_to_pollfd(events: u32) -> i16 {
    events as i16
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
pub struct PollFd {
    pub fd: i32,
    pub events: i16,
    pub revents: i16,
}

mod socket_address_sealed {
    pub trait Sealed {}

    impl Sealed for super::Sockaddr {}
    impl Sealed for super::SockaddrIn {}
}

/// Socket-address layouts accepted by the Wii networking ABI.
///
/// This trait is sealed so downstream crates cannot claim an incompatible
/// layout. Both supported types are 16-byte `repr(C)` structures matching the
/// socket APIs exported by libogc.
pub trait SocketAddress: socket_address_sealed::Sealed + Copy {
    #[doc(hidden)]
    fn as_sockaddr_mut_ptr(&mut self) -> *mut Sockaddr;

    #[doc(hidden)]
    fn socket_address_len() -> SockLenT;
}

impl SocketAddress for Sockaddr {
    fn as_sockaddr_mut_ptr(&mut self) -> *mut Sockaddr {
        self
    }

    fn socket_address_len() -> SockLenT {
        core::mem::size_of::<Self>() as SockLenT
    }
}

impl SocketAddress for SockaddrIn {
    fn as_sockaddr_mut_ptr(&mut self) -> *mut Sockaddr {
        (self as *mut Self).cast()
    }

    fn socket_address_len() -> SockLenT {
        core::mem::size_of::<Self>() as SockLenT
    }
}

fn checked_socket_len(len: usize) -> Option<i32> {
    i32::try_from(len).ok()
}

const MAX_SOCKET_HANDLES: usize = FD_SETSIZE;

#[derive(Copy, Clone)]
struct SocketSlot {
    raw_socket: i32,
    in_use: bool,
}

impl SocketSlot {
    const fn empty() -> Self {
        Self {
            raw_socket: -1,
            in_use: false,
        }
    }
}

struct SpinLock(AtomicBool);

impl SpinLock {
    const fn new() -> Self {
        Self(AtomicBool::new(false))
    }

    fn lock(&self) -> SpinGuard<'_> {
        while self
            .0
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        SpinGuard { lock: self }
    }
}

struct SpinGuard<'a> {
    lock: &'a SpinLock,
}

impl Drop for SpinGuard<'_> {
    fn drop(&mut self) {
        self.lock.0.store(false, Ordering::Release);
    }
}

static SOC_LOCK: SpinLock = SpinLock::new();
static mut SOC_INITIALIZED: bool = false;
static mut SOCKET_TABLE: [SocketSlot; MAX_SOCKET_HANDLES] =
    [SocketSlot::empty(); MAX_SOCKET_HANDLES];

fn socket_table_ptr() -> *mut SocketSlot {
    core::ptr::addr_of_mut!(SOCKET_TABLE).cast()
}

fn socket_table_clear() {
    unsafe {
        for idx in 0..MAX_SOCKET_HANDLES {
            *socket_table_ptr().add(idx) = SocketSlot::empty();
        }
    }
}

fn socket_lookup(fd: i32) -> Option<i32> {
    if fd < 0 || fd as usize >= MAX_SOCKET_HANDLES {
        return None;
    }

    let slot = unsafe { *socket_table_ptr().add(fd as usize) };
    if slot.in_use {
        Some(slot.raw_socket)
    } else {
        None
    }
}

fn socket_raw(fd: i32) -> Option<i32> {
    let _guard = SOC_LOCK.lock();
    socket_lookup(fd)
}

fn soc_initialized() -> bool {
    let _guard = SOC_LOCK.lock();
    unsafe { SOC_INITIALIZED }
}

fn socket_alloc(raw_socket: i32) -> Option<i32> {
    if raw_socket < 0 || raw_socket as usize >= MAX_SOCKET_HANDLES {
        return None;
    }

    let _guard = SOC_LOCK.lock();
    unsafe {
        if !SOC_INITIALIZED {
            return None;
        }
        for idx in 0..MAX_SOCKET_HANDLES {
            let slot = socket_table_ptr().add(idx);
            if !(*slot).in_use {
                *slot = SocketSlot {
                    raw_socket,
                    in_use: true,
                };
                return Some(idx as i32);
            }
        }
    }
    None
}

fn socket_release(fd: i32) -> Option<i32> {
    if fd < 0 || fd as usize >= MAX_SOCKET_HANDLES {
        return None;
    }

    unsafe {
        let slot = socket_table_ptr().add(fd as usize);
        if !(*slot).in_use {
            return None;
        }
        let raw_socket = (*slot).raw_socket;
        *slot = SocketSlot::empty();
        Some(raw_socket)
    }
}

pub fn soc_init() -> i32 {
    let _guard = SOC_LOCK.lock();
    unsafe {
        if SOC_INITIALIZED {
            return 0;
        }
        SOC_INITIALIZED = true;
        socket_table_clear();
    }
    0
}

pub fn soc_exit() -> i32 {
    let _guard = SOC_LOCK.lock();
    unsafe {
        if !SOC_INITIALIZED {
            return 0;
        }

        let mut first_error = 0;
        for idx in 0..MAX_SOCKET_HANDLES {
            let slot = socket_table_ptr().add(idx);
            if (*slot).in_use {
                let close_ret = sys::net_close((*slot).raw_socket);
                if close_ret < 0 {
                    if first_error == 0 {
                        first_error = close_ret;
                    }
                } else {
                    *slot = SocketSlot::empty();
                }
            }
        }
        if first_error < 0 {
            return first_error;
        }
        SOC_INITIALIZED = false;
    }
    0
}

pub fn socket(domain: u32, type_: u32, protocol: u32) -> i32 {
    if !soc_initialized() {
        return SOCKET_ERROR;
    }

    let raw_socket = sys::net_socket(domain, type_, protocol);
    if raw_socket < 0 {
        return raw_socket;
    }

    let Some(fd) = socket_alloc(raw_socket) else {
        let close_ret = sys::net_close(raw_socket);
        return if close_ret < 0 {
            close_ret
        } else {
            SOCKET_ERROR
        };
    };

    fd
}

pub fn socket_raw_fd(fd: i32) -> Option<i32> {
    socket_raw(fd)
}

pub fn soc_get_fd(fd: i32) -> i32 {
    socket_raw(fd).unwrap_or(SOCKET_ERROR)
}

pub fn socket_release_fd(fd: i32) -> i32 {
    let _guard = SOC_LOCK.lock();
    let Some(raw_socket) = socket_lookup(fd) else {
        return SOCKET_ERROR;
    };
    let ret = sys::net_close(raw_socket);
    if ret < 0 {
        SOCKET_ERROR
    } else {
        let released = socket_release(fd);
        debug_assert_eq!(released, Some(raw_socket));
        0
    }
}

/// # Safety
///
/// `name` must point to a valid socket address buffer of length `namelen`.
pub unsafe fn bind(s: i32, name: *mut Sockaddr, namelen: SockLenT) -> i32 {
    let Some(raw_socket) = socket_raw(s) else {
        return SOCKET_ERROR;
    };
    bind_result(|| unsafe { sys::net_bind(raw_socket, name, namelen) })
}

fn bind_result(bind: impl FnOnce() -> i32) -> i32 {
    let result = bind();
    if result < 0 { result } else { 0 }
}

pub fn listen(s: i32, backlog: u32) -> i32 {
    let Some(raw_socket) = socket_raw(s) else {
        return SOCKET_ERROR;
    };
    let ret = sys::net_listen(raw_socket, backlog);
    if ret < 0 { ret } else { 0 }
}

/// # Safety
///
/// `addr` and `addrlen` must satisfy the underlying socket contract.
pub unsafe fn accept(s: i32, addr: *mut Sockaddr, addrlen: *mut SockLenT) -> i32 {
    unsafe {
        let Some(raw_socket) = socket_raw(s) else {
            return SOCKET_ERROR;
        };

        let accepted = sys::net_accept(raw_socket, addr, addrlen);
        if accepted < 0 {
            return accepted;
        }

        let Some(fd) = socket_alloc(accepted) else {
            let close_ret = sys::net_close(accepted);
            return if close_ret < 0 {
                close_ret
            } else {
                SOCKET_ERROR
            };
        };

        fd
    }
}

/// # Safety
///
/// `addr` must point to a valid socket address buffer of length `addrlen`.
pub unsafe fn connect(s: i32, addr: *mut Sockaddr, addrlen: SockLenT) -> i32 {
    let ret = unsafe { connect_backend(s, addr, addrlen) };
    if ret < 0 { SOCKET_ERROR } else { 0 }
}

/// # Safety
///
/// `addr` must point to a valid socket address buffer of length `addrlen`.
unsafe fn connect_backend(s: i32, addr: *mut Sockaddr, addrlen: SockLenT) -> i32 {
    unsafe {
        let Some(raw_socket) = socket_raw(s) else {
            return SOCKET_ERROR;
        };
        sys::net_connect(raw_socket, addr, addrlen)
    }
}

/// # Safety
///
/// `data` must point to a readable buffer of at least `size` bytes.
pub unsafe fn write(s: i32, data: *const c_void, size: i32) -> i32 {
    unsafe {
        let Some(raw_socket) = socket_raw(s) else {
            return SOCKET_ERROR;
        };
        sys::net_write(raw_socket, data, size)
    }
}

/// # Safety
///
/// `data` must point to a readable buffer of at least `size` bytes.
pub unsafe fn send(s: i32, data: *const c_void, size: i32, flags: u32) -> i32 {
    unsafe {
        let Some(raw_socket) = socket_raw(s) else {
            return SOCKET_ERROR;
        };
        let ret = sys::net_send(raw_socket, data, size, flags);
        if ret < 0 { SOCKET_ERROR } else { ret }
    }
}

/// # Safety
///
/// `data` must point to a readable buffer of at least `len` bytes, and `to`
/// must be a valid destination address buffer.
pub unsafe fn sendto(
    s: i32,
    data: *const c_void,
    len: i32,
    flags: u32,
    to: *mut Sockaddr,
    tolen: SockLenT,
) -> i32 {
    unsafe {
        let Some(raw_socket) = socket_raw(s) else {
            return SOCKET_ERROR;
        };
        let ret = sys::net_sendto(raw_socket, data, len, flags, to, tolen);
        if ret < 0 { SOCKET_ERROR } else { ret }
    }
}

/// # Safety
///
/// `mem` must point to a writable buffer of at least `len` bytes.
pub unsafe fn recv(s: i32, mem: *mut c_void, len: i32, flags: u32) -> i32 {
    unsafe {
        let Some(raw_socket) = socket_raw(s) else {
            return SOCKET_ERROR;
        };
        let ret = sys::net_recv(raw_socket, mem, len, flags);
        if ret < 0 { SOCKET_ERROR } else { ret }
    }
}

/// # Safety
///
/// `mem` must point to a writable buffer of at least `len` bytes, and `from`
/// / `fromlen` must satisfy the underlying socket contract.
pub unsafe fn recvfrom(
    s: i32,
    mem: *mut c_void,
    len: i32,
    flags: u32,
    from: *mut Sockaddr,
    fromlen: *mut SockLenT,
) -> i32 {
    unsafe {
        let Some(raw_socket) = socket_raw(s) else {
            return SOCKET_ERROR;
        };
        let ret = sys::net_recvfrom(raw_socket, mem, len, flags, from, fromlen);
        if ret < 0 { SOCKET_ERROR } else { ret }
    }
}

/// # Safety
///
/// `mem` must point to a writable buffer of at least `len` bytes.
pub unsafe fn read(s: i32, mem: *mut c_void, len: i32) -> i32 {
    unsafe {
        let Some(raw_socket) = socket_raw(s) else {
            return SOCKET_ERROR;
        };
        sys::net_read(raw_socket, mem, len)
    }
}

/// Binds a socket using a typed address with a verified Wii ABI layout.
pub fn bind_addr<A: SocketAddress>(s: i32, address: &A) -> i32 {
    let mut address = *address;
    // SAFETY: the sealed `SocketAddress` implementations provide valid,
    // correctly sized `repr(C)` address storage for the duration of the call.
    unsafe { bind(s, address.as_sockaddr_mut_ptr(), A::socket_address_len()) }
}

/// Accepts a connection without requesting the peer address.
pub fn accept_socket(s: i32) -> i32 {
    accept_socket_with(s, |socket, address, length| {
        // SAFETY: the helper supplies writable address and length storage.
        unsafe { accept(socket, address, length) }
    })
}

fn accept_socket_with(
    s: i32,
    accept: impl FnOnce(i32, *mut Sockaddr, *mut SockLenT) -> i32,
) -> i32 {
    // Wii IOS requires peer-address storage even when the caller does not need it.
    let mut peer = SockaddrIn::default();
    let mut length = core::mem::size_of::<SockaddrIn>() as SockLenT;
    accept(s, (&mut peer as *mut SockaddrIn).cast(), &mut length)
}

/// Accepts a connection and writes the peer address into typed storage.
pub fn accept_addr<A: SocketAddress>(s: i32, address: &mut A) -> i32 {
    let mut address_len = A::socket_address_len();
    // SAFETY: `address` is valid writable storage of exactly the advertised
    // sealed ABI layout, and `address_len` remains valid for the call.
    unsafe { accept(s, address.as_sockaddr_mut_ptr(), &mut address_len) }
}

/// Connects a socket using a typed address with a verified Wii ABI layout.
/// Returns the backend's negative error code on failure.
pub fn connect_addr<A: SocketAddress>(s: i32, address: &A) -> i32 {
    let mut address = *address;
    // SAFETY: the local copy is valid, correctly sized address storage for the
    // entire synchronous call.
    unsafe { connect_backend(s, address.as_sockaddr_mut_ptr(), A::socket_address_len()) }
}

/// Writes all or part of `data` using the socket write operation.
pub fn write_slice(s: i32, data: &[u8]) -> i32 {
    let Some(len) = checked_socket_len(data.len()) else {
        return SOCKET_ERROR;
    };
    // SAFETY: `data` is readable for `len` bytes and remains borrowed for the
    // complete synchronous operation.
    unsafe { write(s, data.as_ptr().cast(), len) }
}

/// Sends all or part of `data` using the supplied socket flags.
pub fn send_slice(s: i32, data: &[u8], flags: u32) -> i32 {
    let Some(len) = checked_socket_len(data.len()) else {
        return SOCKET_ERROR;
    };
    // SAFETY: `data` is readable for `len` bytes and remains borrowed for the
    // complete synchronous operation.
    unsafe { send(s, data.as_ptr().cast(), len, flags) }
}

/// Sends `data` to a typed destination address.
pub fn send_to_slice<A: SocketAddress>(s: i32, data: &[u8], flags: u32, destination: &A) -> i32 {
    let Some(len) = checked_socket_len(data.len()) else {
        return SOCKET_ERROR;
    };
    let mut destination = *destination;
    // SAFETY: `data` is readable for `len` bytes, and the sealed address copy
    // has the correct ABI layout and remains valid for the synchronous call.
    unsafe {
        sendto(
            s,
            data.as_ptr().cast(),
            len,
            flags,
            destination.as_sockaddr_mut_ptr(),
            A::socket_address_len(),
        )
    }
}

/// Receives bytes into `buffer` using the supplied socket flags.
/// Retains the backend's negative error code, including a nonblocking retry.
pub fn recv_slice(s: i32, buffer: &mut [u8], flags: u32) -> i32 {
    let Some(len) = checked_socket_len(buffer.len()) else {
        return SOCKET_ERROR;
    };
    // SAFETY: `buffer` is writable for `len` bytes and is uniquely borrowed for
    // the complete synchronous operation.
    let Some(raw_socket) = socket_raw(s) else {
        return SOCKET_ERROR;
    };
    unsafe { sys::net_recv(raw_socket, buffer.as_mut_ptr().cast(), len, flags) }
}

/// Receives bytes and writes the source address into typed storage.
pub fn recv_from_slice<A: SocketAddress>(
    s: i32,
    buffer: &mut [u8],
    flags: u32,
    source: &mut A,
) -> i32 {
    let Some(len) = checked_socket_len(buffer.len()) else {
        return SOCKET_ERROR;
    };
    let mut source_len = A::socket_address_len();
    // SAFETY: the byte slice is valid writable storage, while `source` and its
    // length are valid writable values for the sealed address ABI.
    unsafe {
        recvfrom(
            s,
            buffer.as_mut_ptr().cast(),
            len,
            flags,
            source.as_sockaddr_mut_ptr(),
            &mut source_len,
        )
    }
}

/// Reads bytes into `buffer` using the socket read operation.
pub fn read_slice(s: i32, buffer: &mut [u8]) -> i32 {
    let Some(len) = checked_socket_len(buffer.len()) else {
        return SOCKET_ERROR;
    };
    // SAFETY: `buffer` is writable for `len` bytes and is uniquely borrowed for
    // the complete synchronous operation.
    unsafe { read(s, buffer.as_mut_ptr().cast(), len) }
}

/// Retrieves the local socket address into typed storage.
pub fn getsockname_addr<A: SocketAddress>(s: i32, address: &mut A) -> i32 {
    let mut address_len = A::socket_address_len();
    // SAFETY: `address` is valid writable storage of the sealed ABI layout, and
    // `address_len` remains valid for the entire synchronous call.
    unsafe { getsockname(s, address.as_sockaddr_mut_ptr(), &mut address_len) }
}

/// Polls the supplied public socket descriptors.
pub fn poll_slice(fds: &mut [PollFd], timeout: i32) -> i32 {
    let Some(nfds) = checked_socket_len(fds.len()) else {
        return SOCKET_ERROR;
    };
    // SAFETY: `fds` is a uniquely borrowed contiguous array of `nfds`
    // descriptors for the entire synchronous operation.
    unsafe { poll(fds.as_mut_ptr(), nfds, timeout) }
}

/// Polls backend socket descriptors without exposing pointer arithmetic.
pub fn poll_raw_slice(fds: &mut [PollSd], timeout: i32) -> i32 {
    let Some(nfds) = checked_socket_len(fds.len()) else {
        return SOCKET_ERROR;
    };
    // SAFETY: `fds` is a uniquely borrowed contiguous array of `nfds`
    // descriptors for the entire synchronous operation.
    unsafe { poll_raw(fds.as_mut_ptr(), nfds, timeout) }
}

pub fn close(s: i32) -> i32 {
    let _guard = SOC_LOCK.lock();
    let Some(raw_socket) = socket_lookup(s) else {
        return SOCKET_ERROR;
    };
    let ret = sys::net_close(raw_socket);
    if ret < 0 {
        SOCKET_ERROR
    } else {
        let released = socket_release(s);
        debug_assert_eq!(released, Some(raw_socket));
        0
    }
}

/// # Safety
///
/// Pointer arguments must satisfy the underlying `select` contract.
pub unsafe fn select(
    maxfdp1: i32,
    readset: *mut FdSet,
    writeset: *mut FdSet,
    exceptset: *mut FdSet,
    timeout: *mut Timeval,
) -> i32 {
    if !soc_initialized() {
        return SOCKET_ERROR;
    }

    let maxfdp1 = maxfdp1.min(MAX_SOCKET_HANDLES as i32).max(0);
    let mut raw_read = FdSet::default();
    let mut raw_write = FdSet::default();
    let mut raw_except = FdSet::default();
    let mut raw_maxfd = 0;

    unsafe {
        if !readset.is_null() {
            for fd in 0..maxfdp1 as usize {
                if fd_is_set(&*readset, fd)
                    && let Some(raw) = socket_raw(fd as i32)
                {
                    fd_set(&mut raw_read, raw as usize);
                    raw_maxfd = raw_maxfd.max(raw + 1);
                }
            }
        }

        if !writeset.is_null() {
            for fd in 0..maxfdp1 as usize {
                if fd_is_set(&*writeset, fd)
                    && let Some(raw) = socket_raw(fd as i32)
                {
                    fd_set(&mut raw_write, raw as usize);
                    raw_maxfd = raw_maxfd.max(raw + 1);
                }
            }
        }

        if !exceptset.is_null() {
            for fd in 0..maxfdp1 as usize {
                if fd_is_set(&*exceptset, fd)
                    && let Some(raw) = socket_raw(fd as i32)
                {
                    fd_set(&mut raw_except, raw as usize);
                    raw_maxfd = raw_maxfd.max(raw + 1);
                }
            }
        }
    }

    let ret = unsafe {
        sys::net_select(
            raw_maxfd,
            if readset.is_null() {
                core::ptr::null_mut()
            } else {
                &mut raw_read
            },
            if writeset.is_null() {
                core::ptr::null_mut()
            } else {
                &mut raw_write
            },
            if exceptset.is_null() {
                core::ptr::null_mut()
            } else {
                &mut raw_except
            },
            timeout,
        )
    };
    if ret < 0 {
        return SOCKET_ERROR;
    }

    unsafe {
        if !readset.is_null() {
            fd_zero(&mut *readset);
            for fd in 0..maxfdp1 as usize {
                if let Some(raw) = socket_raw(fd as i32)
                    && fd_is_set(&raw_read, raw as usize)
                {
                    fd_set(&mut *readset, fd);
                }
            }
        }
        if !writeset.is_null() {
            fd_zero(&mut *writeset);
            for fd in 0..maxfdp1 as usize {
                if let Some(raw) = socket_raw(fd as i32)
                    && fd_is_set(&raw_write, raw as usize)
                {
                    fd_set(&mut *writeset, fd);
                }
            }
        }
        if !exceptset.is_null() {
            fd_zero(&mut *exceptset);
            for fd in 0..maxfdp1 as usize {
                if let Some(raw) = socket_raw(fd as i32)
                    && fd_is_set(&raw_except, raw as usize)
                {
                    fd_set(&mut *exceptset, fd);
                }
            }
        }
    }

    ret
}

/// # Safety
///
/// `optval` and `optlen` must satisfy the underlying `getsockopt` contract.
pub unsafe fn getsockopt(
    s: i32,
    level: u32,
    optname: u32,
    optval: *const c_void,
    optlen: *mut SockLenT,
) -> i32 {
    unsafe {
        let Some(raw_socket) = socket_raw(s) else {
            return SOCKET_ERROR;
        };
        let ret = sys::net_getsockopt(raw_socket, level, optname, optval, optlen);
        if ret < 0 { SOCKET_ERROR } else { 0 }
    }
}

/// # Safety
///
/// `optval` must point to a readable buffer of `optlen` bytes.
pub unsafe fn setsockopt(
    s: i32,
    level: u32,
    optname: u32,
    optval: *const c_void,
    optlen: SockLenT,
) -> i32 {
    unsafe {
        let Some(raw_socket) = socket_raw(s) else {
            return SOCKET_ERROR;
        };
        let ret = sys::net_setsockopt(raw_socket, level, optname, optval, optlen);
        if ret < 0 { SOCKET_ERROR } else { 0 }
    }
}

/// # Safety
///
/// `addr` and `addrlen` must satisfy the underlying `getsockname` contract.
pub unsafe fn getsockname(s: i32, addr: *mut Sockaddr, addrlen: *mut SockLenT) -> i32 {
    unsafe {
        let Some(raw_socket) = socket_raw(s) else {
            return SOCKET_ERROR;
        };
        let ret = sys::net_getsockname(raw_socket, addr, addrlen);
        if ret < 0 { SOCKET_ERROR } else { 0 }
    }
}

/// # Safety
///
/// `argp` must satisfy the underlying ioctl command contract.
pub unsafe fn ioctl(s: i32, cmd: u32, argp: *mut c_void) -> i32 {
    unsafe {
        let Some(raw_socket) = socket_raw(s) else {
            return SOCKET_ERROR;
        };
        let ret = sys::net_ioctl(raw_socket, cmd, argp);
        if ret < 0 { SOCKET_ERROR } else { ret }
    }
}

pub fn fcntl(s: i32, cmd: u32, flags: u32) -> i32 {
    let Some(raw_socket) = socket_raw(s) else {
        return SOCKET_ERROR;
    };
    sys::net_fcntl(raw_socket, cmd, flags)
}

/// # Safety
///
/// `fds` must point to an array of `nfds` poll descriptors.
pub unsafe fn poll(fds: *mut PollFd, nfds: i32, _timeout: i32) -> i32 {
    unsafe {
        if nfds < 0 || (nfds > 0 && fds.is_null()) {
            return SOCKET_ERROR;
        }

        if !soc_initialized() {
            return SOCKET_ERROR;
        }

        let nfds = nfds as usize;
        let backend_nfds = nfds.min(MAX_SOCKET_HANDLES);

        let mut backend = [PollSd {
            socket: -1,
            events: 0,
            revents: 0,
        }; MAX_SOCKET_HANDLES];

        // SAFETY: `poll`'s caller contract guarantees the pointer has `nfds`
        // writable descriptors; null is accepted for the zero-descriptor case.
        let slice = { poll_descriptors(fds, nfds) };
        for entry in slice.iter_mut().skip(backend_nfds) {
            if entry.fd >= 0 {
                return SOCKET_ERROR;
            }
            entry.revents = 0;
        }

        for (idx, entry) in slice.iter_mut().take(backend_nfds).enumerate() {
            entry.revents = 0;
            if entry.fd < 0 {
                continue;
            }

            let Some(raw_socket) = socket_raw(entry.fd) else {
                return SOCKET_ERROR;
            };

            backend[idx].socket = raw_socket;
            backend[idx].events = pollfd_events_to_pollsd(entry.events);
        }

        #[cfg(target_arch = "powerpc")]
        {
            let ret = sys::net_poll(backend.as_mut_ptr(), backend_nfds as i32, _timeout);
            if ret < 0 {
                return SOCKET_ERROR;
            }

            for (idx, entry) in slice.iter_mut().take(backend_nfds).enumerate() {
                entry.revents = 0;
                if entry.fd < 0 {
                    continue;
                }

                entry.revents = pollsd_events_to_pollfd(backend[idx].revents);
            }

            ret
        }

        #[cfg(not(target_arch = "powerpc"))]
        {
            for entry in slice.iter_mut().take(backend_nfds) {
                entry.revents = 0;
            }
            0
        }
    }
}

/// # Safety
///
/// `sds` must point to an array of `nsds` backend poll descriptors.
pub unsafe fn poll_raw(sds: *mut PollSd, nsds: i32, timeout: i32) -> i32 {
    unsafe { sys::net_poll(sds, nsds, timeout) }
}

pub fn shutdown(s: i32, how: u32) -> i32 {
    let Some(raw_socket) = socket_raw(s) else {
        return SOCKET_ERROR;
    };
    let ret = sys::net_shutdown(raw_socket, how);
    if ret < 0 { SOCKET_ERROR } else { ret }
}

/// # Safety
///
/// The string pointers must reference writable buffers accepted by libogc.
pub unsafe fn if_config(
    local_ip: *mut c_char,
    netmask: *mut c_char,
    gateway: *mut c_char,
    use_dhcp: bool,
    max_retries: i32,
) -> i32 {
    let mut host_ip = InAddr::default();
    let mut host_mask = InAddr::default();
    let mut host_gateway = InAddr::default();
    let gateway_out = if gateway.is_null() {
        core::ptr::null_mut()
    } else {
        &mut host_gateway as *mut InAddr
    };

    let ret = unsafe {
        if_configex(
            &mut host_ip,
            &mut host_mask,
            gateway_out,
            use_dhcp,
            max_retries,
        )
    };
    if ret < 0 {
        return ret;
    }

    if !local_ip.is_null() && host_ip.s_addr != 0 {
        write_in_addr_string(local_ip, host_ip.s_addr);
    }
    if !netmask.is_null() && host_mask.s_addr != 0 {
        write_in_addr_string(netmask, host_mask.s_addr);
    }
    if !gateway.is_null() && host_gateway.s_addr != 0 {
        write_in_addr_string(gateway, host_gateway.s_addr);
    }

    ret
}

/// # Safety
///
/// The address pointers must reference writable buffers accepted by libogc.
pub unsafe fn if_configex(
    local_ip: *mut InAddr,
    netmask: *mut InAddr,
    gateway: *mut InAddr,
    use_dhcp: bool,
    max_retries: i32,
) -> i32 {
    unsafe { sys::if_configex(local_ip, netmask, gateway, use_dhcp, max_retries) }
}

fn write_in_addr_string(dst: *mut c_char, addr: u32) {
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
        idx += write_decimal_octet(&mut out[idx..], *octet as u8);
    }
    out[idx] = 0;

    for (offset, byte) in out[..=idx].iter().copied().enumerate() {
        unsafe {
            *dst.add(offset) = byte as c_char;
        }
    }
}

fn write_decimal_octet(dst: &mut [u8], value: u8) -> usize {
    if value >= 100 {
        dst[0] = b'0' + value / 100;
        dst[1] = b'0' + (value / 10) % 10;
        dst[2] = b'0' + value % 10;
        3
    } else if value >= 10 {
        dst[0] = b'0' + value / 10;
        dst[1] = b'0' + value % 10;
        2
    } else {
        dst[0] = b'0' + value;
        1
    }
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicBool, Ordering};

    static TEST_LOCK: AtomicBool = AtomicBool::new(false);

    struct TestGuard;

    impl Drop for TestGuard {
        fn drop(&mut self) {
            TEST_LOCK.store(false, Ordering::Release);
        }
    }

    fn test_guard() -> TestGuard {
        while TEST_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }

        TestGuard
    }

    #[test]
    fn socket_length_type_matches_unsigned_c_api() {
        let max: SockLenT = u32::MAX;
        assert_eq!(max, u32::MAX);
        let _: unsafe fn(i32, *mut Sockaddr, SockLenT) -> i32 = bind;
        let _: unsafe fn(i32, *mut Sockaddr, *mut SockLenT) -> i32 = accept;
    }

    #[test]
    fn bind_preserves_backend_error_result() {
        let _guard = test_guard();
        assert_eq!(bind_result(|| -10), -10);
        assert_eq!(bind_result(|| 0), 0);
    }

    #[test]
    fn accept_without_peer_supplies_wii_address_storage() {
        let result = accept_socket_with(7, |socket, address, length| {
            assert_eq!(socket, 7);
            assert!(!address.is_null());
            assert!(!length.is_null());
            // SAFETY: the production helper owns both output buffers.
            unsafe {
                assert_eq!(*length, 16);
                address.cast::<SockaddrIn>().write(SockaddrIn::default());
                *length = 8;
            }
            23
        });
        assert_eq!(result, 23);
        assert_eq!(accept_socket_with(7, |_, _, _| -12), -12);
    }

    #[test]
    fn public_layout_matches_c() {
        assert_eq!(core::mem::size_of::<InAddr>(), 4);
        assert_eq!(core::mem::size_of::<SockaddrIn>(), 16);
        assert_eq!(core::mem::size_of::<Sockaddr>(), 16);
        assert_eq!(core::mem::size_of::<FdSet>(), 8);
        assert_eq!(core::mem::size_of::<Timeval>(), 8);
        assert_eq!(core::mem::size_of::<PollFd>(), 8);
        assert_eq!(core::mem::align_of::<PollFd>(), 4);
        assert_eq!(core::mem::size_of::<PollSd>(), 12);
        assert_eq!(core::mem::align_of::<PollSd>(), 4);
    }

    #[test]
    fn empty_poll_slice_accepts_null_pointer() {
        // SAFETY: A zero-length poll slice does not read or write through `fds`.
        let descriptors = unsafe { poll_descriptors(core::ptr::null_mut(), 0) };
        assert!(descriptors.is_empty());
    }

    #[test]
    fn helper_functions_exist_without_backend() {
        let _ = init as fn() -> i32;
        let _ = init_async as unsafe fn(Netcallback, *mut c_void) -> i32;
        let _ = get_status as fn() -> i32;
        let _ = wc24_cleanup as fn() -> i32;
        let _ = get_mac_address as unsafe fn(*mut c_void) -> i32;
        let _ = deinit as fn() -> i32;
        let _ = convert_error as fn(i32) -> i32;
        let _ = get_host_ip as fn() -> u32;
        let _ = get_host_by_name as unsafe fn(*const c_char) -> *mut HostEnt;
        let _ = inet_addr as unsafe fn(*const c_char) -> u32;
        let _ = fd_zero as fn(&mut FdSet);
        let _ = fd_set as fn(&mut FdSet, usize);
        let _ = fd_clear as fn(&mut FdSet, usize);
        let _ = fd_is_set as fn(&FdSet, usize) -> bool;
        let _ = socket as fn(u32, u32, u32) -> i32;
        let _ = bind as unsafe fn(i32, *mut Sockaddr, SockLenT) -> i32;
        let _ = accept as unsafe fn(i32, *mut Sockaddr, *mut SockLenT) -> i32;
        let _ = connect as unsafe fn(i32, *mut Sockaddr, SockLenT) -> i32;
        let _ = write as unsafe fn(i32, *const c_void, i32) -> i32;
        let _ = send as unsafe fn(i32, *const c_void, i32, u32) -> i32;
        let _ = sendto as unsafe fn(i32, *const c_void, i32, u32, *mut Sockaddr, SockLenT) -> i32;
        let _ = recv as unsafe fn(i32, *mut c_void, i32, u32) -> i32;
        let _ =
            recvfrom as unsafe fn(i32, *mut c_void, i32, u32, *mut Sockaddr, *mut SockLenT) -> i32;
        let _ = read as unsafe fn(i32, *mut c_void, i32) -> i32;
        let _ = bind_addr::<Sockaddr> as fn(i32, &Sockaddr) -> i32;
        let _ = accept_socket as fn(i32) -> i32;
        let _ = accept_addr::<Sockaddr> as fn(i32, &mut Sockaddr) -> i32;
        let _ = connect_addr::<SockaddrIn> as fn(i32, &SockaddrIn) -> i32;
        let _ = write_slice as fn(i32, &[u8]) -> i32;
        let _ = send_slice as fn(i32, &[u8], u32) -> i32;
        let _ = send_to_slice::<SockaddrIn> as fn(i32, &[u8], u32, &SockaddrIn) -> i32;
        let _ = recv_slice as fn(i32, &mut [u8], u32) -> i32;
        let _ = recv_from_slice::<SockaddrIn> as fn(i32, &mut [u8], u32, &mut SockaddrIn) -> i32;
        let _ = read_slice as fn(i32, &mut [u8]) -> i32;
        let _ = getsockname_addr::<SockaddrIn> as fn(i32, &mut SockaddrIn) -> i32;
        let _ = poll_slice as fn(&mut [PollFd], i32) -> i32;
        let _ = poll_raw_slice as fn(&mut [PollSd], i32) -> i32;
        let _ = select as unsafe fn(i32, *mut FdSet, *mut FdSet, *mut FdSet, *mut Timeval) -> i32;
        let _ = getsockopt as unsafe fn(i32, u32, u32, *const c_void, *mut SockLenT) -> i32;
        let _ = setsockopt as unsafe fn(i32, u32, u32, *const c_void, SockLenT) -> i32;
        let _ = getsockname as unsafe fn(i32, *mut Sockaddr, *mut SockLenT) -> i32;
        let _ = ioctl as unsafe fn(i32, u32, *mut c_void) -> i32;
        let _ = poll as unsafe fn(*mut PollFd, i32, i32) -> i32;
        let _ = poll_raw as unsafe fn(*mut PollSd, i32, i32) -> i32;
        let _ = if_config as unsafe fn(*mut c_char, *mut c_char, *mut c_char, bool, i32) -> i32;
        let _ = if_configex as unsafe fn(*mut InAddr, *mut InAddr, *mut InAddr, bool, i32) -> i32;
    }

    #[test]
    fn poll_event_translation_matches_public_flags() {
        assert_eq!(pollfd_events_to_pollsd(POLLRDNORM), POLLRDNORM as u32);
        assert_eq!(pollfd_events_to_pollsd(POLLWRNORM), POLLWRNORM as u32);
        assert_eq!(pollfd_events_to_pollsd(POLLIN), POLLIN as u32);
        assert_eq!(pollfd_events_to_pollsd(POLLOUT), POLLOUT as u32);
        assert_eq!(
            pollfd_events_to_pollsd(POLLIN | POLLOUT | POLLERR | POLLHUP | POLLNVAL),
            (POLLIN | POLLOUT | POLLERR | POLLHUP | POLLNVAL) as u32
        );

        assert_eq!(pollsd_events_to_pollfd(POLLRDNORM as u32), POLLRDNORM);
        assert_eq!(pollsd_events_to_pollfd(POLLWRNORM as u32), POLLWRNORM);
        assert_eq!(pollsd_events_to_pollfd(POLLIN as u32), POLLIN);
        assert_eq!(pollsd_events_to_pollfd(POLLOUT as u32), POLLOUT);
        assert_eq!(
            pollsd_events_to_pollfd((POLLIN | POLLOUT | POLLERR | POLLHUP | POLLNVAL) as u32),
            POLLIN | POLLOUT | POLLERR | POLLHUP | POLLNVAL
        );
    }

    #[test]
    fn fd_bit_helpers_round_trip() {
        let mut set = FdSet { fd_bits: [0; 2] };

        fd_set(&mut set, 3);
        fd_set(&mut set, 17);
        assert!(fd_is_set(&set, 3));
        assert!(fd_is_set(&set, 17));
        assert!(!fd_is_set(&set, 4));

        fd_clear(&mut set, 3);
        assert!(!fd_is_set(&set, 3));
        assert!(fd_is_set(&set, 17));

        fd_zero(&mut set);
        assert_eq!(set.fd_bits, [0; 2]);
    }

    #[test]
    fn gai_strerror_matches_libogc_messages() {
        let cases = [
            (-9, "address family for hostname not supported"),
            (-3, "temporary failure in name resolution"),
            (-1, "invalid value for ai_flags"),
            (-4, "non-recoverable failure in name resolution"),
            (-6, "ai_family not supported"),
            (-10, "memory allocation failure"),
            (-5, "no address associated with hostname"),
            (-2, "hostname nor servname provided, or not known"),
            (-8, "servname not supported for ai_socktype"),
            (-7, "ai_socktype not supported"),
            (-11, "system error returned in errno"),
            (-12, "invalid value for hints"),
            (-13, "resolved protocol is unknown"),
            (-14, "argument buffer overflow"),
            (1234, "unknown error"),
        ];

        for (code, expected) in cases {
            let ptr = gai_strerror(code);
            let got = unsafe { core::ffi::CStr::from_ptr(ptr) };
            assert_eq!(got.to_str().unwrap_or(""), expected);
        }
    }

    #[test]
    fn write_in_addr_string_formats_dotted_quads() {
        let mut buf = [0 as c_char; 16];
        write_in_addr_string(buf.as_mut_ptr(), ip4_addr(192, 168, 1, 7));
        let got = unsafe { core::ffi::CStr::from_ptr(buf.as_ptr()) };
        assert_eq!(got.to_str().unwrap_or(""), "192.168.1.7");
    }

    #[test]
    fn poll_tolerates_extra_negative_descriptors() {
        let _guard = test_guard();
        let _ = soc_exit();
        assert_eq!(soc_init(), 0);

        let mut fds = [PollFd {
            fd: -1,
            events: POLLIN,
            revents: 123,
        }; FD_SETSIZE + 1];

        let ret = poll_slice(&mut fds, 0);
        assert_eq!(ret, 0);
        assert!(fds.iter().all(|fd| fd.revents == 0));

        assert_eq!(soc_exit(), 0);
    }

    #[test]
    fn select_ignores_closed_descriptors() {
        let _guard = test_guard();
        let _ = soc_exit();
        assert_eq!(soc_init(), 0);

        let fd = socket(AF_INET, SOCK_DGRAM, 0);
        assert!(fd >= 0);
        assert_eq!(close(fd), 0);

        let mut readset = FdSet { fd_bits: [0; 2] };
        fd_set(&mut readset, fd as usize);

        let mut timeout = Timeval {
            tv_sec: 0,
            tv_usec: 0,
        };
        let ret = unsafe {
            select(
                fd + 1,
                &mut readset,
                core::ptr::null_mut(),
                core::ptr::null_mut(),
                &mut timeout,
            )
        };
        assert_eq!(ret, 0);
        assert_eq!(readset.fd_bits, [0; 2]);

        assert_eq!(soc_exit(), 0);
    }
}
