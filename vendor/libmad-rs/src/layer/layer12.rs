//! Layer I decoding from libmad's `layer12.c`.
//!
//! Decoding fills the 32-band subband sample plane used by libmad synthesis.

use super::{bits_remaining, check_crc, read_bits as read, requantize};
use crate::{
    bit::BitPtr,
    fixed,
    frame::{FLAG_I_STEREO, FLAG_PROTECTION, Frame, Mode},
    stream::{Error, Stream},
    tables::layer12::SCALEFACTORS,
};

const LINEAR: [i32; 14] = [
    0x1555_5555,
    0x1249_2492,
    0x1111_1111,
    0x1084_2108,
    0x1041_0410,
    0x1020_4081,
    0x1010_1010,
    0x1008_0402,
    0x1004_0100,
    0x1002_0040,
    0x1001_0010,
    0x1000_8004,
    0x1000_4001,
    0x1000_2000,
];

fn sample(stream: &mut Stream, ptr: &mut BitPtr, bits: u32, end: usize) -> Result<i32, Error> {
    let raw = read(stream, ptr, bits, end)?;
    let centered = requantize(raw, bits).wrapping_add(fixed::ONE >> (bits - 1));
    Ok(fixed::mul(centered, LINEAR[(bits - 2) as usize]))
}

/// Decode one Layer I frame and preserve libmad's allocation, joint stereo,
/// scalefactor, and requantization order.
#[allow(clippy::needless_range_loop)]
pub fn decode_layer1(stream: &mut Stream, frame: &mut Frame) -> Result<(), Error> {
    let channels = frame.header.channels();
    let mut bound = if frame.header.mode == Mode::JointStereo {
        frame.header.flags |= FLAG_I_STEREO;
        4 + frame.header.mode_extension as usize * 4
    } else {
        32
    };
    bound = bound.min(32);
    let mut allocation = [[0_u8; 32]; 2];
    let mut scalefactor = [[0_u8; 32]; 2];
    let mut ptr = stream.ptr;
    let frame_end = stream.next_frame;

    if frame.header.flags & FLAG_PROTECTION != 0 {
        let crc_len = 4 * (bound * channels + 32 - bound) as u32;
        if bits_remaining(ptr, stream.buffer.len()) < crc_len as isize {
            stream.error = Error::BadCrc;
            return Err(Error::BadCrc);
        }
        check_crc(frame, stream, ptr, crc_len)?;
    }
    for sb in 0..bound {
        for ch in 0..channels {
            let value = read(stream, &mut ptr, 4, frame_end)? as u8;
            if value == 15 {
                stream.error = Error::BadBitAllocation;
                return Err(Error::BadBitAllocation);
            }
            allocation[ch][sb] = if value == 0 { 0 } else { value + 1 };
        }
    }
    for sb in bound..32 {
        let value = read(stream, &mut ptr, 4, frame_end)? as u8;
        if value == 15 {
            stream.error = Error::BadBitAllocation;
            return Err(Error::BadBitAllocation);
        }
        allocation[0][sb] = if value == 0 { 0 } else { value + 1 };
        allocation[1][sb] = allocation[0][sb];
    }
    for sb in 0..32 {
        for ch in 0..channels {
            if allocation[ch][sb] != 0 {
                scalefactor[ch][sb] = read(stream, &mut ptr, 6, frame_end)? as u8;
            }
        }
    }
    for slot in 0..12 {
        for sb in 0..bound {
            for ch in 0..channels {
                let bits = allocation[ch][sb] as u32;
                frame.sbsample[ch][slot][sb] = if bits == 0 {
                    0
                } else {
                    fixed::mul(
                        sample(stream, &mut ptr, bits, frame_end)?,
                        SCALEFACTORS[scalefactor[ch][sb] as usize],
                    )
                };
            }
        }
        for sb in bound..32 {
            let bits = allocation[0][sb] as u32;
            if bits == 0 {
                for ch in 0..channels {
                    frame.sbsample[ch][slot][sb] = 0;
                }
            } else {
                let value = sample(stream, &mut ptr, bits, frame_end)?;
                for ch in 0..channels {
                    frame.sbsample[ch][slot][sb] =
                        fixed::mul(value, SCALEFACTORS[scalefactor[ch][sb] as usize]);
                }
            }
        }
    }
    stream.ptr = ptr;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::Layer;
    use crate::layer::requantize;

    fn blank_frame(bytes: usize) -> (Stream, Frame) {
        let mut stream = Stream::new();
        stream.buffer(&alloc::vec![0; bytes + 8]);
        stream.this_frame = 0;
        stream.next_frame = bytes;
        stream.ptr = BitPtr::new(4);
        let mut frame = Frame::default();
        frame.header.layer = Layer::I;
        frame.header.mode = Mode::SingleChannel;
        frame.header.bitrate = 32000;
        frame.header.samplerate = 44100;
        (stream, frame)
    }

    #[test]
    fn allocated_samples_cannot_read_guard_bytes_past_frame_end() {
        // 128 allocation bits, 6 scalefactor bits, and twelve 5-bit samples
        // require 194 payload bits. Including the header requires 29 bytes.
        for bytes in [28, 29] {
            let (mut stream, mut frame) = blank_frame(bytes);
            stream.buffer[4] = 0x40; // Subband zero uses five bits per sample.
            stream.sync = true;
            frame.sbsample[0][11][0] = 123;
            let result = decode_layer1(&mut stream, &mut frame);
            assert!(frame.sbsample[0][..11].iter().all(|slot| slot[0] != 0));
            if bytes == 28 {
                assert_eq!(result, Err(Error::LostSync));
                assert_eq!(stream.error, Error::LostSync);
                assert!(!stream.sync);
                assert_eq!(stream.ptr.byte, 27);
                assert_eq!(stream.ptr.left, 3);
                assert_eq!(frame.sbsample[0][11][0], 123);
            } else {
                assert_eq!(result, Ok(()));
                assert_eq!(stream.ptr.byte, 28);
                assert_eq!(stream.ptr.left, 6);
                assert_ne!(frame.sbsample[0][11][0], 123);
            }
        }
    }

    #[test]
    fn decodes_silence_into_subband_samples() {
        let (mut stream, mut frame) = blank_frame(32);
        decode_layer1(&mut stream, &mut frame).unwrap();
        assert!(frame.sbsample[0][..12].iter().flatten().all(|&v| v == 0));
        assert_eq!(stream.ptr.byte, 20);
    }

    #[test]
    fn requantizes_an_allocated_subband_for_each_sample_slot() {
        let (mut stream, mut frame) = blank_frame(32);
        let mut ptr = BitPtr::new(4);
        ptr.write(&mut stream.buffer, 4, 1).unwrap();
        for _ in 0..31 {
            ptr.write(&mut stream.buffer, 4, 0).unwrap();
        }
        ptr.write(&mut stream.buffer, 6, 3).unwrap();
        for _ in 0..12 {
            ptr.write(&mut stream.buffer, 2, 0).unwrap();
        }
        decode_layer1(&mut stream, &mut frame).unwrap();
        let sample = frame.sbsample[0][0][0];
        // Expected from layer12.c: (signed sample + bias) * linear * sf[3].
        assert_eq!(sample, -178_956_971);
        assert!(frame.sbsample[0][..12].iter().all(|slot| slot[0] == sample));
        assert!(
            frame.sbsample[0][..12]
                .iter()
                .all(|slot| slot[1..].iter().all(|&x| x == 0))
        );
    }

    #[test]
    fn rejects_reserved_bit_allocation() {
        let (mut stream, mut frame) = blank_frame(32);
        stream.buffer[4] = 0xf0;
        assert_eq!(
            decode_layer1(&mut stream, &mut frame),
            Err(Error::BadBitAllocation)
        );
    }

    #[test]
    fn sign_extension_and_sign_bit_inversion_match_c() {
        assert_eq!(requantize(0b00, 2), -fixed::ONE);
        assert_eq!(requantize(0b01, 2), -(fixed::ONE / 2));
        assert_eq!(requantize(0b10, 2), 0);
        assert_eq!(requantize(0b11, 2), fixed::ONE / 2);
    }
}
