//! Newlib error classification. IOS socket results use a different code space.
//!
//! Values come from devkitPPC's powerpc-eabi/include/sys/errno.h.

pub const ENOENT: i32 = 2;
pub const EIO: i32 = 5;
pub const EBADMSG: i32 = 77;
pub const ECANCELED: i32 = 140;
pub const EBADF: i32 = 9;
pub const EACCES: i32 = 13;
pub const EBUSY: i32 = 16;
pub const EEXIST: i32 = 17;
pub const EXDEV: i32 = 18;
pub const ENOMEM: i32 = 12;
pub const ENODEV: i32 = 19;
pub const ENOTDIR: i32 = 20;
pub const EISDIR: i32 = 21;
pub const EINVAL: i32 = 22;
pub const EFBIG: i32 = 27;
pub const ENOSPC: i32 = 28;
pub const EROFS: i32 = 30;
pub const ERANGE: i32 = 34;
pub const ENOTEMPTY: i32 = 90;
pub const ENOTSUP: i32 = 134;
pub const EOVERFLOW: i32 = 139;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ErrnoKind {
    NotFound,
    PermissionDenied,
    Interrupted,
    WouldBlock,
    OutOfMemory,
    ResourceBusy,
    AlreadyExists,
    CrossesDevices,
    NotADirectory,
    IsADirectory,
    InvalidInput,
    FileTooLarge,
    StorageFull,
    NotSeekable,
    ReadOnlyFilesystem,
    BrokenPipe,
    Deadlock,
    Unsupported,
    DirectoryNotEmpty,
    InvalidFilename,
    FilesystemLoop,
    ConnectionReset,
    ConnectionRefused,
    AddrInUse,
    ConnectionAborted,
    NetworkUnreachable,
    NetworkDown,
    TimedOut,
    HostUnreachable,
    AddrNotAvailable,
    NotConnected,
    QuotaExceeded,
    StaleNetworkFileHandle,
    InvalidData,
    Other,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ErrnoInfo {
    pub kind: ErrnoKind,
    pub message: &'static str,
}

/// Describe a positive newlib errno without allocation or a libc dependency.
/// Unknown codes retain an unknown classification; callers must retain the code.
pub fn describe_errno(code: i32) -> ErrnoInfo {
    use ErrnoKind::*;
    let (kind, message) = match code {
        0 => (Other, "no error"),
        1 | EACCES => (PermissionDenied, "permission denied"),
        ENOENT => (NotFound, "file or directory not found"),
        4 => (Interrupted, "operation interrupted"),
        EIO => (Other, "input/output error"),
        6 | ENODEV => (NotFound, "device not available"),
        EBADF => (Other, "invalid file descriptor"),
        11 => (WouldBlock, "operation would block"),
        ENOMEM | 105 => (OutOfMemory, "memory or network buffers exhausted"),
        EBUSY => (ResourceBusy, "resource busy"),
        EEXIST => (AlreadyExists, "file already exists"),
        EXDEV => (CrossesDevices, "operation crosses devices"),
        ENOTDIR => (NotADirectory, "path component is not a directory"),
        EISDIR => (IsADirectory, "path is a directory"),
        EINVAL | ERANGE | EOVERFLOW => (InvalidInput, "invalid argument or value out of range"),
        23 | 24 => (Other, "file descriptor capacity exhausted"),
        EFBIG => (FileTooLarge, "file is too large"),
        ENOSPC => (StorageFull, "storage is full"),
        29 => (NotSeekable, "file cannot be seeked"),
        EROFS => (ReadOnlyFilesystem, "filesystem is read only"),
        32 => (BrokenPipe, "broken pipe"),
        45 => (Deadlock, "deadlock detected"),
        88 | 95 | 106 | 123 | ENOTSUP => (Unsupported, "operation not supported"),
        ENOTEMPTY => (DirectoryNotEmpty, "directory is not empty"),
        91 => (InvalidFilename, "file or path name is too long"),
        92 => (FilesystemLoop, "filesystem link loop"),
        104 => (ConnectionReset, "connection reset"),
        111 => (ConnectionRefused, "connection refused"),
        112 => (AddrInUse, "address already in use"),
        113 => (ConnectionAborted, "connection aborted"),
        114 => (NetworkUnreachable, "network unreachable"),
        115 => (NetworkDown, "network is down"),
        116 => (TimedOut, "operation timed out"),
        117 | 118 => (HostUnreachable, "host unreachable"),
        125 => (AddrNotAvailable, "address not available"),
        128 => (NotConnected, "socket not connected"),
        132 => (QuotaExceeded, "storage quota exceeded"),
        133 => (StaleNetworkFileHandle, "stale file handle"),
        71 | EBADMSG => (InvalidData, "invalid protocol or device data"),
        138 => (InvalidData, "invalid byte sequence"),
        ECANCELED => (Other, "operation canceled"),
        119 => (Other, "operation in progress"),
        120 => (Other, "operation already in progress"),
        _ => (Other, "unknown newlib error"),
    };
    ErrnoInfo { kind, message }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uses_newlib_values_without_treating_them_as_linux_or_ios_codes() {
        assert_eq!(describe_errno(116).kind, ErrnoKind::TimedOut);
        assert_eq!(describe_errno(88).kind, ErrnoKind::Unsupported);
        assert_eq!(describe_errno(134).kind, ErrnoKind::Unsupported);
        assert_eq!(describe_errno(139).kind, ErrnoKind::InvalidInput);
        assert_eq!(describe_errno(11).kind, ErrnoKind::WouldBlock);
        assert_eq!(describe_errno(97).kind, ErrnoKind::Other);
        assert_eq!(describe_errno(-116).kind, ErrnoKind::Other);
    }

    #[test]
    fn messages_describe_real_failures_and_unknown_codes_stay_unknown() {
        assert_eq!(describe_errno(28).message, "storage is full");
        assert_eq!(describe_errno(13).kind, ErrnoKind::PermissionDenied);
        assert_eq!(describe_errno(12).kind, ErrnoKind::OutOfMemory);
        assert_eq!(describe_errno(4).kind, ErrnoKind::Interrupted);
        assert_eq!(describe_errno(9000).message, "unknown newlib error");
    }
}
