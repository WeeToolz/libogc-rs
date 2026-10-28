#[derive(Copy, Clone)]
struct DesCtx {
    encrypt_subkeys: [u32; 32],
}

fn md4_digest(data: &[u8]) -> [u8; 16] {
    let mut a: u32 = 0x6745_2301;
    let mut b: u32 = 0xefcd_ab89;
    let mut c: u32 = 0x98ba_dcfe;
    let mut d: u32 = 0x1032_5476;

    let (chunks, rem) = data.as_chunks::<64>();
    for chunk in chunks {
        let mut x = [0u32; 16];
        let mut i = 0usize;
        while i < 16 {
            let j = i * 4;
            x[i] = u32::from_le_bytes([chunk[j], chunk[j + 1], chunk[j + 2], chunk[j + 3]]);
            i += 1;
        }
        md4_process_block(&mut a, &mut b, &mut c, &mut d, &x);
    }

    let len = data.len() as u64;
    let mut tail = [0u8; 128];
    tail[..rem.len()].copy_from_slice(rem);
    tail[rem.len()] = 0x80;
    let final_len = if rem.len() < 56 { 64 } else { 128 };
    tail[final_len - 8..final_len].copy_from_slice(&(len << 3).to_le_bytes());

    let mut offset = 0usize;
    while offset < final_len {
        let mut x = [0u32; 16];
        let mut i = 0usize;
        while i < 16 {
            let j = offset + i * 4;
            x[i] = u32::from_le_bytes([tail[j], tail[j + 1], tail[j + 2], tail[j + 3]]);
            i += 1;
        }
        md4_process_block(&mut a, &mut b, &mut c, &mut d, &x);
        offset += 64;
    }

    let mut out = [0u8; 16];
    out[0..4].copy_from_slice(&a.to_le_bytes());
    out[4..8].copy_from_slice(&b.to_le_bytes());
    out[8..12].copy_from_slice(&c.to_le_bytes());
    out[12..16].copy_from_slice(&d.to_le_bytes());
    out
}

fn md4_process_block(a: &mut u32, b: &mut u32, c: &mut u32, d: &mut u32, x: &[u32; 16]) {
    #[inline]
    fn f(x: u32, y: u32, z: u32) -> u32 {
        z ^ (x & (y ^ z))
    }
    #[inline]
    fn g(x: u32, y: u32, z: u32) -> u32 {
        (x & y) | (x & z) | (y & z)
    }
    #[inline]
    fn h(x: u32, y: u32, z: u32) -> u32 {
        x ^ y ^ z
    }

    let (mut aa, mut bb, mut cc, mut dd) = (*a, *b, *c, *d);

    aa = aa
        .wrapping_add(f(bb, cc, dd))
        .wrapping_add(x[0])
        .rotate_left(3);
    dd = dd
        .wrapping_add(f(aa, bb, cc))
        .wrapping_add(x[1])
        .rotate_left(7);
    cc = cc
        .wrapping_add(f(dd, aa, bb))
        .wrapping_add(x[2])
        .rotate_left(11);
    bb = bb
        .wrapping_add(f(cc, dd, aa))
        .wrapping_add(x[3])
        .rotate_left(19);
    aa = aa
        .wrapping_add(f(bb, cc, dd))
        .wrapping_add(x[4])
        .rotate_left(3);
    dd = dd
        .wrapping_add(f(aa, bb, cc))
        .wrapping_add(x[5])
        .rotate_left(7);
    cc = cc
        .wrapping_add(f(dd, aa, bb))
        .wrapping_add(x[6])
        .rotate_left(11);
    bb = bb
        .wrapping_add(f(cc, dd, aa))
        .wrapping_add(x[7])
        .rotate_left(19);
    aa = aa
        .wrapping_add(f(bb, cc, dd))
        .wrapping_add(x[8])
        .rotate_left(3);
    dd = dd
        .wrapping_add(f(aa, bb, cc))
        .wrapping_add(x[9])
        .rotate_left(7);
    cc = cc
        .wrapping_add(f(dd, aa, bb))
        .wrapping_add(x[10])
        .rotate_left(11);
    bb = bb
        .wrapping_add(f(cc, dd, aa))
        .wrapping_add(x[11])
        .rotate_left(19);
    aa = aa
        .wrapping_add(f(bb, cc, dd))
        .wrapping_add(x[12])
        .rotate_left(3);
    dd = dd
        .wrapping_add(f(aa, bb, cc))
        .wrapping_add(x[13])
        .rotate_left(7);
    cc = cc
        .wrapping_add(f(dd, aa, bb))
        .wrapping_add(x[14])
        .rotate_left(11);
    bb = bb
        .wrapping_add(f(cc, dd, aa))
        .wrapping_add(x[15])
        .rotate_left(19);

    aa = aa
        .wrapping_add(g(bb, cc, dd))
        .wrapping_add(x[0])
        .wrapping_add(0x5a82_7999)
        .rotate_left(3);
    dd = dd
        .wrapping_add(g(aa, bb, cc))
        .wrapping_add(x[4])
        .wrapping_add(0x5a82_7999)
        .rotate_left(5);
    cc = cc
        .wrapping_add(g(dd, aa, bb))
        .wrapping_add(x[8])
        .wrapping_add(0x5a82_7999)
        .rotate_left(9);
    bb = bb
        .wrapping_add(g(cc, dd, aa))
        .wrapping_add(x[12])
        .wrapping_add(0x5a82_7999)
        .rotate_left(13);
    aa = aa
        .wrapping_add(g(bb, cc, dd))
        .wrapping_add(x[1])
        .wrapping_add(0x5a82_7999)
        .rotate_left(3);
    dd = dd
        .wrapping_add(g(aa, bb, cc))
        .wrapping_add(x[5])
        .wrapping_add(0x5a82_7999)
        .rotate_left(5);
    cc = cc
        .wrapping_add(g(dd, aa, bb))
        .wrapping_add(x[9])
        .wrapping_add(0x5a82_7999)
        .rotate_left(9);
    bb = bb
        .wrapping_add(g(cc, dd, aa))
        .wrapping_add(x[13])
        .wrapping_add(0x5a82_7999)
        .rotate_left(13);
    aa = aa
        .wrapping_add(g(bb, cc, dd))
        .wrapping_add(x[2])
        .wrapping_add(0x5a82_7999)
        .rotate_left(3);
    dd = dd
        .wrapping_add(g(aa, bb, cc))
        .wrapping_add(x[6])
        .wrapping_add(0x5a82_7999)
        .rotate_left(5);
    cc = cc
        .wrapping_add(g(dd, aa, bb))
        .wrapping_add(x[10])
        .wrapping_add(0x5a82_7999)
        .rotate_left(9);
    bb = bb
        .wrapping_add(g(cc, dd, aa))
        .wrapping_add(x[14])
        .wrapping_add(0x5a82_7999)
        .rotate_left(13);
    aa = aa
        .wrapping_add(g(bb, cc, dd))
        .wrapping_add(x[3])
        .wrapping_add(0x5a82_7999)
        .rotate_left(3);
    dd = dd
        .wrapping_add(g(aa, bb, cc))
        .wrapping_add(x[7])
        .wrapping_add(0x5a82_7999)
        .rotate_left(5);
    cc = cc
        .wrapping_add(g(dd, aa, bb))
        .wrapping_add(x[11])
        .wrapping_add(0x5a82_7999)
        .rotate_left(9);
    bb = bb
        .wrapping_add(g(cc, dd, aa))
        .wrapping_add(x[15])
        .wrapping_add(0x5a82_7999)
        .rotate_left(13);

    aa = aa
        .wrapping_add(h(bb, cc, dd))
        .wrapping_add(x[0])
        .wrapping_add(0x6ed9_eba1)
        .rotate_left(3);
    dd = dd
        .wrapping_add(h(aa, bb, cc))
        .wrapping_add(x[8])
        .wrapping_add(0x6ed9_eba1)
        .rotate_left(9);
    cc = cc
        .wrapping_add(h(dd, aa, bb))
        .wrapping_add(x[4])
        .wrapping_add(0x6ed9_eba1)
        .rotate_left(11);
    bb = bb
        .wrapping_add(h(cc, dd, aa))
        .wrapping_add(x[12])
        .wrapping_add(0x6ed9_eba1)
        .rotate_left(15);
    aa = aa
        .wrapping_add(h(bb, cc, dd))
        .wrapping_add(x[2])
        .wrapping_add(0x6ed9_eba1)
        .rotate_left(3);
    dd = dd
        .wrapping_add(h(aa, bb, cc))
        .wrapping_add(x[10])
        .wrapping_add(0x6ed9_eba1)
        .rotate_left(9);
    cc = cc
        .wrapping_add(h(dd, aa, bb))
        .wrapping_add(x[6])
        .wrapping_add(0x6ed9_eba1)
        .rotate_left(11);
    bb = bb
        .wrapping_add(h(cc, dd, aa))
        .wrapping_add(x[14])
        .wrapping_add(0x6ed9_eba1)
        .rotate_left(15);
    aa = aa
        .wrapping_add(h(bb, cc, dd))
        .wrapping_add(x[1])
        .wrapping_add(0x6ed9_eba1)
        .rotate_left(3);
    dd = dd
        .wrapping_add(h(aa, bb, cc))
        .wrapping_add(x[9])
        .wrapping_add(0x6ed9_eba1)
        .rotate_left(9);
    cc = cc
        .wrapping_add(h(dd, aa, bb))
        .wrapping_add(x[5])
        .wrapping_add(0x6ed9_eba1)
        .rotate_left(11);
    bb = bb
        .wrapping_add(h(cc, dd, aa))
        .wrapping_add(x[13])
        .wrapping_add(0x6ed9_eba1)
        .rotate_left(15);
    aa = aa
        .wrapping_add(h(bb, cc, dd))
        .wrapping_add(x[3])
        .wrapping_add(0x6ed9_eba1)
        .rotate_left(3);
    dd = dd
        .wrapping_add(h(aa, bb, cc))
        .wrapping_add(x[11])
        .wrapping_add(0x6ed9_eba1)
        .rotate_left(9);
    cc = cc
        .wrapping_add(h(dd, aa, bb))
        .wrapping_add(x[7])
        .wrapping_add(0x6ed9_eba1)
        .rotate_left(11);
    bb = bb
        .wrapping_add(h(cc, dd, aa))
        .wrapping_add(x[15])
        .wrapping_add(0x6ed9_eba1)
        .rotate_left(15);

    *a = a.wrapping_add(aa);
    *b = b.wrapping_add(bb);
    *c = c.wrapping_add(cc);
    *d = d.wrapping_add(dd);
}

fn ntlm_convert_key(key_56: &[u8; 7]) -> [u8; 8] {
    [
        key_56[0],
        (key_56[0] << 7) | (key_56[1] >> 1),
        (key_56[1] << 6) | (key_56[2] >> 2),
        (key_56[2] << 5) | (key_56[3] >> 3),
        (key_56[3] << 4) | (key_56[4] >> 4),
        (key_56[4] << 3) | (key_56[5] >> 5),
        (key_56[5] << 2) | (key_56[6] >> 6),
        key_56[6] << 1,
    ]
}

fn des_key_schedule(rawkey: &[u8; 8]) -> DesCtx {
    const LEFTKEY_SWAP: [u32; 16] = [
        0x00000000, 0x00000001, 0x00000100, 0x00000101, 0x00010000, 0x00010001, 0x00010100,
        0x00010101, 0x01000000, 0x01000001, 0x01000100, 0x01000101, 0x01010000, 0x01010001,
        0x01010100, 0x01010101,
    ];
    const RIGHTKEY_SWAP: [u32; 16] = [
        0x00000000, 0x01000000, 0x00010000, 0x01010000, 0x00000100, 0x01000100, 0x00010100,
        0x01010100, 0x00000001, 0x01000001, 0x00010001, 0x01010001, 0x00000101, 0x01000101,
        0x00010101, 0x01010101,
    ];
    const ENCRYPT_ROTATE_TAB: [u32; 16] = [1, 1, 2, 2, 2, 2, 2, 2, 1, 2, 2, 2, 2, 2, 2, 1];

    let mut left = u32::from_be_bytes([rawkey[0], rawkey[1], rawkey[2], rawkey[3]]);
    let mut right = u32::from_be_bytes([rawkey[4], rawkey[5], rawkey[6], rawkey[7]]);
    let mut work = 0u32;

    do_permutation(&mut right, &mut work, &mut left, 4, 0x0f0f0f0f);
    do_permutation(&mut right, &mut work, &mut left, 0, 0x10101010);
    left = ((LEFTKEY_SWAP[left as usize & 0xf] << 3)
        | (LEFTKEY_SWAP[(left >> 8) as usize & 0xf] << 2)
        | (LEFTKEY_SWAP[(left >> 16) as usize & 0xf] << 1)
        | (LEFTKEY_SWAP[(left >> 24) as usize & 0xf])
        | (LEFTKEY_SWAP[(left >> 5) as usize & 0xf] << 7)
        | (LEFTKEY_SWAP[(left >> 13) as usize & 0xf] << 6)
        | (LEFTKEY_SWAP[(left >> 21) as usize & 0xf] << 5)
        | (LEFTKEY_SWAP[(left >> 29) as usize & 0xf] << 4))
        & 0x0fff_ffff;

    right = ((RIGHTKEY_SWAP[(right >> 1) as usize & 0xf] << 3)
        | (RIGHTKEY_SWAP[(right >> 9) as usize & 0xf] << 2)
        | (RIGHTKEY_SWAP[(right >> 17) as usize & 0xf] << 1)
        | (RIGHTKEY_SWAP[(right >> 25) as usize & 0xf])
        | (RIGHTKEY_SWAP[(right >> 4) as usize & 0xf] << 7)
        | (RIGHTKEY_SWAP[(right >> 12) as usize & 0xf] << 6)
        | (RIGHTKEY_SWAP[(right >> 20) as usize & 0xf] << 5)
        | (RIGHTKEY_SWAP[(right >> 28) as usize & 0xf] << 4))
        & 0x0fff_ffff;

    let mut encrypt_subkeys = [0u32; 32];
    let mut idx = 0usize;
    let mut round = 0usize;
    while round < 16 {
        let rot = ENCRYPT_ROTATE_TAB[round];
        left = ((left << rot) | (left >> (28 - rot))) & 0x0fff_ffff;
        right = ((right << rot) | (right >> (28 - rot))) & 0x0fff_ffff;

        encrypt_subkeys[idx] = ((left << 4) & 0x2400_0000)
            | ((left << 28) & 0x1000_0000)
            | ((left << 14) & 0x0800_0000)
            | ((left << 18) & 0x0208_0000)
            | ((left << 6) & 0x0100_0000)
            | ((left << 9) & 0x0020_0000)
            | ((left >> 1) & 0x0010_0000)
            | ((left << 10) & 0x0004_0000)
            | ((left << 2) & 0x0002_0000)
            | ((left >> 10) & 0x0001_0000)
            | ((right >> 13) & 0x0000_2000)
            | ((right >> 4) & 0x0000_1000)
            | ((right << 6) & 0x0000_0800)
            | ((right >> 1) & 0x0000_0400)
            | ((right >> 14) & 0x0000_0200)
            | (right & 0x0000_0100)
            | ((right >> 5) & 0x0000_0020)
            | ((right >> 10) & 0x0000_0010)
            | ((right >> 3) & 0x0000_0008)
            | ((right >> 18) & 0x0000_0004)
            | ((right >> 26) & 0x0000_0002)
            | ((right >> 24) & 0x0000_0001);
        idx += 1;

        encrypt_subkeys[idx] = ((left << 15) & 0x2000_0000)
            | ((left << 17) & 0x1000_0000)
            | ((left << 10) & 0x0800_0000)
            | ((left << 22) & 0x0400_0000)
            | ((left >> 2) & 0x0200_0000)
            | ((left << 1) & 0x0100_0000)
            | ((left << 16) & 0x0020_0000)
            | ((left << 11) & 0x0010_0000)
            | ((left << 3) & 0x0008_0000)
            | ((left >> 6) & 0x0004_0000)
            | ((left << 15) & 0x0002_0000)
            | ((left >> 4) & 0x0001_0000)
            | ((right >> 2) & 0x0000_2000)
            | ((right << 8) & 0x0000_1000)
            | ((right >> 14) & 0x0000_0808)
            | ((right >> 9) & 0x0000_0400)
            | (right & 0x0000_0200)
            | ((right << 7) & 0x0000_0100)
            | ((right >> 7) & 0x0000_0020)
            | ((right >> 3) & 0x0000_0011)
            | ((right << 2) & 0x0000_0004)
            | ((right >> 21) & 0x0000_0002);
        idx += 1;
        round += 1;
    }

    DesCtx { encrypt_subkeys }
}

fn do_permutation(a: &mut u32, temp: &mut u32, b: &mut u32, offset: u32, mask: u32) {
    *temp = ((*a >> offset) ^ *b) & mask;
    *b ^= *temp;
    *a ^= *temp << offset;
}

fn initial_permutation(left: &mut u32, right: &mut u32) {
    let mut temp = 0u32;
    do_permutation(left, &mut temp, right, 4, 0x0f0f_0f0f);
    do_permutation(left, &mut temp, right, 16, 0x0000_ffff);
    do_permutation(right, &mut temp, left, 2, 0x3333_3333);
    do_permutation(right, &mut temp, left, 8, 0x00ff_00ff);
    *right = right.rotate_left(1);
    temp = (*left ^ *right) & 0xaaaa_aaaa;
    *right ^= temp;
    *left ^= temp;
    *left = left.rotate_left(1);
}

fn final_permutation(left: &mut u32, right: &mut u32) {
    *left = left.rotate_right(1);
    let mut temp = (*left ^ *right) & 0xaaaa_aaaa;
    *left ^= temp;
    *right ^= temp;
    *right = right.rotate_right(1);
    do_permutation(right, &mut temp, left, 8, 0x00ff_00ff);
    do_permutation(right, &mut temp, left, 2, 0x3333_3333);
    do_permutation(left, &mut temp, right, 16, 0x0000_ffff);
    do_permutation(left, &mut temp, right, 4, 0x0f0f_0f0f);
}

const SBOX1: [u32; 64] = [
    0x01010400, 0x00000000, 0x00010000, 0x01010404, 0x01010004, 0x00010404, 0x00000004, 0x00010000,
    0x00000400, 0x01010400, 0x01010404, 0x00000400, 0x01000404, 0x01010004, 0x01000000, 0x00000004,
    0x00000404, 0x01000400, 0x01000400, 0x00010400, 0x00010400, 0x01010000, 0x01010000, 0x01000404,
    0x00010004, 0x01000004, 0x01000004, 0x00010004, 0x00000000, 0x00000404, 0x00010404, 0x01000000,
    0x00010000, 0x01010404, 0x00000004, 0x01010000, 0x01010400, 0x01000000, 0x01000000, 0x00000400,
    0x01010004, 0x00010000, 0x00010400, 0x01000004, 0x00000400, 0x00000004, 0x01000404, 0x00010404,
    0x01010404, 0x00010004, 0x01010000, 0x01000404, 0x01000004, 0x00000404, 0x00010404, 0x01010400,
    0x00000404, 0x01000400, 0x01000400, 0x00000000, 0x00010004, 0x00010400, 0x00000000, 0x01010004,
];

const SBOX2: [u32; 64] = [
    0x80108020, 0x80008000, 0x00008000, 0x00108020, 0x00100000, 0x00000020, 0x80100020, 0x80008020,
    0x80000020, 0x80108020, 0x80108000, 0x80000000, 0x80008000, 0x00100000, 0x00000020, 0x80100020,
    0x00108000, 0x00100020, 0x80008020, 0x00000000, 0x80000000, 0x00008000, 0x00108020, 0x80100000,
    0x00100020, 0x80000020, 0x00000000, 0x00108000, 0x00008020, 0x80108000, 0x80100000, 0x00008020,
    0x00000000, 0x00108020, 0x80100020, 0x00100000, 0x80008020, 0x80100000, 0x80108000, 0x00008000,
    0x80100000, 0x80008000, 0x00000020, 0x80108020, 0x00108020, 0x00000020, 0x00008000, 0x80000000,
    0x00008020, 0x80108000, 0x00100000, 0x80000020, 0x00100020, 0x80008020, 0x80000020, 0x00100020,
    0x00108000, 0x00000000, 0x80008000, 0x00008020, 0x80000000, 0x80100020, 0x80108020, 0x00108000,
];

const SBOX3: [u32; 64] = [
    0x00000208, 0x08020200, 0x00000000, 0x08020008, 0x08000200, 0x00000000, 0x00020208, 0x08000200,
    0x00020008, 0x08000008, 0x08000008, 0x00020000, 0x08020208, 0x00020008, 0x08020000, 0x00000208,
    0x08000000, 0x00000008, 0x08020200, 0x00000200, 0x00020200, 0x08020000, 0x08020008, 0x00020208,
    0x08000208, 0x00020200, 0x00020000, 0x08000208, 0x00000008, 0x08020208, 0x00000200, 0x08000000,
    0x08020200, 0x08000000, 0x00020008, 0x00000208, 0x00020000, 0x08020200, 0x08000200, 0x00000000,
    0x00000200, 0x00020008, 0x08020208, 0x08000200, 0x08000008, 0x00000200, 0x00000000, 0x08020008,
    0x08000208, 0x00020000, 0x08000000, 0x08020208, 0x00000008, 0x00020208, 0x00020200, 0x08000008,
    0x08020000, 0x08000208, 0x00000208, 0x08020000, 0x00020208, 0x00000008, 0x08020008, 0x00020200,
];

const SBOX4: [u32; 64] = [
    0x00802001, 0x00002081, 0x00002081, 0x00000080, 0x00802080, 0x00800081, 0x00800001, 0x00002001,
    0x00000000, 0x00802000, 0x00802000, 0x00802081, 0x00000081, 0x00000000, 0x00800080, 0x00800001,
    0x00000001, 0x00002000, 0x00800000, 0x00802001, 0x00000080, 0x00800000, 0x00002001, 0x00002080,
    0x00800081, 0x00000001, 0x00002080, 0x00800080, 0x00002000, 0x00802080, 0x00802081, 0x00000081,
    0x00800080, 0x00800001, 0x00802000, 0x00802081, 0x00000081, 0x00000000, 0x00000000, 0x00802000,
    0x00002080, 0x00800080, 0x00800081, 0x00000001, 0x00802001, 0x00002081, 0x00002081, 0x00000080,
    0x00802081, 0x00000081, 0x00000001, 0x00002000, 0x00800001, 0x00002001, 0x00802080, 0x00800081,
    0x00002001, 0x00002080, 0x00800000, 0x00802001, 0x00000080, 0x00800000, 0x00002000, 0x00802080,
];

const SBOX5: [u32; 64] = [
    0x00000100, 0x02080100, 0x02080000, 0x42000100, 0x00080000, 0x00000100, 0x40000000, 0x02080000,
    0x40080100, 0x00080000, 0x02000100, 0x40080100, 0x42000100, 0x42080000, 0x00080100, 0x40000000,
    0x02000000, 0x40080000, 0x40080000, 0x00000000, 0x40000100, 0x42080100, 0x42080100, 0x02000100,
    0x42080000, 0x40000100, 0x00000000, 0x42000000, 0x02080100, 0x02000000, 0x42000000, 0x00080100,
    0x00080000, 0x42000100, 0x00000100, 0x02000000, 0x40000000, 0x02080000, 0x42000100, 0x40080100,
    0x02000100, 0x40000000, 0x42080000, 0x02080100, 0x40080100, 0x00000100, 0x02000000, 0x42080000,
    0x42080100, 0x00080100, 0x42000000, 0x42080100, 0x02080000, 0x00000000, 0x40080000, 0x42000000,
    0x00080100, 0x02000100, 0x40000100, 0x00080000, 0x00000000, 0x40080000, 0x02080100, 0x40000100,
];

const SBOX6: [u32; 64] = [
    0x20000010, 0x20400000, 0x00004000, 0x20404010, 0x20400000, 0x00000010, 0x20404010, 0x00400000,
    0x20004000, 0x00404010, 0x00400000, 0x20000010, 0x00400010, 0x20004000, 0x20000000, 0x00004010,
    0x00000000, 0x00400010, 0x20004010, 0x00004000, 0x00404000, 0x20004010, 0x00000010, 0x20400010,
    0x20400010, 0x00000000, 0x00404010, 0x20404000, 0x00004010, 0x00404000, 0x20404000, 0x20000000,
    0x20004000, 0x00000010, 0x20400010, 0x00404000, 0x20404010, 0x00400000, 0x00004010, 0x20000010,
    0x00400000, 0x20004000, 0x20000000, 0x00004010, 0x20000010, 0x20404010, 0x00404000, 0x20400000,
    0x00404010, 0x20404000, 0x00000000, 0x20400010, 0x00000010, 0x00004000, 0x20400000, 0x00404010,
    0x00004000, 0x00400010, 0x20004010, 0x00000000, 0x20404000, 0x20000000, 0x00400010, 0x20004010,
];

const SBOX7: [u32; 64] = [
    0x00200000, 0x04200002, 0x04000802, 0x00000000, 0x00000800, 0x04000802, 0x00200802, 0x04200800,
    0x04200802, 0x00200000, 0x00000000, 0x04000002, 0x00000002, 0x04000000, 0x04200002, 0x00000802,
    0x04000800, 0x00200802, 0x00200002, 0x04000800, 0x04000002, 0x04200000, 0x04200800, 0x00200002,
    0x04200000, 0x00000800, 0x00000802, 0x04200802, 0x00200800, 0x00000002, 0x04000000, 0x00200800,
    0x04000000, 0x00200800, 0x00200000, 0x04000802, 0x04000802, 0x04200002, 0x04200002, 0x00000002,
    0x00200002, 0x04000000, 0x04000800, 0x00200000, 0x04200800, 0x00000802, 0x00200802, 0x04200800,
    0x00000802, 0x04000002, 0x04200802, 0x04200000, 0x00200800, 0x00000000, 0x00000002, 0x04200802,
    0x00000000, 0x00200802, 0x04200000, 0x00000800, 0x04000002, 0x04000800, 0x00000800, 0x00200002,
];

const SBOX8: [u32; 64] = [
    0x10001040, 0x00001000, 0x00040000, 0x10041040, 0x10000000, 0x10001040, 0x00000040, 0x10000000,
    0x00040040, 0x10040000, 0x10041040, 0x00041000, 0x10041000, 0x00041040, 0x00001000, 0x00000040,
    0x10040000, 0x10000040, 0x10001000, 0x00001040, 0x00041000, 0x00040040, 0x10040040, 0x10041000,
    0x00001040, 0x00000000, 0x00000000, 0x10040040, 0x10000040, 0x10001000, 0x00041040, 0x00040000,
    0x00041040, 0x00040000, 0x10041000, 0x00001000, 0x00000040, 0x10040040, 0x00001000, 0x00041040,
    0x10001000, 0x00000040, 0x10000040, 0x10040000, 0x10040040, 0x10000000, 0x00040000, 0x10001040,
    0x00000000, 0x10041040, 0x00040040, 0x10000040, 0x10040000, 0x10001000, 0x10001040, 0x00000000,
    0x10041040, 0x00041000, 0x00041000, 0x00001040, 0x00001040, 0x00040040, 0x10000000, 0x10041000,
];

fn des_round(from: u32, to: &mut u32, subkeys: &[u32; 32], idx: &mut usize) {
    let mut work = from ^ subkeys[*idx];
    *idx += 1;
    *to ^= SBOX8[(work & 0x3f) as usize];
    *to ^= SBOX6[((work >> 8) & 0x3f) as usize];
    *to ^= SBOX4[((work >> 16) & 0x3f) as usize];
    *to ^= SBOX2[((work >> 24) & 0x3f) as usize];
    work = from.rotate_left(28) ^ subkeys[*idx];
    *idx += 1;
    *to ^= SBOX7[(work & 0x3f) as usize];
    *to ^= SBOX5[((work >> 8) & 0x3f) as usize];
    *to ^= SBOX3[((work >> 16) & 0x3f) as usize];
    *to ^= SBOX1[((work >> 24) & 0x3f) as usize];
}

fn des_ecb_encrypt(ctx: &DesCtx, from: &[u8; 8]) -> [u8; 8] {
    let mut left = u32::from_be_bytes([from[0], from[1], from[2], from[3]]);
    let mut right = u32::from_be_bytes([from[4], from[5], from[6], from[7]]);
    initial_permutation(&mut left, &mut right);

    let mut idx = 0usize;
    des_round(right, &mut left, &ctx.encrypt_subkeys, &mut idx);
    des_round(left, &mut right, &ctx.encrypt_subkeys, &mut idx);
    des_round(right, &mut left, &ctx.encrypt_subkeys, &mut idx);
    des_round(left, &mut right, &ctx.encrypt_subkeys, &mut idx);
    des_round(right, &mut left, &ctx.encrypt_subkeys, &mut idx);
    des_round(left, &mut right, &ctx.encrypt_subkeys, &mut idx);
    des_round(right, &mut left, &ctx.encrypt_subkeys, &mut idx);
    des_round(left, &mut right, &ctx.encrypt_subkeys, &mut idx);
    des_round(right, &mut left, &ctx.encrypt_subkeys, &mut idx);
    des_round(left, &mut right, &ctx.encrypt_subkeys, &mut idx);
    des_round(right, &mut left, &ctx.encrypt_subkeys, &mut idx);
    des_round(left, &mut right, &ctx.encrypt_subkeys, &mut idx);
    des_round(right, &mut left, &ctx.encrypt_subkeys, &mut idx);
    des_round(left, &mut right, &ctx.encrypt_subkeys, &mut idx);
    des_round(right, &mut left, &ctx.encrypt_subkeys, &mut idx);
    des_round(left, &mut right, &ctx.encrypt_subkeys, &mut idx);

    final_permutation(&mut right, &mut left);
    let mut out = [0u8; 8];
    out[0..4].copy_from_slice(&right.to_be_bytes());
    out[4..8].copy_from_slice(&left.to_be_bytes());
    out
}

pub(crate) fn ntlm_response(password: &[u8], challenge: &[u8; 8]) -> [u8; 24] {
    let mut nt_pw = [0u8; 256];
    let len = password.len().min(128);
    let mut i = 0usize;
    while i < len {
        nt_pw[2 * i] = password[i];
        nt_pw[2 * i + 1] = 0;
        i += 1;
    }

    let hash = md4_digest(&nt_pw[..len * 2]);
    let mut nt_hash = [0u8; 21];
    nt_hash[..16].copy_from_slice(&hash);

    let mut answer = [0u8; 24];
    let mut round = 0usize;
    while round < 3 {
        let mut key_56 = [0u8; 7];
        key_56.copy_from_slice(&nt_hash[round * 7..round * 7 + 7]);
        let key = ntlm_convert_key(&key_56);
        let ctx = des_key_schedule(&key);
        let block = des_ecb_encrypt(&ctx, challenge);
        answer[round * 8..round * 8 + 8].copy_from_slice(&block);
        round += 1;
    }
    answer
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn md4_matches_known_vectors() {
        assert_eq!(
            md4_digest(b""),
            [
                0x31, 0xd6, 0xcf, 0xe0, 0xd1, 0x6a, 0xe9, 0x31, 0xb7, 0x3c, 0x59, 0xd7, 0xe0, 0xc0,
                0x89, 0xc0,
            ]
        );
        assert_eq!(
            md4_digest(b"abc"),
            [
                0xa4, 0x48, 0x01, 0x7a, 0xaf, 0x21, 0xd8, 0x52, 0x5f, 0xc1, 0x0a, 0xe8, 0x7a, 0xa6,
                0x72, 0x9d,
            ]
        );
    }

    #[test]
    fn des_matches_known_vector() {
        let key = [0x13, 0x34, 0x57, 0x79, 0x9b, 0xbc, 0xdf, 0xf1];
        let ctx = des_key_schedule(&key);
        let out = des_ecb_encrypt(&ctx, &[0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef]);
        assert_eq!(out, [0x85, 0xe8, 0x13, 0x54, 0x0f, 0x0a, 0xb4, 0x05]);
    }

    #[test]
    fn ntlm_response_matches_microsoft_ntlmv1_example() {
        // MS-NLMP section 4.2.2.2.1, NTLMv1 response example:
        // https://winprotocoldocs-bhdugrdyduf5h2e4.b02.azurefd.net/MS-NLMP/%5BMS-NLMP%5D-151016.pdf
        let response = ntlm_response(
            b"Password",
            &[0x01, 0x23, 0x45, 0x67, 0x89, 0xab, 0xcd, 0xef],
        );
        assert_eq!(
            response,
            [
                0x67, 0xc4, 0x30, 0x11, 0xf3, 0x02, 0x98, 0xa2, 0xad, 0x35, 0xec, 0xe6, 0x4f, 0x16,
                0x33, 0x1c, 0x44, 0xbd, 0xbe, 0xd9, 0x27, 0x84, 0x1f, 0x94,
            ]
        );
    }
}
