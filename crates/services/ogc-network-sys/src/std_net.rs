//! Owned IPv4 sockets over the production IOS IPC transport.
pub use super::socket_rules::NetError;
use super::socket_rules::{self as rules, words};
use alloc::sync::Arc;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::sync::atomic::{AtomicI32, AtomicU32, Ordering};
use core::time::Duration;
use ogc_ipc::{FileDescriptor, IoctlBuffers, IoctlvEntry};
use ogc_lwp_driver as lwp;
#[path = "socket_abi.rs"]
mod abi;
#[path = "socket_receive.rs"]
mod receive;
pub use abi::NetRequest;

static NEXT_EPHEMERAL: AtomicU32 = AtomicU32::new(0);
static LAST_REQUEST_COMMAND: AtomicI32 = AtomicI32::new(0);
static LAST_REQUEST_CODE: AtomicI32 = AtomicI32::new(0);

static CLOSE_FAILURES: AtomicU32 = AtomicU32::new(0);
static LAST_CLOSE_CODE: AtomicI32 = AtomicI32::new(0);
static LAST_CLOSE_DOMAIN: AtomicU32 = AtomicU32::new(0);
static LAST_CLOSE_FD: AtomicI32 = AtomicI32::new(-1);
static LIVE_SOCKETS: AtomicU32 = AtomicU32::new(0);

#[derive(Clone, Copy, Debug)]
pub struct NetworkSnapshot {
    pub last_failed_ioctl: i32,
    pub last_failed_ioctl_code: i32,
    pub live_sockets: u32,
    pub close_failures: u32,
    pub initialization_close_failures: u32,
    pub last_initialization_close_error: i32,
    pub last_close_fd: i32,
    pub last_close_error: NetError,
}

pub fn snapshot() -> NetworkSnapshot {
    let irq = lwp::critical_enter();
    let value = NetworkSnapshot {
        last_failed_ioctl: LAST_REQUEST_COMMAND.load(Ordering::Relaxed),
        last_failed_ioctl_code: LAST_REQUEST_CODE.load(Ordering::Relaxed),
        live_sockets: LIVE_SOCKETS.load(Ordering::Relaxed),
        close_failures: CLOSE_FAILURES.load(Ordering::Relaxed),
        initialization_close_failures: super::initialization_close_failures(),
        last_initialization_close_error: super::last_initialization_close_error(),
        last_close_fd: LAST_CLOSE_FD.load(Ordering::Relaxed),
        last_close_error: NetError {
            domain: LAST_CLOSE_DOMAIN.load(Ordering::Relaxed),
            code: LAST_CLOSE_CODE.load(Ordering::Relaxed),
        },
    };
    lwp::critical_exit(irq);
    value
}

fn context() -> Result<(), NetError> {
    lwp::current_thread_checked()
        .map(|_| ())
        .map_err(|_| NetError::local(16))
}

/// Applications select standalone startup or attachment explicitly. Sockets
/// retain the initialized session; net_deinit rejects live leases with EBUSY.
pub fn initialize(attach: bool) -> Result<(), NetError> {
    context()?;
    ensure_network(attach)?;
    let session = Session::lease()?;
    drop(session);
    Ok(())
}

fn ensure_network(attach: bool) -> Result<(), NetError> {
    let result = if attach {
        super::net_attach_active()
    } else {
        super::net_init()
    };
    if result < 0 {
        Err(NetError {
            domain: 4,
            code: result,
        })
    } else {
        Ok(())
    }
}

struct Session {
    fd: FileDescriptor,
}
impl Session {
    fn acquire() -> Result<Self, NetError> {
        context()?;
        ensure_network(false)?;
        Self::lease()
    }
    fn lease() -> Result<Self, NetError> {
        super::wii_backend::lease_acquire()
            .map(|fd| Self { fd })
            .map_err(|code| NetError::local(if code == -16 { 16 } else { 115 }))
    }
    fn request(&self, command: i32, input: &mut [u8], output: &mut [u8]) -> Result<i32, NetError> {
        context()?;
        let result = NetError::reply(self.fd.ioctl_result(
            command,
            IoctlBuffers::from_slices(Some(input), Some(output)),
        ));
        if let Err(error) = result {
            let irq = lwp::critical_enter();
            LAST_REQUEST_COMMAND.store(command, Ordering::Relaxed);
            LAST_REQUEST_CODE.store(error.code, Ordering::Relaxed);
            lwp::critical_exit(irq);
        }
        result
    }
    fn requestv<'a, const N: usize>(
        &self,
        command: i32,
        cnt_in: usize,
        vectors: [IoctlvEntry<'a>; N],
    ) -> Result<i32, NetError> {
        context()?;
        let result = NetError::reply(self.fd.ioctlv_result(command, cnt_in, vectors));
        if let Err(error) = result {
            let irq = lwp::critical_enter();
            LAST_REQUEST_COMMAND.store(command, Ordering::Relaxed);
            LAST_REQUEST_CODE.store(error.code, Ordering::Relaxed);
            lwp::critical_exit(irq);
        }
        result
    }
    fn unit(&self, command: i32, input: &mut [u8]) -> Result<(), NetError> {
        match self.request(command, input, &mut [])? {
            0 => Ok(()),
            _ => Err(NetError::local(77)),
        }
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        super::wii_backend::lease_release();
    }
}

#[repr(align(32))]
struct DnsReply([u8; 0x460]);

#[derive(Clone, Copy)]
struct Settings {
    nonblocking: bool,
    timeouts: [Option<Duration>; 2],
    broadcast_allowed: bool,
    peer: Option<(u32, u16)>,
}
pub type Clock = fn() -> Result<Duration, NetError>;
// Socket clones share both the cursor and its sleepable reader lock.
struct Reader {
    mutex: lwp::MutexHandle,
    buffer: UnsafeCell<receive::ReceiveBuffer>,
}
impl Reader {
    fn new() -> Result<Self, NetError> {
        let mut mutex = lwp::LWP_MUTEX_NULL;
        if unsafe { lwp::init_mutex(&mut mutex, false) } != lwp::LWP_SUCCESSFUL {
            return Err(NetError::local(12));
        }
        Ok(Self {
            mutex,
            buffer: UnsafeCell::new(receive::ReceiveBuffer::default()),
        })
    }
    fn lock(&self) -> Result<ReaderGuard<'_>, NetError> {
        if lwp::mutex_lock(self.mutex) != lwp::LWP_SUCCESSFUL {
            return Err(NetError::local(16));
        }
        Ok(ReaderGuard(self))
    }
}
impl Drop for Reader {
    fn drop(&mut self) {
        if lwp::close_mutex(self.mutex) != lwp::LWP_SUCCESSFUL {
            panic!("Wii socket reader mutex cleanup failed");
        }
    }
}
struct ReaderGuard<'a>(&'a Reader);
impl ReaderGuard<'_> {
    fn buffer(&mut self) -> &mut receive::ReceiveBuffer {
        // SAFETY: the shared socket's reader mutex is held for this guard.
        unsafe { &mut *self.0.buffer.get() }
    }
}
impl Drop for ReaderGuard<'_> {
    fn drop(&mut self) {
        if lwp::mutex_unlock(self.0.mutex) != lwp::LWP_SUCCESSFUL {
            panic!("Wii socket reader mutex unlock failed");
        }
    }
}
struct Inner {
    session: Arc<Session>,
    fd: i32,
    datagram: bool,
    clock: Clock,
    close_attempted: bool,
    settings: UnsafeCell<Settings>,
    reader: Reader,
}
// SAFETY: Settings are accessed only in bounded LWP interrupt critical sections
// on the single Wii CPU. ReceiveBuffer is protected by the shared reader mutex.
// Every transport operation uses the IPC serializer.
unsafe impl Sync for Inner {}
impl Inner {
    fn settings<R>(&self, operation: impl FnOnce(&mut Settings) -> R) -> R {
        let irq = lwp::critical_enter();
        let result = operation(unsafe { &mut *self.settings.get() });
        lwp::critical_exit(irq);
        result
    }
    fn close(&mut self) -> Result<(), NetError> {
        self.close_attempted = true;
        let result = self.session.unit(3, &mut words(&[self.fd as u32]));
        if let Err(error) = result {
            let irq = lwp::critical_enter();
            LAST_CLOSE_FD.store(self.fd, Ordering::Relaxed);
            LAST_CLOSE_DOMAIN.store(error.domain, Ordering::Relaxed);
            LAST_CLOSE_CODE.store(error.code, Ordering::Relaxed);
            CLOSE_FAILURES.fetch_add(1, Ordering::Relaxed);
            lwp::critical_exit(irq);
        }
        result
    }
}
impl Drop for Inner {
    fn drop(&mut self) {
        if !self.close_attempted
            && let Err(error) = self.close()
        {
            panic!("Wii socket cleanup failed: {error:?}");
        }
        // A failed close remains visible in the snapshot. Once no Rust owner
        // remains, explicit session teardown can reclaim any orphan IOS socket.
        LIVE_SOCKETS.fetch_sub(1, Ordering::AcqRel);
    }
}

#[derive(Clone)]
pub struct Socket(Arc<Inner>);
impl Socket {
    fn owned(session: Arc<Session>, fd: i32, datagram: bool, clock: Clock, reader: Reader) -> Self {
        LIVE_SOCKETS.fetch_add(1, Ordering::AcqRel);
        Self(Arc::new(Inner {
            session,
            fd,
            datagram,
            clock,
            close_attempted: false,
            reader,
            settings: UnsafeCell::new(Settings {
                nonblocking: false,
                timeouts: [None; 2],
                broadcast_allowed: false,
                peer: None,
            }),
        }))
    }
    fn create(datagram: bool, clock: Clock) -> Result<Self, NetError> {
        let reader = Reader::new()?;
        let session = Arc::new(Session::acquire()?);
        let fd = session.request(
            15,
            &mut words(&[2, if datagram { 2 } else { 1 }, 0]),
            &mut [],
        )?;
        let socket = Self::owned(session, fd, datagram, clock, reader);
        // Match libogc: request its preferred receive window, then retain IOS's
        // valid default if shared network memory cannot satisfy the request.
        rules::receive_window_setup(socket.set_option(0xffff, 0x1002, 32768))?;
        Ok(socket)
    }
    fn physical_nonblocking(&self) -> Result<(), NetError> {
        let flags = self
            .0
            .session
            .request(5, &mut words(&[self.0.fd as u32, 3, 0]), &mut [])?;
        self.0
            .session
            .unit(5, &mut words(&[self.0.fd as u32, 4, flags as u32 | 4]))
    }
    fn address_operation(&self, command: i32, ip: u32, port: u16) -> Result<(), NetError> {
        self.0.session.unit(
            command,
            &mut rules::addressed(self.0.fd, &rules::address(ip, port), false, 0),
        )
    }
    pub fn connect(ip: u32, port: u16, clock: Clock) -> Result<Self, NetError> {
        let socket = Self::create(false, clock)?;
        // Ordinary connect may block in IOS. connect_timeout stays unsupported
        // until a real connection-completion/SO_ERROR query is verified.
        socket.address_operation(4, ip, port)?;
        socket
            .0
            .settings(|settings| settings.peer = Some((ip, port)));
        socket.physical_nonblocking()?;
        Ok(socket)
    }
    pub fn bind(ip: u32, port: u16, datagram: bool, clock: Clock) -> Result<Self, NetError> {
        let socket = Self::create(datagram, clock)?;
        socket.physical_nonblocking()?;
        rules::bind_port(
            port,
            || 49152 + (NEXT_EPHEMERAL.fetch_add(1, Ordering::Relaxed) % 16384) as u16,
            |candidate| socket.address_operation(2, ip, candidate),
        )?;
        if !datagram {
            socket
                .0
                .session
                .unit(10, &mut words(&[socket.0.fd as u32, 16]))?;
        }
        Ok(socket)
    }
    fn poll(&self, events: u32, milliseconds: i32) -> Result<bool, NetError> {
        let mut output = words(&[self.0.fd as u32, events, 0]);
        let result =
            self.0
                .session
                .request(11, &mut words(&[0, milliseconds as u32]), &mut output)?;
        let revents =
            u32::from_be_bytes(output[8..12].try_into().map_err(|_| NetError::local(77))?);
        if output[..4] != self.0.fd.to_be_bytes() || result > 1 {
            return Err(NetError::local(77));
        }
        if revents & 0x80 != 0 {
            return Err(NetError::local(9));
        }
        if result == 0 {
            if revents != 0 {
                return Err(NetError::local(77));
            }
            return Ok(false);
        }
        if revents & (events | 0x20 | 0x40) == 0 {
            return Err(NetError::local(77));
        }
        Ok(true)
    }
    fn retry<R>(
        &self,
        write: bool,
        operation: impl FnMut() -> Result<R, NetError>,
    ) -> Result<R, NetError> {
        let settings = self.0.settings(|s| *s);
        rules::retry(
            if settings.nonblocking {
                None
            } else {
                settings.timeouts[usize::from(write)]
            },
            settings.nonblocking,
            || (self.0.clock)(),
            operation,
            |ms| self.poll(if write { 8 } else { 1 }, ms),
        )
    }
    pub fn accept(&self) -> Result<(Self, u32, u16), NetError> {
        let reader = Reader::new()?;
        let mut address = rules::address(0, 0);
        let fd = self.retry(false, || {
            self.0
                .session
                .request(1, &mut words(&[self.0.fd as u32]), &mut address)
        })?;
        let accepted = Self::owned(self.0.session.clone(), fd, false, self.0.clock, reader);
        let (ip, port) = rules::read_address(&address)?;
        accepted.physical_nonblocking()?;
        Ok((accepted, ip, port))
    }
    pub fn address(&self, peer: bool) -> Result<(u32, u16), NetError> {
        let mut output = rules::address(0, 0);
        let result = self.0.session.request(
            if peer { 6 } else { 7 },
            &mut words(&[self.0.fd as u32]),
            &mut output,
        )?;
        if result != 0 {
            return Err(NetError::local(77));
        }
        rules::read_address(&output)
    }
    pub fn udp_connect(&self, ip: u32, port: u16) -> Result<(), NetError> {
        if !self.0.datagram {
            return Err(NetError::local(22));
        }
        self.address_operation(4, ip, port)?;
        self.0.settings(|settings| settings.peer = Some((ip, port)));
        Ok(())
    }
    pub fn receive(
        &self,
        buffer: &mut [u8],
        peek: bool,
        from: bool,
    ) -> Result<(usize, Option<(u32, u16)>), NetError> {
        if !self.0.datagram && buffer.is_empty() {
            return Ok((0, None));
        }
        context()?;
        let mut address = rules::address(0, 0);
        let mut reader = self.0.reader.lock()?;
        let count = reader.buffer().receive(
            buffer,
            self.0.datagram,
            peek,
            |wire| {
                self.retry(false, || {
                    let params = words(&[self.0.fd as u32, 4 | if peek { 2 } else { 0 }]);
                    let source = if from {
                        IoctlvEntry::Out(&mut address)
                    } else {
                        IoctlvEntry::OutNull
                    };
                    self.0
                        .session
                        .requestv(
                            12,
                            1,
                            [IoctlvEntry::In(&params), IoctlvEntry::Out(wire), source],
                        )
                        .map(|value| value as usize)
                })
            },
            NetError::local,
        )?;
        Ok((
            count,
            if from {
                Some(rules::read_address(&address)?)
            } else {
                None
            },
        ))
    }
    pub fn send(&self, buffer: &[u8], destination: Option<(u32, u16)>) -> Result<usize, NetError> {
        if !self.0.datagram && buffer.is_empty() {
            return Ok(0);
        }
        if self.0.datagram && buffer.len() > 65507 {
            return Err(NetError::local(122));
        }
        context()?;
        if self.0.datagram && !self.0.settings(|settings| settings.broadcast_allowed) {
            let peer = destination.or_else(|| self.0.settings(|settings| settings.peer));
            if let Some((ip, _)) = peer {
                let (local, mask) = ipv4_config()?;
                rules::check_broadcast_send(ip, local, mask, false)?;
            }
        }
        let length = buffer.len().min(65535);
        self.retry(true, || {
            let params = if let Some((ip, port)) = destination {
                rules::addressed(self.0.fd, &rules::address(ip, port), true, 0)
            } else {
                let mut packet = words(&[self.0.fd as u32, 0, 0]);
                packet.resize(40, 0);
                packet
            };
            let count = self.0.session.requestv(
                13,
                2,
                [IoctlvEntry::In(&buffer[..length]), IoctlvEntry::In(&params)],
            )? as usize;
            if count > length || (self.0.datagram && count != length) {
                return Err(NetError::local(77));
            }
            Ok(count)
        })
    }
    pub fn set_nonblocking(&self, value: bool) -> Result<(), NetError> {
        self.physical_nonblocking()?;
        self.0.settings(|s| s.nonblocking = value);
        Ok(())
    }
    pub fn set_timeout(&self, write: bool, value: Option<Duration>) -> Result<(), NetError> {
        context()?;
        if value.is_some_and(|v| v.is_zero()) {
            return Err(NetError::local(22));
        }
        self.0.settings(|s| s.timeouts[usize::from(write)] = value);
        Ok(())
    }
    pub fn timeout(&self, write: bool) -> Result<Option<Duration>, NetError> {
        context()?;
        Ok(self.0.settings(|s| s.timeouts[usize::from(write)]))
    }
    pub fn shutdown(&self, how: u32) -> Result<(), NetError> {
        if how > 2 {
            return Err(NetError::local(22));
        }
        self.0
            .session
            .unit(14, &mut words(&[self.0.fd as u32, how]))
    }
    pub fn set_option(&self, level: u32, name: u32, value: u32) -> Result<(), NetError> {
        if (level, name) == (0xffff, 0x20) {
            if !self.0.datagram || value > 1 {
                return Err(NetError::local(22));
            }
            // IOS rejects SO_BROADCAST. Enforce the option at the only
            // operation it governs, so success changes real socket behavior.
            self.0
                .settings(|settings| settings.broadcast_allowed = value != 0);
            return Ok(());
        }
        let allowed = match (level, name) {
            (0xffff, 8 | 0x20) => value <= 1,
            (0xffff, 0x1002) => value > 0,
            (6, 1) => value <= 1,
            (0, 4) => value <= 255,
            _ => false,
        };
        if !allowed {
            return Err(NetError::local(134));
        }
        let name = lwip_sys::option_to_ios(level, name);
        let mut params = words(&[self.0.fd as u32, level, name, 4, value]);
        params.resize(36, 0);
        self.0.session.unit(9, &mut params)
    }
    /// Read a real boolean socket option; never return cached setter state.
    pub fn bool_option(&self, level: u32, name: u32) -> Result<bool, NetError> {
        if (level, name) == (0xffff, 0x20) {
            if !self.0.datagram {
                return Err(NetError::local(92));
            }
            return Ok(self.0.settings(|settings| settings.broadcast_allowed));
        }
        if !matches!((level, name), (0xffff, 8 | 0x20) | (6, 1)) {
            return Err(NetError::local(134));
        }
        let name = lwip_sys::option_to_ios(level, name);
        let mut reply = rules::boolean_option_request(self.0.fd, level, name);
        let result = self.0.session.request(8, &mut [], &mut reply)?;
        if result != 0 {
            return Err(NetError::local(77));
        }
        rules::boolean_option_reply(self.0.fd, level, name, &reply)
    }
    pub fn release(self) -> Result<(), NetError> {
        if let Some(mut inner) = Arc::into_inner(self.0) {
            inner.close()
        } else {
            Ok(())
        }
    }
}

fn ipv4_config() -> Result<(u32, u32), NetError> {
    let mut local = super::InAddr::default();
    let mut mask = super::InAddr::default();
    let result =
        unsafe { super::if_configex(&mut local, &mut mask, core::ptr::null_mut(), true, 0) };
    if result < 0 {
        return Err(NetError {
            domain: 4,
            code: result,
        });
    }
    // InAddr values and the std ABI both use the IPv4 network byte order.
    Ok((local.s_addr, mask.s_addr))
}

pub fn lookup(host: &[u8]) -> Result<Vec<u32>, NetError> {
    if host.is_empty() || host.len() > 255 || host.contains(&0) {
        return Err(NetError::local(22));
    }
    let session = Session::acquire()?;
    let mut request = host.to_vec();
    request.push(0);
    // Per-call storage survives the complete synchronous request. No shared
    // hostent pointers escape to std or another DNS caller.
    let mut response = DnsReply([0; 0x460]);
    let result = session.request(0x11, &mut request, &mut response.0)?;
    if result != 0 {
        return Err(NetError::local(77));
    }
    rules::dns(&response.0)
}
