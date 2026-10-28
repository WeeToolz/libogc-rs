//! Newlib errors. Socket and IOS adapters must translate their own code spaces.

use crate::fmt;
use crate::io::ErrorKind;
#[path = "newlib.rs"]
mod newlib;
use newlib::{ErrnoKind, describe_errno};

pub fn errno() -> i32 {
    // SAFETY: The provider reads the calling thread's initialized error storage.
    unsafe { crate::sys::wii::errno() }
}

pub fn is_interrupted(code: i32) -> bool {
    describe_errno(code).kind == ErrnoKind::Interrupted
}

pub fn decode_error_kind(code: i32) -> ErrorKind {
    match describe_errno(code).kind {
        ErrnoKind::NotFound => ErrorKind::NotFound,
        ErrnoKind::PermissionDenied => ErrorKind::PermissionDenied,
        ErrnoKind::Interrupted => ErrorKind::Interrupted,
        ErrnoKind::WouldBlock => ErrorKind::WouldBlock,
        ErrnoKind::OutOfMemory => ErrorKind::OutOfMemory,
        ErrnoKind::ResourceBusy => ErrorKind::ResourceBusy,
        ErrnoKind::AlreadyExists => ErrorKind::AlreadyExists,
        ErrnoKind::CrossesDevices => ErrorKind::CrossesDevices,
        ErrnoKind::NotADirectory => ErrorKind::NotADirectory,
        ErrnoKind::IsADirectory => ErrorKind::IsADirectory,
        ErrnoKind::InvalidInput => ErrorKind::InvalidInput,
        ErrnoKind::FileTooLarge => ErrorKind::FileTooLarge,
        ErrnoKind::StorageFull => ErrorKind::StorageFull,
        ErrnoKind::NotSeekable => ErrorKind::NotSeekable,
        ErrnoKind::ReadOnlyFilesystem => ErrorKind::ReadOnlyFilesystem,
        ErrnoKind::BrokenPipe => ErrorKind::BrokenPipe,
        ErrnoKind::Deadlock => ErrorKind::Deadlock,
        ErrnoKind::Unsupported => ErrorKind::Unsupported,
        ErrnoKind::DirectoryNotEmpty => ErrorKind::DirectoryNotEmpty,
        ErrnoKind::InvalidFilename => ErrorKind::InvalidFilename,
        ErrnoKind::FilesystemLoop => ErrorKind::FilesystemLoop,
        ErrnoKind::ConnectionReset => ErrorKind::ConnectionReset,
        ErrnoKind::ConnectionRefused => ErrorKind::ConnectionRefused,
        ErrnoKind::AddrInUse => ErrorKind::AddrInUse,
        ErrnoKind::ConnectionAborted => ErrorKind::ConnectionAborted,
        ErrnoKind::NetworkUnreachable => ErrorKind::NetworkUnreachable,
        ErrnoKind::NetworkDown => ErrorKind::NetworkDown,
        ErrnoKind::TimedOut => ErrorKind::TimedOut,
        ErrnoKind::HostUnreachable => ErrorKind::HostUnreachable,
        ErrnoKind::AddrNotAvailable => ErrorKind::AddrNotAvailable,
        ErrnoKind::NotConnected => ErrorKind::NotConnected,
        ErrnoKind::QuotaExceeded => ErrorKind::QuotaExceeded,
        ErrnoKind::StaleNetworkFileHandle => ErrorKind::StaleNetworkFileHandle,
        ErrnoKind::InvalidData => ErrorKind::InvalidData,
        ErrnoKind::Other => ErrorKind::Uncategorized,
    }
}

pub fn format_error(code: i32, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
    write!(
        formatter,
        "{} (newlib errno {})",
        describe_errno(code).message,
        code
    )
}
