//! Wii and GameCube timer conversions used by video and probe diagnostics.

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum PlatformClock {
    Wii,
    GameCube,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum VideoStandard {
    Ntsc,
    Pal,
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ViTiming {
    pub equ: u16,
    pub acv: u16,
    pub prb_odd: u16,
    pub prb_even: u16,
    pub psb_odd: u16,
    pub psb_even: u16,
    pub bs1: u16,
    pub bs2: u16,
    pub bs3: u16,
    pub bs4: u16,
    pub be1: u16,
    pub be2: u16,
    pub be3: u16,
    pub be4: u16,
    pub nhlines: u16,
    pub hlw: u16,
    pub hsy: u16,
    pub hcs: u16,
    pub hce: u16,
    pub hbe640: u16,
    pub hbs640: u16,
}

const VI_TIMINGS: [ViTiming; 8] = [
    ViTiming {
        equ: 0x06,
        acv: 0x00f0,
        prb_odd: 0x0018,
        prb_even: 0x0019,
        psb_odd: 0x0003,
        psb_even: 0x0002,
        bs1: 0x0c,
        bs2: 0x0d,
        bs3: 0x0c,
        bs4: 0x0d,
        be1: 0x0208,
        be2: 0x0207,
        be3: 0x0208,
        be4: 0x0207,
        nhlines: 0x020d,
        hlw: 0x01ad,
        hsy: 0x40,
        hcs: 0x47,
        hce: 0x69,
        hbe640: 0x00a2,
        hbs640: 0x0175,
    },
    ViTiming {
        equ: 0x06,
        acv: 0x00f0,
        prb_odd: 0x0018,
        prb_even: 0x0018,
        psb_odd: 0x0004,
        psb_even: 0x0004,
        bs1: 0x0c,
        bs2: 0x0c,
        bs3: 0x0c,
        bs4: 0x0c,
        be1: 0x0208,
        be2: 0x0208,
        be3: 0x0208,
        be4: 0x0208,
        nhlines: 0x020e,
        hlw: 0x01ad,
        hsy: 0x40,
        hcs: 0x47,
        hce: 0x69,
        hbe640: 0x00a2,
        hbs640: 0x0175,
    },
    ViTiming {
        equ: 0x05,
        acv: 0x0120,
        prb_odd: 0x0021,
        prb_even: 0x0022,
        psb_odd: 0x0001,
        psb_even: 0x0000,
        bs1: 0x0d,
        bs2: 0x0c,
        bs3: 0x0b,
        bs4: 0x0a,
        be1: 0x026b,
        be2: 0x026a,
        be3: 0x0269,
        be4: 0x026c,
        nhlines: 0x0271,
        hlw: 0x01b0,
        hsy: 0x40,
        hcs: 0x4b,
        hce: 0x6a,
        hbe640: 0x00ac,
        hbs640: 0x017c,
    },
    ViTiming {
        equ: 0x05,
        acv: 0x0120,
        prb_odd: 0x0021,
        prb_even: 0x0021,
        psb_odd: 0x0000,
        psb_even: 0x0000,
        bs1: 0x0d,
        bs2: 0x0b,
        bs3: 0x0d,
        bs4: 0x0b,
        be1: 0x026b,
        be2: 0x026d,
        be3: 0x026b,
        be4: 0x026d,
        nhlines: 0x0270,
        hlw: 0x01b0,
        hsy: 0x40,
        hcs: 0x4b,
        hce: 0x6a,
        hbe640: 0x00ac,
        hbs640: 0x017c,
    },
    ViTiming {
        equ: 0x06,
        acv: 0x00f0,
        prb_odd: 0x0018,
        prb_even: 0x0019,
        psb_odd: 0x0003,
        psb_even: 0x0002,
        bs1: 0x10,
        bs2: 0x0f,
        bs3: 0x0e,
        bs4: 0x0d,
        be1: 0x0206,
        be2: 0x0205,
        be3: 0x0204,
        be4: 0x0207,
        nhlines: 0x020d,
        hlw: 0x01ad,
        hsy: 0x40,
        hcs: 0x4e,
        hce: 0x70,
        hbe640: 0x00a2,
        hbs640: 0x0175,
    },
    ViTiming {
        equ: 0x06,
        acv: 0x00f0,
        prb_odd: 0x0018,
        prb_even: 0x0018,
        psb_odd: 0x0004,
        psb_even: 0x0004,
        bs1: 0x10,
        bs2: 0x0e,
        bs3: 0x10,
        bs4: 0x0e,
        be1: 0x0206,
        be2: 0x0208,
        be3: 0x0206,
        be4: 0x0208,
        nhlines: 0x020e,
        hlw: 0x01ad,
        hsy: 0x40,
        hcs: 0x4e,
        hce: 0x70,
        hbe640: 0x00a2,
        hbs640: 0x0175,
    },
    ViTiming {
        equ: 0x0c,
        acv: 0x01e0,
        prb_odd: 0x0030,
        prb_even: 0x0030,
        psb_odd: 0x0006,
        psb_even: 0x0006,
        bs1: 0x18,
        bs2: 0x18,
        bs3: 0x18,
        bs4: 0x18,
        be1: 0x040e,
        be2: 0x040e,
        be3: 0x040e,
        be4: 0x040e,
        nhlines: 0x041a,
        hlw: 0x01ad,
        hsy: 0x40,
        hcs: 0x47,
        hce: 0x69,
        hbe640: 0x00a2,
        hbs640: 0x0175,
    },
    ViTiming {
        equ: 0x0a,
        acv: 0x0240,
        prb_odd: 0x0044,
        prb_even: 0x0044,
        psb_odd: 0x0000,
        psb_even: 0x0000,
        bs1: 0x14,
        bs2: 0x14,
        bs3: 0x14,
        bs4: 0x14,
        be1: 0x04d8,
        be2: 0x04d8,
        be3: 0x04d8,
        be4: 0x04d8,
        nhlines: 0x04e2,
        hlw: 0x01b0,
        hsy: 0x40,
        hcs: 0x4b,
        hce: 0x6a,
        hbe640: 0x00ac,
        hbs640: 0x017c,
    },
];

pub const fn vi_timing_for_mode(mode: u32) -> Option<ViTiming> {
    let index = match mode {
        0 | 20 => 0,
        1 | 21 => 1,
        4 => 2,
        5 => 3,
        8 => 4,
        9 => 5,
        2 | 10 | 14 | 22 => 6,
        6 => 7,
        _ => return None,
    };
    Some(VI_TIMINGS[index])
}

/// Return the field-rate family selected by a libogc VI mode ID.
pub const fn video_standard_for_mode(mode: u32) -> Option<VideoStandard> {
    match mode {
        0..=2 | 8..=10 | 14 | 20..=22 => Some(VideoStandard::Ntsc),
        4..=6 => Some(VideoStandard::Pal),
        _ => None,
    }
}

pub const fn vi_control_register_for_mode(mode: u32) -> Option<u16> {
    if vi_timing_for_mode(mode).is_none() {
        return None;
    }
    let format = ((mode >> 2) & 0x7) as u16;
    if vi_mode_is_progressive(mode) {
        Some((format << 8) | 0x0005)
    } else {
        let interlace = (mode & 0x1) as u16;
        Some((format << 8) | (interlace << 2) | 0x0001)
    }
}

pub const fn vi_mode_is_progressive(mode: u32) -> bool {
    mode & 0x2 != 0
}

pub const fn vi_scan_mode_for_mode(mode: u32) -> u32 {
    if mode & 0x3 != 0 { 1 } else { 0 }
}

pub const fn timer_clock_hz(platform: PlatformClock) -> u64 {
    match platform {
        PlatformClock::Wii => 60_750_000,
        PlatformClock::GameCube => 40_500_000,
    }
}

pub const fn retrace_period_ticks(platform: PlatformClock, standard: VideoStandard) -> u64 {
    let clock_hz = timer_clock_hz(platform);
    match standard {
        VideoStandard::Ntsc => (clock_hz * 1001 + 30_000) / 60_000,
        VideoStandard::Pal => (clock_hz + 25) / 50,
    }
}

pub const fn ticks_to_microseconds(ticks: u64, clock_hz: u64) -> u64 {
    if clock_hz == 0 {
        return 0;
    }
    ticks.saturating_mul(1_000_000) / clock_hz
}

#[cfg(test)]
mod tests {
    use super::{
        PlatformClock, VideoStandard, retrace_period_ticks, ticks_to_microseconds,
        vi_control_register_for_mode, vi_mode_is_progressive, vi_scan_mode_for_mode,
        vi_timing_for_mode, video_standard_for_mode,
    };

    #[test]
    fn uses_the_reference_timer_clocks() {
        assert_eq!(super::timer_clock_hz(PlatformClock::Wii), 60_750_000);
        assert_eq!(super::timer_clock_hz(PlatformClock::GameCube), 40_500_000);
    }

    #[test]
    fn calculates_ntsc_and_pal_field_periods_per_platform() {
        assert_eq!(
            retrace_period_ticks(PlatformClock::Wii, VideoStandard::Ntsc),
            1_013_513
        );
        assert_eq!(
            retrace_period_ticks(PlatformClock::Wii, VideoStandard::Pal),
            1_215_000
        );
        assert_eq!(
            retrace_period_ticks(PlatformClock::GameCube, VideoStandard::Ntsc),
            675_675
        );
        assert_eq!(
            retrace_period_ticks(PlatformClock::GameCube, VideoStandard::Pal),
            810_000
        );
    }

    #[test]
    fn converts_ticks_with_the_selected_platform_clock() {
        assert_eq!(ticks_to_microseconds(1_013_505, 60_750_000), 16_683);
        assert_eq!(ticks_to_microseconds(675_675, 40_500_000), 16_683);
        assert_eq!(ticks_to_microseconds(5, 0), 0);
    }

    #[test]
    fn selects_reference_vi_register_timings_by_mode() {
        let ntsc = vi_timing_for_mode(0).unwrap();
        let ntsc_double_strike = vi_timing_for_mode(1).unwrap();
        let pal = vi_timing_for_mode(4).unwrap();
        let pal_double_strike = vi_timing_for_mode(5).unwrap();
        let pal_progressive = vi_timing_for_mode(6).unwrap();
        let mpal = vi_timing_for_mode(8).unwrap();
        let mpal_double_strike = vi_timing_for_mode(9).unwrap();
        let eurgb60 = vi_timing_for_mode(20).unwrap();
        let eurgb60_double_strike = vi_timing_for_mode(21).unwrap();
        let ntsc_progressive = vi_timing_for_mode(2).unwrap();
        let eurgb60_progressive = vi_timing_for_mode(22).unwrap();

        assert_eq!(ntsc.nhlines, 0x020d);
        assert_eq!(ntsc_double_strike.nhlines, 0x020e);
        assert_eq!(pal.nhlines, 0x0271);
        assert_eq!(pal_double_strike.nhlines, 0x0270);
        assert_eq!(pal_progressive.nhlines, 0x04e2);
        assert_eq!(mpal.nhlines, 0x020d);
        assert_eq!(mpal_double_strike.nhlines, 0x020e);
        assert_eq!(eurgb60.nhlines, 0x020d);
        assert_eq!(eurgb60_double_strike.nhlines, 0x020e);
        assert_eq!(ntsc_progressive.nhlines, 0x041a);
        assert_eq!(eurgb60_progressive.nhlines, 0x041a);

        assert_eq!(vi_timing_for_mode(10), Some(ntsc_progressive));
        assert_eq!(vi_timing_for_mode(14), Some(ntsc_progressive));
        assert!(vi_timing_for_mode(12).is_none());
        assert!(vi_timing_for_mode(13).is_none());
        assert!(vi_timing_for_mode(3).is_none());
    }

    #[test]
    fn control_register_matches_the_selected_vi_format_and_scan_mode() {
        assert_eq!(vi_control_register_for_mode(0), Some(0x0001));
        assert_eq!(vi_control_register_for_mode(1), Some(0x0005));
        assert_eq!(vi_control_register_for_mode(4), Some(0x0101));
        assert_eq!(vi_control_register_for_mode(5), Some(0x0105));
        assert_eq!(vi_control_register_for_mode(6), Some(0x0105));
        assert_eq!(vi_control_register_for_mode(8), Some(0x0201));
        assert_eq!(vi_control_register_for_mode(9), Some(0x0205));
        assert_eq!(vi_control_register_for_mode(2), Some(0x0005));
        assert_eq!(vi_control_register_for_mode(10), Some(0x0205));
        assert_eq!(vi_control_register_for_mode(14), Some(0x0305));
        assert_eq!(vi_control_register_for_mode(20), Some(0x0501));
        assert_eq!(vi_control_register_for_mode(21), Some(0x0505));
        assert_eq!(vi_control_register_for_mode(22), Some(0x0505));
        assert_eq!(vi_control_register_for_mode(3), None);
        assert!(vi_mode_is_progressive(2));
        assert!(vi_mode_is_progressive(6));
        assert!(!vi_mode_is_progressive(4));
        assert_eq!(vi_scan_mode_for_mode(0), 0);
        assert_eq!(vi_scan_mode_for_mode(1), 1);
        assert_eq!(vi_scan_mode_for_mode(4), 0);
        assert_eq!(vi_scan_mode_for_mode(5), 1);
        assert_eq!(vi_scan_mode_for_mode(2), 1);
        assert_eq!(vi_scan_mode_for_mode(6), 1);
    }

    #[test]
    fn maps_c_reference_mode_ids_to_ntsc_mpal_pal_and_eurgb60_rates() {
        for mode in [0, 1, 2, 8, 9, 10, 14, 20, 21, 22] {
            assert_eq!(video_standard_for_mode(mode), Some(VideoStandard::Ntsc));
        }
        for mode in [4, 5, 6] {
            assert_eq!(video_standard_for_mode(mode), Some(VideoStandard::Pal));
        }
        for mode in [3, 7, 11, 12, 13, 15, 16, 17, 18, 19] {
            assert_eq!(video_standard_for_mode(mode), None);
        }
    }
}
