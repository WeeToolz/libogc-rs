#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

pub const CONF_EBADFILE: i32 = -0x6001;
pub const CONF_ENOENT: i32 = -0x6002;
pub const CONF_ETOOBIG: i32 = -0x6003;
pub const CONF_ENOTINIT: i32 = -0x6004;
pub const CONF_ENOTIMPL: i32 = -0x6005;
pub const CONF_EBADVALUE: i32 = -0x6006;
pub const CONF_ENOMEM: i32 = -0x6007;
pub const CONF_ERR_OK: i32 = 0;

pub const CONF_BIGARRAY: u8 = 1;
pub const CONF_SMALLARRAY: u8 = 2;
pub const CONF_BYTE: u8 = 3;
pub const CONF_SHORT: u8 = 4;
pub const CONF_LONG: u8 = 5;
pub const CONF_BOOL: u8 = 7;

pub const CONF_VIDEO_NTSC: u32 = 0;
pub const CONF_VIDEO_PAL: u32 = 1;
pub const CONF_VIDEO_MPAL: u32 = 2;

pub const CONF_REGION_JP: u32 = 0;
pub const CONF_REGION_US: u32 = 1;
pub const CONF_REGION_EU: u32 = 2;
pub const CONF_REGION_KR: u32 = 4;
pub const CONF_REGION_CN: u32 = 5;

pub const CONF_AREA_JPN: u32 = 0;
pub const CONF_AREA_USA: u32 = 1;
pub const CONF_AREA_EUR: u32 = 2;
pub const CONF_AREA_AUS: u32 = 3;
pub const CONF_AREA_BRA: u32 = 4;
pub const CONF_AREA_TWN: u32 = 5;
pub const CONF_AREA_ROC: u32 = 6;
pub const CONF_AREA_KOR: u32 = 7;
pub const CONF_AREA_HKG: u32 = 8;
pub const CONF_AREA_ASI: u32 = 9;
pub const CONF_AREA_LTN: u32 = 10;
pub const CONF_AREA_SAF: u32 = 11;
pub const CONF_AREA_CHN: u32 = 12;

pub const CONF_SHUTDOWN_STANDBY: u32 = 0;
pub const CONF_SHUTDOWN_IDLE: u32 = 1;

pub const CONF_LED_OFF: u32 = 0;
pub const CONF_LED_DIM: u32 = 1;
pub const CONF_LED_BRIGHT: u32 = 2;

pub const CONF_SOUND_MONO: u32 = 0;
pub const CONF_SOUND_STEREO: u32 = 1;
pub const CONF_SOUND_SURROUND: u32 = 2;

pub const CONF_LANG_JAPANESE: u32 = 0;
pub const CONF_LANG_ENGLISH: u32 = 1;
pub const CONF_LANG_GERMAN: u32 = 2;
pub const CONF_LANG_FRENCH: u32 = 3;
pub const CONF_LANG_SPANISH: u32 = 4;
pub const CONF_LANG_ITALIAN: u32 = 5;
pub const CONF_LANG_DUTCH: u32 = 6;
pub const CONF_LANG_SIMP_CHINESE: u32 = 7;
pub const CONF_LANG_TRAD_CHINESE: u32 = 8;
pub const CONF_LANG_KOREAN: u32 = 9;

pub const CONF_ASPECT_4_3: u32 = 0;
pub const CONF_ASPECT_16_9: u32 = 1;

pub const CONF_SENSORBAR_BOTTOM: u32 = 0;
pub const CONF_SENSORBAR_TOP: u32 = 1;

pub const CONF_PAD_MAX_REGISTERED: usize = 10;
pub const CONF_PAD_MAX_ACTIVE: usize = 4;
pub const CONF_PAD_ACTIVE_AND_OTHER: usize = CONF_PAD_MAX_ACTIVE + 2;
pub const CONF_PAD_TOTAL: usize = CONF_PAD_MAX_REGISTERED + CONF_PAD_ACTIVE_AND_OTHER;

#[repr(C, packed)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ConfPadDevice {
    pub bdaddr: [u8; 6],
    pub name: [u8; 0x40],
}

impl Default for ConfPadDevice {
    fn default() -> Self {
        Self {
            bdaddr: [0; 6],
            name: [0; 0x40],
        }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ConfPads {
    pub num_registered: u8,
    pub registered: [ConfPadDevice; CONF_PAD_MAX_REGISTERED],
    pub active: [ConfPadDevice; CONF_PAD_MAX_ACTIVE],
    pub balance_board: ConfPadDevice,
    pub unknown: ConfPadDevice,
}

impl Default for ConfPads {
    fn default() -> Self {
        Self {
            num_registered: 0,
            registered: [ConfPadDevice::default(); CONF_PAD_MAX_REGISTERED],
            active: [ConfPadDevice::default(); CONF_PAD_MAX_ACTIVE],
            balance_board: ConfPadDevice::default(),
            unknown: ConfPadDevice::default(),
        }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ConfPadGuestDevice {
    pub bdaddr: [u8; 6],
    pub name: [u8; 0x40],
    pub link_key: [u8; 16],
}

impl Default for ConfPadGuestDevice {
    fn default() -> Self {
        Self {
            bdaddr: [0; 6],
            name: [0; 0x40],
            link_key: [0; 16],
        }
    }
}

#[repr(C, packed)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ConfPadGuests {
    pub num_guests: u8,
    pub guests: [ConfPadGuestDevice; CONF_PAD_MAX_ACTIVE],
    pub unknown: [ConfPadGuestDevice; 2],
}

impl Default for ConfPadGuests {
    fn default() -> Self {
        Self {
            num_guests: 0,
            guests: [ConfPadGuestDevice::default(); CONF_PAD_MAX_ACTIVE],
            unknown: [ConfPadGuestDevice::default(); 2],
        }
    }
}
