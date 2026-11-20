//! Read-only interface to the existing console block device ABI.

use alloc::{boxed::Box, string::String};
use ogc_filesystem_sys::DiscInterface;

use crate::{
    Error,
    reader::{
        directory::Entry,
        mount::{Directory, Mounted},
        stream::{Cursor, Stream},
    },
};

type ReadSector = Box<dyn FnMut(u64, &mut [u8]) -> Result<(), Error>>;

#[repr(align(32))]
struct AlignedSector([u8; 4096]);

/// A read-only volume. The caller manages device startup and shutdown.
pub struct Volume(Mounted<ReadSector>);

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
        Mounted::open(sector_count, sector_size, transfer).map(Self)
    }

    /// Return metadata for a path relative to the root. Use `/` for the root.
    pub fn stat(&mut self, path: &str) -> Result<FileInfo, Error> {
        info(&self.0.lookup(path)?)
    }

    /// Visit the immediate children of a directory, in on-disk order.
    ///
    /// Each name and entry set is checked before the callback. Use `stat` or
    /// `open` to validate a child's allocation. A later read error can stop the
    /// listing after some callbacks have already run.
    pub fn visit_dir(&mut self, path: &str, mut visit: impl FnMut(FileInfo)) -> Result<(), Error> {
        let dir = self.0.lookup(path)?;
        if !dir.directory() {
            return Err(Error::NotDirectory);
        }
        let at_root = dir.name == [u16::from(b'/')];
        let mut walk = Directory::new(dir.stream);
        while let Some(entry) = self.0.next_file(&mut walk, at_root)? {
            visit(info(&entry)?);
        }
        Ok(())
    }

    /// Validate a file's allocation and open it at offset zero.
    pub fn open(&mut self, path: &str) -> Result<File<'_>, Error> {
        let entry = self.0.lookup(path)?;
        if entry.directory() {
            return Err(Error::IsDirectory);
        }
        Ok(File {
            volume: &mut self.0,
            stream: entry.stream,
            cursor: Cursor::default(),
            position: 0,
        })
    }
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
