#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

extern crate alloc;

use alloc::boxed::Box;
use core::ffi::c_void;
use core::ptr;

pub use aesnd_sys as sys;

pub type AudioCallback = sys::AesndAudioCallback;
pub type AudioCallbackArg = sys::AesndAudioCallbackArg;
pub type VoiceCallback = sys::AesndVoiceCallback;
pub type VoiceCallbackArg = sys::AesndVoiceCallbackArg;

/// Error returned when audio data cannot be represented by the AESND ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AudioBufferError {
    /// AESND represents retained buffer lengths as `u32` byte counts.
    TooLong,
}

#[repr(C, align(32))]
#[derive(Clone, Copy)]
struct CacheLine([u8; 32]);

/// An owned, cache-line-aligned AESND audio buffer.
///
/// AESND retains the supplied pointer while a voice is playing. This type copies
/// input bytes into 32-byte-aligned storage and flushes the written cache lines
/// before exposing the allocation to AESND. Owning the allocation also makes
/// forgetting an [`OwnedPlayback`] memory-safe: the allocation leaks rather than
/// being freed while AESND can still read it.
pub struct OwnedAudioBuffer {
    storage: Box<[CacheLine]>,
    len: u32,
}

impl OwnedAudioBuffer {
    /// Copies audio bytes into an owned, cache-line-aligned allocation.
    pub fn copy_from_slice(bytes: &[u8]) -> Result<Self, AudioBufferError> {
        let len = u32::try_from(bytes.len()).map_err(|_| AudioBufferError::TooLong)?;
        let line_len = bytes
            .len()
            .checked_add(core::mem::size_of::<CacheLine>() - 1)
            .ok_or(AudioBufferError::TooLong)?
            / core::mem::size_of::<CacheLine>();
        let mut storage = alloc::vec![CacheLine([0; 32]); line_len].into_boxed_slice();

        if !bytes.is_empty() {
            // SAFETY: `storage` contains at least `bytes.len()` writable bytes,
            // the source and destination do not overlap, and both allocations
            // remain alive for the duration of the copy.
            unsafe {
                ptr::copy_nonoverlapping(
                    bytes.as_ptr(),
                    storage.as_mut_ptr().cast::<u8>(),
                    bytes.len(),
                );

                // `storage` is cache-line aligned and contains at least `len`
                // bytes. Flushing makes the copied data visible to AESND/DSP.
                ogc_cache::dc_flush_range(storage.as_ptr().cast::<c_void>(), len);
            }
        }

        Ok(Self { storage, len })
    }

    /// Returns the retained byte length passed to AESND.
    pub const fn len(&self) -> u32 {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the copied audio data.
    pub fn as_bytes(&self) -> &[u8] {
        // SAFETY: `storage` contains at least `self.len` initialized bytes. Its
        // pointer is non-null and suitably aligned even when the slice is empty.
        unsafe {
            core::slice::from_raw_parts(self.storage.as_ptr().cast::<u8>(), self.len as usize)
        }
    }

    fn as_ptr(&self) -> *const c_void {
        self.storage.as_ptr().cast::<c_void>()
    }
}

impl TryFrom<&[u8]> for OwnedAudioBuffer {
    type Error = AudioBufferError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        Self::copy_from_slice(bytes)
    }
}

impl TryFrom<Box<[u8]>> for OwnedAudioBuffer {
    type Error = AudioBufferError;

    fn try_from(bytes: Box<[u8]>) -> Result<Self, Self::Error> {
        Self::copy_from_slice(&bytes)
    }
}

#[derive(Default)]
pub struct VoiceHandle {
    raw: *mut sys::Aesndpb,
}

impl VoiceHandle {
    pub const fn new(raw: *mut sys::Aesndpb) -> Self {
        Self { raw }
    }

    pub const fn as_ptr(&self) -> *mut sys::Aesndpb {
        self.raw
    }

    pub const fn is_null(&self) -> bool {
        self.raw.is_null()
    }

    pub const fn into_raw(self) -> *mut sys::Aesndpb {
        self.raw
    }

    /// # Safety
    ///
    /// `raw` must remain valid for the lifetime of the returned handle, and
    /// the caller must ensure ownership rules are respected by the backend.
    pub const unsafe fn from_raw(raw: *mut sys::Aesndpb) -> Self {
        Self { raw }
    }

    pub fn free(self) {
        sys::free_voice(self.raw);
    }

    pub fn set_stop(&self, stop: bool) {
        sys::set_voice_stop(self.raw, stop as sys::Bool);
    }

    pub fn set_mute(&self, mute: bool) {
        sys::set_voice_mute(self.raw, mute as sys::Bool);
    }

    pub fn set_loop(&self, looped: bool) {
        sys::set_voice_loop(self.raw, looped as sys::Bool);
    }

    pub fn set_format(&self, format: u32) {
        sys::set_voice_format(self.raw, format);
    }

    pub fn set_stream(&self, stream: bool) {
        sys::set_voice_stream(self.raw, stream as sys::Bool);
    }

    pub fn set_frequency(&self, freq: f32) {
        sys::set_voice_frequency(self.raw, freq);
    }

    pub fn set_volume(&self, volume_l: u16, volume_r: u16) {
        sys::set_voice_volume(self.raw, volume_l, volume_r);
    }

    pub fn set_delay(&self, delay: u32) {
        sys::set_voice_delay(self.raw, delay);
    }

    /// # Safety
    ///
    /// `buffer` must remain valid until the voice stops using it and must obey
    /// the alignment rules imposed by the AESND driver.
    pub unsafe fn set_buffer(&self, buffer: *const core::ffi::c_void, len: u32) {
        // SAFETY: This method exposes the caller contract documented above.
        unsafe { sys::set_voice_buffer(self.raw, buffer, len) };
    }

    /// # Safety
    ///
    /// `buffer` must remain valid until the voice stops using it and must obey
    /// the alignment rules imposed by the AESND driver.
    pub unsafe fn play_voice(
        &self,
        format: u32,
        buffer: *const core::ffi::c_void,
        len: u32,
        freq: f32,
        delay: u32,
        looped: bool,
    ) {
        // SAFETY: This method exposes the caller contract documented above.
        unsafe {
            sys::play_voice(
                self.raw,
                format,
                buffer,
                len,
                freq,
                delay,
                looped as sys::Bool,
            )
        };
    }

    /// Starts playback while transferring ownership of the retained buffer.
    ///
    /// The returned guard keeps both the voice and buffer alive. Dropping it
    /// stops and detaches the voice before freeing either resource.
    pub fn play_owned(
        self,
        format: u32,
        buffer: OwnedAudioBuffer,
        freq: f32,
        delay: u32,
        looped: bool,
    ) -> OwnedPlayback {
        // SAFETY: `OwnedPlayback` owns `buffer` for at least as long as AESND
        // retains its pointer. `OwnedAudioBuffer` guarantees four-byte alignment
        // and stores a checked `u32` byte length.
        unsafe {
            sys::play_voice(
                self.raw,
                format,
                buffer.as_ptr(),
                buffer.len(),
                freq,
                delay,
                looped as sys::Bool,
            );
        }

        OwnedPlayback {
            voice: Some(self),
            buffer: Some(buffer),
        }
    }

    pub fn register_voice_callback(&self, cb: VoiceCallback) -> VoiceCallback {
        sys::register_voice_callback(self.raw, cb)
    }
}

/// A playing AESND voice together with the buffer retained by the backend.
///
/// This type is intentionally not `Copy` or `Clone`. Calling [`core::mem::forget`]
/// leaks the voice and buffer, which is preferable to freeing memory still visible
/// to AESND.
pub struct OwnedPlayback {
    voice: Option<VoiceHandle>,
    buffer: Option<OwnedAudioBuffer>,
}

impl OwnedPlayback {
    pub fn voice(&self) -> &VoiceHandle {
        self.voice
            .as_ref()
            .expect("owned playback always contains a voice")
    }

    pub fn buffer(&self) -> &OwnedAudioBuffer {
        self.buffer
            .as_ref()
            .expect("owned playback always contains a buffer")
    }

    /// Atomically points AESND at a new owned buffer and returns the old one.
    pub fn replace_buffer(&mut self, buffer: OwnedAudioBuffer) -> OwnedAudioBuffer {
        let voice = self
            .voice
            .as_ref()
            .expect("owned playback always contains a voice");

        // SAFETY: `self` takes ownership of `buffer` before the old buffer can be
        // dropped. The new allocation is aligned and remains alive until another
        // replacement, `into_parts`, or `drop` detaches it from AESND.
        unsafe {
            sys::set_voice_buffer(voice.raw, buffer.as_ptr(), buffer.len());
        }

        self.buffer
            .replace(buffer)
            .expect("owned playback always contains a buffer")
    }

    /// Stops playback, detaches the retained pointer, and returns both resources.
    pub fn into_parts(mut self) -> (VoiceHandle, OwnedAudioBuffer) {
        self.stop_and_detach();
        let voice = self
            .voice
            .take()
            .expect("owned playback always contains a voice");
        let buffer = self
            .buffer
            .take()
            .expect("owned playback always contains a buffer");
        (voice, buffer)
    }

    fn stop_and_detach(&self) {
        let Some(voice) = self.voice.as_ref() else {
            return;
        };

        voice.set_stop(true);

        // SAFETY: A null pointer with a zero length describes an empty buffer and
        // removes AESND's reference to the owned allocation before it can be freed.
        unsafe {
            sys::set_voice_buffer(voice.raw, core::ptr::null(), 0);
        }
    }
}

impl Drop for OwnedPlayback {
    fn drop(&mut self) {
        self.stop_and_detach();
        if let Some(voice) = self.voice.take() {
            voice.free();
        }
    }
}

pub fn init() {
    sys::init();
}

pub fn reset() {
    sys::reset();
}

pub fn pause(pause: bool) {
    sys::pause(pause as sys::Bool);
}

pub fn get_dsp_process_time() -> u32 {
    sys::get_dsp_process_time()
}

pub fn get_dsp_process_usage() -> f32 {
    sys::get_dsp_process_usage()
}

pub fn register_audio_callback(cb: AudioCallback) -> AudioCallback {
    sys::register_audio_callback(cb)
}

pub fn allocate_voice(cb: VoiceCallback) -> Option<VoiceHandle> {
    let raw = sys::allocate_voice(cb);

    if raw.is_null() {
        None
    } else {
        Some(VoiceHandle::new(raw))
    }
}

pub fn register_voice_callback(handle: &VoiceHandle, cb: VoiceCallback) -> VoiceCallback {
    handle.register_voice_callback(cb)
}

pub fn free_voice(handle: VoiceHandle) {
    handle.free();
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    #[test]
    fn voice_handle_round_trips_raw_pointer() {
        let ptr = 0x1234usize as *mut sys::Aesndpb;
        let handle = VoiceHandle::new(ptr);
        assert_eq!(handle.as_ptr(), ptr);
        assert!(!handle.is_null());
        assert_eq!(handle.into_raw(), ptr);
    }

    #[test]
    fn owned_audio_buffer_is_aligned_and_preserves_bytes() {
        let bytes = [1_u8, 2, 3, 4, 5];
        let buffer = OwnedAudioBuffer::copy_from_slice(&bytes).expect("small buffer");
        assert_eq!(buffer.as_bytes(), bytes);
        assert_eq!((buffer.as_ptr() as usize) % 32, 0);
    }

    #[test]
    fn helper_functions_exist_without_backend() {
        let _ = init as fn();
        let _ = reset as fn();
        let _ = pause as fn(bool);
        let _ = get_dsp_process_time as fn() -> u32;
        let _ = get_dsp_process_usage as fn() -> f32;
        let _ = register_audio_callback as fn(AudioCallback) -> AudioCallback;
        let _ = allocate_voice as fn(VoiceCallback) -> Option<VoiceHandle>;
    }
}
