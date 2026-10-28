//! Layer III inverse MDCT and block windowing from `layer3.c`.
//!
//! The reference C build uses `FPM_DEFAULT`. Keep its operand rounding and
//! 32-bit multiply-accumulate behavior here. A wide dot product gives a more
//! accurate result, but it does not match that C build.

use crate::fixed::Fixed;

const WINDOW_LONG: [Fixed; 36] = [
    11708990, 35037858, 58100066, 80720098, 102725802, 123949700, 144230265, 163413152, 181352365,
    197911378, 212964166, 226396167, 238105157, 248002024, 256011445, 262072464, 266138953,
    268179965, 268179965, 266138953, 262072464, 256011445, 248002024, 238105157, 226396167,
    212964166, 197911378, 181352365, 163413152, 144230265, 123949700, 102725802, 80720098,
    58100066, 35037858, 11708990,
];
const WINDOW_SHORT: [Fixed; 12] = [
    35037858, 102725802, 163413152, 212964166, 248002024, 266138953, 266138953, 248002024,
    212964166, 163413152, 102725802, 35037858,
];

// C-sources/libmad-c/imdct_s.dat, in its original row order.
const IMDCT_SHORT: [[Fixed; 6]; 6] = [
    [
        0x09bd_7ca0,
        -0x0ec8_35e8,
        -0x0216_a2a2,
        0x0fdc_f549,
        -0x061f_78aa,
        -0x0cb1_9346,
    ],
    [
        -0x0cb1_9346,
        0x061f_78aa,
        0x0fdc_f549,
        0x0216_a2a2,
        -0x0ec8_35e8,
        -0x09bd_7ca0,
    ],
    [
        0x061f_78aa,
        -0x0ec8_35e8,
        0x0ec8_35e8,
        -0x061f_78aa,
        -0x061f_78aa,
        0x0ec8_35e8,
    ],
    [
        -0x0ec8_35e8,
        -0x061f_78aa,
        0x061f_78aa,
        0x0ec8_35e8,
        0x0ec8_35e8,
        0x061f_78aa,
    ],
    [
        0x0216_a2a2,
        -0x061f_78aa,
        0x09bd_7ca0,
        -0x0cb1_9346,
        0x0ec8_35e8,
        -0x0fdc_f549,
    ],
    [
        -0x0fdc_f549,
        -0x0ec8_35e8,
        -0x0cb1_9346,
        -0x09bd_7ca0,
        -0x061f_78aa,
        -0x0216_a2a2,
    ],
];

const SDCT_SCALE: [Fixed; 9] = [
    0x1fe0_d3b4,
    0x1ee8_dd47,
    0x1d00_7930,
    0x1a36_7e59,
    0x16a0_9e66,
    0x125a_bcf8,
    0x0d86_16bc,
    0x0848_3ee1,
    0x02c9_fad7,
];

const DCT_IV_SCALE: [Fixed; 18] = [
    0x1ff8_33fa,
    0x1fb9_ea93,
    0x1f3d_d120,
    0x1e84_d969,
    0x1d90_6bcf,
    0x1c62_648b,
    0x1afd_100f,
    0x1963_268b,
    0x1797_c6a4,
    0x159e_6f5b,
    0x137a_f940,
    0x1131_8ef3,
    0x0ec6_a507,
    0x0c3e_f153,
    0x099f_61c5,
    0x06ed_12c5,
    0x042d_4544,
    0x0165_547c,
];

/// Compute and window one long IMDCT block using the active fast `dctIV()` C
/// operation graph. `block_type` uses the Layer III values: 0 normal, 1
/// start, 3 stop. Other values leave the raw transform unwindowed, as C does.
pub fn imdct_long(input: &[Fixed; 18], output: &mut [Fixed; 36], block_type: u32) {
    let x = imdct36(input);
    output.copy_from_slice(&x);

    match block_type {
        0 => {
            for i in 0..36 {
                output[i] = configured_mul(output[i], WINDOW_LONG[i]);
            }
        }
        1 => {
            for i in 0..18 {
                output[i] = configured_mul(output[i], WINDOW_LONG[i]);
            }
            for i in 24..30 {
                output[i] = configured_mul(output[i], WINDOW_SHORT[i - 18]);
            }
            output[30..36].fill(0);
        }
        3 => {
            output[0..6].fill(0);
            for i in 6..12 {
                output[i] = configured_mul(output[i], WINDOW_SHORT[i - 6]);
            }
            for i in (18..36).step_by(3) {
                for j in 0..3 {
                    output[i + j] = configured_mul(output[i + j], WINDOW_LONG[i + j]);
                }
            }
        }
        _ => {}
    }
}

/// Compute and window three short IMDCT blocks with C's accumulation order.
pub fn imdct_short(input: &[Fixed; 18], output: &mut [Fixed; 36]) {
    let mut y = [0; 36];
    for window in 0..3 {
        let x = &input[window * 6..window * 6 + 6];
        let base = window * 12;
        for i in 0..3 {
            let a = configured_dot(&[
                (x[0], IMDCT_SHORT[i * 2][0]),
                (x[1], IMDCT_SHORT[i * 2][1]),
                (x[2], IMDCT_SHORT[i * 2][2]),
                (x[3], IMDCT_SHORT[i * 2][3]),
                (x[4], IMDCT_SHORT[i * 2][4]),
                (x[5], IMDCT_SHORT[i * 2][5]),
            ]);
            y[base + i] = a;
            y[base + 5 - i] = a.wrapping_neg();

            let b = configured_dot(&[
                (x[0], IMDCT_SHORT[i * 2 + 1][0]),
                (x[1], IMDCT_SHORT[i * 2 + 1][1]),
                (x[2], IMDCT_SHORT[i * 2 + 1][2]),
                (x[3], IMDCT_SHORT[i * 2 + 1][3]),
                (x[4], IMDCT_SHORT[i * 2 + 1][4]),
                (x[5], IMDCT_SHORT[i * 2 + 1][5]),
            ]);
            y[base + 6 + i] = b;
            y[base + 11 - i] = b;
        }
    }

    for i in 0..6 {
        output[i] = 0;
        output[i + 6] = configured_mul(y[i], WINDOW_SHORT[i]);
        output[i + 12] = configured_dot(&[
            (y[i + 6], WINDOW_SHORT[i + 6]),
            (y[i + 12], WINDOW_SHORT[i]),
        ]);
        output[i + 18] = configured_dot(&[
            (y[i + 18], WINDOW_SHORT[i + 6]),
            (y[i + 24], WINDOW_SHORT[i]),
        ]);
        output[i + 24] = configured_mul(y[i + 30], WINDOW_SHORT[i + 6]);
        output[i + 30] = 0;
    }
}

/// Active fast `imdct36()` in the `# if 1` branch of C `layer3.c`.
fn imdct36(input: &[Fixed; 18]) -> [Fixed; 36] {
    let mut dct = [0; 18];
    dct_iv(input, &mut dct);
    let mut output = [0; 36];
    output[..9].copy_from_slice(&dct[9..]);
    for i in 9..27 {
        output[i] = dct[26 - i].wrapping_neg();
    }
    for i in 27..36 {
        output[i] = dct[i - 27].wrapping_neg();
    }
    output
}

/// Active `dctIV()` → `sdctII()` sequence selected by `# if 1` in C.
fn dct_iv(input: &[Fixed; 18], output: &mut [Fixed; 18]) {
    let mut scaled = [0; 18];
    for i in 0..18 {
        scaled[i] = configured_mul(input[i], DCT_IV_SCALE[i]);
    }
    sdct_ii(&scaled, output);

    // C divides signed values by 2, which truncates toward zero.
    output[0] /= 2;
    for i in (1..17).step_by(4) {
        for j in 0..4 {
            let index = i + j;
            output[index] = (output[index] / 2).wrapping_sub(output[index - 1]);
        }
    }
    output[17] = (output[17] / 2).wrapping_sub(output[16]);
}

/// Active `sdctII()` with the source's even/odd butterflies and accumulation.
fn sdct_ii(input: &[Fixed; 18], output: &mut [Fixed; 18]) {
    let mut temporary = [0; 9];
    for i in 0..9 {
        temporary[i] = input[i].wrapping_add(input[17 - i]);
    }
    fast_sdct(&temporary, output, 0);

    for i in 0..9 {
        temporary[i] = configured_mul(input[i].wrapping_sub(input[17 - i]), SDCT_SCALE[i]);
    }
    fast_sdct(&temporary, output, 1);

    for i in (3..18).step_by(8) {
        for j in (0..8).step_by(2) {
            output[i + j] = output[i + j].wrapping_sub(output[i + j - 2]);
        }
    }
}

/// Active `fastsdct()` kernel. `offset` is zero for C's even output pass and
/// one for its odd output pass.
#[allow(clippy::too_many_lines)]
fn fast_sdct(x: &[Fixed; 9], y: &mut [Fixed; 18], offset: usize) {
    const C0: Fixed = 0x1f83_8b8d;
    const C1: Fixed = 0x1bb6_7ae8;
    const C2: Fixed = 0x1883_6fa3;
    const C3: Fixed = 0x1491_b752;
    const C4: Fixed = 0x0af1_d43a;
    const C5: Fixed = 0x058e_86a0;
    const C6: Fixed = -0x1e11_f642;

    let a0 = x[3].wrapping_add(x[5]);
    let a1 = x[3].wrapping_sub(x[5]);
    let a2 = x[6].wrapping_add(x[2]);
    let a3 = x[6].wrapping_sub(x[2]);
    let a4 = x[1].wrapping_add(x[7]);
    let a5 = x[1].wrapping_sub(x[7]);
    let a6 = x[8].wrapping_add(x[0]);
    let a7 = x[8].wrapping_sub(x[0]);

    let a8 = a0.wrapping_add(a2);
    let a9 = a0.wrapping_sub(a2);
    let a10 = a0.wrapping_sub(a6);
    let a11 = a2.wrapping_sub(a6);
    let a12 = a8.wrapping_add(a6);
    let a13 = a1.wrapping_sub(a3);
    let a14 = a13.wrapping_add(a7);
    let a15 = a3.wrapping_add(a7);
    let a16 = a1.wrapping_sub(a7);
    let a17 = a1.wrapping_add(a3);

    let m0 = configured_mul(a17, -C3);
    let m1 = configured_mul(a16, -C0);
    let m2 = configured_mul(a15, -C4);
    let m3 = configured_mul(a14, -C1);
    let m4 = configured_mul(a5, -C1);
    let m5 = configured_mul(a11, -C6);
    let m6 = configured_mul(a10, -C5);
    let m7 = configured_mul(a9, -C2);

    let a18 = x[4].wrapping_add(a4);
    let a19 = x[4].wrapping_mul(2).wrapping_sub(a4);
    let a20 = a19.wrapping_add(m5);
    let a21 = a19.wrapping_sub(m5);
    let a22 = a19.wrapping_add(m6);
    let a23 = m4.wrapping_add(m2);
    let a24 = m4.wrapping_sub(m2);
    let a25 = m4.wrapping_add(m1);

    let values = [
        a18.wrapping_add(a12),
        m0.wrapping_sub(a25),
        m7.wrapping_sub(a20),
        m3,
        a21.wrapping_sub(m6),
        a24.wrapping_sub(m1),
        a12.wrapping_sub(a18.wrapping_mul(2)),
        a23.wrapping_add(m0),
        a22.wrapping_add(m7),
    ];
    for (index, value) in values.into_iter().enumerate() {
        y[offset + index * 2] = value;
    }
}

/// `mad_f_mul()` for the configured libmad fixed-point profile. The result is
/// returned as a fixed value; C's MAC macros then keep its low 32 bits.
#[inline]
fn configured_mul(x: Fixed, y: Fixed) -> Fixed {
    crate::fixed::mul_configured(x, y)
}

/// Run the configured C `MAD_F_ML0`/`MAD_F_MLA`/`MAD_F_MLZ` sequence.
#[inline]
fn configured_dot<const N: usize>(terms: &[(Fixed, Fixed); N]) -> Fixed {
    let Some(&(x, y)) = terms.first() else {
        return 0;
    };
    let mut accumulator = configured_mul(x, y) as u32;
    for &(x, y) in &terms[1..] {
        accumulator = accumulator.wrapping_add(configured_mul(x, y) as u32);
    }
    accumulator as Fixed
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixed::ONE;

    #[cfg(not(target_arch = "powerpc"))]
    #[test]
    fn fpm_default_rounds_operands_before_multiplication() {
        assert_eq!(configured_mul(ONE, ONE), ONE);
        assert_eq!(configured_mul(2048, ONE), 4096);
        assert_eq!(configured_mul(-2048, ONE), 0);
        assert_eq!(configured_mul(-2049, ONE), -4096);
    }

    #[cfg(not(target_arch = "powerpc"))]
    #[test]
    fn fpm_default_mla_accumulates_low_32_bits_after_each_product() {
        assert_eq!(configured_dot(&[(i32::MAX, ONE)]), i32::MIN);
        assert_eq!(configured_dot(&[(i32::MAX, ONE), (i32::MAX, ONE)]), 0);
    }

    #[test]
    fn zero_input_produces_zero_for_each_block_type() {
        let input = [0; 18];
        for block_type in [0, 1, 3] {
            let mut output = [ONE; 36];
            imdct_long(&input, &mut output, block_type);
            assert_eq!(output, [0; 36]);
        }
        let mut output = [ONE; 36];
        imdct_short(&input, &mut output);
        assert_eq!(output, [0; 36]);
    }

    #[test]
    fn long_start_and_stop_apply_c_window_regions() {
        let mut input = [0; 18];
        input[0] = ONE;
        let mut start = [0; 36];
        imdct_long(&input, &mut start, 1);
        assert_eq!(&start[30..], &[0; 6]);

        let mut stop = [0; 36];
        imdct_long(&input, &mut stop, 3);
        assert_eq!(&stop[..6], &[0; 6]);
    }

    #[test]
    fn long_impulse_uses_c_factorized_default_arithmetic() {
        let mut input = [0; 18];
        input[0] = ONE;
        let mut dct = [0; 18];
        dct_iv(&input, &mut dct);
        let mut output = [0; 36];
        imdct_long(&input, &mut output, 2);
        assert_eq!(output[0], dct[9]);
        assert_eq!(output[9], dct[17].wrapping_neg());
        assert_eq!(output[27], dct[0].wrapping_neg());
    }

    #[test]
    fn short_impulse_has_c_guard_regions_and_signal() {
        let mut input = [0; 18];
        input[0] = ONE;
        let mut output = [0; 36];
        imdct_short(&input, &mut output);
        assert_eq!(&output[..6], &[0; 6]);
        assert_eq!(&output[30..], &[0; 6]);
        assert!(output[6..30].iter().any(|&value| value != 0));
    }
}
