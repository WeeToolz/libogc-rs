//! Bluetooth HCI, L2CAP, and HIDP wire formats.
use core::ffi::CStr;

pub const INTERNAL_BT_PATH_BYTES: &[u8; 21] = b"/dev/usb/oh1/57e/305\0";
pub const INTERNAL_BT_PATH: &CStr =
    unsafe { CStr::from_bytes_with_nul_unchecked(INTERNAL_BT_PATH_BYTES) };
pub const NINTENDO_BT_VID: u16 = 0x057e;
pub const NINTENDO_BT_PID: u16 = 0x0305;

pub const IOCTL_USB_CTRLMSG: i32 = 0;
pub const IOCTL_USB_BLKMSG: i32 = 1;
pub const IOCTL_USB_INTRMSG: i32 = 2;

pub const DEFAULT_ACL_OUT_ENDPOINT: u8 = 0x02;
pub const DEFAULT_EVENT_IN_ENDPOINT: u8 = 0x81;
pub const DEFAULT_ACL_IN_ENDPOINT: u8 = 0x82;
pub const MAX_EVENT_PACKET_BYTES: usize = 260;
pub const MAX_INQUIRY_RESULTS: usize = 8;

pub const L2CAP_PSM_SDP: u16 = 0x0001;
pub const L2CAP_PSM_HID_CTRL: u16 = 0x0011;
pub const L2CAP_PSM_HID_INTR: u16 = 0x0013;
pub const L2CAP_CID_SIGNALING: u16 = 0x0001;
pub const L2CAP_MIN_DYNAMIC_CID: u16 = 0x0040;
pub const L2CAP_HID_CTRL_LOCAL_CID: u16 = 0x0040;
pub const L2CAP_HID_INTR_LOCAL_CID: u16 = 0x0041;

pub const L2CAP_ACL_CONT: u8 = 0x01;
pub const L2CAP_ACL_START: u8 = 0x02;

pub const L2CAP_CMD_REJECT: u8 = 0x01;
pub const L2CAP_CMD_NOT_UNDERSTOOD: u16 = 0x0000;
pub const L2CAP_INVALID_CID: u16 = 0x0002;
pub const L2CAP_NULL_CID: u16 = 0x0000;
pub const L2CAP_ECHO_REQ: u8 = 0x08;
pub const L2CAP_ECHO_RSP: u8 = 0x09;
pub const L2CAP_CONN_REQ: u8 = 0x02;
pub const L2CAP_CONN_RSP: u8 = 0x03;
pub const L2CAP_CFG_REQ: u8 = 0x04;
pub const L2CAP_CFG_RSP: u8 = 0x05;
pub const L2CAP_DISCONN_REQ: u8 = 0x06;
pub const L2CAP_DISCONN_RSP: u8 = 0x07;

pub const L2CAP_CONN_SUCCESS: u16 = 0x0000;
pub const L2CAP_CONN_PENDING: u16 = 0x0001;
pub const L2CAP_CFG_SUCCESS: u16 = 0x0000;
pub const L2CAP_CFG_UNACCEPT: u16 = 0x0001;
pub const L2CAP_CFG_REJ: u16 = 0x0002;
pub const L2CAP_CFG_UNKNOWN: u16 = 0x0003;
pub const L2CAP_CONFIG_OPTION_MTU: u8 = 0x01;
pub const L2CAP_HID_RECOMMENDED_MTU: u16 = 64;

pub const HIDP_HDR_TRANS_MASK: u8 = 0xf0;
pub const HIDP_HDR_PARAM_MASK: u8 = 0x0f;
pub const HIDP_TRANS_HANDSHAKE: u8 = 0x00;
pub const HIDP_TRANS_HIDCONTROL: u8 = 0x10;
pub const HIDP_TRANS_GETREPORT: u8 = 0x40;
pub const HIDP_TRANS_SETREPORT: u8 = 0x50;
pub const HIDP_TRANS_GETPROTOCOL: u8 = 0x60;
pub const HIDP_TRANS_SETPROTOCOL: u8 = 0x70;
pub const HIDP_TRANS_GETIDLE: u8 = 0x80;
pub const HIDP_TRANS_SETIDLE: u8 = 0x90;
pub const HIDP_TRANS_DATA: u8 = 0xa0;
pub const HIDP_TRANS_DATAC: u8 = 0xb0;
pub const HIDP_DATA_RTYPE_OTHER: u8 = 0x00;
pub const HIDP_DATA_RTYPE_INPUT: u8 = 0x01;
pub const HIDP_DATA_RTYPE_OUTPUT: u8 = 0x02;
pub const HIDP_DATA_RTYPE_FEATURE: u8 = 0x03;

pub const HCI_EVENT_INQUIRY_COMPLETE: u8 = 0x01;
pub const HCI_EVENT_INQUIRY_RESULT: u8 = 0x02;
pub const HCI_EVENT_CONN_COMPLETE: u8 = 0x03;
pub const HCI_EVENT_CONNECTION_REQUEST: u8 = 0x04;
pub const HCI_EVENT_DISCONN_COMPLETE: u8 = 0x05;
pub const HCI_EVENT_REMOTE_NAME_COMPLETE: u8 = 0x07;
pub const HCI_EVENT_COMMAND_COMPLETE: u8 = 0x0e;
pub const HCI_EVENT_COMMAND_STATUS: u8 = 0x0f;
pub const HCI_EVENT_VENDOR_SPECIFIC: u8 = 0xff;
pub const HCI_EVENT_PIN_CODE_REQUEST: u8 = 0x16;
pub const HCI_EVENT_LINK_KEY_REQUEST: u8 = 0x17;
pub const HCI_EVENT_LINK_KEY_NOTIFICATION: u8 = 0x18;
pub const HCI_EVENT_INQUIRY_RESULT_WITH_RSSI: u8 = 0x22;

pub const CLASS_MAJOR_MISC: u8 = 0x00;
pub const CLASS_MAJOR_COMPUTER: u8 = 0x01;
pub const CLASS_MAJOR_PHONE: u8 = 0x02;
pub const CLASS_MAJOR_LAN: u8 = 0x03;
pub const CLASS_MAJOR_AUDIO_VIDEO: u8 = 0x04;
pub const CLASS_MAJOR_PERIPHERAL: u8 = 0x05;
pub const CLASS_MAJOR_IMAGING: u8 = 0x06;
pub const CLASS_MAJOR_WEARABLE: u8 = 0x07;
pub const CLASS_MAJOR_TOY: u8 = 0x08;
pub const CLASS_MAJOR_HEALTH: u8 = 0x09;

pub const INQUIRY_GIAC: [u8; 3] = [0x33, 0x8b, 0x9e];

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
/// Bluetooth device address in controller/HCI byte order.
pub struct BluetoothAddress {
    pub bytes: [u8; 6],
}

impl BluetoothAddress {
    pub const fn new(bytes: [u8; 6]) -> Self {
        Self { bytes }
    }

    pub const fn as_raw(&self) -> [u8; 6] {
        self.bytes
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
/// Three-byte Bluetooth class-of-device value.
pub struct ClassOfDevice {
    pub bytes: [u8; 3],
}

impl ClassOfDevice {
    pub const fn new(bytes: [u8; 3]) -> Self {
        Self { bytes }
    }

    pub const fn major(self) -> u8 {
        // Major Device Class occupies bits 8..12 of the 24-bit wire value.
        self.bytes[1] & 0x1f
    }

    pub const fn minor(self) -> u8 {
        (self.bytes[0] >> 2) & 0x3f
    }

    pub const fn likely_game_controller(self) -> bool {
        self.major() == CLASS_MAJOR_PERIPHERAL
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
/// Device discovered by an HCI inquiry.
pub struct InquiryResult {
    pub address: BluetoothAddress,
    pub class_of_device: ClassOfDevice,
    pub page_scan_rep_mode: u8,
    pub page_scan_mode: u8,
    pub clock_offset: u16,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(transparent)]
/// Encoded HCI opcode split by Bluetooth into OGF and OCF fields.
pub struct HciOpcode(u16);

impl HciOpcode {
    pub const fn new(ogf: u16, ocf: u16) -> Self {
        Self((ogf << 10) | (ocf & 0x03ff))
    }

    pub const fn raw(self) -> u16 {
        self.0
    }

    pub const RESET: Self = Self::new(0x03, 0x0003);
    pub const INQUIRY: Self = Self::new(0x01, 0x0001);
    pub const PERIODIC_INQUIRY: Self = Self::new(0x01, 0x0003);
    pub const EXIT_PERIODIC_INQUIRY: Self = Self::new(0x01, 0x0004);
    pub const CREATE_CONNECTION: Self = Self::new(0x01, 0x0005);
    pub const DISCONNECT: Self = Self::new(0x01, 0x0006);
    pub const ACCEPT_CONNECTION_REQUEST: Self = Self::new(0x01, 0x0009);
    pub const REJECT_CONNECTION_REQUEST: Self = Self::new(0x01, 0x000a);
    pub const PIN_CODE_REQUEST_REPLY: Self = Self::new(0x01, 0x000d);
    pub const PIN_CODE_REQUEST_NEG_REPLY: Self = Self::new(0x01, 0x000e);
    pub const AUTH_REQUESTED: Self = Self::new(0x01, 0x0011);
    pub const WRITE_LINK_POLICY: Self = Self::new(0x02, 0x000d);
    pub const REMOTE_NAME_REQUEST: Self = Self::new(0x01, 0x0019);
    pub const SET_EVENT_MASK: Self = Self::new(0x03, 0x0001);
    pub const SET_EVENT_FILTER: Self = Self::new(0x03, 0x0005);
    pub const WRITE_SCAN_ENABLE: Self = Self::new(0x03, 0x001a);
    pub const WRITE_AUTH_ENABLE: Self = Self::new(0x03, 0x0020);
    pub const LINK_KEY_REQUEST_REPLY: Self = Self::new(0x01, 0x000b);
    pub const LINK_KEY_REQUEST_NEG_REPLY: Self = Self::new(0x01, 0x000c);
    pub const READ_BD_ADDR: Self = Self::new(0x04, 0x0009);
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
/// HCI inquiry/page scan mode.
pub enum ScanEnable {
    Off = 0x00,
    Inquiry = 0x01,
    Page = 0x02,
    InquiryAndPage = 0x03,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
/// Errors returned by Bluetooth host helpers and packet parsers.
pub enum Error {
    Ipc(i32),
    BufferTooSmall,
    UnexpectedEvent,
    UnexpectedOpcode,
    CommandFailed(u8),
    MalformedEvent,
    EventBufferTooSmall,
    MalformedAcl,
    MalformedL2cap,
    InvalidEndpoint,
}

/// Endpoint addresses for a USB Bluetooth HCI controller.
///
/// The values are USB endpoint addresses, including the direction bit. This
/// validates address shape and HCI endpoint roles. Descriptor transfer types
/// still require inspection by the USB discovery layer.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct HciEndpointAddresses {
    pub acl_out: u8,
    pub event_in: u8,
    pub acl_in: u8,
}

impl HciEndpointAddresses {
    pub const fn new(acl_out: u8, event_in: u8, acl_in: u8) -> Result<Self> {
        let valid_out = acl_out & 0x80 == 0 && acl_out & 0x70 == 0 && acl_out & 0x0f != 0;
        let valid_event_in = event_in & 0x80 != 0 && event_in & 0x70 == 0 && event_in & 0x0f != 0;
        let valid_acl_in = acl_in & 0x80 != 0 && acl_in & 0x70 == 0 && acl_in & 0x0f != 0;
        if !(valid_out && valid_event_in && valid_acl_in) || event_in == acl_in {
            return Err(Error::InvalidEndpoint);
        }
        Ok(Self {
            acl_out,
            event_in,
            acl_in,
        })
    }
}

pub type Result<T> = core::result::Result<T, Error>;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
/// Parsed HCI event packet.
pub enum HciEvent<'a> {
    CommandComplete {
        opcode: HciOpcode,
        status: u8,
        params: &'a [u8],
    },
    CommandStatus {
        opcode: HciOpcode,
        status: u8,
    },
    InquiryComplete {
        status: u8,
    },
    InquiryResults {
        count: usize,
        results: [InquiryResult; MAX_INQUIRY_RESULTS],
    },
    ConnectionComplete {
        status: u8,
        address: BluetoothAddress,
        handle: u16,
        link_type: u8,
        encryption: u8,
    },
    ConnectionRequest {
        address: BluetoothAddress,
        class_of_device: [u8; 3],
        link_type: u8,
    },
    DisconnectionComplete {
        status: u8,
        handle: u16,
        reason: u8,
    },
    RemoteNameComplete {
        status: u8,
        address: BluetoothAddress,
        name: &'a [u8],
    },
    PinCodeRequest {
        address: BluetoothAddress,
    },
    LinkKeyRequest {
        address: BluetoothAddress,
    },
    LinkKeyNotification {
        address: BluetoothAddress,
        key: [u8; 16],
        key_type: u8,
    },
    Unknown {
        event_code: u8,
        payload: &'a [u8],
    },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
/// Parsed ACL packet header and payload.
pub struct AclPacket<'a> {
    pub handle: u16,
    pub packet_boundary: u8,
    pub broadcast: u8,
    pub payload: &'a [u8],
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
/// Parsed L2CAP packet header and payload.
pub struct L2capPacket<'a> {
    pub length: u16,
    pub channel_id: u16,
    pub payload: &'a [u8],
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
/// Parsed L2CAP signaling command.
pub struct L2capSignal<'a> {
    pub code: u8,
    pub identifier: u8,
    pub payload: &'a [u8],
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
/// Decoded payload for a known L2CAP signaling command.
pub enum L2capSignalPayload<'a> {
    ConnectionRequest {
        psm: u16,
        source_cid: u16,
    },
    ConnectionResponse {
        dest_cid: u16,
        source_cid: u16,
        result: u16,
        status: u16,
    },
    ConfigurationRequest {
        dest_cid: u16,
        flags: u16,
        options: &'a [u8],
    },
    ConfigurationResponse {
        source_cid: u16,
        flags: u16,
        result: u16,
        options: &'a [u8],
    },
    DisconnectionRequest {
        dest_cid: u16,
        source_cid: u16,
    },
    DisconnectionResponse {
        dest_cid: u16,
        source_cid: u16,
    },
    Unknown {
        code: u8,
        payload: &'a [u8],
    },
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
/// Parsed Bluetooth HIDP packet.
pub struct HidpPacket<'a> {
    pub transaction: u8,
    pub parameter: u8,
    pub payload: &'a [u8],
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
/// Local state for one outbound L2CAP HID channel.
pub struct L2capHidChannelState {
    pub psm: u16,
    pub local_cid: u16,
    pub remote_cid: u16,
    pub active: bool,
    pub connected: bool,
    pub local_configured: bool,
    pub remote_configured: bool,
}

impl L2capHidChannelState {
    pub const fn new(psm: u16, local_cid: u16) -> Self {
        Self {
            psm,
            local_cid,
            remote_cid: 0,
            active: false,
            connected: false,
            local_configured: false,
            remote_configured: false,
        }
    }

    pub const fn ready(self) -> bool {
        self.active
            && self.connected
            && self.local_configured
            && self.remote_configured
            && self.remote_cid != 0
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
/// Tracks the two L2CAP HID channels used by Bluetooth HID controllers.
pub struct HidL2capState {
    pub control: L2capHidChannelState,
    pub interrupt: L2capHidChannelState,
}

impl HidL2capState {
    pub const fn new() -> Self {
        Self {
            control: L2capHidChannelState::new(L2CAP_PSM_HID_CTRL, L2CAP_HID_CTRL_LOCAL_CID),
            interrupt: L2capHidChannelState::new(L2CAP_PSM_HID_INTR, L2CAP_HID_INTR_LOCAL_CID),
        }
    }

    pub const fn ready(self) -> bool {
        self.control.ready() && self.interrupt.ready()
    }

    pub(crate) fn local_channel_is_active(&self, local_cid: u16) -> bool {
        (self.control.local_cid == local_cid && self.control.active)
            || (self.interrupt.local_cid == local_cid && self.interrupt.active)
    }

    pub(crate) fn close_local_channel(&mut self, local_cid: u16) -> bool {
        let channel = if self.control.local_cid == local_cid {
            &mut self.control
        } else if self.interrupt.local_cid == local_cid {
            &mut self.interrupt
        } else {
            return false;
        };
        *channel = L2capHidChannelState::new(channel.psm, channel.local_cid);
        true
    }

    pub fn observe_signal(&mut self, signal: L2capSignal<'_>) -> Result<()> {
        match parse_l2cap_signal_payload(signal)? {
            L2capSignalPayload::ConnectionResponse {
                dest_cid,
                source_cid,
                result,
                ..
            } => {
                if result == L2CAP_CONN_SUCCESS
                    && let Some(channel) = self.channel_by_local_mut(source_cid)
                {
                    channel.remote_cid = dest_cid;
                    channel.active = true;
                    channel.connected = true;
                }
            }
            L2capSignalPayload::ConfigurationRequest {
                dest_cid, flags, ..
            } => {
                // BUG FIX: C marks the peer's configuration complete only
                // after the final segment clears the continuation flag.
                if flags & 1 == 0
                    && let Some(channel) = self.channel_by_local_mut(dest_cid)
                {
                    channel.remote_configured = true;
                }
            }
            L2capSignalPayload::ConfigurationResponse {
                source_cid,
                result: L2CAP_CFG_SUCCESS,
                ..
            } => {
                // BUG FIX: The Source CID in a peer's Configuration Response
                // is that peer's CID, stored here as `remote_cid`.
                if let Some(channel) = self.channel_by_remote_mut(source_cid) {
                    channel.local_configured = true;
                }
            }
            _ => {}
        }
        Ok(())
    }

    fn channel_by_local_mut(&mut self, local_cid: u16) -> Option<&mut L2capHidChannelState> {
        if self.control.local_cid == local_cid {
            Some(&mut self.control)
        } else if self.interrupt.local_cid == local_cid {
            Some(&mut self.interrupt)
        } else {
            None
        }
    }

    fn channel_by_remote_mut(&mut self, remote_cid: u16) -> Option<&mut L2capHidChannelState> {
        if self.control.remote_cid == remote_cid {
            Some(&mut self.control)
        } else if self.interrupt.remote_cid == remote_cid {
            Some(&mut self.interrupt)
        } else {
            None
        }
    }
}

impl Default for HidL2capState {
    fn default() -> Self {
        Self::new()
    }
}

pub fn parse_acl_packet(buf: &[u8]) -> Result<AclPacket<'_>> {
    if buf.len() < 4 {
        return Err(Error::MalformedAcl);
    }
    let flags_handle = u16::from_le_bytes([buf[0], buf[1]]);
    let data_len = u16::from_le_bytes([buf[2], buf[3]]) as usize;
    if buf.len() < 4 + data_len {
        return Err(Error::MalformedAcl);
    }
    Ok(AclPacket {
        handle: flags_handle & 0x0fff,
        packet_boundary: ((flags_handle >> 12) & 0x03) as u8,
        broadcast: ((flags_handle >> 14) & 0x03) as u8,
        payload: &buf[4..4 + data_len],
    })
}

pub fn parse_l2cap_packet(buf: &[u8]) -> Result<L2capPacket<'_>> {
    if buf.len() < 4 {
        return Err(Error::MalformedL2cap);
    }
    let length = u16::from_le_bytes([buf[0], buf[1]]);
    let channel_id = u16::from_le_bytes([buf[2], buf[3]]);
    let payload_len = length as usize;
    if buf.len() < 4 + payload_len {
        return Err(Error::MalformedL2cap);
    }
    Ok(L2capPacket {
        length,
        channel_id,
        payload: &buf[4..4 + payload_len],
    })
}

pub fn parse_l2cap_signal(buf: &[u8]) -> Result<L2capSignal<'_>> {
    if buf.len() < 4 {
        return Err(Error::MalformedL2cap);
    }
    let length = u16::from_le_bytes([buf[2], buf[3]]) as usize;
    if buf.len() < 4 + length {
        return Err(Error::MalformedL2cap);
    }
    Ok(L2capSignal {
        code: buf[0],
        identifier: buf[1],
        payload: &buf[4..4 + length],
    })
}

pub fn parse_next_l2cap_signal<'a>(buf: &mut &'a [u8]) -> Result<Option<L2capSignal<'a>>> {
    if buf.is_empty() {
        return Ok(None);
    }
    let signal = parse_l2cap_signal(buf)?;
    let consumed = 4 + signal.payload.len();
    *buf = &buf[consumed..];
    Ok(Some(signal))
}

pub fn parse_l2cap_signal_payload(signal: L2capSignal<'_>) -> Result<L2capSignalPayload<'_>> {
    let payload = signal.payload;
    match signal.code {
        L2CAP_CONN_REQ => {
            if payload.len() < 4 {
                return Err(Error::MalformedL2cap);
            }
            Ok(L2capSignalPayload::ConnectionRequest {
                psm: u16::from_le_bytes([payload[0], payload[1]]),
                source_cid: u16::from_le_bytes([payload[2], payload[3]]),
            })
        }
        L2CAP_CONN_RSP => {
            if payload.len() < 8 {
                return Err(Error::MalformedL2cap);
            }
            Ok(L2capSignalPayload::ConnectionResponse {
                dest_cid: u16::from_le_bytes([payload[0], payload[1]]),
                source_cid: u16::from_le_bytes([payload[2], payload[3]]),
                result: u16::from_le_bytes([payload[4], payload[5]]),
                status: u16::from_le_bytes([payload[6], payload[7]]),
            })
        }
        L2CAP_CFG_REQ => {
            if payload.len() < 4 {
                return Err(Error::MalformedL2cap);
            }
            Ok(L2capSignalPayload::ConfigurationRequest {
                dest_cid: u16::from_le_bytes([payload[0], payload[1]]),
                flags: u16::from_le_bytes([payload[2], payload[3]]),
                options: &payload[4..],
            })
        }
        L2CAP_CFG_RSP => {
            if payload.len() < 6 {
                return Err(Error::MalformedL2cap);
            }
            Ok(L2capSignalPayload::ConfigurationResponse {
                source_cid: u16::from_le_bytes([payload[0], payload[1]]),
                flags: u16::from_le_bytes([payload[2], payload[3]]),
                result: u16::from_le_bytes([payload[4], payload[5]]),
                options: &payload[6..],
            })
        }
        L2CAP_DISCONN_REQ => {
            if payload.len() < 4 {
                return Err(Error::MalformedL2cap);
            }
            Ok(L2capSignalPayload::DisconnectionRequest {
                dest_cid: u16::from_le_bytes([payload[0], payload[1]]),
                source_cid: u16::from_le_bytes([payload[2], payload[3]]),
            })
        }
        L2CAP_DISCONN_RSP => {
            if payload.len() < 4 {
                return Err(Error::MalformedL2cap);
            }
            Ok(L2capSignalPayload::DisconnectionResponse {
                dest_cid: u16::from_le_bytes([payload[0], payload[1]]),
                source_cid: u16::from_le_bytes([payload[2], payload[3]]),
            })
        }
        _ => Ok(L2capSignalPayload::Unknown {
            code: signal.code,
            payload,
        }),
    }
}

pub fn parse_hidp_packet(buf: &[u8]) -> Result<HidpPacket<'_>> {
    if buf.is_empty() {
        return Err(Error::MalformedL2cap);
    }
    Ok(HidpPacket {
        transaction: buf[0] & HIDP_HDR_TRANS_MASK,
        parameter: buf[0] & HIDP_HDR_PARAM_MASK,
        payload: &buf[1..],
    })
}

pub fn build_l2cap_connection_request(
    buf: &mut [u8],
    handle: u16,
    signal_id: u8,
    psm: u16,
    source_cid: u16,
) -> Result<usize> {
    let mut payload = [0u8; 4];
    payload[0..2].copy_from_slice(&psm.to_le_bytes());
    payload[2..4].copy_from_slice(&source_cid.to_le_bytes());
    build_l2cap_signal(buf, handle, signal_id, L2CAP_CONN_REQ, &payload)
}

pub fn build_l2cap_configuration_request(
    buf: &mut [u8],
    handle: u16,
    signal_id: u8,
    dest_cid: u16,
) -> Result<usize> {
    let mut payload = [0u8; 4];
    payload[0..2].copy_from_slice(&dest_cid.to_le_bytes());
    build_l2cap_signal(buf, handle, signal_id, L2CAP_CFG_REQ, &payload)
}

pub fn build_l2cap_configuration_request_with_mtu(
    buf: &mut [u8],
    handle: u16,
    signal_id: u8,
    dest_cid: u16,
    mtu: u16,
) -> Result<usize> {
    let mut payload = [0u8; 8];
    payload[0..2].copy_from_slice(&dest_cid.to_le_bytes());
    payload[4] = L2CAP_CONFIG_OPTION_MTU;
    payload[5] = 2;
    payload[6..8].copy_from_slice(&mtu.to_le_bytes());
    build_l2cap_signal(buf, handle, signal_id, L2CAP_CFG_REQ, &payload)
}

pub fn build_l2cap_configuration_response(
    buf: &mut [u8],
    handle: u16,
    signal_id: u8,
    source_cid: u16,
    result: u16,
) -> Result<usize> {
    build_l2cap_configuration_response_with_options(buf, handle, signal_id, source_cid, result, &[])
}

pub(crate) fn build_l2cap_configuration_response_with_options(
    buf: &mut [u8],
    handle: u16,
    signal_id: u8,
    source_cid: u16,
    result: u16,
    options: &[u8],
) -> Result<usize> {
    let payload_len = 6usize
        .checked_add(options.len())
        .ok_or(Error::BufferTooSmall)?;
    if payload_len > u16::MAX as usize - 8 {
        return Err(Error::BufferTooSmall);
    }
    let mut payload = alloc::vec![0u8; payload_len];
    payload[0..2].copy_from_slice(&source_cid.to_le_bytes());
    payload[4..6].copy_from_slice(&result.to_le_bytes());
    payload[6..].copy_from_slice(options);
    build_l2cap_signal(buf, handle, signal_id, L2CAP_CFG_RSP, &payload)
}

pub fn build_l2cap_data(
    buf: &mut [u8],
    handle: u16,
    dest_cid: u16,
    payload: &[u8],
) -> Result<usize> {
    if handle & !0x0fff != 0 {
        return Err(Error::MalformedAcl);
    }
    let l2cap_len = payload.len();
    if l2cap_len > u16::MAX as usize - 4 || buf.len() < 8 + l2cap_len {
        return Err(Error::BufferTooSmall);
    }
    let flags_handle = handle | ((L2CAP_ACL_START as u16) << 12);
    let acl_len = (4 + l2cap_len) as u16;
    buf[0..2].copy_from_slice(&flags_handle.to_le_bytes());
    buf[2..4].copy_from_slice(&acl_len.to_le_bytes());
    buf[4..6].copy_from_slice(&(l2cap_len as u16).to_le_bytes());
    buf[6..8].copy_from_slice(&dest_cid.to_le_bytes());
    buf[8..8 + l2cap_len].copy_from_slice(payload);
    Ok(8 + l2cap_len)
}

pub fn build_l2cap_connection_response(
    buf: &mut [u8],
    handle: u16,
    id: u8,
    local_cid: u16,
    remote_cid: u16,
    result: u16,
) -> Result<usize> {
    let mut payload = [0; 8];
    payload[0..2].copy_from_slice(&local_cid.to_le_bytes());
    payload[2..4].copy_from_slice(&remote_cid.to_le_bytes());
    payload[4..6].copy_from_slice(&result.to_le_bytes());
    build_l2cap_signal(buf, handle, id, L2CAP_CONN_RSP, &payload)
}
pub fn build_l2cap_disconnection_response(
    buf: &mut [u8],
    handle: u16,
    id: u8,
    local_cid: u16,
    remote_cid: u16,
) -> Result<usize> {
    let mut payload = [0; 4];
    payload[0..2].copy_from_slice(&local_cid.to_le_bytes());
    payload[2..4].copy_from_slice(&remote_cid.to_le_bytes());
    build_l2cap_signal(buf, handle, id, L2CAP_DISCONN_RSP, &payload)
}

pub fn build_l2cap_command_reject(
    buf: &mut [u8],
    handle: u16,
    id: u8,
    reason: u16,
) -> Result<usize> {
    build_l2cap_signal(buf, handle, id, L2CAP_CMD_REJECT, &reason.to_le_bytes())
}

pub fn build_l2cap_invalid_cid_reject(
    buf: &mut [u8],
    handle: u16,
    id: u8,
    requested_local_cid: u16,
) -> Result<usize> {
    let mut payload = [0u8; 6];
    payload[0..2].copy_from_slice(&L2CAP_INVALID_CID.to_le_bytes());
    payload[2..4].copy_from_slice(&requested_local_cid.to_le_bytes());
    payload[4..6].copy_from_slice(&L2CAP_NULL_CID.to_le_bytes());
    build_l2cap_signal(buf, handle, id, L2CAP_CMD_REJECT, &payload)
}

fn build_l2cap_signal(
    buf: &mut [u8],
    handle: u16,
    signal_id: u8,
    code: u8,
    payload: &[u8],
) -> Result<usize> {
    if payload.len() > u16::MAX as usize - 8 || buf.len() < 12 + payload.len() {
        return Err(Error::BufferTooSmall);
    }
    let mut header = [0u8; 4];
    header[0] = code;
    header[1] = signal_id;
    header[2..4].copy_from_slice(&(payload.len() as u16).to_le_bytes());
    let total = build_l2cap_data(buf, handle, L2CAP_CID_SIGNALING, &[])?;
    let signal_len = header.len() + payload.len();
    let acl_len = (4 + signal_len) as u16;
    buf[2..4].copy_from_slice(&acl_len.to_le_bytes());
    buf[4..6].copy_from_slice(&(signal_len as u16).to_le_bytes());
    buf[8..12].copy_from_slice(&header);
    buf[12..12 + payload.len()].copy_from_slice(payload);
    Ok(total + signal_len)
}

pub fn parse_event(buf: &[u8]) -> Result<HciEvent<'_>> {
    if buf.len() < 2 {
        return Err(Error::EventBufferTooSmall);
    }
    let packet_len = 2 + buf[1] as usize;
    if buf.len() < packet_len {
        return Err(Error::MalformedEvent);
    }
    let buf = &buf[..packet_len];

    match buf[0] {
        HCI_EVENT_COMMAND_COMPLETE => {
            if buf.len() < 6 {
                return Err(Error::MalformedEvent);
            }
            let opcode = u16::from_le_bytes([buf[3], buf[4]]);
            Ok(HciEvent::CommandComplete {
                opcode: HciOpcode(opcode),
                status: buf[5],
                params: &buf[6..],
            })
        }
        HCI_EVENT_COMMAND_STATUS => {
            if buf.len() < 6 {
                return Err(Error::MalformedEvent);
            }
            let opcode = u16::from_le_bytes([buf[4], buf[5]]);
            Ok(HciEvent::CommandStatus {
                opcode: HciOpcode(opcode),
                status: buf[2],
            })
        }
        HCI_EVENT_INQUIRY_COMPLETE => {
            if buf.len() < 3 {
                return Err(Error::MalformedEvent);
            }
            Ok(HciEvent::InquiryComplete { status: buf[2] })
        }
        HCI_EVENT_INQUIRY_RESULT | HCI_EVENT_INQUIRY_RESULT_WITH_RSSI => {
            parse_inquiry_results(buf, buf[0])
        }
        HCI_EVENT_CONN_COMPLETE => {
            if buf.len() < 13 {
                return Err(Error::MalformedEvent);
            }
            let mut address = [0u8; 6];
            address.copy_from_slice(&buf[5..11]);
            Ok(HciEvent::ConnectionComplete {
                status: buf[2],
                address: BluetoothAddress::new(address),
                handle: u16::from_le_bytes([buf[3], buf[4]]),
                link_type: buf[11],
                encryption: buf[12],
            })
        }
        HCI_EVENT_CONNECTION_REQUEST => {
            if buf.len() < 12 {
                return Err(Error::MalformedEvent);
            }
            let mut address = [0u8; 6];
            address.copy_from_slice(&buf[2..8]);
            let mut class_of_device = [0u8; 3];
            class_of_device.copy_from_slice(&buf[8..11]);
            Ok(HciEvent::ConnectionRequest {
                address: BluetoothAddress::new(address),
                class_of_device,
                link_type: buf[11],
            })
        }
        HCI_EVENT_DISCONN_COMPLETE => {
            if buf.len() < 6 {
                return Err(Error::MalformedEvent);
            }
            Ok(HciEvent::DisconnectionComplete {
                status: buf[2],
                handle: u16::from_le_bytes([buf[3], buf[4]]),
                reason: buf[5],
            })
        }
        HCI_EVENT_REMOTE_NAME_COMPLETE => {
            if buf.len() < 9 {
                return Err(Error::MalformedEvent);
            }
            let mut address = [0u8; 6];
            address.copy_from_slice(&buf[3..9]);
            Ok(HciEvent::RemoteNameComplete {
                status: buf[2],
                address: BluetoothAddress::new(address),
                name: &buf[9..],
            })
        }
        HCI_EVENT_PIN_CODE_REQUEST => {
            if buf.len() < 8 {
                return Err(Error::MalformedEvent);
            }
            let mut address = [0u8; 6];
            address.copy_from_slice(&buf[2..8]);
            Ok(HciEvent::PinCodeRequest {
                address: BluetoothAddress::new(address),
            })
        }
        HCI_EVENT_LINK_KEY_REQUEST => {
            if buf.len() < 8 {
                return Err(Error::MalformedEvent);
            }
            let mut address = [0u8; 6];
            address.copy_from_slice(&buf[2..8]);
            Ok(HciEvent::LinkKeyRequest {
                address: BluetoothAddress::new(address),
            })
        }
        HCI_EVENT_LINK_KEY_NOTIFICATION => {
            if buf.len() < 25 {
                return Err(Error::MalformedEvent);
            }
            let mut address = [0u8; 6];
            let mut key = [0u8; 16];
            address.copy_from_slice(&buf[2..8]);
            key.copy_from_slice(&buf[8..24]);
            Ok(HciEvent::LinkKeyNotification {
                address: BluetoothAddress::new(address),
                key,
                key_type: buf[24],
            })
        }
        _ => Ok(HciEvent::Unknown {
            event_code: buf[0],
            payload: &buf[2..],
        }),
    }
}

fn parse_inquiry_results(buf: &[u8], event_code: u8) -> Result<HciEvent<'_>> {
    if buf.len() < 3 {
        return Err(Error::MalformedEvent);
    }

    let count = buf[2] as usize;
    let packet_end = 3usize
        .checked_add(count.checked_mul(14).ok_or(Error::MalformedEvent)?)
        .ok_or(Error::MalformedEvent)?;
    if packet_end > buf.len() {
        return Err(Error::MalformedEvent);
    }
    let mut results = [InquiryResult {
        address: BluetoothAddress { bytes: [0; 6] },
        class_of_device: ClassOfDevice { bytes: [0; 3] },
        page_scan_rep_mode: 0,
        page_scan_mode: 0,
        clock_offset: 0,
    }; MAX_INQUIRY_RESULTS];

    let mut written = 0usize;
    let addresses = 3usize;
    let page_scan_modes = addresses + count * 6;
    let reserved = page_scan_modes + count;
    let classes = reserved
        + if event_code == HCI_EVENT_INQUIRY_RESULT_WITH_RSSI {
            count
        } else {
            count * 2
        };
    let clocks = classes + count * 3;
    for index in 0..count {
        if written == results.len() {
            break;
        }

        let mut address = [0u8; 6];
        let mut class = [0u8; 3];
        let address_offset = addresses + index * 6;
        let class_offset = classes + index * 3;
        let clock_offset = clocks + index * 2;
        address.copy_from_slice(&buf[address_offset..address_offset + 6]);
        class.copy_from_slice(&buf[class_offset..class_offset + 3]);
        results[written] = InquiryResult {
            address: BluetoothAddress::new(address),
            class_of_device: ClassOfDevice::new(class),
            page_scan_rep_mode: buf[page_scan_modes + index],
            page_scan_mode: buf[reserved + index],
            clock_offset: u16::from_le_bytes([buf[clock_offset], buf[clock_offset + 1]]),
        };
        written += 1;
    }

    Ok(HciEvent::InquiryResults {
        count: written,
        results,
    })
}

#[cfg(test)]
extern crate std;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hci_endpoint_addresses_validate_direction_and_address_bits() {
        assert_eq!(
            HciEndpointAddresses::new(0x02, 0x81, 0x82),
            Ok(HciEndpointAddresses {
                acl_out: 0x02,
                event_in: 0x81,
                acl_in: 0x82,
            })
        );
        assert_eq!(
            HciEndpointAddresses::new(0x82, 0x81, 0x83),
            Err(Error::InvalidEndpoint)
        );
        assert_eq!(
            HciEndpointAddresses::new(0x02, 0x01, 0x82),
            Err(Error::InvalidEndpoint)
        );
        assert_eq!(
            HciEndpointAddresses::new(0x02, 0x91, 0x82),
            Err(Error::InvalidEndpoint)
        );
        assert_eq!(
            HciEndpointAddresses::new(0x00, 0x81, 0x82),
            Err(Error::InvalidEndpoint)
        );
        assert_eq!(
            HciEndpointAddresses::new(0x02, 0x81, 0x81),
            Err(Error::InvalidEndpoint)
        );
    }

    #[test]
    fn class_of_device_extracts_major_bits() {
        let class = ClassOfDevice::new([0x08, 0x05, 0x00]);
        assert_eq!(class.major(), CLASS_MAJOR_PERIPHERAL);
        assert_eq!(class.minor(), 2);
        assert!(class.likely_game_controller());

        // A peripheral with the limited-discoverable service bit set.
        let remote = ClassOfDevice::new([0x04, 0x25, 0x00]);
        assert_eq!(remote.major(), 5);
        assert_eq!(remote.minor(), 1);
        assert!(remote.likely_game_controller());
        assert!(!ClassOfDevice::new([0x04, 0x02, 0x40]).likely_game_controller());

        let class = ClassOfDevice::new([0xfc, 0x17, 0x00]);
        assert_eq!(class.minor(), 0x3f);
    }

    #[test]
    fn opcode_encoding_matches_bluetooth_spec_examples() {
        assert_eq!(HciOpcode::RESET.raw(), 0x0c03);
        assert_eq!(HciOpcode::READ_BD_ADDR.raw(), 0x1009);
        assert_eq!(HciOpcode::WRITE_SCAN_ENABLE.raw(), 0x0c1a);
        assert_eq!(HciOpcode::INQUIRY.raw(), 0x0401);
        assert_eq!(HciOpcode::CREATE_CONNECTION.raw(), 0x0405);
        assert_eq!(HciOpcode::REMOTE_NAME_REQUEST.raw(), 0x0419);
    }

    #[test]
    fn internal_path_is_short_enough_for_ipc() {
        assert!(INTERNAL_BT_PATH.to_bytes_with_nul().len() <= 64);
    }

    #[test]
    fn parses_inquiry_event() {
        let buf = [
            HCI_EVENT_INQUIRY_RESULT,
            0x0f,
            0x01,
            0x66,
            0x55,
            0x44,
            0x33,
            0x22,
            0x11,
            0x02,
            0x00,
            0x00,
            0x08,
            0x05,
            0x04,
            0x13,
            0x34,
        ];
        match parse_event(&buf).unwrap() {
            HciEvent::InquiryResults { count, results } => {
                assert_eq!(count, 1);
                assert_eq!(
                    results[0].address.bytes,
                    [0x66, 0x55, 0x44, 0x33, 0x22, 0x11]
                );
                assert_eq!(results[0].class_of_device.bytes, [0x08, 0x05, 0x04]);
            }
            _ => panic!("unexpected event"),
        }
    }

    #[test]
    fn parses_multiple_inquiry_results_from_field_arrays() {
        let buf = [
            HCI_EVENT_INQUIRY_RESULT,
            0x1d,
            0x02,
            1,
            2,
            3,
            4,
            5,
            6,
            11,
            12,
            13,
            14,
            15,
            16,
            1,
            2,
            0,
            0,
            0,
            0,
            0x08,
            0x05,
            0x04,
            0x0c,
            0x02,
            0x40,
            0x34,
            0x12,
            0x78,
            0x56,
        ];
        match parse_event(&buf).unwrap() {
            HciEvent::InquiryResults { count, results } => {
                assert_eq!(count, 2);
                assert_eq!(results[0].address.bytes, [1, 2, 3, 4, 5, 6]);
                assert_eq!(results[1].address.bytes, [11, 12, 13, 14, 15, 16]);
                assert_eq!(results[0].class_of_device.bytes, [0x08, 0x05, 0x04]);
                assert_eq!(results[1].class_of_device.bytes, [0x0c, 0x02, 0x40]);
                assert_eq!(results[0].clock_offset, 0x1234);
                assert_eq!(results[1].clock_offset, 0x5678);
            }
            _ => panic!("unexpected event"),
        }
    }

    #[test]
    fn parses_multiple_inquiry_results_with_rssi_field_arrays() {
        let buf = [
            HCI_EVENT_INQUIRY_RESULT_WITH_RSSI,
            0x1d,
            0x02,
            1,
            2,
            3,
            4,
            5,
            6,
            11,
            12,
            13,
            14,
            15,
            16,
            1,
            2,
            0,
            0,
            0x08,
            0x05,
            0x04,
            0x0c,
            0x02,
            0x40,
            0x34,
            0x12,
            0x78,
            0x56,
            0xf0,
            0x10,
        ];
        match parse_event(&buf).unwrap() {
            HciEvent::InquiryResults { count, results } => {
                assert_eq!(count, 2);
                assert_eq!(results[0].address.bytes, [1, 2, 3, 4, 5, 6]);
                assert_eq!(results[1].address.bytes, [11, 12, 13, 14, 15, 16]);
                assert_eq!(results[0].class_of_device.bytes, [0x08, 0x05, 0x04]);
                assert_eq!(results[1].class_of_device.bytes, [0x0c, 0x02, 0x40]);
                assert_eq!(results[0].clock_offset, 0x1234);
                assert_eq!(results[1].clock_offset, 0x5678);
            }
            _ => panic!("unexpected event"),
        }
    }

    #[test]
    fn parses_pin_request_event() {
        let buf = [
            HCI_EVENT_PIN_CODE_REQUEST,
            0x06,
            0x10,
            0x20,
            0x30,
            0x40,
            0x50,
            0x60,
        ];
        match parse_event(&buf).unwrap() {
            HciEvent::PinCodeRequest { address } => {
                assert_eq!(address.bytes, [0x10, 0x20, 0x30, 0x40, 0x50, 0x60]);
            }
            _ => panic!("unexpected event"),
        }
    }

    #[test]
    fn parses_command_status_event() {
        let buf = [HCI_EVENT_COMMAND_STATUS, 0x04, 0x00, 0x01, 0x05, 0x04];
        match parse_event(&buf).unwrap() {
            HciEvent::CommandStatus { opcode, status } => {
                assert_eq!(opcode, HciOpcode::CREATE_CONNECTION);
                assert_eq!(status, 0);
            }
            _ => panic!("unexpected event"),
        }
    }

    #[test]
    fn parses_connection_complete_event() {
        let buf = [
            HCI_EVENT_CONN_COMPLETE,
            0x0b,
            0x00,
            0x2a,
            0x00,
            0x66,
            0x55,
            0x44,
            0x33,
            0x22,
            0x11,
            0x01,
            0x00,
        ];
        match parse_event(&buf).unwrap() {
            HciEvent::ConnectionComplete {
                status,
                address,
                handle,
                link_type,
                encryption,
            } => {
                assert_eq!(status, 0);
                assert_eq!(handle, 0x002a);
                assert_eq!(address.bytes, [0x66, 0x55, 0x44, 0x33, 0x22, 0x11]);
                assert_eq!(link_type, 1);
                assert_eq!(encryption, 0);
            }
            _ => panic!("unexpected event"),
        }
    }

    #[test]
    fn parses_connection_request_event() {
        let buf = [
            HCI_EVENT_CONNECTION_REQUEST,
            0x0a,
            0x66,
            0x55,
            0x44,
            0x33,
            0x22,
            0x11,
            0x04,
            0x02,
            0x40,
            0x01,
        ];
        match parse_event(&buf).unwrap() {
            HciEvent::ConnectionRequest {
                address,
                class_of_device,
                link_type,
            } => {
                assert_eq!(address.bytes, [0x66, 0x55, 0x44, 0x33, 0x22, 0x11]);
                assert_eq!(class_of_device, [0x04, 0x02, 0x40]);
                assert_eq!(link_type, 1);
            }
            _ => panic!("unexpected event"),
        }
    }

    #[test]
    fn parses_link_key_events() {
        let request = [
            HCI_EVENT_LINK_KEY_REQUEST,
            0x06,
            0x10,
            0x20,
            0x30,
            0x40,
            0x50,
            0x60,
        ];
        match parse_event(&request).unwrap() {
            HciEvent::LinkKeyRequest { address } => {
                assert_eq!(address.bytes, [0x10, 0x20, 0x30, 0x40, 0x50, 0x60]);
            }
            _ => panic!("unexpected event"),
        }

        let notification = [
            HCI_EVENT_LINK_KEY_NOTIFICATION,
            0x17,
            0x10,
            0x20,
            0x30,
            0x40,
            0x50,
            0x60,
            0,
            1,
            2,
            3,
            4,
            5,
            6,
            7,
            8,
            9,
            10,
            11,
            12,
            13,
            14,
            15,
            4,
        ];
        match parse_event(&notification).unwrap() {
            HciEvent::LinkKeyNotification {
                address,
                key,
                key_type,
            } => {
                assert_eq!(address.bytes, [0x10, 0x20, 0x30, 0x40, 0x50, 0x60]);
                assert_eq!(key, [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15]);
                assert_eq!(key_type, 4);
            }
            _ => panic!("unexpected event"),
        }
    }

    #[test]
    fn builds_and_parses_l2cap_connection_request() {
        let mut buf = [0u8; 32];
        let used = build_l2cap_connection_request(&mut buf, 0x002a, 7, L2CAP_PSM_HID_CTRL, 0x0040)
            .unwrap();
        assert_eq!(used, 16);

        let acl = parse_acl_packet(&buf[..used]).unwrap();
        assert_eq!(acl.handle, 0x002a);
        assert_eq!(acl.packet_boundary, L2CAP_ACL_START);

        let l2cap = parse_l2cap_packet(acl.payload).unwrap();
        assert_eq!(l2cap.channel_id, L2CAP_CID_SIGNALING);

        let signal = parse_l2cap_signal(l2cap.payload).unwrap();
        assert_eq!(signal.code, L2CAP_CONN_REQ);
        assert_eq!(signal.identifier, 7);

        match parse_l2cap_signal_payload(signal).unwrap() {
            L2capSignalPayload::ConnectionRequest { psm, source_cid } => {
                assert_eq!(psm, L2CAP_PSM_HID_CTRL);
                assert_eq!(source_cid, 0x0040);
            }
            _ => panic!("unexpected l2cap payload"),
        }
    }

    #[test]
    fn builds_l2cap_configuration_request_with_hid_mtu() {
        let mut buf = [0u8; 32];
        let used = build_l2cap_configuration_request_with_mtu(
            &mut buf,
            0x002a,
            8,
            0x0070,
            L2CAP_HID_RECOMMENDED_MTU,
        )
        .unwrap();

        let acl = parse_acl_packet(&buf[..used]).unwrap();
        let l2cap = parse_l2cap_packet(acl.payload).unwrap();
        let signal = parse_l2cap_signal(l2cap.payload).unwrap();
        assert_eq!(signal.code, L2CAP_CFG_REQ);
        assert_eq!(signal.identifier, 8);

        match parse_l2cap_signal_payload(signal).unwrap() {
            L2capSignalPayload::ConfigurationRequest {
                dest_cid,
                flags,
                options,
            } => {
                assert_eq!(dest_cid, 0x0070);
                assert_eq!(flags, 0);
                assert_eq!(
                    options,
                    &[
                        L2CAP_CONFIG_OPTION_MTU,
                        2,
                        L2CAP_HID_RECOMMENDED_MTU as u8,
                        0
                    ]
                );
            }
            _ => panic!("unexpected l2cap payload"),
        }
    }

    #[test]
    fn builds_and_parses_l2cap_hid_data() {
        let mut buf = [0u8; 32];
        let hid = [HIDP_TRANS_DATA | HIDP_DATA_RTYPE_INPUT, 0x30, 0x01, 0x02];
        let used = build_l2cap_data(&mut buf, 0x002a, 0x0041, &hid).unwrap();
        let acl = parse_acl_packet(&buf[..used]).unwrap();
        let l2cap = parse_l2cap_packet(acl.payload).unwrap();
        assert_eq!(l2cap.channel_id, 0x0041);

        let hidp = parse_hidp_packet(l2cap.payload).unwrap();
        assert_eq!(hidp.transaction, HIDP_TRANS_DATA);
        assert_eq!(hidp.parameter, HIDP_DATA_RTYPE_INPUT);
        assert_eq!(hidp.payload, &[0x30, 0x01, 0x02]);
    }

    #[test]
    fn disconnection_response_preserves_request_cid_order() {
        let local_cid = 0x0040;
        let remote_cid = 0x0070;
        let mut buf = [0; 32];
        let used =
            build_l2cap_disconnection_response(&mut buf, 0x002a, 9, local_cid, remote_cid).unwrap();
        let acl = parse_acl_packet(&buf[..used]).unwrap();
        let l2cap = parse_l2cap_packet(acl.payload).unwrap();
        let signal = parse_l2cap_signal(l2cap.payload).unwrap();

        assert_eq!(signal.code, L2CAP_DISCONN_RSP);
        assert_eq!(
            parse_l2cap_signal_payload(signal).unwrap(),
            L2capSignalPayload::DisconnectionResponse {
                dest_cid: local_cid,
                source_cid: remote_cid,
            }
        );
    }

    #[test]
    fn hid_l2cap_state_tracks_control_and_interrupt_readiness() {
        let mut state = HidL2capState::new();

        let ctrl_conn = [L2CAP_CONN_RSP, 1, 8, 0, 0x70, 0x00, 0x40, 0x00, 0, 0, 0, 0];
        state
            .observe_signal(parse_l2cap_signal(&ctrl_conn).unwrap())
            .unwrap();
        assert!(state.control.connected);
        assert_eq!(state.control.remote_cid, 0x0070);
        assert!(!state.ready());

        let ctrl_cfg_req = [L2CAP_CFG_REQ, 2, 4, 0, 0x40, 0x00, 0, 0];
        let ctrl_cfg_rsp = [L2CAP_CFG_RSP, 3, 6, 0, 0x70, 0x00, 0, 0, 0, 0];
        let ctrl_cfg_req_cont = [L2CAP_CFG_REQ, 2, 4, 0, 0x40, 0x00, 1, 0];
        state
            .observe_signal(parse_l2cap_signal(&ctrl_cfg_req_cont).unwrap())
            .unwrap();
        assert!(!state.control.remote_configured);
        state
            .observe_signal(parse_l2cap_signal(&ctrl_cfg_req).unwrap())
            .unwrap();
        state
            .observe_signal(parse_l2cap_signal(&ctrl_cfg_rsp).unwrap())
            .unwrap();
        assert!(state.control.ready());

        let intr_conn = [L2CAP_CONN_RSP, 4, 8, 0, 0x71, 0x00, 0x41, 0x00, 0, 0, 0, 0];
        let intr_cfg_req = [L2CAP_CFG_REQ, 5, 4, 0, 0x41, 0x00, 0, 0];
        let intr_cfg_rsp = [L2CAP_CFG_RSP, 6, 6, 0, 0x71, 0x00, 0, 0, 0, 0];
        for packet in [intr_conn.as_slice(), &intr_cfg_req, &intr_cfg_rsp] {
            state
                .observe_signal(parse_l2cap_signal(packet).unwrap())
                .unwrap();
        }

        assert!(state.interrupt.ready());
        assert!(state.ready());
    }

    #[test]
    fn parses_multiple_signals_from_one_l2cap_payload() {
        let payload = [
            L2CAP_CONN_RSP,
            1,
            8,
            0,
            0x40,
            0,
            0x41,
            0,
            0,
            0,
            0,
            0,
            L2CAP_CFG_REQ,
            2,
            4,
            0,
            0x41,
            0,
            0,
            0,
        ];
        let mut remaining = payload.as_slice();
        let first = parse_next_l2cap_signal(&mut remaining).unwrap().unwrap();
        assert_eq!(first.code, L2CAP_CONN_RSP);
        let second = parse_next_l2cap_signal(&mut remaining).unwrap().unwrap();
        assert_eq!(second.code, L2CAP_CFG_REQ);
        assert!(parse_next_l2cap_signal(&mut remaining).unwrap().is_none());
    }
}
