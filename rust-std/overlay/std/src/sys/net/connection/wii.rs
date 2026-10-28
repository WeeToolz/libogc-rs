use crate::fmt;
use crate::io::{self, BorrowedCursor, IoSlice, IoSliceMut};
use crate::net::{Ipv4Addr, Ipv6Addr, Shutdown, SocketAddr, SocketAddrV4, ToSocketAddrs};
use crate::sys::wii::NetRequest;
use crate::time::Duration;
#[path = "wii_error.rs"]
mod wii_error;
use wii_error::NetError;

#[derive(Debug)]
struct Socket {
    handle: *mut core::ffi::c_void,
}
// SAFETY: Each handle owns one application-provider Arc. The provider protects
// settings and the production IPC transport serializes hardware operations.
unsafe impl Send for Socket {}
unsafe impl Sync for Socket {}

#[derive(Debug)]
struct SocketError(NetError);
impl fmt::Display for SocketError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Wii network error in domain {}: {} (newlib {})",
            self.0.domain,
            self.0.code,
            self.0.errno()
        )
    }
}
impl crate::error::Error for SocketError {}

fn call(request: &mut NetRequest) -> io::Result<i64> {
    let result = unsafe { crate::sys::wii::net(request) };
    if result < 0 {
        let error = NetError::decode(result);
        let kind = io::Error::from_raw_os_error(error.errno()).kind();
        Err(io::Error::new(kind, SocketError(error)))
    } else {
        Ok(result)
    }
}
fn unsupported<T>() -> io::Result<T> {
    Err(io::Error::UNSUPPORTED_PLATFORM)
}
fn ipv4(address: &SocketAddr) -> io::Result<(u32, u16)> {
    match address {
        SocketAddr::V4(a) => Ok((u32::from_be_bytes(a.ip().octets()), a.port())),
        SocketAddr::V6(_) => unsupported(),
    }
}
fn address(ip: u32, port: u16) -> SocketAddr {
    SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::from(ip), port))
}
fn set_ipv4_ttl(socket: &Socket, value: u32) -> io::Result<()> {
    if value > 255 {
        return Err(io::const_error!(
            io::ErrorKind::InvalidInput,
            "IPv4 TTL exceeds 255"
        ));
    }
    socket.set_option(0, 4, value)
}

impl Socket {
    fn request(&self, operation: u32) -> NetRequest {
        NetRequest::new(operation, self.handle)
    }
    fn from_reply(result: i64) -> io::Result<Self> {
        let handle = usize::try_from(result).map_err(|_| io::Error::UNSUPPORTED_PLATFORM)?
            as *mut core::ffi::c_void;
        if handle.is_null() {
            return Err(io::const_error!(
                io::ErrorKind::InvalidData,
                "null Wii socket handle"
            ));
        }
        Ok(Self { handle })
    }
    fn open(operation: u32, address: &SocketAddr, datagram: bool) -> io::Result<Self> {
        let (ip, port) = ipv4(address)?;
        let mut request = NetRequest::new(operation, crate::ptr::null_mut());
        request.ip = ip;
        request.port = port;
        request.flags = datagram.into();
        Self::from_reply(call(&mut request)?)
    }
    fn unit(&self, operation: u32, flags: u16) -> io::Result<()> {
        let mut request = self.request(operation);
        request.flags = flags;
        call(&mut request).map(|_| ())
    }
    fn duplicate(&self) -> io::Result<Self> {
        Self::from_reply(call(&mut self.request(3))?)
    }
    fn socket_addr(&self, peer: bool) -> io::Result<SocketAddr> {
        let mut request = self.request(6);
        request.flags = peer.into();
        call(&mut request)?;
        Ok(address(request.ip, request.port))
    }
    fn set_timeout(&self, write: bool, value: Option<Duration>) -> io::Result<()> {
        let mut request = self.request(10);
        request.flags = u16::from(write);
        if let Some(value) = value {
            request.flags |= 2;
            request.seconds = value.as_secs();
            request.nanos = value.subsec_nanos();
        }
        call(&mut request).map(|_| ())
    }
    fn timeout(&self, write: bool) -> io::Result<Option<Duration>> {
        let mut request = self.request(11);
        request.flags = write.into();
        if call(&mut request)? == 0 {
            Ok(None)
        } else {
            Ok(Some(Duration::new(request.seconds, request.nanos)))
        }
    }
    fn receive(
        &self,
        buffer: &mut [u8],
        peek: bool,
        from: bool,
    ) -> io::Result<(usize, SocketAddr)> {
        let mut request = self.request(7);
        request.data = buffer.as_mut_ptr();
        request.length = buffer.len();
        request.flags = u16::from(peek) | (u16::from(from) << 1);
        let count = call(&mut request)? as usize;
        Ok((count, address(request.ip, request.port)))
    }
    fn send(&self, buffer: &[u8], destination: Option<&SocketAddr>) -> io::Result<usize> {
        let mut request = self.request(8);
        request.data = buffer.as_ptr().cast_mut();
        request.length = buffer.len();
        if let Some(destination) = destination {
            let (ip, port) = ipv4(destination)?;
            request.flags = 1;
            request.ip = ip;
            request.port = port;
        }
        Ok(call(&mut request)? as usize)
    }
    fn bool_option(&self, level: u32, name: u16) -> io::Result<bool> {
        let mut request = self.request(18);
        request.ip = level;
        request.port = name;
        match call(&mut request)? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(io::const_error!(io::ErrorKind::InvalidData, "invalid boolean socket option")),
        }
    }
    fn set_option(&self, level: u32, name: u16, value: u32) -> io::Result<()> {
        let mut request = self.request(13);
        request.ip = level;
        request.port = name;
        request.nanos = value;
        call(&mut request).map(|_| ())
    }
}
impl Drop for Socket {
    fn drop(&mut self) {
        // A final close error has already been recorded by the provider. The
        // infallible std Drop path stops rather than claim cleanup succeeded.
        if let Err(error) = call(&mut self.request(4)) {
            panic!("Wii socket cleanup failed: {error}");
        }
    }
}

pub struct LookupHost {
    handle: *mut core::ffi::c_void,
    port: u16,
    literal: Option<u32>,
}
// SAFETY: This iterator owns its provider allocation and has one next caller.
unsafe impl Send for LookupHost {}
impl Iterator for LookupHost {
    type Item = SocketAddr;
    fn next(&mut self) -> Option<SocketAddr> {
        if let Some(ip) = self.literal.take() {
            return Some(address(ip, self.port));
        }
        if self.handle.is_null() {
            return None;
        }
        match call(&mut NetRequest::new(16, self.handle)) {
            Ok(value) if value == (1i64 << 32) => None,
            Ok(value) => Some(address(value as u32, self.port)),
            Err(error) => panic!("Wii DNS iterator failed: {error}"),
        }
    }
}
impl Drop for LookupHost {
    fn drop(&mut self) {
        if self.handle.is_null() {
            return;
        }
        if let Err(error) = call(&mut NetRequest::new(17, self.handle)) {
            panic!("Wii DNS cleanup failed: {error}");
        }
    }
}
pub fn lookup_host(host: &str, port: u16) -> io::Result<LookupHost> {
    if let Ok(ip) = host.parse::<Ipv4Addr>() {
        return Ok(LookupHost {
            handle: crate::ptr::null_mut(),
            port,
            literal: Some(u32::from_be_bytes(ip.octets())),
        });
    }
    let unbracketed = host
        .strip_prefix('[')
        .and_then(|host| host.strip_suffix(']'))
        .unwrap_or(host);
    if unbracketed.parse::<Ipv6Addr>().is_ok() {
        return unsupported();
    }
    let mut request = NetRequest::new(15, crate::ptr::null_mut());
    request.data = host.as_ptr().cast_mut();
    request.length = host.len();
    let handle = call(&mut request)? as usize as *mut core::ffi::c_void;
    if handle.is_null() {
        return Err(io::const_error!(
            io::ErrorKind::InvalidData,
            "null Wii DNS handle"
        ));
    }
    Ok(LookupHost {
        handle,
        port,
        literal: None,
    })
}

pub struct TcpStream(Socket);

impl TcpStream {
    pub fn connect<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        super::each_addr(addr, |a| Socket::open(1, a, false).map(Self))
    }

    pub fn connect_timeout(addr: &SocketAddr, timeout: Duration) -> io::Result<Self> {
        if timeout.is_zero() {
            return Err(io::const_error!(
                io::ErrorKind::InvalidInput,
                "zero connect timeout"
            ));
        }
        ipv4(addr)?;
        unsupported()
    }

    pub fn set_read_timeout(&self, value: Option<Duration>) -> io::Result<()> {
        self.0.set_timeout(false, value)
    }

    pub fn set_write_timeout(&self, value: Option<Duration>) -> io::Result<()> {
        self.0.set_timeout(true, value)
    }

    pub fn read_timeout(&self) -> io::Result<Option<Duration>> {
        self.0.timeout(false)
    }

    pub fn write_timeout(&self) -> io::Result<Option<Duration>> {
        self.0.timeout(true)
    }

    pub fn peek(&self, buffer: &mut [u8]) -> io::Result<usize> {
        self.0.receive(buffer, true, false).map(|(count, _)| count)
    }

    pub fn read(&self, buffer: &mut [u8]) -> io::Result<usize> {
        self.0.receive(buffer, false, false).map(|(count, _)| count)
    }

    pub fn read_buf(&self, mut cursor: BorrowedCursor<'_, u8>) -> io::Result<()> {
        let mut buffer = vec![0; cursor.capacity().min(8192)];
        let count = self.read(&mut buffer)?;
        cursor.append(&buffer[..count]);
        Ok(())
    }

    pub fn read_vectored(&self, buffers: &mut [IoSliceMut<'_>]) -> io::Result<usize> {
        match buffers.iter_mut().find(|b| !b.is_empty()) {
            Some(buffer) => self.read(buffer),
            None => self.read(&mut []),
        }
    }

    pub fn is_read_vectored(&self) -> bool {
        false
    }

    pub fn write(&self, buffer: &[u8]) -> io::Result<usize> {
        self.0.send(buffer, None)
    }

    pub fn write_vectored(&self, buffers: &[IoSlice<'_>]) -> io::Result<usize> {
        match buffers.iter().find(|b| !b.is_empty()) {
            Some(buffer) => self.write(buffer),
            None => self.write(&[]),
        }
    }

    pub fn is_write_vectored(&self) -> bool {
        false
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.0.socket_addr(true)
    }

    pub fn socket_addr(&self) -> io::Result<SocketAddr> {
        self.0.socket_addr(false)
    }

    pub fn shutdown(&self, how: Shutdown) -> io::Result<()> {
        self.0.unit(
            12,
            match how {
                Shutdown::Read => 0,
                Shutdown::Write => 1,
                Shutdown::Both => 2,
            },
        )
    }

    pub fn duplicate(&self) -> io::Result<TcpStream> {
        self.0.duplicate().map(Self)
    }

    pub fn set_linger(&self, _: Option<Duration>) -> io::Result<()> {
        unsupported()
    }

    pub fn linger(&self) -> io::Result<Option<Duration>> {
        unsupported()
    }

    pub fn set_keepalive(&self, value: bool) -> io::Result<()> {
        self.0.set_option(0xffff, 8, value.into())
    }

    pub fn keepalive(&self) -> io::Result<bool> {
        self.0.bool_option(0xffff, 8)
    }

    pub fn set_nodelay(&self, value: bool) -> io::Result<()> {
        self.0.set_option(6, 1, value.into())
    }

    pub fn nodelay(&self) -> io::Result<bool> {
        self.0.bool_option(6, 1)
    }

    pub fn set_ttl(&self, value: u32) -> io::Result<()> {
        set_ipv4_ttl(&self.0, value)
    }

    pub fn ttl(&self) -> io::Result<u32> {
        unsupported()
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        unsupported()
    }

    pub fn set_nonblocking(&self, value: bool) -> io::Result<()> {
        self.0.unit(9, value.into())
    }
}

impl fmt::Debug for TcpStream {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TcpStream").field(&self.0).finish()
    }
}

pub struct TcpListener(Socket);

impl TcpListener {
    pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        super::each_addr(addr, |a| Socket::open(2, a, false).map(Self))
    }

    pub fn socket_addr(&self) -> io::Result<SocketAddr> {
        self.0.socket_addr(false)
    }

    pub fn accept(&self) -> io::Result<(TcpStream, SocketAddr)> {
        let mut request = self.0.request(5);
        let stream = Socket::from_reply(call(&mut request)?)?;
        Ok((TcpStream(stream), address(request.ip, request.port)))
    }

    pub fn duplicate(&self) -> io::Result<TcpListener> {
        self.0.duplicate().map(Self)
    }

    pub fn set_ttl(&self, value: u32) -> io::Result<()> {
        set_ipv4_ttl(&self.0, value)
    }

    pub fn ttl(&self) -> io::Result<u32> {
        unsupported()
    }

    pub fn set_only_v6(&self, _: bool) -> io::Result<()> {
        unsupported()
    }

    pub fn only_v6(&self) -> io::Result<bool> {
        unsupported()
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        unsupported()
    }

    pub fn set_nonblocking(&self, value: bool) -> io::Result<()> {
        self.0.unit(9, value.into())
    }
}

impl fmt::Debug for TcpListener {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TcpListener").field(&self.0).finish()
    }
}

pub struct UdpSocket(Socket);

impl UdpSocket {
    pub fn bind<A: ToSocketAddrs>(addr: A) -> io::Result<Self> {
        super::each_addr(addr, |a| Socket::open(2, a, true).map(Self))
    }

    pub fn peer_addr(&self) -> io::Result<SocketAddr> {
        self.0.socket_addr(true)
    }

    pub fn socket_addr(&self) -> io::Result<SocketAddr> {
        self.0.socket_addr(false)
    }

    pub fn recv_from(&self, buffer: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.0.receive(buffer, false, true)
    }

    pub fn peek_from(&self, buffer: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        self.0.receive(buffer, true, true)
    }

    pub fn send_to(&self, buffer: &[u8], destination: &SocketAddr) -> io::Result<usize> {
        self.0.send(buffer, Some(destination))
    }

    pub fn duplicate(&self) -> io::Result<UdpSocket> {
        self.0.duplicate().map(Self)
    }

    pub fn set_read_timeout(&self, value: Option<Duration>) -> io::Result<()> {
        self.0.set_timeout(false, value)
    }

    pub fn set_write_timeout(&self, value: Option<Duration>) -> io::Result<()> {
        self.0.set_timeout(true, value)
    }

    pub fn read_timeout(&self) -> io::Result<Option<Duration>> {
        self.0.timeout(false)
    }

    pub fn write_timeout(&self) -> io::Result<Option<Duration>> {
        self.0.timeout(true)
    }

    pub fn set_broadcast(&self, value: bool) -> io::Result<()> {
        self.0.set_option(0xffff, 0x20, value.into())
    }

    pub fn broadcast(&self) -> io::Result<bool> {
        self.0.bool_option(0xffff, 0x20)
    }

    pub fn set_multicast_loop_v4(&self, _: bool) -> io::Result<()> {
        unsupported()
    }

    pub fn multicast_loop_v4(&self) -> io::Result<bool> {
        unsupported()
    }

    pub fn set_multicast_ttl_v4(&self, _: u32) -> io::Result<()> {
        unsupported()
    }

    pub fn multicast_ttl_v4(&self) -> io::Result<u32> {
        unsupported()
    }

    pub fn set_multicast_loop_v6(&self, _: bool) -> io::Result<()> {
        unsupported()
    }

    pub fn multicast_loop_v6(&self) -> io::Result<bool> {
        unsupported()
    }

    pub fn join_multicast_v4(&self, _: &Ipv4Addr, _: &Ipv4Addr) -> io::Result<()> {
        unsupported()
    }

    pub fn join_multicast_v6(&self, _: &Ipv6Addr, _: u32) -> io::Result<()> {
        unsupported()
    }

    pub fn leave_multicast_v4(&self, _: &Ipv4Addr, _: &Ipv4Addr) -> io::Result<()> {
        unsupported()
    }

    pub fn leave_multicast_v6(&self, _: &Ipv6Addr, _: u32) -> io::Result<()> {
        unsupported()
    }

    pub fn set_ttl(&self, value: u32) -> io::Result<()> {
        set_ipv4_ttl(&self.0, value)
    }

    pub fn ttl(&self) -> io::Result<u32> {
        unsupported()
    }

    pub fn take_error(&self) -> io::Result<Option<io::Error>> {
        unsupported()
    }

    pub fn set_nonblocking(&self, value: bool) -> io::Result<()> {
        self.0.unit(9, value.into())
    }

    pub fn recv(&self, buffer: &mut [u8]) -> io::Result<usize> {
        self.0.receive(buffer, false, false).map(|(count, _)| count)
    }

    pub fn peek(&self, buffer: &mut [u8]) -> io::Result<usize> {
        self.0.receive(buffer, true, false).map(|(count, _)| count)
    }

    pub fn send(&self, buffer: &[u8]) -> io::Result<usize> {
        self.0.send(buffer, None)
    }

    pub fn connect<A: ToSocketAddrs>(&self, addr: A) -> io::Result<()> {
        super::each_addr(addr, |a| {
            let (ip, port) = ipv4(a)?;
            let mut request = self.0.request(14);
            request.ip = ip;
            request.port = port;
            call(&mut request).map(|_| ())
        })
    }
}

impl fmt::Debug for UdpSocket {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("UdpSocket").field(&self.0).finish()
    }
}
