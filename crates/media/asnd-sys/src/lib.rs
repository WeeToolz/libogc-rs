#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

extern crate alloc;

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::ffi::c_void;
use core::fmt;
use core::ptr;
use core::ptr::{read_unaligned, write_bytes};
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicU32, AtomicUsize, Ordering};

use ogc_audio_sys as audio;
use ogc_cache::dc_flush_range;
use ogc_lwp_driver::{TICKS_PER_SECOND, now_ticks};

pub const ASND_LIB: i32 = 0x100;
pub const SND_LIB: i32 = ASND_LIB + 2;

pub const MAX_SND_VOICES: usize = 16;

pub const SND_OK: i32 = 0;
pub const SND_INVALID: i32 = -1;
pub const SND_ISNOTASONGVOICE: i32 = -2;
pub const SND_BUSY: i32 = 1;

pub const SND_UNUSED: i32 = 0;
pub const SND_WORKING: i32 = 1;
pub const SND_WAITING: i32 = 2;

pub const VOICE_MONO_8BIT: i32 = 0;
pub const VOICE_MONO_16BIT: i32 = 1;
pub const VOICE_MONO_16BIT_BE: i32 = 1;
pub const VOICE_STEREO_8BIT: i32 = 2;
pub const VOICE_STEREO_16BIT: i32 = 3;
pub const VOICE_STEREO_16BIT_BE: i32 = 3;
pub const VOICE_MONO_8BIT_U: i32 = 4;
pub const VOICE_MONO_16BIT_LE: i32 = 5;
pub const VOICE_STEREO_8BIT_U: i32 = 6;
pub const VOICE_STEREO_16BIT_LE: i32 = 7;

pub const MIN_VOLUME: i32 = 0;
pub const MID_VOLUME: i32 = 127;
pub const MAX_VOLUME: i32 = 255;

pub const MIN_PITCH: i32 = 1;
pub const F44100HZ_PITCH: i32 = 44_100;
pub const MAX_PITCH: i32 = 144_000;

pub const NOTE_DO: i32 = 0;
pub const NOTE_DOS: i32 = 1;
pub const NOTE_REB: i32 = NOTE_DOS;
pub const NOTE_RE: i32 = 2;
pub const NOTE_RES: i32 = 3;
pub const NOTE_MIB: i32 = NOTE_RES;
pub const NOTE_MI: i32 = 4;
pub const NOTE_FA: i32 = 5;
pub const NOTE_FAS: i32 = 6;
pub const NOTE_SOLB: i32 = NOTE_FAS;
pub const NOTE_SOL: i32 = 7;
pub const NOTE_SOLS: i32 = 8;
pub const NOTE_LAB: i32 = NOTE_SOLS;
pub const NOTE_LA: i32 = 9;
pub const NOTE_LAS: i32 = 10;
pub const NOTE_SIB: i32 = NOTE_LAS;
pub const NOTE_SI: i32 = 11;

pub const NOTE_C: i32 = 0;
pub const NOTE_CS: i32 = 1;
pub const NOTE_DB: i32 = NOTE_CS;
pub const NOTE_D: i32 = 2;
pub const NOTE_DS: i32 = 3;
pub const NOTE_EB: i32 = NOTE_DS;
pub const NOTE_E: i32 = 4;
pub const NOTE_F: i32 = 5;
pub const NOTE_FS: i32 = 6;
pub const NOTE_GB: i32 = NOTE_FS;
pub const NOTE_G: i32 = 7;
pub const NOTE_GS: i32 = 8;
pub const NOTE_AB: i32 = NOTE_GS;
pub const NOTE_A: i32 = 9;
pub const NOTE_AS: i32 = 10;
pub const NOTE_BB: i32 = NOTE_AS;
pub const NOTE_B: i32 = 11;

pub type AsndVoiceCallback = Option<unsafe extern "C" fn(voice: i32)>;
pub type AsndCallback = Option<unsafe extern "C" fn()>;

const AUDIO_RATE: i32 = 48_000;
const SND_BUFFERSIZE: u32 = 4096;
const SAMPLES_PER_TICK: u32 = SND_BUFFERSIZE / 4;
const SYS_BASE_UNCACHED: u32 = 0xC000_0000;

const VOICE_UPDATEADD: i32 = 1 << 12;
const VOICE_UPDATE: i32 = 1 << 11;
const VOICE_VOLUPDATE: i32 = 1 << 10;
const VOICE_PAUSE: i32 = 1 << 9;
const VOICE_SETLOOP: i32 = 1 << 8;

static ASND_INITED: AtomicBool = AtomicBool::new(false);
static ASND_PAUSED: AtomicI32 = AtomicI32::new(1);
static ASND_GLOBAL_COUNTER: AtomicU32 = AtomicU32::new(0);
static ASND_CALLBACK_SET: AtomicBool = AtomicBool::new(false);
static ASND_GLOBAL_CALLBACK: AtomicUsize = AtomicUsize::new(0);
static ASND_CALLBACK_ACTIVE: AtomicBool = AtomicBool::new(false);
static ASND_DSP_PERCENT: AtomicU32 = AtomicU32::new(0);
static ASND_DSP_PROCESS_TIME: AtomicU32 = AtomicU32::new(0);
static ASND_CURRENT_BUFFER: AtomicUsize = AtomicUsize::new(0);

static VOICE_FLAGS: [AtomicI32; MAX_SND_VOICES] = [const { AtomicI32::new(0) }; MAX_SND_VOICES];
static VOICE_FORMAT: [AtomicI32; MAX_SND_VOICES] = [const { AtomicI32::new(0) }; MAX_SND_VOICES];
static VOICE_PITCH: [AtomicI32; MAX_SND_VOICES] = [const { AtomicI32::new(0) }; MAX_SND_VOICES];
static VOICE_DELAY_SAMPLES: [AtomicU32; MAX_SND_VOICES] =
    [const { AtomicU32::new(0) }; MAX_SND_VOICES];
static VOICE_VOLUME_L: [AtomicI32; MAX_SND_VOICES] = [const { AtomicI32::new(0) }; MAX_SND_VOICES];
static VOICE_VOLUME_R: [AtomicI32; MAX_SND_VOICES] = [const { AtomicI32::new(0) }; MAX_SND_VOICES];
static VOICE_PTR_1: [AtomicUsize; MAX_SND_VOICES] = [const { AtomicUsize::new(0) }; MAX_SND_VOICES];
static VOICE_PTR_2: [AtomicUsize; MAX_SND_VOICES] = [const { AtomicUsize::new(0) }; MAX_SND_VOICES];
static VOICE_BACKUP_PTR: [AtomicUsize; MAX_SND_VOICES] =
    [const { AtomicUsize::new(0) }; MAX_SND_VOICES];
static VOICE_END_1: [AtomicUsize; MAX_SND_VOICES] = [const { AtomicUsize::new(0) }; MAX_SND_VOICES];
static VOICE_END_2: [AtomicUsize; MAX_SND_VOICES] = [const { AtomicUsize::new(0) }; MAX_SND_VOICES];
static VOICE_TICKS: [AtomicU32; MAX_SND_VOICES] = [const { AtomicU32::new(0) }; MAX_SND_VOICES];
static VOICE_HAS_CALLBACK: [AtomicBool; MAX_SND_VOICES] =
    [const { AtomicBool::new(false) }; MAX_SND_VOICES];
static VOICE_CALLBACK_ACTIVE: [AtomicBool; MAX_SND_VOICES] =
    [const { AtomicBool::new(false) }; MAX_SND_VOICES];
static VOICE_CALLBACK: [AtomicUsize; MAX_SND_VOICES] =
    [const { AtomicUsize::new(0) }; MAX_SND_VOICES];
static OWNED_VOICE_CALLBACK: [AtomicUsize; MAX_SND_VOICES] =
    [const { AtomicUsize::new(0) }; MAX_SND_VOICES];
static VOICE_PHASE: [AtomicU32; MAX_SND_VOICES] = [const { AtomicU32::new(0) }; MAX_SND_VOICES];

#[repr(C, align(32))]
struct AudioBuffers([[u8; SND_BUFFERSIZE as usize]; 2]);

#[repr(C, align(32))]
struct MuteBuffer([u8; SND_BUFFERSIZE as usize]);

static mut AUDIO_BUFFERS: AudioBuffers = AudioBuffers([[0; SND_BUFFERSIZE as usize]; 2]);
static mut MUTE_BUFFER: MuteBuffer = MuteBuffer([0; SND_BUFFERSIZE as usize]);

const fn note_code(note: i32, octave: i32) -> i32 {
    note + (octave << 3) + (octave << 2)
}

fn voice_index(voice: i32) -> Option<usize> {
    if (0..MAX_SND_VOICES as i32).contains(&voice) {
        Some(voice as usize)
    } else {
        None
    }
}

fn valid_voice_buffer(ptr: *const c_void, size: i32) -> bool {
    size > 0 && !ptr.is_null()
}

#[inline(always)]
fn mem_virtual_to_physical(ptr: *const c_void) -> u32 {
    (ptr as usize as u32) & !SYS_BASE_UNCACHED
}

#[inline(always)]
fn mem_physical_to_virtual(addr: usize) -> *const u8 {
    (addr as u32 | 0x8000_0000) as usize as *const u8
}

fn voice_step(format: i32) -> i32 {
    match format & 3 {
        0 => 1 << 16,
        1 | 2 => 2 << 16,
        _ => 4 << 16,
    }
}

fn voice_pitch_step(pitch: i32) -> u32 {
    let pitch = pitch.clamp(MIN_PITCH, MAX_PITCH) as u64;
    ((pitch << 16) / AUDIO_RATE as u64).max(1) as u32
}

fn ticks_to_nanoseconds(ticks: u64) -> u32 {
    ((u128::from(ticks) * 1_000_000_000) / u128::from(TICKS_PER_SECOND)) as u32
}

fn voice_bytes_per_frame(format: i32) -> usize {
    match format & 7 {
        VOICE_MONO_8BIT | VOICE_MONO_8BIT_U => 1,
        VOICE_MONO_16BIT | VOICE_MONO_16BIT_LE => 2,
        VOICE_STEREO_8BIT | VOICE_STEREO_8BIT_U => 2,
        VOICE_STEREO_16BIT | VOICE_STEREO_16BIT_LE => 4,
        _ => 1,
    }
}

fn voice_frame_count(slot: usize) -> usize {
    let format = VOICE_FORMAT[slot].load(Ordering::Acquire);
    let start = VOICE_PTR_1[slot].load(Ordering::Acquire);
    let end = VOICE_END_1[slot].load(Ordering::Acquire);
    if end <= start {
        return 0;
    }
    let bytes = end - start;
    let frame_bytes = voice_bytes_per_frame(format);
    bytes.checked_div(frame_bytes).unwrap_or(0)
}

fn voice_buffer_empty(slot: usize) -> bool {
    VOICE_PTR_1[slot].load(Ordering::Acquire) == 0 && VOICE_PTR_2[slot].load(Ordering::Acquire) == 0
}

fn read_voice_frame(slot: usize, frame_index: usize) -> (i16, i16) {
    let format = VOICE_FORMAT[slot].load(Ordering::Acquire) & 7;
    let start = VOICE_PTR_1[slot].load(Ordering::Acquire);
    if start == 0 {
        return (0, 0);
    }
    let bytes_per_frame = voice_bytes_per_frame(format);
    let ptr =
        mem_physical_to_virtual(start.wrapping_add(frame_index.wrapping_mul(bytes_per_frame)));

    unsafe {
        match format {
            VOICE_MONO_8BIT => {
                let sample = read_unaligned(ptr.cast::<i8>()) as i16;
                let sample = sample << 8;
                (sample, sample)
            }
            VOICE_MONO_16BIT => {
                let sample = i16::from_be_bytes([*ptr, *ptr.add(1)]);
                (sample, sample)
            }
            VOICE_MONO_16BIT_LE => {
                let sample = i16::from_le_bytes([*ptr, *ptr.add(1)]);
                (sample, sample)
            }
            VOICE_STEREO_8BIT => {
                let left = (read_unaligned(ptr.cast::<i8>()) as i16) << 8;
                let right = (read_unaligned(ptr.add(1).cast::<i8>()) as i16) << 8;
                (left, right)
            }
            VOICE_STEREO_16BIT => {
                let left = i16::from_be_bytes([*ptr, *ptr.add(1)]);
                let right = i16::from_be_bytes([*ptr.add(2), *ptr.add(3)]);
                (left, right)
            }
            VOICE_STEREO_16BIT_LE => {
                let left = i16::from_le_bytes([*ptr, *ptr.add(1)]);
                let right = i16::from_le_bytes([*ptr.add(2), *ptr.add(3)]);
                (left, right)
            }
            VOICE_MONO_8BIT_U => {
                let sample = ((read_unaligned(ptr.cast::<u8>()) as i32 - 128) << 8) as i16;
                (sample, sample)
            }
            VOICE_STEREO_8BIT_U => {
                let left = ((read_unaligned(ptr.cast::<u8>()) as i32 - 128) << 8) as i16;
                let right = ((read_unaligned(ptr.add(1).cast::<u8>()) as i32 - 128) << 8) as i16;
                (left, right)
            }
            _ => (0, 0),
        }
    }
}

fn clear_voice_runtime(slot: usize) {
    clear_voice(slot);
}

fn deactivate_voice(slot: usize) {
    VOICE_FLAGS[slot].store(0, Ordering::Release);
    VOICE_PTR_1[slot].store(0, Ordering::Release);
    VOICE_PTR_2[slot].store(0, Ordering::Release);
    VOICE_BACKUP_PTR[slot].store(0, Ordering::Release);
    VOICE_END_1[slot].store(0, Ordering::Release);
    VOICE_END_2[slot].store(0, Ordering::Release);
    VOICE_PHASE[slot].store(0, Ordering::Release);
}

fn reset_runtime_state() {
    ASND_PAUSED.store(1, Ordering::Release);
    ASND_GLOBAL_COUNTER.store(0, Ordering::Release);
    ASND_CALLBACK_SET.store(false, Ordering::Release);
    ASND_GLOBAL_CALLBACK.store(0, Ordering::Release);
    ASND_CALLBACK_ACTIVE.store(false, Ordering::Release);
    ASND_DSP_PERCENT.store(0, Ordering::Release);
    ASND_DSP_PROCESS_TIME.store(0, Ordering::Release);
    ASND_CURRENT_BUFFER.store(0, Ordering::Release);
    for (slot, ticks) in VOICE_TICKS.iter().enumerate() {
        clear_voice_runtime(slot);
        ticks.store(0, Ordering::Release);
    }

    unsafe {
        write_bytes(
            core::ptr::addr_of_mut!(MUTE_BUFFER.0).cast::<u8>(),
            0,
            SND_BUFFERSIZE as usize,
        );
        write_bytes(
            core::ptr::addr_of_mut!(AUDIO_BUFFERS.0).cast::<u8>(),
            0,
            core::mem::size_of::<AudioBuffers>(),
        );
    }
}

fn current_audio_buffer_ptr(index: usize) -> *mut u8 {
    unsafe { core::ptr::addr_of_mut!(AUDIO_BUFFERS.0[index]).cast::<u8>() }
}

fn mute_buffer_ptr() -> *mut u8 {
    unsafe { core::ptr::addr_of_mut!(MUTE_BUFFER.0).cast::<u8>() }
}

fn voice_has_pending_second_buffer(slot: usize) -> bool {
    VOICE_PTR_2[slot].load(Ordering::Acquire) != 0
}

fn promote_pending_buffer(slot: usize) -> bool {
    let next = VOICE_PTR_2[slot].load(Ordering::Acquire);
    if next == 0 {
        return false;
    }

    VOICE_PTR_1[slot].store(next, Ordering::Release);
    VOICE_BACKUP_PTR[slot].store(next, Ordering::Release);
    VOICE_END_1[slot].store(VOICE_END_2[slot].load(Ordering::Acquire), Ordering::Release);
    VOICE_PTR_2[slot].store(0, Ordering::Release);
    VOICE_END_2[slot].store(0, Ordering::Release);
    VOICE_PHASE[slot].store(0, Ordering::Release);
    true
}

fn invoke_voice_callback(slot: usize) {
    if !VOICE_HAS_CALLBACK[slot].load(Ordering::Acquire)
        || VOICE_CALLBACK_ACTIVE[slot]
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
    {
        return;
    }
    let callback = VOICE_CALLBACK[slot].load(Ordering::Acquire);
    if callback != 0 {
        unsafe {
            (core::mem::transmute::<usize, unsafe extern "C" fn(i32)>(callback))(slot as i32);
        }
    }
    VOICE_CALLBACK_ACTIVE[slot].store(false, Ordering::Release);
}

fn claim_voice_callback(
    invoked_this_buffer: &mut bool,
    has_callback: bool,
    has_pending_buffer: bool,
) -> bool {
    if *invoked_this_buffer || !has_callback || has_pending_buffer {
        return false;
    }
    *invoked_this_buffer = true;
    true
}

unsafe extern "C" fn owned_voice_callback(voice: i32) {
    let Some(slot) = voice_index(voice) else {
        return;
    };
    let callback = OWNED_VOICE_CALLBACK[slot].load(Ordering::Acquire);
    if callback != 0 {
        // SAFETY: set_voice_owned stores a valid safe Rust function pointer
        // for this voice before it registers this trampoline.
        let callback = unsafe { core::mem::transmute::<usize, fn(i32)>(callback) };
        callback(voice);
    }
}

fn voice_active(flags: i32) -> bool {
    (flags >> 16) != 0
}

fn clear_voice(slot: usize) {
    deactivate_voice(slot);
    VOICE_FORMAT[slot].store(0, Ordering::Release);
    VOICE_PITCH[slot].store(0, Ordering::Release);
    VOICE_DELAY_SAMPLES[slot].store(0, Ordering::Release);
    VOICE_VOLUME_L[slot].store(0, Ordering::Release);
    VOICE_VOLUME_R[slot].store(0, Ordering::Release);
    VOICE_HAS_CALLBACK[slot].store(false, Ordering::Release);
    VOICE_CALLBACK[slot].store(0, Ordering::Release);
    OWNED_VOICE_CALLBACK[slot].store(0, Ordering::Release);
    VOICE_CALLBACK_ACTIVE[slot].store(false, Ordering::Release);
}

/// Parameters for starting a finite ASND voice.
#[derive(Clone, Copy)]
pub struct VoiceConfig {
    /// Voice slot in `0..MAX_SND_VOICES`.
    pub voice: i32,
    /// One of the `VOICE_*` sample-format constants.
    pub format: i32,
    /// Playback frequency in hertz.
    pub pitch: i32,
    /// Delay before playback in milliseconds.
    pub delay: i32,
    /// Start of the sample buffer.
    pub snd: *mut c_void,
    /// Sample-buffer length in bytes.
    pub size_snd: i32,
    /// Left-channel volume.
    pub volume_l: i32,
    /// Right-channel volume.
    pub volume_r: i32,
    /// Function invoked when the voice needs another buffer.
    pub callback: AsndVoiceCallback,
}

/// Parameters for starting a looping ASND voice.
#[derive(Clone, Copy)]
pub struct InfiniteVoiceConfig {
    /// Voice slot in `0..MAX_SND_VOICES`.
    pub voice: i32,
    /// One of the `VOICE_*` sample-format constants.
    pub format: i32,
    /// Playback frequency in hertz.
    pub pitch: i32,
    /// Delay before playback in milliseconds.
    pub delay: i32,
    /// Start of the sample buffer.
    pub snd: *mut c_void,
    /// Sample-buffer length in bytes.
    pub size_snd: i32,
    /// Left-channel volume.
    pub volume_l: i32,
    /// Right-channel volume.
    pub volume_r: i32,
}

/// Error returned when sample data cannot be represented by the ASND ABI.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OwnedVoiceBufferError {
    /// ASND does not accept empty retained buffers.
    Empty,
    /// ASND represents retained buffer lengths as signed 32-bit byte counts.
    TooLong,
}

#[repr(C, align(32))]
#[derive(Clone, Copy)]
struct OwnedVoiceCacheLine([u8; 32]);

/// Owned, 32-byte-aligned sample storage for the ASND mixer.
///
/// ASND retains submitted pointers while a voice is active. This type copies
/// input bytes into padded storage whose allocation remains stable when moved.
/// Forgetting an [`OwnedVoice`] therefore leaks the allocation instead of
/// freeing memory that the audio callback could still read.
pub struct OwnedVoiceBuffer {
    storage: Box<[OwnedVoiceCacheLine]>,
    len: i32,
}

impl OwnedVoiceBuffer {
    /// Copies sample data into owned, cache-line-aligned storage.
    pub fn copy_from_slice(bytes: &[u8]) -> Result<Self, OwnedVoiceBufferError> {
        if bytes.is_empty() {
            return Err(OwnedVoiceBufferError::Empty);
        }

        let len = i32::try_from(bytes.len()).map_err(|_| OwnedVoiceBufferError::TooLong)?;
        let line_size = core::mem::size_of::<OwnedVoiceCacheLine>();
        let line_count = bytes
            .len()
            .checked_add(line_size - 1)
            .ok_or(OwnedVoiceBufferError::TooLong)?
            / line_size;
        let mut storage = alloc::vec![OwnedVoiceCacheLine([0; 32]); line_count].into_boxed_slice();

        // SAFETY: `storage` contains at least `bytes.len()` writable bytes, and
        // the independently allocated source and destination cannot overlap.
        unsafe {
            ptr::copy_nonoverlapping(
                bytes.as_ptr(),
                storage.as_mut_ptr().cast::<u8>(),
                bytes.len(),
            );
        }

        Ok(Self { storage, len })
    }

    /// Number of sample bytes submitted to ASND.
    pub const fn len(&self) -> i32 {
        self.len
    }

    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the copied sample bytes, excluding alignment padding.
    pub fn as_bytes(&self) -> &[u8] {
        // SAFETY: construction initializes at least `self.len` bytes in the
        // non-empty allocation, which remains alive for the returned borrow.
        unsafe {
            core::slice::from_raw_parts(self.storage.as_ptr().cast::<u8>(), self.len as usize)
        }
    }

    fn as_mut_ptr(&mut self) -> *mut c_void {
        self.storage.as_mut_ptr().cast::<c_void>()
    }
}

impl fmt::Debug for OwnedVoiceBuffer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OwnedVoiceBuffer")
            .field("len", &self.len)
            .finish_non_exhaustive()
    }
}

impl TryFrom<&[u8]> for OwnedVoiceBuffer {
    type Error = OwnedVoiceBufferError;

    fn try_from(bytes: &[u8]) -> Result<Self, Self::Error> {
        Self::copy_from_slice(bytes)
    }
}

impl TryFrom<Box<[u8]>> for OwnedVoiceBuffer {
    type Error = OwnedVoiceBufferError;

    fn try_from(bytes: Box<[u8]>) -> Result<Self, Self::Error> {
        Self::copy_from_slice(&bytes)
    }
}

/// Safe callback used by [`set_voice_owned`].
///
/// The callback runs from the ASND audio path and should remain brief and
/// nonblocking. A safe function pointer is promoted internally to the raw ASND
/// callback type.
pub type OwnedVoiceCallback = Option<fn(voice: i32)>;

/// Parameters for starting a finite voice with an owned sample buffer.
#[derive(Clone, Copy)]
pub struct OwnedVoiceConfig {
    pub voice: i32,
    pub format: i32,
    pub pitch: i32,
    pub delay: i32,
    pub volume_l: i32,
    pub volume_r: i32,
    pub callback: OwnedVoiceCallback,
}

/// Parameters for starting a looping voice with an owned sample buffer.
#[derive(Clone, Copy)]
pub struct OwnedInfiniteVoiceConfig {
    pub voice: i32,
    pub format: i32,
    pub pitch: i32,
    pub delay: i32,
    pub volume_l: i32,
    pub volume_r: i32,
}

/// Failed owned-buffer submission.
///
/// The buffer is returned because ASND did not retain it.
pub struct OwnedVoiceError {
    status: i32,
    buffer: OwnedVoiceBuffer,
}

impl OwnedVoiceError {
    pub const fn status(&self) -> i32 {
        self.status
    }

    pub fn buffer(&self) -> &OwnedVoiceBuffer {
        &self.buffer
    }

    pub fn into_buffer(self) -> OwnedVoiceBuffer {
        self.buffer
    }
}

impl fmt::Debug for OwnedVoiceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OwnedVoiceError")
            .field("status", &self.status)
            .field("buffer", &self.buffer)
            .finish()
    }
}

/// A live ASND voice and every sample allocation submitted to it.
///
/// Accepted queued buffers are retained until this handle is stopped or
/// dropped. This is conservative for streaming voices, but prevents a buffer
/// from being freed while ASND may still hold either its primary or secondary
/// pointer.
pub struct OwnedVoice {
    voice: i32,
    buffers: Vec<OwnedVoiceBuffer>,
    active: bool,
}

impl OwnedVoice {
    pub const fn voice(&self) -> i32 {
        self.voice
    }

    pub fn status(&self) -> i32 {
        status_voice(self.voice)
    }

    pub fn pause(&self, paused: bool) -> i32 {
        pause_voice(self.voice, if paused { 1 } else { 0 })
    }

    pub fn buffer_count(&self) -> usize {
        self.buffers.len()
    }

    /// Queues another owned sample buffer.
    ///
    /// Successful submissions remain owned by this handle. On failure, the
    /// error returns the buffer so the caller can retry after ASND becomes
    /// ready for another queue entry.
    pub fn add_buffer(&mut self, mut buffer: OwnedVoiceBuffer) -> Result<(), OwnedVoiceError> {
        // SAFETY: `self.buffers` retains every accepted allocation until the
        // voice is stopped. The new buffer is aligned, padded, non-empty, and
        // remains owned by this call until ASND reports successful retention.
        let status = unsafe { add_voice(self.voice, buffer.as_mut_ptr(), buffer.len()) };
        if status == SND_OK {
            self.buffers.push(buffer);
            Ok(())
        } else {
            Err(OwnedVoiceError { status, buffer })
        }
    }

    /// Stops the voice and releases all owned sample allocations.
    pub fn stop(mut self) {
        self.stop_backend();
    }

    fn stop_backend(&mut self) {
        if self.active {
            let _ = stop_voice(self.voice);
            self.active = false;
        }
    }
}

impl fmt::Debug for OwnedVoice {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OwnedVoice")
            .field("voice", &self.voice)
            .field("buffer_count", &self.buffers.len())
            .field("active", &self.active)
            .finish()
    }
}

impl Drop for OwnedVoice {
    fn drop(&mut self) {
        self.stop_backend();
    }
}

struct VoiceRequest {
    voice: i32,
    format: i32,
    pitch: i32,
    delay: i32,
    snd: *mut c_void,
    size_snd: i32,
    volume_l: i32,
    volume_r: i32,
    callback: AsndVoiceCallback,
    infinite: bool,
}

fn set_voice_inner(request: VoiceRequest) -> i32 {
    let Some(slot) = voice_index(request.voice) else {
        return SND_INVALID;
    };
    if !valid_voice_buffer(request.snd.cast_const(), request.size_snd) {
        return SND_INVALID;
    }
    let format = request.format & 7;
    let pitch = request.pitch.clamp(MIN_PITCH, MAX_PITCH);
    let delay_samples = (48_000u64 * request.delay.max(0) as u64 / 1000) as u32;
    let volume_l = request.volume_l & 255;
    let volume_r = request.volume_r & 255;
    let start = mem_virtual_to_physical(request.snd.cast_const()) as usize;
    let end = start.wrapping_add(request.size_snd as usize);
    let loop_flag = if request.infinite { VOICE_SETLOOP } else { 0 };

    unsafe { dc_flush_range(request.snd.cast_const(), request.size_snd as u32) };

    VOICE_FLAGS[slot].store(
        format | voice_step(format) | VOICE_UPDATE | loop_flag,
        Ordering::Release,
    );
    VOICE_FORMAT[slot].store(format, Ordering::Release);
    VOICE_PITCH[slot].store(pitch, Ordering::Release);
    VOICE_DELAY_SAMPLES[slot].store(delay_samples, Ordering::Release);
    VOICE_VOLUME_L[slot].store(volume_l, Ordering::Release);
    VOICE_VOLUME_R[slot].store(volume_r, Ordering::Release);
    VOICE_PTR_1[slot].store(start, Ordering::Release);
    VOICE_BACKUP_PTR[slot].store(start, Ordering::Release);
    VOICE_END_1[slot].store(end, Ordering::Release);
    if request.infinite {
        VOICE_PTR_2[slot].store(start, Ordering::Release);
        VOICE_END_2[slot].store(end, Ordering::Release);
    } else {
        VOICE_PTR_2[slot].store(0, Ordering::Release);
        VOICE_END_2[slot].store(0, Ordering::Release);
    }
    VOICE_TICKS[slot].store(0, Ordering::Release);
    VOICE_PHASE[slot].store(0, Ordering::Release);
    VOICE_HAS_CALLBACK[slot].store(request.callback.is_some(), Ordering::Release);
    VOICE_CALLBACK[slot].store(
        request.callback.map_or(0, |cb| cb as *const () as usize),
        Ordering::Release,
    );
    VOICE_CALLBACK_ACTIVE[slot].store(false, Ordering::Release);
    SND_OK
}

pub fn a_note2_freq(note: i32, freq_base: i32, note_base: i32) -> i32 {
    const TAB_PIANO_FRAC: [u32; 12] = [
        10_000, 10_594, 11_224, 11_891, 12_598, 13_346, 14_140, 14_980, 15_871, 16_814, 17_813,
        18_872,
    ];

    let mut freq = freq_base;
    let octave_delta = (note / 12) - (note_base / 12);
    if octave_delta >= 0 {
        freq = freq.checked_shl(octave_delta as u32).unwrap_or(0);
    } else {
        freq = freq.checked_shr((-octave_delta) as u32).unwrap_or(0);
    }

    let note_idx = note.rem_euclid(12) as usize;
    let base_idx = note_base.rem_euclid(12) as usize;
    let numerator = i64::from(freq) * i64::from(TAB_PIANO_FRAC[note_idx]);
    (numerator / i64::from(TAB_PIANO_FRAC[base_idx]))
        .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32
}

fn voice_ready_for_mix(slot: usize) -> bool {
    let flags = VOICE_FLAGS[slot].load(Ordering::Acquire);
    voice_active(flags)
        && (flags & VOICE_PAUSE) == 0
        && VOICE_DELAY_SAMPLES[slot].load(Ordering::Acquire) == 0
        && !voice_buffer_empty(slot)
}

fn clear_pending_voice_flags(slot: usize, mut flags: i32) -> i32 {
    if (flags & VOICE_UPDATE) != 0 {
        let clear_mask = !(VOICE_UPDATE | VOICE_VOLUPDATE | VOICE_PAUSE | VOICE_UPDATEADD);
        VOICE_FLAGS[slot].fetch_and(clear_mask, Ordering::AcqRel);
        flags &= clear_mask;
    } else if (flags & VOICE_VOLUPDATE) != 0 {
        VOICE_FLAGS[slot].fetch_and(!VOICE_VOLUPDATE, Ordering::AcqRel);
        flags &= !VOICE_VOLUPDATE;
    }

    flags
}

fn advance_voice_at_end(slot: usize, callback_invoked: &mut bool) -> bool {
    let flags = VOICE_FLAGS[slot].load(Ordering::Acquire);
    if (flags & VOICE_SETLOOP) != 0 {
        VOICE_PHASE[slot].store(0, Ordering::Release);
        return true;
    }

    if voice_has_pending_second_buffer(slot) {
        VOICE_FLAGS[slot].fetch_and(!VOICE_UPDATEADD, Ordering::AcqRel);
        return promote_pending_buffer(slot);
    }

    if VOICE_HAS_CALLBACK[slot].load(Ordering::Acquire) {
        VOICE_PTR_1[slot].store(0, Ordering::Release);
        VOICE_END_1[slot].store(0, Ordering::Release);
        VOICE_PHASE[slot].store(0, Ordering::Release);
        if claim_voice_callback(callback_invoked, true, false) {
            invoke_voice_callback(slot);
        }
        if voice_has_pending_second_buffer(slot) {
            VOICE_FLAGS[slot].fetch_and(!VOICE_UPDATEADD, Ordering::AcqRel);
            return promote_pending_buffer(slot);
        }
        return false;
    }

    deactivate_voice(slot);
    false
}

fn mix_output_buffer(output: *mut u8) -> u32 {
    if output.is_null() {
        return 0;
    }

    unsafe {
        write_bytes(output, 0, SND_BUFFERSIZE as usize);
    }

    let frames = SND_BUFFERSIZE as usize / 4;

    // libogc's DSP mixer processes one complete voice pass at a time. Voice
    // callbacks are prepared before that voice is submitted, and every voice
    // addition saturates the 16-bit output before the next voice is mixed.
    for slot in 0..MAX_SND_VOICES {
        let flags = VOICE_FLAGS[slot].load(Ordering::Acquire);
        if !voice_active(flags) {
            continue;
        }

        clear_pending_voice_flags(slot, flags);
        let mut callback_invoked = false;
        if claim_voice_callback(
            &mut callback_invoked,
            VOICE_HAS_CALLBACK[slot].load(Ordering::Acquire),
            voice_has_pending_second_buffer(slot),
        ) {
            invoke_voice_callback(slot);
        }

        if voice_buffer_empty(slot) && voice_has_pending_second_buffer(slot) {
            VOICE_FLAGS[slot].fetch_and(!VOICE_UPDATEADD, Ordering::AcqRel);
            let _ = promote_pending_buffer(slot);
        }

        for frame in 0..frames {
            let flags = VOICE_FLAGS[slot].load(Ordering::Acquire);
            if !voice_active(flags) {
                break;
            }

            let delay = VOICE_DELAY_SAMPLES[slot].load(Ordering::Acquire);
            if delay > 0 {
                VOICE_DELAY_SAMPLES[slot].store(delay.saturating_sub(1), Ordering::Release);
                continue;
            }
            if (flags & VOICE_PAUSE) != 0 {
                continue;
            }

            let mut frames_in_buffer = voice_frame_count(slot);
            if frames_in_buffer == 0 {
                continue;
            }

            let mut phase = VOICE_PHASE[slot].load(Ordering::Acquire);
            let step = voice_pitch_step(VOICE_PITCH[slot].load(Ordering::Acquire));
            let mut frame_index = (phase >> 16) as usize;

            if frame_index >= frames_in_buffer {
                if !advance_voice_at_end(slot, &mut callback_invoked) {
                    break;
                }
                phase = 0;
                frame_index = 0;
                frames_in_buffer = voice_frame_count(slot);
                if frames_in_buffer == 0 {
                    continue;
                }
            }

            let (sample_l, sample_r) = read_voice_frame(slot, frame_index);
            let volume_l = VOICE_VOLUME_L[slot].load(Ordering::Acquire).clamp(0, 255);
            let volume_r = VOICE_VOLUME_R[slot].load(Ordering::Acquire).clamp(0, 255);
            let add_l = scale_sample(sample_l, volume_l);
            let add_r = scale_sample(sample_r, volume_r);
            let offset = frame * 4;

            unsafe {
                let current_l =
                    i16::from_be_bytes([output.add(offset).read(), output.add(offset + 1).read()]);
                let current_r = i16::from_be_bytes([
                    output.add(offset + 2).read(),
                    output.add(offset + 3).read(),
                ]);
                let mixed_l = saturating_voice_add(current_l, add_l).to_be_bytes();
                let mixed_r = saturating_voice_add(current_r, add_r).to_be_bytes();
                output.add(offset).write(mixed_l[0]);
                output.add(offset + 1).write(mixed_l[1]);
                output.add(offset + 2).write(mixed_r[0]);
                output.add(offset + 3).write(mixed_r[1]);
            }

            phase = phase.wrapping_add(step);
            if (phase >> 16) as usize >= frames_in_buffer {
                let _ = advance_voice_at_end(slot, &mut callback_invoked);
                phase = 0;
            }
            VOICE_PHASE[slot].store(phase, Ordering::Release);
        }
    }

    unsafe {
        dc_flush_range(output as *const c_void, SND_BUFFERSIZE);
    }
    SND_BUFFERSIZE
}

fn saturating_voice_add(current: i16, addition: i32) -> i16 {
    (i32::from(current) + addition).clamp(i32::from(i16::MIN), i32::from(i16::MAX)) as i16
}

fn scale_sample(sample: i16, volume: i32) -> i32 {
    // The signed 16-bit sample times the masked 8-bit volume fits in i32.
    // Signed right shift matches the DSP's arithmetic shift, including for
    // negative products where it rounds toward negative infinity.
    (i32::from(sample) * volume.clamp(0, 255)) >> 8
}

unsafe extern "C" fn audio_dma_callback() {
    let start = now_ticks();
    let next = ASND_CURRENT_BUFFER.fetch_xor(1, Ordering::AcqRel) ^ 1;
    let paused = ASND_PAUSED.load(Ordering::Acquire) != 0;
    let output = if paused {
        mute_buffer_ptr()
    } else {
        current_audio_buffer_ptr(next)
    };

    if paused {
        unsafe {
            write_bytes(output, 0, SND_BUFFERSIZE as usize);
            dc_flush_range(output as *const c_void, SND_BUFFERSIZE);
            audio::audio_init_dma(
                mem_virtual_to_physical(output as *const c_void),
                SND_BUFFERSIZE,
            );
        }
        ASND_DSP_PROCESS_TIME.store(0, Ordering::Release);
        ASND_DSP_PERCENT.store(0, Ordering::Release);
        return;
    }

    if ASND_CALLBACK_SET.load(Ordering::Acquire)
        && ASND_CALLBACK_ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    {
        let callback = ASND_GLOBAL_CALLBACK.load(Ordering::Acquire);
        if callback != 0 {
            unsafe {
                (core::mem::transmute::<usize, unsafe extern "C" fn()>(callback))();
            }
        }
        ASND_CALLBACK_ACTIVE.store(false, Ordering::Release);
    }

    ASND_GLOBAL_COUNTER.fetch_add(1, Ordering::AcqRel);
    for (slot, ticks) in VOICE_TICKS.iter().enumerate() {
        if voice_ready_for_mix(slot) {
            ticks.fetch_add(1, Ordering::AcqRel);
        }
    }

    let filled = mix_output_buffer(output);
    let elapsed = now_ticks().wrapping_sub(start);
    let elapsed_us = ((elapsed as u128 * 1_000_000u128) / TICKS_PER_SECOND as u128) as u32;
    let elapsed_ns = ticks_to_nanoseconds(elapsed);
    let window_us = ((SAMPLES_PER_TICK as u128 * 1_000_000u128) / AUDIO_RATE as u128) as u64;
    let percent = (elapsed_us as u64 * 100)
        .checked_div(window_us)
        .unwrap_or(0) as u32;

    ASND_DSP_PROCESS_TIME.store(elapsed_ns, Ordering::Release);
    ASND_DSP_PERCENT.store(percent, Ordering::Release);

    audio::audio_init_dma(mem_virtual_to_physical(output as *const c_void), filled);
}

pub fn init() {
    // C ASND_Init initializes the DSP before it starts AI DMA. This also
    // resets and unhalts the DSP interrupt path used by audio playback.
    ogc_dsp_sys::init();
    let was_inited = ASND_INITED.swap(true, Ordering::AcqRel);
    if !was_inited {
        reset_runtime_state();
        ASND_PAUSED.store(1, Ordering::Release);
    }
    audio::audio_init(core::ptr::null_mut());
    audio::audio_stop_dma();
    audio::audio_set_dsp_sample_rate(audio::AI_SAMPLERATE_48KHZ as u8);
    audio::audio_register_dma_callback(Some(audio_dma_callback));
    audio::audio_init_dma(
        mem_virtual_to_physical(current_audio_buffer_ptr(
            ASND_CURRENT_BUFFER.load(Ordering::Acquire),
        ) as *const c_void),
        SND_BUFFERSIZE,
    );
    audio::audio_start_dma();
}

pub fn end() {
    audio::audio_stop_dma();
    audio::audio_register_dma_callback(None);
    // C only tears down the DMA/DSP plumbing here. Observable ASND state is
    // retained until the next first-time init, which performs the reset.
    ASND_INITED.store(false, Ordering::Release);
}

pub fn pause(paused: i32) {
    ASND_PAUSED.store(paused, Ordering::Release);
}

pub fn is_paused() -> i32 {
    ASND_PAUSED.load(Ordering::Acquire)
}

pub fn get_time() -> u32 {
    ASND_GLOBAL_COUNTER
        .load(Ordering::Acquire)
        .wrapping_mul(SND_BUFFERSIZE)
        / 4
        / 48
}

pub fn get_sample_counter() -> u32 {
    ASND_GLOBAL_COUNTER
        .load(Ordering::Acquire)
        .wrapping_mul(SND_BUFFERSIZE)
        / 4
}

pub fn get_samples_per_tick() -> u32 {
    SAMPLES_PER_TICK
}

pub fn set_time(time: u32) {
    ASND_GLOBAL_COUNTER.store(48u32.wrapping_mul(time), Ordering::Release);
}

pub fn set_callback(callback: AsndCallback) {
    ASND_CALLBACK_SET.store(callback.is_some(), Ordering::Release);
    ASND_GLOBAL_CALLBACK.store(
        callback.map_or(0, |cb| cb as *const () as usize),
        Ordering::Release,
    );
}

pub fn get_audio_rate() -> i32 {
    AUDIO_RATE
}

/// # Safety
///
/// `config.snd` must remain readable for `config.size_snd` bytes until playback
/// completes or the voice is stopped. It must also satisfy ASND's 32-byte
/// padding and alignment contract. The callback can run from audio interrupt
/// context and must obey the platform's callback restrictions.
pub unsafe fn set_voice(config: VoiceConfig) -> i32 {
    set_voice_inner(VoiceRequest {
        voice: config.voice,
        format: config.format,
        pitch: config.pitch,
        delay: config.delay,
        snd: config.snd,
        size_snd: config.size_snd,
        volume_l: config.volume_l,
        volume_r: config.volume_r,
        callback: config.callback,
        infinite: false,
    })
}

/// # Safety
///
/// `config.snd` must remain readable for `config.size_snd` bytes until the
/// voice is stopped. It must also satisfy ASND's 32-byte padding and alignment
/// contract.
pub unsafe fn set_infinite_voice(config: InfiniteVoiceConfig) -> i32 {
    set_voice_inner(VoiceRequest {
        voice: config.voice,
        format: config.format,
        pitch: config.pitch,
        delay: config.delay,
        snd: config.snd,
        size_snd: config.size_snd,
        volume_l: config.volume_l,
        volume_r: config.volume_r,
        callback: None,
        infinite: true,
    })
}

/// # Safety
///
/// `snd` must point to a second buffer with the same format as the voice's
/// first buffer and must satisfy ASND's hardware alignment rules.
pub unsafe fn add_voice(voice: i32, snd: *mut c_void, size_snd: i32) -> i32 {
    unsafe {
        let Some(slot) = voice_index(voice) else {
            return SND_INVALID;
        };
        let flags = VOICE_FLAGS[slot].load(Ordering::Acquire);
        if !valid_voice_buffer(snd.cast_const(), size_snd)
            || (flags & (VOICE_UPDATE | VOICE_UPDATEADD)) != 0
            || !voice_active(flags)
        {
            return SND_INVALID;
        }
        if VOICE_PTR_2[slot].load(Ordering::Acquire) != 0 {
            return SND_BUSY;
        }
        let start = mem_virtual_to_physical(snd.cast_const()) as usize;
        dc_flush_range(snd.cast_const(), size_snd as u32);
        VOICE_PTR_2[slot].store(start, Ordering::Release);
        VOICE_END_2[slot].store(start.wrapping_add(size_snd as usize), Ordering::Release);
        VOICE_FLAGS[slot].store(
            (flags & !VOICE_SETLOOP) | VOICE_UPDATEADD,
            Ordering::Release,
        );
        SND_OK
    }
}

/// Starts a finite ASND voice while transferring ownership of its sample data.
pub fn set_voice_owned(
    config: OwnedVoiceConfig,
    mut buffer: OwnedVoiceBuffer,
) -> Result<OwnedVoice, OwnedVoiceError> {
    let (callback, owned_callback) = if let Some(callback) = config.callback {
        let Some(slot) = voice_index(config.voice) else {
            return Err(OwnedVoiceError {
                status: SND_INVALID,
                buffer,
            });
        };
        let previous =
            OWNED_VOICE_CALLBACK[slot].swap(callback as *const () as usize, Ordering::AcqRel);
        (
            Some(owned_voice_callback as unsafe extern "C" fn(i32)),
            Some((slot, previous)),
        )
    } else {
        (None, None)
    };
    // SAFETY: the returned `OwnedVoice` retains `buffer` until it stops the
    // voice. On failure ASND does not retain the pointer and the buffer is
    // returned in `OwnedVoiceError`.
    let status = unsafe {
        set_voice(VoiceConfig {
            voice: config.voice,
            format: config.format,
            pitch: config.pitch,
            delay: config.delay,
            snd: buffer.as_mut_ptr(),
            size_snd: buffer.len(),
            volume_l: config.volume_l,
            volume_r: config.volume_r,
            callback,
        })
    };

    if status == SND_OK {
        Ok(OwnedVoice {
            voice: config.voice,
            buffers: alloc::vec![buffer],
            active: true,
        })
    } else {
        if let Some((slot, previous)) = owned_callback {
            OWNED_VOICE_CALLBACK[slot].store(previous, Ordering::Release);
        }
        Err(OwnedVoiceError { status, buffer })
    }
}

/// Starts a looping ASND voice while transferring ownership of its sample data.
pub fn set_infinite_voice_owned(
    config: OwnedInfiniteVoiceConfig,
    mut buffer: OwnedVoiceBuffer,
) -> Result<OwnedVoice, OwnedVoiceError> {
    // SAFETY: the returned `OwnedVoice` retains `buffer` until it stops the
    // voice. On failure ASND does not retain the pointer and the buffer is
    // returned in `OwnedVoiceError`.
    let status = unsafe {
        set_infinite_voice(InfiniteVoiceConfig {
            voice: config.voice,
            format: config.format,
            pitch: config.pitch,
            delay: config.delay,
            snd: buffer.as_mut_ptr(),
            size_snd: buffer.len(),
            volume_l: config.volume_l,
            volume_r: config.volume_r,
        })
    };

    if status == SND_OK {
        Ok(OwnedVoice {
            voice: config.voice,
            buffers: alloc::vec![buffer],
            active: true,
        })
    } else {
        Err(OwnedVoiceError { status, buffer })
    }
}

pub fn stop_voice(voice: i32) -> i32 {
    let Some(slot) = voice_index(voice) else {
        return SND_INVALID;
    };
    deactivate_voice(slot);
    SND_OK
}

pub fn pause_voice(voice: i32, pause: i32) -> i32 {
    let Some(slot) = voice_index(voice) else {
        return SND_INVALID;
    };
    let flags = VOICE_FLAGS[slot].load(Ordering::Acquire);
    let next = if pause != 0 {
        flags | VOICE_PAUSE
    } else {
        flags & !VOICE_PAUSE
    };
    VOICE_FLAGS[slot].store(next, Ordering::Release);
    SND_OK
}

pub fn status_voice(voice: i32) -> i32 {
    let Some(slot) = voice_index(voice) else {
        return SND_INVALID;
    };
    let flags = VOICE_FLAGS[slot].load(Ordering::Acquire);
    if (flags & VOICE_PAUSE) != 0 {
        SND_WAITING
    } else if !voice_active(flags) {
        SND_UNUSED
    } else {
        SND_WORKING
    }
}

pub fn get_first_unused_voice() -> i32 {
    for (slot, flags) in VOICE_FLAGS.iter().enumerate().skip(1) {
        if !voice_active(flags.load(Ordering::Acquire)) {
            return slot as i32;
        }
    }
    if !voice_active(VOICE_FLAGS[0].load(Ordering::Acquire)) {
        0
    } else {
        SND_INVALID
    }
}

pub fn change_pitch_voice(voice: i32, pitch: i32) -> i32 {
    let Some(slot) = voice_index(voice) else {
        return SND_INVALID;
    };
    VOICE_PITCH[slot].store(pitch.clamp(MIN_PITCH, MAX_PITCH), Ordering::Release);
    SND_OK
}

pub fn change_volume_voice(voice: i32, volume_l: i32, volume_r: i32) -> i32 {
    let Some(slot) = voice_index(voice) else {
        return SND_INVALID;
    };
    let flags = VOICE_FLAGS[slot].load(Ordering::Acquire);
    VOICE_FLAGS[slot].store(flags | VOICE_VOLUPDATE, Ordering::Release);
    VOICE_VOLUME_L[slot].store(volume_l & 255, Ordering::Release);
    VOICE_VOLUME_R[slot].store(volume_r & 255, Ordering::Release);
    SND_OK
}

pub fn get_tick_counter_voice(voice: i32) -> u32 {
    voice_index(voice).map_or(0, |slot| {
        VOICE_TICKS[slot]
            .load(Ordering::Acquire)
            .wrapping_mul(SND_BUFFERSIZE)
            / 4
    })
}

pub fn get_timer_voice(voice: i32) -> u32 {
    get_tick_counter_voice(voice) / 48
}

pub fn test_pointer(voice: i32, pointer: *mut c_void) -> i32 {
    let Some(slot) = voice_index(voice) else {
        return SND_INVALID;
    };
    let ptr = mem_virtual_to_physical(pointer.cast_const()) as usize;
    if ptr != 0
        && (VOICE_BACKUP_PTR[slot].load(Ordering::Acquire) == ptr
            || VOICE_PTR_1[slot].load(Ordering::Acquire) == ptr
            || VOICE_PTR_2[slot].load(Ordering::Acquire) == ptr)
    {
        SND_BUSY
    } else {
        SND_OK
    }
}

pub fn test_voice_buffer_ready(voice: i32) -> i32 {
    let Some(slot) = voice_index(voice) else {
        return 0;
    };
    i32::from(
        !(VOICE_PTR_1[slot].load(Ordering::Acquire) != 0
            && VOICE_PTR_2[slot].load(Ordering::Acquire) != 0),
    )
}

pub fn get_dsp_percent_use() -> u32 {
    ASND_DSP_PERCENT.load(Ordering::Acquire)
}

pub fn get_dsp_process_time() -> u32 {
    ASND_DSP_PROCESS_TIME.load(Ordering::Acquire)
}

pub const fn note(note: i32, octave: i32) -> i32 {
    note_code(note, octave)
}

#[cfg(all(test, target_arch = "powerpc"))]
fn host_audio_tick() {
    if ASND_PAUSED.load(Ordering::Acquire) == 0 {
        ASND_GLOBAL_COUNTER.fetch_add(1, Ordering::AcqRel);
    }

    for slot in 0..MAX_SND_VOICES {
        let flags = VOICE_FLAGS[slot].load(Ordering::Acquire);
        if !voice_active(flags) {
            continue;
        }

        let next_flags = flags & !(VOICE_UPDATE | VOICE_UPDATEADD | VOICE_VOLUPDATE);
        VOICE_FLAGS[slot].store(next_flags, Ordering::Release);

        let delay = VOICE_DELAY_SAMPLES[slot].load(Ordering::Acquire);
        if delay > 0 {
            VOICE_DELAY_SAMPLES[slot]
                .store(delay.saturating_sub(SAMPLES_PER_TICK), Ordering::Release);
            continue;
        }

        if (next_flags & VOICE_PAUSE) == 0
            && (VOICE_PTR_1[slot].load(Ordering::Acquire) != 0
                || VOICE_PTR_2[slot].load(Ordering::Acquire) != 0)
        {
            VOICE_TICKS[slot].fetch_add(1, Ordering::AcqRel);
        }
    }
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;
    use core::sync::atomic::AtomicBool;

    unsafe extern "C" fn voice_callback(_voice: i32) {}
    unsafe extern "C" fn global_callback() {}

    static TEST_LOCK: AtomicBool = AtomicBool::new(false);

    struct TestGuard;

    impl Drop for TestGuard {
        fn drop(&mut self) {
            TEST_LOCK.store(false, Ordering::Release);
        }
    }

    fn test_guard() -> TestGuard {
        while TEST_LOCK
            .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
            .is_err()
        {
            core::hint::spin_loop();
        }
        end();
        init();
        TestGuard
    }

    #[test]
    fn sample_volume_scaling_matches_signed_dsp_shift() {
        let cases = [
            (1_000, 0, 0),
            (1_000, 1, 3),
            (1_000, 128, 500),
            (1_000, 255, 996),
            (-1_000, 0, 0),
            (-1_000, 1, -4),
            (-1_000, 128, -500),
            (-1_000, 255, -997),
            (i16::MAX, 255, 32_639),
            (i16::MIN, 255, -32_640),
        ];
        for (sample, volume, expected) in cases {
            assert_eq!(scale_sample(sample as i16, volume), expected);
        }
    }

    #[test]
    fn per_voice_saturation_matches_dsp_accumulation_order() {
        assert_eq!(saturating_voice_add(30_000, 10_000), i16::MAX);
        assert_eq!(saturating_voice_add(i16::MAX, -10_000), 22_767);
        assert_eq!(saturating_voice_add(-30_000, -10_000), i16::MIN);
        assert_eq!(saturating_voice_add(i16::MIN, 10_000), -22_768);

        // Aggregate clamping would produce 30_000 here. The DSP saturates the
        // first positive addition, then applies the opposing voice.
        let after_first = saturating_voice_add(30_000, 10_000);
        assert_eq!(saturating_voice_add(after_first, -10_000), 22_767);
    }

    #[test]
    fn voice_callback_is_claimed_once_per_mix_buffer_unless_a_second_buffer_is_pending() {
        let mut invoked = false;
        assert!(claim_voice_callback(&mut invoked, true, false));
        assert!(!claim_voice_callback(&mut invoked, true, false));

        let mut next_buffer_pending = false;
        assert!(!claim_voice_callback(&mut next_buffer_pending, true, true));
        assert!(claim_voice_callback(&mut next_buffer_pending, true, false));

        let mut no_callback = false;
        assert!(!claim_voice_callback(&mut no_callback, false, false));
    }

    #[test]
    fn constants_match_asnd_header() {
        let _guard = test_guard();
        assert_eq!(ASND_LIB, 0x100);
        assert_eq!(SND_LIB, 0x102);
        assert_eq!(MAX_SND_VOICES, 16);
        assert_eq!(SND_OK, 0);
        assert_eq!(SND_INVALID, -1);
        assert_eq!(SND_BUSY, 1);
        assert_eq!(SND_UNUSED, 0);
        assert_eq!(SND_WORKING, 1);
        assert_eq!(SND_WAITING, 2);
        assert_eq!(VOICE_STEREO_16BIT_LE, 7);
        assert_eq!(MIN_VOLUME, 0);
        assert_eq!(MID_VOLUME, 127);
        assert_eq!(MAX_VOLUME, 255);
        assert_eq!(MIN_PITCH, 1);
        assert_eq!(F44100HZ_PITCH, 44_100);
        assert_eq!(MAX_PITCH, 144_000);
        assert_eq!(note(NOTE_C, 4), 48);
        assert_eq!(NOTE_LA, NOTE_A);
        assert_eq!(NOTE_SIB, NOTE_BB);
    }

    #[test]
    fn api_exports_are_available() {
        let _guard = test_guard();
        let _ = a_note2_freq as fn(i32, i32, i32) -> i32;
        let _ = init as fn();
        let _ = end as fn();
        let _ = pause as fn(i32);
        let _ = is_paused as fn() -> i32;
        let _ = get_time as fn() -> u32;
        let _ = get_sample_counter as fn() -> u32;
        let _ = get_samples_per_tick as fn() -> u32;
        let _ = set_time as fn(u32);
        let _ = set_callback as fn(AsndCallback);
        let _ = get_audio_rate as fn() -> i32;
        let _ = set_voice as unsafe fn(VoiceConfig) -> i32;
        let _ = set_infinite_voice as unsafe fn(InfiniteVoiceConfig) -> i32;
        let _ = add_voice as unsafe fn(i32, *mut c_void, i32) -> i32;
        let _ = set_voice_owned
            as fn(OwnedVoiceConfig, OwnedVoiceBuffer) -> Result<OwnedVoice, OwnedVoiceError>;
        let _ = set_infinite_voice_owned
            as fn(
                OwnedInfiniteVoiceConfig,
                OwnedVoiceBuffer,
            ) -> Result<OwnedVoice, OwnedVoiceError>;
        let _ = stop_voice as fn(i32) -> i32;
        let _ = pause_voice as fn(i32, i32) -> i32;
        let _ = status_voice as fn(i32) -> i32;
        let _ = get_first_unused_voice as fn() -> i32;
        let _ = change_pitch_voice as fn(i32, i32) -> i32;
        let _ = change_volume_voice as fn(i32, i32, i32) -> i32;
        let _ = get_tick_counter_voice as fn(i32) -> u32;
        let _ = get_timer_voice as fn(i32) -> u32;
        let _ = test_pointer as fn(i32, *mut c_void) -> i32;
        let _ = test_voice_buffer_ready as fn(i32) -> i32;
        let _ = get_dsp_percent_use as fn() -> u32;
        let _ = get_dsp_process_time as fn() -> u32;
    }

    #[test]
    fn init_pause_time_and_callback_state_are_host_compat() {
        let _guard = test_guard();
        assert_eq!(is_paused(), 1);
        pause(0);
        assert_eq!(is_paused(), 0);
        host_audio_tick();
        assert_eq!(get_sample_counter(), 1024);
        assert_eq!(get_time(), 21);
        set_time(10);
        assert_eq!(get_sample_counter(), 491_520);
        assert_eq!(get_time(), 10_240);
        assert_eq!(get_audio_rate(), 48_000);
        assert_eq!(get_samples_per_tick(), 1024);
        set_callback(Some(global_callback));
        assert!(ASND_CALLBACK_SET.load(Ordering::Acquire));
        assert_eq!(
            ASND_GLOBAL_CALLBACK.load(Ordering::Acquire),
            global_callback as *const () as usize
        );
        set_callback(None);
        assert_eq!(ASND_GLOBAL_CALLBACK.load(Ordering::Acquire), 0);
        end();
        assert_eq!(is_paused(), 1);
    }

    #[test]
    fn pause_preserves_the_c_api_integer_value() {
        let _guard = test_guard();
        pause(2);
        assert_eq!(is_paused(), 2);
        pause(-1);
        assert_eq!(is_paused(), -1);
    }

    #[test]
    fn time_counters_wrap_like_the_c_unsigned_arithmetic() {
        let _guard = test_guard();
        ASND_GLOBAL_COUNTER.store(u32::MAX, Ordering::Release);
        VOICE_TICKS[3].store(u32::MAX, Ordering::Release);
        let expected_samples = u32::MAX.wrapping_mul(SND_BUFFERSIZE) / 4;
        assert_eq!(get_sample_counter(), expected_samples);
        assert_eq!(get_time(), expected_samples / 48);
        assert_eq!(get_tick_counter_voice(3), expected_samples);
        assert_eq!(get_timer_voice(3), expected_samples / 48);

        set_time(u32::MAX);
        let expected_global_counter = 48u32.wrapping_mul(u32::MAX);
        assert_eq!(
            ASND_GLOBAL_COUNTER.load(Ordering::Acquire),
            expected_global_counter
        );
        assert_eq!(
            get_sample_counter(),
            expected_global_counter.wrapping_mul(SND_BUFFERSIZE) / 4
        );
    }

    #[test]
    fn voice_lifecycle_and_double_buffer_state_are_host_compat() {
        let _guard = test_guard();
        let mut first = [0u8; 64];
        let mut second = [0u8; 64];
        unsafe {
            assert_eq!(
                set_voice(VoiceConfig {
                    voice: 3,
                    format: VOICE_STEREO_16BIT_LE + 8,
                    pitch: 0,
                    delay: 0,
                    snd: first.as_mut_ptr().cast(),
                    size_snd: first.len() as i32,
                    volume_l: 300,
                    volume_r: -10,
                    callback: Some(voice_callback),
                }),
                SND_OK
            );
        }
        assert_eq!(status_voice(3), SND_WORKING);
        assert_ne!(VOICE_FLAGS[3].load(Ordering::Acquire) & VOICE_UPDATE, 0);
        assert_eq!(
            VOICE_FORMAT[3].load(Ordering::Acquire),
            VOICE_STEREO_16BIT_LE
        );
        assert_eq!(VOICE_PITCH[3].load(Ordering::Acquire), MIN_PITCH);
        assert_eq!(VOICE_VOLUME_L[3].load(Ordering::Acquire), 44);
        assert_eq!(VOICE_VOLUME_R[3].load(Ordering::Acquire), 246);
        assert_eq!(test_pointer(3, first.as_mut_ptr().cast()), SND_BUSY);
        assert_eq!(test_voice_buffer_ready(3), 1);
        assert_eq!(
            VOICE_BACKUP_PTR[3].load(Ordering::Acquire),
            first.as_ptr() as usize
        );
        let alias = 0xfeed_cafeusize as *mut c_void;
        VOICE_BACKUP_PTR[3].store(alias as usize, Ordering::Release);
        assert_eq!(test_pointer(3, alias), SND_BUSY);
        VOICE_BACKUP_PTR[3].store(first.as_ptr() as usize, Ordering::Release);

        unsafe {
            assert_eq!(
                add_voice(3, second.as_mut_ptr().cast(), second.len() as i32),
                SND_INVALID
            );
        }
        host_audio_tick();
        unsafe {
            assert_eq!(
                add_voice(3, second.as_mut_ptr().cast(), second.len() as i32),
                SND_OK
            );
        }
        assert_ne!(VOICE_FLAGS[3].load(Ordering::Acquire) & VOICE_UPDATEADD, 0);
        assert_eq!(test_pointer(3, second.as_mut_ptr().cast()), SND_BUSY);
        assert_eq!(test_voice_buffer_ready(3), 0);
        unsafe {
            assert_eq!(
                add_voice(3, second.as_mut_ptr().cast(), second.len() as i32),
                SND_INVALID
            );
        }
        host_audio_tick();
        unsafe {
            assert_eq!(
                add_voice(3, second.as_mut_ptr().cast(), second.len() as i32),
                SND_BUSY
            );
        }

        assert_eq!(change_pitch_voice(3, MAX_PITCH + 1), SND_OK);
        assert_eq!(VOICE_PITCH[3].load(Ordering::Acquire), MAX_PITCH);
        assert_eq!(change_volume_voice(3, 320, -1), SND_OK);
        assert_ne!(VOICE_FLAGS[3].load(Ordering::Acquire) & VOICE_VOLUPDATE, 0);
        assert_eq!(VOICE_VOLUME_L[3].load(Ordering::Acquire), 64);
        assert_eq!(VOICE_VOLUME_R[3].load(Ordering::Acquire), 255);
        assert_eq!(pause_voice(3, 1), SND_OK);
        assert_eq!(status_voice(3), SND_WAITING);
        let ticks_before = get_tick_counter_voice(3);
        host_audio_tick();
        assert_eq!(get_tick_counter_voice(3), ticks_before);
        assert_eq!(pause_voice(3, 0), SND_OK);
        host_audio_tick();
        assert_eq!(get_tick_counter_voice(3), ticks_before + SAMPLES_PER_TICK);
        let ticks_at_stop = get_tick_counter_voice(3);
        assert_ne!(ticks_at_stop, 0);
        assert_eq!(stop_voice(3), SND_OK);
        assert_eq!(status_voice(3), SND_UNUSED);
        assert_eq!(get_tick_counter_voice(3), ticks_at_stop);
    }

    #[test]
    fn promoted_buffer_replaces_pointer_checked_by_test_pointer() {
        let _guard = test_guard();
        let mut first = [0u8; 64];
        let mut second = [0u8; 64];
        unsafe {
            assert_eq!(
                set_voice(VoiceConfig {
                    voice: 3,
                    format: VOICE_STEREO_16BIT_LE,
                    pitch: 1,
                    delay: 0,
                    snd: first.as_mut_ptr().cast(),
                    size_snd: first.len() as i32,
                    volume_l: 255,
                    volume_r: 255,
                    callback: None,
                }),
                SND_OK
            );
        }
        host_audio_tick();
        unsafe {
            assert_eq!(
                add_voice(3, second.as_mut_ptr().cast(), second.len() as i32),
                SND_OK
            );
        }

        assert!(promote_pending_buffer(3));
        assert_eq!(test_pointer(3, first.as_mut_ptr().cast()), SND_OK);
        assert_eq!(test_pointer(3, second.as_mut_ptr().cast()), SND_BUSY);
    }

    #[test]
    fn natural_voice_completion_preserves_tick_counter() {
        let _guard = test_guard();
        let mut samples = [0u8; (SAMPLES_PER_TICK as usize * 2 + 1) * 4];
        unsafe {
            assert_eq!(
                set_voice(VoiceConfig {
                    voice: 3,
                    format: VOICE_STEREO_16BIT_LE,
                    pitch: AUDIO_RATE,
                    delay: 0,
                    snd: samples.as_mut_ptr().cast(),
                    size_snd: samples.len() as i32,
                    volume_l: 255,
                    volume_r: 255,
                    callback: None,
                }),
                SND_OK
            );
        }

        host_audio_tick();
        let ticks_before_completion = get_tick_counter_voice(3);
        assert_ne!(ticks_before_completion, 0);

        host_audio_tick();
        assert_eq!(status_voice(3), SND_UNUSED);
        assert_eq!(get_tick_counter_voice(3), ticks_before_completion);
    }

    #[test]
    fn invalid_voice_inputs_return_c_style_errors() {
        let _guard = test_guard();
        let mut buf = [0u8; 32];
        unsafe {
            assert_eq!(
                set_voice(VoiceConfig {
                    voice: -1,
                    format: VOICE_MONO_8BIT,
                    pitch: 8_000,
                    delay: 0,
                    snd: core::ptr::null_mut(),
                    size_snd: 0,
                    volume_l: 1,
                    volume_r: 1,
                    callback: None,
                }),
                SND_INVALID
            );
            assert_eq!(
                set_voice(VoiceConfig {
                    voice: 0,
                    format: VOICE_STEREO_16BIT_LE + 1,
                    pitch: 8_000,
                    delay: 0,
                    snd: core::ptr::null_mut(),
                    size_snd: 0,
                    volume_l: 1,
                    volume_r: 1,
                    callback: None,
                }),
                SND_INVALID
            );
            assert_eq!(
                set_voice(VoiceConfig {
                    voice: 0,
                    format: VOICE_STEREO_16BIT_LE + 1,
                    pitch: 8_000,
                    delay: -20,
                    snd: buf.as_mut_ptr().cast(),
                    size_snd: buf.len() as i32,
                    volume_l: 1,
                    volume_r: 1,
                    callback: None,
                }),
                SND_OK
            );
            assert_eq!(add_voice(0, core::ptr::null_mut(), 32), SND_INVALID);
        }
        assert_eq!(status_voice(MAX_SND_VOICES as i32), SND_INVALID);
        assert_eq!(pause_voice(0, 1), SND_OK);
        assert_eq!(change_pitch_voice(0, 0), SND_OK);
        assert_eq!(VOICE_PITCH[0].load(Ordering::Acquire), MIN_PITCH);
        assert_eq!(test_pointer(-1, core::ptr::null_mut()), SND_INVALID);
        assert_eq!(test_voice_buffer_ready(-1), 0);
    }

    #[test]
    fn note_frequency_and_dsp_defaults_are_stable() {
        let _guard = test_guard();
        assert_eq!(
            a_note2_freq(note(NOTE_A, 4), 44_100, note(NOTE_A, 4)),
            44_100
        );
        assert_eq!(
            a_note2_freq(note(NOTE_A, 5), 44_100, note(NOTE_A, 4)),
            88_200
        );
        assert_eq!(
            a_note2_freq(note(NOTE_C, 4), 44_100, note(NOTE_A, 4)),
            26_228
        );
        assert_eq!(
            a_note2_freq(note(NOTE_C, 5), 8_000, note(NOTE_C, 4)),
            16_000
        );
        assert_eq!(get_dsp_percent_use(), 0);
        assert_eq!(get_dsp_process_time(), 0);
    }

    #[test]
    fn note_table_matches_the_c_float_sequence() {
        let expected = [
            10_000, 10_594, 11_224, 11_891, 12_598, 13_346, 14_140, 14_980, 15_871, 16_814, 17_813,
            18_872,
        ];
        for (note, expected_frequency) in expected.into_iter().enumerate() {
            assert_eq!(a_note2_freq(note as i32, 10_000, 0), expected_frequency);
        }
    }

    #[test]
    fn dsp_process_time_uses_nanoseconds_like_the_c_api() {
        assert_eq!(ticks_to_nanoseconds(TICKS_PER_SECOND), 1_000_000_000);
    }

    #[test]
    fn repeated_init_preserves_live_voice_until_end() {
        let _guard = test_guard();
        let mut buf = [0x80u8; 32];
        unsafe {
            assert_eq!(
                set_voice(VoiceConfig {
                    voice: 2,
                    format: VOICE_MONO_8BIT_U,
                    pitch: 8_000,
                    delay: 0,
                    snd: buf.as_mut_ptr().cast(),
                    size_snd: buf.len() as i32,
                    volume_l: 255,
                    volume_r: 255,
                    callback: Some(voice_callback),
                }),
                SND_OK
            );
        }
        assert_eq!(status_voice(2), SND_WORKING);
        init();
        assert_eq!(status_voice(2), SND_WORKING);
        end();
        assert_eq!(status_voice(2), SND_UNUSED);
    }

    #[test]
    fn mixer_callback_path_records_progress_and_invokes_voice_callback() {
        let _guard = test_guard();
        static CALLBACK_COUNT: core::sync::atomic::AtomicU32 =
            core::sync::atomic::AtomicU32::new(0);
        CALLBACK_COUNT.store(0, Ordering::Release);

        unsafe extern "C" fn callback(_voice: i32) {
            CALLBACK_COUNT.fetch_add(1, Ordering::AcqRel);
        }

        // This source buffer lasts beyond one mixer output. C calls the stream
        // callback at the buffer pass boundary, before this source is empty.
        let mut buf = [0xffu8; 4096];
        unsafe {
            assert_eq!(
                set_voice(VoiceConfig {
                    voice: 4,
                    format: VOICE_MONO_8BIT_U,
                    pitch: 48_000,
                    delay: 0,
                    snd: buf.as_mut_ptr().cast(),
                    size_snd: buf.len() as i32,
                    volume_l: 255,
                    volume_r: 255,
                    callback: Some(callback),
                }),
                SND_OK
            );
        }
        let mut out = [0u8; SND_BUFFERSIZE as usize];
        let mixed = mix_output_buffer(out.as_mut_ptr());
        assert_eq!(mixed, SND_BUFFERSIZE);
        assert!(out.iter().any(|byte| *byte != 0));
        assert_eq!(CALLBACK_COUNT.load(Ordering::Acquire), 1);
        assert!(get_dsp_process_time() <= 1_000_000);
    }

    #[test]
    fn infinite_voice_first_unused_and_end_reset_follow_c_shape() {
        let _guard = test_guard();
        let mut buffers = [[0u8; 32]; MAX_SND_VOICES];
        assert_eq!(get_first_unused_voice(), 1);
        for (voice, buffer) in buffers.iter_mut().enumerate().skip(1) {
            unsafe {
                assert_eq!(
                    set_infinite_voice(InfiniteVoiceConfig {
                        voice: voice as i32,
                        format: VOICE_MONO_8BIT,
                        pitch: 8_000,
                        delay: 10,
                        snd: buffer.as_mut_ptr().cast(),
                        size_snd: buffer.len() as i32,
                        volume_l: 1,
                        volume_r: 2,
                    }),
                    SND_OK
                );
            }
            assert_ne!(
                VOICE_FLAGS[voice].load(Ordering::Acquire) & VOICE_SETLOOP,
                0
            );
            assert_eq!(
                VOICE_PTR_1[voice].load(Ordering::Acquire),
                VOICE_PTR_2[voice].load(Ordering::Acquire)
            );
        }
        assert_eq!(get_first_unused_voice(), 0);
        unsafe {
            assert_eq!(
                set_voice(VoiceConfig {
                    voice: 0,
                    format: VOICE_MONO_8BIT,
                    pitch: 8_000,
                    delay: 0,
                    snd: buffers[0].as_mut_ptr().cast(),
                    size_snd: buffers[0].len() as i32,
                    volume_l: 1,
                    volume_r: 2,
                    callback: None,
                }),
                SND_OK
            );
        }
        assert_eq!(get_first_unused_voice(), SND_INVALID);
        end();
        for voice in 0..MAX_SND_VOICES {
            assert_eq!(status_voice(voice as i32), SND_UNUSED);
            assert_eq!(test_voice_buffer_ready(voice as i32), 1);
        }
        assert_eq!(get_first_unused_voice(), 1);
    }
}
