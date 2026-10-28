//! Wii socket backend for the host-testable GDB transport engine.

use gdb_transport::{
    AcceptStatus, CloseError, GdbTransport, IoStatus, PollReport, SocketBackend, TransportError,
};
use ogc_network as network;
use ogc_network::sys::{F_GETFL, F_SETFL};

pub use gdb_transport::{
    ArmError, ControlQueueError, ControlRequest, CooperativeControl, CooperativeState,
    DEFAULT_PORT, DIAGNOSTIC_QUERY, DIAGNOSTIC_REPLY_LEN, DebuggerState, DiagnosticFailure,
    DiagnosticOperation, ListenerDiagnostics, MAX_PACKET_SIZE, MainControlResult, PauseConditions,
};

const LISTEN_BACKLOG: u32 = 1;
const EAGAIN_IOS: i32 = -6;
const EAGAIN_POSIX: i32 = -11;
const EINTR_IOS: i32 = -27;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WiiSocketError {
    pub operation: DiagnosticOperation,
    pub code: i32,
    pub cleanup: Option<i32>,
}

pub struct WiiNetworkTransport(GdbTransport<WiiSocketBackend>);

impl WiiNetworkTransport {
    pub fn bind(port: u16) -> Result<Self, TransportError<WiiSocketError>> {
        GdbTransport::bind(port).map(Self)
    }

    pub fn bind_with_control(
        port: u16,
        control: &'static CooperativeControl,
    ) -> Result<Self, TransportError<WiiSocketError>> {
        GdbTransport::bind_with_control(port, control).map(Self)
    }

    pub fn poll(&mut self, now_ms: u64) -> Result<PollReport, TransportError<WiiSocketError>> {
        self.0.poll(now_ms)
    }

    pub fn state(&self) -> DebuggerState {
        self.0.state()
    }

    pub fn arm(&mut self) -> Result<(), ArmError> {
        self.0.arm()
    }

    pub fn disarm(&mut self) {
        self.0.disarm();
    }

    pub fn close(&mut self) -> Result<(), CloseError<WiiSocketError>> {
        self.0.close()
    }
}

struct WiiSocketBackend {
    listener: Option<i32>,
    client: Option<i32>,
    pending_client_close: Option<i32>,
}

impl SocketBackend for WiiSocketBackend {
    type Error = WiiSocketError;

    fn bind(port: u16) -> Result<Self, Self::Error> {
        let local_ip = network::get_host_ip();
        if local_ip == 0 {
            return Err(socket_error(
                DiagnosticOperation::HostIp,
                network::SOCKET_ERROR,
            ));
        }
        let listener = network::socket(network::PF_INET, network::SOCK_STREAM, 0);
        if listener < 0 {
            return Err(socket_error(DiagnosticOperation::Socket, listener));
        }
        if let Err(error) = set_nonblocking(listener) {
            return Err(close_after_failure(listener, error));
        }

        let mut address = network::SockaddrIn::default();
        address.sin_len = core::mem::size_of::<network::SockaddrIn>() as u8;
        address.sin_family = network::AF_INET as u8;
        address.sin_port = port.to_be();
        address.sin_addr.s_addr = local_ip;
        let bind_result = network::bind_addr(listener, &address);
        if bind_result < 0 {
            return Err(close_after_failure(
                listener,
                socket_error(DiagnosticOperation::Bind, bind_result),
            ));
        }
        let listen_result = network::listen(listener, LISTEN_BACKLOG);
        if listen_result < 0 {
            return Err(close_after_failure(
                listener,
                socket_error(DiagnosticOperation::Listen, listen_result),
            ));
        }
        Ok(Self {
            listener: Some(listener),
            client: None,
            pending_client_close: None,
        })
    }

    fn accept_one(&mut self, reject_new_client: bool) -> Result<AcceptStatus, Self::Error> {
        let listener = self
            .listener
            .ok_or_else(|| socket_error(DiagnosticOperation::Transport, network::SOCKET_ERROR))?;
        let mut peer = network::SockaddrIn::default();
        let accepted = network::accept_addr(listener, &mut peer);
        if accepted == EINTR_IOS {
            return Ok(AcceptStatus::Interrupted);
        }
        if is_would_block(accepted) {
            return Ok(AcceptStatus::WouldBlock);
        }
        if accepted < 0 {
            return Err(socket_error(DiagnosticOperation::Accept, accepted));
        }
        if reject_new_client {
            let result = network::close(accepted);
            if result < 0 {
                self.pending_client_close = Some(accepted);
                return Err(socket_error(DiagnosticOperation::Close, result));
            }
            return Ok(AcceptStatus::RejectedExtraClient);
        }
        if let Err(error) = set_nonblocking(accepted) {
            let error = close_after_failure(accepted, error);
            if error.cleanup.is_some() {
                self.pending_client_close = Some(accepted);
            }
            return Err(error);
        }
        self.client = Some(accepted);
        Ok(AcceptStatus::Accepted)
    }

    fn read(&mut self, buffer: &mut [u8]) -> Result<IoStatus, Self::Error> {
        let client = self
            .client
            .ok_or_else(|| socket_error(DiagnosticOperation::Transport, network::SOCKET_ERROR))?;
        let result = network::recv_slice(client, buffer, 0);
        if result == EINTR_IOS {
            return Ok(IoStatus::Interrupted);
        }
        if is_would_block(result) {
            return Ok(IoStatus::WouldBlock);
        }
        if result == 0 {
            return Ok(IoStatus::Disconnected);
        }
        if result < 0 {
            return Err(socket_error(DiagnosticOperation::Read, result));
        }
        Ok(IoStatus::Bytes(result as usize))
    }

    fn write(&mut self, buffer: &[u8]) -> Result<IoStatus, Self::Error> {
        let client = self
            .client
            .ok_or_else(|| socket_error(DiagnosticOperation::Transport, network::SOCKET_ERROR))?;
        let result = network::write_slice(client, buffer);
        if result == EINTR_IOS {
            return Ok(IoStatus::Interrupted);
        }
        if is_would_block(result) {
            return Ok(IoStatus::WouldBlock);
        }
        if result < 0 {
            return Err(socket_error(DiagnosticOperation::Write, result));
        }
        if result == 0 && !buffer.is_empty() {
            return Ok(IoStatus::Disconnected);
        }
        Ok(IoStatus::Bytes(result as usize))
    }

    fn close_client(&mut self) -> Result<(), Self::Error> {
        if let Some(client) = self.client {
            let result = network::close(client);
            if result < 0 {
                return Err(socket_error(DiagnosticOperation::Close, result));
            }
            self.client = None;
        }
        Ok(())
    }

    fn close_listener(&mut self) -> Result<(), Self::Error> {
        if let Some(socket) = self.pending_client_close {
            let result = network::close(socket);
            if result < 0 {
                return Err(socket_error(DiagnosticOperation::Close, result));
            }
            self.pending_client_close = None;
        }
        if let Some(listener) = self.listener {
            let result = network::close(listener);
            if result < 0 {
                return Err(socket_error(DiagnosticOperation::Close, result));
            }
            self.listener = None;
        }
        Ok(())
    }
}

fn set_nonblocking(socket: i32) -> Result<(), WiiSocketError> {
    let flags = network::fcntl(socket, F_GETFL, 0);
    if flags < 0 {
        return Err(socket_error(DiagnosticOperation::GetFlags, flags));
    }
    let result = network::fcntl(socket, F_SETFL, (flags as u32) | network::O_NONBLOCK);
    if result < 0 {
        return Err(socket_error(DiagnosticOperation::SetFlags, result));
    }
    Ok(())
}

fn close_after_failure(socket: i32, mut error: WiiSocketError) -> WiiSocketError {
    let cleanup = network::close(socket);
    if cleanup < 0 {
        error.cleanup = Some(cleanup);
    }
    error
}

fn socket_error(operation: DiagnosticOperation, code: i32) -> WiiSocketError {
    WiiSocketError {
        operation,
        code,
        cleanup: None,
    }
}

fn is_would_block(result: i32) -> bool {
    result == EAGAIN_IOS || result == EAGAIN_POSIX
}

/// Preserve the backend operation and raw IOS result without formatting or allocation.
pub fn diagnostic_failure(error: &TransportError<WiiSocketError>) -> DiagnosticFailure {
    match error {
        TransportError::Backend { source, .. } => failure_from_socket(*source),
        TransportError::BackendAndCleanup {
            source, cleanup, ..
        } => {
            let mut failure = failure_from_socket(*source);
            failure.cleanup = failure.cleanup.or(Some(cleanup.code));
            failure
        }
        _ => DiagnosticFailure {
            operation: DiagnosticOperation::Transport,
            code: None,
            cleanup: None,
        },
    }
}

pub fn failure_from_socket(error: WiiSocketError) -> DiagnosticFailure {
    DiagnosticFailure {
        operation: error.operation,
        code: if matches!(
            error.operation,
            DiagnosticOperation::HostIp | DiagnosticOperation::Transport
        ) {
            None
        } else {
            Some(error.code)
        },
        cleanup: error.cleanup,
    }
}
