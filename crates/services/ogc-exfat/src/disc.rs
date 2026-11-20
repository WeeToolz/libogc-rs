//! Read-only interface to the existing console block device ABI.

use alloc::{boxed::Box, string::String};
use core::sync::atomic::{AtomicU32, Ordering};
use ogc_filesystem_sys::DiscInterface;

use crate::{
    Error,
    reader::{
        directory::Entry,
        mount::{Directory, Mounted},
        stream::{Cursor, Stream},
    },
};

type ReadSector = Box<dyn FnMut(u64, &mut [u8]) -> Result<(), Error> + Send>;

#[repr(align(32))]
struct AlignedSector([u8; 4096]);

/// A read-only volume. The caller manages device startup and shutdown.
pub struct Volume {
    mounted: Mounted<ReadSector>,
    identity: u32,
}

static NEXT_VOLUME_ID: AtomicU32 = AtomicU32::new(1);

fn next_volume_id() -> u32 {
    loop {
        let identity = NEXT_VOLUME_ID.fetch_add(1, Ordering::Relaxed);
        if identity != 0 {
            return identity;
        }
    }
}

/// The stored name, length in bytes, and type of a directory entry.
#[derive(Debug, Eq, PartialEq)]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
    pub is_dir: bool,
}

fn info(entry: &Entry) -> Result<FileInfo, Error> {
    Ok(FileInfo {
        name: String::from_utf16(&entry.name).map_err(|_| Error::Corrupt)?,
        size: entry.stream.length,
        is_dir: entry.directory(),
    })
}

impl Volume {
    /// Open the exFAT volume at sector zero or in a primary MBR partition.
    ///
    /// # Safety
    ///
    /// The caller must start the device and give its actual sector size and count.
    /// The device must remain available, with the same medium and contents, until
    /// this volume is dropped. The read callback must obey the `DiscInterface`
    /// buffer contract and complete the transfer before it returns.
    pub unsafe fn mount(
        disc: &'static DiscInterface,
        sector_count: u64,
        sector_size: usize,
    ) -> Result<Self, Error> {
        if sector_count > u64::from(u32::MAX) + 1 {
            return Err(Error::Unsupported);
        }
        let read = disc.read_sectors.ok_or(Error::Unsupported)?;
        let mut scratch = Box::new(AlignedSector([0; 4096]));
        let transfer: ReadSector = Box::new(move |lba, out| {
            let sector = u32::try_from(lba).map_err(|_| Error::Io)?;
            if u64::from(sector) >= sector_count {
                return Err(Error::Io);
            }
            // SAFETY: Mount validates the sector size and the caller guarantees
            // the geometry and callback contract. The DMA storage is aligned.
            if !unsafe { read(sector, 1, scratch.0.as_mut_ptr().cast()) } {
                return Err(Error::Io);
            }
            out.copy_from_slice(&scratch.0[..out.len()]);
            Ok(())
        });
        Mounted::open(sector_count, sector_size, transfer).map(|mounted| Self {
            mounted,
            identity: next_volume_id(),
        })
    }

    /// Return metadata for a path relative to the root. Use `/` for the root.
    pub fn stat(&mut self, path: &str) -> Result<FileInfo, Error> {
        info(&self.mounted.lookup(path)?)
    }

    /// Visit the immediate children of a directory, in on-disk order.
    ///
    /// Each name and entry set is checked before the callback. Use `stat` or
    /// `open` to validate a child's allocation. A later read error can stop the
    /// listing after some callbacks have already run.
    pub fn visit_dir(&mut self, path: &str, mut visit: impl FnMut(FileInfo)) -> Result<(), Error> {
        let dir = self.mounted.lookup(path)?;
        if !dir.directory() {
            return Err(Error::NotDirectory);
        }
        let at_root = dir.name == [u16::from(b'/')];
        let mut walk = Directory::new(dir.stream);
        while let Some(entry) = self.mounted.next_file(&mut walk, at_root)? {
            visit(info(&entry)?);
        }
        Ok(())
    }

    /// Validate a file's allocation and open it at offset zero.
    pub fn open(&mut self, path: &str) -> Result<File<'_>, Error> {
        let entry = self.mounted.lookup(path)?;
        if entry.directory() {
            return Err(Error::IsDirectory);
        }
        Ok(File {
            volume: &mut self.mounted,
            stream: entry.stream,
            cursor: Cursor::default(),
            position: 0,
        })
    }

    /// Open a file with cursor state that does not borrow the volume.
    ///
    /// Use `read_handle` and `seek_handle` to operate on the returned handle.
    /// A volume can own several handles at the same time. Dropping a handle
    /// closes it without I/O.
    pub fn open_handle(&mut self, path: &str) -> Result<ReadHandle, Error> {
        let entry = self.mounted.lookup(path)?;
        if entry.directory() {
            return Err(Error::IsDirectory);
        }
        Ok(ReadHandle {
            identity: self.identity,
            stream: entry.stream,
            cursor: Cursor::default(),
            position: 0,
        })
    }

    /// Move a handle to an absolute byte offset.
    pub fn seek_handle(&self, handle: &mut ReadHandle, position: u64) -> Result<(), Error> {
        self.check_handle(handle.identity)?;
        handle.position = position;
        Ok(())
    }

    /// Return the file size for a handle.
    pub fn handle_len(&self, handle: &ReadHandle) -> Result<u64, Error> {
        self.check_handle(handle.identity)?;
        Ok(handle.stream.length)
    }

    /// Read a handle and advance its independent cursor.
    pub fn read_handle(&mut self, handle: &mut ReadHandle, out: &mut [u8]) -> Result<usize, Error> {
        self.check_handle(handle.identity)?;
        let count =
            self.mounted
                .data
                .read(handle.stream, &mut handle.cursor, handle.position, out)?;
        handle.position = handle
            .position
            .checked_add(count as u64)
            .ok_or(Error::Corrupt)?;
        Ok(count)
    }

    /// Open a directory with cursor state that does not borrow the volume.
    pub fn open_dir_handle(&mut self, path: &str) -> Result<DirectoryHandle, Error> {
        let entry = self.mounted.lookup(path)?;
        if !entry.directory() {
            return Err(Error::NotDirectory);
        }
        Ok(DirectoryHandle {
            identity: self.identity,
            walk: Directory::new(entry.stream),
            at_root: entry.name == [u16::from(b'/')],
        })
    }

    /// Return the next directory entry, or `None` when iteration is complete.
    pub fn next_dir_handle(
        &mut self,
        handle: &mut DirectoryHandle,
    ) -> Result<Option<FileInfo>, Error> {
        self.check_handle(handle.identity)?;
        self.mounted
            .next_file(&mut handle.walk, handle.at_root)?
            .as_ref()
            .map(info)
            .transpose()
    }

    fn check_handle(&self, identity: u32) -> Result<(), Error> {
        if identity == self.identity {
            Ok(())
        } else {
            Err(Error::InvalidHandle)
        }
    }
}

/// An independent read-only file cursor owned by a volume.
pub struct ReadHandle {
    identity: u32,
    stream: Stream,
    cursor: Cursor,
    position: u64,
}

/// An independent directory cursor owned by a volume.
pub struct DirectoryHandle {
    identity: u32,
    walk: Directory,
    at_root: bool,
}

/// An open file that borrows the volume until it is dropped.
pub struct File<'a> {
    volume: &'a mut Mounted<ReadSector>,
    stream: Stream,
    cursor: Cursor,
    position: u64,
}

impl File<'_> {
    /// Move to an absolute offset. Reads at or past the file length return zero bytes.
    pub fn seek(&mut self, position: u64) {
        self.position = position;
    }

    /// Read at the current offset and advance by the number of bytes read.
    ///
    /// Bytes between the valid data length and the end of the file read as zero.
    /// A device failure leaves the offset unchanged and can modify the buffer
    /// before returning the error.
    pub fn read(&mut self, out: &mut [u8]) -> Result<usize, Error> {
        let count = self
            .volume
            .data
            .read(self.stream, &mut self.cursor, self.position, out)?;
        self.position += count as u64;
        Ok(count)
    }
}
