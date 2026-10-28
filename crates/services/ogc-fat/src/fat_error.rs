//! FAT service errors and their Newlib mapping.
use ogc_sysbase as sysbase;

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum FatError {
    AlreadyMounted,
    AlreadyExists,
    Busy,
    DeviceUnavailable,
    MediaRemoved,
    InvalidBootSector,
    UnsupportedVariant,
    PathError,
    Io,
    NotFound,
    NoSpace,
    ReadOnly,
    IsDirectory,
    NotDirectory,
    DirectoryNotEmpty,
    NotSupported,
    PermissionDenied,
    CrossDevice,
    BadDescriptor,
    FileTooLarge,
}

/// Translate a FAT service error into the Newlib errno domain.
pub fn fat_errno(error: FatError) -> i32 {
    match error {
        FatError::AlreadyMounted | FatError::AlreadyExists => sysbase::EEXIST,
        FatError::Busy => sysbase::EBUSY,
        FatError::DeviceUnavailable | FatError::MediaRemoved => sysbase::ENODEV,
        FatError::InvalidBootSector => sysbase::EBADMSG,
        FatError::UnsupportedVariant => sysbase::ENOTSUP,
        FatError::PathError => sysbase::EINVAL,
        FatError::Io => sysbase::EIO,
        FatError::NotFound => sysbase::ENOENT,
        FatError::NoSpace => sysbase::ENOSPC,
        FatError::ReadOnly => sysbase::EROFS,
        FatError::IsDirectory => sysbase::EISDIR,
        FatError::NotDirectory => sysbase::ENOTDIR,
        FatError::DirectoryNotEmpty => sysbase::ENOTEMPTY,
        FatError::NotSupported => sysbase::ENOTSUP,
        FatError::PermissionDenied => sysbase::EACCES,
        FatError::CrossDevice => sysbase::EXDEV,
        FatError::BadDescriptor => sysbase::EBADF,
        FatError::FileTooLarge => sysbase::EFBIG,
    }
}
