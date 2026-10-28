#![no_std]

#[cfg(all(not(target_arch = "powerpc"), not(test)))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::ffi::c_void;
use core::slice;

pub mod sys {
    use core::ffi::c_void;
    use core::slice;

    pub use ogc_sys::Bool;

    pub const SYS_BASE_CACHED: u32 = 0x8000_0000;
    pub const SYS_BASE_UNCACHED: u32 = 0xC000_0000;
    pub const SYS_WD_NULL: u32 = 0xffff_ffff;

    pub const SYS_RESTART: u32 = 0;
    pub const SYS_HOTRESET: u32 = 1;
    pub const SYS_SHUTDOWN: u32 = 2;
    pub const SYS_RETURNTOMENU: u32 = 3;
    pub const SYS_POWEROFF: u32 = 4;
    pub const SYS_POWEROFF_STANDBY: u32 = 5;
    pub const SYS_POWEROFF_IDLE: u32 = 6;

    pub const SYS_PROTECTCHAN0: u32 = 0;
    pub const SYS_PROTECTCHAN1: u32 = 1;
    pub const SYS_PROTECTCHAN2: u32 = 2;
    pub const SYS_PROTECTCHAN3: u32 = 3;
    pub const SYS_PROTECTCHANMAX: u32 = 4;

    pub const SYS_PROTECTNONE: u32 = 0x0000_0000;
    pub const SYS_PROTECTREAD: u32 = 0x0000_0001;
    pub const SYS_PROTECTWRITE: u32 = 0x0000_0002;
    pub const SYS_PROTECTRDWR: u32 = SYS_PROTECTREAD | SYS_PROTECTWRITE;

    pub const SYS_LANG_ENGLISH: u32 = 0;
    pub const SYS_LANG_GERMAN: u32 = 1;
    pub const SYS_LANG_FRENCH: u32 = 2;
    pub const SYS_LANG_SPANISH: u32 = 3;
    pub const SYS_LANG_ITALIAN: u32 = 4;
    pub const SYS_LANG_DUTCH: u32 = 5;

    pub const SYS_VIDEO_NTSC: u32 = 0;
    pub const SYS_VIDEO_PAL: u32 = 1;
    pub const SYS_VIDEO_MPAL: u32 = 2;

    pub const SYS_SOUND_MONO: u32 = 0;
    pub const SYS_SOUND_STEREO: u32 = 1;

    pub const SYS_FONTSIZE_ANSI: u32 = 288 + 131_072;
    pub const SYS_FONTSIZE_SJIS: u32 = 3_840 + 1_179_648;

    pub const RNC_FILE_IS_NOT_RNC: i32 = -1;
    pub const RNC_HUF_DECODE_ERROR: i32 = -2;
    pub const RNC_FILE_SIZE_MISMATCH: i32 = -3;
    pub const RNC_PACKED_CRC_ERROR: i32 = -4;
    pub const RNC_UNPACKED_CRC_ERROR: i32 = -5;

    /// # Safety
    ///
    /// `packed` must point to a readable RNC1 header buffer with at least 8 bytes.
    pub unsafe fn depackrnc1_ulen(packed: *mut c_void) -> i32 {
        unsafe {
            if packed.is_null() {
                return RNC_FILE_IS_NOT_RNC;
            }
            let packed = slice::from_raw_parts(packed.cast::<u8>(), 8);
            super::depackrnc1_unpacked_len(packed)
        }
    }

    /// # Safety
    ///
    /// `packed` and `unpacked` must satisfy the legacy libogc RNC1 ABI contract.
    pub unsafe fn depackrnc1(packed: *mut c_void, unpacked: *mut c_void) -> i32 {
        unsafe { super::depackrnc1(packed, unpacked) }
    }

    /// # Safety
    ///
    /// `packed` and `unpacked` must satisfy the legacy libogc RNC2 ABI contract.
    pub unsafe fn depackrnc2(packed: *mut c_void, unpacked: *mut c_void) {
        unsafe { super::depackrnc2(packed, unpacked) }
    }
}

pub use sys::{
    Bool, RNC_FILE_IS_NOT_RNC, RNC_FILE_SIZE_MISMATCH, RNC_HUF_DECODE_ERROR, RNC_PACKED_CRC_ERROR,
    RNC_UNPACKED_CRC_ERROR, SYS_BASE_CACHED, SYS_BASE_UNCACHED, SYS_FONTSIZE_ANSI,
    SYS_FONTSIZE_SJIS, SYS_HOTRESET, SYS_LANG_DUTCH, SYS_LANG_ENGLISH, SYS_LANG_FRENCH,
    SYS_LANG_GERMAN, SYS_LANG_ITALIAN, SYS_LANG_SPANISH, SYS_POWEROFF, SYS_POWEROFF_IDLE,
    SYS_POWEROFF_STANDBY, SYS_PROTECTCHAN0, SYS_PROTECTCHAN1, SYS_PROTECTCHAN2, SYS_PROTECTCHAN3,
    SYS_PROTECTCHANMAX, SYS_PROTECTNONE, SYS_PROTECTRDWR, SYS_PROTECTREAD, SYS_PROTECTWRITE,
    SYS_RESTART, SYS_RETURNTOMENU, SYS_SHUTDOWN, SYS_SOUND_MONO, SYS_SOUND_STEREO, SYS_VIDEO_MPAL,
    SYS_VIDEO_NTSC, SYS_VIDEO_PAL, SYS_WD_NULL,
};
pub const fn mem_virtual_to_physical(addr: usize) -> u32 {
    (addr as u32) & !SYS_BASE_UNCACHED
}

pub const fn mem_physical_to_k0(addr: u32) -> usize {
    addr.wrapping_add(SYS_BASE_CACHED) as usize
}

pub const fn mem_physical_to_k1(addr: u32) -> usize {
    addr.wrapping_add(SYS_BASE_UNCACHED) as usize
}

pub const fn mem_k0_to_physical(addr: usize) -> u32 {
    (addr as u32).wrapping_sub(SYS_BASE_CACHED)
}

pub const fn mem_k1_to_physical(addr: usize) -> u32 {
    (addr as u32).wrapping_sub(SYS_BASE_UNCACHED)
}

pub const fn mem_k0_to_k1(addr: usize) -> usize {
    (addr as u32).wrapping_add(SYS_BASE_UNCACHED - SYS_BASE_CACHED) as usize
}

pub const fn mem_k1_to_k0(addr: usize) -> usize {
    (addr as u32).wrapping_sub(SYS_BASE_UNCACHED - SYS_BASE_CACHED) as usize
}

const RNC_SIGNATURE: u32 = 0x524e_4301;

#[derive(Copy, Clone, Default)]
struct BitStream {
    bitbuf: u32,
    bitcount: i32,
}

#[derive(Copy, Clone, Default)]
struct HuffEntry {
    code: u32,
    codelen: u8,
    value: u8,
}

#[derive(Copy, Clone, Default)]
struct HuffTable {
    num: usize,
    table: [HuffEntry; 32],
}

fn read_be_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(4)?;
    let chunk = bytes.get(offset..end)?;
    Some(
        ((chunk[0] as u32) << 24)
            | ((chunk[1] as u32) << 16)
            | ((chunk[2] as u32) << 8)
            | (chunk[3] as u32),
    )
}

fn read_be_u16(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(2)?;
    let chunk = bytes.get(offset..end)?;
    Some(((chunk[0] as u32) << 8) | (chunk[1] as u32))
}

fn read_le_u16(bytes: &[u8], offset: usize) -> Option<u32> {
    let end = offset.checked_add(2)?;
    let chunk = bytes.get(offset..end)?;
    Some((chunk[0] as u32) | ((chunk[1] as u32) << 8))
}

fn mirror(mut x: u32, n: u32) -> u32 {
    let mut top = 1u32 << (n - 1);
    let mut bottom = 1u32;
    while top > bottom {
        let mask = top | bottom;
        let masked = x & mask;
        if masked != 0 && masked != mask {
            x ^= mask;
        }
        top >>= 1;
        bottom <<= 1;
    }
    x
}

fn rnc_crc(data: &[u8]) -> u32 {
    let mut crctab = [0u16; 256];
    let mut i = 0usize;
    while i < 256 {
        let mut val = i as u16;
        let mut j = 0;
        while j < 8 {
            if val & 1 != 0 {
                val = (val >> 1) ^ 0xA001;
            } else {
                val >>= 1;
            }
            j += 1;
        }
        crctab[i] = val;
        i += 1;
    }

    let mut val = 0u16;
    for &byte in data {
        val ^= byte as u16;
        val = (val >> 8) ^ crctab[(val & 0xff) as usize];
    }

    val as u32
}

fn bitread_init(bitstream: &mut BitStream, input: &[u8], input_idx: usize) -> Option<()> {
    bitstream.bitbuf = read_le_u16(input, input_idx)?;
    bitstream.bitcount = 16;
    Some(())
}

fn bitread_fix(bitstream: &mut BitStream, input: &[u8], input_idx: usize) -> Option<()> {
    bitstream.bitcount -= 16;
    bitstream.bitbuf &= (1u32 << bitstream.bitcount) - 1;
    bitstream.bitbuf |= read_le_u16(input, input_idx)? << bitstream.bitcount;
    bitstream.bitcount += 16;
    Some(())
}

fn bit_peek(bitstream: &BitStream, mask: u32) -> u32 {
    bitstream.bitbuf & mask
}

fn bit_advance(
    bitstream: &mut BitStream,
    n: i32,
    input: &[u8],
    input_idx: &mut usize,
) -> Option<()> {
    bitstream.bitbuf >>= n;
    bitstream.bitcount -= n;
    if bitstream.bitcount < 16 {
        *input_idx = input_idx.checked_add(2)?;
        bitstream.bitbuf |= read_le_u16(input, *input_idx)? << bitstream.bitcount;
        bitstream.bitcount += 16;
    }
    Some(())
}

fn bit_read(
    bitstream: &mut BitStream,
    mask: u32,
    n: i32,
    input: &[u8],
    input_idx: &mut usize,
) -> Option<u32> {
    let result = bit_peek(bitstream, mask);
    bit_advance(bitstream, n, input, input_idx)?;
    Some(result)
}

fn read_huftable(
    h: &mut HuffTable,
    bitstream: &mut BitStream,
    input: &[u8],
    input_idx: &mut usize,
) -> Option<()> {
    let num = bit_read(bitstream, 0x1f, 5, input, input_idx)? as usize;
    h.num = 0;
    if num == 0 {
        return Some(());
    }

    let mut leaflen = [0u32; 32];
    let mut leafmax = 1u32;
    let mut i = 0usize;
    while i < num {
        leaflen[i] = bit_read(bitstream, 0x0f, 4, input, input_idx)?;
        if leafmax < leaflen[i] {
            leafmax = leaflen[i];
        }
        i += 1;
    }

    let mut codeb = 0u32;
    let mut k = 0usize;
    let mut bitlen = 1u32;
    while bitlen <= leafmax {
        let mut j = 0usize;
        while j < num {
            if leaflen[j] == bitlen {
                h.table[k] = HuffEntry {
                    code: mirror(codeb, bitlen),
                    codelen: bitlen as u8,
                    value: j as u8,
                };
                codeb += 1;
                k += 1;
            }
            j += 1;
        }
        codeb <<= 1;
        bitlen += 1;
    }

    h.num = k;
    Some(())
}

fn huf_read(
    h: &HuffTable,
    bitstream: &mut BitStream,
    input: &[u8],
    input_idx: &mut usize,
) -> Option<u32> {
    let mut i = 0usize;
    while i < h.num {
        let mask = (1u32 << h.table[i].codelen) - 1;
        if bit_peek(bitstream, mask) == h.table[i].code {
            break;
        }
        i += 1;
    }

    if i == h.num {
        return None;
    }

    bit_advance(bitstream, h.table[i].codelen as i32, input, input_idx)?;

    let mut val = h.table[i].value as u32;
    if val >= 2 {
        let extra_mask = (1u32 << (val - 1)) - 1;
        val = 1 << (val - 1);
        val |= bit_read(
            bitstream,
            extra_mask,
            (h.table[i].value - 1) as i32,
            input,
            input_idx,
        )?;
    }

    Some(val)
}

fn depackrnc1_ulen_impl(packed: &[u8]) -> i32 {
    if read_be_u32(packed, 0) != Some(RNC_SIGNATURE) {
        return RNC_FILE_IS_NOT_RNC;
    }

    match read_be_u32(packed, 4) {
        Some(len) => len as i32,
        None => RNC_FILE_IS_NOT_RNC,
    }
}

fn depackrnc1_impl(packed: &[u8], unpacked: &mut [u8]) -> i32 {
    if packed.len() < 18 || read_be_u32(packed, 0) != Some(RNC_SIGNATURE) {
        return RNC_FILE_IS_NOT_RNC;
    }

    let ret_len = match read_be_u32(packed, 4) {
        Some(len) => len as usize,
        None => return RNC_FILE_IS_NOT_RNC,
    };
    let packed_len = match read_be_u32(packed, 8) {
        Some(len) => len as usize,
        None => return RNC_FILE_IS_NOT_RNC,
    };
    let out_crc = match read_be_u16(packed, 12) {
        Some(crc) => crc,
        None => return RNC_FILE_IS_NOT_RNC,
    };
    let packed_crc = match read_be_u16(packed, 14) {
        Some(crc) => crc,
        None => return RNC_FILE_IS_NOT_RNC,
    };

    let Some(input_end) = 18usize.checked_add(packed_len) else {
        return RNC_FILE_SIZE_MISMATCH;
    };
    if input_end > packed.len() {
        return RNC_FILE_SIZE_MISMATCH;
    }
    if unpacked.len() != ret_len {
        return RNC_FILE_SIZE_MISMATCH;
    }

    let input = &packed[18..input_end];
    if rnc_crc(input) != packed_crc {
        return RNC_PACKED_CRC_ERROR;
    }

    let mut bitstream = BitStream::default();
    let mut input_idx = 0usize;
    if bitread_init(&mut bitstream, input, input_idx).is_none() {
        return RNC_FILE_SIZE_MISMATCH;
    }
    if bit_advance(&mut bitstream, 2, input, &mut input_idx).is_none() {
        return RNC_FILE_SIZE_MISMATCH;
    }

    let mut raw = HuffTable::default();
    let mut dist = HuffTable::default();
    let mut len = HuffTable::default();
    let mut output_idx = 0usize;

    while output_idx < unpacked.len() {
        if read_huftable(&mut raw, &mut bitstream, input, &mut input_idx).is_none() {
            return RNC_HUF_DECODE_ERROR;
        }
        if read_huftable(&mut dist, &mut bitstream, input, &mut input_idx).is_none() {
            return RNC_HUF_DECODE_ERROR;
        }
        if read_huftable(&mut len, &mut bitstream, input, &mut input_idx).is_none() {
            return RNC_HUF_DECODE_ERROR;
        }

        let mut ch_count = match bit_read(&mut bitstream, 0xffff, 16, input, &mut input_idx) {
            Some(v) => v,
            None => return RNC_HUF_DECODE_ERROR,
        };

        loop {
            let length = match huf_read(&raw, &mut bitstream, input, &mut input_idx) {
                Some(v) => v as usize,
                None => return RNC_HUF_DECODE_ERROR,
            };
            if length != 0 {
                let copy_end = match output_idx.checked_add(length) {
                    Some(end) if end <= unpacked.len() => end,
                    _ => return RNC_FILE_SIZE_MISMATCH,
                };
                while output_idx < copy_end {
                    unpacked[output_idx] = input[input_idx];
                    output_idx += 1;
                    input_idx = match input_idx.checked_add(1) {
                        Some(next) if next <= input.len() => next,
                        _ => return RNC_FILE_SIZE_MISMATCH,
                    };
                }
                if bitread_fix(&mut bitstream, input, input_idx).is_none() {
                    return RNC_FILE_SIZE_MISMATCH;
                }
            }

            ch_count -= 1;
            if ch_count == 0 {
                break;
            }

            let posn = match huf_read(&dist, &mut bitstream, input, &mut input_idx) {
                Some(v) => v as usize,
                None => return RNC_HUF_DECODE_ERROR,
            };
            let length = match huf_read(&len, &mut bitstream, input, &mut input_idx) {
                Some(v) => v as usize,
                None => return RNC_HUF_DECODE_ERROR,
            };

            let copy_len = match (posn + 1).checked_add(0) {
                Some(_) => length + 2,
                None => return RNC_HUF_DECODE_ERROR,
            };
            let back = posn + 1;
            if back == 0 || back > output_idx {
                return RNC_FILE_SIZE_MISMATCH;
            }

            let mut copied = 0usize;
            while copied < copy_len {
                if output_idx >= unpacked.len() {
                    return RNC_FILE_SIZE_MISMATCH;
                }
                let src = output_idx - back;
                unpacked[output_idx] = unpacked[src];
                output_idx += 1;
                copied += 1;
            }
        }
    }

    if output_idx != unpacked.len() {
        return RNC_FILE_SIZE_MISMATCH;
    }

    if rnc_crc(unpacked) != out_crc {
        return RNC_UNPACKED_CRC_ERROR;
    }

    ret_len as i32
}

#[inline]
unsafe fn depackrnc1_from_raw(packed: *mut c_void, unpacked: *mut c_void) -> i32 {
    unsafe {
        if packed.is_null() || unpacked.is_null() {
            return RNC_FILE_IS_NOT_RNC;
        }

        let packed_ptr = packed.cast::<u8>();
        let header = slice::from_raw_parts(packed_ptr, 18);
        if read_be_u32(header, 0) != Some(RNC_SIGNATURE) {
            return RNC_FILE_IS_NOT_RNC;
        }

        let ret_len = match read_be_u32(header, 4) {
            Some(len) => len as usize,
            None => return RNC_FILE_IS_NOT_RNC,
        };
        let packed_len = match read_be_u32(header, 8) {
            Some(len) => len as usize,
            None => return RNC_FILE_IS_NOT_RNC,
        };
        let Some(total_len) = 18usize.checked_add(packed_len) else {
            return RNC_FILE_SIZE_MISMATCH;
        };

        let packed = slice::from_raw_parts(packed_ptr, total_len);
        let unpacked = slice::from_raw_parts_mut(unpacked.cast::<u8>(), ret_len);
        depackrnc1_impl(packed, unpacked)
    }
}

/// Returns the unpacked size declared by an RNC1 blob.
pub fn depackrnc1_unpacked_len(packed: &[u8]) -> i32 {
    depackrnc1_ulen_impl(packed)
}

/// # Safety
///
/// `packed` and `unpacked` must point to valid buffers matching the legacy
/// libogc `depackrnc1` contract.
pub unsafe fn depackrnc1(packed: *mut c_void, unpacked: *mut c_void) -> i32 {
    unsafe { depackrnc1_from_raw(packed, unpacked) }
}

unsafe fn depackrnc2_load(input: &mut *mut u8, carry: &mut u32, reg: &mut u32) -> bool {
    unsafe {
        *input = (*input).add(1);
        let mut v = { **input } as u32;
        v <<= 1;
        v |= *carry;
        *carry = (v >> 8) & 1;
        *reg = v & 0xff;
        *reg == 0
    }
}

unsafe fn depackrnc2_getbit(carry: &mut u32, reg: &mut u32) -> bool {
    let v = *reg << 1;
    *carry = (v >> 8) & 1;
    *reg = v & 0xff;
    *reg == 0
}

unsafe fn depackrnc2_copyraw(input: &mut *mut u8, output: &mut *mut u8) {
    unsafe {
        *input = (*input).add(1);
        *output = (*output).add(1);
        {
            **output = **input;
        }
    }
}

#[inline]
fn depackrnc2_rotate(value: &mut u32, carry: &mut u32) {
    let shifted = value.wrapping_shl(1) | *carry;
    *carry = (shifted >> 8) & 1;
    *value = shifted & 0xff;
}

#[derive(Copy, Clone)]
enum DepackRnc2Length {
    Raw,
    Copy(u32),
    ByteDistance(u32),
    End,
}

unsafe fn depackrnc2_decode_length(
    input: &mut *mut u8,
    carry: &mut u32,
    reg: &mut u32,
) -> DepackRnc2Length {
    unsafe {
        let mut y = 2u32;

        let zero = depackrnc2_getbit(carry, reg);
        if zero {
            let _ = depackrnc2_load(input, carry, reg);
        }

        if *carry == 0 {
            let zero = depackrnc2_getbit(carry, reg);
            if zero {
                let _ = depackrnc2_load(input, carry, reg);
            }

            depackrnc2_rotate(&mut y, carry);

            let zero = depackrnc2_getbit(carry, reg);
            if zero {
                let _ = depackrnc2_load(input, carry, reg);
            }
            if *carry == 0 {
                return DepackRnc2Length::Copy(y);
            }

            let zero = depackrnc2_getbit(carry, reg);
            if zero {
                let _ = depackrnc2_load(input, carry, reg);
            }

            y = y.wrapping_sub(1);
            depackrnc2_rotate(&mut y, carry);
            if y == 9 {
                DepackRnc2Length::Raw
            } else {
                DepackRnc2Length::Copy(y)
            }
        } else {
            let zero = depackrnc2_getbit(carry, reg);
            if zero {
                let _ = depackrnc2_load(input, carry, reg);
            }
            if *carry == 0 {
                return DepackRnc2Length::ByteDistance(y);
            }

            y = y.wrapping_add(1);
            let zero = depackrnc2_getbit(carry, reg);
            if zero {
                let _ = depackrnc2_load(input, carry, reg);
            }
            if *carry == 0 {
                return DepackRnc2Length::Copy(y);
            }

            *input = (*input).add(1);
            let length = { **input } as u32;
            if length == 0 {
                DepackRnc2Length::End
            } else {
                y = y.wrapping_add(8);
                DepackRnc2Length::Copy(y)
            }
        }
    }
}

unsafe fn depackrnc2_copy_match(input: &mut *mut u8, output: &mut *mut u8, length: u32, x: u32) {
    unsafe {
        *input = (*input).add(1);
        let displacement = { **input } as u32;
        let output_addr = *output as usize as u32;
        let source_addr = (output_addr & 0xff)
            .wrapping_sub(displacement)
            .wrapping_add(((output_addr >> 8).wrapping_sub(x)) << 8)
            .wrapping_sub(1);
        let mut source = source_addr as usize as *const u8;
        let mut remaining = length;
        while remaining != 0 {
            source = source.add(1);
            *output = (*output).add(1);
            {
                **output = *source;
            }
            remaining = remaining.wrapping_sub(1);
        }
    }
}

unsafe fn depackrnc2_from_raw(packed: *mut c_void, unpacked: *mut c_void) {
    unsafe {
        if packed.is_null() || unpacked.is_null() {
            return;
        }

        let mut input = packed.cast::<u8>().add(17);
        let mut output = unpacked.cast::<u8>().sub(1);
        let mut reg = 0u32;
        let mut carry = 1u32;
        let mut a = 0u32;
        let mut x = 0u32;
        let mut y: u32;

        let _ = depackrnc2_load(&mut input, &mut carry, &mut reg);
        let _ = depackrnc2_getbit(&mut carry, &mut reg);

        'xloop: loop {
            let _ = depackrnc2_getbit(&mut carry, &mut reg);
            if carry & 1 == 0 {
                depackrnc2_copyraw(&mut input, &mut output);
                let _ = depackrnc2_getbit(&mut carry, &mut reg);
                if carry & 1 != 0 {
                    carry = 1;
                } else {
                    depackrnc2_copyraw(&mut input, &mut output);
                    continue 'xloop;
                }
            }

            if a & 0xff == 0 {
                let _ = depackrnc2_load(&mut input, &mut carry, &mut reg);
                if carry != 0 {
                } else {
                    depackrnc2_copyraw(&mut input, &mut output);
                    continue 'xloop;
                }
            }

            match depackrnc2_decode_length(&mut input, &mut carry, &mut reg) {
                DepackRnc2Length::End => {
                    let zero = depackrnc2_getbit(&mut carry, &mut reg);
                    if zero {
                        let _ = depackrnc2_load(&mut input, &mut carry, &mut reg);
                    }
                    if carry != 0 {
                        continue 'xloop;
                    }
                    return;
                }
                DepackRnc2Length::Raw => {
                    y = 4;
                    while y != 0 {
                        let zero = depackrnc2_getbit(&mut carry, &mut reg);
                        if zero {
                            let _ = depackrnc2_load(&mut input, &mut carry, &mut reg);
                        }
                        depackrnc2_rotate(&mut x, &mut carry);
                        y = y.wrapping_sub(1);
                    }
                    y = x.wrapping_add(3);
                    depackrnc2_rotate(&mut y, &mut carry);
                    let mut remaining = y;
                    while remaining != 0 {
                        depackrnc2_copyraw(&mut input, &mut output);
                        depackrnc2_copyraw(&mut input, &mut output);
                        remaining = remaining.wrapping_sub(1);
                    }
                    continue 'xloop;
                }
                DepackRnc2Length::ByteDistance(length) => {
                    depackrnc2_copy_match(&mut input, &mut output, length, x);
                    continue 'xloop;
                }
                DepackRnc2Length::Copy(length) => {
                    y = length;
                }
            }

            let zero = depackrnc2_getbit(&mut carry, &mut reg);
            if zero {
                let _ = depackrnc2_load(&mut input, &mut carry, &mut reg);
            }

            if carry == 0 {
                depackrnc2_copy_match(&mut input, &mut output, y, x);
                continue 'xloop;
            }

            let zero = depackrnc2_getbit(&mut carry, &mut reg);
            if zero {
                let _ = depackrnc2_load(&mut input, &mut carry, &mut reg);
            }

            depackrnc2_rotate(&mut x, &mut carry);
            depackrnc2_rotate(&mut a, &mut carry);
            if a & 0xff == 0 {
                let _ = depackrnc2_load(&mut input, &mut carry, &mut reg);
            }

            if carry != 0 {
                let zero = depackrnc2_getbit(&mut carry, &mut reg);
                if zero {
                    let _ = depackrnc2_load(&mut input, &mut carry, &mut reg);
                }
                depackrnc2_rotate(&mut x, &mut carry);
                x |= 4;

                let zero = depackrnc2_getbit(&mut carry, &mut reg);
                if zero {
                    let _ = depackrnc2_load(&mut input, &mut carry, &mut reg);
                }
                if carry == 0 {
                    let zero = depackrnc2_getbit(&mut carry, &mut reg);
                    if zero {
                        let _ = depackrnc2_load(&mut input, &mut carry, &mut reg);
                    }
                    depackrnc2_rotate(&mut x, &mut carry);
                }
            } else if x == 0 {
                x = x.wrapping_add(1);
                let zero = depackrnc2_getbit(&mut carry, &mut reg);
                if zero {
                    let _ = depackrnc2_load(&mut input, &mut carry, &mut reg);
                }
                depackrnc2_rotate(&mut x, &mut carry);
            }

            depackrnc2_copy_match(&mut input, &mut output, y, x);
        }
    }
}

/// # Safety
///
/// `packed` and `unpacked` must point to valid buffers matching the legacy
/// libogc `depackrnc2` contract.
pub unsafe fn depackrnc2(packed: *mut c_void, unpacked: *mut c_void) {
    unsafe { depackrnc2_from_raw(packed, unpacked) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn valid_empty_rnc1_blob() -> [u8; 24] {
        [
            0x52, 0x4e, 0x43, 0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x06, 0x00, 0x00,
            0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        ]
    }

    fn valid_empty_rnc2_blob() -> [u8; 22] {
        [
            0x52, 0x4e, 0x43, 0x02, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x04, 0x00, 0x00,
            0xf8, 0x21, 0x00, 0x00, 0xff, 0xf1, 0x00, 0x2b,
        ]
    }

    fn valid_raw_rnc2_blob() -> [u8; 37] {
        [
            0x52, 0x4e, 0x43, 0x02, 0x00, 0x00, 0x00, 0x0a, 0x00, 0x00, 0x00, 0x13, 0xca, 0xbf,
            0x5c, 0x34, 0x00, 0x00, 0x67, 0x04, 0xf9, 0x91, 0x9b, 0xa5, 0x8e, 0x79, 0x4f, 0x34,
            0xa7, 0x2d, 0x30, 0x16, 0xde, 0xf7, 0x00, 0xa5, 0x73,
        ]
    }

    #[test]
    fn bitstream_reads_only_16_bits_at_init_and_refill() {
        let input = [0x34, 0x12, 0x78, 0x56, 0xbc, 0x9a];
        let mut bitstream = BitStream::default();
        let mut input_idx = 0;

        bitread_init(&mut bitstream, &input, input_idx).unwrap();
        assert_eq!(bitstream.bitbuf, 0x1234);
        assert_eq!(bitstream.bitcount, 16);

        bit_advance(&mut bitstream, 16, &input, &mut input_idx).unwrap();
        assert_eq!(input_idx, 2);
        assert_eq!(bitstream.bitbuf, 0x5678);
        assert_eq!(bitstream.bitcount, 16);
    }

    #[test]
    fn memory_helpers_match_c_address_macros() {
        let physical = 0x0123_4567;
        let k0 = 0x8123_4567usize;
        let k1 = 0xc123_4567usize;

        assert_eq!(mem_virtual_to_physical(k0), physical);
        assert_eq!(mem_virtual_to_physical(k1), physical);
        assert_eq!(mem_physical_to_k0(physical), k0);
        assert_eq!(mem_physical_to_k1(physical), k1);
        assert_eq!(mem_k0_to_physical(k0), physical);
        assert_eq!(mem_k1_to_physical(k1), physical);
        assert_eq!(mem_k0_to_k1(k0), k1);
        assert_eq!(mem_k1_to_k0(k1), k0);
    }

    #[test]
    fn unpacked_len_reports_header_value() {
        let packed = valid_empty_rnc1_blob();
        assert_eq!(depackrnc1_unpacked_len(&packed), 0);
        assert_eq!(
            unsafe { sys::depackrnc1_ulen(packed.as_ptr() as *mut _) },
            0
        );
    }

    #[test]
    fn unpacked_len_rejects_truncated_headers() {
        let packed = valid_empty_rnc1_blob();
        for len in 0..8 {
            assert_eq!(depackrnc1_unpacked_len(&packed[..len]), RNC_FILE_IS_NOT_RNC);
        }
    }

    #[test]
    fn valid_empty_blob_decompresses() {
        let packed = valid_empty_rnc1_blob();
        let mut unpacked = [];
        assert_eq!(depackrnc1_impl(&packed, &mut unpacked), 0);
        assert_eq!(
            unsafe { depackrnc1(packed.as_ptr() as *mut _, unpacked.as_mut_ptr().cast()) },
            0
        );
        assert_eq!(
            unsafe { sys::depackrnc1(packed.as_ptr() as *mut _, unpacked.as_mut_ptr().cast()) },
            0
        );
    }

    #[test]
    fn empty_rnc2_stream_stops_at_end_marker() {
        let mut packed = valid_empty_rnc2_blob();
        let mut unpacked = [0xa5u8; 1];
        let output = unsafe { unpacked.as_mut_ptr().add(1) };

        unsafe {
            depackrnc2(packed.as_mut_ptr().cast(), output.cast());
        }
        assert_eq!(unpacked, [0xa5]);

        let mut packed = valid_empty_rnc2_blob();
        let mut unpacked = [0xa5u8; 1];
        let output = unsafe { unpacked.as_mut_ptr().add(1) };
        unsafe {
            sys::depackrnc2(packed.as_mut_ptr().cast(), output.cast());
        }
        assert_eq!(unpacked, [0xa5]);
    }

    #[test]
    fn rnc2_stream_copies_raw_block_and_stops() {
        let mut packed = valid_raw_rnc2_blob();
        assert_eq!(rnc_crc(&packed[18..]), read_be_u16(&packed, 14).unwrap());
        let mut unpacked = [0xa5u8; 11];
        let output = unsafe { unpacked.as_mut_ptr().add(1) };

        unsafe {
            depackrnc2(packed.as_mut_ptr().cast(), output.cast());
        }
        assert_eq!(rnc_crc(&unpacked[1..]), read_be_u16(&packed, 12).unwrap());
        assert_eq!(
            &unpacked[1..],
            &[0xf9, 0x91, 0x9b, 0xa5, 0x8e, 0x4f, 0xa7, 0x2d, 0x16, 0xde]
        );

        let mut packed = valid_raw_rnc2_blob();
        let mut unpacked = [0xa5u8; 11];
        let output = unsafe { unpacked.as_mut_ptr().add(1) };
        unsafe {
            sys::depackrnc2(packed.as_mut_ptr().cast(), output.cast());
        }
        assert_eq!(rnc_crc(&unpacked[1..]), read_be_u16(&packed, 12).unwrap());
        assert_eq!(
            &unpacked[1..],
            &[0xf9, 0x91, 0x9b, 0xa5, 0x8e, 0x4f, 0xa7, 0x2d, 0x16, 0xde]
        );
    }

    #[test]
    fn invalid_magic_is_rejected() {
        let mut packed = valid_empty_rnc1_blob();
        packed[0] = 0;
        let mut unpacked = [];
        assert_eq!(depackrnc1_impl(&packed, &mut unpacked), RNC_FILE_IS_NOT_RNC);
    }

    #[test]
    fn truncated_input_is_rejected() {
        let packed = &valid_empty_rnc1_blob()[..10];
        let mut unpacked = [];
        assert_eq!(depackrnc1_impl(packed, &mut unpacked), RNC_FILE_IS_NOT_RNC);
    }

    #[test]
    fn bad_checksum_is_rejected() {
        let mut packed = valid_empty_rnc1_blob();
        packed[18] = 1;
        let mut unpacked = [];
        assert_eq!(
            depackrnc1_impl(&packed, &mut unpacked),
            RNC_PACKED_CRC_ERROR
        );
    }

    #[test]
    fn small_output_buffer_is_rejected() {
        let packed = valid_empty_rnc1_blob();
        let mut unpacked = [0u8; 1];
        assert_eq!(
            depackrnc1_impl(&packed, &mut unpacked),
            RNC_FILE_SIZE_MISMATCH
        );
    }
}

/// Shared PowerPC primitives used by low-level runtime and hardware drivers.
///
/// These functions expose architectural facts only. They do not initialize a
/// scheduler, own callbacks, or maintain a host-side model.
#[cfg(target_arch = "powerpc")]
pub mod powerpc {
    use core::arch::asm;

    /// Reads the 64-bit PowerPC timebase using the stable upper/lower/upper
    /// sequence required when the lower half can roll over during the read.
    #[inline(always)]
    pub fn read_timebase() -> u64 {
        loop {
            let upper_before: u32;
            let lower: u32;
            let upper_after: u32;
            unsafe {
                asm!(
                    "mftbu {upper_before}",
                    "mftb {lower}",
                    "mftbu {upper_after}",
                    upper_before = out(reg) upper_before,
                    lower = out(reg) lower,
                    upper_after = out(reg) upper_after,
                    options(nostack, nomem)
                );
            }
            if upper_before == upper_after {
                return ((upper_before as u64) << 32) | lower as u64;
            }
        }
    }

    /// Reads the Hollywood revision published by IOS in Wii low memory.
    #[inline]
    pub fn hollywood_revision() -> u32 {
        const HOLLYWOOD_REVISION_ADDR: usize = 0x8000_3138;
        unsafe {
            asm!(
                "dcbi 0, {address}",
                "sync",
                address = in(reg) HOLLYWOOD_REVISION_ADDR,
                options(nostack)
            );
            core::ptr::read_volatile(HOLLYWOOD_REVISION_ADDR as *const u32)
        }
    }
}
