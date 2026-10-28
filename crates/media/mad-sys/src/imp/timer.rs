//! `mad_timer_*` ABI helpers from `timer.c`.

use libmad_rs::timer::{Timer, Units};

use super::abi::*;

pub const MAD_TIMER_ZERO: MadTimerT = MadTimerT {
    seconds: 0,
    fraction: 0,
};

#[inline]
fn to_timer(value: MadTimerT) -> Timer {
    Timer {
        seconds: i64::from(value.seconds),
        fraction: u64::from(value.fraction),
    }
}

#[inline]
fn from_timer(value: Timer) -> MadTimerT {
    MadTimerT {
        seconds: value.seconds as i32,
        fraction: value.fraction as u32,
    }
}

#[inline]
fn units(value: i32) -> Option<Units> {
    Some(match value {
        MAD_UNITS_HOURS => Units::Hours,
        MAD_UNITS_MINUTES => Units::Minutes,
        MAD_UNITS_SECONDS => Units::Seconds,
        MAD_UNITS_DECISECONDS => Units::Deciseconds,
        MAD_UNITS_CENTISECONDS => Units::Centiseconds,
        MAD_UNITS_MILLISECONDS => Units::Milliseconds,
        MAD_UNITS_8000_HZ => Units::Hz8000,
        MAD_UNITS_11025_HZ => Units::Hz11025,
        MAD_UNITS_12000_HZ => Units::Hz12000,
        MAD_UNITS_16000_HZ => Units::Hz16000,
        MAD_UNITS_22050_HZ => Units::Hz22050,
        MAD_UNITS_24000_HZ => Units::Hz24000,
        MAD_UNITS_32000_HZ => Units::Hz32000,
        MAD_UNITS_44100_HZ => Units::Hz44100,
        MAD_UNITS_48000_HZ => Units::Hz48000,
        MAD_UNITS_24_FPS => Units::Fps24,
        MAD_UNITS_25_FPS => Units::Fps25,
        MAD_UNITS_30_FPS => Units::Fps30,
        MAD_UNITS_48_FPS => Units::Fps48,
        MAD_UNITS_50_FPS => Units::Fps50,
        MAD_UNITS_60_FPS => Units::Fps60,
        MAD_UNITS_75_FPS => Units::Fps75,
        MAD_UNITS_23_976_FPS => Units::Drop23976,
        MAD_UNITS_24_975_FPS => Units::Drop24975,
        MAD_UNITS_29_97_FPS => Units::Drop2997,
        MAD_UNITS_47_952_FPS => Units::Drop47952,
        MAD_UNITS_49_95_FPS => Units::Drop4995,
        MAD_UNITS_59_94_FPS => Units::Drop5994,
        _ => return None,
    })
}

pub fn timer_compare(lhs: MadTimerT, rhs: MadTimerT) -> i32 {
    // C subtracts signed 32-bit seconds first, then unsigned fractions through
    // an s32 temporary. Ordering gives the same defined result for normalized
    // timers without depending on debug-overflow behavior.
    match (lhs.seconds, lhs.fraction).cmp(&(rhs.seconds, rhs.fraction)) {
        core::cmp::Ordering::Less => -1,
        core::cmp::Ordering::Equal => 0,
        core::cmp::Ordering::Greater => 1,
    }
}

/// # Safety
/// `timer` must be writable when non-null.
pub unsafe fn timer_negate(timer: *mut MadTimerT) {
    if timer.is_null() {
        return;
    }
    let mut value = to_timer(unsafe { *timer });
    value.negate();
    unsafe { *timer = from_timer(value) };
}

pub fn timer_abs(timer: MadTimerT) -> MadTimerT {
    from_timer(to_timer(timer).abs())
}

/// # Safety
/// `timer` must be writable when non-null.
pub unsafe fn timer_set(timer: *mut MadTimerT, seconds: u32, numer: u32, denom: u32) {
    if timer.is_null() {
        return;
    }
    let mut value = Timer::ZERO;
    value.set(u64::from(seconds), u64::from(numer), u64::from(denom));
    unsafe { *timer = from_timer(value) };
}

/// # Safety
/// `timer` must be writable when non-null.
pub unsafe fn timer_add(timer: *mut MadTimerT, incr: MadTimerT) {
    if timer.is_null() {
        return;
    }
    let mut value = to_timer(unsafe { *timer });
    value.add(to_timer(incr));
    unsafe { *timer = from_timer(value) };
}

/// # Safety
/// `timer` must be writable when non-null.
pub unsafe fn timer_multiply(timer: *mut MadTimerT, scalar: i32) {
    if timer.is_null() {
        return;
    }
    let mut value = to_timer(unsafe { *timer });
    value.multiply(i64::from(scalar));
    unsafe { *timer = from_timer(value) };
}

pub fn timer_count(timer: MadTimerT, unit: i32) -> i32 {
    units(unit)
        .map(|unit| to_timer(timer).count(unit) as i32)
        .unwrap_or(0)
}

pub fn timer_fraction(timer: MadTimerT, denom: u32) -> u32 {
    to_timer(timer).fraction_in(u64::from(denom)) as u32
}

/// Format into caller-provided C storage using the numeric conversion subset
/// used by libmad's `mad_timer_string`.
///
/// # Safety
/// `dest` must have room for the complete string plus a trailing NUL. `format`
/// must be a readable NUL-terminated string.
pub unsafe fn timer_string(
    timer: MadTimerT,
    dest: *mut i8,
    format: *const i8,
    unit: i32,
    fracunit: i32,
    subparts: u32,
) {
    if dest.is_null() || format.is_null() {
        return;
    }
    let (Some(unit), Some(fracunit)) = (units(unit), units(fracunit)) else {
        unsafe { *dest = 0 };
        return;
    };
    let mut len = 0usize;
    while unsafe { *format.add(len) } != 0 {
        len += 1;
    }
    let bytes = unsafe { core::slice::from_raw_parts(format.cast::<u8>(), len) };
    let Ok(template) = core::str::from_utf8(bytes) else {
        unsafe { *dest = 0 };
        return;
    };
    let output = to_timer(timer).format(template, unit, fracunit, u64::from(subparts));
    unsafe {
        core::ptr::copy_nonoverlapping(output.as_ptr(), dest.cast::<u8>(), output.len());
        *dest.add(output.len()) = 0;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timer_operations_match_common_c_cases() {
        let mut timer = MAD_TIMER_ZERO;
        unsafe { timer_set(&mut timer, 1, 1, 2) };
        assert_eq!(timer.seconds, 1);
        assert_eq!(timer.fraction, (MAD_TIMER_RESOLUTION / 2));
        assert_eq!(timer_count(timer, MAD_UNITS_MILLISECONDS), 1500);
        assert_eq!(timer_fraction(timer, 1000), 500);
        let neg = timer_abs(MadTimerT {
            seconds: -2,
            fraction: (MAD_TIMER_RESOLUTION / 2),
        });
        assert_eq!(neg.seconds, 1);
        assert_eq!(neg.fraction, (MAD_TIMER_RESOLUTION / 2));
    }
}
