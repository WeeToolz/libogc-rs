//! `mad_bit_*` ABI helpers from `bit.c`.

use super::abi::MadBitPtr;

const CHAR_BIT: u16 = 8;
const CRC_POLY: u16 = 0x8005;

/// Initialize a bit pointer at `byte`.
///
/// # Safety
/// `bitptr` must be writable. `byte` must remain valid for later bit reads.
pub unsafe fn bit_init(bitptr: *mut MadBitPtr, byte: *const u8) {
    if bitptr.is_null() {
        return;
    }
    unsafe {
        (*bitptr).byte = byte;
        (*bitptr).cache = 0;
        (*bitptr).left = CHAR_BIT;
    }
}

/// Return the number of bits between two bit pointers.
///
/// # Safety
/// Both pointers must be readable and refer to positions in the same input.
pub unsafe fn bit_length(begin: *const MadBitPtr, end: *const MadBitPtr) -> u32 {
    if begin.is_null() || end.is_null() {
        return 0;
    }
    let begin = unsafe { &*begin };
    let end = unsafe { &*end };
    let bytes = unsafe { end.byte.offset_from(begin.byte.add(1)) };
    let trailing_bits = i32::from(CHAR_BIT) - i32::from(end.left);
    (begin.left as isize + 8 * bytes + trailing_bits as isize) as u32
}

/// Return the next byte that has not been fully consumed.
///
/// # Safety
/// `bitptr` must be readable.
pub unsafe fn bit_nextbyte(bitptr: *const MadBitPtr) -> *const u8 {
    if bitptr.is_null() {
        return core::ptr::null();
    }
    let bitptr = unsafe { &*bitptr };
    if bitptr.left == CHAR_BIT {
        bitptr.byte
    } else {
        unsafe { bitptr.byte.add(1) }
    }
}

/// Advance a bit pointer by `len` bits.
///
/// # Safety
/// `bitptr` must be writable and the resulting current byte must be readable
/// when the pointer is not byte-aligned.
pub unsafe fn bit_skip(bitptr: *mut MadBitPtr, len: u32) {
    if bitptr.is_null() {
        return;
    }
    let bitptr = unsafe { &mut *bitptr };
    bitptr.byte = unsafe { bitptr.byte.add((len / 8) as usize) };
    bitptr.left = bitptr.left.wrapping_sub((len % 8) as u16);
    if bitptr.left > CHAR_BIT {
        bitptr.byte = unsafe { bitptr.byte.add(1) };
        bitptr.left = bitptr.left.wrapping_add(CHAR_BIT);
    }
    if bitptr.left < CHAR_BIT {
        bitptr.cache = unsafe { *bitptr.byte } as u16;
    }
}

/// Read `len` bits as an unsigned MSB-first value.
///
/// # Safety
/// `bitptr` must be writable and enough input bytes must be readable.
pub unsafe fn bit_read(bitptr: *mut MadBitPtr, mut len: u32) -> u32 {
    if bitptr.is_null() {
        return 0;
    }
    let bitptr = unsafe { &mut *bitptr };
    if bitptr.left == CHAR_BIT {
        bitptr.cache = unsafe { *bitptr.byte } as u16;
    }
    if len < u32::from(bitptr.left) {
        let mask = (1u32 << bitptr.left) - 1;
        let value = (u32::from(bitptr.cache) & mask) >> (u32::from(bitptr.left) - len);
        bitptr.left -= len as u16;
        return value;
    }

    let mask = (1u32 << bitptr.left) - 1;
    let mut value = u32::from(bitptr.cache) & mask;
    len -= u32::from(bitptr.left);
    bitptr.byte = unsafe { bitptr.byte.add(1) };
    bitptr.left = CHAR_BIT;

    while len >= 8 {
        value = (value << 8) | u32::from(unsafe { *bitptr.byte });
        bitptr.byte = unsafe { bitptr.byte.add(1) };
        len -= 8;
    }
    if len > 0 {
        bitptr.cache = unsafe { *bitptr.byte } as u16;
        value = (value << len) | (u32::from(bitptr.cache) >> (8 - len));
        bitptr.left -= len as u16;
    }
    value
}

/// Compute the libmad CRC over `len` bits without changing the caller's bit
/// pointer.
///
/// # Safety
/// The bit pointer copy must reference at least `len` readable bits.
pub unsafe fn bit_crc(mut bitptr: MadBitPtr, mut len: u32, init: u16) -> u16 {
    let mut crc = init;
    while len != 0 {
        let input = unsafe { bit_read(&mut bitptr, 1) } as u16;
        let msb = input ^ (crc >> 15);
        crc = crc.wrapping_shl(1);
        if msb & 1 != 0 {
            crc ^= CRC_POLY;
        }
        len -= 1;
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bit_cursor_and_crc_match_reference_examples() {
        let bytes = [0b1011_0010, 0b0110_1101, 0xff];
        let mut ptr = MadBitPtr::default();
        unsafe { bit_init(&mut ptr, bytes.as_ptr()) };
        assert_eq!(unsafe { bit_read(&mut ptr, 3) }, 0b101);
        assert_eq!(ptr.left, 5);
        assert_eq!(unsafe { bit_nextbyte(&ptr) }, unsafe {
            bytes.as_ptr().add(1)
        });

        let mut begin = MadBitPtr::default();
        unsafe { bit_init(&mut begin, bytes.as_ptr()) };
        let mut end = begin;
        unsafe { bit_skip(&mut end, 11) };
        assert_eq!(unsafe { bit_length(&begin, &end) }, 11);

        let mut crc_ptr = MadBitPtr::default();
        let all_ones = [0xff, 0xff];
        unsafe { bit_init(&mut crc_ptr, all_ones.as_ptr()) };
        assert_eq!(unsafe { bit_crc(crc_ptr, 16, 0xffff) }, 0x0000);
    }
}
