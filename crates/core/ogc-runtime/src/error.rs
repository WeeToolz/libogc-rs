//! Shared error vocabulary used below compatibility ABI boundaries.

/// EXI-specific status retained without collapsing it to a generic `-1`.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ExiError(pub i32);

/// USB-specific status retained without collapsing it to a generic `-1`.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct UsbError(pub i32);

/// Network-specific status retained without collapsing it to a generic `-1`.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct NetworkError(pub i32);

/// Detailed internal Wii runtime error.
///
/// Compatibility crates translate this type into historical libogc return
/// codes only at their public ABI boundary.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[non_exhaustive]
pub enum WiiError {
    Ios(i32),
    Exi(ExiError),
    Usb(UsbError),
    Network(NetworkError),
    Timeout,
    Busy,
    Disconnected,
    InvalidArgument,
    UnsupportedByHardware,
    ProtocolError,
    ResourceExhausted,
    NotInitialized,
    AlreadyInitialized,
    Cancelled,
}

impl From<ExiError> for WiiError {
    fn from(value: ExiError) -> Self {
        Self::Exi(value)
    }
}

impl From<UsbError> for WiiError {
    fn from(value: UsbError) -> Self {
        Self::Usb(value)
    }
}

impl From<NetworkError> for WiiError {
    fn from(value: NetworkError) -> Self {
        Self::Network(value)
    }
}
