#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::sync::atomic::{AtomicBool, Ordering};

pub const SHA_MSGBLOCK_SIZE: u32 = 0x10000;

static SHA_INITIALIZED: AtomicBool = AtomicBool::new(false);
const SHA1_ABC: [u8; 20] = [
    0xa9, 0x99, 0x3e, 0x36, 0x47, 0x06, 0x81, 0x6a, 0xba, 0x3e, 0x25, 0x71, 0x78, 0x50, 0xc2, 0x6c,
    0x9c, 0xd0, 0xd8, 0x9d,
];

#[repr(C)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ShaContext {
    pub states: [u32; 5],
    pub upper_length: u32,
    pub lower_length: u32,
}

impl Default for ShaContext {
    fn default() -> Self {
        Self::new()
    }
}

impl ShaContext {
    pub const fn new() -> Self {
        Self {
            states: [
                0x6745_2301,
                0xEFCD_AB89,
                0x98BA_DCFE,
                0x1032_5476,
                0xC3D2_E1F0,
            ],
            upper_length: 0,
            lower_length: 0,
        }
    }

    pub const fn bit_length(self) -> u64 {
        ((self.upper_length as u64) << 32) | self.lower_length as u64
    }

    pub fn set_bit_length(&mut self, bit_length: u64) {
        self.upper_length = (bit_length >> 32) as u32;
        self.lower_length = bit_length as u32;
    }
}

#[repr(C, align(32))]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct AlignedShaContext {
    pub context: ShaContext,
}

impl Default for AlignedShaContext {
    fn default() -> Self {
        Self::new()
    }
}

impl AlignedShaContext {
    pub const fn new() -> Self {
        Self {
            context: ShaContext::new(),
        }
    }

    pub fn as_ptr(&self) -> *const ShaContext {
        &self.context
    }

    pub fn as_mut_ptr(&mut self) -> *mut ShaContext {
        &mut self.context
    }
}

#[inline(always)]
fn compress(state: &mut [u32; 5], block: &[u8; 64]) {
    let mut w = [0u32; 80];
    let mut i = 0;
    while i < 16 {
        let j = i * 4;
        w[i] = u32::from_be_bytes([block[j], block[j + 1], block[j + 2], block[j + 3]]);
        i += 1;
    }

    while i < 80 {
        w[i] = (w[i - 3] ^ w[i - 8] ^ w[i - 14] ^ w[i - 16]).rotate_left(1);
        i += 1;
    }

    let mut a = state[0];
    let mut b = state[1];
    let mut c = state[2];
    let mut d = state[3];
    let mut e = state[4];

    let mut round = 0;
    while round < 80 {
        let (f, k) = match round {
            0..=19 => ((b & c) | ((!b) & d), 0x5A82_7999),
            20..=39 => (b ^ c ^ d, 0x6ED9_EBA1),
            40..=59 => ((b & c) | (b & d) | (c & d), 0x8F1B_BCDC),
            _ => (b ^ c ^ d, 0xCA62_C1D6),
        };

        let temp = a
            .rotate_left(5)
            .wrapping_add(f)
            .wrapping_add(e)
            .wrapping_add(k)
            .wrapping_add(w[round]);
        e = d;
        d = c;
        c = b.rotate_left(30);
        b = a;
        a = temp;
        round += 1;
    }

    state[0] = state[0].wrapping_add(a);
    state[1] = state[1].wrapping_add(b);
    state[2] = state[2].wrapping_add(c);
    state[3] = state[3].wrapping_add(d);
    state[4] = state[4].wrapping_add(e);
}

#[inline(always)]
fn process_full_blocks(state: &mut [u32; 5], data: &[u8]) {
    for block in data.as_chunks::<64>().0 {
        compress(state, block);
    }
}

fn finalize_from_state(mut state: [u32; 5], bit_length: u64, data: &[u8], digest: &mut [u8; 20]) {
    process_full_blocks(&mut state, data);

    let remainder = data.as_chunks::<64>().1;
    let mut block = [0u8; 128];
    block[..remainder.len()].copy_from_slice(remainder);
    block[remainder.len()] = 0x80;

    let pad_end = if remainder.len() < 56 { 56 } else { 120 };
    let bit_length = bit_length.wrapping_add((data.len() as u64) << 3);
    block[pad_end..pad_end + 8].copy_from_slice(&bit_length.to_be_bytes());

    let mut first = [0u8; 64];
    first.copy_from_slice(&block[..64]);
    compress(&mut state, &first);

    if remainder.len() >= 56 {
        let mut second = [0u8; 64];
        second.copy_from_slice(&block[64..128]);
        compress(&mut state, &second);
    }

    for (idx, word) in state.iter().enumerate() {
        digest[idx * 4..idx * 4 + 4].copy_from_slice(&word.to_be_bytes());
    }
}

pub fn init() -> i32 {
    if SHA_INITIALIZED.load(Ordering::Acquire) {
        return 0;
    }

    // Verify the software compression path before the lifecycle becomes active.
    let mut digest = [0u8; 20];
    finalize_from_state(ShaContext::new().states, 0, b"abc", &mut digest);
    if digest != SHA1_ABC {
        return -1;
    }

    SHA_INITIALIZED.store(true, Ordering::Release);
    0
}

pub fn close() -> i32 {
    if SHA_INITIALIZED.swap(false, Ordering::AcqRel) {
        0
    } else {
        -1
    }
}

pub fn calculate(data: &[u8], message_digest: &mut [u8; 20]) -> i32 {
    if !SHA_INITIALIZED.load(Ordering::Acquire) || data.is_empty() {
        return -1;
    }
    finalize_from_state(ShaContext::new().states, 0, data, message_digest);
    0
}

pub fn initialize_context(context: &mut AlignedShaContext) -> i32 {
    if !SHA_INITIALIZED.load(Ordering::Acquire) {
        return -1;
    }
    context.context = ShaContext::new();
    0
}

pub fn input(context: &mut AlignedShaContext, data: &[u8]) -> i32 {
    if !SHA_INITIALIZED.load(Ordering::Acquire) || data.is_empty() {
        return -1;
    }
    if data.len() & 0x3f != 0 {
        return -4;
    }

    process_full_blocks(&mut context.context.states, data);
    context.context.set_bit_length(
        context
            .context
            .bit_length()
            .wrapping_add((data.len() as u64) << 3),
    );
    0
}

pub fn finalize(
    context: &mut AlignedShaContext,
    data: &[u8],
    message_digest: &mut [u8; 20],
) -> i32 {
    if !SHA_INITIALIZED.load(Ordering::Acquire) || data.is_empty() {
        return -1;
    }

    finalize_from_state(
        context.context.states,
        context.context.bit_length(),
        data,
        message_digest,
    );
    0
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    fn hex(bytes: &[u8]) -> [u8; 40] {
        const TABLE: &[u8; 16] = b"0123456789abcdef";
        let mut out = [0u8; 40];
        let mut i = 0;
        while i < bytes.len() {
            out[i * 2] = TABLE[(bytes[i] >> 4) as usize];
            out[i * 2 + 1] = TABLE[(bytes[i] & 0x0f) as usize];
            i += 1;
        }
        out
    }

    #[test]
    fn context_layout_matches_c() {
        assert_eq!(core::mem::size_of::<ShaContext>(), 28);
        assert_eq!(core::mem::align_of::<ShaContext>(), 4);
        assert_eq!(core::mem::size_of::<AlignedShaContext>(), 32);
        assert_eq!(core::mem::align_of::<AlignedShaContext>(), 32);
        assert_eq!(SHA_MSGBLOCK_SIZE, 0x10000);
    }

    #[test]
    fn sha1_known_vectors() {
        let mut digest = [0u8; 20];
        assert_eq!(init(), 0);
        assert_eq!(calculate(b"", &mut digest), -1);

        assert_eq!(calculate(b"abc", &mut digest), 0);
        assert_eq!(hex(&digest), *b"a9993e364706816aba3e25717850c26c9cd0d89d");

        let mut context = AlignedShaContext::new();
        assert_eq!(context.context, ShaContext::new());
        assert_eq!(context.as_ptr(), &context.context as *const ShaContext);
        assert_eq!(
            context.as_mut_ptr(),
            &mut context.context as *mut ShaContext
        );
        assert_eq!(initialize_context(&mut context), 0);

        let full_block = [b'a'; 64];
        assert_eq!(input(&mut context, &full_block), 0);
        assert_eq!(context.context.bit_length(), 512);
        assert_eq!(finalize(&mut context, b"x", &mut digest), 0);

        let mut expected = [0u8; 20];
        let mut message = [b'a'; 65];
        message[64] = b'x';
        assert_eq!(calculate(&message, &mut expected), 0);
        assert_eq!(digest, expected);

        let mut plain_context = ShaContext::new();
        plain_context.set_bit_length(u64::MAX);
        assert_eq!(plain_context.bit_length(), u64::MAX);
        assert_eq!(close(), 0);
        assert_eq!(calculate(b"abc", &mut digest), -1);
        assert_eq!(close(), -1);
    }

    #[test]
    fn helper_functions_exist_without_backend() {
        let _ = init as fn() -> i32;
        let _ = close as fn() -> i32;
        let _ = calculate as fn(&[u8], &mut [u8; 20]) -> i32;
        let _ = initialize_context as fn(&mut AlignedShaContext) -> i32;
        let _ = input as fn(&mut AlignedShaContext, &[u8]) -> i32;
        let _ = finalize as fn(&mut AlignedShaContext, &[u8], &mut [u8; 20]) -> i32;
    }
}
