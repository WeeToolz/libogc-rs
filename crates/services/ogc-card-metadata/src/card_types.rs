#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::vec;

    fn file() -> CardFile {
        CardFile {
            chn: 1,
            filenum: 7,
            offset: 0x1122,
            len: 512,
            iblock: 3,
        }
    }

    #[test]
    fn owned_completion_accessors_preserve_status_and_payloads() {
        let read_error = CardReadSubmitError::from_parts(-5, file());
        assert_eq!(read_error.status(), -5);
        assert_eq!(read_error.file().filenum, 7);
        assert_eq!(read_error.into_file().offset, 0x1122);

        let write_bytes = vec![0x5a, 0xa5];
        let write_error = CardWriteSubmitError::from_parts(-6, file(), write_bytes.clone());
        assert_eq!(write_error.status(), -6);
        assert_eq!(write_error.file().filenum, 7);
        assert_eq!(write_error.bytes(), &[0x5a, 0xa5]);
        let (write_status, write_file, returned_bytes) = write_error.into_parts();
        assert_eq!(write_status, -6);
        assert_eq!(write_file.offset, 0x1122);
        assert_eq!(returned_bytes, write_bytes);

        let read_bytes = vec![0x12, 0x34];
        let read_completion = CardReadCompletion::from_parts(0, file(), read_bytes.clone());
        assert_eq!(read_completion.status(), 0);
        assert_eq!(read_completion.file().filenum, 7);
        assert_eq!(read_completion.bytes(), &[0x12, 0x34]);
        let (read_status, read_file, returned_bytes) = read_completion.into_parts();
        assert_eq!(read_status, 0);
        assert_eq!(read_file.offset, 0x1122);
        assert_eq!(returned_bytes, read_bytes);

        let output_bytes = vec![0x3c, 0xc3];
        let write_completion = CardWriteCompletion::from_parts(0, file(), output_bytes.clone());
        assert_eq!(write_completion.status(), 0);
        assert_eq!(write_completion.file().filenum, 7);
        assert_eq!(write_completion.bytes(), &[0x3c, 0xc3]);
        let (write_status, write_file, returned_bytes) = write_completion.into_parts();
        assert_eq!(write_status, 0);
        assert_eq!(write_file.offset, 0x1122);
        assert_eq!(returned_bytes, output_bytes);
    }

    #[test]
    fn create_completion_only_exposes_a_file_on_success() {
        let success = CardCreateCompletion::from_parts(0, file());
        assert_eq!(success.status(), 0);
        assert_eq!(success.file().unwrap().filenum, 7);
        assert_eq!(success.into_result().unwrap().offset, 0x1122);

        let failure = CardCreateCompletion::from_parts(-4, file());
        assert!(failure.file().is_none());
        assert!(matches!(failure.into_result(), Err(-4)));
        let (status, returned_file) = CardCreateCompletion::from_parts(-4, file()).into_parts();
        assert_eq!(status, -4);
        assert_eq!(returned_file.filenum, 7);
    }

    #[test]
    fn metadata_completion_accessors_keep_failed_payloads_unavailable() {
        let mut entry = CardDir {
            fileno: 9,
            ..CardDir::default()
        };
        entry.filename[..4].copy_from_slice(b"SAVE");
        let create_entry = CardCreateEntryCompletion::from_parts(-3, entry, file());
        assert_eq!(create_entry.entry().fileno, 9);
        assert!(create_entry.file().is_none());
        let (status, returned_entry, returned_file) = create_entry.into_parts();
        assert_eq!(status, -3);
        assert_eq!(returned_entry.fileno, 9);
        assert_eq!(returned_file.filenum, 7);

        let entry = CardDir {
            fileno: 10,
            ..CardDir::default()
        };
        let delete_entry = CardDeleteEntryCompletion::from_parts(0, entry);
        assert_eq!(delete_entry.entry().fileno, 10);
        let (status, returned_entry) = delete_entry.into_parts();
        assert_eq!(status, 0);
        assert_eq!(returned_entry.fileno, 10);

        let mut stat = CardStat::default();
        stat.filename[..4].copy_from_slice(b"META");
        stat.len = 1024;
        let set_status = CardSetStatusCompletion::from_parts(-7, stat);
        assert_eq!(set_status.stat().len, 1024);
        let (status, returned_stat) = set_status.into_parts();
        assert_eq!(status, -7);
        assert_eq!(&returned_stat.filename[..4], b"META");
    }
}
extern crate alloc;

use alloc::vec::Vec;
use ogc_sys::Bool;

pub const CARD_FILENAMELEN: usize = 32;
pub const CARD_MAXICONS: usize = 8;
const CARD_STATUS_READY: i32 = 0;

#[repr(C)]
#[derive(Copy, Clone, Default)]
pub struct CardFile {
    pub chn: i32,
    pub filenum: i32,
    pub offset: i32,
    pub len: i32,
    pub iblock: u16,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CardDir {
    pub chn: i32,
    pub fileno: u32,
    pub filelen: u32,
    pub permissions: u8,
    pub filename: [u8; CARD_FILENAMELEN],
    pub gamecode: [u8; 4],
    pub company: [u8; 2],
    pub showall: Bool,
}

impl Default for CardDir {
    fn default() -> Self {
        Self {
            chn: 0,
            fileno: 0,
            filelen: 0,
            permissions: 0,
            filename: [0; CARD_FILENAMELEN],
            gamecode: [0; 4],
            company: [0; 2],
            showall: 0,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct CardStat {
    pub filename: [u8; CARD_FILENAMELEN],
    pub len: u32,
    pub time: u32,
    pub gamecode: [u8; 4],
    pub company: [u8; 2],
    pub banner_fmt: u8,
    pub icon_addr: u32,
    pub icon_fmt: u16,
    pub iconfmt: [u16; CARD_MAXICONS],
    pub icon_speed: u16,
    pub iconspeed: [u16; CARD_MAXICONS],
    pub comment_addr: u32,
    pub offset_banner: u32,
    pub offset_banner_tlut: u32,
    pub offset_icon: [u32; CARD_MAXICONS],
    pub offset_icon_tlut: [u32; CARD_MAXICONS],
    pub offset_data: u32,
}

impl Default for CardStat {
    fn default() -> Self {
        Self {
            filename: [0; CARD_FILENAMELEN],
            len: 0,
            time: 0,
            gamecode: [0; 4],
            company: [0; 2],
            banner_fmt: 0,
            icon_addr: 0,
            icon_fmt: 0,
            iconfmt: [0; CARD_MAXICONS],
            icon_speed: 0,
            iconspeed: [0; CARD_MAXICONS],
            comment_addr: 0,
            offset_banner: 0,
            offset_banner_tlut: 0,
            offset_icon: [0; CARD_MAXICONS],
            offset_icon_tlut: [0; CARD_MAXICONS],
            offset_data: 0,
        }
    }
}

macro_rules! status_file {
    ($name:ident) => {
        pub struct $name {
            status: i32,
            file: CardFile,
        }
        impl $name {
            pub const fn from_parts(status: i32, file: CardFile) -> Self {
                Self { status, file }
            }
            pub const fn status(&self) -> i32 {
                self.status
            }
            pub const fn file(&self) -> &CardFile {
                &self.file
            }
            pub fn into_file(self) -> CardFile {
                self.file
            }
        }
    };
}

status_file!(CardReadSubmitError);

pub struct CardWriteSubmitError {
    status: i32,
    file: CardFile,
    bytes: Vec<u8>,
}
impl CardWriteSubmitError {
    pub fn from_parts(status: i32, file: CardFile, bytes: Vec<u8>) -> Self {
        Self {
            status,
            file,
            bytes,
        }
    }
    pub const fn status(&self) -> i32 {
        self.status
    }
    pub const fn file(&self) -> &CardFile {
        &self.file
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn into_parts(self) -> (i32, CardFile, Vec<u8>) {
        (self.status, self.file, self.bytes)
    }
}

macro_rules! data_completion {
    ($name:ident) => {
        pub struct $name {
            status: i32,
            file: CardFile,
            bytes: Vec<u8>,
        }
        impl $name {
            pub fn from_parts(status: i32, file: CardFile, bytes: Vec<u8>) -> Self {
                Self {
                    status,
                    file,
                    bytes,
                }
            }
            pub const fn status(&self) -> i32 {
                self.status
            }
            pub const fn file(&self) -> &CardFile {
                &self.file
            }
            pub fn bytes(&self) -> &[u8] {
                &self.bytes
            }
            pub fn into_parts(self) -> (i32, CardFile, Vec<u8>) {
                (self.status, self.file, self.bytes)
            }
        }
    };
}

data_completion!(CardReadCompletion);
data_completion!(CardWriteCompletion);

pub struct CardCreateCompletion {
    status: i32,
    file: CardFile,
}
impl CardCreateCompletion {
    pub const fn from_parts(status: i32, file: CardFile) -> Self {
        Self { status, file }
    }
    pub const fn status(&self) -> i32 {
        self.status
    }
    pub fn file(&self) -> Option<&CardFile> {
        (self.status == CARD_STATUS_READY).then_some(&self.file)
    }
    pub fn into_result(self) -> Result<CardFile, i32> {
        if self.status == CARD_STATUS_READY {
            Ok(self.file)
        } else {
            Err(self.status)
        }
    }
    pub fn into_parts(self) -> (i32, CardFile) {
        (self.status, self.file)
    }
}

pub struct CardCreateEntryCompletion {
    status: i32,
    entry: CardDir,
    file: CardFile,
}
impl CardCreateEntryCompletion {
    pub const fn from_parts(status: i32, entry: CardDir, file: CardFile) -> Self {
        Self {
            status,
            entry,
            file,
        }
    }
    pub const fn status(&self) -> i32 {
        self.status
    }
    pub const fn entry(&self) -> &CardDir {
        &self.entry
    }
    pub fn file(&self) -> Option<&CardFile> {
        (self.status == CARD_STATUS_READY).then_some(&self.file)
    }
    pub fn into_parts(self) -> (i32, CardDir, CardFile) {
        (self.status, self.entry, self.file)
    }
}

pub struct CardDeleteEntryCompletion {
    status: i32,
    entry: CardDir,
}
impl CardDeleteEntryCompletion {
    pub const fn from_parts(status: i32, entry: CardDir) -> Self {
        Self { status, entry }
    }
    pub const fn status(&self) -> i32 {
        self.status
    }
    pub const fn entry(&self) -> &CardDir {
        &self.entry
    }
    pub fn into_parts(self) -> (i32, CardDir) {
        (self.status, self.entry)
    }
}

pub struct CardSetStatusCompletion {
    status: i32,
    stat: CardStat,
}
impl CardSetStatusCompletion {
    pub const fn from_parts(status: i32, stat: CardStat) -> Self {
        Self { status, stat }
    }
    pub const fn status(&self) -> i32 {
        self.status
    }
    pub const fn stat(&self) -> &CardStat {
        &self.stat
    }
    pub fn into_parts(self) -> (i32, CardStat) {
        (self.status, self.stat)
    }
}
