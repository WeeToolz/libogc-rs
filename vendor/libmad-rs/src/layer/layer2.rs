//! ISO/IEC Layer II allocation, scalefactor, requantization and stereo decode.

use super::{check_crc, read_bits as read, requantize};
use crate::bit::BitPtr;
use crate::{
    fixed,
    frame::{FLAG_FREEFORMAT, FLAG_I_STEREO, FLAG_LSF_EXT, FLAG_PROTECTION, Frame, Mode},
    stream::{Error, Stream},
    tables::{
        layer2::{QUANT_CLASSES, QuantClass},
        layer12::SCALEFACTORS,
    },
};

const SBQUANT: [(usize, &[u8]); 5] = [
    (
        27,
        &[
            7, 7, 7, 6, 6, 6, 6, 6, 6, 6, 6, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 0, 0, 0, 0,
        ],
    ),
    (
        30,
        &[
            7, 7, 7, 6, 6, 6, 6, 6, 6, 6, 6, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 0, 0, 0, 0, 0, 0,
            0,
        ],
    ),
    (8, &[5, 5, 2, 2, 2, 2, 2, 2]),
    (12, &[5, 5, 2, 2, 2, 2, 2, 2, 2, 2, 2, 2]),
    (
        30,
        &[
            4, 4, 4, 4, 2, 2, 2, 2, 2, 2, 2, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1,
            1,
        ],
    ),
];
const BITALLOC: [(u8, usize); 8] = [
    (2, 0),
    (2, 3),
    (3, 3),
    (3, 1),
    (4, 2),
    (4, 3),
    (4, 4),
    (4, 5),
];
const OFFSETS: [&[u8]; 6] = [
    &[0, 1, 16],
    &[0, 1, 2, 3, 4, 5, 16],
    &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14],
    &[0, 1, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15],
    &[0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 16],
    &[0, 2, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16],
];

fn layer2_samples(
    stream: &mut Stream,
    ptr: &mut BitPtr,
    q: QuantClass,
    end: usize,
) -> Result<[i32; 3], Error> {
    let mut raw = [0_u32; 3];
    if q.group != 0 {
        let mut code = read(stream, ptr, q.bits as u32, end)?;
        for item in &mut raw {
            *item = code % q.levels as u32;
            code /= q.levels as u32;
        }
    } else {
        for item in &mut raw {
            *item = read(stream, ptr, q.bits as u32, end)?;
        }
    }
    let mut output = [0; 3];
    for i in 0..3 {
        let x = requantize(raw[i], q.bits as u32).wrapping_add(q.d);
        output[i] = fixed::mul(x, q.c);
    }
    Ok(output)
}

/// Decode one Layer II frame into `Frame::sbsample`.
#[allow(clippy::needless_range_loop)]
pub fn decode_layer2(stream: &mut Stream, frame: &mut Frame) -> Result<(), Error> {
    let channels = frame.header.channels();
    let index = if frame.header.flags & FLAG_LSF_EXT != 0 {
        4
    } else if frame.header.flags & FLAG_FREEFORMAT != 0 {
        if frame.header.samplerate == 48000 {
            0
        } else {
            1
        }
    } else {
        let bpc = if channels == 2 {
            frame.header.bitrate / 2
        } else {
            frame.header.bitrate
        };
        if channels == 1 && bpc > 192000 {
            stream.error = Error::BadMode;
            return Err(Error::BadMode);
        }
        if bpc <= 48000 {
            if frame.header.samplerate == 32000 {
                3
            } else {
                2
            }
        } else if bpc <= 80000 || frame.header.samplerate == 48000 {
            0
        } else {
            1
        }
    };
    let (sblimit, offsets) = SBQUANT[index];
    let mut quant_offsets = [0_u8; 32];
    for sb in 0..sblimit {
        quant_offsets[sb] = BITALLOC[offsets[sb] as usize].1 as u8;
    }
    let mut bound = if frame.header.mode == Mode::JointStereo {
        frame.header.flags |= FLAG_I_STEREO;
        4 + frame.header.mode_extension as usize * 4
    } else {
        32
    };
    bound = bound.min(sblimit);
    let mut allocation = [[0_u8; 32]; 2];
    let mut scfsi = [[0_u8; 32]; 2];
    let mut scalefactor = [[[0_u8; 3]; 32]; 2];
    let start = stream.ptr;
    let mut ptr = start;
    let end = stream.next_frame;

    for sb in 0..bound {
        let nbal = BITALLOC[offsets[sb] as usize].0 as u32;
        for ch in 0..channels {
            allocation[ch][sb] = read(stream, &mut ptr, nbal, end)? as u8;
        }
    }
    for sb in bound..sblimit {
        let nbal = BITALLOC[offsets[sb] as usize].0 as u32;
        allocation[0][sb] = read(stream, &mut ptr, nbal, end)? as u8;
        allocation[1][sb] = allocation[0][sb];
    }
    for sb in 0..sblimit {
        for ch in 0..channels {
            if allocation[ch][sb] != 0 {
                scfsi[ch][sb] = read(stream, &mut ptr, 2, end)? as u8;
            }
        }
    }
    if frame.header.flags & FLAG_PROTECTION != 0 {
        let nbits = BitPtr::bit_length(start, ptr) as u32;
        check_crc(frame, stream, start, nbits)?;
    }
    for sb in 0..sblimit {
        for ch in 0..channels {
            if allocation[ch][sb] == 0 {
                continue;
            }
            let scale0 = read(stream, &mut ptr, 6, end)? as u8;
            scalefactor[ch][sb][0] = scale0;
            match scfsi[ch][sb] {
                2 => scalefactor[ch][sb] = [scale0; 3],
                0 => {
                    scalefactor[ch][sb][1] = read(stream, &mut ptr, 6, end)? as u8;
                    scalefactor[ch][sb][2] = read(stream, &mut ptr, 6, end)? as u8;
                }
                1 => {
                    scalefactor[ch][sb][2] = read(stream, &mut ptr, 6, end)? as u8;
                    scalefactor[ch][sb][1] = scalefactor[ch][sb][0];
                }
                _ => {
                    scalefactor[ch][sb][2] = read(stream, &mut ptr, 6, end)? as u8;
                    scalefactor[ch][sb][1] = scalefactor[ch][sb][2];
                }
            }
        }
    }
    for group in 0..12 {
        for sb in 0..bound {
            for ch in 0..channels {
                decode_group(
                    stream,
                    &mut ptr,
                    frame,
                    end,
                    group,
                    sb,
                    ch,
                    allocation[ch][sb],
                    quant_offsets[sb],
                    scalefactor[ch][sb][group / 4],
                )?;
            }
        }
        for sb in bound..sblimit {
            let alloc = allocation[0][sb];
            if alloc == 0 {
                for ch in 0..channels {
                    for slot in group * 3..group * 3 + 3 {
                        frame.sbsample[ch][slot][sb] = 0;
                    }
                }
            } else {
                let q = quant_class(quant_offsets[sb], alloc)?;
                let decoded = layer2_samples(stream, &mut ptr, q, end)?;
                for ch in 0..channels {
                    let scale = SCALEFACTORS[scalefactor[ch][sb][group / 4] as usize];
                    for i in 0..3 {
                        frame.sbsample[ch][group * 3 + i][sb] = fixed::mul(decoded[i], scale);
                    }
                }
            }
        }
        for ch in 0..channels {
            for s in group * 3..group * 3 + 3 {
                for sb in sblimit..32 {
                    frame.sbsample[ch][s][sb] = 0;
                }
            }
        }
    }
    stream.ptr = ptr;
    Ok(())
}

fn quant_class(offset: u8, allocation: u8) -> Result<QuantClass, Error> {
    let table = OFFSETS
        .get(offset as usize)
        .ok_or(Error::BadBitAllocation)?;
    let class = *table
        .get(allocation as usize - 1)
        .ok_or(Error::BadBitAllocation)?;
    QUANT_CLASSES
        .get(class as usize)
        .copied()
        .ok_or(Error::BadBitAllocation)
}

#[allow(clippy::too_many_arguments, clippy::needless_range_loop)]
fn decode_group(
    stream: &mut Stream,
    ptr: &mut BitPtr,
    frame: &mut Frame,
    end: usize,
    group: usize,
    sb: usize,
    ch: usize,
    allocation: u8,
    quant_offset: u8,
    scale_index: u8,
) -> Result<(), Error> {
    if allocation == 0 {
        for i in 0..3 {
            frame.sbsample[ch][group * 3 + i][sb] = 0;
        }
    } else {
        let q = quant_class(quant_offset, allocation)?;
        let decoded = layer2_samples(stream, ptr, q, end)?;
        let scale = SCALEFACTORS[scale_index as usize];
        for i in 0..3 {
            frame.sbsample[ch][group * 3 + i][sb] = fixed::mul(decoded[i], scale);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame_data() -> (Stream, Frame) {
        let mut stream = Stream::new();
        stream.buffer(&alloc::vec![0; 208 + 8]);
        stream.this_frame = 0;
        stream.next_frame = 208;
        stream.ptr = BitPtr::new(4);
        let mut frame = Frame::default();
        frame.header.layer = crate::frame::Layer::II;
        frame.header.mode = Mode::SingleChannel;
        frame.header.bitrate = 64000;
        frame.header.samplerate = 44100;
        (stream, frame)
    }

    #[test]
    fn decodes_layer2_silence_and_clears_unused_subbands() {
        let (mut stream, mut frame) = frame_data();
        decode_layer2(&mut stream, &mut frame).unwrap();
        assert!(frame.sbsample[0][..36].iter().flatten().all(|&v| v == 0));
    }

    #[test]
    fn grouped_samples_are_degrouped_and_scaled() {
        let (mut stream, mut frame) = frame_data();
        let mut ptr = stream.ptr;
        for sb in 0..27 {
            let nbal = BITALLOC[SBQUANT[0].1[sb] as usize].0 as u32;
            ptr.write(&mut stream.buffer, nbal, u32::from(sb == 0))
                .unwrap();
        }
        ptr.write(&mut stream.buffer, 2, 2).unwrap();
        ptr.write(&mut stream.buffer, 6, 3).unwrap();
        for group in 0..12 {
            ptr.write(&mut stream.buffer, 5, if group == 0 { 1 } else { 0 })
                .unwrap();
        }
        decode_layer2(&mut stream, &mut frame).unwrap();
        let a = frame.sbsample[0][0][0];
        let b = frame.sbsample[0][1][0];
        let c = frame.sbsample[0][2][0];
        // C table class 0 yields raw [1, 0, 0], followed by its C/D
        // requantization and scale factor 3.
        assert_eq!(a, -156_587_350);
        assert_eq!(b, c);
        assert_eq!(b, -178_956_971);
        assert_eq!(frame.sbsample[0][3][0], b);
        assert!(
            frame.sbsample[0][..36]
                .iter()
                .all(|slot| slot[1..].iter().all(|&v| v == 0))
        );
    }

    #[test]
    fn rejects_invalid_bitrate_and_channel_mode_combination() {
        let (mut stream, mut frame) = frame_data();
        frame.header.bitrate = 256000;
        assert_eq!(decode_layer2(&mut stream, &mut frame), Err(Error::BadMode));
        assert_eq!(stream.error, Error::BadMode);
    }
}
