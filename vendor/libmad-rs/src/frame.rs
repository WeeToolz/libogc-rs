//! MPEG header fields and frame boundary parsing from `frame.c`.

use crate::timer::Timer;
use crate::{
    bit::{BitError, BitPtr},
    stream::{Error, MAD_BUFFER_GUARD, Stream},
};

pub const FLAG_NPRIVATE_III: u32 = 0x0007;
pub const FLAG_INCOMPLETE: u32 = 0x0008;
pub const FLAG_PROTECTION: u32 = 0x0010;
pub const FLAG_COPYRIGHT: u32 = 0x0020;
pub const FLAG_ORIGINAL: u32 = 0x0040;
pub const FLAG_PADDING: u32 = 0x0080;
pub const FLAG_I_STEREO: u32 = 0x0100;
pub const FLAG_MS_STEREO: u32 = 0x0200;
pub const FLAG_FREEFORMAT: u32 = 0x0400;
pub const FLAG_LSF_EXT: u32 = 0x1000;
pub const FLAG_MC_EXT: u32 = 0x2000;
pub const FLAG_MPEG_2_5_EXT: u32 = 0x4000;
pub const PRIVATE_HEADER: u32 = 0x0100;
pub const PRIVATE_III: u32 = 0x001f;
pub const OPTION_IGNORECRC: u32 = 1;
pub const OPTION_HALFSAMPLERATE: u32 = 2;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Layer {
    Unknown = 0,
    I = 1,
    II = 2,
    III = 3,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Mode {
    SingleChannel = 0,
    DualChannel = 1,
    JointStereo = 2,
    Stereo = 3,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum Emphasis {
    None = 0,
    FiftyFifteenUs = 1,
    Reserved = 2,
    CcittJ17 = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Header {
    pub layer: Layer,
    pub mode: Mode,
    pub mode_extension: u8,
    pub emphasis: Emphasis,
    pub bitrate: u32,
    pub samplerate: u32,
    pub crc_check: u16,
    pub crc_target: u16,
    pub flags: u32,
    pub private_bits: u32,
    pub duration: Timer,
}

/// Decoded libmad frame data. Layer I and II fill `sbsample` directly, in
/// channel, time-slot, subband order, as `frame.c` expects.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    pub header: Header,
    pub options: u32,
    pub sbsample: [[[i32; 32]; 36]; 2],
}

impl Default for Frame {
    fn default() -> Self {
        Self {
            header: Header::default(),
            options: 0,
            sbsample: [[[0; 32]; 36]; 2],
        }
    }
}

impl Frame {
    pub fn mute(&mut self) {
        self.sbsample = [[[0; 32]; 36]; 2];
    }
}
impl Default for Header {
    fn default() -> Self {
        Self {
            layer: Layer::Unknown,
            mode: Mode::SingleChannel,
            mode_extension: 0,
            emphasis: Emphasis::None,
            bitrate: 0,
            samplerate: 0,
            crc_check: 0,
            crc_target: 0,
            flags: 0,
            private_bits: 0,
            duration: Timer::ZERO,
        }
    }
}
impl Header {
    pub fn channels(&self) -> usize {
        if self.mode == Mode::SingleChannel {
            1
        } else {
            2
        }
    }
    pub fn subband_samples(&self) -> usize {
        if self.layer == Layer::I {
            12
        } else if self.layer == Layer::III && self.flags & FLAG_LSF_EXT != 0 {
            18
        } else {
            36
        }
    }
    pub fn decode(stream: &mut Stream) -> Result<Self, Error> {
        decode_header(stream)
    }
}

const BITRATES: [[u32; 15]; 5] = [
    [
        0, 32000, 64000, 96000, 128000, 160000, 192000, 224000, 256000, 288000, 320000, 352000,
        384000, 416000, 448000,
    ],
    [
        0, 32000, 48000, 56000, 64000, 80000, 96000, 112000, 128000, 160000, 192000, 224000,
        256000, 320000, 384000,
    ],
    [
        0, 32000, 40000, 48000, 56000, 64000, 80000, 96000, 112000, 128000, 160000, 192000, 224000,
        256000, 320000,
    ],
    [
        0, 32000, 48000, 56000, 64000, 80000, 96000, 112000, 128000, 144000, 160000, 176000,
        192000, 224000, 256000,
    ],
    [
        0, 8000, 16000, 24000, 32000, 40000, 48000, 56000, 64000, 80000, 96000, 112000, 128000,
        144000, 160000,
    ],
];
const SAMPLERATES: [u32; 3] = [44100, 48000, 32000];

fn read(stream: &Stream, ptr: &mut BitPtr, n: u32) -> Result<u32, Error> {
    ptr.read(&stream.buffer, n).map_err(|e| match e {
        BitError::TooManyBits => Error::LostSync,
        BitError::OutOfBounds => Error::BufferLength,
    })
}

fn detect_free_bitrate(stream: &Stream, header: &Header, from: usize) -> Option<u32> {
    let layer_slots = if header.layer == Layer::III && header.flags & FLAG_LSF_EXT != 0 {
        72
    } else {
        144
    };
    let padded = u32::from(header.flags & FLAG_PADDING != 0);
    let mut from = from;
    while from + MAD_BUFFER_GUARD <= stream.buffer.len() {
        let candidate = (from..stream.buffer.len().saturating_sub(1))
            .find(|&i| stream.buffer[i] == 0xff && stream.buffer[i + 1] & 0xe0 == 0xe0)?;
        if stream.buffer.len() - candidate < MAD_BUFFER_GUARD {
            return None;
        }
        let mut p = BitPtr::new(candidate);
        let parsed = (|| {
            read(stream, &mut p, 11)?;
            let is_25 = read(stream, &mut p, 1)? == 0;
            let lsf = read(stream, &mut p, 1)? == 0;
            if is_25 && !lsf {
                return Err(Error::LostSync);
            }
            let raw_layer = read(stream, &mut p, 2)?;
            let layer = match raw_layer {
                1 => Layer::III,
                2 => Layer::II,
                3 => Layer::I,
                _ => return Err(Error::BadLayer),
            };
            read(stream, &mut p, 1)?;
            if read(stream, &mut p, 4)? == 15 {
                return Err(Error::BadBitrate);
            }
            let sr_index = read(stream, &mut p, 2)? as usize;
            if sr_index == 3 {
                return Err(Error::BadSampleRate);
            }
            let mut rate = SAMPLERATES[sr_index];
            if lsf {
                rate /= 2;
                if is_25 {
                    rate /= 2;
                }
            }
            Ok((layer, rate))
        })();
        if let Ok((layer, sample_rate)) = parsed
            && layer == header.layer
            && sample_rate == header.samplerate
        {
            let distance = candidate - stream.this_frame;
            let bitrate_kbps = if header.layer == Layer::I {
                (header.samplerate as usize * (distance - 4 * padded as usize + 4)) / 48 / 1000
            } else {
                (header.samplerate as usize * (distance - padded as usize + 1))
                    / layer_slots as usize
                    / 1000
            };
            if bitrate_kbps >= 8 {
                if header.layer == Layer::III && bitrate_kbps > 640 {
                    return None;
                }
                return Some((bitrate_kbps * 1000) as u32);
            }
        }
        from = candidate + 1;
    }
    None
}
pub fn decode_header(stream: &mut Stream) -> Result<Header, Error> {
    let end = stream.buffer.len();
    let mut pos = stream.next_frame;
    if !stream.buffer_set {
        stream.error = Error::BufferPointer;
        return Err(stream.error);
    }
    if stream.skiplen != 0 {
        if !stream.sync {
            pos = stream.this_frame;
        }
        let available = end.saturating_sub(pos);
        if available < stream.skiplen as usize {
            stream.skiplen -= available as u32;
            stream.next_frame = end;
            stream.error = Error::BufferLength;
            stream.sync = false;
            return Err(stream.error);
        }
        pos += stream.skiplen as usize;
        stream.skiplen = 0;
        stream.sync = true;
    }
    'candidate: loop {
        if stream.sync {
            if end.saturating_sub(pos) < MAD_BUFFER_GUARD {
                stream.next_frame = pos;
                stream.error = Error::BufferLength;
                stream.sync = false;
                return Err(stream.error);
            }
            if end.saturating_sub(pos) >= 2
                && !(stream.buffer[pos] == 0xff && stream.buffer[pos + 1] & 0xe0 == 0xe0)
            {
                stream.this_frame = pos;
                stream.next_frame = pos + 1;
                stream.error = Error::LostSync;
                stream.sync = false;
                return Err(stream.error);
            }
        } else {
            stream.ptr = BitPtr::new(pos);
            if stream.sync_to_next().is_err() {
                if end.saturating_sub(stream.next_frame) >= MAD_BUFFER_GUARD {
                    stream.next_frame = end - MAD_BUFFER_GUARD;
                }
                stream.error = Error::BufferLength;
                return Err(stream.error);
            }
            pos = stream.ptr.next_byte();
        }
        stream.error = Error::None;
        stream.this_frame = pos;
        stream.next_frame = pos.saturating_add(1);
        let mut p = BitPtr::new(pos);
        if end.saturating_sub(pos) * 8 < 32 {
            stream.error = Error::BufferLength;
            stream.sync = false;
            return Err(stream.error);
        }
        read(stream, &mut p, 11)?;
        let mut h = Header::default();
        if read(stream, &mut p, 1)? == 0 {
            h.flags |= FLAG_MPEG_2_5_EXT;
        }
        if read(stream, &mut p, 1)? == 0 {
            h.flags |= FLAG_LSF_EXT;
        } else if h.flags & FLAG_MPEG_2_5_EXT != 0 {
            stream.error = Error::LostSync;
            stream.sync = false;
            return Err(stream.error);
        }
        let layer = 4 - read(stream, &mut p, 2)?;
        h.layer = match layer {
            1 => Layer::I,
            2 => Layer::II,
            3 => Layer::III,
            _ => {
                stream.error = Error::BadLayer;
                stream.sync = false;
                return Err(stream.error);
            }
        };
        if read(stream, &mut p, 1)? == 0 {
            h.flags |= FLAG_PROTECTION;
            h.crc_check =
                crate::bit::crc(p, &stream.buffer, 16, 0xffff).map_err(|_| Error::BufferLength)?;
        }
        let bi = read(stream, &mut p, 4)? as usize;
        if bi == 15 {
            stream.error = Error::BadBitrate;
            stream.sync = false;
            return Err(stream.error);
        }
        h.bitrate = if h.flags & FLAG_LSF_EXT != 0 {
            BITRATES[3 + (h.layer as usize / 2)][bi]
        } else {
            BITRATES[h.layer as usize - 1][bi]
        };
        let si = read(stream, &mut p, 2)? as usize;
        if si == 3 {
            stream.error = Error::BadSampleRate;
            stream.sync = false;
            return Err(stream.error);
        }
        h.samplerate = SAMPLERATES[si];
        if h.flags & FLAG_LSF_EXT != 0 {
            h.samplerate /= 2;
            if h.flags & FLAG_MPEG_2_5_EXT != 0 {
                h.samplerate /= 2;
            }
        }
        if read(stream, &mut p, 1)? != 0 {
            h.flags |= FLAG_PADDING;
        }
        if read(stream, &mut p, 1)? != 0 {
            h.private_bits |= PRIVATE_HEADER;
        }
        h.mode = match 3 - read(stream, &mut p, 2)? {
            0 => Mode::SingleChannel,
            1 => Mode::DualChannel,
            2 => Mode::JointStereo,
            _ => Mode::Stereo,
        };
        h.mode_extension = read(stream, &mut p, 2)? as u8;
        if read(stream, &mut p, 1)? != 0 {
            h.flags |= FLAG_COPYRIGHT;
        }
        if read(stream, &mut p, 1)? != 0 {
            h.flags |= FLAG_ORIGINAL;
        }
        h.emphasis = match read(stream, &mut p, 2)? {
            0 => Emphasis::None,
            1 => Emphasis::FiftyFifteenUs,
            2 => Emphasis::Reserved,
            _ => Emphasis::CcittJ17,
        };
        if h.flags & FLAG_PROTECTION != 0 {
            if (end - p.byte) * 8 - (8 - p.left as usize) < 16 {
                stream.error = Error::BufferLength;
                stream.sync = false;
                return Err(stream.error);
            }
            h.crc_target = read(stream, &mut p, 16)? as u16;
        }
        let samples = 32 * h.subband_samples() as u64;
        h.duration.set(0, samples, h.samplerate as u64);
        stream.ptr = p;
        if h.bitrate == 0 {
            let rate = if stream.freerate == 0
                || !stream.sync
                || (h.layer == Layer::III && stream.freerate > 640000)
            {
                detect_free_bitrate(stream, &h, p.next_byte())
            } else {
                Some(stream.freerate)
            };
            let Some(rate) = rate else {
                stream.error = Error::LostSync;
                stream.sync = false;
                return Err(stream.error);
            };
            stream.freerate = rate;
            h.bitrate = rate;
            h.flags |= FLAG_FREEFORMAT;
        }
        let pad = u32::from(h.flags & FLAG_PADDING != 0);
        let slots = if h.layer == Layer::III && h.flags & FLAG_LSF_EXT != 0 {
            72
        } else {
            144
        };
        let frame_len = if h.layer == Layer::I {
            ((12 * h.bitrate / h.samplerate) + pad) * 4
        } else {
            slots * h.bitrate / h.samplerate + pad
        } as usize;
        if frame_len.saturating_add(MAD_BUFFER_GUARD) > end.saturating_sub(stream.this_frame) {
            stream.next_frame = stream.this_frame;
            stream.error = Error::BufferLength;
            stream.sync = false;
            return Err(stream.error);
        }
        stream.next_frame = stream.this_frame + frame_len;
        if !stream.sync {
            let next = stream.next_frame;
            if end.saturating_sub(next) >= 2
                && !(stream.buffer[next] == 0xff && stream.buffer[next + 1] & 0xe0 == 0xe0)
            {
                stream.next_frame = stream.this_frame + 1;
                stream.ptr = BitPtr::new(stream.next_frame);
                stream.sync = false;
                pos = stream.next_frame;
                continue 'candidate;
            }
            stream.sync = true;
        }
        h.flags |= FLAG_INCOMPLETE;
        stream.ptr = p;
        stream.error = Error::None;
        return Ok(h);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_mpeg1_layer3_header_and_frame_boundary() {
        let mut bytes = vec![0xff, 0xfb, 0x90, 0x64];
        bytes.resize(417 + 8, 0);
        let mut s = Stream::new();
        s.buffer(&bytes);
        let h = decode_header(&mut s).unwrap();
        assert_eq!(
            (h.layer, h.mode, h.bitrate, h.samplerate),
            (Layer::III, Mode::JointStereo, 128000, 44100)
        );
        assert_eq!(s.this_frame, 0);
        assert_eq!(s.next_frame, 417);
        assert_ne!(h.flags & FLAG_INCOMPLETE, 0);
    }
    #[test]
    fn rejects_reserved_layer_rate_and_sampling_index() {
        let mut s = Stream::new();
        s.buffer(&[0xff, 0xe0, 0x90, 0x64, 0, 0, 0, 0]);
        assert_eq!(decode_header(&mut s), Err(Error::BadLayer));
        let mut s = Stream::new();
        s.buffer(&[0xff, 0xfb, 0x9c, 0x64, 0, 0, 0, 0]);
        assert_eq!(decode_header(&mut s), Err(Error::BadSampleRate));
    }
    #[test]
    fn protected_header_reads_crc_word_and_flags() {
        let mut bytes = vec![0xff, 0xfa, 0x90, 0x64, 0x12, 0x34];
        bytes.resize(417 + 8, 0);
        let mut s = Stream::new();
        s.buffer(&bytes);
        let h = decode_header(&mut s).unwrap();
        assert_eq!(h.crc_target, 0x1234);
        assert_ne!(h.flags & FLAG_PROTECTION, 0);
    }

    #[test]
    fn detects_free_format_bitrate_from_following_sync() {
        let mut bytes = vec![0; 417 + 8];
        bytes[..4].copy_from_slice(&[0xff, 0xfb, 0x00, 0x64]);
        bytes[417..421].copy_from_slice(&[0xff, 0xfb, 0x00, 0x64]);
        let mut s = Stream::new();
        s.buffer(&bytes);
        let h = decode_header(&mut s).unwrap();
        assert_eq!(h.bitrate, 128_000);
        assert_ne!(h.flags & FLAG_FREEFORMAT, 0);
        assert_eq!(s.freerate, 128_000);
    }

    #[test]
    fn scanning_retries_after_a_false_frame_boundary() {
        let mut bytes = vec![0; 417 + 13 + 417 + 417 + 8];
        bytes[..4].copy_from_slice(&[0xff, 0xfb, 0x90, 0x64]);
        bytes[430..434].copy_from_slice(&[0xff, 0xfb, 0x90, 0x64]);
        bytes[847..851].copy_from_slice(&[0xff, 0xfb, 0x90, 0x64]);
        let mut s = Stream::new();
        s.buffer(&bytes);
        s.sync = false;
        let h = decode_header(&mut s).unwrap();
        assert_eq!(h.layer, Layer::III);
        assert_eq!(s.this_frame, 430);
    }
}
