//! 28-bit fractional arithmetic from `fixed.h` and `fixed.c`.

pub type Fixed = i32;
pub const FRAC_BITS: u32 = 28;
pub const ONE: Fixed = 1 << FRAC_BITS;
pub const MIN: Fixed = i32::MIN;
pub const MAX: Fixed = i32::MAX;

#[inline]
pub const fn from_int(value: i32) -> Fixed {
    value.wrapping_shl(FRAC_BITS)
}

#[inline]
pub const fn int_part(value: Fixed) -> i32 {
    value >> FRAC_BITS
}

#[inline]
pub const fn frac_part(value: Fixed) -> u32 {
    value as u32 & ((1 << FRAC_BITS) - 1)
}

#[inline]
pub fn abs(value: Fixed) -> Fixed {
    value.wrapping_abs()
}

/// Multiply using libmad's default 64-bit fixed-point scaling.
#[inline]
pub fn mul(a: Fixed, b: Fixed) -> Fixed {
    (((a as i64) * (b as i64)) >> FRAC_BITS) as i32
}

/// Multiply with libmad's default FPM path, without `OPT_SPEED`.
///
/// FPM_DEFAULT rounds each operand at different bit positions before it
/// multiplies them. This loses precision and depends on operand order, but it
/// matches the C macro used by the default build.
#[inline]
pub fn mul_fpm_default(x: Fixed, y: Fixed) -> Fixed {
    // The host C macro uses `1L`, so both rounding additions are evaluated
    // as 64-bit `long` values before the shifts.
    let x = (i64::from(x) + (1 << 11)) >> 12;
    let y = (i64::from(y) + (1 << 15)) >> 16;
    x.wrapping_mul(y) as Fixed
}

/// Multiply with the fixed-point profile used by the bundled libmad build.
///
/// Wii and GameCube use `FPM_PPC` without `OPT_ACCURACY`, which truncates a
/// signed 64-bit product back to Q28. Host differential tests keep using the
/// bundled host harness profile (`FPM_DEFAULT`) so existing fixture evidence
/// remains directly comparable.
#[inline]
pub fn mul_configured(x: Fixed, y: Fixed) -> Fixed {
    #[cfg(target_arch = "powerpc")]
    {
        mul(x, y)
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        mul_fpm_default(x, y)
    }
}

/// Divide like `mad_f_div`. A zero divisor returns zero as a defined Rust
/// result; the C implementation has undefined behavior for that input.
pub fn div(x: Fixed, y: Fixed) -> Fixed {
    if y == 0 {
        return 0;
    }

    let mut q = ((x as i64) / (y as i64)).abs();
    let mut numerator = x as i64;
    let mut denominator = y as i64;

    if numerator < 0 {
        numerator = -numerator;
        denominator = -denominator;
    }
    let mut remainder = numerator % denominator;
    if denominator < 0 {
        denominator = -denominator;
    }

    if q > int_part(MAX) as i64
        && !(q == -(int_part(MIN) as i64) && remainder == 0 && ((x < 0) != (y < 0)))
    {
        return 0;
    }

    let mut bits = FRAC_BITS;
    while bits != 0 && remainder != 0 {
        q <<= 1;
        remainder <<= 1;
        if remainder >= denominator {
            remainder -= denominator;
            q += 1;
        }
        bits -= 1;
    }

    if 2 * remainder >= denominator {
        q += 1;
    }
    if (x < 0) != (y < 0) {
        q = -q;
    }
    (q.wrapping_shl(bits)) as i32
}

#[inline]
pub fn to_f64(value: Fixed) -> f64 {
    value as f64 / ONE as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_multiply_and_divide_keep_fractional_bits() {
        assert_eq!(mul(from_int(3), from_int(2)), from_int(6));
        assert_eq!(div(from_int(1), from_int(2)), ONE / 2);
        assert_eq!(div(-ONE, from_int(2)), -(ONE / 2));
    }

    #[test]
    fn fixed_parts_match_twos_complement_format() {
        assert_eq!(int_part(from_int(-3) + (ONE / 4)), -3);
        assert_eq!(frac_part(from_int(-3) + (ONE / 4)), 1 << 26);
    }

    #[test]
    fn fpm_default_multiply_matches_rounded_operand_macro_and_order() {
        assert_eq!(mul_fpm_default(0x1234_5678, 0x0abc_def0), 204_979_185);
        assert_eq!(mul_fpm_default(0x0abc_def0, 0x1234_5678), 204_956_120);
        assert_ne!(
            mul_fpm_default(0x1234_5678, 0x0abc_def0),
            mul(0x1234_5678, 0x0abc_def0)
        );
    }

    #[test]
    fn fpm_default_rounding_additions_do_not_wrap_at_fixed_limits() {
        assert_eq!(mul_fpm_default(i32::MAX, ONE / 2), 1 << 30);
        assert_eq!(mul_fpm_default(i32::MIN, ONE / 2), -(1 << 30));
        assert_eq!(mul_fpm_default(ONE / 2, i32::MAX), 1 << 30);
        assert_eq!(mul_fpm_default(ONE / 2, i32::MIN), -(1 << 30));
    }
}
