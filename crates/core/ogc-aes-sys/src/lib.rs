#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::ffi::{CStr, c_void};
use core::sync::atomic::{AtomicI32, Ordering};

use ogc_ipc::{FileDescriptor, HeapId, IPC_OPEN_NONE, IoctlvEntry, open_file};

pub const AES_BLOCK_SIZE: u32 = 0x10000;

const AES_HEAPSIZE: i32 = 0x200;
const AES_IOCTLV_ENCRYPT: i32 = 2;
const AES_IOCTLV_DECRYPT: i32 = 3;
const AES_EINVAL: i32 = -4;
const AES_ENOMEM: i32 = -1;
const AES_DEV: &[u8] = b"/dev/aes\0";

static AES_FD: AtomicI32 = AtomicI32::new(-1);
static AES_HID: AtomicI32 = AtomicI32::new(-1);

#[inline]
fn aes_device_path() -> &'static CStr {
    unsafe { CStr::from_bytes_with_nul_unchecked(AES_DEV) }
}

#[inline]
fn is_aligned_16(ptr: *const c_void) -> bool {
    (ptr as usize & 0x0f) == 0
}

#[inline]
fn current_fd() -> i32 {
    AES_FD.load(Ordering::Acquire)
}

#[inline]
fn current_heap() -> i32 {
    AES_HID.load(Ordering::Acquire)
}

#[inline]
fn set_fd(fd: i32) {
    AES_FD.store(fd, Ordering::Release);
}

#[inline]
fn set_heap(heap: i32) {
    AES_HID.store(heap, Ordering::Release);
}

/// Buffers and sizes used by one AES encrypt or decrypt operation.
#[derive(Clone, Copy)]
pub struct AesCommandConfig {
    /// Address of the AES key.
    pub key: *const c_void,
    /// Key length in bytes; IOS requires exactly 16.
    pub key_size: u32,
    /// Address of the mutable initialization vector.
    pub iv: *mut c_void,
    /// Initialization-vector length in bytes; IOS requires exactly 16.
    pub iv_size: u32,
    /// Address of the plaintext or ciphertext input.
    pub in_data: *const c_void,
    /// Address of the output buffer.
    pub out_data: *mut c_void,
    /// Number of input and output bytes; this must be a multiple of 16.
    pub data_size: u32,
}

/// Execute one AES IOS command.
///
/// This mirrors `libogc/libogc/aes.c`:
///
/// - input and output buffers must be 16-byte aligned
/// - key and IV sizes must be exactly 16 bytes
/// - data size must be a multiple of 16 bytes
/// - data is submitted to `/dev/aes` in 0x10000-byte chunks
/// - ioctlv command 2 encrypts and command 3 decrypts
unsafe fn aes_execute_command(command: i32, config: AesCommandConfig) -> i32 {
    if !is_aligned_16(config.in_data) || !is_aligned_16(config.out_data.cast_const()) {
        return AES_EINVAL;
    }

    if config.key_size != 16 || config.iv_size != 16 || (config.data_size & 15) != 0 {
        return AES_EINVAL;
    }

    if config.data_size != 0
        && (config.key.is_null()
            || config.iv.is_null()
            || config.in_data.is_null()
            || config.out_data.is_null())
    {
        return AES_EINVAL;
    }

    let heap = HeapId::new(current_heap());
    let Some(params) = heap.alloc((core::mem::size_of::<ogc_ipc::Ioctlv>() * 4) as i32) else {
        return AES_ENOMEM;
    };

    let mut ret = AES_ENOMEM;
    let fd = FileDescriptor::new(current_fd());
    let mut offset = 0usize;
    let total_len = config.data_size as usize;

    while offset < total_len {
        let remaining = total_len - offset;
        let size = remaining.min(AES_BLOCK_SIZE as usize);

        let input =
            unsafe { core::slice::from_raw_parts(config.in_data.cast::<u8>().add(offset), size) };
        let key_slice = unsafe {
            core::slice::from_raw_parts(config.key.cast::<u8>(), config.key_size as usize)
        };
        let output = unsafe {
            core::slice::from_raw_parts_mut(config.out_data.cast::<u8>().add(offset), size)
        };
        let iv_slice = unsafe {
            core::slice::from_raw_parts_mut(config.iv.cast::<u8>(), config.iv_size as usize)
        };

        ret = fd.ioctlv(
            command,
            2,
            [
                IoctlvEntry::In(input),
                IoctlvEntry::In(key_slice),
                IoctlvEntry::Out(output),
                IoctlvEntry::Out(iv_slice),
            ],
        );

        if ret < 0 {
            break;
        }

        offset += size;
    }

    unsafe {
        heap.free(params);
    }

    ret
}

pub fn aes_init() -> i32 {
    if current_fd() >= 0 {
        return -1;
    }

    let fd = match open_file(aes_device_path(), IPC_OPEN_NONE) {
        Ok(fd) => fd,
        Err(err) => return err,
    };

    set_fd(fd.raw());

    if current_heap() < 0 {
        match HeapId::create(AES_HEAPSIZE) {
            Ok(heap) => set_heap(heap.raw()),
            Err(err) => {
                aes_close();
                return err;
            }
        }
    }

    0
}

pub fn aes_close() -> i32 {
    let fd = current_fd();
    if fd < 0 {
        return -1;
    }

    let _close_result = FileDescriptor::new(fd).close();
    set_fd(-1);
    0
}

/// Encrypt a buffer through IOS AES.
///
/// # Safety
///
/// For nonzero `config.data_size`, `config.key` and `config.in_data` must be
/// readable for their specified lengths, while `config.iv` and
/// `config.out_data` must be writable for their specified lengths. Input and
/// output must be 16-byte aligned, and none of the writable regions may alias
/// any region accessed during the operation.
pub unsafe fn aes_encrypt(config: AesCommandConfig) -> i32 {
    unsafe { aes_execute_command(AES_IOCTLV_ENCRYPT, config) }
}

/// Decrypt a buffer through IOS AES.
///
/// # Safety
///
/// For nonzero `config.data_size`, `config.key` and `config.in_data` must be
/// readable for their specified lengths, while `config.iv` and
/// `config.out_data` must be writable for their specified lengths. Input and
/// output must be 16-byte aligned, and none of the writable regions may alias
/// any region accessed during the operation.
pub unsafe fn aes_decrypt(config: AesCommandConfig) -> i32 {
    unsafe { aes_execute_command(AES_IOCTLV_DECRYPT, config) }
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    #[repr(align(16))]
    struct Aligned<const N: usize>([u8; N]);

    #[test]
    fn rejects_bad_sizes_before_ios() {
        let mut key = Aligned([0u8; 16]);
        let mut iv = Aligned([0u8; 16]);
        let mut input = Aligned([0u8; 16]);
        let mut output = Aligned([0u8; 16]);

        assert_eq!(
            unsafe {
                aes_encrypt(AesCommandConfig {
                    key: key.0.as_mut_ptr().cast(),
                    key_size: 15,
                    iv: iv.0.as_mut_ptr().cast(),
                    iv_size: 16,
                    in_data: input.0.as_mut_ptr().cast(),
                    out_data: output.0.as_mut_ptr().cast(),
                    data_size: 16,
                })
            },
            AES_EINVAL
        );

        assert_eq!(
            unsafe {
                aes_decrypt(AesCommandConfig {
                    key: key.0.as_mut_ptr().cast(),
                    key_size: 16,
                    iv: iv.0.as_mut_ptr().cast(),
                    iv_size: 15,
                    in_data: input.0.as_mut_ptr().cast(),
                    out_data: output.0.as_mut_ptr().cast(),
                    data_size: 16,
                })
            },
            AES_EINVAL
        );

        assert_eq!(
            unsafe {
                aes_encrypt(AesCommandConfig {
                    key: key.0.as_mut_ptr().cast(),
                    key_size: 16,
                    iv: iv.0.as_mut_ptr().cast(),
                    iv_size: 16,
                    in_data: input.0.as_mut_ptr().cast(),
                    out_data: output.0.as_mut_ptr().cast(),
                    data_size: 15,
                })
            },
            AES_EINVAL
        );
    }
}
