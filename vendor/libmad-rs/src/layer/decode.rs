//! Stateful Layer III frame integration corresponding to `mad_layer_III()`.

use crate::{
    bit::{self, BitPtr},
    fixed::Fixed,
    frame::{FLAG_LSF_EXT, FLAG_PROTECTION, Frame, Layer, Mode, OPTION_IGNORECRC},
    layer::{
        huffdecode::{self, HuffDecodeError},
        layer3::{
            self, ChannelSideInfo, FLAG_MIXED_BLOCK, MainData, MainDataReservoir, ReservoirError,
            SideInfo,
        },
        scalefactor::{self, DecodedScalefactors, Mpeg1ScalefactorParams, ScalefactorError},
        stereo::{self, StereoError},
        transform::{self, TransformError},
    },
    stream::{Error, Stream},
    tables::bands::{self, BandLayout, BandWidths},
};

/// Persistent state for Layer III reservoir and overlap processing.
#[derive(Clone, Debug, Default)]
pub struct Layer3Decoder {
    reservoir: MainDataReservoir,
    overlap: [[[Fixed; 18]; 32]; 2],
}

impl Layer3Decoder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Clear the saved IMDCT overlap samples, as `mad_frame_mute()` does.
    pub fn mute_overlap(&mut self) {
        self.overlap = [[[0; 18]; 32]; 2];
    }

    /// Import the C-visible Layer III main-data reservoir before a decode.
    pub fn set_reservoir_bytes(&mut self, bytes: &[u8]) {
        self.reservoir.replace_bytes(bytes);
    }

    /// Return the retained bytes that C exposes through `mad_stream::main_data`.
    pub fn reservoir_bytes(&self) -> &[u8] {
        self.reservoir.bytes()
    }

    /// Import C-visible overlap history before a Layer III decode.
    pub fn set_overlap(&mut self, overlap: [[[Fixed; 18]; 32]; 2]) {
        self.overlap = overlap;
    }

    /// Return current overlap history for the C-visible `mad_frame::overlap`.
    pub fn overlap(&self) -> &[[[Fixed; 18]; 32]; 2] {
        &self.overlap
    }

    /// Decode one Layer III frame. The caller must set `frame.header`,
    /// `stream.ptr` to the first side-information bit, and `stream.next_frame`
    /// to the frame boundary before calling this method.
    pub fn decode(&mut self, stream: &mut Stream, frame: &mut Frame) -> Result<(), Error> {
        let result = self.decode_inner(stream, frame);
        stream.error = result.err().unwrap_or(Error::None);
        result
    }

    fn decode_inner(&mut self, stream: &mut Stream, frame: &mut Frame) -> Result<(), Error> {
        if !stream.buffer_set {
            return Err(Error::BufferPointer);
        }
        if frame.header.layer != Layer::III {
            return Err(Error::BadLayer);
        }
        let channels = frame.header.channels();
        let lsf = frame.header.flags & FLAG_LSF_EXT != 0;
        let private_bitlen = layer3::private_bits_width(lsf, channels);
        let side_info_len = match (lsf, channels) {
            (true, 1) => 9,
            (true, _) => 17,
            (false, 1) => 17,
            (false, _) => 32,
        };
        let side_start = stream.ptr;
        let side_start_byte = side_start.next_byte();
        if stream.next_frame < side_start_byte
            || stream.next_frame - side_start_byte < side_info_len
            || stream.next_frame > stream.buffer.len()
        {
            self.reservoir = MainDataReservoir::default();
            stream.md_len = 0;
            return Err(Error::BadFrameLength);
        }

        let mut first_error = None;
        if frame.header.flags & FLAG_PROTECTION != 0 {
            match bit::crc(
                side_start,
                &stream.buffer,
                (side_info_len * 8) as u32,
                frame.header.crc_check,
            ) {
                Ok(check) => {
                    frame.header.crc_check = check;
                    if check != frame.header.crc_target && frame.options & OPTION_IGNORECRC == 0 {
                        first_error = Some(Error::BadCrc);
                    }
                }
                Err(_) => first_error = Some(Error::BadFrameLength),
            }
        }

        let mut side_ptr = side_start;
        let side_result = layer3::parse_side_info(&mut side_ptr, &stream.buffer, channels, lsf);
        stream.ptr = side_ptr;
        frame.header.flags |= private_bitlen as u32;
        frame.header.private_bits |= read_private_bits(side_start, &stream.buffer, lsf, channels);
        if let Err(error) = side_result.as_ref() {
            first_error.get_or_insert(map_side_info_error(*error));
        }

        let main_data_begin = read_main_data_begin(side_start, &stream.buffer, lsf)?;
        let side_end_byte = side_ptr.next_byte();
        if side_end_byte > stream.next_frame {
            self.reservoir = MainDataReservoir::default();
            stream.md_len = 0;
            return Err(first_error.unwrap_or(Error::BadFrameLength));
        }
        let frame_space = stream.next_frame - side_end_byte;
        let mut next_main_data_begin = peek_next_main_data_begin(stream);
        if next_main_data_begin > main_data_begin.saturating_add(frame_space) {
            next_main_data_begin = 0;
        }
        let payload = &stream.buffer[side_end_byte..stream.next_frame];
        let main_data_result =
            self.reservoir
                .assemble(payload, main_data_begin, next_main_data_begin);
        let mut main_data = match main_data_result {
            Ok(main_data) => {
                stream.md_len = self.reservoir.bytes().len() as u32;
                Some(main_data)
            }
            Err(error) => {
                match error {
                    ReservoirError::Underflow => {
                        stream.md_len = self.reservoir.bytes().len() as u32;
                        first_error.get_or_insert(Error::BadDataPointer);
                        None
                    }
                    ReservoirError::CapacityExceeded => {
                        // C asserts this capacity bound. Keep the prior
                        // reservoir length and fail immediately, overriding
                        // earlier recoverable errors.
                        stream.sync = false;
                        return Err(Error::LostSync);
                    }
                }
            }
        };

        if first_error.is_none() {
            let side = side_result.map_err(map_side_info_error)?;
            let data = main_data.as_mut().ok_or(Error::BadDataPointer)?;
            let decoded = self.decode_main_data(data, &side, frame, channels, lsf);
            stream.anc_ptr = data.ptr;
            stream.anc_bitlen = data
                .bytes
                .len()
                .saturating_mul(8)
                .saturating_sub(side.data_bitlen as usize) as u32;
            decoded?;
        }
        first_error.map_or(Ok(()), Err)
    }

    fn decode_main_data(
        &mut self,
        main_data: &mut MainData,
        side: &SideInfo,
        frame: &mut Frame,
        channels: usize,
        lsf: bool,
    ) -> Result<(), Error> {
        let rate_index = samplerate_index(frame);
        frame.sbsample = [[[0; 32]; 36]; 2];
        let mut granule_zero = [[0_u8; 39]; 2];

        for granule in 0..side.granule_count {
            let mut spectral = [[0_i32; 576]; 2];
            let mut decoded_sf: [Option<DecodedScalefactors>; 2] = [None, None];
            let mut channel_widths = [[0_u8; 39]; 2];

            for channel_index in 0..channels {
                let channel = &side.granules[granule].channels[channel_index];
                let layout = band_layout(channel);
                let widths = bands::get(rate_index, layout).ok_or(Error::BadSampleRate)?;
                channel_widths[channel_index] = widths_array(widths);
                let total_part_bits = usize::from(channel.part2_3_length);
                let mut decode_channel = channel.clone();

                let factors = if lsf {
                    let factors = scalefactor::decode_lsf(
                        &mut main_data.ptr,
                        &main_data.bytes,
                        channel.scalefac_compress,
                        channel.block_type,
                        channel.flags & FLAG_MIXED_BLOCK != 0,
                        channel_index == 1 && frame.header.mode_extension & 1 != 0,
                        total_part_bits,
                    )
                    .map_err(map_scalefactor_error)?;
                    if factors.preflag {
                        decode_channel.flags |= layer3::FLAG_PREFLAG;
                    }
                    factors
                } else {
                    let params = Mpeg1ScalefactorParams {
                        scalefac_compress: channel.scalefac_compress as u8,
                        block_type: channel.block_type,
                        mixed_block: channel.flags & FLAG_MIXED_BLOCK != 0,
                        scfsi: if granule == 0 {
                            0
                        } else {
                            side.scfsi[channel_index]
                        },
                        granule_zero: if granule == 0 {
                            None
                        } else {
                            Some(&granule_zero[channel_index])
                        },
                    };
                    scalefactor::decode_mpeg1(
                        &mut main_data.ptr,
                        &main_data.bytes,
                        params,
                        total_part_bits,
                    )
                    .map_err(map_scalefactor_error)?
                };
                if granule == 0 {
                    granule_zero[channel_index] = factors.values;
                }
                let part3_length = total_part_bits
                    .checked_sub(factors.part2_length)
                    .ok_or(Error::BadPart3Length)?;
                spectral[channel_index] = huffdecode::decode_channel(
                    &mut main_data.ptr,
                    &main_data.bytes,
                    &decode_channel,
                    &factors.values,
                    widths.as_slice(),
                    part3_length as u32,
                )
                .map_err(map_huffman_decode_error)?;
                decoded_sf[channel_index] = Some(factors);
            }

            if frame.header.mode == Mode::JointStereo && frame.header.mode_extension != 0 {
                let left = &side.granules[granule].channels[0];
                let right = &side.granules[granule].channels[1];
                let right_factors = decoded_sf[1].as_ref().ok_or(Error::BadStereo)?;
                let illegal_positions = right_factors.intensity.as_ref();
                stereo::process_joint_stereo(
                    &mut spectral,
                    frame.header.mode_extension,
                    &mut frame.header.flags,
                    lsf,
                    right.scalefac_compress,
                    left.block_type,
                    right.block_type,
                    left.flags & FLAG_MIXED_BLOCK != 0,
                    right.flags & FLAG_MIXED_BLOCK != 0,
                    &right_factors.values,
                    illegal_positions,
                    &channel_widths[0],
                )
                .map_err(map_stereo_error)?;
            }

            for channel_index in 0..channels {
                let channel = &side.granules[granule].channels[channel_index];
                let layout_widths = &channel_widths[channel_index];
                let mut samples = [[0_i32; 32]; 18];
                transform::transform_channel(
                    &mut spectral[channel_index],
                    channel,
                    layout_widths,
                    &mut self.overlap[channel_index],
                    &mut samples,
                )
                .map_err(map_transform_error)?;
                for (slot, samples) in samples.iter().enumerate() {
                    frame.sbsample[channel_index][granule * 18 + slot] = *samples;
                }
            }
        }
        Ok(())
    }
}

fn read_main_data_begin(start: BitPtr, data: &[u8], lsf: bool) -> Result<usize, Error> {
    let mut ptr = start;
    ptr.read(data, if lsf { 8 } else { 9 })
        .map(|value| value as usize)
        .map_err(|_| Error::BadFrameLength)
}

fn read_private_bits(start: BitPtr, data: &[u8], lsf: bool, channels: usize) -> u32 {
    let mut ptr = start;
    let _ = ptr.read(data, if lsf { 8 } else { 9 });
    let width = u32::from(layer3::private_bits_width(lsf, channels));
    ptr.read(data, width).unwrap_or(0)
}

fn peek_next_main_data_begin(stream: &Stream) -> usize {
    if stream.next_frame.saturating_add(4) > stream.buffer.len() {
        return 0;
    }
    let mut ptr = BitPtr::new(stream.next_frame);
    let Ok(header) = ptr.read(&stream.buffer, 32) else {
        return 0;
    };
    if header & 0xffe6_0000 != 0xffe2_0000 {
        return 0;
    }
    if header & 0x0001_0000 == 0 && ptr.skip(16, &stream.buffer).is_err() {
        return 0;
    }
    ptr.read(
        &stream.buffer,
        if header & 0x0008_0000 != 0 { 9 } else { 8 },
    )
    .unwrap_or(0) as usize
}

fn samplerate_index(frame: &Frame) -> usize {
    let mut rate = frame.header.samplerate;
    let is_25 = frame.header.flags & crate::frame::FLAG_MPEG_2_5_EXT != 0;
    if is_25 {
        rate = rate.saturating_mul(2);
    }
    let index = (((rate >> 7) & 0x0f) + ((rate >> 15) & 1)) as i32 - 8 + if is_25 { 3 } else { 0 };
    usize::try_from(index).unwrap_or(usize::MAX)
}

fn band_layout(channel: &ChannelSideInfo) -> BandLayout {
    if channel.block_type == 2 {
        if channel.flags & FLAG_MIXED_BLOCK != 0 {
            BandLayout::Mixed
        } else {
            BandLayout::Short
        }
    } else {
        BandLayout::Long
    }
}

fn widths_array(widths: &BandWidths) -> [u8; 39] {
    let mut result = [0; 39];
    result[..widths.len()].copy_from_slice(widths.as_slice());
    result
}

fn map_side_info_error(error: layer3::SideInfoError) -> Error {
    match error {
        layer3::SideInfoError::Truncated | layer3::SideInfoError::InvalidChannelCount => {
            Error::BadFrameLength
        }
        layer3::SideInfoError::BadBigValues => Error::BadBigValues,
        layer3::SideInfoError::BadBlockType => Error::BadBlockType,
        layer3::SideInfoError::BadScfsi => Error::BadScfsi,
    }
}

fn map_scalefactor_error(error: ScalefactorError) -> Error {
    match error {
        ScalefactorError::BadScfsi => Error::BadScfsi,
        ScalefactorError::Truncated => Error::BadPart3Length,
        ScalefactorError::InvalidCompress => Error::BadScaleFactor,
    }
}

fn map_huffman_decode_error(error: HuffDecodeError) -> Error {
    match error {
        HuffDecodeError::BadPart3Length => Error::BadPart3Length,
        HuffDecodeError::BadBigValues => Error::BadBigValues,
        HuffDecodeError::BadHuffmanTable => Error::BadHuffmanTable,
        HuffDecodeError::BadHuffmanData | HuffDecodeError::Truncated => Error::BadHuffmanData,
        HuffDecodeError::InvalidBandWidths => Error::BadFrameLength,
    }
}

fn map_stereo_error(error: StereoError) -> Error {
    match error {
        StereoError::BadStereo => Error::BadStereo,
        StereoError::InvalidBandWidths | StereoError::MissingIllegalPositionFlags => {
            Error::BadFrameLength
        }
    }
}

fn map_transform_error(error: TransformError) -> Error {
    match error {
        TransformError::InvalidBlockType => Error::BadBlockType,
        TransformError::Spectral(_) => Error::BadFrameLength,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mpeg1_mono_frame() -> (Stream, Frame) {
        let mut stream = Stream::new();
        let bytes = [0_u8; 36];
        // The cursor starts after a four-byte MPEG header. This synthetic
        // frame has 17 side-information bytes and no main-data payload.
        stream.buffer(&bytes);
        stream.ptr = BitPtr::new(4);
        stream.next_frame = 21;
        let mut frame = Frame::default();
        frame.header.layer = Layer::III;
        frame.header.mode = Mode::SingleChannel;
        frame.header.samplerate = 44100;
        frame.header.bitrate = 64000;
        (stream, frame)
    }

    fn lsf_mono_frame() -> (Stream, Frame) {
        let mut stream = Stream::new();
        let bytes = [0_u8; 30];
        stream.buffer(&bytes);
        stream.ptr = BitPtr::new(4);
        stream.next_frame = 13;
        let mut frame = Frame::default();
        frame.header.layer = Layer::III;
        frame.header.mode = Mode::SingleChannel;
        frame.header.samplerate = 22050;
        frame.header.flags = FLAG_LSF_EXT;
        (stream, frame)
    }

    #[test]
    fn decodes_complete_zeroed_mpeg1_and_lsf_frames() {
        let (mut stream, mut frame) = mpeg1_mono_frame();
        let mut decoder = Layer3Decoder::new();
        decoder.decode(&mut stream, &mut frame).unwrap();
        assert!(frame.sbsample[0][..36].iter().flatten().all(|&v| v == 0));
        assert_eq!(stream.ptr, BitPtr::new(21));

        let (mut stream, mut frame) = lsf_mono_frame();
        decoder.decode(&mut stream, &mut frame).unwrap();
        assert!(frame.sbsample[0][..18].iter().flatten().all(|&v| v == 0));
    }

    #[test]
    fn real_mpeg1_frame_matches_compiled_c_default_samples() {
        use crate::{
            MAD_BUFFER_GUARD,
            frame::{FLAG_INCOMPLETE, Header},
            synth::Synth,
        };

        let path = std::env::var_os("LIBOGC_MAD_REFERENCE_MP3").expect(
            "set LIBOGC_MAD_REFERENCE_MP3 to the external reference fixture; see docs/dependencies.md",
        );
        let mut bytes = std::fs::read(path).expect("read the external MAD reference fixture");
        bytes.resize(bytes.len() + MAD_BUFFER_GUARD, 0);
        let mut stream = Stream::new();
        stream.buffer(&bytes);
        let header = loop {
            match Header::decode(&mut stream) {
                Ok(header) => break header,
                Err(error) if error.is_recoverable() => continue,
                Err(error) => panic!("reference fixture header: {error:?}"),
            }
        };
        let mut frame = Frame {
            header,
            ..Frame::default()
        };
        frame.header.flags &= !FLAG_INCOMPLETE;
        Layer3Decoder::new()
            .decode(&mut stream, &mut frame)
            .unwrap();

        // Golden values from a fresh run of the prebuilt FPM_DEFAULT C
        // decoder. These detect the inactive IMDCT and fused alias-MAC bugs.
        let expected_subbands = [
            [
                [3580, 5549, -1074, 1253],
                [3745, 12840, -14445, 535],
                [-4435, 0, 8870, -5322],
                [20944, -25872, 0, -4928],
            ],
            [
                [-3043, 11098, -3580, -7697],
                [-13375, -2675, 17655, 9630],
                [-27497, 3548, 13305, -7096],
                [8624, 24640, 24640, 9856],
            ],
        ];
        for (channel, expected) in frame.sbsample.iter().zip(expected_subbands) {
            for (slot, values) in channel.iter().zip(expected) {
                assert_eq!(&slot[..4], &values);
            }
        }
        let mut synth = Synth::new();
        let pcm = synth.frame(&frame);
        let expected_pcm = [
            [
                2395, 11886, 8799, 1611, 5454, 14332, 11752, 11020, 9422, 11310, -250, 7294, 13858,
                1643, 10112, 11383,
            ],
            [
                -26672, -30261, -51502, -40937, -41007, -43067, -38175, -42500, -18859, -20292,
                -54061, -28402, -3910, -1497, 20730, 20879,
            ],
        ];
        assert_eq!((pcm.channels, pcm.length), (2, 1152));
        for (channel, expected) in pcm.samples.iter().zip(expected_pcm) {
            assert_eq!(&channel[256..272], &expected);
        }
    }

    #[test]
    fn rejects_short_side_information_and_resets_reservoir() {
        let (mut stream, mut frame) = mpeg1_mono_frame();
        stream.next_frame = 20;
        let mut decoder = Layer3Decoder::new();
        assert_eq!(
            decoder.decode(&mut stream, &mut frame),
            Err(Error::BadFrameLength)
        );
        assert_eq!(stream.md_len, 0);
    }

    #[test]
    fn crc_error_is_reported_after_side_information_parse() {
        let (mut stream, mut frame) = mpeg1_mono_frame();
        stream.ptr = BitPtr::new(6);
        stream.next_frame = 23;
        frame.header.flags |= FLAG_PROTECTION;
        frame.header.crc_target = 1;
        assert_eq!(
            Layer3Decoder::new().decode(&mut stream, &mut frame),
            Err(Error::BadCrc)
        );
    }

    #[test]
    fn nonzero_mpeg1_mono_frame_runs_the_full_decode_path() {
        let (mut stream, mut frame) = mpeg1_mono_frame();
        stream.next_frame = 22;
        let mut side = BitPtr::new(4);
        side.write(&mut stream.buffer, 9, 0).unwrap(); // main_data_begin
        side.write(&mut stream.buffer, 5, 0).unwrap(); // private bits
        side.write(&mut stream.buffer, 4, 0).unwrap(); // scfsi

        // Granule 0: five part2_3 bits, one big-values pair, gain 210,
        // scalefac_compress 0, and table 1 in region 0.
        side.write(&mut stream.buffer, 12, 5).unwrap();
        side.write(&mut stream.buffer, 9, 1).unwrap();
        side.write(&mut stream.buffer, 8, 210).unwrap();
        side.write(&mut stream.buffer, 4, 0).unwrap();
        side.write(&mut stream.buffer, 1, 0).unwrap(); // no window switching
        side.write(&mut stream.buffer, 5, 1).unwrap();
        side.write(&mut stream.buffer, 5, 0).unwrap();
        side.write(&mut stream.buffer, 5, 0).unwrap();
        side.write(&mut stream.buffer, 4, 0).unwrap(); // region0_count
        side.write(&mut stream.buffer, 3, 0).unwrap(); // region1_count
        side.write(&mut stream.buffer, 3, 0).unwrap(); // count1/scale/preflag

        // Granule 1 has no main data.
        side.write(&mut stream.buffer, 12, 0).unwrap();
        side.write(&mut stream.buffer, 9, 0).unwrap();
        side.write(&mut stream.buffer, 8, 210).unwrap();
        side.write(&mut stream.buffer, 4, 0).unwrap();
        side.write(&mut stream.buffer, 1, 0).unwrap();
        side.write(&mut stream.buffer, 5, 0).unwrap();
        side.write(&mut stream.buffer, 5, 0).unwrap();
        side.write(&mut stream.buffer, 5, 0).unwrap();
        side.write(&mut stream.buffer, 4, 0).unwrap();
        side.write(&mut stream.buffer, 3, 0).unwrap();
        side.write(&mut stream.buffer, 3, 0).unwrap();

        // Table 1 code `000`, followed by positive signs for x=y=1.
        let mut main_data = BitPtr::new(21);
        main_data.write(&mut stream.buffer, 5, 0).unwrap();

        let mut decoder = Layer3Decoder::new();
        decoder.decode(&mut stream, &mut frame).unwrap();
        assert!(
            frame.sbsample[0][..18]
                .iter()
                .flatten()
                .any(|&sample| sample != 0)
        );
        assert_eq!(stream.anc_bitlen, 3);
    }

    #[test]
    fn reservoir_capacity_overflow_loses_sync_and_overrides_crc_error() {
        let (mut stream, mut frame) = mpeg1_mono_frame();
        stream.ptr = BitPtr::new(6);
        stream.next_frame = 23;
        stream.md_len = 19;
        frame.header.flags |= FLAG_PROTECTION;
        frame.header.crc_target = 1;
        let mut decoder = Layer3Decoder::new();
        decoder.reservoir = MainDataReservoir::new(1, 2);

        assert_eq!(
            decoder.decode(&mut stream, &mut frame),
            Err(Error::LostSync)
        );
        assert!(!stream.sync);
        assert_eq!(stream.md_len, 19);
    }

    #[test]
    fn mute_overlap_clears_saved_imdct_history() {
        let mut decoder = Layer3Decoder::new();
        decoder.overlap[1][7][12] = 1234;
        decoder.mute_overlap();
        assert!(
            decoder
                .overlap
                .iter()
                .flatten()
                .flatten()
                .all(|&sample| sample == 0)
        );
    }
}
