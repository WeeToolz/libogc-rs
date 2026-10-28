#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::cell::UnsafeCell;
use core::ffi::c_void;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicI32, AtomicPtr, AtomicU32, AtomicUsize, Ordering};

#[cfg(feature = "sndlib")]
use asnd_sys as snd;
pub use mad_sys::{MadFrame, MadStream};
use mad_sys::{MadPcm, MadSynth};
#[cfg(not(feature = "sndlib"))]
use ogc_audio_sys as audio;
#[cfg(not(feature = "sndlib"))]
use ogc_cache::dc_flush_range;
use ogc_lwp_sys as lwp;
pub use ogc_sys::Bool;

mod output_ring;
mod resample;

pub type Mp3FilterFn = Option<unsafe extern "C" fn(stream: *mut MadStream, frame: *mut MadFrame)>;
pub type Mp3ReadFn =
    Option<unsafe extern "C" fn(cb_data: *mut c_void, buffer: *mut c_void, len: i32) -> i32>;

const INPUT_BUFFER_SIZE: usize = 32 * 1024;
const INPUT_BUFFER_STORAGE: usize = INPUT_BUFFER_SIZE + mad_sys::MAD_BUFFER_GUARD;
#[cfg(not(feature = "sndlib"))]
const DMA_BUFFER_SIZE: usize = 4_992;
#[cfg(feature = "sndlib")]
const DMA_BUFFER_SIZE: usize = 8_192;
const DMA_BUFFER_COUNT: usize = 3;
const OUTPUT_RATE: u32 = 48_000;
// Keep enough decoded audio to cover short CPU stalls in the game thread.
// At stereo 16-bit 48 kHz output, 128 KiB holds about 0.68 seconds.
const OUTPUT_RING_SIZE: usize = output_ring::CAPACITY;
// The Rust libmad ABI path uses large frame and synthesis values in nested
// calls. Keep enough stack for those temporaries on real Wii hardware.
const STREAM_STACK_SIZE: usize = 128 * 1024;
// The LWP scheduler runs higher priority values first. Match the MP3 player
// priority so decoding can refill audio before the application thread runs.
const STREAM_PRIORITY: u8 = 80;

#[repr(C, align(32))]
struct DmaBuffers([[u8; DMA_BUFFER_SIZE]; DMA_BUFFER_COUNT]);

#[repr(C, align(32))]
struct StreamStack([u8; STREAM_STACK_SIZE]);

struct OutputRing {
    buffer: UnsafeCell<[u8; OUTPUT_RING_SIZE]>,
    put: AtomicUsize,
    get: AtomicUsize,
    primed: AtomicBool,
}

unsafe impl Sync for OutputRing {}

impl OutputRing {
    const fn new() -> Self {
        Self {
            buffer: UnsafeCell::new([0; OUTPUT_RING_SIZE]),
            put: AtomicUsize::new(0),
            get: AtomicUsize::new(0),
            primed: AtomicBool::new(false),
        }
    }

    fn reset(&self) {
        self.put.store(0, Ordering::Release);
        self.get.store(0, Ordering::Release);
        self.primed.store(false, Ordering::Release);
    }

    fn used(&self) -> usize {
        let put = self.put.load(Ordering::Acquire);
        let get = self.get.load(Ordering::Acquire);
        (OUTPUT_RING_SIZE + put - get) % OUTPUT_RING_SIZE
    }

    fn space(&self) -> usize {
        let put = self.put.load(Ordering::Acquire);
        let get = self.get.load(Ordering::Acquire);
        (OUTPUT_RING_SIZE + get - put - 1) % OUTPUT_RING_SIZE
    }
}

#[derive(Copy, Clone)]
struct EqState {
    lf: f32,
    f1p0: f32,
    f1p1: f32,
    f1p2: f32,
    f1p3: f32,
    hf: f32,
    f2p0: f32,
    f2p1: f32,
    f2p2: f32,
    f2p3: f32,
    sdm1: f32,
    sdm2: f32,
    sdm3: f32,
    lg: f32,
    mg: f32,
    hg: f32,
}

impl EqState {
    fn new(low_freq: i32, high_freq: i32, mix_freq: i32) -> Self {
        let mut state = Self {
            lf: 0.0,
            f1p0: 0.0,
            f1p1: 0.0,
            f1p2: 0.0,
            f1p3: 0.0,
            hf: 0.0,
            f2p0: 0.0,
            f2p1: 0.0,
            f2p2: 0.0,
            f2p3: 0.0,
            sdm1: 0.0,
            sdm2: 0.0,
            sdm3: 0.0,
            lg: 1.0,
            mg: 1.0,
            hg: 1.0,
        };
        let pi = core::f32::consts::PI;
        state.lf = 2.0 * libm::sinf(pi * (low_freq as f32 / mix_freq as f32));
        state.hf = 2.0 * libm::sinf(pi * (high_freq as f32 / mix_freq as f32));
        state
    }

    fn process(&mut self, sample: i16) -> i16 {
        const VSA: f32 = 1.0 / 4_294_967_295.0;
        let sample = sample as f32;
        self.f1p0 += self.lf * (sample - self.f1p0) + VSA;
        self.f1p1 += self.lf * (self.f1p0 - self.f1p1);
        self.f1p2 += self.lf * (self.f1p1 - self.f1p2);
        self.f1p3 += self.lf * (self.f1p2 - self.f1p3);
        let mut low = self.f1p3;

        self.f2p0 += self.hf * (sample - self.f2p0) + VSA;
        self.f2p1 += self.hf * (self.f2p0 - self.f2p1);
        self.f2p2 += self.hf * (self.f2p1 - self.f2p2);
        self.f2p3 += self.hf * (self.f2p2 - self.f2p3);
        let mut high = self.sdm3 - self.f2p3;
        let mut mid = self.sdm3 - (high + low);

        low *= self.lg;
        mid *= self.mg;
        high *= self.hg;

        self.sdm3 = self.sdm2;
        self.sdm2 = self.sdm1;
        self.sdm1 = sample;
        (low + mid + high) as i16
    }
}

#[derive(Copy, Clone, Eq, PartialEq)]
enum InputKind {
    None,
    Buffer,
    Reader,
}

struct PlaybackState {
    input_kind: InputKind,
    buffer: *const u8,
    buffer_len: usize,
    buffer_pos: usize,
    reader: Mp3ReadFn,
    reader_data: *mut c_void,
    filter: Mp3FilterFn,
    looped: bool,
}

impl PlaybackState {
    const fn new() -> Self {
        Self {
            input_kind: InputKind::None,
            buffer: ptr::null(),
            buffer_len: 0,
            buffer_pos: 0,
            reader: None,
            reader_data: ptr::null_mut(),
            filter: None,
            looped: false,
        }
    }
}

static INIT_DONE: AtomicBool = AtomicBool::new(false);
static PLAYING: AtomicBool = AtomicBool::new(false);
static THREAD_CREATED: AtomicBool = AtomicBool::new(false);
static STOP_REQUESTED: AtomicBool = AtomicBool::new(false);
static DMA_PLAYING: AtomicBool = AtomicBool::new(false);
#[cfg(feature = "sndlib")]
static HAVE_SAMPLES: AtomicBool = AtomicBool::new(false);
static CURRENT_OUTPUT: AtomicUsize = AtomicUsize::new(0);
static VOLUME: AtomicU32 = AtomicU32::new(255);
// Playback speed in thousandths; 1000 is the recorded speed. The application
// game adjusts this while the song plays, without restarting the decoder.
static PLAYBACK_SPEED_PER_MILLE: AtomicU32 = AtomicU32::new(1_000);
static AUDIO_PAUSED: AtomicBool = AtomicBool::new(false);
static BACKEND_ERROR: AtomicI32 = AtomicI32::new(0);
static LAST_LEN: AtomicU32 = AtomicU32::new(0);
static LAST_CB_DATA: AtomicPtr<c_void> = AtomicPtr::new(ptr::null_mut());
static FILTER_CALLED: AtomicBool = AtomicBool::new(false);

static mut STATE: PlaybackState = PlaybackState::new();
static mut INPUT_BUFFER: [u8; INPUT_BUFFER_STORAGE] = [0; INPUT_BUFFER_STORAGE];
static mut OUTPUT_BUFFERS: DmaBuffers = DmaBuffers([[0; DMA_BUFFER_SIZE]; DMA_BUFFER_COUNT]);
static OUTPUT_RING: OutputRing = OutputRing::new();
static mut STREAM_STACK: StreamStack = StreamStack([0; STREAM_STACK_SIZE]);
static mut STREAM_THREAD: lwp::LwpThread = lwp::LWP_THREAD_NULL;
static STREAM_QUEUE: AtomicU32 = AtomicU32::new(lwp::LWP_TQUEUE_NULL);

fn state_ptr() -> *mut PlaybackState {
    ptr::addr_of_mut!(STATE)
}

fn input_ptr() -> *mut u8 {
    ptr::addr_of_mut!(INPUT_BUFFER).cast::<u8>()
}

fn output_ptr(index: usize) -> *mut u8 {
    unsafe { ptr::addr_of_mut!(OUTPUT_BUFFERS.0[index]).cast::<u8>() }
}

fn ring() -> &'static OutputRing {
    &OUTPUT_RING
}

unsafe fn ring_get(data: *mut u8, len: usize) -> usize {
    unsafe {
        let ring = ring();
        let read_len = len.min(ring.used());
        if read_len == 0 {
            let output = core::slice::from_raw_parts_mut(data, len);
            output_ring::silence_unfilled(output, 0);
            #[cfg(not(feature = "sndlib"))]
            dc_flush_range(data.cast_const().cast(), len as u32);
            let queue = STREAM_QUEUE.load(Ordering::Acquire);
            if queue != lwp::LWP_TQUEUE_NULL {
                lwp::thread_signal(queue);
            }
            return 0;
        }

        let get = ring.get.load(Ordering::Acquire);
        let first = read_len.min(OUTPUT_RING_SIZE - get);
        let buffer = ring.buffer.get().cast::<u8>();
        ptr::copy_nonoverlapping(buffer.add(get), data, first);
        if first != read_len {
            ptr::copy_nonoverlapping(buffer, data.add(first), read_len - first);
        }
        ring.get
            .store((get + read_len) % OUTPUT_RING_SIZE, Ordering::Release);
        let output = core::slice::from_raw_parts_mut(data, len);
        output_ring::silence_unfilled(output, read_len);
        #[cfg(not(feature = "sndlib"))]
        dc_flush_range(data.cast_const().cast(), len as u32);

        let queue = STREAM_QUEUE.load(Ordering::Acquire);
        if queue != lwp::LWP_TQUEUE_NULL {
            lwp::thread_signal(queue);
        }
        read_len
    }
}

#[cfg(not(feature = "sndlib"))]
unsafe fn start_backend() {
    unsafe {
        CURRENT_OUTPUT.store(0, Ordering::Release);
        ptr::write_bytes(output_ptr(0), 0, DMA_BUFFER_SIZE);
        dc_flush_range(output_ptr(0).cast_const().cast(), DMA_BUFFER_SIZE as u32);
        audio::audio_init_dma(output_ptr(0) as usize as u32, DMA_BUFFER_SIZE as u32);
        if !AUDIO_PAUSED.load(Ordering::Acquire) {
            audio::audio_start_dma();
        }
        CURRENT_OUTPUT.store(1, Ordering::Release);
    }
}

#[cfg(feature = "sndlib")]
unsafe fn start_backend() {
    CURRENT_OUTPUT.store(0, Ordering::Release);
    HAVE_SAMPLES.store(false, Ordering::Release);
    let volume = VOLUME.load(Ordering::Acquire) as i32;
    let status = unsafe {
        snd::set_voice(snd::VoiceConfig {
            voice: 0,
            format: snd::VOICE_STEREO_16BIT,
            pitch: OUTPUT_RATE as i32,
            delay: 0,
            snd: output_ptr(0).cast(),
            size_snd: DMA_BUFFER_SIZE as i32,
            volume_l: volume,
            volume_r: volume,
            callback: Some(snd_callback),
        })
    };
    if status != snd::SND_OK {
        BACKEND_ERROR.store(status, Ordering::Release);
        PLAYING.store(false, Ordering::Release);
        return;
    }
    if AUDIO_PAUSED.load(Ordering::Acquire) {
        let pause_status = snd::pause_voice(0, 1);
        if pause_status != snd::SND_OK {
            BACKEND_ERROR.store(pause_status, Ordering::Release);
            PLAYING.store(false, Ordering::Release);
            let stop_status = snd::stop_voice(0);
            if stop_status != snd::SND_OK {
                BACKEND_ERROR.store(stop_status, Ordering::Release);
            }
            return;
        }
    }
    CURRENT_OUTPUT.store(1, Ordering::Release);
}

unsafe fn start_backend_if_primed() {
    let ring = ring();
    if ring.primed.load(Ordering::Acquire) || ring.used() < OUTPUT_RING_SIZE / 2 {
        return;
    }
    if ring
        .primed
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_ok()
    {
        unsafe { start_backend() };
    }
}

unsafe fn ring_put(data: *const u8, len: usize) -> bool {
    let ring = ring();
    while ring.space() < len {
        if !PLAYING.load(Ordering::Acquire) {
            return false;
        }
        let queue = STREAM_QUEUE.load(Ordering::Acquire);
        if queue == lwp::LWP_TQUEUE_NULL {
            return false;
        }
        lwp::thread_sleep(queue);
    }

    let put = ring.put.load(Ordering::Acquire);
    let first = len.min(OUTPUT_RING_SIZE - put);
    let buffer = ring.buffer.get().cast::<u8>();
    unsafe {
        ptr::copy_nonoverlapping(data, buffer.add(put), first);
        if first != len {
            ptr::copy_nonoverlapping(data.add(first), buffer, len - first);
        }
    }
    ring.put
        .store((put + len) % OUTPUT_RING_SIZE, Ordering::Release);
    unsafe { start_backend_if_primed() };
    true
}

unsafe fn read_source(state: *mut PlaybackState, destination: *mut u8, len: usize) -> i32 {
    match unsafe { (*state).input_kind } {
        InputKind::None => 0,
        InputKind::Buffer => {
            let position = unsafe { (*state).buffer_pos };
            let remaining = unsafe { (*state).buffer_len.saturating_sub(position) };
            let amount = remaining.min(len);
            if amount == 0 {
                return 0;
            }
            unsafe {
                ptr::copy_nonoverlapping((*state).buffer.add(position), destination, amount);
                (*state).buffer_pos += amount;
            }
            amount as i32
        }
        InputKind::Reader => {
            let Some(reader) = (unsafe { (*state).reader }) else {
                return 0;
            };
            let read = unsafe { reader((*state).reader_data, destination.cast(), len as i32) };
            if read > 0 {
                LAST_LEN.store(read as u32, Ordering::Release);
            }
            read
        }
    }
}

fn fixed_to_short(fixed: mad_sys::MadFixedT) -> i16 {
    if fixed >= mad_sys::MAD_F_ONE {
        return i16::MAX;
    }
    if fixed <= -mad_sys::MAD_F_ONE {
        return -i16::MAX;
    }
    (fixed >> (mad_sys::MAD_F_FRACBITS - 15)) as i16
}

fn average_pcm_level(pcm: &MadPcm) -> u32 {
    let channels = usize::from(pcm.channels.min(2));
    let length = usize::from(pcm.length);
    let sample_count = channels * length;
    if sample_count == 0 {
        return 0;
    }

    let mut total = 0_u64;
    for channel in &pcm.samples[..channels] {
        for &sample in &channel[..length] {
            total += u64::from(fixed_to_short(sample).unsigned_abs());
        }
    }
    (total / sample_count as u64) as u32
}

fn mad_recoverable(error: i32) -> bool {
    error & 0xff00 != 0
}

unsafe fn resample(
    pcm: *const MadPcm,
    eqs: &mut [EqState; 2],
    phase: &mut u32,
    current_speed: &mut u32,
    previous_level: &mut u32,
) -> bool {
    let pcm = unsafe { &*pcm };
    let stereo = pcm.channels == 2;
    // MP3 has no note markers. Use a clear level rise as an onset estimate
    // and apply speed changes at that decoded-frame boundary.
    let target = PLAYBACK_SPEED_PER_MILLE.load(Ordering::Relaxed);
    let level = average_pcm_level(pcm);
    if target != *current_speed && resample::is_note_onset(*previous_level, level) {
        *current_speed = target;
    }
    *previous_level = level;

    let increment = (((pcm.samplerate as u64) * (*current_speed as u64) * 65_536)
        / ((OUTPUT_RATE as u64) * 1_000))
        .max(1) as u32;
    let mut position = *phase;
    while (position >> 16) < u32::from(pcm.length) {
        if !PLAYING.load(Ordering::Acquire) {
            return false;
        }
        let index = (position >> 16) as usize;
        let next_index = (index + 1).min(usize::from(pcm.length).saturating_sub(1));
        let fraction = position & 0xffff;
        let left_sample = resample::interpolate_sample(
            fixed_to_short(pcm.samples[0][index]),
            fixed_to_short(pcm.samples[0][next_index]),
            fraction,
        );
        let left = eqs[0].process(left_sample);
        let right = if stereo {
            let right_sample = resample::interpolate_sample(
                fixed_to_short(pcm.samples[1][index]),
                fixed_to_short(pcm.samples[1][next_index]),
                fraction,
            );
            eqs[1].process(right_sample)
        } else {
            left
        };
        let word = [
            (left >> 8) as u8,
            left as u8,
            (right >> 8) as u8,
            right as u8,
        ];
        if !unsafe { ring_put(word.as_ptr(), word.len()) } {
            return false;
        }
        position = position.wrapping_add(increment);
    }
    // Keep the fractional sample position across MP3 frames. Otherwise each
    // frame resets the interpolation phase and adds timing jitter.
    *phase = position - (u32::from(pcm.length) << 16);
    true
}

unsafe fn decode_stream(state: *mut PlaybackState) {
    let mut stream = MadStream::default();
    let mut frame = MadFrame::default();
    let mut synth = MadSynth::default();
    let mut eqs = [
        EqState::new(880, 5000, 48_000),
        EqState::new(880, 5000, 48_000),
    ];
    let mut at_end = false;
    let mut guard_ptr: *const u8 = ptr::null();
    let mut phase = 0u32;
    let mut current_speed = 1_000u32;
    let mut previous_level = 0_u32;

    unsafe {
        mad_sys::stream_init(&mut stream);
        mad_sys::frame_init(&mut frame);
        mad_sys::synth_init(&mut synth);
    }

    while PLAYING.load(Ordering::Acquire) {
        if stream.buffer.is_null() || stream.error == mad_sys::MAD_ERROR_BUFLEN {
            let (read_start, read_size, remaining) = if !stream.next_frame.is_null() {
                let remaining = unsafe { stream.bufend.offset_from(stream.next_frame) as usize };
                unsafe { ptr::copy(stream.next_frame, input_ptr(), remaining) };
                (
                    unsafe { input_ptr().add(remaining) },
                    INPUT_BUFFER_SIZE - remaining,
                    remaining,
                )
            } else {
                (input_ptr(), INPUT_BUFFER_SIZE, 0)
            };

            let mut read_size = unsafe { read_source(state, read_start, read_size) };
            if read_size <= 0 {
                guard_ptr = read_start.cast_const();
                unsafe { ptr::write_bytes(read_start, 0, mad_sys::MAD_BUFFER_GUARD) };
                read_size = mad_sys::MAD_BUFFER_GUARD as i32;
                at_end = true;
            }
            unsafe {
                mad_sys::stream_buffer(
                    &mut stream,
                    input_ptr().cast_const(),
                    read_size as u32 + remaining as u32,
                );
            }
        }

        while PLAYING.load(Ordering::Acquire)
            && unsafe { mad_sys::frame_decode(&mut frame, &mut stream) } == 0
        {
            if let Some(filter) = unsafe { (*state).filter } {
                unsafe { filter(&mut stream, &mut frame) };
                FILTER_CALLED.store(true, Ordering::Release);
            }
            unsafe { mad_sys::synth_frame(&mut synth, &frame) };
            if !unsafe {
                resample(
                    &synth.pcm,
                    &mut eqs,
                    &mut phase,
                    &mut current_speed,
                    &mut previous_level,
                )
            } {
                break;
            }
        }

        if mad_recoverable(stream.error) {
            if stream.error != mad_sys::MAD_ERROR_LOSTSYNC || stream.this_frame != guard_ptr {
                continue;
            }
        } else if stream.error != mad_sys::MAD_ERROR_BUFLEN {
            break;
        }

        if at_end {
            if unsafe { (*state).looped } && PLAYING.load(Ordering::Acquire) {
                unsafe {
                    mad_sys::frame_finish(&mut frame);
                    mad_sys::stream_finish(&mut stream);
                    mad_sys::stream_init(&mut stream);
                    mad_sys::frame_init(&mut frame);
                    (*state).buffer_pos = 0;
                }
                guard_ptr = ptr::null();
                at_end = false;
            } else {
                break;
            }
        }
    }

    unsafe {
        mad_sys::frame_finish(&mut frame);
        mad_sys::stream_finish(&mut stream);
    }
}

#[cfg(not(feature = "sndlib"))]
unsafe extern "C" fn dma_callback() {
    let current = CURRENT_OUTPUT.load(Ordering::Acquire);
    audio::audio_init_dma(output_ptr(current) as usize as u32, DMA_BUFFER_SIZE as u32);
    let next = (current + 1) % DMA_BUFFER_COUNT;
    CURRENT_OUTPUT.store(next, Ordering::Release);
    let read = unsafe { ring_get(output_ptr(next), DMA_BUFFER_SIZE) };
    DMA_PLAYING.store(read != 0, Ordering::Release);
}

#[cfg(feature = "sndlib")]
unsafe extern "C" fn snd_callback(_voice: i32) {
    let current = CURRENT_OUTPUT.load(Ordering::Acquire);
    if !PLAYING.load(Ordering::Acquire) {
        let read = unsafe { ring_get(output_ptr(current), DMA_BUFFER_SIZE) };
        DMA_PLAYING.store(read != 0, Ordering::Release);
        return;
    }

    if HAVE_SAMPLES.load(Ordering::Acquire) {
        let status =
            unsafe { snd::add_voice(0, output_ptr(current).cast(), DMA_BUFFER_SIZE as i32) };
        if status == snd::SND_OK {
            HAVE_SAMPLES.store(false, Ordering::Release);
            CURRENT_OUTPUT.store((current + 1) % DMA_BUFFER_COUNT, Ordering::Release);
        }
    }

    let current = CURRENT_OUTPUT.load(Ordering::Acquire);
    let pointer_busy = snd::test_pointer(0, output_ptr(current).cast()) != snd::SND_OK;
    if !(pointer_busy && snd::status_voice(0) != snd::SND_UNUSED)
        && !HAVE_SAMPLES.load(Ordering::Acquire)
    {
        let read = unsafe { ring_get(output_ptr(current), DMA_BUFFER_SIZE) };
        DMA_PLAYING.store(read != 0, Ordering::Release);
        HAVE_SAMPLES.store(true, Ordering::Release);
    }
}

unsafe fn cleanup_stream() {
    while DMA_PLAYING.load(Ordering::Acquire) && PLAYING.load(Ordering::Acquire) {
        let queue = STREAM_QUEUE.load(Ordering::Acquire);
        if queue == lwp::LWP_TQUEUE_NULL {
            break;
        }
        lwp::thread_sleep(queue);
    }
    #[cfg(not(feature = "sndlib"))]
    {
        audio::audio_stop_dma();
        audio::audio_register_dma_callback(None);
    }
    #[cfg(feature = "sndlib")]
    {
        snd::stop_voice(0);
    }
    let queue = STREAM_QUEUE.swap(lwp::LWP_TQUEUE_NULL, Ordering::AcqRel);
    if queue != lwp::LWP_TQUEUE_NULL {
        lwp::close_queue(queue);
    }
    PLAYING.store(false, Ordering::Release);
    THREAD_CREATED.store(false, Ordering::Release);
}

unsafe extern "C" fn stream_play(_arg: *mut c_void) -> *mut c_void {
    PLAYING.store(true, Ordering::Release);
    if STOP_REQUESTED.load(Ordering::Acquire) {
        PLAYING.store(false, Ordering::Release);
        THREAD_CREATED.store(false, Ordering::Release);
        return ptr::null_mut();
    }
    CURRENT_OUTPUT.store(0, Ordering::Release);
    DMA_PLAYING.store(false, Ordering::Release);
    unsafe {
        ptr::write_bytes(
            ptr::addr_of_mut!(OUTPUT_BUFFERS).cast::<u8>(),
            0,
            core::mem::size_of::<DmaBuffers>(),
        );
    }
    ring().reset();
    let mut queue = lwp::LWP_TQUEUE_NULL;
    if unsafe { lwp::init_queue(&mut queue) } < 0 {
        PLAYING.store(false, Ordering::Release);
        THREAD_CREATED.store(false, Ordering::Release);
        return ptr::null_mut();
    }
    STREAM_QUEUE.store(queue, Ordering::Release);
    #[cfg(not(feature = "sndlib"))]
    audio::audio_register_dma_callback(Some(dma_callback));
    unsafe { decode_stream(state_ptr()) };
    unsafe { cleanup_stream() };
    ptr::null_mut()
}

unsafe fn create_stream_thread() -> i32 {
    STOP_REQUESTED.store(false, Ordering::Release);
    let rc = unsafe {
        lwp::create_thread(
            ptr::addr_of_mut!(STREAM_THREAD),
            Some(stream_play),
            ptr::null_mut(),
            ptr::addr_of_mut!(STREAM_STACK.0).cast::<c_void>(),
            STREAM_STACK_SIZE as u32,
            STREAM_PRIORITY,
        )
    };
    if rc < 0 {
        THREAD_CREATED.store(false, Ordering::Release);
        return -1;
    }
    0
}

pub fn mp3_player_init() {
    if INIT_DONE.swap(true, Ordering::AcqRel) {
        return;
    }
    PLAYING.store(false, Ordering::Release);
    THREAD_CREATED.store(false, Ordering::Release);
    STOP_REQUESTED.store(false, Ordering::Release);
    DMA_PLAYING.store(false, Ordering::Release);
    AUDIO_PAUSED.store(false, Ordering::Release);
    BACKEND_ERROR.store(0, Ordering::Release);
    LAST_LEN.store(0, Ordering::Release);
    LAST_CB_DATA.store(ptr::null_mut(), Ordering::Release);
    FILTER_CALLED.store(false, Ordering::Release);
    PLAYBACK_SPEED_PER_MILLE.store(1_000, Ordering::Release);
    #[cfg(not(feature = "sndlib"))]
    {
        audio::audio_init(ptr::null_mut());
        audio::audio_set_dsp_sample_rate(audio::AI_SAMPLERATE_48KHZ as u8);
    }
    #[cfg(feature = "sndlib")]
    {
        // ASND owns the DSP/DMA callback path that consumes the MP3 output
        // ring. Starting a voice before ASND initialization leaves the decode
        // worker blocked once the ring fills.
        snd::init();
        snd::pause(0);
        snd::stop_voice(0);
    }
}

pub fn mp3_player_stop() {
    AUDIO_PAUSED.store(false, Ordering::Release);
    PLAYBACK_SPEED_PER_MILLE.store(1_000, Ordering::Release);
    if !THREAD_CREATED.load(Ordering::Acquire) {
        return;
    }
    STOP_REQUESTED.store(true, Ordering::Release);
    PLAYING.store(false, Ordering::Release);
    let queue = STREAM_QUEUE.load(Ordering::Acquire);
    if queue != lwp::LWP_TQUEUE_NULL {
        lwp::thread_signal(queue);
    }
    let thread = unsafe { STREAM_THREAD };
    if thread != lwp::LWP_THREAD_NULL {
        unsafe { lwp::join_thread(thread, ptr::null_mut()) };
        unsafe { STREAM_THREAD = lwp::LWP_THREAD_NULL };
    }
    THREAD_CREATED.store(false, Ordering::Release);
    unsafe { *state_ptr() = PlaybackState::new() };
}

pub fn mp3_player_pause(paused: bool) -> i32 {
    AUDIO_PAUSED.store(paused, Ordering::Release);
    #[cfg(feature = "sndlib")]
    {
        snd::pause_voice(0, i32::from(paused))
    }
    #[cfg(not(feature = "sndlib"))]
    {
        if paused {
            audio::audio_stop_dma();
        } else {
            audio::audio_start_dma();
        }
        0
    }
}

pub fn mp3_player_is_playing() -> Bool {
    PLAYING.load(Ordering::Acquire) as Bool
}

pub fn mp3_player_last_error() -> i32 {
    BACKEND_ERROR.load(Ordering::Acquire)
}

pub fn mp3_player_volume(volume: u32) {
    let volume = volume.min(255);
    VOLUME.store(volume, Ordering::Release);
    #[cfg(feature = "sndlib")]
    snd::change_volume_voice(0, volume as i32, volume as i32);
}

/// Change music speed without restarting playback. 1000 = normal speed;
/// 1400 = 40% faster. This changes pitch as well as tempo.
pub fn mp3_player_set_speed_per_mille(speed: u32) {
    PLAYBACK_SPEED_PER_MILLE.store(speed.clamp(1_000, 1_400), Ordering::Release);
}

/// # Safety
///
/// `buffer` must be readable for `len` bytes and remain valid until playback stops or completes; callbacks must uphold their contracts.
pub unsafe fn mp3_player_play_buffer(
    buffer: *const c_void,
    len: i32,
    filterfunc: Mp3FilterFn,
) -> i32 {
    unsafe { play_buffer_with_repeat(buffer, len, filterfunc, false) }
}

/// Start a buffer and restart it inside the decoder worker when it reaches EOF.
/// This keeps the audio voice and output ring active between track repeats.
///
/// # Safety
///
/// `buffer` must be readable for `len` bytes and remain valid until playback
/// stops; callbacks must uphold their contracts.
pub unsafe fn mp3_player_play_buffer_looped(
    buffer: *const c_void,
    len: i32,
    filterfunc: Mp3FilterFn,
) -> i32 {
    unsafe { play_buffer_with_repeat(buffer, len, filterfunc, true) }
}

unsafe fn play_buffer_with_repeat(
    buffer: *const c_void,
    len: i32,
    filterfunc: Mp3FilterFn,
    looped: bool,
) -> i32 {
    if buffer.is_null() || len <= 0 {
        return -1;
    }
    if THREAD_CREATED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return -1;
    }
    LAST_LEN.store(len as u32, Ordering::Release);
    BACKEND_ERROR.store(0, Ordering::Release);
    FILTER_CALLED.store(false, Ordering::Release);

    let state = state_ptr();
    unsafe {
        *state = PlaybackState::new();
        (*state).input_kind = InputKind::Buffer;
        (*state).buffer = buffer.cast();
        (*state).buffer_len = len as usize;
        (*state).filter = filterfunc;
        (*state).looped = looped;
        create_stream_thread()
    }
}

/// # Safety
///
/// `cb_data` must remain valid for every later `reader` call until playback stops or completes; callbacks must uphold their contracts.
pub unsafe fn mp3_player_play_file(
    cb_data: *mut c_void,
    reader: Mp3ReadFn,
    filterfunc: Mp3FilterFn,
) -> i32 {
    if reader.is_none() {
        return -1;
    }
    if THREAD_CREATED
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return -1;
    }
    LAST_CB_DATA.store(cb_data, Ordering::Release);
    LAST_LEN.store(0, Ordering::Release);
    BACKEND_ERROR.store(0, Ordering::Release);
    FILTER_CALLED.store(false, Ordering::Release);

    let state = state_ptr();
    unsafe {
        *state = PlaybackState::new();
        (*state).input_kind = InputKind::Reader;
        (*state).reader = reader;
        (*state).reader_data = cb_data;
        (*state).filter = filterfunc;
        create_stream_thread()
    }
}

pub fn init() {
    mp3_player_init();
}

pub fn stop() {
    mp3_player_stop();
}

pub fn is_playing() -> bool {
    mp3_player_is_playing() != 0
}

pub fn last_error() -> i32 {
    mp3_player_last_error()
}

pub fn volume(volume: u32) {
    mp3_player_volume(volume);
}

pub fn set_speed_per_mille(speed: u32) {
    mp3_player_set_speed_per_mille(speed);
}

pub fn pause(paused: bool) -> i32 {
    mp3_player_pause(paused)
}

/// # Safety
///
/// The `mp3_player_play_buffer` requirements apply for the full asynchronous playback lifetime.
pub unsafe fn play_buffer(buffer: *const c_void, len: i32, filterfunc: Mp3FilterFn) -> i32 {
    unsafe { mp3_player_play_buffer(buffer, len, filterfunc) }
}

/// # Safety
///
/// The `mp3_player_play_buffer_looped` requirements apply for the full
/// asynchronous playback lifetime.
pub unsafe fn play_buffer_looped(buffer: *const c_void, len: i32, filterfunc: Mp3FilterFn) -> i32 {
    unsafe { mp3_player_play_buffer_looped(buffer, len, filterfunc) }
}

/// # Safety
///
/// The `mp3_player_play_file` requirements apply for the full asynchronous playback lifetime.
pub unsafe fn play_file(cb_data: *mut c_void, reader: Mp3ReadFn, filterfunc: Mp3FilterFn) -> i32 {
    unsafe { mp3_player_play_file(cb_data, reader, filterfunc) }
}
