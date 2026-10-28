#![no_std]

//! Verified Wii launch-state persistence and launch sequencing.

use core::ffi::CStr;
use core::mem::size_of;

pub const NANDBOOTINFO_BYTES: usize = 4_128;
pub const STATEFLAGS_BYTES: usize = 32;

#[repr(align(32))]
pub struct Aligned<T>(pub T);

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct NandBootInfo {
    pub checksum: u32,
    pub argsoff: u32,
    pub unk1: u8,
    pub unk2: u8,
    pub apptype: u8,
    pub titletype: u8,
    pub launchcode: u32,
    pub unknown: [u32; 2],
    pub launcher: u64,
    pub argbuf: [u8; 0x1000],
}

impl Default for NandBootInfo {
    fn default() -> Self {
        Self {
            checksum: 0,
            argsoff: 0,
            unk1: 0,
            unk2: 0,
            apptype: 0,
            titletype: 0,
            launchcode: 0,
            unknown: [0; 2],
            launcher: 0,
            argbuf: [0; 0x1000],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
pub struct StateFlags {
    pub checksum: u32,
    pub flags: u8,
    pub type_: u8,
    pub discstate: u8,
    pub returnto: u8,
    pub unknown: [u32; 6],
}

mod sealed {
    pub trait Sealed {}
}

/// A checksum-bearing Wii launch-state structure with no uninitialized padding.
pub trait StateValue: sealed::Sealed + Copy + Default + Eq {}

impl sealed::Sealed for NandBootInfo {}
impl StateValue for NandBootInfo {}
impl sealed::Sealed for StateFlags {}
impl StateValue for StateFlags {}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum ArgsError {
    TooLarge,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PersistError<E> {
    OpenWrite(E),
    Write(E),
    ShortWrite { expected: usize, actual: usize },
    CloseWrite(E),
    OpenRead(E),
    Read(E),
    ShortRead { expected: usize, actual: usize },
    CloseRead(E),
    Checksum,
    Mismatch,
}

pub trait FileOperations {
    type Handle;
    type Error;

    fn open_write(&mut self) -> Result<Self::Handle, Self::Error>;
    fn write(&mut self, handle: &Self::Handle, bytes: &[u8]) -> Result<usize, Self::Error>;
    fn close(&mut self, handle: Self::Handle) -> Result<(), Self::Error>;
    fn open_read(&mut self) -> Result<Self::Handle, Self::Error>;
    fn read(&mut self, handle: &Self::Handle, bytes: &mut [u8]) -> Result<usize, Self::Error>;
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum StateFile {
    NandBootInfo,
    StateFlags,
}

pub trait LaunchOperations {
    type Error;

    fn persist(&mut self, file: StateFile) -> Result<(), Self::Error>;
    fn launch(&mut self) -> Result<(), Self::Error>;
}

fn bytes_of<T: StateValue>(value: &T) -> &[u8] {
    // SAFETY: StateValue is sealed to the two repr(C) launch-state structures.
    // Both structures have no padding, and the slice does not outlive the value.
    unsafe { core::slice::from_raw_parts((value as *const T).cast(), size_of::<T>()) }
}

fn bytes_of_mut<T: StateValue>(value: &mut T) -> &mut [u8] {
    // SAFETY: StateValue is sealed to the two repr(C) structures without padding.
    // The slice covers a unique borrow and does not outlive the value.
    unsafe { core::slice::from_raw_parts_mut((value as *mut T).cast(), size_of::<T>()) }
}

pub fn checksum(bytes: &[u8]) -> u32 {
    let Some(payload) = bytes.get(4..) else {
        return 0;
    };
    payload.as_chunks::<4>().0.iter().fold(0u32, |sum, chunk| {
        sum.wrapping_add(u32::from_be_bytes(*chunk))
    })
}

pub fn set_checksum<T: StateValue>(value: &mut T) {
    let bytes = bytes_of_mut(value);
    if bytes.len() < 4 {
        return;
    }
    let checksum = checksum(bytes);
    bytes[..4].copy_from_slice(&checksum.to_be_bytes());
}

pub fn valid_checksum<T: StateValue>(value: &T) -> bool {
    let bytes = bytes_of(value);
    bytes.len() >= 4
        && u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]) == checksum(bytes)
}

pub fn format_title_id(title_id: u64) -> [u8; 17] {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = [0u8; 17];
    for (index, byte) in out[..16].iter_mut().enumerate() {
        let shift = 60 - (index * 4);
        *byte = HEX[((title_id >> shift) & 0xf) as usize];
    }
    out
}

pub fn set_args(nandboot: &mut NandBootInfo, args: &[&CStr]) -> Result<(), ArgsError> {
    let argbuf_len = nandboot.argbuf.len();
    let mut args_len = 0usize;
    for arg in args {
        args_len = args_len
            .checked_add(arg.to_bytes_with_nul().len())
            .ok_or(ArgsError::TooLarge)?;
    }

    let argc = args.len();
    let mut buf_len = args_len
        .checked_add(3)
        .map(|len| len & !3)
        .ok_or(ArgsError::TooLarge)?;
    buf_len = buf_len
        .checked_add(argc.checked_mul(4).ok_or(ArgsError::TooLarge)?)
        .and_then(|len| len.checked_add(8))
        .ok_or(ArgsError::TooLarge)?;
    if buf_len > argbuf_len {
        return Err(ArgsError::TooLarge);
    }

    let mut argp = argbuf_len - args_len;
    let mut argvp = argbuf_len - buf_len;
    nandboot.argbuf.fill(0);
    nandboot.argsoff = argbuf_len as u32 + argvp as u32;
    nandboot.argbuf[argvp..argvp + 4].copy_from_slice(&(argc as u32).to_be_bytes());
    argvp += 4;

    for arg in args {
        let bytes = arg.to_bytes_with_nul();
        nandboot.argbuf[argp..argp + bytes.len()].copy_from_slice(bytes);
        nandboot.argbuf[argvp..argvp + 4]
            .copy_from_slice(&((argp as u32) + argbuf_len as u32).to_be_bytes());
        argvp += 4;
        argp += bytes.len();
    }
    nandboot.argbuf[argvp..argvp + 4].copy_from_slice(&0u32.to_be_bytes());
    Ok(())
}

pub fn write_verified<T, O>(operations: &mut O, value: &mut T) -> Result<(), PersistError<O::Error>>
where
    T: StateValue,
    O: FileOperations,
{
    set_checksum(value);
    let expected = Aligned(*value);
    let expected_bytes = bytes_of(&expected.0);

    let write_handle = operations.open_write().map_err(PersistError::OpenWrite)?;
    let write_result = operations.write(&write_handle, expected_bytes);
    let write_close_result = operations.close(write_handle);
    match write_result {
        Err(error) => return Err(PersistError::Write(error)),
        Ok(actual) if actual != expected_bytes.len() => {
            return Err(PersistError::ShortWrite {
                expected: expected_bytes.len(),
                actual,
            });
        }
        Ok(_) => {}
    }
    write_close_result.map_err(PersistError::CloseWrite)?;

    let read_handle = operations.open_read().map_err(PersistError::OpenRead)?;
    let mut actual = Aligned(T::default());
    let actual_bytes = bytes_of_mut(&mut actual.0);
    let read_result = operations.read(&read_handle, actual_bytes);
    let read_close_result = operations.close(read_handle);
    match read_result {
        Err(error) => return Err(PersistError::Read(error)),
        Ok(count) if count != actual_bytes.len() => {
            return Err(PersistError::ShortRead {
                expected: actual_bytes.len(),
                actual: count,
            });
        }
        Ok(_) => {}
    }
    read_close_result.map_err(PersistError::CloseRead)?;

    if !valid_checksum(&actual.0) {
        return Err(PersistError::Checksum);
    }
    if actual.0 != expected.0 {
        return Err(PersistError::Mismatch);
    }
    Ok(())
}

pub fn commit_and_launch<O: LaunchOperations>(
    operations: &mut O,
    persist_stateflags: bool,
) -> Result<(), O::Error> {
    if persist_stateflags {
        operations.persist(StateFile::StateFlags)?;
    }
    operations.persist(StateFile::NandBootInfo)?;
    operations.launch()
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use std::vec;
    use std::vec::Vec;

    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    enum Call {
        OpenWrite,
        Write,
        CloseWrite,
        OpenRead,
        Read,
        CloseRead,
    }

    struct TestFileOperations {
        calls: Vec<Call>,
        failure: Option<Call>,
        short_write: bool,
        short_read: bool,
        corrupt_checksum: bool,
        mismatch: bool,
        written: Vec<u8>,
        next_handle: u8,
    }

    impl TestFileOperations {
        fn new() -> Self {
            Self {
                calls: Vec::new(),
                failure: None,
                short_write: false,
                short_read: false,
                corrupt_checksum: false,
                mismatch: false,
                written: Vec::new(),
                next_handle: 1,
            }
        }

        fn record(&mut self, call: Call) -> Result<(), i32> {
            self.calls.push(call);
            if self.failure == Some(call) {
                Err(-(self.calls.len() as i32))
            } else {
                Ok(())
            }
        }
    }

    impl FileOperations for TestFileOperations {
        type Handle = u8;
        type Error = i32;

        fn open_write(&mut self) -> Result<Self::Handle, Self::Error> {
            self.record(Call::OpenWrite)?;
            Ok(self.next_handle)
        }

        fn write(&mut self, _handle: &Self::Handle, bytes: &[u8]) -> Result<usize, Self::Error> {
            self.record(Call::Write)?;
            self.written = bytes.to_vec();
            Ok(bytes.len() - usize::from(self.short_write))
        }

        fn close(&mut self, handle: Self::Handle) -> Result<(), Self::Error> {
            let call = if handle == 1 {
                self.next_handle = 2;
                Call::CloseWrite
            } else {
                Call::CloseRead
            };
            self.record(call)
        }

        fn open_read(&mut self) -> Result<Self::Handle, Self::Error> {
            self.record(Call::OpenRead)?;
            Ok(self.next_handle)
        }

        fn read(&mut self, _handle: &Self::Handle, bytes: &mut [u8]) -> Result<usize, Self::Error> {
            self.record(Call::Read)?;
            bytes.copy_from_slice(&self.written);
            if self.corrupt_checksum {
                bytes[0] ^= 1;
            } else if self.mismatch {
                bytes[4] ^= 1;
                let checksum = checksum(bytes);
                bytes[..4].copy_from_slice(&checksum.to_be_bytes());
            }
            Ok(bytes.len() - usize::from(self.short_read))
        }
    }

    fn sample_flags() -> StateFlags {
        StateFlags {
            type_: 3,
            returnto: 2,
            ..StateFlags::default()
        }
    }

    #[test]
    fn protocol_apis_are_exercised_directly() {
        let mut flags = sample_flags();
        let bytes = bytes_of(&flags);
        assert_eq!(checksum(bytes), 0x0003_0002);
        set_checksum(&mut flags);
        assert!(valid_checksum(&flags));

        assert_eq!(
            &format_title_id(0x0001_0001_4841_4445),
            b"0001000148414445\0"
        );
        let mut info = NandBootInfo::default();
        set_args(&mut info, &[c"title"]).unwrap();

        let mut file = TestFileOperations::new();
        let write_handle = FileOperations::open_write(&mut file).unwrap();
        let data = [0x5a; 4];
        assert_eq!(
            FileOperations::write(&mut file, &write_handle, &data),
            Ok(data.len())
        );
        FileOperations::close(&mut file, write_handle).unwrap();
        let read_handle = FileOperations::open_read(&mut file).unwrap();
        let mut readback = [0; 4];
        assert_eq!(
            FileOperations::read(&mut file, &read_handle, &mut readback),
            Ok(4)
        );
        FileOperations::close(&mut file, read_handle).unwrap();
        assert_eq!(readback, data);

        let mut file = TestFileOperations::new();
        assert_eq!(write_verified(&mut file, &mut flags), Ok(()));

        let mut launch = TestLaunchOperations {
            calls: Vec::new(),
            failure: None,
        };
        LaunchOperations::persist(&mut launch, StateFile::NandBootInfo).unwrap();
        LaunchOperations::launch(&mut launch).unwrap();
        assert_eq!(commit_and_launch(&mut launch, false), Ok(()));
    }

    #[test]
    fn layouts_and_title_format_match_the_wii_contract() {
        assert_eq!(size_of::<NandBootInfo>(), NANDBOOTINFO_BYTES);
        assert_eq!(size_of::<StateFlags>(), STATEFLAGS_BYTES);
        assert_eq!(core::mem::align_of::<Aligned<NandBootInfo>>(), 32);
        assert_eq!(
            &format_title_id(0x0001_0001_4841_4445),
            b"0001000148414445\0"
        );
    }

    #[test]
    fn arguments_are_serialized_at_the_end_of_the_buffer() {
        let mut info = NandBootInfo::default();
        set_args(&mut info, &[c"title", c"argument"]).unwrap();
        let argv = (info.argsoff - 0x1000) as usize;
        assert_eq!(
            u32::from_be_bytes(info.argbuf[argv..argv + 4].try_into().unwrap()),
            2
        );
        let first = u32::from_be_bytes(info.argbuf[argv + 4..argv + 8].try_into().unwrap());
        let second = u32::from_be_bytes(info.argbuf[argv + 8..argv + 12].try_into().unwrap());
        assert_eq!(&info.argbuf[(first - 0x1000) as usize..][..6], b"title\0");
        assert_eq!(
            &info.argbuf[(second - 0x1000) as usize..][..9],
            b"argument\0"
        );
        assert_eq!(&info.argbuf[argv + 12..argv + 16], &[0; 4]);
    }

    #[test]
    fn oversized_arguments_do_not_modify_the_buffer() {
        let bytes = [b'x'; 0x1000];
        let argument_bytes = [bytes.as_slice(), b"\0"].concat();
        let argument = CStr::from_bytes_with_nul(&argument_bytes).unwrap();
        let mut info = NandBootInfo::default();
        let original = info;
        assert_eq!(set_args(&mut info, &[argument]), Err(ArgsError::TooLarge));
        assert_eq!(info, original);
    }

    #[test]
    fn verified_write_checks_every_persistence_step() {
        let expected = vec![
            Call::OpenWrite,
            Call::Write,
            Call::CloseWrite,
            Call::OpenRead,
            Call::Read,
            Call::CloseRead,
        ];
        let mut operations = TestFileOperations::new();
        let mut flags = sample_flags();
        assert_eq!(write_verified(&mut operations, &mut flags), Ok(()));
        assert_eq!(operations.calls, expected);
        assert!(valid_checksum(&flags));
    }

    #[test]
    fn every_file_error_stops_verification() {
        for failure in [
            Call::OpenWrite,
            Call::Write,
            Call::CloseWrite,
            Call::OpenRead,
            Call::Read,
            Call::CloseRead,
        ] {
            let mut operations = TestFileOperations::new();
            operations.failure = Some(failure);
            let mut flags = sample_flags();
            assert!(write_verified(&mut operations, &mut flags).is_err());
            let expected: &[Call] = match failure {
                Call::OpenWrite => &[Call::OpenWrite],
                Call::Write | Call::CloseWrite => &[Call::OpenWrite, Call::Write, Call::CloseWrite],
                Call::OpenRead => &[
                    Call::OpenWrite,
                    Call::Write,
                    Call::CloseWrite,
                    Call::OpenRead,
                ],
                Call::Read | Call::CloseRead => &[
                    Call::OpenWrite,
                    Call::Write,
                    Call::CloseWrite,
                    Call::OpenRead,
                    Call::Read,
                    Call::CloseRead,
                ],
            };
            assert_eq!(operations.calls, expected);
        }
    }

    #[test]
    fn short_write_and_short_read_are_rejected() {
        let mut operations = TestFileOperations::new();
        operations.short_write = true;
        let mut flags = sample_flags();
        assert!(matches!(
            write_verified(&mut operations, &mut flags),
            Err(PersistError::ShortWrite { .. })
        ));
        assert!(!operations.calls.contains(&Call::OpenRead));

        let mut operations = TestFileOperations::new();
        operations.short_read = true;
        assert!(matches!(
            write_verified(&mut operations, &mut flags),
            Err(PersistError::ShortRead { .. })
        ));
    }

    #[test]
    fn corrupt_or_different_readback_is_rejected() {
        let mut operations = TestFileOperations::new();
        operations.corrupt_checksum = true;
        let mut flags = sample_flags();
        assert_eq!(
            write_verified(&mut operations, &mut flags),
            Err(PersistError::Checksum)
        );

        let mut operations = TestFileOperations::new();
        operations.mismatch = true;
        assert_eq!(
            write_verified(&mut operations, &mut flags),
            Err(PersistError::Mismatch)
        );
    }

    #[derive(Copy, Clone, Debug, Eq, PartialEq)]
    enum LaunchCall {
        Persist(StateFile),
        Launch,
    }

    struct TestLaunchOperations {
        calls: Vec<LaunchCall>,
        failure: Option<LaunchCall>,
    }

    impl LaunchOperations for TestLaunchOperations {
        type Error = i32;

        fn persist(&mut self, file: StateFile) -> Result<(), Self::Error> {
            let call = LaunchCall::Persist(file);
            self.calls.push(call);
            if self.failure == Some(call) {
                Err(-1)
            } else {
                Ok(())
            }
        }

        fn launch(&mut self) -> Result<(), Self::Error> {
            self.calls.push(LaunchCall::Launch);
            if self.failure == Some(LaunchCall::Launch) {
                Err(-2)
            } else {
                Ok(())
            }
        }
    }

    #[test]
    fn transaction_commits_stateflags_then_nandboot_then_launches() {
        let mut operations = TestLaunchOperations {
            calls: Vec::new(),
            failure: None,
        };
        assert_eq!(commit_and_launch(&mut operations, true), Ok(()));
        assert_eq!(
            operations.calls,
            [
                LaunchCall::Persist(StateFile::StateFlags),
                LaunchCall::Persist(StateFile::NandBootInfo),
                LaunchCall::Launch,
            ]
        );
    }

    #[test]
    fn transaction_never_launches_after_a_persistence_failure() {
        for failure in [
            LaunchCall::Persist(StateFile::NandBootInfo),
            LaunchCall::Persist(StateFile::StateFlags),
        ] {
            let mut operations = TestLaunchOperations {
                calls: Vec::new(),
                failure: Some(failure),
            };
            assert_eq!(commit_and_launch(&mut operations, true), Err(-1));
            assert!(!operations.calls.contains(&LaunchCall::Launch));
        }
    }

    #[test]
    fn plain_transaction_skips_stateflags_and_forwards_launch_result() {
        let mut operations = TestLaunchOperations {
            calls: Vec::new(),
            failure: Some(LaunchCall::Launch),
        };
        assert_eq!(commit_and_launch(&mut operations, false), Err(-2));
        assert_eq!(
            operations.calls,
            [
                LaunchCall::Persist(StateFile::NandBootInfo),
                LaunchCall::Launch,
            ]
        );
    }
}
