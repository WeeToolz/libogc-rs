//! Private Ethernet/ARP/IPv4/TCP stack for the libdb debugger transport.
//!
//! The C reference carries a private uIP copy. Reproducing that entire generic
//! stack would make review difficult, so this module implements only the
//! server-side protocol surface used by libdb: ARP, IPv4, and a listening TCP
//! endpoint with ordered byte-stream I/O. The public `tcpip_*` ABI uses this
//! module directly; the C uIP implementation remains reference-only.

use alloc::collections::VecDeque;
use alloc::vec::Vec;
use core::cmp;

use ogc_lwp_driver::{TICKS_PER_SECOND, now_ticks};

use crate::bba::{BBA_RX_MAX_PACKET_SIZE, BBA_TX_MAX_PACKET_SIZE, BbaDevice, BbaError};

const ETH_HEADER_LEN: usize = 14;
const ETH_TYPE_IPV4: u16 = 0x0800;
const ETH_TYPE_ARP: u16 = 0x0806;
const ETH_BROADCAST: [u8; 6] = [0xff; 6];

const ARP_PACKET_LEN: usize = 28;
const ARP_HW_ETHERNET: u16 = 1;
const ARP_OP_REQUEST: u16 = 1;
const ARP_OP_REPLY: u16 = 2;
const ARP_TABLE_SIZE: usize = 8;
const ARP_TIMER_TICKS: u64 = TICKS_PER_SECOND * 5;
const ARP_MAX_AGE: u16 = 240;

const IPV4_HEADER_LEN: usize = 20;
const IP_PROTO_TCP: u8 = 6;
const IP_FLAG_DF: u16 = 0x4000;
const IP_FRAGMENT_MASK: u16 = 0x3fff;
const IP_TTL: u8 = 255;

const TCP_HEADER_LEN: usize = 20;
const TCP_FLAG_FIN: u8 = 0x01;
const TCP_FLAG_SYN: u8 = 0x02;
const TCP_FLAG_RST: u8 = 0x04;
const TCP_FLAG_PSH: u8 = 0x08;
const TCP_FLAG_ACK: u8 = 0x10;
const TCP_MSS: usize = 1460;
const TCP_WINDOW: u16 = 4 * TCP_MSS as u16;
const TCP_INITIAL_SEQUENCE: u32 = 6510;
const TCP_RTO_TICKS: u64 = TICKS_PER_SECOND * 3;
const TCP_MAX_RETRIES: u8 = 12;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) struct Ipv4Config {
    pub local: [u8; 4],
    pub netmask: [u8; 4],
    pub gateway: [u8; 4],
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub(crate) enum NetError {
    Device(BbaError),
    NotConnected,
    WouldBlock,
    PacketTooLarge,
    ConnectionReset,
}

impl From<BbaError> for NetError {
    fn from(value: BbaError) -> Self {
        Self::Device(value)
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct ArpEntry {
    ip: [u8; 4],
    mac: [u8; 6],
    age: u16,
    valid: bool,
}

impl ArpEntry {
    const EMPTY: Self = Self {
        ip: [0; 4],
        mac: [0; 6],
        age: 0,
        valid: false,
    };
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
enum TcpState {
    SynReceived,
    Established,
    FinWait,
    CloseWait,
    LastAck,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingSegment {
    seq: u32,
    flags: u8,
    payload: Vec<u8>,
    sent_at: u64,
    retries: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct TcpConnection {
    state: TcpState,
    remote_ip: [u8; 4],
    remote_mac: [u8; 6],
    local_port: u16,
    remote_port: u16,
    snd_una: u32,
    snd_nxt: u32,
    rcv_nxt: u32,
    peer_window: u16,
    peer_mss: usize,
    receive: VecDeque<u8>,
    pending: Option<PendingSegment>,
    close_requested: bool,
}

impl TcpConnection {
    fn new_syn_received(
        remote_ip: [u8; 4],
        remote_mac: [u8; 6],
        local_port: u16,
        remote_port: u16,
        remote_seq: u32,
        peer_window: u16,
        peer_mss: usize,
    ) -> Self {
        Self {
            state: TcpState::SynReceived,
            remote_ip,
            remote_mac,
            local_port,
            remote_port,
            snd_una: TCP_INITIAL_SEQUENCE,
            snd_nxt: TCP_INITIAL_SEQUENCE.wrapping_add(1),
            rcv_nxt: remote_seq.wrapping_add(1),
            peer_window,
            peer_mss: peer_mss.clamp(1, TCP_MSS),
            receive: VecDeque::new(),
            pending: Some(PendingSegment {
                seq: TCP_INITIAL_SEQUENCE,
                flags: TCP_FLAG_SYN | TCP_FLAG_ACK,
                payload: Vec::new(),
                sent_at: now_ticks(),
                retries: 0,
            }),
            close_requested: false,
        }
    }

    fn matches(&self, source_ip: [u8; 4], source_port: u16, destination_port: u16) -> bool {
        self.remote_ip == source_ip
            && self.remote_port == source_port
            && self.local_port == destination_port
    }

    fn acknowledge(&mut self, ack: u32) {
        if seq_after(ack, self.snd_una) && !seq_after(ack, self.snd_nxt) {
            self.snd_una = ack;
        }
        if let Some(pending) = self.pending.as_ref() {
            let consumed = pending.payload.len() as u32
                + u32::from(pending.flags & (TCP_FLAG_SYN | TCP_FLAG_FIN) != 0);
            let end = pending.seq.wrapping_add(consumed);
            if !seq_before(ack, end) {
                self.pending = None;
            }
        }
    }
}

#[derive(Clone, Copy)]
struct TcpPacket<'a> {
    source_port: u16,
    destination_port: u16,
    seq: u32,
    ack: u32,
    flags: u8,
    window: u16,
    payload: &'a [u8],
    peer_mss: usize,
}

#[derive(Clone, Copy)]
struct TcpSegment<'a> {
    remote_mac: [u8; 6],
    remote_ip: [u8; 4],
    source_port: u16,
    destination_port: u16,
    seq: u32,
    ack: u32,
    flags: u8,
    payload: &'a [u8],
    options: &'a [u8],
}

pub(crate) struct PrivateNet {
    device: BbaDevice,
    config: Ipv4Config,
    arp: [ArpEntry; ARP_TABLE_SIZE],
    arp_timer_at: u64,
    listen_port: Option<u16>,
    accepted: bool,
    connection: Option<TcpConnection>,
    frame: [u8; BBA_RX_MAX_PACKET_SIZE],
    ip_id: u16,
}

impl PrivateNet {
    pub(crate) fn initialize(config: Ipv4Config) -> Result<Self, NetError> {
        Ok(Self {
            device: BbaDevice::initialize()?,
            config,
            arp: [ArpEntry::EMPTY; ARP_TABLE_SIZE],
            arp_timer_at: now_ticks(),
            listen_port: None,
            accepted: false,
            connection: None,
            frame: [0; BBA_RX_MAX_PACKET_SIZE],
            ip_id: 0,
        })
    }

    pub(crate) fn listen(&mut self, port: u16) {
        self.listen_port = Some(port);
    }

    pub(crate) fn stop_listen(&mut self) {
        self.listen_port = None;
    }

    pub(crate) fn is_accepted(&self) -> bool {
        self.accepted
    }

    pub(crate) fn has_receive_data(&self) -> bool {
        self.connection
            .as_ref()
            .is_some_and(|connection| !connection.receive.is_empty())
    }

    pub(crate) fn poll(&mut self) -> Result<(), NetError> {
        self.age_arp_table();
        self.retransmit_if_needed()?;
        if let Some(len) = self.device.poll_frame(&mut self.frame)? {
            self.process_ethernet(len)?;
        }
        Ok(())
    }

    pub(crate) fn read(&mut self, output: &mut [u8]) -> Result<usize, NetError> {
        let Some(connection) = self.connection.as_mut() else {
            return Err(NetError::NotConnected);
        };
        if !matches!(
            connection.state,
            TcpState::Established | TcpState::CloseWait
        ) {
            return Err(NetError::NotConnected);
        }
        if connection.receive.is_empty() {
            return Err(NetError::WouldBlock);
        }
        let count = cmp::min(output.len(), connection.receive.len());
        for byte in &mut output[..count] {
            *byte = connection.receive.pop_front().unwrap_or(0);
        }
        Ok(count)
    }

    pub(crate) fn write(&mut self, input: &[u8]) -> Result<usize, NetError> {
        let Some(connection) = self.connection.as_ref() else {
            return Err(NetError::NotConnected);
        };
        if connection.state != TcpState::Established {
            return Err(NetError::NotConnected);
        }
        if connection.pending.is_some() {
            return Err(NetError::WouldBlock);
        }
        if input.is_empty() {
            return Ok(0);
        }

        if connection.peer_window == 0 {
            return Err(NetError::WouldBlock);
        }
        let count = cmp::min(
            cmp::min(input.len(), connection.peer_mss),
            usize::from(connection.peer_window),
        );
        let payload = input[..count].to_vec();
        let seq = connection.snd_nxt;
        let flags = TCP_FLAG_ACK | TCP_FLAG_PSH;
        self.send_connection_segment(seq, flags, &payload, &[])?;

        let connection = self.connection.as_mut().ok_or(NetError::NotConnected)?;
        connection.snd_nxt = connection.snd_nxt.wrapping_add(count as u32);
        connection.pending = Some(PendingSegment {
            seq,
            flags,
            payload,
            sent_at: now_ticks(),
            retries: 0,
        });
        Ok(count)
    }

    pub(crate) fn close(&mut self) -> Result<(), NetError> {
        let Some(connection) = self.connection.as_mut() else {
            self.accepted = false;
            return Ok(());
        };
        if connection.pending.is_some() {
            connection.close_requested = true;
            self.accepted = false;
            return Ok(());
        }
        self.start_close()
    }

    fn start_close(&mut self) -> Result<(), NetError> {
        let Some(connection) = self.connection.as_ref() else {
            self.accepted = false;
            return Ok(());
        };
        let state = connection.state;
        if !matches!(state, TcpState::Established | TcpState::CloseWait) {
            self.accepted = false;
            return Ok(());
        }
        let seq = connection.snd_nxt;
        self.send_connection_segment(seq, TCP_FLAG_FIN | TCP_FLAG_ACK, &[], &[])?;
        let connection = self.connection.as_mut().ok_or(NetError::NotConnected)?;
        connection.snd_nxt = connection.snd_nxt.wrapping_add(1);
        connection.state = if state == TcpState::Established {
            TcpState::FinWait
        } else {
            TcpState::LastAck
        };
        connection.close_requested = false;
        connection.pending = Some(PendingSegment {
            seq,
            flags: TCP_FLAG_FIN | TCP_FLAG_ACK,
            payload: Vec::new(),
            sent_at: now_ticks(),
            retries: 0,
        });
        self.accepted = false;
        Ok(())
    }

    fn process_ethernet(&mut self, len: usize) -> Result<(), NetError> {
        if len < ETH_HEADER_LEN {
            return Ok(());
        }
        let destination = array6(&self.frame[0..6]);
        if destination != self.device.mac_address() && destination != ETH_BROADCAST {
            return Ok(());
        }
        let source_mac = array6(&self.frame[6..12]);
        let ethertype = be16(&self.frame[12..14]);
        match ethertype {
            ETH_TYPE_ARP => self.process_arp(source_mac, len),
            ETH_TYPE_IPV4 => self.process_ipv4(source_mac, len),
            _ => Ok(()),
        }
    }

    fn process_arp(&mut self, _source_mac: [u8; 6], len: usize) -> Result<(), NetError> {
        if len < ETH_HEADER_LEN + ARP_PACKET_LEN {
            return Ok(());
        }
        let packet = &self.frame[ETH_HEADER_LEN..ETH_HEADER_LEN + ARP_PACKET_LEN];
        if be16(&packet[0..2]) != ARP_HW_ETHERNET
            || be16(&packet[2..4]) != ETH_TYPE_IPV4
            || packet[4] != 6
            || packet[5] != 4
        {
            return Ok(());
        }
        let operation = be16(&packet[6..8]);
        let sender_mac = array6(&packet[8..14]);
        let sender_ip = array4(&packet[14..18]);
        let target_ip = array4(&packet[24..28]);
        self.learn_arp(sender_ip, sender_mac);

        if operation == ARP_OP_REQUEST && target_ip == self.config.local {
            let local_mac = self.device.mac_address();
            let mut reply = [0u8; ETH_HEADER_LEN + ARP_PACKET_LEN];
            reply[0..6].copy_from_slice(&sender_mac);
            reply[6..12].copy_from_slice(&local_mac);
            put_be16(&mut reply[12..14], ETH_TYPE_ARP);
            put_be16(&mut reply[14..16], ARP_HW_ETHERNET);
            put_be16(&mut reply[16..18], ETH_TYPE_IPV4);
            reply[18] = 6;
            reply[19] = 4;
            put_be16(&mut reply[20..22], ARP_OP_REPLY);
            reply[22..28].copy_from_slice(&local_mac);
            reply[28..32].copy_from_slice(&self.config.local);
            reply[32..38].copy_from_slice(&sender_mac);
            reply[38..42].copy_from_slice(&sender_ip);
            self.device.send_frame(&reply)?;
        }
        Ok(())
    }

    fn process_ipv4(&mut self, source_mac: [u8; 6], len: usize) -> Result<(), NetError> {
        if len < ETH_HEADER_LEN + IPV4_HEADER_LEN {
            return Ok(());
        }
        let packet = &self.frame[ETH_HEADER_LEN..len];
        let version = packet[0] >> 4;
        let header_len = usize::from(packet[0] & 0x0f) * 4;
        if version != 4 || header_len < IPV4_HEADER_LEN || header_len > packet.len() {
            return Ok(());
        }
        let total_len = usize::from(be16(&packet[2..4]));
        if total_len < header_len || total_len > packet.len() {
            return Ok(());
        }
        if internet_checksum(&packet[..header_len]) != 0 {
            return Ok(());
        }
        if be16(&packet[6..8]) & IP_FRAGMENT_MASK != 0 {
            // The private libdb uIP enables reassembly. This compact server
            // intentionally does not accept fragmented TCP datagrams yet, so
            // the public backend must remain blocked until integration review.
            return Ok(());
        }
        let source_ip = array4(&packet[12..16]);
        let destination_ip = array4(&packet[16..20]);
        if !self.is_local_destination(destination_ip) {
            return Ok(());
        }
        let tcp = (packet[9] == IP_PROTO_TCP).then(|| packet[header_len..total_len].to_vec());
        if same_subnet(source_ip, self.config.local, self.config.netmask) {
            self.learn_arp(source_ip, source_mac);
        }
        if let Some(tcp) = tcp {
            self.process_tcp(source_mac, source_ip, &tcp)?;
        }
        Ok(())
    }

    fn process_tcp(
        &mut self,
        source_mac: [u8; 6],
        source_ip: [u8; 4],
        bytes: &[u8],
    ) -> Result<(), NetError> {
        let Some(packet) = parse_tcp(bytes) else {
            return Ok(());
        };
        if tcp_checksum(source_ip, self.config.local, bytes) != 0 {
            return Ok(());
        }

        if packet.flags & TCP_FLAG_RST != 0 {
            if self.connection.as_ref().is_some_and(|connection| {
                connection.matches(source_ip, packet.source_port, packet.destination_port)
            }) {
                self.connection = None;
                self.accepted = false;
                return Err(NetError::ConnectionReset);
            }
            return Ok(());
        }

        if let Some(connection) = self.connection.as_ref()
            && connection.matches(source_ip, packet.source_port, packet.destination_port)
        {
            return self.process_connected_tcp(packet);
        }

        let Some(listen_port) = self.listen_port else {
            return Ok(());
        };
        if packet.destination_port != listen_port || packet.flags & TCP_FLAG_SYN == 0 {
            if packet.flags & TCP_FLAG_ACK == 0 {
                self.send_reset(source_mac, source_ip, packet)?;
            }
            return Ok(());
        }

        let connection = TcpConnection::new_syn_received(
            source_ip,
            source_mac,
            listen_port,
            packet.source_port,
            packet.seq,
            packet.window,
            packet.peer_mss,
        );
        self.connection = Some(connection);
        self.accepted = false;
        self.send_connection_segment(
            TCP_INITIAL_SEQUENCE,
            TCP_FLAG_SYN | TCP_FLAG_ACK,
            &[],
            &[2, 4, (TCP_MSS >> 8) as u8, TCP_MSS as u8],
        )
    }

    fn process_connected_tcp(&mut self, packet: TcpPacket<'_>) -> Result<(), NetError> {
        let mut send_ack = false;
        let mut close_after_ack = false;
        {
            let connection = self.connection.as_mut().ok_or(NetError::NotConnected)?;
            connection.peer_window = packet.window;
            if packet.flags & TCP_FLAG_ACK != 0 {
                connection.acknowledge(packet.ack);
                match connection.state {
                    TcpState::SynReceived if packet.ack == connection.snd_nxt => {
                        connection.state = TcpState::Established;
                        self.accepted = true;
                    }
                    TcpState::FinWait if connection.pending.is_none() => {
                        if packet.flags & TCP_FLAG_FIN == 0 {
                            // Match the useful part of FIN_WAIT_2 without
                            // carrying a separate public socket state.
                        }
                    }
                    TcpState::LastAck if connection.pending.is_none() => {
                        close_after_ack = true;
                    }
                    _ => {}
                }
            }

            let sequence_matches = packet.seq == connection.rcv_nxt;
            if sequence_matches && !packet.payload.is_empty() {
                connection.receive.extend(packet.payload.iter().copied());
                connection.rcv_nxt = connection.rcv_nxt.wrapping_add(packet.payload.len() as u32);
                send_ack = true;
            } else if !packet.payload.is_empty() {
                send_ack = true;
            }

            if sequence_matches && packet.flags & TCP_FLAG_FIN != 0 {
                connection.rcv_nxt = connection.rcv_nxt.wrapping_add(1);
                send_ack = true;
                match connection.state {
                    TcpState::Established => connection.state = TcpState::CloseWait,
                    TcpState::FinWait => close_after_ack = true,
                    _ => {}
                }
            }
        }

        if send_ack {
            let seq = self
                .connection
                .as_ref()
                .ok_or(NetError::NotConnected)?
                .snd_nxt;
            self.send_connection_segment(seq, TCP_FLAG_ACK, &[], &[])?;
        }
        if close_after_ack {
            self.connection = None;
            self.accepted = false;
            return Ok(());
        }
        let should_start_close = self.connection.as_ref().is_some_and(|connection| {
            connection.close_requested
                && connection.pending.is_none()
                && matches!(
                    connection.state,
                    TcpState::Established | TcpState::CloseWait
                )
        });
        if should_start_close {
            self.start_close()?;
        }
        Ok(())
    }

    fn send_reset(
        &mut self,
        remote_mac: [u8; 6],
        remote_ip: [u8; 4],
        packet: TcpPacket<'_>,
    ) -> Result<(), NetError> {
        let tcp_len = packet.payload.len() as u32
            + u32::from(packet.flags & TCP_FLAG_SYN != 0)
            + u32::from(packet.flags & TCP_FLAG_FIN != 0);
        let (seq, ack, flags) = if packet.flags & TCP_FLAG_ACK != 0 {
            (packet.ack, 0, TCP_FLAG_RST)
        } else {
            (
                0,
                packet.seq.wrapping_add(tcp_len),
                TCP_FLAG_RST | TCP_FLAG_ACK,
            )
        };
        self.send_tcp_frame(TcpSegment {
            remote_mac,
            remote_ip,
            source_port: packet.destination_port,
            destination_port: packet.source_port,
            seq,
            ack,
            flags,
            payload: &[],
            options: &[],
        })
    }

    fn send_connection_segment(
        &mut self,
        seq: u32,
        flags: u8,
        payload: &[u8],
        options: &[u8],
    ) -> Result<(), NetError> {
        let (remote_mac, remote_ip, local_port, remote_port, ack) = {
            let connection = self.connection.as_ref().ok_or(NetError::NotConnected)?;
            (
                connection.remote_mac,
                connection.remote_ip,
                connection.local_port,
                connection.remote_port,
                connection.rcv_nxt,
            )
        };
        self.send_tcp_frame(TcpSegment {
            remote_mac,
            remote_ip,
            source_port: local_port,
            destination_port: remote_port,
            seq,
            ack,
            flags,
            payload,
            options,
        })
    }

    fn send_tcp_frame(&mut self, segment: TcpSegment<'_>) -> Result<(), NetError> {
        let TcpSegment {
            remote_mac,
            remote_ip,
            source_port,
            destination_port,
            seq,
            ack,
            flags,
            payload,
            options,
        } = segment;
        if options.len() % 4 != 0 {
            return Err(NetError::PacketTooLarge);
        }
        let tcp_header_len = TCP_HEADER_LEN + options.len();
        let ip_len = IPV4_HEADER_LEN + tcp_header_len + payload.len();
        let frame_len = ETH_HEADER_LEN + ip_len;
        if frame_len > BBA_TX_MAX_PACKET_SIZE {
            return Err(NetError::PacketTooLarge);
        }

        let mut frame = [0u8; BBA_TX_MAX_PACKET_SIZE];
        let local_mac = self.device.mac_address();
        frame[0..6].copy_from_slice(&remote_mac);
        frame[6..12].copy_from_slice(&local_mac);
        put_be16(&mut frame[12..14], ETH_TYPE_IPV4);

        let ip = &mut frame[ETH_HEADER_LEN..ETH_HEADER_LEN + IPV4_HEADER_LEN];
        ip[0] = 0x45;
        ip[1] = 0;
        put_be16(&mut ip[2..4], ip_len as u16);
        put_be16(&mut ip[4..6], self.ip_id);
        self.ip_id = self.ip_id.wrapping_add(1);
        put_be16(&mut ip[6..8], IP_FLAG_DF);
        ip[8] = IP_TTL;
        ip[9] = IP_PROTO_TCP;
        ip[10] = 0;
        ip[11] = 0;
        ip[12..16].copy_from_slice(&self.config.local);
        ip[16..20].copy_from_slice(&remote_ip);
        let ip_checksum = internet_checksum(ip);
        put_be16(&mut ip[10..12], ip_checksum);

        let tcp_start = ETH_HEADER_LEN + IPV4_HEADER_LEN;
        let tcp_end = tcp_start + tcp_header_len + payload.len();
        let tcp = &mut frame[tcp_start..tcp_end];
        put_be16(&mut tcp[0..2], source_port);
        put_be16(&mut tcp[2..4], destination_port);
        put_be32(&mut tcp[4..8], seq);
        put_be32(&mut tcp[8..12], ack);
        tcp[12] = ((tcp_header_len / 4) as u8) << 4;
        tcp[13] = flags;
        put_be16(&mut tcp[14..16], TCP_WINDOW);
        tcp[16] = 0;
        tcp[17] = 0;
        tcp[18] = 0;
        tcp[19] = 0;
        tcp[TCP_HEADER_LEN..tcp_header_len].copy_from_slice(options);
        tcp[tcp_header_len..].copy_from_slice(payload);
        let checksum = tcp_checksum(self.config.local, remote_ip, tcp);
        put_be16(&mut tcp[16..18], checksum);

        self.device.send_frame(&frame[..frame_len])?;
        Ok(())
    }

    fn retransmit_if_needed(&mut self) -> Result<(), NetError> {
        let Some(connection) = self.connection.as_ref() else {
            return Ok(());
        };
        let Some(pending) = connection.pending.as_ref() else {
            return Ok(());
        };
        let now = now_ticks();
        if now.wrapping_sub(pending.sent_at) < TCP_RTO_TICKS {
            return Ok(());
        }
        if pending.retries >= TCP_MAX_RETRIES {
            self.connection = None;
            self.accepted = false;
            return Err(NetError::ConnectionReset);
        }

        let seq = pending.seq;
        let flags = pending.flags;
        let payload = pending.payload.clone();
        let syn_options = [2u8, 4, (TCP_MSS >> 8) as u8, TCP_MSS as u8];
        let options: &[u8] = if flags & TCP_FLAG_SYN != 0 {
            &syn_options
        } else {
            &[]
        };
        self.send_connection_segment(seq, flags, &payload, options)?;
        let pending = self
            .connection
            .as_mut()
            .and_then(|connection| connection.pending.as_mut())
            .ok_or(NetError::NotConnected)?;
        pending.sent_at = now;
        pending.retries = pending.retries.saturating_add(1);
        Ok(())
    }

    fn learn_arp(&mut self, ip: [u8; 4], mac: [u8; 6]) {
        if ip == [0; 4] || ip == [255; 4] || ip[0] & 0xf0 == 0xe0 {
            return;
        }
        if let Some(entry) = self
            .arp
            .iter_mut()
            .find(|entry| entry.valid && entry.ip == ip)
        {
            entry.mac = mac;
            entry.age = 0;
            return;
        }
        if let Some(entry) = self.arp.iter_mut().find(|entry| !entry.valid) {
            *entry = ArpEntry {
                ip,
                mac,
                age: 0,
                valid: true,
            };
            return;
        }
        let oldest = self
            .arp
            .iter()
            .enumerate()
            .max_by_key(|(_, entry)| entry.age)
            .map(|(index, _)| index)
            .unwrap_or(0);
        self.arp[oldest] = ArpEntry {
            ip,
            mac,
            age: 0,
            valid: true,
        };
    }

    fn age_arp_table(&mut self) {
        let now = now_ticks();
        while now.wrapping_sub(self.arp_timer_at) >= ARP_TIMER_TICKS {
            self.arp_timer_at = self.arp_timer_at.wrapping_add(ARP_TIMER_TICKS);
            for entry in &mut self.arp {
                if entry.valid {
                    entry.age = entry.age.saturating_add(1);
                    if entry.age >= ARP_MAX_AGE {
                        *entry = ArpEntry::EMPTY;
                    }
                }
            }
        }
    }

    fn is_local_destination(&self, address: [u8; 4]) -> bool {
        is_local_destination(self.config, address)
    }
}

fn same_subnet(left: [u8; 4], right: [u8; 4], netmask: [u8; 4]) -> bool {
    (0..4).all(|index| left[index] & netmask[index] == right[index] & netmask[index])
}

fn is_local_destination(config: Ipv4Config, address: [u8; 4]) -> bool {
    if address == config.local || address == [255; 4] || address == [0; 4] {
        return true;
    }
    let mut same_network = true;
    let mut host_bits_all_ones = true;
    for index in 0..4 {
        if address[index] & config.netmask[index] != config.local[index] & config.netmask[index] {
            same_network = false;
        }
        if address[index] & !config.netmask[index] != !config.netmask[index] {
            host_bits_all_ones = false;
        }
    }
    same_network && host_bits_all_ones
}

fn parse_tcp(bytes: &[u8]) -> Option<TcpPacket<'_>> {
    if bytes.len() < TCP_HEADER_LEN {
        return None;
    }
    let header_len = usize::from(bytes[12] >> 4) * 4;
    if header_len < TCP_HEADER_LEN || header_len > bytes.len() {
        return None;
    }
    Some(TcpPacket {
        source_port: be16(&bytes[0..2]),
        destination_port: be16(&bytes[2..4]),
        seq: be32(&bytes[4..8]),
        ack: be32(&bytes[8..12]),
        flags: bytes[13],
        window: be16(&bytes[14..16]),
        payload: &bytes[header_len..],
        peer_mss: parse_mss(&bytes[TCP_HEADER_LEN..header_len]).unwrap_or(TCP_MSS),
    })
}

fn parse_mss(mut options: &[u8]) -> Option<usize> {
    while !options.is_empty() {
        match options[0] {
            0 => return None,
            1 => options = &options[1..],
            2 => {
                if options.len() < 4 || options[1] != 4 {
                    return None;
                }
                return Some(usize::from(be16(&options[2..4])));
            }
            _ => {
                if options.len() < 2 {
                    return None;
                }
                let len = usize::from(options[1]);
                if len < 2 || len > options.len() {
                    return None;
                }
                options = &options[len..];
            }
        }
    }
    None
}

fn internet_checksum(bytes: &[u8]) -> u16 {
    finalize_checksum(checksum_sum(bytes, 0))
}

fn tcp_checksum(source: [u8; 4], destination: [u8; 4], tcp: &[u8]) -> u16 {
    let mut sum = 0u32;
    sum = checksum_sum(&source, sum);
    sum = checksum_sum(&destination, sum);
    sum = sum.wrapping_add(IP_PROTO_TCP as u32);
    sum = sum.wrapping_add(tcp.len() as u32);
    finalize_checksum(checksum_sum(tcp, sum))
}

fn checksum_sum(bytes: &[u8], mut sum: u32) -> u32 {
    let (chunks, remainder) = bytes.as_chunks::<2>();
    for word in chunks {
        sum = sum.wrapping_add(u32::from(be16(word)));
    }
    if let Some(&last) = remainder.first() {
        sum = sum.wrapping_add(u32::from(last) << 8);
    }
    sum
}

fn finalize_checksum(mut sum: u32) -> u16 {
    while sum >> 16 != 0 {
        sum = (sum & 0xffff) + (sum >> 16);
    }
    !(sum as u16)
}

fn seq_before(left: u32, right: u32) -> bool {
    (left.wrapping_sub(right) as i32) < 0
}

fn seq_after(left: u32, right: u32) -> bool {
    seq_before(right, left)
}

fn be16(bytes: &[u8]) -> u16 {
    u16::from_be_bytes([bytes[0], bytes[1]])
}

fn be32(bytes: &[u8]) -> u32 {
    u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn put_be16(bytes: &mut [u8], value: u16) {
    bytes[..2].copy_from_slice(&value.to_be_bytes());
}

fn put_be32(bytes: &mut [u8], value: u32) {
    bytes[..4].copy_from_slice(&value.to_be_bytes());
}

fn array4(bytes: &[u8]) -> [u8; 4] {
    [bytes[0], bytes[1], bytes[2], bytes[3]]
}

fn array6(bytes: &[u8]) -> [u8; 6] {
    [bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5]]
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    #[test]
    fn ipv4_checksum_matches_rfc_style_vector() {
        let header = [
            0x45, 0x00, 0x00, 0x73, 0x00, 0x00, 0x40, 0x00, 0x40, 0x11, 0xb8, 0x61, 0xc0, 0xa8,
            0x00, 0x01, 0xc0, 0xa8, 0x00, 0xc7,
        ];
        assert_eq!(internet_checksum(&header), 0);
    }

    #[test]
    fn tcp_parser_reads_mss_and_payload() {
        let mut packet = [0u8; 27];
        put_be16(&mut packet[0..2], 1234);
        put_be16(&mut packet[2..4], 2828);
        put_be32(&mut packet[4..8], 10);
        put_be32(&mut packet[8..12], 20);
        packet[12] = 6 << 4;
        packet[13] = TCP_FLAG_SYN;
        put_be16(&mut packet[14..16], 4096);
        packet[20..24].copy_from_slice(&[2, 4, 0x05, 0xb4]);
        packet[24..27].copy_from_slice(b"gdb");
        let parsed = parse_tcp(&packet).unwrap();
        assert_eq!(parsed.source_port, 1234);
        assert_eq!(parsed.destination_port, 2828);
        assert_eq!(parsed.peer_mss, 1460);
        assert_eq!(parsed.payload, b"gdb");
    }

    #[test]
    fn sequence_comparison_wraps_like_tcp() {
        assert!(seq_after(1, u32::MAX));
        assert!(seq_before(u32::MAX, 1));
        assert!(!seq_after(5, 5));
    }

    #[test]
    fn subnet_broadcast_detection_matches_ipv4_mask() {
        let config = Ipv4Config {
            local: [192, 168, 1, 10],
            netmask: [255, 255, 255, 0],
            gateway: [192, 168, 1, 1],
        };
        assert!(is_local_destination(config, [192, 168, 1, 255]));
        assert!(!is_local_destination(config, [192, 168, 2, 255]));
    }
}
