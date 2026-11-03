//! Probe the MP3 player entry points with one fixed MPEG Layer III fixture.

use core::ffi::c_void;
use core::sync::atomic::{AtomicU32, AtomicUsize, Ordering};

use crate::probe_support::ProbeLogger;

const MP3: &[u8] = include_bytes!(env!(
    "LIBOGC_MAD_PROBE_MP3",
    "set LIBOGC_MAD_PROBE_MP3 to the external Layer III fixture; see docs/dependencies.md"
));
const MIN_FILTER_FRAMES: u32 = 20;
const MAX_WAIT_FRAMES: usize = 600;

type LoopPlayFn = unsafe fn(*const c_void, i32, mp3player_sys::Mp3FilterFn) -> i32;
type BufferPlayFn = LoopPlayFn;
type FilePlayFn =
    unsafe fn(*mut c_void, mp3player_sys::Mp3ReadFn, mp3player_sys::Mp3FilterFn) -> i32;
type StopFn = fn();

static FILTER_FRAMES: AtomicU32 = AtomicU32::new(0);
static AUDIO_STAGE: AtomicU32 = AtomicU32::new(0);
static AUDIO_STAGE_CALLS: [AtomicU32; 5] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];
static AUDIO_STAGE_ENERGY: [AtomicU32; 5] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];
static AUDIO_STAGE_HASH: [AtomicU32; 5] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];
static AUDIO_CAPTURE_ERRORS: AtomicU32 = AtomicU32::new(0);
static PREVIOUS_AUDIO_CALLBACK: AtomicUsize = AtomicUsize::new(0);

struct ReaderState {
    offset: AtomicUsize,
    calls: AtomicU32,
}

impl ReaderState {
    const fn new() -> Self {
        Self {
            offset: AtomicUsize::new(0),
            calls: AtomicU32::new(0),
        }
    }

    fn reset(&self) {
        self.offset.store(0, Ordering::Release);
        self.calls.store(0, Ordering::Release);
    }
}

static READER_STATE: ReaderState = ReaderState::new();

pub fn run(logger: &mut ProbeLogger) {
    logger.section("MP3 PLAYER API PROBE");
    logger.note(&alloc::format!(
        "fixture bytes={} layer=III rate=44100 stereo",
        MP3.len()
    ));
    if MP3.is_empty() || MP3.len() > i32::MAX as usize {
        logger.record("fixed MP3 fixture has a valid length", false);
        return;
    }
    logger.record("fixed MP3 fixture has a valid length", true);

    mp3player_sys::mp3_player_init();
    mp3player_sys::init();
    logger.record(
        "direct and alias init leave playback stopped",
        mp3player_sys::mp3_player_is_playing() == 0
            && !mp3player_sys::is_playing()
            && mp3player_sys::mp3_player_last_error() == 0
            && mp3player_sys::last_error() == 0,
    );

    let null = core::ptr::null();
    let null_start = unsafe { mp3player_sys::mp3_player_play_buffer(null, MP3.len() as i32, None) };
    let null_alias = unsafe { mp3player_sys::play_buffer(null, MP3.len() as i32, None) };
    let null_loop =
        unsafe { mp3player_sys::mp3_player_play_buffer_looped(null, MP3.len() as i32, None) };
    let null_loop_alias =
        unsafe { mp3player_sys::play_buffer_looped(null, MP3.len() as i32, None) };
    let empty_start =
        unsafe { mp3player_sys::mp3_player_play_buffer(MP3.as_ptr().cast(), 0, None) };
    logger.record(
        "direct and alias buffer starts reject null and empty input",
        null_start == -1
            && null_alias == -1
            && null_loop == -1
            && null_loop_alias == -1
            && empty_start == -1
            && !mp3player_sys::is_playing(),
    );

    let no_reader =
        unsafe { mp3player_sys::mp3_player_play_file(core::ptr::null_mut(), None, None) };
    let no_reader_alias = unsafe { mp3player_sys::play_file(core::ptr::null_mut(), None, None) };
    logger.record(
        "direct and alias file starts reject a missing reader",
        no_reader == -1 && no_reader_alias == -1 && !mp3player_sys::is_playing(),
    );

    mp3player_sys::mp3_player_volume(255);
    mp3player_sys::volume(u32::MAX);
    mp3player_sys::mp3_player_set_speed_per_mille(1_000);
    mp3player_sys::set_speed_per_mille(1_400);
    probe_volume_output(logger);
    probe_speed_output(logger);

    probe_buffer_playback(logger, mp3player_sys::mp3_player_play_buffer, "direct");
    probe_buffer_playback(logger, mp3player_sys::play_buffer, "alias");
    probe_looped_playback(
        logger,
        mp3player_sys::mp3_player_play_buffer_looped,
        mp3player_sys::mp3_player_stop,
        "direct",
    );
    probe_looped_playback(
        logger,
        mp3player_sys::play_buffer_looped,
        mp3player_sys::stop,
        "alias",
    );
    probe_reader_playback(logger, mp3player_sys::mp3_player_play_file, "direct");
    probe_reader_playback(logger, mp3player_sys::play_file, "alias");
}

fn probe_volume_output(logger: &mut ProbeLogger) {
    for value in &AUDIO_STAGE_CALLS {
        value.store(0, Ordering::Release);
    }
    for value in &AUDIO_STAGE_ENERGY {
        value.store(0, Ordering::Release);
    }
    AUDIO_CAPTURE_ERRORS.store(0, Ordering::Release);

    let previous = ogc_audio::register_dma_callback(Some(capture_mp3_output));
    PREVIOUS_AUDIO_CALLBACK.store(
        previous.map_or(0, |callback| callback as usize),
        Ordering::Release,
    );
    FILTER_FRAMES.store(0, Ordering::Release);
    let start = unsafe {
        mp3player_sys::mp3_player_play_buffer_looped(
            MP3.as_ptr().cast(),
            MP3.len() as i32,
            Some(record_filter),
        )
    };
    let started = start == 0 && wait_for_filter(logger, 4);
    if started {
        AUDIO_STAGE.store(1, Ordering::Release);
        mp3player_sys::mp3_player_volume(255);
        let full_volume = wait_for_audio_stage(logger, 1, 3);

        AUDIO_STAGE.store(2, Ordering::Release);
        mp3player_sys::volume(0);
        let muted = wait_for_audio_stage(logger, 2, 3);

        AUDIO_STAGE.store(3, Ordering::Release);
        mp3player_sys::volume(u32::MAX);
        let clamped = wait_for_audio_stage(logger, 3, 3);

        let full_energy = AUDIO_STAGE_ENERGY[0].load(Ordering::Acquire);
        let mute_energy = AUDIO_STAGE_ENERGY[1].load(Ordering::Acquire);
        let clamp_energy = AUDIO_STAGE_ENERGY[2].load(Ordering::Acquire);
        let capture_errors = AUDIO_CAPTURE_ERRORS.load(Ordering::Acquire);
        logger.note(&alloc::format!(
            "MP3 DMA capture full_energy={full_energy} muted_energy={mute_energy} clamped_energy={clamp_energy} capture_errors={capture_errors}"
        ));
        logger.record(
            "direct and alias MP3 volume setters change audible DMA samples",
            full_volume
                && muted
                && clamped
                && full_energy > 0
                && mute_energy == 0
                && clamp_energy > 0
                && capture_errors == 0,
        );
    } else {
        logger.record(
            "direct and alias MP3 volume setters change audible DMA samples",
            false,
        );
    }

    AUDIO_STAGE.store(0, Ordering::Release);
    mp3player_sys::mp3_player_stop();
    ogc_audio::register_dma_callback(previous);
    PREVIOUS_AUDIO_CALLBACK.store(0, Ordering::Release);
}

fn probe_speed_output(logger: &mut ProbeLogger) {
    for stage in 3..5 {
        AUDIO_STAGE_CALLS[stage].store(0, Ordering::Release);
        AUDIO_STAGE_ENERGY[stage].store(0, Ordering::Release);
        AUDIO_STAGE_HASH[stage].store(0, Ordering::Release);
    }
    AUDIO_CAPTURE_ERRORS.store(0, Ordering::Release);
    let previous = ogc_audio::register_dma_callback(Some(capture_mp3_output));
    PREVIOUS_AUDIO_CALLBACK.store(
        previous.map_or(0, |callback| callback as usize),
        Ordering::Release,
    );

    mp3player_sys::mp3_player_set_speed_per_mille(1_000);
    let normal_started = start_speed_capture(logger, 4);
    let normal_hash = AUDIO_STAGE_HASH[3].load(Ordering::Acquire);
    let normal_energy = AUDIO_STAGE_ENERGY[3].load(Ordering::Acquire);
    mp3player_sys::mp3_player_stop();

    mp3player_sys::set_speed_per_mille(1_400);
    let fast_started = start_speed_capture(logger, 5);
    let fast_hash = AUDIO_STAGE_HASH[4].load(Ordering::Acquire);
    let fast_energy = AUDIO_STAGE_ENERGY[4].load(Ordering::Acquire);
    let capture_errors = AUDIO_CAPTURE_ERRORS.load(Ordering::Acquire);
    logger.note(&alloc::format!(
        "MP3 speed DMA normal={normal_hash:08x}/{normal_energy} fast={fast_hash:08x}/{fast_energy} capture_errors={capture_errors}"
    ));
    logger.record(
        "direct and alias MP3 speed setters change captured PCM",
        normal_started
            && fast_started
            && normal_energy > 0
            && fast_energy > 0
            && normal_hash != fast_hash
            && capture_errors == 0,
    );

    AUDIO_STAGE.store(0, Ordering::Release);
    mp3player_sys::mp3_player_stop();
    ogc_audio::register_dma_callback(previous);
    PREVIOUS_AUDIO_CALLBACK.store(0, Ordering::Release);
}

fn start_speed_capture(logger: &mut ProbeLogger, stage: usize) -> bool {
    AUDIO_STAGE.store(0, Ordering::Release);
    mp3player_sys::mp3_player_volume(255);
    FILTER_FRAMES.store(0, Ordering::Release);
    let start = unsafe {
        mp3player_sys::mp3_player_play_buffer_looped(
            MP3.as_ptr().cast(),
            MP3.len() as i32,
            Some(record_filter),
        )
    };
    let ready = start == 0 && wait_for_filter(logger, MIN_FILTER_FRAMES);
    if !ready {
        return false;
    }
    AUDIO_STAGE.store(stage as u32, Ordering::Release);
    wait_for_audio_stage(logger, stage, 4)
}

fn wait_for_audio_stage(logger: &mut ProbeLogger, stage: usize, required: u32) -> bool {
    for _ in 0..MAX_WAIT_FRAMES {
        if AUDIO_STAGE_CALLS[stage - 1].load(Ordering::Acquire) >= required {
            return true;
        }
        logger.display().wait_vsync();
    }
    false
}

unsafe extern "C" fn capture_mp3_output() {
    let previous = PREVIOUS_AUDIO_CALLBACK.load(Ordering::Acquire);
    if previous != 0 {
        let callback: unsafe extern "C" fn() = unsafe { core::mem::transmute(previous) };
        unsafe { callback() };
    }

    let stage = AUDIO_STAGE.load(Ordering::Acquire);
    if !(1..=5).contains(&stage) {
        return;
    }
    let snapshot = ogc_audio::snapshot();
    let address = snapshot.dma_start_addr;
    let length = snapshot.dma_length as usize;
    if address >= 0x0400_0000 || length == 0 || length > 4096 || !length.is_multiple_of(2) {
        AUDIO_CAPTURE_ERRORS.fetch_add(1, Ordering::Relaxed);
        return;
    }

    let samples =
        unsafe { core::slice::from_raw_parts((address | 0x8000_0000) as *const u8, length) };
    let energy = samples
        .as_chunks::<2>()
        .0
        .iter()
        .map(|sample| i16::from_be_bytes(*sample).unsigned_abs() as u32)
        .fold(0_u32, u32::wrapping_add);
    let mut hash = 2_166_136_261_u32;
    for &byte in samples {
        hash = (hash ^ u32::from(byte)).wrapping_mul(16_777_619);
    }
    let index = stage as usize - 1;
    AUDIO_STAGE_ENERGY[index].fetch_add(energy, Ordering::Relaxed);
    let sequence = AUDIO_STAGE_CALLS[index].fetch_add(1, Ordering::AcqRel);
    AUDIO_STAGE_HASH[index].fetch_xor(hash.rotate_left(sequence), Ordering::Relaxed);
}

fn probe_buffer_playback(logger: &mut ProbeLogger, play: BufferPlayFn, label: &str) {
    FILTER_FRAMES.store(0, Ordering::Release);
    let start = unsafe { play(MP3.as_ptr().cast(), MP3.len() as i32, Some(record_filter)) };
    let started = start == 0 && wait_for_filter(logger, 1);
    let mut completed = false;
    if started {
        for _ in 0..MAX_WAIT_FRAMES {
            if !mp3player_sys::is_playing() {
                completed = FILTER_FRAMES.load(Ordering::Acquire) >= MIN_FILTER_FRAMES;
                break;
            }
            logger.display().wait_vsync();
        }
    }
    let clean = mp3player_sys::mp3_player_last_error() == 0 && mp3player_sys::last_error() == 0;
    mp3player_sys::mp3_player_stop();
    let stopped = !mp3player_sys::is_playing();
    mp3player_sys::stop();

    logger.note(&alloc::format!(
        "{label} buffer start={start} filter_frames={} completed={completed}",
        FILTER_FRAMES.load(Ordering::Acquire)
    ));
    logger.record(
        &alloc::format!("{label} buffer playback decodes the fixture and completes"),
        started && completed && clean,
    );
    logger.record(
        "direct and alias stop leave playback inactive",
        stopped && mp3player_sys::mp3_player_is_playing() == 0,
    );
}

fn probe_looped_playback(logger: &mut ProbeLogger, play: LoopPlayFn, stop: StopFn, label: &str) {
    FILTER_FRAMES.store(0, Ordering::Release);
    let start = unsafe { play(MP3.as_ptr().cast(), MP3.len() as i32, Some(record_filter)) };
    let progressed = start == 0 && wait_for_filter(logger, MIN_FILTER_FRAMES);
    let active = mp3player_sys::mp3_player_is_playing() != 0 && mp3player_sys::is_playing();
    let pause_status = mp3player_sys::mp3_player_pause(true);
    let voice_paused = asnd_sys::status_voice(0) == asnd_sys::SND_WAITING;
    let resume_status = mp3player_sys::pause(false);
    let voice_resumed = asnd_sys::status_voice(0) == asnd_sys::SND_WORKING;
    stop();
    let stopped = !mp3player_sys::is_playing();
    let stopped_frames = FILTER_FRAMES.load(Ordering::Acquire);
    for _ in 0..2 {
        logger.display().wait_vsync();
    }

    logger.note(&alloc::format!(
        "{label} looped start={start} filter_frames={stopped_frames} active={active}"
    ));
    logger.record(
        &alloc::format!("{label} looped playback repeats the fixture until stop"),
        progressed && active && stopped && FILTER_FRAMES.load(Ordering::Acquire) == stopped_frames,
    );
    if label == "direct" {
        logger.record(
            "direct pause and alias resume change ASND voice state",
            pause_status == asnd_sys::SND_OK
                && voice_paused
                && resume_status == asnd_sys::SND_OK
                && voice_resumed,
        );
    }
}

fn probe_reader_playback(logger: &mut ProbeLogger, play: FilePlayFn, label: &str) {
    FILTER_FRAMES.store(0, Ordering::Release);
    READER_STATE.reset();
    let start = unsafe {
        play(
            (&READER_STATE as *const ReaderState).cast_mut().cast(),
            Some(read_fixture),
            Some(record_filter),
        )
    };
    let started = start == 0 && wait_for_filter(logger, 1);
    let mut completed = false;
    if started {
        for _ in 0..MAX_WAIT_FRAMES {
            if !mp3player_sys::is_playing() {
                completed = FILTER_FRAMES.load(Ordering::Acquire) >= MIN_FILTER_FRAMES;
                break;
            }
            logger.display().wait_vsync();
        }
    }
    mp3player_sys::mp3_player_stop();
    let reader_calls = READER_STATE.calls.load(Ordering::Acquire);
    let frames = FILTER_FRAMES.load(Ordering::Acquire);
    let clean = !mp3player_sys::is_playing() && mp3player_sys::last_error() == 0;

    logger.note(&alloc::format!(
        "{label} file start={start} reader_calls={reader_calls} filter_frames={frames} completed={completed}"
    ));
    logger.record(
        &alloc::format!("{label} file reader supplies fixture bytes and playback completes"),
        start == 0 && reader_calls > 0 && completed && clean,
    );
}

fn wait_for_filter(logger: &mut ProbeLogger, required: u32) -> bool {
    for _ in 0..MAX_WAIT_FRAMES {
        if FILTER_FRAMES.load(Ordering::Acquire) >= required {
            return true;
        }
        logger.display().wait_vsync();
    }
    false
}

unsafe extern "C" fn record_filter(
    _stream: *mut mad_sys::MadStream,
    _frame: *mut mad_sys::MadFrame,
) {
    FILTER_FRAMES.fetch_add(1, Ordering::AcqRel);
}

unsafe extern "C" fn read_fixture(cb_data: *mut c_void, buffer: *mut c_void, len: i32) -> i32 {
    if cb_data.is_null() || buffer.is_null() || len <= 0 {
        return -1;
    }
    let state = unsafe { &*cb_data.cast::<ReaderState>() };
    state.calls.fetch_add(1, Ordering::AcqRel);
    let offset = state.offset.load(Ordering::Acquire);
    let amount = MP3.len().saturating_sub(offset).min(len as usize);
    if amount == 0 {
        return 0;
    }
    unsafe {
        core::ptr::copy_nonoverlapping(MP3.as_ptr().add(offset), buffer.cast(), amount);
    }
    state.offset.store(offset + amount, Ordering::Release);
    amount as i32
}
