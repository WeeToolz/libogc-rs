//! `mad_stream_*` ABI wrappers backed by the local Rust codec state.

use alloc::alloc::{Layout, alloc, dealloc};
use core::ffi::c_void;

use libmad_rs::{
    bit::BitPtr,
    layer::decode::Layer3Decoder,
    stream::{Error, Stream},
};

use super::{abi::*, error::mad_error_string};

/// Private state whose first field is the C-visible Layer III reservoir.
///
/// C allocates `MAD_BUFFER_MDLEN` bytes and stores that allocation in
/// `mad_stream::main_data`. Keeping the reservoir as the first `repr(C)` field
/// preserves that pointer contract while allowing the Rust port to keep its
/// codec and transform state in the same allocation. Callers may inspect or
/// mutate the reservoir bytes, but must preserve the pointer and its ownership
/// until `stream_finish`, as they must for the C-owned reservoir allocation.
#[repr(C)]
#[derive(Debug)]
pub(crate) struct StreamOwner {
    pub(crate) main_data: [u8; MAD_BUFFER_MDLEN],
    pub(crate) codec: Stream,
    pub(crate) layer3: Layer3Decoder,
}

impl StreamOwner {
    fn from_abi(stream: &MadStream) -> Self {
        Self {
            main_data: [0; MAD_BUFFER_MDLEN],
            codec: load_codec(stream),
            layer3: Layer3Decoder::new(),
        }
    }
}

#[inline]
unsafe fn owner_from_stream(stream: *const MadStream) -> *mut StreamOwner {
    if stream.is_null() || unsafe { (*stream).main_data.is_null() } {
        return core::ptr::null_mut();
    }
    // `main_data` points at the first field of `StreamOwner`, so both pointers
    // have the same address.
    unsafe { (*stream).main_data.cast::<StreamOwner>() }
}

/// Return the persistent private state, allocating it on first frame decode.
///
/// # Safety
/// `stream` must point to a valid writable `MadStream` initialized by this
/// wrapper. If `main_data` is non-null, it must still point to the allocation
/// created by this wrapper; callers may change its bytes but must not replace
/// or free the pointer.
pub(crate) unsafe fn stream_owner(stream: *mut MadStream) -> *mut StreamOwner {
    if stream.is_null() {
        return core::ptr::null_mut();
    }
    if unsafe { (*stream).main_data.is_null() } {
        let layout = Layout::new::<StreamOwner>();
        let owner = unsafe { alloc(layout).cast::<StreamOwner>() };
        if owner.is_null() {
            unsafe { (*stream).error = MAD_ERROR_NOMEM };
            return core::ptr::null_mut();
        }
        unsafe {
            core::ptr::write(owner, StreamOwner::from_abi(&*stream));
            (*stream).main_data = core::ptr::addr_of_mut!((*owner).main_data).cast::<c_void>();
        }
        return owner;
    }
    unsafe { owner_from_stream(stream) }
}

/// Copy public C stream fields into the local Rust stream, retaining decoder
/// state in the `StreamOwner`.
///
/// # Safety
/// `stream` must be valid. Its non-null input range must contain `bufend -
/// buffer` readable bytes, and pointer fields must refer to that range.
pub(crate) unsafe fn refresh_codec(owner: &mut StreamOwner, stream: &MadStream) {
    owner.codec = load_codec(stream);
}

/// Copy local cursor and status fields back into the public C structure.
///
/// # Safety
/// `stream` must be valid and its `buffer` range must still be writable only
/// through the caller's ownership rules. This function does not write to it.
pub(crate) unsafe fn publish_codec(owner: &StreamOwner, stream: &mut MadStream) {
    publish_stream_fields(&owner.codec, stream);
}

pub(crate) fn publish_stream_fields(codec: &Stream, stream: &mut MadStream) {
    stream.skiplen = codec.skiplen;
    stream.sync = i32::from(codec.sync);
    stream.freerate = codec.freerate;
    stream.this_frame = pointer_at(stream.buffer, codec.this_frame);
    stream.next_frame = pointer_at(stream.buffer, codec.next_frame);
    stream.ptr = bitptr_to_abi(stream.buffer, codec.ptr);
    stream.anc_ptr = bitptr_to_abi(stream.buffer, codec.anc_ptr);
    stream.anc_bitlen = codec.anc_bitlen;
    stream.md_len = codec.md_len;
    stream.options = codec.options as i32;
    stream.error = codec.error as i32;
}

pub(crate) fn load_codec(stream: &MadStream) -> Stream {
    let mut codec = Stream::new();
    let len = pointer_distance(stream.buffer, stream.bufend).unwrap_or(0);
    if !stream.buffer.is_null() && len != 0 {
        // The C caller owns this memory and guarantees it remains readable
        // through the duration of each ABI call. The Rust codec takes a copy.
        let input = unsafe { core::slice::from_raw_parts(stream.buffer, len) };
        codec.buffer(input);
    } else if stream.buffer.is_null() {
        codec.buffer_set = false;
    } else {
        codec.buffer(&[]);
    }
    codec.skiplen = stream.skiplen;
    codec.sync = stream.sync != 0;
    codec.freerate = stream.freerate;
    codec.this_frame = pointer_index(stream.buffer, stream.this_frame, len).unwrap_or(0);
    codec.next_frame = pointer_index(stream.buffer, stream.next_frame, len).unwrap_or(0);
    let pointer_limit = len;
    codec.ptr = bitptr_from_abi(stream.buffer, stream.ptr, pointer_limit);
    codec.anc_ptr = bitptr_from_abi(stream.buffer, stream.anc_ptr, pointer_limit);
    codec.anc_bitlen = stream.anc_bitlen;
    codec.md_len = stream.md_len;
    codec.options = stream.options as u32;
    codec.error = error_from_abi(stream.error);
    codec
}

fn error_from_abi(error: i32) -> Error {
    Error::from_code(error as u16).unwrap_or(Error::None)
}

fn pointer_distance(start: *const u8, end: *const u8) -> Option<usize> {
    if start.is_null() || end.is_null() {
        return None;
    }
    (end as usize).checked_sub(start as usize)
}

fn pointer_index(base: *const u8, ptr: *const u8, limit: usize) -> Option<usize> {
    if base.is_null() || ptr.is_null() {
        return None;
    }
    let offset = (ptr as usize).checked_sub(base as usize)?;
    (offset <= limit).then_some(offset)
}

fn pointer_at(base: *const u8, offset: usize) -> *const u8 {
    base.wrapping_add(offset)
}

fn bitptr_from_abi(base: *const u8, ptr: MadBitPtr, limit: usize) -> BitPtr {
    BitPtr {
        byte: pointer_index(base, ptr.byte, limit).unwrap_or(0),
        cache: ptr.cache,
        left: ptr.left,
    }
}

fn bitptr_to_abi(base: *const u8, ptr: BitPtr) -> MadBitPtr {
    MadBitPtr {
        byte: pointer_at(base, ptr.byte),
        cache: ptr.cache,
        left: ptr.left,
    }
}

/// Initialize a stream structure.
///
/// # Safety
/// `stream` may be null or point to writable `MadStream` storage.
pub unsafe fn stream_init(stream: *mut MadStream) {
    if !stream.is_null() {
        let mut value = MadStream::default();
        value.ptr.left = 8;
        value.anc_ptr.left = 8;
        unsafe { *stream = value };
    }
}

/// Release the private reservoir/overlap state associated with a stream.
///
/// # Safety
/// `stream` may be null or point to a valid `MadStream` initialized by this
/// wrapper. Do not copy an initialized stream structure and finish both copies.
pub unsafe fn stream_finish(stream: *mut MadStream) {
    if stream.is_null() {
        return;
    }
    let state = unsafe { owner_from_stream(stream) };
    if !state.is_null() {
        unsafe {
            core::ptr::drop_in_place(state);
            dealloc(state.cast::<u8>(), Layout::new::<StreamOwner>());
            (*stream).main_data = core::ptr::null_mut();
        }
    }
}

/// Set the input bytes for a stream.
///
/// # Safety
/// `stream` must be writable. For nonzero `len`, `buffer` must point to at
/// least `len` readable bytes that remain valid while the stream is in use.
pub unsafe fn stream_buffer(stream: *mut MadStream, buffer: *const u8, len: u32) {
    if stream.is_null() {
        return;
    }
    unsafe {
        (*stream).buffer = buffer;
        (*stream).bufend = buffer.wrapping_add(len as usize);
        (*stream).this_frame = buffer;
        (*stream).next_frame = buffer;
        (*stream).sync = 1;
        (*stream).ptr = MadBitPtr {
            byte: buffer,
            cache: 0,
            left: 8,
        };
    }
    let state = unsafe { owner_from_stream(stream) };
    if !state.is_null() {
        unsafe {
            refresh_codec(&mut *state, &*stream);
            publish_codec(&*state, &mut *stream);
        }
    }
}

/// Add bytes to skip before the next frame.
///
/// # Safety
/// `stream` may be null or point to a writable `MadStream`.
pub unsafe fn stream_skip(stream: *mut MadStream, len: u32) {
    if stream.is_null() {
        return;
    }
    unsafe { (*stream).skiplen = (*stream).skiplen.wrapping_add(len) };
    let state = unsafe { owner_from_stream(stream) };
    if !state.is_null() {
        unsafe { (*state).codec.skip(len) };
    }
}

/// Find the next sync word while preserving the C cursor behavior.
///
/// # Safety
/// `stream` must be writable. The declared input range and cursor pointer must
/// be valid for reads until `bufend`.
pub unsafe fn stream_sync(stream: *mut MadStream) -> i32 {
    if stream.is_null() {
        return -1;
    }
    let mut codec = load_codec(unsafe { &*stream });
    if codec.sync_to_next().is_err() {
        return -1;
    }
    let ptr = bitptr_to_abi(unsafe { (*stream).buffer }, codec.ptr);
    unsafe {
        (*stream).ptr = ptr;
        let state = owner_from_stream(stream);
        if !state.is_null() {
            (*state).codec.ptr = codec.ptr;
        }
    }
    0
}

/// Return the C error string for the current stream error, or null if unknown.
///
/// # Safety
/// `stream` may be null or point to readable `MadStream` storage.
pub unsafe fn stream_errorstr(stream: *const MadStream) -> *const i8 {
    if stream.is_null() {
        return core::ptr::null();
    }
    // The bundled C switch has no MAD_ERROR_BADMODE case. Keep that internal
    // decoder value observable as an unknown error through this public API.
    if unsafe { (*stream).error } == MAD_ERROR_BADMODE {
        return core::ptr::null();
    }
    mad_error_string(unsafe { (*stream).error })
        .map_or(core::ptr::null(), |bytes| bytes.as_ptr().cast())
}

#[cfg(test)]
mod tests {
    use super::*;
    use core::ffi::CStr;

    #[test]
    fn stream_error_strings_preserve_bad_mode_exception_and_unknown_values() {
        let mut stream = MadStream::default();
        for error in [MAD_ERROR_BADMODE, 0x7fff, -1, 0x10001] {
            stream.error = error;
            assert!(unsafe { stream_errorstr(&stream) }.is_null());
        }
        stream.error = MAD_ERROR_BADCRC;
        let text = unsafe { stream_errorstr(&stream) };
        assert!(!text.is_null());
        assert_eq!(
            unsafe { CStr::from_ptr(text) }.to_bytes(),
            b"CRC check failed"
        );
    }

    #[test]
    fn imported_errors_preserve_low_word_conversion_and_unknown_fallback() {
        let mut stream = MadStream::default();
        for (raw, expected) in [
            (0x0000, Error::None),
            (0x0201, Error::BadCrc),
            (0x0222, Error::BadMode),
            (0x10201, Error::BadCrc),
            (0x7fff, Error::None),
            (-1, Error::None),
        ] {
            stream.error = raw;
            assert_eq!(load_codec(&stream).error, expected, "raw={raw}");
        }
    }
}
