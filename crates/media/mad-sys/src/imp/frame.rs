//! `mad_header_*` and `mad_frame_*` ABI wrappers over the pure Rust decoder.

use alloc::alloc::{Layout, alloc_zeroed, dealloc};

use libmad_rs::{
    bit::BitPtr,
    frame::{self, Emphasis, FLAG_INCOMPLETE, Frame, Header, Layer, Mode},
    layer::{layer2, layer12},
    stream::Error,
    timer::Timer,
};

use super::{abi::*, stream};

fn header_from_abi(header: MadHeader) -> Header {
    Header {
        layer: match header.layer {
            MAD_LAYER_I => Layer::I,
            MAD_LAYER_II => Layer::II,
            MAD_LAYER_III => Layer::III,
            _ => Layer::Unknown,
        },
        mode: match header.mode {
            MAD_MODE_SINGLE_CHANNEL => Mode::SingleChannel,
            MAD_MODE_DUAL_CHANNEL => Mode::DualChannel,
            MAD_MODE_JOINT_STEREO => Mode::JointStereo,
            MAD_MODE_STEREO => Mode::Stereo,
            _ => Mode::SingleChannel,
        },
        mode_extension: header.mode_extension.clamp(0, 3) as u8,
        emphasis: match header.emphasis {
            MAD_EMPHASIS_50_15_US => Emphasis::FiftyFifteenUs,
            MAD_EMPHASIS_RESERVED => Emphasis::Reserved,
            MAD_EMPHASIS_CCITT_J_17 => Emphasis::CcittJ17,
            _ => Emphasis::None,
        },
        bitrate: header.bitrate,
        samplerate: header.samplerate,
        crc_check: header.crc_check,
        crc_target: header.crc_target,
        flags: header.flags as u32,
        private_bits: header.private_bits as u32,
        duration: Timer {
            seconds: i64::from(header.duration.seconds),
            fraction: u64::from(header.duration.fraction),
        },
    }
}

fn header_to_abi(header: Header) -> MadHeader {
    MadHeader {
        layer: header.layer as i32,
        mode: header.mode as i32,
        mode_extension: i32::from(header.mode_extension),
        emphasis: header.emphasis as i32,
        bitrate: header.bitrate,
        samplerate: header.samplerate,
        crc_check: header.crc_check,
        crc_target: header.crc_target,
        flags: header.flags as i32,
        private_bits: header.private_bits as i32,
        duration: MadTimerT {
            seconds: header
                .duration
                .seconds
                .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
            fraction: header.duration.fraction as u32,
        },
    }
}

fn frame_from_abi(frame: &MadFrame) -> Frame {
    Frame {
        header: header_from_abi(frame.header),
        options: frame.options as u32,
        sbsample: frame.sbsample,
    }
}

fn frame_to_abi(source: &Frame, target: &mut MadFrame) {
    target.header = header_to_abi(source.header);
    target.options = source.options as i32;
    target.sbsample = source.sbsample;
}

fn record_layer_error(codec: &mut libmad_rs::Stream, error: Error) {
    codec.error = error;
    if !error.is_recoverable() {
        codec.next_frame = codec.this_frame;
    }
}

type MadOverlapData = [[[MadFixedT; 18]; 32]; 2];

unsafe fn ensure_overlap(frame: *mut MadFrame, stream_ptr: *mut MadStream) -> *mut MadOverlapData {
    let current = unsafe { (*frame).overlap.cast::<MadOverlapData>() };
    if !current.is_null() {
        return current;
    }

    let layout = Layout::new::<MadOverlapData>();
    let overlap = unsafe { alloc_zeroed(layout).cast::<MadOverlapData>() };
    if overlap.is_null() {
        unsafe { (*stream_ptr).error = MAD_ERROR_NOMEM };
        return core::ptr::null_mut();
    }
    unsafe { (*frame).overlap = overlap.cast() };
    overlap
}

/// Initialize a header structure.
///
/// # Safety
/// `header` may be null or point to writable `MadHeader` storage.
pub unsafe fn header_init(header: *mut MadHeader) {
    if !header.is_null() {
        unsafe { *header = MadHeader::default() };
    }
}

/// Decode one MPEG header from the stream cursor.
///
/// # Safety
/// Non-null pointers must reference valid writable structures. The stream's
/// declared input memory must remain readable for the duration of this call.
pub unsafe fn header_decode(header: *mut MadHeader, stream_ptr: *mut MadStream) -> i32 {
    if header.is_null() || stream_ptr.is_null() {
        return -1;
    }
    let mut codec = stream::load_codec(unsafe { &*stream_ptr });
    let result = frame::decode_header(&mut codec);
    if let Ok(decoded) = result {
        unsafe { *header = header_to_abi(decoded) };
    }
    unsafe { stream::publish_stream_fields(&codec, &mut *stream_ptr) };
    if result.is_ok() { 0 } else { -1 }
}

/// Initialize a frame structure.
///
/// # Safety
/// `frame` may be null or point to writable `MadFrame` storage.
pub unsafe fn frame_init(frame: *mut MadFrame) {
    if !frame.is_null() {
        unsafe { *frame = MadFrame::default() };
    }
}

/// Release the Layer III overlap allocation owned by this frame.
///
/// # Safety
/// `frame` may be null or point to a valid `MadFrame` initialized by this
/// wrapper.
pub unsafe fn frame_finish(frame: *mut MadFrame) {
    if frame.is_null() {
        return;
    }
    let overlap = unsafe { (*frame).overlap.cast::<MadOverlapData>() };
    if !overlap.is_null() {
        unsafe {
            dealloc(overlap.cast::<u8>(), Layout::new::<MadOverlapData>());
            (*frame).overlap = core::ptr::null_mut();
        }
    }
}

/// Decode one frame using the local pure Rust Layer I/II/III implementations.
///
/// Layer I/II use only public stream state. Layer III allocates the same two
/// public dynamic objects as C (`stream.main_data` and `frame.overlap`), then
/// imports callback-visible mutations into the pure Rust decoder and exports
/// the updated reservoir/overlap after each decode attempt.
///
/// # Safety
/// `frame` and `stream` must point to valid writable ABI structures. Any
/// non-null input range in the stream must remain readable during this call.
pub unsafe fn frame_decode(frame_ptr: *mut MadFrame, stream_ptr: *mut MadStream) -> i32 {
    if frame_ptr.is_null() || stream_ptr.is_null() {
        return -1;
    }

    let frame_abi = unsafe { &mut *frame_ptr };
    frame_abi.options = unsafe { (*stream_ptr).options };
    let mut codec = stream::load_codec(unsafe { &*stream_ptr });

    let header = if frame_abi.header.flags & MAD_FLAG_INCOMPLETE == 0 {
        match frame::decode_header(&mut codec) {
            Ok(header) => header,
            Err(error) => {
                codec.error = error;
                codec.anc_bitlen = 0;
                unsafe { stream::publish_stream_fields(&codec, &mut *stream_ptr) };
                return -1;
            }
        }
    } else {
        header_from_abi(frame_abi.header)
    };

    let mut decoded = frame_from_abi(frame_abi);
    decoded.header = header;
    decoded.header.flags &= !FLAG_INCOMPLETE;
    decoded.options = frame_abi.options as u32;
    codec.options = decoded.options;
    codec.error = Error::None;

    if decoded.header.layer == Layer::III {
        let owner_ptr = unsafe { stream::stream_owner(stream_ptr) };
        if owner_ptr.is_null() {
            decoded.header.flags &= !FLAG_INCOMPLETE;
            frame_to_abi(&decoded, frame_abi);
            unsafe { (*stream_ptr).anc_bitlen = 0 };
            return -1;
        }
        let overlap = unsafe { ensure_overlap(frame_ptr, stream_ptr) };
        if overlap.is_null() {
            frame_to_abi(&decoded, frame_abi);
            unsafe { (*stream_ptr).anc_bitlen = 0 };
            return -1;
        }

        let owner = unsafe { &mut *owner_ptr };
        owner.codec = codec;

        let reservoir_len = (unsafe { (*stream_ptr).md_len } as usize).min(MAD_BUFFER_MDLEN);
        owner
            .layer3
            .set_reservoir_bytes(&owner.main_data[..reservoir_len]);
        owner.layer3.set_overlap(unsafe { *overlap });

        let result = owner.layer3.decode(&mut owner.codec, &mut decoded);
        if let Err(error) = result {
            record_layer_error(&mut owner.codec, error);
            owner.codec.anc_bitlen = 0;
        }

        let retained = owner.layer3.reservoir_bytes();
        let retained_len = retained.len().min(MAD_BUFFER_MDLEN);
        owner.main_data[..retained_len].copy_from_slice(&retained[..retained_len]);
        unsafe { *overlap = *owner.layer3.overlap() };

        frame_to_abi(&decoded, frame_abi);
        unsafe { stream::publish_codec(owner, &mut *stream_ptr) };
        return if result.is_ok() { 0 } else { -1 };
    }

    let result = match decoded.header.layer {
        Layer::I => layer12::decode_layer1(&mut codec, &mut decoded),
        Layer::II => layer2::decode_layer2(&mut codec, &mut decoded),
        Layer::Unknown => Err(Error::BadLayer),
        Layer::III => unreachable!(),
    };
    if let Err(error) = result {
        record_layer_error(&mut codec, error);
        codec.anc_bitlen = 0;
    } else {
        codec.anc_ptr = codec.ptr;
        codec.anc_bitlen =
            BitPtr::bit_length(codec.ptr, BitPtr::new(codec.next_frame)).max(0) as u32;
    }
    frame_to_abi(&decoded, frame_abi);
    unsafe { stream::publish_stream_fields(&codec, &mut *stream_ptr) };
    if result.is_ok() { 0 } else { -1 }
}

/// Clear decoded subband samples from a frame.
///
/// # Safety
/// `frame` may be null or point to writable `MadFrame` storage.
pub unsafe fn frame_mute(frame: *mut MadFrame) {
    if frame.is_null() {
        return;
    }
    unsafe { (*frame).sbsample = [[[0; 32]; 36]; 2] };
    let overlap = unsafe { (*frame).overlap.cast::<MadOverlapData>() };
    if !overlap.is_null() {
        unsafe { *overlap = [[[0; 18]; 32]; 2] };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imp::stream::{stream_buffer, stream_finish, stream_init};

    fn stream_for_layer1(bytes: &mut [u8], end_offset: usize) -> MadStream {
        let mut stream = MadStream::default();
        unsafe {
            stream_init(&mut stream);
            stream_buffer(&mut stream, bytes.as_ptr(), bytes.len() as u32);
        }
        stream.this_frame = bytes.as_ptr();
        stream.next_frame = bytes.as_ptr().wrapping_add(end_offset);
        stream.ptr.byte = bytes.as_ptr().wrapping_add(4);
        stream
    }

    fn layer1_frame() -> MadFrame {
        let mut frame = MadFrame::default();
        frame.header.layer = MAD_LAYER_I;
        frame.header.mode = MAD_MODE_SINGLE_CHANNEL;
        frame.header.samplerate = 44_100;
        frame.header.bitrate = 128_000;
        frame.header.flags = MAD_FLAG_INCOMPLETE;
        frame
    }

    #[test]
    fn header_decode_reads_fixed_mpeg_header_and_reports_truncation() {
        let mut bytes = [0u8; 425];
        bytes[..4].copy_from_slice(&[0xff, 0xfb, 0x90, 0x64]);
        let mut stream = MadStream::default();
        let mut header = MadHeader {
            bitrate: 77,
            ..MadHeader::default()
        };
        unsafe { header_init(&mut header) };
        assert_eq!(header.bitrate, 0);
        unsafe {
            stream_init(&mut stream);
            stream_buffer(&mut stream, bytes.as_ptr(), 4);
        }
        assert_eq!(unsafe { header_decode(&mut header, &mut stream) }, -1);
        assert_eq!(stream.error, MAD_ERROR_BUFLEN);
        unsafe { stream_buffer(&mut stream, bytes.as_ptr(), bytes.len() as u32) };
        assert_eq!(unsafe { header_decode(&mut header, &mut stream) }, 0);
        assert_eq!(
            (header.layer, header.mode, header.bitrate, header.samplerate),
            (MAD_LAYER_III, MAD_MODE_JOINT_STEREO, 128000, 44100)
        );
        assert_eq!(stream.next_frame, unsafe { bytes.as_ptr().add(417) });
        unsafe { stream_finish(&mut stream) };
    }

    #[test]
    fn frame_init_mute_and_finish_clear_owned_state() {
        let mut frame = MadFrame {
            options: 7,
            ..MadFrame::default()
        };
        frame.sbsample[0][0][0] = 42;
        unsafe { frame_init(&mut frame) };
        assert_eq!(frame.options, 0);
        assert_eq!(frame.sbsample[0][0][0], 0);
        let mut stream = MadStream::default();
        let overlap = unsafe { ensure_overlap(&mut frame, &mut stream) };
        assert!(!overlap.is_null());
        unsafe {
            (*overlap)[0][0][0] = 123;
        }
        frame.sbsample[0][0][0] = 456;
        unsafe { frame_mute(&mut frame) };
        assert_eq!(frame.sbsample[0][0][0], 0);
        assert_eq!(unsafe { (*overlap)[0][0][0] }, 0);
        unsafe { frame_finish(&mut frame) };
        assert!(frame.overlap.is_null());
    }

    #[test]
    fn layer1_decode_sets_ancillary_range_after_audio_bits() {
        let mut bytes = [0_u8; 30];
        let mut stream = stream_for_layer1(&mut bytes, 22);
        let mut frame = layer1_frame();
        assert_eq!(unsafe { frame_decode(&mut frame, &mut stream) }, 0);
        assert_eq!(stream.anc_bitlen, 16);
        assert_eq!(stream.anc_ptr.byte, bytes.as_ptr().wrapping_add(20));
        assert!(stream.main_data.is_null());
        unsafe { stream_finish(&mut stream) };
    }

    #[test]
    fn layer_error_clears_ancillary_bits_and_recoverable_error_keeps_next_frame() {
        let mut bytes = [0_u8; 30];
        bytes[4] = 0xf0; // Reserved Layer I allocation value.
        let mut stream = stream_for_layer1(&mut bytes, 22);
        stream.anc_bitlen = 77;
        let next = stream.next_frame;
        let mut frame = layer1_frame();
        assert_eq!(unsafe { frame_decode(&mut frame, &mut stream) }, -1);
        assert_eq!(stream.error, MAD_ERROR_BADBITALLOC);
        assert_eq!(stream.anc_bitlen, 0);
        assert_eq!(stream.next_frame, next);
        unsafe { stream_finish(&mut stream) };
    }

    #[test]
    fn nonrecoverable_layer_error_rewinds_to_frame_start() {
        let mut codec = libmad_rs::Stream::new();
        codec.this_frame = 17;
        codec.next_frame = 90;
        record_layer_error(&mut codec, Error::BufferLength);
        assert_eq!(codec.next_frame, codec.this_frame);
        assert_eq!(codec.error, Error::BufferLength);
    }

    #[test]
    fn layer3_exposes_reservoir_and_overlap_and_mute_clears_overlap() {
        let bytes = [0_u8; 64];
        let mut stream = MadStream::default();
        unsafe {
            stream_init(&mut stream);
            stream_buffer(&mut stream, bytes.as_ptr(), bytes.len() as u32);
        }
        stream.this_frame = bytes.as_ptr();
        stream.next_frame = bytes.as_ptr().wrapping_add(40);
        stream.ptr.byte = bytes.as_ptr().wrapping_add(4);

        let mut frame = MadFrame::default();
        frame.header.layer = MAD_LAYER_III;
        frame.header.mode = MAD_MODE_SINGLE_CHANNEL;
        frame.header.samplerate = 44_100;
        frame.header.bitrate = 128_000;
        frame.header.flags = MAD_FLAG_INCOMPLETE;

        let _ = unsafe { frame_decode(&mut frame, &mut stream) };
        assert!(!stream.main_data.is_null());
        assert!(!frame.overlap.is_null());

        let overlap = frame.overlap.cast::<MadOverlapData>();
        unsafe {
            (*overlap)[0][0][0] = 123;
            frame.sbsample[0][0][0] = 456;
            frame_mute(&mut frame);
        }
        assert_eq!(frame.sbsample[0][0][0], 0);
        assert_eq!(unsafe { (*overlap)[0][0][0] }, 0);

        unsafe {
            frame_finish(&mut frame);
            stream_finish(&mut stream);
        }
        assert!(frame.overlap.is_null());
        assert!(stream.main_data.is_null());
    }
}
