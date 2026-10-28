#![no_std]

#[cfg(test)]
extern crate std;

use core::fmt;
use core::sync::atomic::{AtomicBool, AtomicU8, AtomicU32, Ordering};

pub const DEFAULT_PORT: u16 = 2828;
mod diagnostics;
pub use diagnostics::{
    DIAGNOSTIC_QUERY, DIAGNOSTIC_REPLY_LEN, DiagnosticFailure, DiagnosticOperation,
    ListenerDiagnosticState, ListenerDiagnostics,
};

pub const MAX_PACKET_SIZE: usize = 4096;
pub const PARTIAL_PACKET_TIMEOUT_MS: u64 = 30_000;
const TX_QUEUE_SIZE: usize = MAX_PACKET_SIZE;
const MAX_REPLY_SIZE: usize = 192;
const MIN_REPLY_SPACE_BEFORE_FRAME: usize = MAX_REPLY_SIZE + 1;
const STATUS_QUERY: &[u8] = b"qHbcStatus";
const ARM_QUERY: &[u8] = b"qHbcArm";
const PAUSE_QUERY: &[u8] = b"qHbcPause";
const RESUME_QUERY: &[u8] = b"qHbcResume";
const CONTROL_QUEUE_CAPACITY: u8 = 4;
const GENERATION_MASK: u32 = 0x3fff_ffff;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AcceptStatus {
    WouldBlock,
    Interrupted,
    Accepted,
    RejectedExtraClient,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum IoStatus {
    WouldBlock,
    Interrupted,
    Bytes(usize),
    Disconnected,
}

/// Connection and explicit execution authorization state.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DebuggerState {
    Listening,
    Connected,
    Armed,
    Closed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ArmError {
    NoClient,
    Closed,
}

/// Main-thread cooperative execution state. This does not represent CPU
/// exception handling or a stopped processor.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum CooperativeState {
    Disabled = 0,
    Running = 1,
    Authorized = 2,
    PauseRequested = 3,
    Paused = 4,
    ResumeRequested = 5,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ControlRequest {
    Arm = 1,
    Pause = 2,
    Resume = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ControlQueueError {
    Disconnected,
    Unauthorized,
    InvalidState,
    Full,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MainControlResult {
    Unchanged,
    Authorized,
    EnteredPause,
    Resumed,
    PauseBusy,
    Reset,
    Unavailable,
}

/// Main-loop conditions that must all be true before HBC accepts a pause.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PauseConditions {
    pub normal_menu: bool,
    pub no_exit_pending: bool,
    pub no_install_pending: bool,
    pub no_storage_operation: bool,
    pub no_remote_operation: bool,
    pub no_network_transition: bool,
    pub no_modal_workflow: bool,
}

impl PauseConditions {
    pub const fn is_eligible(self) -> bool {
        self.normal_menu
            && self.no_exit_pending
            && self.no_install_pending
            && self.no_storage_operation
            && self.no_remote_operation
            && self.no_network_transition
            && self.no_modal_workflow
    }
}

/// Fixed-size, single-producer/single-consumer control plane.
///
/// The network worker is the only request producer. The HBC main thread is the
/// only state-machine consumer and state writer. Atomic fields provide status
/// snapshots to the worker without sharing mutable main-thread data.
pub struct CooperativeControl {
    state: AtomicU8,
    connected: AtomicBool,
    transport_ready: AtomicBool,
    worker_running: AtomicBool,
    generation: AtomicU32,
    authorized_generation: AtomicU32,
    queue_head: AtomicU8,
    queue_tail: AtomicU8,
    queue: [AtomicU32; CONTROL_QUEUE_CAPACITY as usize],
    queue_overflows: AtomicU32,
    last_result: AtomicU8,
}

impl CooperativeControl {
    pub const fn new() -> Self {
        Self {
            state: AtomicU8::new(CooperativeState::Disabled as u8),
            connected: AtomicBool::new(false),
            transport_ready: AtomicBool::new(false),
            worker_running: AtomicBool::new(false),
            generation: AtomicU32::new(0),
            authorized_generation: AtomicU32::new(GENERATION_MASK),
            queue_head: AtomicU8::new(0),
            queue_tail: AtomicU8::new(0),
            queue: [const { AtomicU32::new(0) }; CONTROL_QUEUE_CAPACITY as usize],
            queue_overflows: AtomicU32::new(0),
            last_result: AtomicU8::new(0),
        }
    }

    pub fn reset_for_worker_start(&self) {
        self.connected.store(false, Ordering::Release);
        self.transport_ready.store(false, Ordering::Release);
        self.worker_running.store(true, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
        self.authorized_generation
            .store(GENERATION_MASK, Ordering::Release);
        self.state
            .store(CooperativeState::Disabled as u8, Ordering::Release);
        self.last_result.store(0, Ordering::Release);
        self.clear_queue();
    }

    pub fn set_transport_ready(&self, ready: bool) {
        self.transport_ready.store(ready, Ordering::Release);
    }

    pub fn worker_stopped(&self) {
        self.worker_running.store(false, Ordering::Release);
        self.transport_ready.store(false, Ordering::Release);
        self.connected.store(false, Ordering::Release);
        self.generation.fetch_add(1, Ordering::AcqRel);
    }

    /// Call only from the transport worker when it accepts or closes a client.
    pub fn set_client_connected(&self, connected: bool) {
        let previous = self.connected.swap(connected, Ordering::AcqRel);
        if previous != connected {
            self.generation.fetch_add(1, Ordering::AcqRel);
        }
    }

    pub fn state(&self) -> CooperativeState {
        let connected = self.connected.load(Ordering::Acquire);
        let generation = self.generation.load(Ordering::Acquire) & GENERATION_MASK;
        let authorized_generation =
            self.authorized_generation.load(Ordering::Acquire) & GENERATION_MASK;
        let state = load_cooperative_state(self.state.load(Ordering::Acquire));
        if !connected || generation != authorized_generation {
            if matches!(
                state,
                CooperativeState::Authorized
                    | CooperativeState::PauseRequested
                    | CooperativeState::Paused
                    | CooperativeState::ResumeRequested
            ) {
                return if self.is_available() {
                    CooperativeState::Running
                } else {
                    CooperativeState::Disabled
                };
            }
        }
        state
    }

    pub fn is_available(&self) -> bool {
        self.worker_running.load(Ordering::Acquire) && self.transport_ready.load(Ordering::Acquire)
    }

    pub fn is_connected(&self) -> bool {
        self.connected.load(Ordering::Acquire)
    }

    pub fn is_authorized(&self) -> bool {
        let generation = self.generation.load(Ordering::Acquire) & GENERATION_MASK;
        self.is_authorized_for(generation)
    }

    pub fn last_result(&self) -> &'static [u8] {
        match self.last_result.load(Ordering::Acquire) {
            1 => b"authorized",
            2 => b"paused",
            3 => b"busy",
            4 => b"resumed",
            5 => b"reset",
            6 => b"worker-stopped",
            _ => b"none",
        }
    }

    pub fn queue_overflows(&self) -> u32 {
        self.queue_overflows.load(Ordering::Acquire)
    }

    /// Enqueue one validated protocol request from the network worker.
    /// `OK` from the remote protocol means the event entered this bounded
    /// queue; the main thread reports the final state through qHbcStatus.
    pub fn enqueue(&self, request: ControlRequest) -> Result<(), ControlQueueError> {
        if !self.connected.load(Ordering::Acquire) {
            return Err(ControlQueueError::Disconnected);
        }
        let state = self.state();
        match request {
            ControlRequest::Arm
                if matches!(
                    state,
                    CooperativeState::Running | CooperativeState::Authorized
                ) => {}
            ControlRequest::Pause
                if matches!(
                    state,
                    CooperativeState::Authorized
                        | CooperativeState::PauseRequested
                        | CooperativeState::Paused
                ) => {}
            ControlRequest::Resume
                if matches!(
                    state,
                    CooperativeState::PauseRequested
                        | CooperativeState::Paused
                        | CooperativeState::ResumeRequested
                ) => {}
            ControlRequest::Pause | ControlRequest::Resume => {
                return Err(ControlQueueError::Unauthorized);
            }
            ControlRequest::Arm => return Err(ControlQueueError::InvalidState),
        }

        let head = self.queue_head.load(Ordering::Relaxed);
        let tail = self.queue_tail.load(Ordering::Acquire);
        if head.wrapping_sub(tail) >= CONTROL_QUEUE_CAPACITY {
            self.queue_overflows.fetch_add(1, Ordering::Relaxed);
            return Err(ControlQueueError::Full);
        }
        let generation = self.generation.load(Ordering::Acquire) & GENERATION_MASK;
        let packed = (generation << 2) | request as u32;
        self.queue[(head % CONTROL_QUEUE_CAPACITY) as usize].store(packed, Ordering::Relaxed);
        self.queue_head
            .store(head.wrapping_add(1), Ordering::Release);
        Ok(())
    }

    /// Consume requests and apply state changes at a main-thread safe point.
    /// Reset is checked before queue events and always wins.
    pub fn service(&self, reset_pending: bool, pause_eligible: bool) -> MainControlResult {
        if reset_pending {
            self.authorized_generation
                .store(GENERATION_MASK, Ordering::Release);
            self.state
                .store(CooperativeState::Disabled as u8, Ordering::Release);
            self.last_result.store(5, Ordering::Release);
            self.clear_queue();
            return MainControlResult::Reset;
        }

        if !self.is_available() {
            self.authorized_generation
                .store(GENERATION_MASK, Ordering::Release);
            self.state
                .store(CooperativeState::Disabled as u8, Ordering::Release);
            self.last_result.store(6, Ordering::Release);
            self.clear_queue();
            return MainControlResult::Unavailable;
        }

        let current_generation = self.generation.load(Ordering::Acquire) & GENERATION_MASK;
        if !self.connected.load(Ordering::Acquire) {
            let was_paused = matches!(
                load_cooperative_state(self.state.load(Ordering::Acquire)),
                CooperativeState::Paused
                    | CooperativeState::PauseRequested
                    | CooperativeState::ResumeRequested
            );
            self.authorized_generation
                .store(GENERATION_MASK, Ordering::Release);
            self.state
                .store(CooperativeState::Running as u8, Ordering::Release);
            self.last_result
                .store(if was_paused { 4 } else { 0 }, Ordering::Release);
            self.clear_queue();
            return if was_paused {
                MainControlResult::Resumed
            } else {
                MainControlResult::Unchanged
            };
        }

        if self.authorized_generation.load(Ordering::Acquire) & GENERATION_MASK
            != current_generation
        {
            self.state
                .store(CooperativeState::Running as u8, Ordering::Release);
            self.authorized_generation
                .store(GENERATION_MASK, Ordering::Release);
        }

        let mut result = MainControlResult::Unchanged;
        while let Some((generation, request)) = self.pop_request() {
            if generation != current_generation || !self.connected.load(Ordering::Acquire) {
                continue;
            }
            let state = load_cooperative_state(self.state.load(Ordering::Acquire));
            match request {
                ControlRequest::Arm
                    if matches!(
                        state,
                        CooperativeState::Running | CooperativeState::Authorized
                    ) =>
                {
                    self.authorized_generation
                        .store(current_generation, Ordering::Release);
                    self.state
                        .store(CooperativeState::Authorized as u8, Ordering::Release);
                    result = MainControlResult::Authorized;
                    self.last_result.store(1, Ordering::Release);
                }
                ControlRequest::Pause if self.is_authorized_for(current_generation) => {
                    self.state
                        .store(CooperativeState::PauseRequested as u8, Ordering::Release);
                }
                ControlRequest::Resume if self.is_authorized_for(current_generation) => {
                    self.state
                        .store(CooperativeState::ResumeRequested as u8, Ordering::Release);
                }
                _ => {}
            }
        }

        match load_cooperative_state(self.state.load(Ordering::Acquire)) {
            CooperativeState::PauseRequested if pause_eligible => {
                self.state
                    .store(CooperativeState::Paused as u8, Ordering::Release);
                self.last_result.store(2, Ordering::Release);
                MainControlResult::EnteredPause
            }
            CooperativeState::PauseRequested => {
                self.state
                    .store(CooperativeState::Authorized as u8, Ordering::Release);
                self.last_result.store(3, Ordering::Release);
                MainControlResult::PauseBusy
            }
            CooperativeState::ResumeRequested => {
                self.state
                    .store(CooperativeState::Authorized as u8, Ordering::Release);
                self.last_result.store(4, Ordering::Release);
                MainControlResult::Resumed
            }
            CooperativeState::Paused => MainControlResult::EnteredPause,
            _ => result,
        }
    }

    pub fn local_resume(&self) -> bool {
        if load_cooperative_state(self.state.load(Ordering::Acquire)) != CooperativeState::Paused {
            return false;
        }
        self.state
            .store(CooperativeState::ResumeRequested as u8, Ordering::Release);
        true
    }

    fn is_authorized_for(&self, generation: u32) -> bool {
        self.authorized_generation.load(Ordering::Acquire) & GENERATION_MASK == generation
    }

    fn pop_request(&self) -> Option<(u32, ControlRequest)> {
        let tail = self.queue_tail.load(Ordering::Relaxed);
        let head = self.queue_head.load(Ordering::Acquire);
        if tail == head {
            return None;
        }
        let packed = self.queue[(tail % CONTROL_QUEUE_CAPACITY) as usize].load(Ordering::Relaxed);
        self.queue_tail
            .store(tail.wrapping_add(1), Ordering::Release);
        let generation = (packed >> 2) & GENERATION_MASK;
        let request = match packed as u8 & 3 {
            1 => ControlRequest::Arm,
            2 => ControlRequest::Pause,
            3 => ControlRequest::Resume,
            _ => return None,
        };
        Some((generation, request))
    }

    fn clear_queue(&self) {
        let head = self.queue_head.load(Ordering::Acquire);
        self.queue_tail.store(head, Ordering::Release);
    }
}

impl Default for CooperativeControl {
    fn default() -> Self {
        Self::new()
    }
}

fn load_cooperative_state(value: u8) -> CooperativeState {
    match value {
        1 => CooperativeState::Running,
        2 => CooperativeState::Authorized,
        3 => CooperativeState::PauseRequested,
        4 => CooperativeState::Paused,
        5 => CooperativeState::ResumeRequested,
        _ => CooperativeState::Disabled,
    }
}

/// Socket operations used by the production adapter and host tests.
///
/// `accept_one(true)` must close the newly accepted socket and return
/// `RejectedExtraClient`. Both `close_*` methods must be safe to retry after
/// an error. The engine calls them only while it owns the matching socket.
pub trait SocketBackend: Sized {
    type Error: fmt::Debug;

    fn bind(port: u16) -> Result<Self, Self::Error>;
    fn accept_one(&mut self, reject_new_client: bool) -> Result<AcceptStatus, Self::Error>;
    fn read(&mut self, buffer: &mut [u8]) -> Result<IoStatus, Self::Error>;
    fn write(&mut self, buffer: &[u8]) -> Result<IoStatus, Self::Error>;
    fn close_client(&mut self) -> Result<(), Self::Error>;
    fn close_listener(&mut self) -> Result<(), Self::Error>;
}

#[derive(Debug, Eq, PartialEq)]
pub enum TransportError<E> {
    InvalidPort,
    Closed,
    Backend {
        operation: &'static str,
        source: E,
    },
    BackendAndCleanup {
        operation: &'static str,
        source: E,
        cleanup: E,
    },
    InvalidBackendResult {
        operation: &'static str,
        value: usize,
    },
    AcknowledgementQueueFull,
}

#[derive(Debug, Eq, PartialEq)]
pub struct CloseError<E> {
    pub client: Option<E>,
    pub listener: Option<E>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PollReport {
    pub accepted_client: bool,
    pub rejected_extra_client: bool,
    pub disconnected_client: bool,
    pub timed_out_partial_packet: bool,
    pub valid_packets: u16,
    pub rejected_packets: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PacketEvent {
    Packet { size: usize },
    ChecksumMismatch { expected: u8, received: u8 },
    InvalidChecksum,
    Oversized,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum DecodeState {
    Idle,
    Payload,
    ChecksumHigh,
    ChecksumLow { high: u8 },
}

/// Incremental decoder for GDB remote packets in the form `$packet#checksum`.
///
/// The checksum covers the encoded packet bytes between `$` and `#`. The
/// decoder keeps at most `MAX_PACKET_SIZE` payload bytes and discards any
/// excess bytes until it reaches the checksum.
pub struct GdbPacketDecoder {
    state: DecodeState,
    packet: [u8; MAX_PACKET_SIZE],
    len: usize,
    checksum: u8,
    oversized: bool,
}

impl GdbPacketDecoder {
    pub const fn new() -> Self {
        Self {
            state: DecodeState::Idle,
            packet: [0; MAX_PACKET_SIZE],
            len: 0,
            checksum: 0,
            oversized: false,
        }
    }

    pub fn in_frame(&self) -> bool {
        self.state != DecodeState::Idle
    }

    pub fn packet(&self, size: usize) -> Result<&[u8], usize> {
        if size <= self.len {
            Ok(&self.packet[..size])
        } else {
            Err(self.len)
        }
    }

    pub fn reset(&mut self) {
        self.state = DecodeState::Idle;
        self.len = 0;
        self.checksum = 0;
        self.oversized = false;
    }

    pub fn push(&mut self, byte: u8) -> Option<PacketEvent> {
        match self.state {
            DecodeState::Idle => {
                if byte == b'$' {
                    self.begin_packet();
                }
                None
            }
            DecodeState::Payload => {
                if byte == b'#' {
                    self.state = DecodeState::ChecksumHigh;
                } else {
                    self.checksum = self.checksum.wrapping_add(byte);
                    if self.len < self.packet.len() {
                        self.packet[self.len] = byte;
                        self.len += 1;
                    } else {
                        self.oversized = true;
                    }
                }
                None
            }
            DecodeState::ChecksumHigh => match hex_nibble(byte) {
                Some(high) => {
                    self.state = DecodeState::ChecksumLow { high };
                    None
                }
                None => {
                    self.reset();
                    Some(PacketEvent::InvalidChecksum)
                }
            },
            DecodeState::ChecksumLow { high } => match hex_nibble(byte) {
                Some(low) => {
                    let received = (high << 4) | low;
                    let expected = self.checksum;
                    let oversized = self.oversized;
                    let size = self.len;
                    self.finish_packet();
                    if received != expected {
                        Some(PacketEvent::ChecksumMismatch { expected, received })
                    } else if oversized {
                        Some(PacketEvent::Oversized)
                    } else {
                        Some(PacketEvent::Packet { size })
                    }
                }
                None => {
                    self.reset();
                    Some(PacketEvent::InvalidChecksum)
                }
            },
        }
    }

    fn begin_packet(&mut self) {
        self.state = DecodeState::Payload;
        self.len = 0;
        self.checksum = 0;
        self.oversized = false;
    }

    fn finish_packet(&mut self) {
        self.state = DecodeState::Idle;
        self.checksum = 0;
        self.oversized = false;
    }
}

impl Default for GdbPacketDecoder {
    fn default() -> Self {
        Self::new()
    }
}

/// Owns packet framing and client state. The Wii socket adapter supplies the
/// socket calls. Tests use a deterministic backend through the same engine.
#[must_use = "close the transport and handle its cleanup result"]
pub struct GdbTransport<B: SocketBackend> {
    backend: B,
    control: &'static CooperativeControl,
    decoder: GdbPacketDecoder,
    receive_buffer: [u8; MAX_PACKET_SIZE],
    transmit_buffer: [u8; TX_QUEUE_SIZE],
    transmit_start: usize,
    transmit_len: usize,
    receive_len: usize,
    receive_pos: usize,
    partial_packet_started_ms: Option<u64>,
    client_open: bool,
    armed: bool,
    listener_open: bool,
}

impl<B: SocketBackend> GdbTransport<B> {
    pub fn bind(port: u16) -> Result<Self, TransportError<B::Error>> {
        static DEFAULT_CONTROL: CooperativeControl = CooperativeControl::new();
        Self::bind_with_control(port, &DEFAULT_CONTROL)
    }

    pub fn bind_with_control(
        port: u16,
        control: &'static CooperativeControl,
    ) -> Result<Self, TransportError<B::Error>> {
        if port == 0 {
            return Err(TransportError::InvalidPort);
        }
        let backend = B::bind(port).map_err(|source| TransportError::Backend {
            operation: "bind listener",
            source,
        })?;
        Ok(Self {
            backend,
            control,
            decoder: GdbPacketDecoder::new(),
            receive_buffer: [0; MAX_PACKET_SIZE],
            transmit_buffer: [0; TX_QUEUE_SIZE],
            transmit_start: 0,
            transmit_len: 0,
            receive_len: 0,
            receive_pos: 0,
            partial_packet_started_ms: None,
            client_open: false,
            armed: false,
            listener_open: true,
        })
    }

    pub fn client_connected(&self) -> bool {
        self.client_open
    }

    pub fn state(&self) -> DebuggerState {
        if !self.listener_open {
            DebuggerState::Closed
        } else if self.armed && self.client_open {
            DebuggerState::Armed
        } else if self.client_open {
            DebuggerState::Connected
        } else {
            DebuggerState::Listening
        }
    }

    /// Authorize a future execution-control implementation. This transition
    /// does not install handlers or perform execution control.
    pub fn arm(&mut self) -> Result<(), ArmError> {
        match self.state() {
            DebuggerState::Connected => {
                self.armed = true;
                Ok(())
            }
            DebuggerState::Listening => Err(ArmError::NoClient),
            DebuggerState::Armed => Ok(()),
            DebuggerState::Closed => Err(ArmError::Closed),
        }
    }

    pub fn disarm(&mut self) {
        self.armed = false;
    }

    /// Service at most one bounded read and up to two bounded writes. No
    /// operation waits for a peer. A partial packet times out after 30 seconds.
    pub fn poll(&mut self, now_ms: u64) -> Result<PollReport, TransportError<B::Error>> {
        if !self.listener_open {
            return Err(TransportError::Closed);
        }
        let mut report = PollReport::default();

        match self
            .backend
            .accept_one(self.client_connected())
            .map_err(|source| TransportError::Backend {
                operation: "accept client",
                source,
            })? {
            AcceptStatus::WouldBlock => {}
            AcceptStatus::Interrupted => {}
            AcceptStatus::Accepted if !self.client_connected() => {
                self.client_open = true;
                self.armed = false;
                self.control.set_client_connected(true);
                report.accepted_client = true;
            }
            AcceptStatus::RejectedExtraClient if self.client_connected() => {
                report.rejected_extra_client = true;
            }
            status => {
                return Err(TransportError::InvalidBackendResult {
                    operation: "accept client",
                    value: status as usize,
                });
            }
        }

        if !self.client_connected() {
            return Ok(report);
        }

        if self
            .partial_packet_started_ms
            .is_some_and(|started| now_ms.wrapping_sub(started) >= PARTIAL_PACKET_TIMEOUT_MS)
        {
            self.close_client("close timed-out client")?;
            report.disconnected_client = true;
            report.timed_out_partial_packet = true;
            return Ok(report);
        }

        if self.flush_transmit()? {
            report.disconnected_client = true;
            return Ok(report);
        }

        if self.receive_pos < self.receive_len {
            self.decode_pending(now_ms, &mut report)?;
        } else if TX_QUEUE_SIZE - (self.transmit_start + self.transmit_len)
            >= MIN_REPLY_SPACE_BEFORE_FRAME
        {
            let received = match self.backend.read(&mut self.receive_buffer) {
                Ok(status) => status,
                Err(source) => return Err(self.close_after_error("receive packet", source)),
            };
            match received {
                IoStatus::WouldBlock => {}
                IoStatus::Interrupted => {}
                IoStatus::Disconnected => {
                    self.close_client("close disconnected client")?;
                    report.disconnected_client = true;
                    return Ok(report);
                }
                IoStatus::Bytes(size) if size <= self.receive_buffer.len() => {
                    self.receive_pos = 0;
                    self.receive_len = size;
                    self.decode_pending(now_ms, &mut report)?;
                }
                IoStatus::Bytes(size) => {
                    return Err(TransportError::InvalidBackendResult {
                        operation: "receive packet",
                        value: size,
                    });
                }
            }
        }

        if self.flush_transmit()? {
            report.disconnected_client = true;
        }
        Ok(report)
    }

    pub fn close(&mut self) -> Result<(), CloseError<B::Error>> {
        self.armed = false;
        self.control.set_client_connected(false);
        let client = if self.client_open {
            match self.backend.close_client() {
                Ok(()) => {
                    self.client_open = false;
                    self.armed = false;
                    self.reset_client_state();
                    None
                }
                Err(error) => Some(error),
            }
        } else {
            None
        };
        let listener = if self.listener_open {
            match self.backend.close_listener() {
                Ok(()) => {
                    self.listener_open = false;
                    None
                }
                Err(error) => Some(error),
            }
        } else {
            None
        };
        if client.is_some() || listener.is_some() {
            Err(CloseError { client, listener })
        } else {
            Ok(())
        }
    }

    fn decode_pending(
        &mut self,
        now_ms: u64,
        report: &mut PollReport,
    ) -> Result<(), TransportError<B::Error>> {
        while self.receive_pos < self.receive_len {
            let free_tx = TX_QUEUE_SIZE - (self.transmit_start + self.transmit_len);
            if !self.decoder.in_frame() && free_tx < MIN_REPLY_SPACE_BEFORE_FRAME {
                break;
            }
            let index = self.receive_pos;
            self.receive_pos += 1;
            let was_in_frame = self.decoder.in_frame();
            let event = self.decoder.push(self.receive_buffer[index]);
            if !was_in_frame && self.decoder.in_frame() {
                self.partial_packet_started_ms = Some(now_ms);
            }
            if let Some(event) = event {
                self.partial_packet_started_ms = None;
                match event {
                    PacketEvent::Packet { size } => {
                        report.valid_packets = report.valid_packets.saturating_add(1);
                        self.queue_transmit(b'+')?;
                        self.handle_packet(size)?;
                    }
                    PacketEvent::ChecksumMismatch { .. }
                    | PacketEvent::InvalidChecksum
                    | PacketEvent::Oversized => {
                        report.rejected_packets = report.rejected_packets.saturating_add(1);
                        self.queue_transmit(b'-')?;
                    }
                }
            }
        }
        Ok(())
    }

    /// Return true when the client disconnected while the engine sent data.
    fn flush_transmit(&mut self) -> Result<bool, TransportError<B::Error>> {
        if self.transmit_len == 0 {
            return Ok(false);
        }
        let end = self.transmit_start + self.transmit_len;
        let sent = match self
            .backend
            .write(&self.transmit_buffer[self.transmit_start..end])
        {
            Ok(status) => status,
            Err(source) => return Err(self.close_after_error("send acknowledgement", source)),
        };
        match sent {
            IoStatus::WouldBlock => Ok(false),
            IoStatus::Interrupted => Ok(false),
            IoStatus::Disconnected | IoStatus::Bytes(0) => {
                self.close_client("close disconnected client")?;
                Ok(true)
            }
            IoStatus::Bytes(size) if size <= self.transmit_len => {
                self.transmit_start += size;
                self.transmit_len -= size;
                if self.transmit_len == 0 {
                    self.transmit_start = 0;
                }
                Ok(false)
            }
            IoStatus::Bytes(size) => Err(TransportError::InvalidBackendResult {
                operation: "send acknowledgement",
                value: size,
            }),
        }
    }

    fn queue_transmit(&mut self, byte: u8) -> Result<(), TransportError<B::Error>> {
        let end = self.transmit_start + self.transmit_len;
        if end >= self.transmit_buffer.len() {
            return Err(TransportError::AcknowledgementQueueFull);
        }
        self.transmit_buffer[end] = byte;
        self.transmit_len += 1;
        Ok(())
    }

    fn close_client(&mut self, operation: &'static str) -> Result<(), TransportError<B::Error>> {
        self.armed = false;
        self.control.set_client_connected(false);
        self.backend
            .close_client()
            .map_err(|source| TransportError::Backend { operation, source })?;
        self.client_open = false;
        self.armed = false;
        self.reset_client_state();
        Ok(())
    }

    fn close_after_error(
        &mut self,
        operation: &'static str,
        source: B::Error,
    ) -> TransportError<B::Error> {
        self.armed = false;
        self.control.set_client_connected(false);
        match self.backend.close_client() {
            Ok(()) => {
                self.client_open = false;
                self.armed = false;
                self.reset_client_state();
                TransportError::Backend { operation, source }
            }
            Err(cleanup) => TransportError::BackendAndCleanup {
                operation,
                source,
                cleanup,
            },
        }
    }

    fn reset_client_state(&mut self) {
        self.decoder.reset();
        self.transmit_start = 0;
        self.transmit_len = 0;
        self.receive_len = 0;
        self.receive_pos = 0;
        self.partial_packet_started_ms = None;
    }

    fn handle_packet(&mut self, size: usize) -> Result<(), TransportError<B::Error>> {
        let packet = self.decoder.packet(size);
        if packet.is_ok_and(|packet| packet == STATUS_QUERY) {
            let mut output = [0u8; MAX_REPLY_SIZE];
            let len = format_status(self.control, self.client_open, &mut output);
            return self.queue_response(&output[..len]);
        }

        let request = match packet {
            Ok(packet) if packet == ARM_QUERY => Some(ControlRequest::Arm),
            Ok(packet) if packet == PAUSE_QUERY => Some(ControlRequest::Pause),
            Ok(packet) if packet == RESUME_QUERY => Some(ControlRequest::Resume),
            _ => None,
        };
        if let Some(request) = request {
            let response: &[u8] = match self.control.enqueue(request) {
                Ok(()) => b"OK",
                Err(ControlQueueError::Full) => b"E02",
                Err(
                    ControlQueueError::Disconnected
                    | ControlQueueError::Unauthorized
                    | ControlQueueError::InvalidState,
                ) => b"E01",
            };
            return self.queue_response(response);
        }
        self.queue_response(&[])
    }

    fn queue_response(&mut self, payload: &[u8]) -> Result<(), TransportError<B::Error>> {
        if payload.len() > MAX_REPLY_SIZE - 4 {
            return Err(TransportError::AcknowledgementQueueFull);
        }
        let mut checksum = 0u8;
        self.queue_transmit(b'$')?;
        for byte in payload {
            checksum = checksum.wrapping_add(*byte);
            self.queue_transmit(*byte)?;
        }
        self.queue_transmit(b'#')?;
        self.queue_transmit(hex_digit(checksum >> 4))?;
        self.queue_transmit(hex_digit(checksum & 0x0f))?;
        Ok(())
    }
}

fn format_status(
    control: &CooperativeControl,
    client_open: bool,
    output: &mut [u8; MAX_REPLY_SIZE],
) -> usize {
    let mut writer = StatusWriter { output, len: 0 };
    writer.push(if client_open {
        b"transport=connected;client=present;"
    } else {
        b"transport=listening;client=none;"
    });
    writer.push(if control.is_available() {
        b"cooperative=enabled;"
    } else {
        b"cooperative=disabled;"
    });
    writer.push(b"pause=");
    writer.push(match control.state() {
        CooperativeState::PauseRequested => b"requested;" as &[u8],
        CooperativeState::Paused => b"paused;",
        CooperativeState::ResumeRequested => b"resuming;",
        CooperativeState::Disabled => b"disabled;",
        CooperativeState::Running | CooperativeState::Authorized => b"running;",
    });
    writer.push(if control.is_authorized() {
        b"authorization=armed;"
    } else {
        b"authorization=disarmed;"
    });
    writer.push(b"exception=disabled;execution=disabled;reset=normal;result=");
    writer.push(control.last_result());
    writer.len
}

struct StatusWriter<'a> {
    output: &'a mut [u8; MAX_REPLY_SIZE],
    len: usize,
}

impl StatusWriter<'_> {
    fn push(&mut self, value: &[u8]) {
        let remaining = self.output.len().saturating_sub(self.len);
        let count = value.len().min(remaining);
        self.output[self.len..self.len + count].copy_from_slice(&value[..count]);
        self.len += count;
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

fn hex_digit(nibble: u8) -> u8 {
    match nibble {
        0..=9 => b'0' + nibble,
        _ => b'a' + nibble - 10,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::boxed::Box;
    use std::collections::VecDeque;
    use std::vec;
    use std::vec::Vec;

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum FakeError {
        Accept,
        Read,
        Write,
        CloseClient,
        CloseListener,
    }

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    enum AcceptStep {
        WouldBlock,
        Interrupted,
        Connected,
    }

    struct FakeSocket {
        port: u16,
        connected: bool,
        listener_open: bool,
        accept_steps: VecDeque<AcceptStep>,
        read_steps: VecDeque<Result<Vec<u8>, IoStatus>>,
        fail_accept: bool,
        fail_read: bool,
        fail_write: bool,
        write_limit: usize,
        write_would_block: bool,
        write_interrupted: bool,
        written: Vec<u8>,
        client_closes: usize,
        extra_client_closes: usize,
        listener_closes: usize,
        fail_close_client: bool,
        fail_close_listener: bool,
    }

    impl SocketBackend for FakeSocket {
        type Error = FakeError;

        fn bind(port: u16) -> Result<Self, Self::Error> {
            Ok(Self {
                port,
                connected: false,
                listener_open: true,
                accept_steps: VecDeque::new(),
                read_steps: VecDeque::new(),
                fail_accept: false,
                fail_read: false,
                fail_write: false,
                write_limit: usize::MAX,
                write_would_block: false,
                write_interrupted: false,
                written: Vec::new(),
                client_closes: 0,
                extra_client_closes: 0,
                listener_closes: 0,
                fail_close_client: false,
                fail_close_listener: false,
            })
        }

        fn accept_one(&mut self, reject_new_client: bool) -> Result<AcceptStatus, Self::Error> {
            if self.fail_accept {
                return Err(FakeError::Accept);
            }
            match self
                .accept_steps
                .pop_front()
                .unwrap_or(AcceptStep::WouldBlock)
            {
                AcceptStep::WouldBlock => Ok(AcceptStatus::WouldBlock),
                AcceptStep::Interrupted => Ok(AcceptStatus::Interrupted),
                AcceptStep::Connected if reject_new_client => {
                    self.extra_client_closes += 1;
                    Ok(AcceptStatus::RejectedExtraClient)
                }
                AcceptStep::Connected => {
                    self.connected = true;
                    Ok(AcceptStatus::Accepted)
                }
            }
        }

        fn read(&mut self, buffer: &mut [u8]) -> Result<IoStatus, Self::Error> {
            if self.fail_read {
                return Err(FakeError::Read);
            }
            match self.read_steps.pop_front() {
                None => Ok(IoStatus::WouldBlock),
                Some(Err(status)) => Ok(status),
                Some(Ok(mut bytes)) => {
                    if bytes.len() > buffer.len() {
                        return Err(FakeError::Read);
                    }
                    let size = bytes.len();
                    buffer[..bytes.len()].copy_from_slice(&bytes);
                    bytes.clear();
                    Ok(IoStatus::Bytes(size))
                }
            }
        }

        fn write(&mut self, buffer: &[u8]) -> Result<IoStatus, Self::Error> {
            if self.fail_write {
                return Err(FakeError::Write);
            }
            if self.write_would_block {
                return Ok(IoStatus::WouldBlock);
            }
            if self.write_interrupted {
                return Ok(IoStatus::Interrupted);
            }
            let size = buffer.len().min(self.write_limit);
            self.written.extend_from_slice(&buffer[..size]);
            Ok(IoStatus::Bytes(size))
        }

        fn close_client(&mut self) -> Result<(), Self::Error> {
            if self.fail_close_client {
                return Err(FakeError::CloseClient);
            }
            if self.connected {
                self.client_closes += 1;
                self.connected = false;
            }
            Ok(())
        }

        fn close_listener(&mut self) -> Result<(), Self::Error> {
            if self.fail_close_listener {
                return Err(FakeError::CloseListener);
            }
            if self.listener_open {
                self.listener_closes += 1;
                self.listener_open = false;
            }
            Ok(())
        }
    }

    fn valid_packet(payload: &[u8]) -> Vec<u8> {
        let checksum = payload
            .iter()
            .fold(0u8, |sum, byte| sum.wrapping_add(*byte));
        let mut packet = vec![b'$'];
        packet.extend_from_slice(payload);
        packet.extend_from_slice(b"#");
        packet.push(hex_digit(checksum >> 4));
        packet.push(hex_digit(checksum & 0x0f));
        packet
    }

    fn hex_digit(nibble: u8) -> u8 {
        match nibble {
            0..=9 => b'0' + nibble,
            _ => b'a' + nibble - 10,
        }
    }

    fn connected_transport() -> GdbTransport<FakeSocket> {
        let control = Box::leak(Box::new(CooperativeControl::new()));
        control.reset_for_worker_start();
        control.set_transport_ready(true);
        let mut transport = GdbTransport::<FakeSocket>::bind_with_control(DEFAULT_PORT, control)
            .expect("bind fake socket");
        transport
            .backend
            .accept_steps
            .push_back(AcceptStep::Connected);
        let report = transport.poll(0).expect("accept fake client");
        assert!(report.accepted_client);
        assert_eq!(control.service(false, false), MainControlResult::Unchanged);
        transport
    }

    #[test]
    fn gdb_and_wiiload_ports_are_distinct() {
        assert_ne!(DEFAULT_PORT, 4299);
        let gdb = GdbTransport::<FakeSocket>::bind(DEFAULT_PORT).expect("bind GDB port");
        let wiiload = GdbTransport::<FakeSocket>::bind(4299).expect("bind Wiiload port");
        assert_eq!(gdb.backend.port, DEFAULT_PORT);
        assert_eq!(wiiload.backend.port, 4299);
    }

    #[test]
    fn packet_decoder_handles_partial_reads_and_valid_checksums() {
        let packet = valid_packet(b"qSupported");
        let split = packet.len() - 1;
        let mut decoder = GdbPacketDecoder::new();
        let mut event = None;
        for byte in &packet[..split] {
            event = decoder.push(*byte).or(event);
        }
        assert_eq!(event, None);
        assert!(decoder.in_frame());
        event = decoder.push(packet[split]);
        assert_eq!(event, Some(PacketEvent::Packet { size: 10 }));
        assert_eq!(decoder.packet(10), Ok(&b"qSupported"[..]));
    }

    #[test]
    fn packet_decoder_rejects_bad_checksum_and_non_hex_checksum() {
        let mut decoder = GdbPacketDecoder::new();
        let packet = b"$g#00";
        let mut events = Vec::new();
        for byte in packet {
            if let Some(event) = decoder.push(*byte) {
                events.push(event);
            }
        }
        assert_eq!(
            events,
            [PacketEvent::ChecksumMismatch {
                expected: b'g',
                received: 0
            }]
        );

        for byte in b"$g#x0" {
            if let Some(event) = decoder.push(*byte) {
                events.push(event);
            }
        }
        assert_eq!(events.last(), Some(&PacketEvent::InvalidChecksum));
    }

    #[test]
    fn oversized_packet_is_discarded_without_writing_past_the_buffer() {
        let mut decoder = GdbPacketDecoder::new();
        let mut checksum = 0u8;
        let mut event = None;
        assert_eq!(decoder.push(b'$'), None);
        for _ in 0..=MAX_PACKET_SIZE {
            checksum = checksum.wrapping_add(b'a');
            event = decoder.push(b'a').or(event);
        }
        assert_eq!(
            decoder.packet(MAX_PACKET_SIZE).map(|bytes| bytes.len()),
            Ok(MAX_PACKET_SIZE)
        );
        assert_eq!(decoder.push(b'#'), None);
        assert_eq!(decoder.push(hex_digit(checksum >> 4)), None);
        event = decoder.push(hex_digit(checksum & 0x0f));
        assert_eq!(event, Some(PacketEvent::Oversized));
    }

    #[test]
    fn engine_acks_valid_frames_and_nacks_invalid_frames() {
        let mut transport = connected_transport();
        let mut bytes = valid_packet(b"m80000000,4");
        bytes.extend_from_slice(b"$g#00");
        transport.backend.read_steps.push_back(Ok(bytes));
        let report = transport.poll(10).expect("process GDB frames");
        assert_eq!(report.valid_packets, 1);
        assert_eq!(report.rejected_packets, 1);
        assert_eq!(transport.backend.written, b"+$#00-");
    }

    #[test]
    fn diagnostic_query_returns_only_fixed_read_only_status() {
        let mut transport = connected_transport();
        transport
            .backend
            .read_steps
            .push_back(Ok(valid_packet(STATUS_QUERY)));
        let report = transport.poll(10).expect("answer status query");
        assert_eq!(report.valid_packets, 1);
        assert_eq!(
            transport.backend.written,
            b"+$transport=connected;client=present;cooperative=enabled;pause=running;authorization=disarmed;exception=disabled;execution=disabled;reset=normal;result=none#90"
        );
    }

    #[test]
    fn status_queries_are_drained_without_overflowing_the_bounded_reply_queue() {
        let mut transport = connected_transport();
        let packet = valid_packet(STATUS_QUERY);
        let mut input = Vec::new();
        for _ in 0..100 {
            input.extend_from_slice(&packet);
        }
        transport.backend.read_steps.push_back(Ok(input));

        let mut accepted = 0u16;
        for now in 10..40 {
            let report = transport.poll(now).expect("drain buffered status requests");
            accepted += report.valid_packets;
            if transport.receive_pos == transport.receive_len && accepted == 100 {
                break;
            }
        }

        let expected_reply = b"+$transport=connected;client=present;cooperative=enabled;pause=running;authorization=disarmed;exception=disabled;execution=disabled;reset=normal;result=none#90";
        let expected = expected_reply.repeat(100);
        assert_eq!(accepted, 100);
        assert_eq!(transport.backend.written, expected);
    }

    fn connected_control() -> CooperativeControl {
        let control = CooperativeControl::new();
        control.reset_for_worker_start();
        control.set_transport_ready(true);
        control.set_client_connected(true);
        assert_eq!(control.service(false, false), MainControlResult::Unchanged);
        control
    }

    fn authorize(control: &CooperativeControl) {
        control.enqueue(ControlRequest::Arm).unwrap();
        assert_eq!(control.service(false, false), MainControlResult::Authorized);
        assert_eq!(control.state(), CooperativeState::Authorized);
    }

    #[test]
    fn cooperative_pause_resume_cycles_return_to_authorized_running_state() {
        let control = connected_control();
        authorize(&control);
        for _ in 0..8 {
            control.enqueue(ControlRequest::Pause).unwrap();
            assert_eq!(
                control.service(false, true),
                MainControlResult::EnteredPause
            );
            assert_eq!(control.state(), CooperativeState::Paused);
            control.enqueue(ControlRequest::Resume).unwrap();
            assert_eq!(control.service(false, true), MainControlResult::Resumed);
            assert_eq!(control.state(), CooperativeState::Authorized);
        }
    }

    #[test]
    fn duplicate_arm_pause_and_resume_requests_are_idempotent() {
        let control = connected_control();
        control.enqueue(ControlRequest::Arm).unwrap();
        control.enqueue(ControlRequest::Arm).unwrap();
        assert_eq!(control.service(false, false), MainControlResult::Authorized);
        assert_eq!(control.state(), CooperativeState::Authorized);

        control.enqueue(ControlRequest::Pause).unwrap();
        control.enqueue(ControlRequest::Pause).unwrap();
        assert_eq!(
            control.service(false, true),
            MainControlResult::EnteredPause
        );
        assert_eq!(control.state(), CooperativeState::Paused);

        control.enqueue(ControlRequest::Resume).unwrap();
        control.enqueue(ControlRequest::Resume).unwrap();
        assert_eq!(control.service(false, true), MainControlResult::Resumed);
        assert_eq!(control.state(), CooperativeState::Authorized);
    }

    #[test]
    fn pause_request_at_an_unsafe_safe_point_is_rejected_with_busy_state() {
        let control = connected_control();
        authorize(&control);
        control.enqueue(ControlRequest::Pause).unwrap();
        assert_eq!(control.service(false, false), MainControlResult::PauseBusy);
        assert_eq!(control.state(), CooperativeState::Authorized);
        assert_eq!(control.last_result(), b"busy");
    }

    #[test]
    fn pause_conditions_reject_each_active_operation() {
        let eligible = PauseConditions {
            normal_menu: true,
            no_exit_pending: true,
            no_install_pending: true,
            no_storage_operation: true,
            no_remote_operation: true,
            no_network_transition: true,
            no_modal_workflow: true,
        };
        assert!(eligible.is_eligible());
        let unsafe_conditions = [
            PauseConditions {
                normal_menu: false,
                ..eligible
            },
            PauseConditions {
                no_exit_pending: false,
                ..eligible
            },
            PauseConditions {
                no_install_pending: false,
                ..eligible
            },
            PauseConditions {
                no_storage_operation: false,
                ..eligible
            },
            PauseConditions {
                no_remote_operation: false,
                ..eligible
            },
            PauseConditions {
                no_network_transition: false,
                ..eligible
            },
            PauseConditions {
                no_modal_workflow: false,
                ..eligible
            },
        ];
        assert!(
            unsafe_conditions
                .into_iter()
                .all(|conditions| !conditions.is_eligible())
        );
    }

    #[test]
    fn disconnect_clears_authorization_and_resumes_a_paused_main_thread() {
        let control = connected_control();
        authorize(&control);
        control.enqueue(ControlRequest::Pause).unwrap();
        assert_eq!(
            control.service(false, true),
            MainControlResult::EnteredPause
        );
        control.set_client_connected(false);
        assert_eq!(control.service(false, true), MainControlResult::Resumed);
        assert_eq!(control.state(), CooperativeState::Running);
        assert!(!control.is_authorized());
    }

    #[test]
    fn reset_wins_over_queued_resume_and_disconnect_regardless_of_order() {
        let control = connected_control();
        authorize(&control);
        control.enqueue(ControlRequest::Pause).unwrap();
        assert_eq!(
            control.service(false, true),
            MainControlResult::EnteredPause
        );
        control.enqueue(ControlRequest::Resume).unwrap();
        control.set_client_connected(false);
        assert_eq!(control.service(true, true), MainControlResult::Reset);
        assert_eq!(control.state(), CooperativeState::Disabled);
        assert_eq!(control.last_result(), b"reset");

        control.set_client_connected(true);
        assert_eq!(control.service(false, false), MainControlResult::Unchanged);
        assert_eq!(control.state(), CooperativeState::Running);
        assert!(!control.is_authorized());
    }

    #[test]
    fn full_control_queue_rejects_new_events_without_replacing_existing_events() {
        let control = connected_control();
        for _ in 0..CONTROL_QUEUE_CAPACITY {
            control.enqueue(ControlRequest::Arm).unwrap();
        }
        assert_eq!(
            control.enqueue(ControlRequest::Arm),
            Err(ControlQueueError::Full)
        );
        assert_eq!(control.queue_overflows(), 1);
        assert_eq!(control.service(false, false), MainControlResult::Authorized);
        assert_eq!(control.state(), CooperativeState::Authorized);
    }

    #[test]
    fn worker_failure_while_paused_is_detected_without_a_queue_event() {
        let control = connected_control();
        authorize(&control);
        control.enqueue(ControlRequest::Pause).unwrap();
        assert_eq!(
            control.service(false, true),
            MainControlResult::EnteredPause
        );
        control.worker_stopped();
        assert_eq!(control.service(false, true), MainControlResult::Unavailable);
        assert_eq!(control.state(), CooperativeState::Disabled);
        assert!(!control.is_authorized());
    }

    #[test]
    fn repeated_worker_initialization_clears_previous_authorization_and_events() {
        let control = connected_control();
        authorize(&control);
        control.enqueue(ControlRequest::Pause).unwrap();
        control.worker_stopped();
        control.reset_for_worker_start();
        control.set_transport_ready(true);
        control.set_client_connected(true);
        assert_eq!(control.service(false, true), MainControlResult::Unchanged);
        assert_eq!(control.state(), CooperativeState::Running);
        assert!(!control.is_authorized());
    }

    #[test]
    fn local_resume_changes_only_a_paused_state() {
        let control = connected_control();
        authorize(&control);
        assert!(!control.local_resume());
        control.enqueue(ControlRequest::Pause).unwrap();
        assert_eq!(
            control.service(false, true),
            MainControlResult::EnteredPause
        );
        assert!(control.local_resume());
        assert_eq!(control.service(false, true), MainControlResult::Resumed);
    }

    #[test]
    fn custom_commands_require_authorization_and_unknown_commands_stay_inert() {
        let mut transport = connected_transport();
        transport.backend.written.clear();
        let mut input = valid_packet(PAUSE_QUERY);
        input.extend_from_slice(&valid_packet(b"M80000000,1:ff"));
        transport.backend.read_steps.push_back(Ok(input));
        let report = transport.poll(10).unwrap();
        assert_eq!(report.valid_packets, 2);
        assert_eq!(transport.backend.written, b"+$E01#a6+$#00");
        assert_eq!(transport.control.state(), CooperativeState::Running);
    }

    #[test]
    fn connection_does_not_arm_and_disconnect_removes_authorization() {
        let mut transport = GdbTransport::<FakeSocket>::bind(DEFAULT_PORT).unwrap();
        assert_eq!(transport.state(), DebuggerState::Listening);
        assert_eq!(transport.arm(), Err(ArmError::NoClient));

        transport
            .backend
            .accept_steps
            .push_back(AcceptStep::Connected);
        transport.poll(0).unwrap();
        assert_eq!(transport.state(), DebuggerState::Connected);
        assert!(!transport.armed);
        transport.arm().unwrap();
        assert_eq!(transport.state(), DebuggerState::Armed);

        transport
            .backend
            .read_steps
            .push_back(Err(IoStatus::Disconnected));
        transport.poll(1).unwrap();
        assert_eq!(transport.state(), DebuggerState::Listening);
        assert!(!transport.armed);
        assert_eq!(transport.arm(), Err(ArmError::NoClient));
    }

    #[test]
    fn unknown_gdb_command_gets_empty_reply_without_execution() {
        let mut transport = connected_transport();
        transport
            .backend
            .read_steps
            .push_back(Ok(valid_packet(b"M80000000,1:ff")));
        transport.poll(10).expect("reject unsupported command");
        assert_eq!(transport.backend.written, b"+$#00");
    }

    #[test]
    fn frame_split_across_transport_polls_is_acked_after_completion() {
        let mut transport = connected_transport();
        let packet = valid_packet(b"qAttached");
        let split = packet.len() - 2;
        transport
            .backend
            .read_steps
            .push_back(Ok(packet[..split].to_vec()));
        transport
            .backend
            .read_steps
            .push_back(Ok(packet[split..].to_vec()));

        let partial = transport.poll(10).expect("read partial frame");
        assert_eq!(partial.valid_packets, 0);
        assert!(transport.backend.written.is_empty());

        let complete = transport.poll(11).expect("complete frame");
        assert_eq!(complete.valid_packets, 1);
        assert_eq!(transport.backend.written, b"+$#00");
    }

    #[test]
    fn partial_writes_keep_unsent_acknowledgements_for_the_next_poll() {
        let mut transport = connected_transport();
        transport.backend.write_limit = 1;
        let mut bytes = valid_packet(b"qAttached");
        bytes.extend_from_slice(&valid_packet(b"qOffsets"));
        transport.backend.read_steps.push_back(Ok(bytes));
        transport.poll(10).expect("queue acknowledgements");
        assert_eq!(transport.backend.written, b"+");
        for now in 11..120 {
            transport.poll(now).expect("flush queued replies");
        }
        assert_eq!(transport.backend.written, b"+$#00+$#00");
    }

    #[test]
    fn would_block_and_disconnect_return_without_waiting() {
        let mut transport = connected_transport();
        transport
            .backend
            .read_steps
            .push_back(Err(IoStatus::WouldBlock));
        assert_eq!(transport.poll(10).unwrap(), PollReport::default());
        transport
            .backend
            .read_steps
            .push_back(Err(IoStatus::Disconnected));
        let report = transport.poll(11).unwrap();
        assert!(report.disconnected_client);
        assert!(!transport.client_connected());
        assert_eq!(transport.backend.client_closes, 1);
    }

    #[test]
    fn interrupted_accept_read_and_write_are_retried_on_later_polls() {
        let mut transport = GdbTransport::<FakeSocket>::bind(DEFAULT_PORT).unwrap();
        transport
            .backend
            .accept_steps
            .push_back(AcceptStep::Interrupted);
        assert_eq!(transport.poll(0).unwrap(), PollReport::default());
        transport
            .backend
            .accept_steps
            .push_back(AcceptStep::Connected);
        assert!(transport.poll(1).unwrap().accepted_client);

        transport
            .backend
            .read_steps
            .push_back(Err(IoStatus::Interrupted));
        assert_eq!(transport.poll(2).unwrap(), PollReport::default());

        transport
            .backend
            .read_steps
            .push_back(Ok(valid_packet(b"?")));
        transport.backend.write_interrupted = true;
        transport.poll(3).unwrap();
        assert!(transport.backend.written.is_empty());
        transport.backend.write_interrupted = false;
        transport.poll(4).unwrap();
        assert_eq!(transport.backend.written, b"+$#00");
    }

    #[test]
    fn second_client_is_rejected_and_does_not_replace_the_first() {
        let mut transport = connected_transport();
        transport
            .backend
            .accept_steps
            .push_back(AcceptStep::Connected);
        let report = transport.poll(10).unwrap();
        assert!(report.rejected_extra_client);
        assert!(transport.client_connected());
        assert_eq!(transport.backend.client_closes, 0);
        assert_eq!(transport.backend.extra_client_closes, 1);
    }

    #[test]
    fn partial_packet_times_out_and_closes_the_client() {
        let mut transport = connected_transport();
        transport
            .backend
            .read_steps
            .push_back(Ok(b"$qSupport".to_vec()));
        transport.poll(10).unwrap();
        let report = transport.poll(10 + PARTIAL_PACKET_TIMEOUT_MS).unwrap();
        assert!(report.timed_out_partial_packet);
        assert!(!transport.client_connected());
        assert_eq!(transport.backend.client_closes, 1);
    }

    #[test]
    fn oversized_frame_spanning_reads_is_rejected_without_buffer_growth() {
        let mut transport = connected_transport();
        let payload = vec![b'a'; MAX_PACKET_SIZE + 1];
        let packet = valid_packet(&payload);
        transport
            .backend
            .read_steps
            .push_back(Ok(packet[..MAX_PACKET_SIZE].to_vec()));
        transport
            .backend
            .read_steps
            .push_back(Ok(packet[MAX_PACKET_SIZE..].to_vec()));
        let first = transport.poll(10).unwrap();
        assert_eq!(first.rejected_packets, 0);
        let second = transport.poll(11).unwrap();
        assert_eq!(second.rejected_packets, 1);
        assert_eq!(second.valid_packets, 0);
        assert_eq!(transport.backend.written, b"-");
    }

    #[test]
    fn backend_failures_are_returned_and_close_the_active_client() {
        let mut accept_transport = GdbTransport::<FakeSocket>::bind(DEFAULT_PORT).unwrap();
        accept_transport.backend.fail_accept = true;
        assert_eq!(
            accept_transport.poll(0),
            Err(TransportError::Backend {
                operation: "accept client",
                source: FakeError::Accept,
            })
        );

        let mut read_transport = connected_transport();
        read_transport.backend.fail_read = true;
        assert_eq!(
            read_transport.poll(0),
            Err(TransportError::Backend {
                operation: "receive packet",
                source: FakeError::Read,
            })
        );
        assert!(!read_transport.client_connected());
        assert_eq!(read_transport.backend.client_closes, 1);

        let mut write_transport = connected_transport();
        write_transport
            .backend
            .read_steps
            .push_back(Ok(valid_packet(b"?")));
        write_transport.backend.fail_write = true;
        assert_eq!(
            write_transport.poll(0),
            Err(TransportError::Backend {
                operation: "send acknowledgement",
                source: FakeError::Write,
            })
        );
        assert!(!write_transport.client_connected());
        assert_eq!(write_transport.backend.client_closes, 1);
    }

    #[test]
    fn repeated_bind_close_cycles_do_not_double_close_sockets() {
        for _ in 0..3 {
            let mut transport = GdbTransport::<FakeSocket>::bind(DEFAULT_PORT).unwrap();
            transport
                .backend
                .accept_steps
                .push_back(AcceptStep::Connected);
            transport.poll(0).unwrap();
            transport.close().unwrap();
            assert_eq!(transport.backend.client_closes, 1);
            assert_eq!(transport.backend.listener_closes, 1);
            transport.close().unwrap();
            assert_eq!(transport.backend.client_closes, 1);
            assert_eq!(transport.backend.listener_closes, 1);
        }
    }

    #[test]
    fn failed_close_keeps_socket_state_for_retry_and_reports_both_failures() {
        let mut transport = connected_transport();
        transport.arm().unwrap();
        transport.backend.fail_close_client = true;
        transport.backend.fail_close_listener = true;
        assert_eq!(
            transport.close(),
            Err(CloseError {
                client: Some(FakeError::CloseClient),
                listener: Some(FakeError::CloseListener),
            })
        );
        assert!(transport.client_connected());
        assert_eq!(transport.state(), DebuggerState::Connected);
        assert!(transport.listener_open);
        transport.backend.fail_close_client = false;
        transport.backend.fail_close_listener = false;
        transport.close().unwrap();
        assert!(!transport.client_connected());
        assert!(!transport.listener_open);
    }
}
