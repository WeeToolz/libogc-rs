#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::cell::UnsafeCell;
use core::ffi::c_void;
use core::ptr::null_mut;

use ogc_audio::{self, AI_SAMPLERATE_48KHZ};
use ogc_cache::dc_flush_range;

mod native_dsp;
mod process_cycle;
use ogc_lwp_sys as lwp;
pub use ogc_sys::{Bool, F32};
use process_cycle::ProcessCycle;

pub const MAX_VOICES: u32 = 32;
pub const SND_BUFFERSIZE: u32 = 384;
pub const DSP_STREAMBUFFER_SIZE: u32 = 1152;
#[cfg(gamecube)]
pub const DSP_DEFAULT_FREQ: F32 = 54_000_000.0 / 1124.0;
#[cfg(not(gamecube))]
pub const DSP_DEFAULT_FREQ: F32 = 48_000.0;

pub const VOICE_STATE_STOPPED: u32 = 0;
pub const VOICE_STATE_RUNNING: u32 = 1;
pub const VOICE_STATE_STREAM: u32 = 2;

pub const VOICE_MONO8: u32 = 0x0000_0000;
pub const VOICE_STEREO8: u32 = 0x0000_0001;
pub const VOICE_MONO16: u32 = 0x0000_0002;
pub const VOICE_STEREO16: u32 = 0x0000_0003;
pub const VOICE_MONO8_UNSIGNED: u32 = 0x0000_0004;
pub const VOICE_STEREO8_UNSIGNED: u32 = 0x0000_0005;
pub const VOICE_MONO16_UNSIGNED: u32 = 0x0000_0006;
pub const VOICE_STEREO16_UNSIGNED: u32 = 0x0000_0007;

pub const VOICE_FREQ32KHZ: u32 = 32_000;
pub const VOICE_FREQ48KHZ: u32 = 48_000;

const VOICE_PAUSE: u32 = 0x0000_0008;
const VOICE_LOOP: u32 = 0x0000_0010;
const VOICE_ONCE: u32 = 0x0000_0020;
const VOICE_STREAM: u32 = 0x0000_0040;
const VOICE_FINISHED: u32 = 0x0010_0000;
const VOICE_STOPPED: u32 = 0x0020_0000;
const VOICE_RUNNING: u32 = 0x4000_0000;
const VOICE_USED: u32 = 0x8000_0000;

const SYS_BASE_UNCACHED: u32 = 0xC000_0000;

#[repr(C)]
#[derive(Copy, Clone)]
pub struct Aesndpb {
    out_buf: u32,
    buf_start: u32,
    buf_end: u32,
    buf_curr: u32,
    yn1: u16,
    yn2: u16,
    pds: u16,
    freq_h: u16,
    freq_l: u16,
    counter: u16,
    left: i16,
    right: i16,
    volume_l: u16,
    volume_r: u16,
    delay: u32,
    flags: u32,
    pad: [u8; 20],
    mram_start: u32,
    mram_curr: u32,
    mram_end: u32,
    stream_last: u32,
    voiceno: u32,
    shift: u32,
    cb: AesndVoiceCallbackArg,
    callback_arg: *mut c_void,
    audio_callback: AesndAudioCallbackArg,
    audio_callback_arg: *mut c_void,
}

const _: () = {
    assert!(core::mem::offset_of!(Aesndpb, out_buf) == 0);
    assert!(core::mem::offset_of!(Aesndpb, buf_curr) == 12);
    assert!(core::mem::offset_of!(Aesndpb, flags) == 40);
    assert!(core::mem::offset_of!(Aesndpb, mram_start) == 64);
};

// SAFETY: `Aesndpb` is only stored inside interrupt-protected AESND state.
// Its raw pointers are opaque callback contexts governed by the registration
// APIs' safety contracts.
unsafe impl Send for Aesndpb {}

impl Aesndpb {
    const fn zeroed() -> Self {
        Self {
            out_buf: 0,
            buf_start: 0,
            buf_end: 0,
            buf_curr: 0,
            yn1: 0,
            yn2: 0,
            pds: 0,
            freq_h: 0,
            freq_l: 0,
            counter: 0,
            left: 0,
            right: 0,
            volume_l: 0,
            volume_r: 0,
            delay: 0,
            flags: 0,
            pad: [0; 20],
            mram_start: 0,
            mram_curr: 0,
            mram_end: 0,
            stream_last: 0,
            voiceno: 0,
            shift: 0,
            cb: None,
            callback_arg: null_mut(),
            audio_callback: None,
            audio_callback_arg: null_mut(),
        }
    }
}

pub type AesndVoiceCallbackArg =
    Option<unsafe extern "C" fn(pb: *mut Aesndpb, state: u32, cb_arg: *mut c_void)>;
pub type AesndVoiceCallback = Option<unsafe extern "C" fn(pb: *mut Aesndpb, state: u32)>;
pub type AesndAudioCallbackArg =
    Option<unsafe extern "C" fn(audio_buffer: *mut c_void, len: u32, cb_arg: *mut c_void)>;
pub type AesndAudioCallback = Option<unsafe extern "C" fn(audio_buffer: *mut c_void, len: u32)>;

const MSR_EE: u32 = 1 << 15;

#[inline(always)]
unsafe fn ppc_mfmsr() -> u32 {
    let value: u32;
    unsafe {
        core::arch::asm!("mfmsr {0}", out(reg) value, options(nostack));
    }
    value
}

#[inline(always)]
unsafe fn ppc_mtmsr(value: u32) {
    unsafe {
        core::arch::asm!("mtmsr {0}", in(reg) value, options(nostack));
    }
}

#[inline(always)]
unsafe fn disable_interrupts() -> u32 {
    let msr = unsafe { ppc_mfmsr() };
    unsafe { ppc_mtmsr(msr & !MSR_EE) };
    msr & MSR_EE
}

#[inline(always)]
unsafe fn restore_interrupts(state: u32) {
    let msr = unsafe { ppc_mfmsr() } & !MSR_EE;
    unsafe { ppc_mtmsr(msr | (state & MSR_EE)) };
}

/// Single-core critical-section storage for state shared with the audio IRQ.
///
/// This is deliberately not a spinlock: an audio interrupt must never spin on
/// a lock held by the code it interrupted. Every guard masks external
/// interrupts and restores the previous interrupt-enable state when dropped.
struct InterruptLock<T> {
    value: UnsafeCell<T>,
}

// SAFETY: `InterruptLock` only exposes `T` while external interrupts are
// masked. The Wii is single-core, and AESND code never yields or invokes user
// callbacks while a guard exists. Therefore two executions cannot access the
// contained value concurrently.
unsafe impl<T: Send> Sync for InterruptLock<T> {}

impl<T> InterruptLock<T> {
    const fn new(value: T) -> Self {
        Self {
            value: UnsafeCell::new(value),
        }
    }

    fn lock(&self) -> InterruptGuard<'_, T> {
        // SAFETY: This target is PowerPC-only. The returned state is restored by
        // `InterruptGuard::drop`, including when interrupts were already masked.
        let interrupt_state = unsafe { disable_interrupts() };
        InterruptGuard {
            lock: self,
            interrupt_state,
        }
    }
}

struct InterruptGuard<'a, T> {
    lock: &'a InterruptLock<T>,
    interrupt_state: u32,
}

impl<T> core::ops::Deref for InterruptGuard<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        // SAFETY: The guard masks interrupts for its full lifetime and is the
        // only API that exposes the contained value.
        unsafe { &*self.lock.value.get() }
    }
}

impl<T> core::ops::DerefMut for InterruptGuard<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        // SAFETY: See `Deref`; the mutable reference cannot overlap another
        // state access because interrupts remain masked and no guard is held
        // across a callback or scheduler operation.
        unsafe { &mut *self.lock.value.get() }
    }
}

impl<T> Drop for InterruptGuard<'_, T> {
    fn drop(&mut self) {
        // SAFETY: `interrupt_state` came from `disable_interrupts` for this
        // guard and must be restored exactly once.
        unsafe { restore_interrupts(self.interrupt_state) };
    }
}

#[derive(Copy, Clone)]
enum AudioCallbackDispatch {
    WithArg {
        callback: unsafe extern "C" fn(*mut c_void, u32, *mut c_void),
        callback_arg: *mut c_void,
    },
    Legacy {
        callback: unsafe extern "C" fn(*mut c_void, u32),
    },
}

impl AudioCallbackDispatch {
    unsafe fn invoke(self, audio_buffer: *mut c_void, len: u32) {
        match self {
            Self::WithArg {
                callback,
                callback_arg,
            } => unsafe { callback(audio_buffer, len, callback_arg) },
            Self::Legacy { callback } => unsafe { callback(audio_buffer, len) },
        }
    }
}

#[derive(Copy, Clone)]
enum VoiceCallbackDispatch {
    WithArg {
        callback: unsafe extern "C" fn(*mut Aesndpb, u32, *mut c_void),
        voice: *mut Aesndpb,
        event: u32,
        callback_arg: *mut c_void,
    },
    Legacy {
        callback: unsafe extern "C" fn(*mut Aesndpb, u32),
        voice: *mut Aesndpb,
        event: u32,
    },
}

impl VoiceCallbackDispatch {
    unsafe fn invoke(self) {
        match self {
            Self::WithArg {
                callback,
                voice,
                event,
                callback_arg,
            } => unsafe { callback(voice, event, callback_arg) },
            Self::Legacy {
                callback,
                voice,
                event,
            } => unsafe { callback(voice, event) },
        }
    }
}

#[repr(C, align(32))]
#[derive(Clone, Copy)]
struct DmaAligned<T>(T);

impl<T> core::ops::Deref for DmaAligned<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}
impl<T> core::ops::DerefMut for DmaAligned<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

struct AesndState {
    initialized: bool,
    session: u32,
    currab: usize,
    process: ProcessCycle,
    current_voice: Option<(usize, VoiceIdentity)>,
    stopping: bool,
    globalpause: bool,
    voicesstopped: bool,
    voicepb: [Aesndpb; MAX_VOICES as usize],
    voice_generation: [u32; MAX_VOICES as usize],
    stream_buffer: [DmaAligned<[u8; (DSP_STREAMBUFFER_SIZE as usize) * 2]>; MAX_VOICES as usize],
    legacy_voice_callbacks: [AesndVoiceCallback; MAX_VOICES as usize],
    command: DmaAligned<Aesndpb>,
    legacy_audio_callback: AesndAudioCallback,
    mute_buffer: DmaAligned<[u8; SND_BUFFERSIZE as usize]>,
    audio_buffer: [DmaAligned<[u8; SND_BUFFERSIZE as usize]>; 2],
}

// SAFETY: Raw callback context pointers are only copied while state is in an
// interrupt-masked critical section. Dereferencing them is delegated to the
// registered unsafe callbacks, whose public contracts require valid lifetimes
// and synchronization.
unsafe impl Send for AesndState {}

impl AesndState {
    const fn new() -> Self {
        Self {
            initialized: false,
            session: 0,
            currab: 0,
            process: ProcessCycle::new(),
            current_voice: None,
            stopping: false,
            globalpause: false,
            voicesstopped: true,
            voicepb: [Aesndpb::zeroed(); MAX_VOICES as usize],
            voice_generation: [0; MAX_VOICES as usize],
            stream_buffer: [DmaAligned([0; (DSP_STREAMBUFFER_SIZE as usize) * 2]);
                MAX_VOICES as usize],
            legacy_voice_callbacks: [None; MAX_VOICES as usize],
            command: DmaAligned(Aesndpb::zeroed()),
            legacy_audio_callback: None,
            mute_buffer: DmaAligned([0; SND_BUFFERSIZE as usize]),
            audio_buffer: [DmaAligned([0; SND_BUFFERSIZE as usize]); 2],
        }
    }
}

static AESND_STATE: InterruptLock<AesndState> = InterruptLock::new(AesndState::new());

fn replace_state(state: &mut AesndState, initialized: bool) {
    let next_session = state.session.wrapping_add(1);
    *state = AesndState::new();
    state.session = next_session;
    state.initialized = initialized;
}

fn bump_voice_generation(state: &mut AesndState, index: usize) {
    state.voice_generation[index] = state.voice_generation[index].wrapping_add(1);
}

fn voice_index_from_ptr(state: &AesndState, pb: *mut Aesndpb) -> Option<usize> {
    if pb.is_null() {
        return None;
    }
    if core::ptr::eq(pb, &state.command.0) {
        return state.current_voice.map(|(index, _)| index);
    }
    let base = state.voicepb.as_ptr() as usize;
    let end = base + core::mem::size_of_val(&state.voicepb);
    let ptr = pb as usize;
    if ptr < base || ptr >= end {
        return None;
    }
    let stride = core::mem::size_of::<Aesndpb>();
    if stride == 0 || !(ptr - base).is_multiple_of(stride) {
        return None;
    }
    Some((ptr - base) / stride)
}

fn voice_mut_by_ptr(state: &mut AesndState, pb: *mut Aesndpb) -> Option<&mut Aesndpb> {
    let index = voice_index_from_ptr(state, pb)?;
    if core::ptr::eq(pb, &state.command.0) {
        Some(&mut state.command.0)
    } else {
        Some(&mut state.voicepb[index])
    }
}

fn request_voice(state: &AesndState, index: usize) -> &Aesndpb {
    if state
        .current_voice
        .is_some_and(|(current, _)| current == index)
    {
        &state.command.0
    } else {
        &state.voicepb[index]
    }
}

fn request_voice_mut(state: &mut AesndState, index: usize) -> &mut Aesndpb {
    if state
        .current_voice
        .is_some_and(|(current, _)| current == index)
    {
        &mut state.command.0
    } else {
        &mut state.voicepb[index]
    }
}

#[cfg(gamecube)]
fn mark_aram_voice_running(state: &mut AesndState, index: usize) {
    state.voicepb[index].flags |= VOICE_RUNNING;
}

fn set_voice_format_inner(pb: &mut Aesndpb, format: u32) {
    pb.flags = (pb.flags & !0x07) | (format & 0x07);
    pb.shift = match format & 0x07 {
        VOICE_MONO16 | VOICE_STEREO16 | VOICE_MONO16_UNSIGNED | VOICE_STEREO16_UNSIGNED => 1,
        _ => 0,
    };
}

fn set_voice_buffer_inner(pb: &mut Aesndpb, buffer: *const c_void, len: u32) {
    let start = (buffer as usize as u32) & !SYS_BASE_UNCACHED;
    pb.mram_start = start;
    pb.mram_curr = start;
    pb.mram_end = start.wrapping_add(len);
}

fn set_voice_frequency_inner(pb: &mut Aesndpb, freq: F32) {
    let ratio = ((0x0001_0000u32 as F32) * (freq / DSP_DEFAULT_FREQ) + 0.5) as u32;
    pb.freq_h = (ratio >> 16) as u16;
    pb.freq_l = (ratio & 0xffff) as u16;
}

fn audio_callback_for(state: &AesndState) -> Option<AudioCallbackDispatch> {
    if let Some(callback) = state.command.audio_callback {
        Some(AudioCallbackDispatch::WithArg {
            callback,
            callback_arg: state.command.audio_callback_arg,
        })
    } else {
        state
            .legacy_audio_callback
            .map(|callback| AudioCallbackDispatch::Legacy { callback })
    }
}

fn voice_callback_for(
    voice: &mut Aesndpb,
    legacy_callback: AesndVoiceCallback,
    event: u32,
) -> Option<VoiceCallbackDispatch> {
    let voice_ptr = voice as *mut Aesndpb;
    if let Some(callback) = voice.cb {
        Some(VoiceCallbackDispatch::WithArg {
            callback,
            voice: voice_ptr,
            event,
            callback_arg: voice.callback_arg,
        })
    } else {
        legacy_callback.map(|callback| VoiceCallbackDispatch::Legacy {
            callback,
            voice: voice_ptr,
            event,
        })
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
struct VoiceIdentity {
    session: u32,
    generation: u32,
}

#[derive(Default)]
struct RequestProgress {
    source_exhausted: bool,
    stream_callback_invoked: bool,
}

enum RequestAction {
    InvokeStream(VoiceCallbackDispatch),
    Complete,
    Invalid,
}

fn cached_sample_ptr(physical: u32) -> *const u8 {
    (physical | 0x8000_0000) as usize as *const u8
}

fn staged_buffer_physical(state: &AesndState, index: usize) -> u32 {
    native_dsp::staging_address(state, index)
}

fn fill_stream_half(state: &mut AesndState, index: usize, half: usize) {
    let (mram_curr, mram_end) = {
        let voice = request_voice(state, index);
        (voice.mram_curr, voice.mram_end)
    };
    let remaining = mram_end.saturating_sub(mram_curr);
    let copy_len = remaining.min(DSP_STREAMBUFFER_SIZE) as usize;
    let offset = half * DSP_STREAMBUFFER_SIZE as usize;
    let dst = &mut state.stream_buffer[index][offset..offset + DSP_STREAMBUFFER_SIZE as usize];
    if copy_len != 0 {
        // SAFETY: The public voice-buffer contract requires MRAM to remain valid
        // while AESND can stage from it. `copy_len` is bounded by that MRAM end.
        let src = unsafe { core::slice::from_raw_parts(cached_sample_ptr(mram_curr), copy_len) };
        dst[..copy_len].copy_from_slice(src);
    }
    dst[copy_len..].fill(0);
    request_voice_mut(state, index).mram_curr = mram_curr.wrapping_add(copy_len as u32);
    native_dsp::publish_staging(state, index, offset, DSP_STREAMBUFFER_SIZE);
}

fn stage_initial_stream(state: &mut AesndState, index: usize) {
    let base = staged_buffer_physical(state, index);
    let shift = request_voice_mut(state, index).shift;
    request_voice_mut(state, index).buf_start = base >> shift;
    request_voice_mut(state, index).buf_end =
        (base + (DSP_STREAMBUFFER_SIZE * 2) - (1_u32 << shift)) >> shift;
    request_voice_mut(state, index).buf_curr = request_voice_mut(state, index).buf_start;

    let (mram_curr, mram_end) = {
        let voice = request_voice(state, index);
        (voice.mram_curr, voice.mram_end)
    };
    let remaining = mram_end.saturating_sub(mram_curr);
    let copy_len = remaining.min(DSP_STREAMBUFFER_SIZE * 2) as usize;
    let dst = &mut state.stream_buffer[index];
    if copy_len != 0 {
        // SAFETY: See `fill_stream_half`; this is the same C staging operation,
        // bounded to the two-block stream buffer.
        let src = unsafe { core::slice::from_raw_parts(cached_sample_ptr(mram_curr), copy_len) };
        dst[..copy_len].copy_from_slice(src);
    }
    dst[copy_len..].fill(0);
    let voice = request_voice_mut(state, index);
    voice.mram_curr = mram_curr.wrapping_add(copy_len as u32);
    #[cfg(not(gamecube))]
    {
        voice.flags |= VOICE_RUNNING;
    }
    native_dsp::publish_staging(state, index, 0, DSP_STREAMBUFFER_SIZE * 2);
}

fn voice_identity_is_current(index: usize, identity: VoiceIdentity) -> bool {
    let state = AESND_STATE.lock();
    state.initialized
        && state.session == identity.session
        && state.voice_generation[index] == identity.generation
        && (state.voicepb[index].flags & VOICE_USED) != 0
}

fn next_voice_request_action(
    index: usize,
    identity: VoiceIdentity,
    progress: &mut RequestProgress,
) -> RequestAction {
    let mut state = AESND_STATE.lock();
    if !state.initialized
        || state.session != identity.session
        || state.voice_generation[index] != identity.generation
        || (request_voice(&state, index).flags & VOICE_USED) == 0
    {
        return RequestAction::Invalid;
    }
    let legacy_callback = state.legacy_voice_callbacks[index];

    if progress.source_exhausted
        || request_voice(&state, index).mram_curr >= request_voice(&state, index).mram_end
    {
        progress.source_exhausted = true;
        if (request_voice(&state, index).flags & VOICE_STREAM) != 0
            && !progress.stream_callback_invoked
        {
            progress.stream_callback_invoked = true;
            if let Some(callback) = voice_callback_for(
                request_voice_mut(&mut state, index),
                legacy_callback,
                VOICE_STATE_STREAM,
            ) {
                return RequestAction::InvokeStream(callback);
            }
        }

        // BUG FIX: C stays in its entered end-of-source branch after the
        // callback. Read the updated flags, but do not test source exhaustion
        // again: a replacement buffer must not bypass ONCE or LOOP handling.
        if (request_voice(&state, index).flags & VOICE_ONCE) != 0 {
            // RVL libogc clears the staged-buffer address before stopping.
            #[cfg(not(gamecube))]
            {
                request_voice_mut(&mut state, index).buf_start = 0;
            }
            request_voice_mut(&mut state, index).flags |= VOICE_STOPPED;
            return RequestAction::Complete;
        }
        if (request_voice(&state, index).flags & VOICE_LOOP) != 0 {
            request_voice_mut(&mut state, index).mram_curr =
                request_voice(&state, index).mram_start;
        }
    }

    if request_voice(&state, index).buf_start != 0 {
        let curr_pos = request_voice(&state, index).buf_curr;
        let stream_last = request_voice(&state, index).stream_last;
        if curr_pos < stream_last {
            fill_stream_half(&mut state, index, 1);
        }
        let threshold = state.voicepb[index]
            .buf_start
            .wrapping_add(DSP_STREAMBUFFER_SIZE >> request_voice(&state, index).shift);
        if curr_pos >= threshold && stream_last < threshold {
            fill_stream_half(&mut state, index, 0);
        }
        request_voice_mut(&mut state, index).stream_last = curr_pos;
        return RequestAction::Complete;
    }

    stage_initial_stream(&mut state, index);
    RequestAction::Complete
}

fn service_voice_request(index: usize, identity: VoiceIdentity) {
    let mut progress = RequestProgress::default();
    loop {
        match next_voice_request_action(index, identity, &mut progress) {
            RequestAction::InvokeStream(callback) => {
                // No state guard is held across callback re-entry. A freed or
                // replaced source invalidates this request before further reads.
                unsafe { callback.invoke() };
                if !voice_identity_is_current(index, identity) {
                    return;
                }
            }
            RequestAction::Complete => break,
            RequestAction::Invalid => return,
        }
    }
    let callback = {
        let mut state = AESND_STATE.lock();
        let legacy = state.legacy_voice_callbacks[index];
        let voice = request_voice_mut(&mut state, index);
        if voice.flags & VOICE_STOPPED != 0 {
            voice_callback_for(voice, legacy, VOICE_STATE_STOPPED)
        } else {
            None
        }
    };
    if let Some(callback) = callback {
        unsafe { callback.invoke() };
    }
}

pub fn init() {
    if let Err(error) = native_dsp::init() {
        ogc_system::report(format_args!("AESND initialization failed: {error:?}\n"));
    }
}

pub fn reset() {
    if let Err(error) = native_dsp::reset() {
        ogc_system::report(format_args!("AESND reset failed: {error:?}\n"));
    }
}

pub fn pause(pause: Bool) {
    let mut state = AESND_STATE.lock();
    state.globalpause = pause != 0;
}

pub fn get_dsp_process_time() -> u32 {
    AESND_STATE
        .lock()
        .process
        .microseconds(lwp::TICKS_PER_SECOND)
}

pub fn get_dsp_process_usage() -> F32 {
    get_dsp_process_time() as F32 * 100.0 / 2000.0
}

/// Registers the global AESND audio callback and its opaque context pointer.
///
/// # Safety
///
/// When `cb` is `Some`, `cb_arg` must satisfy the callback's requirements and
/// remain valid until the callback is replaced, unregistered, or AESND is reset.
/// Any concurrent access to the referenced context must be synchronized.
pub unsafe fn register_audio_callback_with_arg(
    cb: AesndAudioCallbackArg,
    cb_arg: *mut c_void,
) -> AesndAudioCallbackArg {
    let mut state = AESND_STATE.lock();
    let old = state.command.audio_callback;
    state.command.audio_callback = cb;
    state.command.audio_callback_arg = cb_arg;
    state.legacy_audio_callback = None;
    old
}

/// Registers the two-argument compatibility audio callback without changing its
/// function-pointer signature.
pub fn register_audio_callback(cb: AesndAudioCallback) -> AesndAudioCallback {
    let mut state = AESND_STATE.lock();
    let old = state.legacy_audio_callback;

    state.legacy_audio_callback = cb;
    state.command.audio_callback = None;
    state.command.audio_callback_arg = null_mut();

    old
}

/// Allocates a voice and stores an optional callback context for later use.
///
/// # Safety
///
/// When `cb` is `Some`, `cb_arg` must satisfy the callback's requirements and
/// remain valid until the callback is replaced, the voice is freed, or AESND is
/// reset. Any concurrent access to the referenced context must be synchronized.
pub unsafe fn allocate_voice_with_arg(
    cb: AesndVoiceCallbackArg,
    cb_arg: *mut c_void,
) -> *mut Aesndpb {
    let mut state = AESND_STATE.lock();
    for i in 0..state.voicepb.len() {
        if (state.voicepb[i].flags & VOICE_USED) != 0 {
            continue;
        }

        bump_voice_generation(&mut state, i);
        state.legacy_voice_callbacks[i] = None;
        let voice = &mut state.voicepb[i];
        *voice = Aesndpb::zeroed();
        voice.voiceno = i as u32;
        voice.flags = VOICE_USED | VOICE_STOPPED;
        voice.volume_l = 0x100;
        voice.volume_r = 0x100;
        voice.freq_h = 0x0001;
        voice.freq_l = 0x0000;
        voice.cb = cb;
        voice.callback_arg = cb_arg;
        let ptr = voice as *mut Aesndpb;
        return ptr;
    }
    null_mut()
}

/// Allocates a voice using the two-argument compatibility callback.
pub fn allocate_voice(cb: AesndVoiceCallback) -> *mut Aesndpb {
    let mut state = AESND_STATE.lock();

    for i in 0..state.voicepb.len() {
        if (state.voicepb[i].flags & VOICE_USED) != 0 {
            continue;
        }

        bump_voice_generation(&mut state, i);
        state.legacy_voice_callbacks[i] = cb;

        let voice = &mut state.voicepb[i];
        *voice = Aesndpb::zeroed();
        voice.voiceno = i as u32;
        voice.flags = VOICE_USED | VOICE_STOPPED;
        voice.volume_l = 0x100;
        voice.volume_r = 0x100;
        voice.freq_h = 0x0001;
        voice.freq_l = 0x0000;

        let ptr = voice as *mut Aesndpb;
        return ptr;
    }

    null_mut()
}

pub fn free_voice(pb: *mut Aesndpb) {
    let mut state = AESND_STATE.lock();
    let Some(index) = voice_index_from_ptr(&state, pb) else {
        return;
    };
    bump_voice_generation(&mut state, index);
    state.voicepb[index] = Aesndpb::zeroed();
    state.legacy_voice_callbacks[index] = None;
}

pub fn set_voice_stop(pb: *mut Aesndpb, stop: Bool) {
    let mut state = AESND_STATE.lock();
    let Some(index) = voice_index_from_ptr(&state, pb) else {
        return;
    };
    let is_slot = !core::ptr::eq(pb, &state.command.0);
    if stop != 0 && is_slot {
        bump_voice_generation(&mut state, index);
    }
    let Some(voice) = voice_mut_by_ptr(&mut state, pb) else {
        return;
    };
    if stop != 0 {
        voice.flags |= VOICE_STOPPED;
    } else {
        voice.flags &= !VOICE_STOPPED;
    }
}

pub fn set_voice_mute(pb: *mut Aesndpb, mute: Bool) {
    let mut state = AESND_STATE.lock();
    let Some(voice) = voice_mut_by_ptr(&mut state, pb) else {
        return;
    };
    if mute != 0 {
        voice.flags |= VOICE_PAUSE;
    } else {
        voice.flags &= !VOICE_PAUSE;
    }
}

pub fn set_voice_loop(pb: *mut Aesndpb, loop_: Bool) {
    let mut state = AESND_STATE.lock();
    let Some(voice) = voice_mut_by_ptr(&mut state, pb) else {
        return;
    };
    if loop_ != 0 {
        voice.flags |= VOICE_LOOP;
    } else {
        voice.flags &= !VOICE_LOOP;
    }
}

pub fn set_voice_format(pb: *mut Aesndpb, format: u32) {
    let mut state = AESND_STATE.lock();
    let Some(voice) = voice_mut_by_ptr(&mut state, pb) else {
        return;
    };
    set_voice_format_inner(voice, format);
}

pub fn set_voice_stream(pb: *mut Aesndpb, stream: Bool) {
    let mut state = AESND_STATE.lock();
    let Some(voice) = voice_mut_by_ptr(&mut state, pb) else {
        return;
    };
    if stream != 0 {
        voice.flags |= VOICE_STREAM;
    } else {
        voice.flags &= !VOICE_STREAM;
    }
}

pub fn set_voice_frequency(pb: *mut Aesndpb, freq: F32) {
    let mut state = AESND_STATE.lock();
    let Some(voice) = voice_mut_by_ptr(&mut state, pb) else {
        return;
    };
    set_voice_frequency_inner(voice, freq);
}

pub fn set_voice_volume(pb: *mut Aesndpb, volume_l: u16, volume_r: u16) {
    let mut state = AESND_STATE.lock();
    let Some(voice) = voice_mut_by_ptr(&mut state, pb) else {
        return;
    };
    voice.volume_l = volume_l;
    voice.volume_r = volume_r;
}

pub fn set_voice_delay(pb: *mut Aesndpb, delay: u32) {
    let mut state = AESND_STATE.lock();
    let Some(voice) = voice_mut_by_ptr(&mut state, pb) else {
        return;
    };
    voice.delay = delay.wrapping_mul(48);
}

/// Stores a voice buffer for later consumption by the AESND/DSP path.
///
/// # Safety
///
/// If `len` is nonzero, `buffer` must be properly aligned for the selected audio
/// format and valid for reads of at least `len` bytes. The allocation must remain
/// alive and must not be mutated incompatibly until the voice is stopped and no
/// AESND callback or DSP operation can read it, or until the buffer is replaced.
pub unsafe fn set_voice_buffer(pb: *mut Aesndpb, buffer: *const c_void, len: u32) {
    let mut state = AESND_STATE.lock();
    if !core::ptr::eq(pb, &state.command.0)
        && let Some(index) = voice_index_from_ptr(&state, pb)
    {
        bump_voice_generation(&mut state, index);
    }
    let Some(voice) = voice_mut_by_ptr(&mut state, pb) else {
        return;
    };
    set_voice_buffer_inner(voice, buffer, len);
}

/// Starts playback using a caller-owned buffer retained by AESND.
///
/// # Safety
///
/// If `len` is nonzero, `buffer` must be properly aligned for `format` and valid
/// for reads of at least `len` bytes. The allocation must remain alive and must
/// not be mutated incompatibly until playback has stopped and no AESND callback
/// or DSP operation can read it, or until the buffer is replaced.
pub unsafe fn play_voice(
    pb: *mut Aesndpb,
    format: u32,
    buffer: *const c_void,
    len: u32,
    freq: F32,
    delay: u32,
    looped: Bool,
) {
    let mut state = AESND_STATE.lock();
    if !core::ptr::eq(pb, &state.command.0)
        && let Some(index) = voice_index_from_ptr(&state, pb)
    {
        bump_voice_generation(&mut state, index);
    }
    let Some(voice) = voice_mut_by_ptr(&mut state, pb) else {
        return;
    };
    set_voice_format_inner(voice, format);
    set_voice_frequency_inner(voice, freq);
    set_voice_buffer_inner(voice, buffer, len);

    voice.flags &= !(VOICE_RUNNING | VOICE_STOPPED | VOICE_LOOP | VOICE_ONCE);
    if looped != 0 {
        voice.flags |= VOICE_LOOP;
    } else {
        voice.flags |= VOICE_ONCE;
    }

    voice.buf_start = 0;
    voice.buf_curr = 0;
    voice.buf_end = 0;
    voice.stream_last = 0;
    voice.delay = delay.wrapping_mul(48);
    voice.pds = 0;
    voice.yn1 = 0;
    voice.yn2 = 0;
    voice.counter = 0;
}

/// Registers a voice callback and stores its opaque context pointer.
///
/// # Safety
///
/// When `cb` is `Some`, `cb_arg` must satisfy the callback's requirements and
/// remain valid until the callback is replaced, the voice is freed, or AESND is
/// reset. Any concurrent access to the referenced context must be synchronized.
pub unsafe fn register_voice_callback_with_arg(
    pb: *mut Aesndpb,
    cb: AesndVoiceCallbackArg,
    cb_arg: *mut c_void,
) -> AesndVoiceCallbackArg {
    let mut state = AESND_STATE.lock();
    let index = voice_index_from_ptr(&state, pb)?;
    let voice = voice_mut_by_ptr(&mut state, pb)?;
    let old = voice.cb;
    voice.cb = cb;
    voice.callback_arg = cb_arg;
    state.legacy_voice_callbacks[index] = None;
    old
}

/// Registers the two-argument compatibility callback for an allocated voice.
pub fn register_voice_callback(pb: *mut Aesndpb, cb: AesndVoiceCallback) -> AesndVoiceCallback {
    let mut state = AESND_STATE.lock();
    let index = voice_index_from_ptr(&state, pb)?;
    let old = state.legacy_voice_callbacks[index];
    state.legacy_voice_callbacks[index] = cb;
    let voice = voice_mut_by_ptr(&mut state, pb)?;
    voice.cb = None;
    voice.callback_arg = null_mut();
    old
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;
    use core::sync::atomic::{AtomicBool, Ordering};

    fn test_register_audio_callback_with_arg(
        cb: AesndAudioCallbackArg,
        cb_arg: *mut c_void,
    ) -> AesndAudioCallbackArg {
        // SAFETY: The test keeps its synthetic callback context valid until
        // global AESND state is reset.
        unsafe { super::register_audio_callback_with_arg(cb, cb_arg) }
    }

    fn test_allocate_voice_with_arg(
        cb: AesndVoiceCallbackArg,
        cb_arg: *mut c_void,
    ) -> *mut Aesndpb {
        // SAFETY: Test callback contexts remain valid until test state is reset.
        unsafe { super::allocate_voice_with_arg(cb, cb_arg) }
    }

    fn test_register_voice_callback_with_arg(
        pb: *mut Aesndpb,
        cb: AesndVoiceCallbackArg,
        cb_arg: *mut c_void,
    ) -> AesndVoiceCallbackArg {
        // SAFETY: Test callback contexts remain valid until test state is reset.
        unsafe { super::register_voice_callback_with_arg(pb, cb, cb_arg) }
    }

    fn test_set_voice_buffer(pb: *mut Aesndpb, buffer: *const c_void, len: u32) {
        // SAFETY: These tests inspect bookkeeping only and do not run the DSP
        // consumption path for their synthetic addresses.
        unsafe { super::set_voice_buffer(pb, buffer, len) }
    }

    fn test_play_voice(
        pb: *mut Aesndpb,
        format: u32,
        buffer: *const c_void,
        len: u32,
        freq: F32,
        delay: u32,
        looped: Bool,
    ) {
        // SAFETY: These tests inspect bookkeeping only and do not run the DSP
        // consumption path for their synthetic addresses.
        unsafe { super::play_voice(pb, format, buffer, len, freq, delay, looped) }
    }

    static TEST_LOCK: AtomicBool = AtomicBool::new(false);

    struct TestGuard;

    impl Drop for TestGuard {
        fn drop(&mut self) {
            TEST_LOCK.store(false, Ordering::Release);
        }
    }

    unsafe extern "C" fn voice_cb_a(_: *mut Aesndpb, _: u32, _: *mut c_void) {}

    unsafe extern "C" fn voice_cb_b(_: *mut Aesndpb, _: u32, _: *mut c_void) {}

    unsafe extern "C" fn stream_replace_buffer_cb(pb: *mut Aesndpb, event: u32) {
        if event == VOICE_STATE_STREAM {
            // SAFETY: The test never dereferences this synthetic replacement;
            // it only verifies that request decisions are re-read after callback.
            unsafe { super::set_voice_buffer(pb, 0x2000usize as *const c_void, 0x100) };
        }
    }

    unsafe extern "C" fn stream_replace_and_advance_cb(pb: *mut Aesndpb, event: u32) {
        if event == VOICE_STATE_STREAM {
            unsafe {
                stream_replace_buffer_cb(pb, event);
                (*pb).mram_curr += 0x40;
            }
        }
    }

    unsafe extern "C" fn stream_clear_once_cb(pb: *mut Aesndpb, event: u32) {
        if event == VOICE_STATE_STREAM {
            unsafe {
                stream_replace_and_advance_cb(pb, event);
                (*pb).flags &= !VOICE_ONCE;
            }
        }
    }

    unsafe extern "C" fn stream_reallocate_voice_cb(pb: *mut Aesndpb, event: u32) {
        if event == VOICE_STATE_STREAM {
            free_voice(pb);
            let replacement = allocate_voice(None);
            assert_eq!(replacement, pb);
        }
    }

    unsafe extern "C" fn audio_cb_a(_: *mut c_void, _: u32, _: *mut c_void) {}

    unsafe extern "C" fn audio_cb_b(_: *mut c_void, _: u32, _: *mut c_void) {}

    fn bool_to_flag(value: bool) -> Bool {
        value as Bool
    }

    fn test_guard() -> TestGuard {
        while TEST_LOCK.swap(true, Ordering::Acquire) {
            core::hint::spin_loop();
        }
        TestGuard
    }

    fn reset_host_state() {
        *AESND_STATE.lock() = AesndState::new();
    }

    fn voice_for(ptr: *mut Aesndpb) -> Aesndpb {
        let state = AESND_STATE.lock();
        let index = match voice_index_from_ptr(&state, ptr) {
            Some(index) => index,
            None => return Aesndpb::zeroed(),
        };
        state.voicepb[index]
    }

    #[test]
    fn voice_pool_roundtrip() {
        let _guard = test_guard();
        reset_host_state();
        let ptr = test_allocate_voice_with_arg(None, null_mut());
        assert!(!ptr.is_null());
        set_voice_volume(ptr, 0x80, 0x81);
        set_voice_delay(ptr, 2);
        free_voice(ptr);
    }

    #[test]
    fn play_voice_sets_expected_fields() {
        let _guard = test_guard();
        reset_host_state();
        let ptr = test_allocate_voice_with_arg(None, null_mut());
        assert!(!ptr.is_null());
        test_play_voice(
            ptr,
            VOICE_STEREO16,
            0x1000usize as *const c_void,
            0x200,
            32_000.0,
            3,
            bool_to_flag(false),
        );
        let state = AESND_STATE.lock();
        let index = match voice_index_from_ptr(&state, ptr) {
            Some(index) => index,
            None => return,
        };
        let voice = &state.voicepb[index];
        assert_eq!(voice.mram_start, 0x1000);
        assert_eq!(voice.mram_end, 0x1200);
        assert_eq!(voice.delay, 144);
        assert_eq!(voice.shift, 1);
        assert_ne!(voice.flags & VOICE_ONCE, 0);
        drop(state);
        free_voice(ptr);
    }

    #[test]
    fn voice_pool_exhaustion_and_reuse_match_c_bookkeeping() {
        let _guard = test_guard();
        reset_host_state();
        let mut ptrs = [null_mut(); MAX_VOICES as usize];
        for (i, slot) in ptrs.iter_mut().enumerate() {
            *slot = test_allocate_voice_with_arg(None, null_mut());
            assert!(!slot.is_null());
            let voice = voice_for(*slot);
            assert_eq!(voice.voiceno, i as u32);
            assert_ne!(voice.flags & VOICE_USED, 0);
            assert_ne!(voice.flags & VOICE_STOPPED, 0);
            assert_eq!(voice.volume_l, 0x100);
            assert_eq!(voice.volume_r, 0x100);
            assert_eq!(voice.freq_h, 1);
            assert_eq!(voice.freq_l, 0);
        }

        assert!(test_allocate_voice_with_arg(None, null_mut()).is_null());
        free_voice(ptrs[3]);
        let reused = test_allocate_voice_with_arg(None, null_mut());
        assert_eq!(reused, ptrs[3]);
    }

    #[test]
    fn callback_registration_returns_previous_callback() {
        let _guard = test_guard();
        reset_host_state();
        let audio_arg_a = 0x1111usize as *mut c_void;
        let audio_arg_b = 0x2222usize as *mut c_void;
        assert!(test_register_audio_callback_with_arg(Some(audio_cb_a), audio_arg_a).is_none());
        assert_eq!(
            test_register_audio_callback_with_arg(Some(audio_cb_b), audio_arg_b)
                .map(|cb| cb as usize),
            Some(audio_cb_a as *const () as usize)
        );
        let state = AESND_STATE.lock();
        assert_eq!(
            state
                .command
                .audio_callback
                .map(|cb| cb as *const () as usize),
            Some(audio_cb_b as *const () as usize)
        );
        assert_eq!(state.command.audio_callback_arg, audio_arg_b);
        drop(state);

        let voice_arg_a = 0x3333usize as *mut c_void;
        let voice_arg_b = 0x4444usize as *mut c_void;
        let ptr = test_allocate_voice_with_arg(Some(voice_cb_a), voice_arg_a);
        assert!(!ptr.is_null());
        assert_eq!(
            test_register_voice_callback_with_arg(ptr, Some(voice_cb_b), voice_arg_b)
                .map(|cb| cb as usize),
            Some(voice_cb_a as *const () as usize)
        );
        let voice = voice_for(ptr);
        assert_eq!(
            voice.cb.map(|cb| cb as *const () as usize),
            Some(voice_cb_b as *const () as usize)
        );
        assert_eq!(voice.callback_arg, voice_arg_b);
    }

    #[test]
    fn voice_flag_setters_toggle_stop_mute_loop_and_stream() {
        let _guard = test_guard();
        reset_host_state();
        let ptr = test_allocate_voice_with_arg(None, null_mut());
        assert!(!ptr.is_null());

        set_voice_stop(ptr, bool_to_flag(false));
        assert_eq!(voice_for(ptr).flags & VOICE_STOPPED, 0);
        set_voice_stop(ptr, bool_to_flag(true));
        assert_ne!(voice_for(ptr).flags & VOICE_STOPPED, 0);

        set_voice_mute(ptr, bool_to_flag(true));
        assert_ne!(voice_for(ptr).flags & VOICE_PAUSE, 0);
        set_voice_mute(ptr, bool_to_flag(false));
        assert_eq!(voice_for(ptr).flags & VOICE_PAUSE, 0);

        set_voice_loop(ptr, bool_to_flag(true));
        assert_ne!(voice_for(ptr).flags & VOICE_LOOP, 0);
        set_voice_loop(ptr, bool_to_flag(false));
        assert_eq!(voice_for(ptr).flags & VOICE_LOOP, 0);

        set_voice_stream(ptr, bool_to_flag(true));
        assert_ne!(voice_for(ptr).flags & VOICE_STREAM, 0);
        set_voice_stream(ptr, bool_to_flag(false));
        assert_eq!(voice_for(ptr).flags & VOICE_STREAM, 0);
    }

    #[test]
    fn voice_delay_frequency_format_and_buffer_match_c_conversions() {
        let _guard = test_guard();
        reset_host_state();
        let ptr = test_allocate_voice_with_arg(None, null_mut());
        assert!(!ptr.is_null());

        set_voice_delay(ptr, 7);
        set_voice_frequency(ptr, 32_000.0);
        set_voice_format(ptr, VOICE_STEREO16_UNSIGNED);
        test_set_voice_buffer(ptr, 0x5000usize as *const c_void, 0x180);

        let voice = voice_for(ptr);
        assert_eq!(voice.delay, 7 * 48);
        assert_eq!(
            ((voice.freq_h as u32) << 16) | voice.freq_l as u32,
            0x0000_aaab
        );
        assert_eq!(voice.flags & 0x07, VOICE_STEREO16_UNSIGNED);
        assert_eq!(voice.shift, 1);
        assert_eq!(voice.mram_start, 0x5000);
        assert_eq!(voice.mram_curr, 0x5000);
        assert_eq!(voice.mram_end, 0x5180);
    }

    #[test]
    fn play_voice_sets_once_loop_and_running_state_like_c_entrypoint() {
        let _guard = test_guard();
        reset_host_state();
        let ptr = test_allocate_voice_with_arg(None, null_mut());
        assert!(!ptr.is_null());

        test_play_voice(
            ptr,
            VOICE_MONO8,
            0x7000usize as *const c_void,
            0x20,
            48_000.0,
            0,
            bool_to_flag(true),
        );
        let looped = voice_for(ptr);
        assert_eq!(looped.flags & VOICE_STOPPED, 0);
        assert_eq!(looped.flags & VOICE_ONCE, 0);
        assert_ne!(looped.flags & VOICE_LOOP, 0);
        assert_eq!(looped.buf_start, 0);
        assert_eq!(looped.buf_curr, 0);
        assert_eq!(looped.buf_end, 0);
        assert_eq!(looped.stream_last, 0);

        test_play_voice(
            ptr,
            VOICE_MONO8,
            0x7000usize as *const c_void,
            0x20,
            48_000.0,
            0,
            bool_to_flag(false),
        );
        let once = voice_for(ptr);
        assert_eq!(once.flags & VOICE_LOOP, 0);
        assert_ne!(once.flags & VOICE_ONCE, 0);
    }

    #[test]
    fn invalid_voice_pointers_are_ignored() {
        let _guard = test_guard();
        reset_host_state();
        let invalid = 0x1234usize as *mut Aesndpb;
        free_voice(invalid);
        set_voice_stop(invalid, bool_to_flag(false));
        set_voice_mute(invalid, bool_to_flag(true));
        set_voice_loop(invalid, bool_to_flag(true));
        set_voice_format(invalid, VOICE_STEREO16);
        set_voice_stream(invalid, bool_to_flag(true));
        set_voice_frequency(invalid, 32_000.0);
        set_voice_volume(invalid, 1, 2);
        set_voice_delay(invalid, 3);
        test_set_voice_buffer(invalid, 0x1000usize as *const c_void, 4);
        test_play_voice(
            invalid,
            VOICE_STEREO16,
            0x1000usize as *const c_void,
            4,
            32_000.0,
            1,
            bool_to_flag(false),
        );
        assert!(
            test_register_voice_callback_with_arg(invalid, Some(voice_cb_a), null_mut()).is_none()
        );
        assert!(!test_allocate_voice_with_arg(None, null_mut()).is_null());
    }

    #[test]
    #[cfg(gamecube)]
    fn aram_completion_marks_canonical_voice_during_request() {
        let _guard = test_guard();
        reset_host_state();
        let mut state = AESND_STATE.lock();
        let index = 0;
        state.current_voice = Some((
            index,
            VoiceIdentity {
                session: 1,
                generation: 1,
            },
        ));
        state.command.0.flags = VOICE_USED;
        state.voicepb[index].flags = VOICE_USED;

        mark_aram_voice_running(&mut state, index);

        assert_ne!(state.voicepb[index].flags & VOICE_RUNNING, 0);
        assert_eq!(state.command.0.flags & VOICE_RUNNING, 0);
    }

    #[test]
    fn stream_callback_buffer_replacement_still_stops_once_voice() {
        let _guard = test_guard();
        reset_host_state();
        {
            let mut state = AESND_STATE.lock();
            state.initialized = true;
            state.session = 7;
        }
        let ptr = allocate_voice(Some(stream_replace_buffer_cb));
        assert!(!ptr.is_null());
        let (index, identity) = {
            let mut state = AESND_STATE.lock();
            let index = voice_index_from_ptr(&state, ptr).unwrap();
            let voice = &mut state.voicepb[index];
            voice.flags = VOICE_USED | VOICE_STREAM | VOICE_ONCE | VOICE_RUNNING;
            voice.mram_start = 0x1000;
            voice.mram_curr = 0x1100;
            voice.mram_end = 0x1100;
            voice.buf_start = 0x3000;
            voice.buf_curr = 0x3000;
            voice.stream_last = 0x3000;
            (
                index,
                VoiceIdentity {
                    session: state.session,
                    generation: state.voice_generation[index],
                },
            )
        };

        service_voice_request(index, identity);

        let voice = voice_for(ptr);
        assert_eq!(voice.mram_curr, 0x2000);
        assert_eq!(voice.mram_end, 0x2100);
        assert_ne!(voice.flags & VOICE_STOPPED, 0);
        #[cfg(not(gamecube))]
        assert_eq!(voice.buf_start, 0);
        #[cfg(gamecube)]
        assert_eq!(voice.buf_start, 0x3000);
    }

    #[test]
    fn stream_callback_loop_reset_and_updated_once_flag_match_c() {
        let _guard = test_guard();
        for (flags, callback, expected_cursor) in [
            (
                VOICE_LOOP,
                stream_replace_and_advance_cb as unsafe extern "C" fn(_, _),
                0x2000,
            ),
            (
                VOICE_ONCE,
                stream_clear_once_cb as unsafe extern "C" fn(_, _),
                0x2040,
            ),
        ] {
            reset_host_state();
            {
                let mut state = AESND_STATE.lock();
                state.initialized = true;
                state.session = 11;
            }
            let ptr = allocate_voice(Some(callback));
            assert!(!ptr.is_null());
            let (index, identity) = {
                let mut state = AESND_STATE.lock();
                let index = voice_index_from_ptr(&state, ptr).unwrap();
                let voice = &mut state.voicepb[index];
                voice.flags = VOICE_USED | VOICE_STREAM | VOICE_RUNNING | flags;
                voice.mram_start = 0x1000;
                voice.mram_curr = 0x1100;
                voice.mram_end = 0x1100;
                // Keep the staged cursor stationary, so no source copy occurs.
                voice.buf_start = 0x3000;
                voice.buf_curr = 0x3000;
                voice.stream_last = 0x3000;
                (
                    index,
                    VoiceIdentity {
                        session: state.session,
                        generation: state.voice_generation[index],
                    },
                )
            };
            service_voice_request(index, identity);
            let voice = voice_for(ptr);
            assert_eq!(voice.mram_start, 0x2000);
            assert_eq!(voice.mram_curr, expected_cursor);
            assert_eq!(voice.flags & VOICE_STOPPED, 0);
            assert_eq!(voice.flags & VOICE_ONCE, 0);
        }
    }

    #[test]
    fn stream_callback_reallocation_invalidates_old_request() {
        let _guard = test_guard();
        reset_host_state();
        {
            let mut state = AESND_STATE.lock();
            state.initialized = true;
            state.session = 9;
        }
        let ptr = allocate_voice(Some(stream_reallocate_voice_cb));
        assert!(!ptr.is_null());
        let (index, identity) = {
            let mut state = AESND_STATE.lock();
            let index = voice_index_from_ptr(&state, ptr).unwrap();
            let voice = &mut state.voicepb[index];
            voice.flags = VOICE_USED | VOICE_STREAM | VOICE_ONCE | VOICE_RUNNING;
            voice.mram_curr = 0x1000;
            voice.mram_end = 0x1000;
            (
                index,
                VoiceIdentity {
                    session: state.session,
                    generation: state.voice_generation[index],
                },
            )
        };

        service_voice_request(index, identity);

        let state = AESND_STATE.lock();
        assert_ne!(state.voice_generation[index], identity.generation);
        assert_eq!(state.voicepb[index].flags, VOICE_USED | VOICE_STOPPED);
        assert_eq!(state.voicepb[index].buf_start, 0);
    }
}
