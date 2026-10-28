//! Exact timer arithmetic based on libmad's `timer.c`.

use alloc::{
    string::{String, ToString},
    vec,
};

pub const RESOLUTION: u64 = 352_800_000;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Timer {
    pub seconds: i64,
    pub fraction: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(i32)]
pub enum Units {
    Hours = -2,
    Minutes = -1,
    Seconds = 0,
    Deciseconds = 10,
    Centiseconds = 100,
    Milliseconds = 1000,
    Hz8000 = 8000,
    Hz11025 = 11025,
    Hz12000 = 12000,
    Hz16000 = 16000,
    Hz22050 = 22050,
    Hz24000 = 24000,
    Hz32000 = 32000,
    Hz44100 = 44100,
    Hz48000 = 48000,
    Fps24 = 24,
    Fps25 = 25,
    Fps30 = 30,
    Fps48 = 48,
    Fps50 = 50,
    Fps60 = 60,
    Fps75 = 75,
    Drop23976 = -24,
    Drop24975 = -25,
    Drop2997 = -30,
    Drop47952 = -48,
    Drop4995 = -50,
    Drop5994 = -60,
}

impl Timer {
    pub const ZERO: Self = Self {
        seconds: 0,
        fraction: 0,
    };
    pub fn compare(self, other: Self) -> core::cmp::Ordering {
        (self.seconds, self.fraction).cmp(&(other.seconds, other.fraction))
    }
    pub fn negate(&mut self) {
        self.seconds = -self.seconds;
        if self.fraction != 0 {
            self.seconds -= 1;
            self.fraction = RESOLUTION - self.fraction;
        }
    }
    pub fn abs(mut self) -> Self {
        if self.seconds < 0 {
            self.negate();
        }
        self
    }
    pub fn set(&mut self, seconds: u64, numer: u64, denom: u64) {
        let extra = denom.checked_div(denom).map(|_| numer / denom).unwrap_or(0);
        let rem = denom.checked_div(denom).map(|_| numer % denom).unwrap_or(0);
        self.seconds = seconds.saturating_add(extra) as i64;
        self.fraction = if denom == 0 {
            0
        } else {
            scale(rem, denom, RESOLUTION)
        };
        self.reduce();
    }
    pub fn add(&mut self, incr: Self) {
        self.seconds += incr.seconds;
        self.fraction += incr.fraction;
        self.reduce();
    }
    pub fn multiply(&mut self, scalar: i64) {
        if scalar < 0 {
            self.negate();
        }
        let mut factor = scalar.unsigned_abs();
        let mut addend = *self;
        *self = Self::ZERO;
        while factor != 0 {
            if factor & 1 != 0 {
                self.add(addend);
            }
            addend.add(addend);
            factor >>= 1;
        }
    }
    pub fn count(self, units: Units) -> i64 {
        match units {
            Units::Hours => self.seconds / 3600,
            Units::Minutes => self.seconds / 60,
            Units::Seconds => self.seconds,
            Units::Drop23976
            | Units::Drop24975
            | Units::Drop2997
            | Units::Drop47952
            | Units::Drop4995
            | Units::Drop5994 => (self.count(drop_base(units)) + 1) * 1000 / 1001,
            _ => {
                self.seconds * units as i32 as i64
                    + scale(self.fraction, RESOLUTION, units as i32 as u64) as i64
            }
        }
    }
    pub fn fraction_in(self, denom: u64) -> u64 {
        let t = self.abs();
        if denom == 0 {
            RESOLUTION.checked_div(t.fraction).unwrap_or(RESOLUTION + 1)
        } else {
            scale(t.fraction, RESOLUTION, denom)
        }
    }

    /// Format timer fields using the numeric C `printf` conversions used by
    /// libmad's `mad_timer_string` (`%u`, `%lu`, optional width and zero pad).
    pub fn format(self, format: &str, units: Units, fracunits: Units, subparts: u64) -> String {
        let timer = self.abs();
        let mut seconds = timer.seconds.max(0) as u64;
        let (mut fraction, mut sub) = (0_u64, 0_u64);
        match fracunits {
            Units::Hours | Units::Minutes | Units::Seconds => {}
            Units::Drop23976
            | Units::Drop24975
            | Units::Drop2997
            | Units::Drop47952
            | Units::Drop4995
            | Units::Drop5994 => {
                let rate = -(fracunits as i32) as u64;
                let frame = timer.count(fracunits).max(0) as u64;
                let cycle = rate * 600 - 18;
                let d = frame / cycle;
                let m = frame % cycle;
                let frame = frame
                    + 18 * d
                    + if m > 2 {
                        2 * ((m - 2) / (cycle / 10))
                    } else {
                        0
                    };
                fraction = frame % rate;
                seconds = frame / rate;
            }
            value => {
                let scale_units = value as i32 as u64;
                let denom = RESOLUTION / scale_units;
                fraction = timer.fraction / denom;
                sub = scale(timer.fraction % denom, denom, subparts);
            }
        }
        let values = match units {
            Units::Hours => {
                let minutes = seconds / 60;
                vec![seconds / 3600, minutes % 60, seconds % 60, fraction, sub]
            }
            Units::Minutes => vec![seconds / 60, seconds % 60, fraction, sub],
            Units::Seconds => vec![seconds, fraction, sub],
            _ => {
                if (fracunits as i32) < 0 {
                    sub = 0;
                }
                vec![timer.count(units).max(0) as u64, sub]
            }
        };
        format_values(format, &values)
    }
    fn reduce(&mut self) {
        self.seconds += (self.fraction / RESOLUTION) as i64;
        self.fraction %= RESOLUTION;
    }
}

fn format_values(template: &str, values: &[u64]) -> String {
    let bytes = template.as_bytes();
    let mut out = String::with_capacity(template.len() + 24);
    let mut i = 0;
    let mut arg = 0;
    while i < bytes.len() {
        if bytes[i] != b'%' {
            let next = template[i..].chars().next().expect("valid UTF-8 boundary");
            out.push(next);
            i += next.len_utf8();
            continue;
        }
        if bytes.get(i + 1) == Some(&b'%') {
            out.push('%');
            i += 2;
            continue;
        }
        let start = i;
        i += 1;
        let mut zero = false;
        let mut left = false;
        while let Some(&flag) = bytes.get(i) {
            match flag {
                b'0' => zero = true,
                b'-' => left = true,
                b'+' | b' ' => {}
                _ => break,
            }
            i += 1;
        }
        let width_start = i;
        while bytes.get(i).is_some_and(u8::is_ascii_digit) {
            i += 1;
        }
        let width = core::str::from_utf8(&bytes[width_start..i])
            .ok()
            .and_then(|s| s.parse::<usize>().ok())
            .unwrap_or(0);
        if bytes.get(i) == Some(&b'l') {
            i += 1;
        }
        let Some(&spec) = bytes.get(i) else {
            out.push('%');
            break;
        };
        i += 1;
        if !matches!(spec, b'u' | b'd' | b'i') || arg >= values.len() {
            out.push_str(&template[start..i]);
            continue;
        }
        let digits = values[arg].to_string();
        arg += 1;
        let padding = width.saturating_sub(digits.len());
        if left {
            out.push_str(&digits);
            for _ in 0..padding {
                out.push(' ');
            }
        } else {
            for _ in 0..padding {
                out.push(if zero { '0' } else { ' ' });
            }
            out.push_str(&digits);
        }
    }
    out
}

fn drop_base(units: Units) -> Units {
    match units {
        Units::Drop23976 => Units::Fps24,
        Units::Drop24975 => Units::Fps25,
        Units::Drop2997 => Units::Fps30,
        Units::Drop47952 => Units::Fps48,
        Units::Drop4995 => Units::Fps50,
        _ => Units::Fps60,
    }
}

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let r = a % b;
        a = b;
        b = r;
    }
    a
}
fn scale(mut numer: u64, mut denom: u64, mut target: u64) -> u64 {
    if numer == 0 || target == 0 {
        return 0;
    }
    let g = gcd(numer, denom);
    numer /= g;
    denom /= g;
    let g = gcd(target, denom);
    target /= g;
    denom /= g;
    if denom == 0 {
        return 0;
    }
    if denom < target {
        numer
            .saturating_mul(target / denom)
            .saturating_add(numer.saturating_mul(target % denom) / denom)
    } else if denom < numer {
        target
            .saturating_mul(numer / denom)
            .saturating_add(target.saturating_mul(numer % denom) / denom)
    } else {
        numer.saturating_mul(target) / denom
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn set_add_and_count_preserve_fractional_time() {
        let mut t = Timer::ZERO;
        t.set(0, 1, 44100);
        assert_eq!(t.fraction, RESOLUTION / 44100);
        assert_eq!(t.count(Units::Hz44100), 1);
        t.add(t);
        assert_eq!(t.count(Units::Milliseconds), 0);
        assert_eq!(t.fraction_in(0), RESOLUTION / t.fraction);
    }
    #[test]
    fn negate_uses_borrowed_fraction_representation() {
        let mut t = Timer {
            seconds: 1,
            fraction: RESOLUTION / 4,
        };
        t.negate();
        assert_eq!(
            t,
            Timer {
                seconds: -2,
                fraction: RESOLUTION * 3 / 4
            }
        );
        assert_eq!(
            t.abs(),
            Timer {
                seconds: 1,
                fraction: RESOLUTION / 4
            }
        );
    }

    #[test]
    fn formats_seconds_and_fraction_fields() {
        let t = Timer {
            seconds: 65,
            fraction: RESOLUTION / 2,
        };
        assert_eq!(
            t.format("%lu:%02u.%03u", Units::Minutes, Units::Milliseconds, 0),
            "1:05.500"
        );
        assert_eq!(
            t.format("%02lu:%02u:%02u", Units::Hours, Units::Seconds, 0),
            "00:01:05"
        );
    }
}
