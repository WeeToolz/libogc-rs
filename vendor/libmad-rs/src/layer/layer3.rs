//! Layer III side information parsing from `III_sideinfo()` in `layer3.c`.
//!
//! This is the first stage of Layer III decoding. It describes where the
//! scalefactor and Huffman data for each granule and channel are stored.

use alloc::vec::Vec;

use crate::bit::{BitError, BitPtr};

pub const FLAG_COUNT1TABLE_SELECT: u8 = 0x01;
pub const FLAG_SCALEFAC_SCALE: u8 = 0x02;
pub const FLAG_PREFLAG: u8 = 0x04;
pub const FLAG_MIXED_BLOCK: u8 = 0x08;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SideInfoError {
    Truncated,
    InvalidChannelCount,
    BadBigValues,
    BadBlockType,
    BadScfsi,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct ChannelSideInfo {
    pub part2_3_length: u16,
    pub big_values: u16,
    pub global_gain: u16,
    pub scalefac_compress: u16,
    pub flags: u8,
    pub block_type: u8,
    pub table_select: [u8; 3],
    pub subblock_gain: [u8; 3],
    pub region0_count: u8,
    pub region1_count: u8,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct GranuleSideInfo {
    pub channels: [ChannelSideInfo; 2],
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SideInfo {
    pub main_data_begin: u16,
    pub private_bits: u8,
    pub scfsi: [u8; 2],
    pub granules: [GranuleSideInfo; 2],
    pub granule_count: usize,
    pub channel_count: usize,
    pub data_bitlen: u32,
    pub private_bitlen: u8,
}

/// Main-data bytes for one frame, ready for scalefactor and Huffman reads.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MainData {
    pub bytes: Vec<u8>,
    pub ptr: BitPtr,
    pub bit_len: usize,
    pub frame_used: usize,
    pub frame_free: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ReservoirError {
    /// The current frame refers to bytes that are no longer in the reservoir.
    Underflow,
    /// The assembled data plus libmad's required guard exceeds capacity.
    CapacityExceeded,
}

/// The Layer III main-data reservoir from `mad_stream::main_data`.
///
/// `assemble` accepts the bytes between the end of side information and the
/// next frame boundary. It returns the current frame's readable main data and
/// retains the bytes needed by the next frame. `next_main_data_begin` is zero
/// when no valid following Layer III header is available.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MainDataReservoir {
    bytes: Vec<u8>,
    capacity: usize,
    guard: usize,
}

impl Default for MainDataReservoir {
    fn default() -> Self {
        Self::new(
            crate::stream::MAD_BUFFER_MDLEN,
            crate::stream::MAD_BUFFER_GUARD,
        )
    }
}

impl MainDataReservoir {
    pub fn new(capacity: usize, guard: usize) -> Self {
        Self {
            bytes: Vec::new(),
            capacity,
            guard,
        }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// Replace the retained bytes with an externally visible libmad reservoir.
    ///
    /// The C ABI exposes `mad_stream::main_data`, so the ABI wrapper imports
    /// callback mutations before decoding the next Layer III frame.
    pub fn replace_bytes(&mut self, bytes: &[u8]) {
        self.bytes.clear();
        self.bytes
            .extend_from_slice(&bytes[..bytes.len().min(self.capacity)]);
    }

    /// Assemble one frame's main data and advance the retained reservoir.
    pub fn assemble(
        &mut self,
        frame_payload: &[u8],
        main_data_begin: usize,
        next_main_data_begin: usize,
    ) -> Result<MainData, ReservoirError> {
        let frame_space = frame_payload.len();
        let mut next_begin = next_main_data_begin;
        if next_begin > main_data_begin.saturating_add(frame_space) {
            next_begin = 0;
        }
        let main_data_len = main_data_begin
            .saturating_add(frame_space)
            .saturating_sub(next_begin);
        if main_data_len.saturating_add(self.guard) > self.capacity {
            return Err(ReservoirError::CapacityExceeded);
        }

        let underflow = main_data_begin > self.bytes.len();
        let frame_used = if underflow {
            0
        } else if main_data_begin == 0 {
            main_data_len
        } else {
            main_data_len.saturating_sub(main_data_begin)
        };
        let frame_free = frame_space.saturating_sub(frame_used);

        let assembled = if underflow {
            None
        } else if main_data_begin == 0 {
            Some(frame_payload[..main_data_len].to_vec())
        } else {
            let mut data = self.bytes[self.bytes.len() - main_data_begin..].to_vec();
            let copy_len = main_data_len.saturating_sub(main_data_begin);
            data.extend_from_slice(&frame_payload[..copy_len]);
            data.truncate(main_data_len);
            Some(data)
        };

        self.retain_next(
            frame_payload,
            frame_used,
            frame_free,
            main_data_len,
            main_data_begin,
            next_begin,
        );

        let Some(bytes) = assembled else {
            return Err(ReservoirError::Underflow);
        };
        Ok(MainData {
            bit_len: bytes.len() * 8,
            bytes,
            ptr: BitPtr::new(0),
            frame_used,
            frame_free,
        })
    }

    fn retain_next(
        &mut self,
        frame_payload: &[u8],
        frame_used: usize,
        frame_free: usize,
        main_data_len: usize,
        main_data_begin: usize,
        next_begin: usize,
    ) {
        if frame_free >= next_begin {
            self.bytes.clear();
            self.bytes
                .extend_from_slice(&frame_payload[frame_payload.len() - next_begin..]);
            return;
        }

        if main_data_len < main_data_begin {
            let mut extra = main_data_begin - main_data_len;
            extra = extra.min(next_begin.saturating_sub(frame_free));
            if extra < self.bytes.len() {
                let start = self.bytes.len() - extra;
                self.bytes = self.bytes[start..].to_vec();
            } else {
                self.bytes.clear();
            }
        } else {
            self.bytes.clear();
        }
        self.bytes
            .extend_from_slice(&frame_payload[frame_used..frame_used + frame_free]);
    }
}

fn read(ptr: &mut BitPtr, data: &[u8], bits: u32) -> Result<u32, SideInfoError> {
    ptr.read(data, bits).map_err(|error| match error {
        BitError::OutOfBounds | BitError::TooManyBits => SideInfoError::Truncated,
    })
}

/// Width of the private field for a validated channel count.
pub(super) const fn private_bits_width(lsf: bool, channels: usize) -> u8 {
    match (lsf, channels) {
        (true, 1) => 1,
        (true, _) => 2,
        (false, 1) => 5,
        (false, _) => 3,
    }
}

/// Parse the Layer III side information at `ptr`, matching `III_sideinfo()`.
///
/// `lsf` is true for MPEG-2 and MPEG-2.5. `channels` must be one or two.
/// The cursor advances as fields are read, including when a semantic error
/// is found, as in the C decoder.
pub fn parse_side_info(
    ptr: &mut BitPtr,
    data: &[u8],
    channels: usize,
    lsf: bool,
) -> Result<SideInfo, SideInfoError> {
    if !(1..=2).contains(&channels) {
        return Err(SideInfoError::InvalidChannelCount);
    }

    let granule_count = if lsf { 1 } else { 2 };
    let private_bitlen = private_bits_width(lsf, channels);
    let mut side = SideInfo {
        main_data_begin: read(ptr, data, if lsf { 8 } else { 9 })? as u16,
        private_bits: read(ptr, data, private_bitlen as u32)? as u8,
        granule_count,
        channel_count: channels,
        private_bitlen,
        ..SideInfo::default()
    };
    if !lsf {
        for ch in 0..channels {
            side.scfsi[ch] = read(ptr, data, 4)? as u8;
        }
    }

    let mut semantic_error = None;
    for granule in side.granules.iter_mut().take(granule_count) {
        for (ch, channel) in granule.channels.iter_mut().take(channels).enumerate() {
            channel.part2_3_length = read(ptr, data, 12)? as u16;
            channel.big_values = read(ptr, data, 9)? as u16;
            channel.global_gain = read(ptr, data, 8)? as u16;
            channel.scalefac_compress = read(ptr, data, if lsf { 9 } else { 4 })? as u16;
            side.data_bitlen += u32::from(channel.part2_3_length);

            if channel.big_values > 288 && semantic_error.is_none() {
                semantic_error = Some(SideInfoError::BadBigValues);
            }

            if read(ptr, data, 1)? != 0 {
                channel.block_type = read(ptr, data, 2)? as u8;
                if channel.block_type == 0 && semantic_error.is_none() {
                    semantic_error = Some(SideInfoError::BadBlockType);
                }
                if !lsf
                    && channel.block_type == 2
                    && side.scfsi[ch] != 0
                    && semantic_error.is_none()
                {
                    semantic_error = Some(SideInfoError::BadScfsi);
                }
                channel.region0_count = 7;
                channel.region1_count = 36;
                if read(ptr, data, 1)? != 0 {
                    channel.flags |= FLAG_MIXED_BLOCK;
                } else if channel.block_type == 2 {
                    channel.region0_count = 8;
                }
                for table in channel.table_select.iter_mut().take(2) {
                    *table = read(ptr, data, 5)? as u8;
                }
                for gain in &mut channel.subblock_gain {
                    *gain = read(ptr, data, 3)? as u8;
                }
            } else {
                channel.block_type = 0;
                for table in &mut channel.table_select {
                    *table = read(ptr, data, 5)? as u8;
                }
                channel.region0_count = read(ptr, data, 4)? as u8;
                channel.region1_count = read(ptr, data, 3)? as u8;
            }
            let flags = read(ptr, data, if lsf { 2 } else { 3 })? as u8;
            channel.flags |= flags;
        }
    }

    if let Some(error) = semantic_error {
        Err(error)
    } else {
        Ok(side)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn private_widths_cover_mpeg_versions_and_channel_counts() {
        assert_eq!(super::private_bits_width(true, 1), 1);
        assert_eq!(super::private_bits_width(true, 2), 2);
        assert_eq!(super::private_bits_width(false, 1), 5);
        assert_eq!(super::private_bits_width(false, 2), 3);
    }

    use super::*;

    #[test]
    fn parses_zeroed_mpeg1_stereo_side_info() {
        let mut ptr = BitPtr::new(0);
        let side = parse_side_info(&mut ptr, &[0; 32], 2, false).unwrap();
        assert_eq!(side.main_data_begin, 0);
        assert_eq!(side.private_bitlen, 3);
        assert_eq!(side.granule_count, 2);
        assert_eq!(side.channel_count, 2);
        assert_eq!(side.data_bitlen, 0);
        assert_eq!(ptr.byte, 32);
        assert_eq!(ptr.left, 8);
    }

    #[test]
    fn parses_zeroed_lsf_mono_side_info() {
        let mut ptr = BitPtr::new(0);
        let side = parse_side_info(&mut ptr, &[0; 17], 1, true).unwrap();
        assert_eq!(side.granule_count, 1);
        assert_eq!(side.private_bitlen, 1);
        assert_eq!(ptr.byte, 9);
        assert_eq!(ptr.left, 8);
    }

    #[test]
    fn rejects_truncated_side_info_without_advancing_failed_read() {
        let mut ptr = BitPtr::new(0);
        assert_eq!(
            parse_side_info(&mut ptr, &[0], 2, false),
            Err(SideInfoError::Truncated)
        );
        assert_eq!(ptr.byte, 0);
        assert_eq!(ptr.left, 8);
    }

    #[test]
    fn reservoir_carries_main_data_across_frame_boundary() {
        let mut reservoir = MainDataReservoir::new(64, 8);
        let first = reservoir.assemble(&[1, 2, 3, 4, 5], 0, 3).unwrap();
        assert_eq!(first.bytes, [1, 2]);
        assert_eq!(first.bit_len, 16);
        assert_eq!(first.frame_used, 2);
        assert_eq!(first.frame_free, 3);
        assert_eq!(reservoir.bytes(), [3, 4, 5]);

        let second = reservoir.assemble(&[6, 7], 3, 0).unwrap();
        assert_eq!(second.bytes, [3, 4, 5, 6, 7]);
        assert_eq!(second.ptr, BitPtr::new(0));
        assert_eq!(second.frame_used, 2);
        assert_eq!(second.frame_free, 0);
        assert!(reservoir.bytes().is_empty());
    }

    #[test]
    fn reservoir_underflow_reports_error_and_keeps_following_frame_bytes() {
        let mut reservoir = MainDataReservoir::new(64, 8);
        assert_eq!(
            reservoir.assemble(&[10, 11, 12, 13], 2, 1),
            Err(ReservoirError::Underflow)
        );
        assert_eq!(reservoir.bytes(), [13]);
    }

    #[test]
    fn reservoir_rejects_data_that_exceeds_guarded_capacity() {
        let mut reservoir = MainDataReservoir::new(8, 2);
        assert_eq!(
            reservoir.assemble(&[1; 7], 0, 0),
            Err(ReservoirError::CapacityExceeded)
        );
        assert!(reservoir.bytes().is_empty());
    }
}
