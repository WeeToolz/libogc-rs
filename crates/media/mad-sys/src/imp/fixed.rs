//! Safe fixed-point helpers corresponding to `fixed.h`.

use super::abi::{MAD_F_FRACBITS, MAD_F_ONE, MadFixedT};

/// Convert an integer to libmad's signed 28-bit-fraction representation.
#[inline]
pub const fn mad_f_fromint(value: i32) -> MadFixedT {
    value.wrapping_shl(MAD_F_FRACBITS as u32)
}

/// Return the arithmetic integer portion of a fixed-point value.
#[inline]
pub const fn mad_f_intpart(value: MadFixedT) -> i32 {
    value >> MAD_F_FRACBITS
}

/// Return the low 28 fractional bits. The C macro expects a non-negative value.
#[inline]
pub const fn mad_f_fracpart(value: MadFixedT) -> u32 {
    (value as u32) & ((1u32 << MAD_F_FRACBITS) - 1)
}

#[inline]
pub const fn mad_f_add(lhs: MadFixedT, rhs: MadFixedT) -> MadFixedT {
    lhs.wrapping_add(rhs)
}

#[inline]
pub const fn mad_f_sub(lhs: MadFixedT, rhs: MadFixedT) -> MadFixedT {
    lhs.wrapping_sub(rhs)
}

/// Multiply two fixed-point values and truncate the low fractional bits.
#[inline]
pub const fn mad_f_mul(lhs: MadFixedT, rhs: MadFixedT) -> MadFixedT {
    (((lhs as i64) * (rhs as i64)) >> MAD_F_FRACBITS) as i32
}

/// Return the absolute fixed-point value.
#[inline]
pub fn mad_f_abs(value: MadFixedT) -> MadFixedT {
    libmad_rs::fixed::abs(value)
}

/// Divide two fixed-point values with libmad's Q28 quotient algorithm.
#[inline]
pub fn mad_f_div(lhs: MadFixedT, rhs: MadFixedT) -> MadFixedT {
    libmad_rs::fixed::div(lhs, rhs)
}

/// Convert a fixed-point value to `f64` without changing its bit value.
#[inline]
pub const fn mad_f_todouble(value: MadFixedT) -> f64 {
    value as f64 / MAD_F_ONE as f64
}

/// Convert `f64` to fixed-point with the C macro's add-half-then-truncate rule.
///
/// C's cast for values outside the signed integer range is implementation
/// defined. This safe helper clamps those inputs to the representable range.
#[inline]
pub fn mad_f_tofixed(value: f64) -> MadFixedT {
    let scaled = value * MAD_F_ONE as f64 + 0.5;
    if scaled.is_nan() {
        0
    } else if scaled >= i32::MAX as f64 {
        i32::MAX
    } else if scaled <= i32::MIN as f64 {
        i32::MIN
    } else {
        scaled as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_helpers_match_28_bit_format() {
        assert_eq!(mad_f_fromint(2), 0x2000_0000);
        assert_eq!(mad_f_intpart(-MAD_F_ONE - 1), -2);
        assert_eq!(mad_f_fracpart(MAD_F_ONE + 3), 3);
        assert_eq!(mad_f_add(i32::MAX, 1), i32::MIN);
        assert_eq!(mad_f_sub(i32::MIN, 1), i32::MAX);
        assert_eq!(mad_f_sub(0x1800_0000, 0x0800_0000), 0x1000_0000);
        assert_eq!(mad_f_mul(MAD_F_ONE / 2, MAD_F_ONE), MAD_F_ONE / 2);
        assert_eq!(mad_f_abs(-MAD_F_ONE / 2), MAD_F_ONE / 2);
        assert_eq!(mad_f_div(MAD_F_ONE, MAD_F_ONE / 2), MAD_F_ONE * 2);
        assert_eq!(mad_f_todouble(-MAD_F_ONE / 2), -0.5);
        assert_eq!(mad_f_tofixed(1.0), MAD_F_ONE);
    }
}
