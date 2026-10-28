//! C-compatible public data layouts and constants from libmad headers.
//!
//! These layouts use the 32-bit integer widths of the Wii/GameCube ABI. In
//! particular, C `long` and `unsigned long` are 32 bits on that target.

use core::ffi::c_void;

pub type MadFixedT = i32;
pub type MadSampleT = MadFixedT;

pub const MAD_VERSION_MAJOR: i32 = 0;
pub const MAD_VERSION_MINOR: i32 = 15;
pub const MAD_VERSION_PATCH: i32 = 0;
pub const MAD_VERSION_EXTRA: &str = " (beta)";
pub static MAD_VERSION: &[u8; 33] = b"MPEG Audio Decoder 0.15.0 (beta)\0";
pub static MAD_COPYRIGHT: &[u8; 52] = b"Copyright (C) 2000-2003 Underbit Technologies, Inc.\0";
pub static MAD_AUTHOR: &[u8; 48] = b"Underbit Technologies, Inc. <info@underbit.com>\0";
// The Wii/GameCube build selects FPM_PPC in its config.h.
pub static MAD_BUILD: &[u8; 9] = b"FPM_PPC \0";
pub const MAD_F_FRACBITS: i32 = 28;
pub const MAD_F_ONE: MadFixedT = 0x1000_0000;
pub const MAD_F_MIN: MadFixedT = i32::MIN;
pub const MAD_F_MAX: MadFixedT = i32::MAX;

pub const MAD_ERROR_NONE: i32 = libmad_rs::Error::None as i32;
pub const MAD_ERROR_BUFLEN: i32 = libmad_rs::Error::BufferLength as i32;
pub const MAD_ERROR_BUFPTR: i32 = libmad_rs::Error::BufferPointer as i32;
pub const MAD_ERROR_NOMEM: i32 = libmad_rs::Error::NoMemory as i32;
pub const MAD_ERROR_LOSTSYNC: i32 = libmad_rs::Error::LostSync as i32;
pub const MAD_ERROR_BADLAYER: i32 = libmad_rs::Error::BadLayer as i32;
pub const MAD_ERROR_BADBITRATE: i32 = libmad_rs::Error::BadBitrate as i32;
pub const MAD_ERROR_BADSAMPLERATE: i32 = libmad_rs::Error::BadSampleRate as i32;
pub const MAD_ERROR_BADEMPHASIS: i32 = libmad_rs::Error::BadEmphasis as i32;
pub const MAD_ERROR_BADCRC: i32 = libmad_rs::Error::BadCrc as i32;
pub const MAD_ERROR_BADBITALLOC: i32 = libmad_rs::Error::BadBitAllocation as i32;
pub const MAD_ERROR_BADSCALEFACTOR: i32 = libmad_rs::Error::BadScaleFactor as i32;
pub const MAD_ERROR_BADMODE: i32 = libmad_rs::Error::BadMode as i32;
pub const MAD_ERROR_BADFRAMELEN: i32 = libmad_rs::Error::BadFrameLength as i32;
pub const MAD_ERROR_BADBIGVALUES: i32 = libmad_rs::Error::BadBigValues as i32;
pub const MAD_ERROR_BADBLOCKTYPE: i32 = libmad_rs::Error::BadBlockType as i32;
pub const MAD_ERROR_BADSCFSI: i32 = libmad_rs::Error::BadScfsi as i32;
pub const MAD_ERROR_BADDATAPTR: i32 = libmad_rs::Error::BadDataPointer as i32;
pub const MAD_ERROR_BADPART3LEN: i32 = libmad_rs::Error::BadPart3Length as i32;
pub const MAD_ERROR_BADHUFFTABLE: i32 = libmad_rs::Error::BadHuffmanTable as i32;
pub const MAD_ERROR_BADHUFFDATA: i32 = libmad_rs::Error::BadHuffmanData as i32;
pub const MAD_ERROR_BADSTEREO: i32 = libmad_rs::Error::BadStereo as i32;

pub const MAD_OPTION_IGNORECRC: i32 = 0x0001;
pub const MAD_OPTION_HALFSAMPLERATE: i32 = 0x0002;

pub const MAD_LAYER_I: i32 = 1;
pub const MAD_LAYER_II: i32 = 2;
pub const MAD_LAYER_III: i32 = 3;
pub const MAD_MODE_SINGLE_CHANNEL: i32 = 0;
pub const MAD_MODE_DUAL_CHANNEL: i32 = 1;
pub const MAD_MODE_JOINT_STEREO: i32 = 2;
pub const MAD_MODE_STEREO: i32 = 3;
pub const MAD_EMPHASIS_NONE: i32 = 0;
pub const MAD_EMPHASIS_50_15_US: i32 = 1;
pub const MAD_EMPHASIS_RESERVED: i32 = 2;
pub const MAD_EMPHASIS_CCITT_J_17: i32 = 3;

pub const MAD_FLAG_NPRIVATE_III: i32 = 0x0007;
pub const MAD_FLAG_INCOMPLETE: i32 = 0x0008;
pub const MAD_FLAG_PROTECTION: i32 = 0x0010;
pub const MAD_FLAG_COPYRIGHT: i32 = 0x0020;
pub const MAD_FLAG_ORIGINAL: i32 = 0x0040;
pub const MAD_FLAG_PADDING: i32 = 0x0080;
pub const MAD_FLAG_I_STEREO: i32 = 0x0100;
pub const MAD_FLAG_MS_STEREO: i32 = 0x0200;
pub const MAD_FLAG_FREEFORMAT: i32 = 0x0400;
pub const MAD_FLAG_LSF_EXT: i32 = 0x1000;
pub const MAD_FLAG_MC_EXT: i32 = 0x2000;
pub const MAD_FLAG_MPEG_2_5_EXT: i32 = 0x4000;
pub const MAD_PRIVATE_HEADER: i32 = 0x0100;
pub const MAD_PRIVATE_III: i32 = 0x001f;
pub const MAD_BUFFER_GUARD: usize = 8;
pub const MAD_BUFFER_MDLEN: usize = 511 + 2048 + MAD_BUFFER_GUARD;
pub const MAD_TIMER_RESOLUTION: u32 = 352_800_000;

pub const MAD_FLOW_CONTINUE: i32 = 0x0000;
pub const MAD_FLOW_STOP: i32 = 0x0010;
pub const MAD_FLOW_BREAK: i32 = 0x0011;
pub const MAD_FLOW_IGNORE: i32 = 0x0020;
pub const MAD_DECODER_MODE_SYNC: i32 = 0;
pub const MAD_DECODER_MODE_ASYNC: i32 = 1;

pub const MAD_UNITS_HOURS: i32 = -2;
pub const MAD_UNITS_MINUTES: i32 = -1;
pub const MAD_UNITS_SECONDS: i32 = 0;
pub const MAD_UNITS_DECISECONDS: i32 = 10;
pub const MAD_UNITS_CENTISECONDS: i32 = 100;
pub const MAD_UNITS_MILLISECONDS: i32 = 1_000;
pub const MAD_UNITS_8000_HZ: i32 = 8_000;
pub const MAD_UNITS_11025_HZ: i32 = 11_025;
pub const MAD_UNITS_12000_HZ: i32 = 12_000;
pub const MAD_UNITS_16000_HZ: i32 = 16_000;
pub const MAD_UNITS_22050_HZ: i32 = 22_050;
pub const MAD_UNITS_24000_HZ: i32 = 24_000;
pub const MAD_UNITS_32000_HZ: i32 = 32_000;
pub const MAD_UNITS_44100_HZ: i32 = 44_100;
pub const MAD_UNITS_48000_HZ: i32 = 48_000;
pub const MAD_UNITS_24_FPS: i32 = 24;
pub const MAD_UNITS_25_FPS: i32 = 25;
pub const MAD_UNITS_30_FPS: i32 = 30;
pub const MAD_UNITS_48_FPS: i32 = 48;
pub const MAD_UNITS_50_FPS: i32 = 50;
pub const MAD_UNITS_60_FPS: i32 = 60;
pub const MAD_UNITS_75_FPS: i32 = 75;
pub const MAD_UNITS_23_976_FPS: i32 = -24;
pub const MAD_UNITS_24_975_FPS: i32 = -25;
pub const MAD_UNITS_29_97_FPS: i32 = -30;
pub const MAD_UNITS_47_952_FPS: i32 = -48;
pub const MAD_UNITS_49_95_FPS: i32 = -50;
pub const MAD_UNITS_59_94_FPS: i32 = -60;

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct MadBitPtr {
    pub byte: *const u8,
    pub cache: u16,
    pub left: u16,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct MadTimerT {
    pub seconds: i32,
    pub fraction: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct MadStream {
    pub buffer: *const u8,
    pub bufend: *const u8,
    pub skiplen: u32,
    pub sync: i32,
    pub freerate: u32,
    pub this_frame: *const u8,
    pub next_frame: *const u8,
    pub ptr: MadBitPtr,
    pub anc_ptr: MadBitPtr,
    pub anc_bitlen: u32,
    pub main_data: *mut c_void,
    pub md_len: u32,
    pub options: i32,
    pub error: i32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct MadHeader {
    pub layer: i32,
    pub mode: i32,
    pub mode_extension: i32,
    pub emphasis: i32,
    pub bitrate: u32,
    pub samplerate: u32,
    pub crc_check: u16,
    pub crc_target: u16,
    pub flags: i32,
    pub private_bits: i32,
    pub duration: MadTimerT,
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct MadFrame {
    pub header: MadHeader,
    pub options: i32,
    pub sbsample: [[[MadFixedT; 32]; 36]; 2],
    pub overlap: *mut c_void,
}

impl Default for MadFrame {
    fn default() -> Self {
        Self {
            header: MadHeader::default(),
            options: 0,
            sbsample: [[[0; 32]; 36]; 2],
            overlap: core::ptr::null_mut(),
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct MadPcm {
    pub samplerate: u32,
    pub channels: u16,
    pub length: u16,
    pub samples: [[MadFixedT; 1152]; 2],
}

impl Default for MadPcm {
    fn default() -> Self {
        Self {
            samplerate: 0,
            channels: 0,
            length: 0,
            samples: [[0; 1152]; 2],
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct MadSynth {
    pub filter: [[[[[MadFixedT; 8]; 16]; 2]; 2]; 2],
    pub phase: u32,
    pub pcm: MadPcm,
}

pub type MadInputFn = Option<unsafe extern "C" fn(*mut c_void, *mut MadStream) -> i32>;
pub type MadHeaderFn = Option<unsafe extern "C" fn(*mut c_void, *const MadHeader) -> i32>;
pub type MadFilterFn =
    Option<unsafe extern "C" fn(*mut c_void, *const MadStream, *mut MadFrame) -> i32>;
pub type MadOutputFn =
    Option<unsafe extern "C" fn(*mut c_void, *const MadHeader, *mut MadPcm) -> i32>;
pub type MadErrorFn =
    Option<unsafe extern "C" fn(*mut c_void, *mut MadStream, *mut MadFrame) -> i32>;
pub type MadMessageFn = Option<unsafe extern "C" fn(*mut c_void, *mut c_void, *mut u32) -> i32>;

#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct MadDecoderAsync {
    pub pid: i32,
    pub in_: i32,
    pub out: i32,
}

impl Default for MadDecoderAsync {
    fn default() -> Self {
        Self {
            pid: 0,
            in_: -1,
            out: -1,
        }
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct MadDecoderSync {
    pub stream: MadStream,
    pub frame: MadFrame,
    pub synth: MadSynth,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct MadDecoder {
    pub mode: i32,
    pub options: i32,
    pub async_: MadDecoderAsync,
    pub sync: *mut MadDecoderSync,
    pub cb_data: *mut c_void,
    pub input_func: MadInputFn,
    pub header_func: MadHeaderFn,
    pub filter_func: MadFilterFn,
    pub output_func: MadOutputFn,
    pub error_func: MadErrorFn,
    pub message_func: MadMessageFn,
}

#[derive(Copy, Clone, Debug, Default)]
pub struct MadDecoderCallbacks {
    pub input: MadInputFn,
    pub header: MadHeaderFn,
    pub filter: MadFilterFn,
    pub output: MadOutputFn,
    pub error: MadErrorFn,
    pub message: MadMessageFn,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_metadata_matches_the_c_reference() {
        assert_eq!(MAD_VERSION_MAJOR, 0);
        assert_eq!(MAD_VERSION_MINOR, 15);
        assert_eq!(MAD_VERSION_PATCH, 0);
        assert_eq!(MAD_VERSION_EXTRA, " (beta)");
        assert_eq!(MAD_VERSION, b"MPEG Audio Decoder 0.15.0 (beta)\0");
        assert_eq!(
            MAD_COPYRIGHT,
            b"Copyright (C) 2000-2003 Underbit Technologies, Inc.\0"
        );
        assert_eq!(MAD_BUILD, b"FPM_PPC \0");
    }
}
