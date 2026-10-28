//! FAT calendar fields use UTC on Wii. No timezone is stored on a FAT volume.
const fn leap(year: u32) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}
fn month_days(year: u32, month: u32) -> Option<u32> {
    Some(match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap(year) {
                29
            } else {
                28
            }
        }
        _ => return None,
    })
}
pub fn decode(date: u16, time: u16) -> Option<u64> {
    let year = 1980 + u32::from(date >> 9);
    let month = u32::from((date >> 5) & 15);
    let day = u32::from(date & 31);
    let hour = u32::from(time >> 11);
    let minute = u32::from((time >> 5) & 63);
    let second = u32::from(time & 31) * 2;
    if day == 0 || day > month_days(year, month)? || hour > 23 || minute > 59 || second > 59 {
        return None;
    }
    let mut days = 0u64;
    for year in 1970..year {
        days += if leap(year) { 366 } else { 365 };
    }
    for month in 1..month {
        days += u64::from(month_days(year, month)?);
    }
    days += u64::from(day - 1);
    Some(days * 86400 + u64::from(hour * 3600 + minute * 60 + second))
}
pub fn encode(seconds: u64) -> Option<(u16, u16)> {
    let mut days = seconds / 86400;
    let mut year = 1970;
    loop {
        let count = if leap(year) { 366 } else { 365 };
        if days < count {
            break;
        }
        days -= count;
        year += 1;
        if year > 2107 {
            return None;
        }
    }
    if year < 1980 {
        return None;
    }
    let mut month = 1;
    while days >= u64::from(month_days(year, month)?) {
        days -= u64::from(month_days(year, month)?);
        month += 1;
    }
    let date = (((year - 1980) << 9) | (month << 5) | (days as u32 + 1)) as u16;
    let within = seconds % 86400;
    let time = (((within / 3600) << 11) | ((within / 60 % 60) << 5) | (within % 60 / 2)) as u16;
    Some((date, time))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn calendar_matches_independent_epoch_and_leap_day_fields() {
        assert_eq!(encode(315532800), Some((0x0021, 0)));
        assert_eq!(decode(0x0021, 0), Some(315532800));
        assert_eq!(encode(946684800), Some((0x2821, 0)));
        assert_eq!(decode(0x585d, 0), Some(1709164800)); // 2024-02-29
        assert_eq!(encode(1709164800), Some((0x585d, 0)));
        assert_eq!(encode(946730097), Some((0x2821, 0x645c))); // 12:34:57 floors to :56
        assert_eq!(decode(0x2821, 0x645c), Some(946730096));
    }
    #[test]
    fn calendar_rejects_bad_fields_and_values_outside_fat_range() {
        for (date, time) in [
            (0, 0),
            (0x025d, 0),
            (0x2821, 0xc000),
            (0x2821, 0x07e0),
            (0x2821, 31),
        ] {
            assert_eq!(decode(date, time), None);
        }
        assert_eq!(encode(315532799), None);
        assert_eq!(encode(4354819200), None); // 2108-01-01
        assert_eq!(encode(u64::MAX), None);
    }
}
