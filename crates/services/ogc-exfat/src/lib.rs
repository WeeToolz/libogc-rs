#![no_std]

//! Read-only exFAT volumes on Wii and GameCube block devices.

extern crate alloc;

#[cfg(test)]
extern crate std;

#[cfg(any(test, target_arch = "powerpc"))]
mod reader;

#[cfg(target_arch = "powerpc")]
mod disc;

#[cfg(target_arch = "powerpc")]
pub use disc::{DirectoryHandle, File, FileInfo, ReadHandle, Volume};
/// Result of a read-only filesystem operation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    /// The boot record or partition geometry is invalid.
    InvalidBoot,
    /// The device or filesystem layout is unsupported.
    Unsupported,
    /// The on-disk metadata is inconsistent.
    Corrupt,
    /// The block device could not read a sector.
    Io,
    /// No file or directory matches the name.
    NotFound,
    /// A path component or listing target is a file.
    NotDirectory,
    /// The open target is a directory.
    IsDirectory,
    /// A path has an invalid name component.
    InvalidPath,
    /// A file or directory handle belongs to another volume.
    InvalidHandle,
}
