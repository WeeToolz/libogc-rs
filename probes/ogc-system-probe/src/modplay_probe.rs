use core::ffi::c_void;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicU32, Ordering};

use modplay_capi::{self as capi, Mod, ModPlay};
use modplay_capi::{
    mod_play_init, mod_play_set_mod, mod_play_start, mod_play_stop, mod_play_unload,
};
use ogc_lwp as lwp;

use crate::probe_support::{self, ProbeLogger};

const MOD_IMAGE_SIZE: usize = 2112;
const AUDIO_MIX_BYTES: u32 = 5760;
const MIX_WAIT_FRAMES: u32 = 240;

static MIX_CALLBACK_COUNT: AtomicU32 = AtomicU32::new(0);
static LAST_MIX_BYTES: AtomicU32 = AtomicU32::new(0);
static MAIN_THREAD: AtomicU32 = AtomicU32::new(u32::MAX);
static MIX_THREAD: AtomicU32 = AtomicU32::new(u32::MAX);

static mut MODPLAY: MaybeUninit<ModPlay> = MaybeUninit::uninit();
static mut MOD_IMAGE: [u8; MOD_IMAGE_SIZE] = [0; MOD_IMAGE_SIZE];

unsafe extern "C" fn record_mix_callback(mod_ptr: *mut c_void) {
    if mod_ptr.is_null() {
        return;
    }
    let module = unsafe { &*(mod_ptr.cast::<Mod>()) };
    LAST_MIX_BYTES.store(module.mixingbuflen.max(0) as u32, Ordering::Release);
    MIX_THREAD.store(lwp::current_thread().raw(), Ordering::Release);
    MIX_CALLBACK_COUNT.fetch_add(1, Ordering::AcqRel);
}

fn wait_for_mix_count(target: u32) -> bool {
    for _ in 0..MIX_WAIT_FRAMES {
        if MIX_CALLBACK_COUNT.load(Ordering::Acquire) >= target {
            return true;
        }
        probe_support::wait_vsync();
    }
    MIX_CALLBACK_COUNT.load(Ordering::Acquire) >= target
}

unsafe fn prepare_minimal_mod_image() -> *mut u8 {
    let image = core::ptr::addr_of_mut!(MOD_IMAGE).cast::<u8>();
    unsafe {
        core::ptr::write_bytes(image, 0, MOD_IMAGE_SIZE);
        *image.add(950) = 1;
        core::ptr::copy_nonoverlapping(b"M.K.".as_ptr(), image.add(1080), 4);
    }
    image
}

fn stop_player(player: *mut ModPlay) -> i32 {
    unsafe { mod_play_stop(player) }
}

pub fn run(logger: &mut ProbeLogger) {
    logger.section("MODPLAY DMA WORKER PROBE");
    MIX_CALLBACK_COUNT.store(0, Ordering::Release);
    LAST_MIX_BYTES.store(0, Ordering::Release);
    MIX_THREAD.store(u32::MAX, Ordering::Release);
    let main_thread = lwp::current_thread().raw();
    MAIN_THREAD.store(main_thread, Ordering::Release);

    let player = core::ptr::addr_of_mut!(MODPLAY).cast::<ModPlay>();
    unsafe { mod_play_init(player) };
    let image = unsafe { prepare_minimal_mod_image() };
    let set_mod_rc = unsafe { mod_play_set_mod(player, image.cast()) };
    logger.note_i32("load caller-owned MOD image", set_mod_rc);
    if set_mod_rc != 0 {
        logger.record("MOD image is accepted", false);
        unsafe { mod_play_unload(player) };
        return;
    }

    probe_modplay_state_apis(logger, player, image);
    unsafe { mod_play_init(player) };
    let restored_mod_rc = unsafe { mod_play_set_mod(player, image.cast()) };
    logger.record(
        "C API reloads caller-owned MOD after state checks",
        restored_mod_rc == 0,
    );
    if restored_mod_rc != 0 {
        unsafe { mod_play_unload(player) };
        return;
    }

    unsafe {
        (*player).mod_.callback = Some(record_mix_callback);
    }
    let start_rc = unsafe { mod_play_start(player) };
    logger.note_i32("first playback start result", start_rc);
    logger.record("playback starts", start_rc == 0);
    logger.record("public player state reports active playback", unsafe {
        (*player).playing != 0
    });
    if start_rc != 0 {
        unsafe { mod_play_unload(player) };
        return;
    }

    let first_mix = wait_for_mix_count(1);
    logger.record("DMA signal wakes the production mixer", first_mix);
    let first_len = LAST_MIX_BYTES.load(Ordering::Acquire);
    logger.note_u32("production mix callback length", first_len);
    logger.record(
        "production mix uses 5760 bytes",
        first_len == AUDIO_MIX_BYTES,
    );
    let first_thread = MIX_THREAD.load(Ordering::Acquire);
    logger.note_u32("main LWP thread", main_thread);
    logger.note_u32("mixer LWP thread", first_thread);
    logger.record(
        "production mixer runs outside the main thread",
        first_thread != u32::MAX && first_thread != main_thread,
    );

    let stop_rc = stop_player(player);
    logger.note_i32("first playback stop result", stop_rc);
    logger.record("stop joins the worker", stop_rc == 0);
    logger.record("public player state reports stopped playback", unsafe {
        (*player).playing == 0
    });
    let stopped_count = MIX_CALLBACK_COUNT.load(Ordering::Acquire);
    for _ in 0..3 {
        probe_support::wait_vsync();
    }
    logger.record(
        "mix callback count stays fixed after stop",
        MIX_CALLBACK_COUNT.load(Ordering::Acquire) == stopped_count,
    );

    let before_restart = MIX_CALLBACK_COUNT.load(Ordering::Acquire);
    let restart_rc = unsafe { mod_play_start(player) };
    logger.note_i32("restart result", restart_rc);
    logger.record("playback restarts after worker join", restart_rc == 0);
    if restart_rc == 0 {
        let restarted_mix = wait_for_mix_count(before_restart.saturating_add(1));
        logger.record("restarted worker mixes after DMA signal", restarted_mix);
        logger.record(
            "restarted mix uses 5760 bytes",
            LAST_MIX_BYTES.load(Ordering::Acquire) == AUDIO_MIX_BYTES,
        );
        logger.record(
            "restarted mixer runs outside the main thread",
            MIX_THREAD.load(Ordering::Acquire) != main_thread,
        );
        let restart_stop_rc = stop_player(player);
        logger.note_i32("restart stop result", restart_stop_rc);
        logger.record("restarted worker joins", restart_stop_rc == 0);
        let count_after_restart_stop = MIX_CALLBACK_COUNT.load(Ordering::Acquire);
        for _ in 0..3 {
            probe_support::wait_vsync();
        }
        logger.record(
            "restarted mix count stays fixed after stop",
            MIX_CALLBACK_COUNT.load(Ordering::Acquire) == count_after_restart_stop,
        );
    }

    unsafe { mod_play_unload(player) };
    let image_still_valid = unsafe {
        let image = core::ptr::addr_of!(MOD_IMAGE).cast::<u8>();
        core::slice::from_raw_parts(image.add(1080), 4) == b"M.K."
    };
    logger.record(
        "caller-owned MOD bytes remain valid after unload",
        image_still_valid,
    );
}

fn probe_modplay_state_apis(logger: &mut ProbeLogger, player: *mut ModPlay, image: *mut u8) {
    let layout = capi::layout_snapshot();
    logger.record(
        "modplay state ABI layout",
        layout[0].size == 52
            && layout[1].size == 3112
            && layout[2].size == 24
            && layout[3].size == 3164
            && layout.iter().all(|item| item.align == 4),
    );

    // SAFETY: `player` points to the initialized static ModPlay object.
    unsafe { capi::init_state(&mut *player) };
    let init_ok = unsafe {
        (*player).playing == 0
            && (*player).paused == 0
            && (*player).playfreq == 48_000
            && (*player).stereo == 1
    };
    // SAFETY: `image` points to the complete caller-owned MOD image, and the
    // caller keeps it alive until the player is unloaded.
    let set_mod_rc = unsafe { capi::set_mod_state(&mut *player, image) };
    logger.record(
        "modplay state init and caller-owned load",
        init_ok && set_mod_rc == 0,
    );
    if set_mod_rc != 0 {
        return;
    }

    let sfx_rc = unsafe { capi::alloc_sfx_channels_state(&mut *player, 1) };
    let (note_values_are_zero, voice_note_rc, mod_note_rc) = unsafe {
        let mod_ = &mut (*player).mod_;
        capi::mod_start_state(mod_);
        let values = capi::get_note_value(mod_, 0, 0) == 0
            && capi::get_instr_value(mod_, 0, 0) == 0
            && capi::get_effect_value(mod_, 0, 0) == 0
            && capi::get_effect_op_value(mod_, 0, 0) == 0;
        (
            values,
            capi::trigger_note_on_voice_state(mod_, 1, 0xff, 428, 31),
            capi::mod_trigger_note_state(mod_, 0, 0xff, 572, 47),
        )
    };
    let mod_note_changed =
        unsafe { (*player).mod_.chanfreq[4] == 572 && (*player).mod_.volume[4] == 47 };
    let player_note_rc = unsafe { capi::trigger_note_state(&mut *player, 0, 0xff, 856, 55) };
    let notes_changed = unsafe {
        let mod_ = &(*player).mod_;
        mod_.chanfreq[1] == 428
            && mod_.volume[1] == 31
            && mod_.chanfreq[4] == 856
            && mod_.volume[4] == 55
    };
    logger.note_i32("state SFX allocation result", sfx_rc);
    logger.note_i32("state channel count", unsafe {
        (*player).mod_.num_channels
    });
    logger.note_i32("state voice count", unsafe { (*player).mod_.num_voices });
    logger.note_i32("state voice note result", voice_note_rc);
    logger.note_i32("state module note result", mod_note_rc);
    logger.note_i32("state player note result", player_note_rc);
    logger.note_u32("state note values observed", u32::from(notes_changed));
    logger.record(
        "modplay state SFX allocation, pattern reads, and note routing",
        sfx_rc == 0
            && unsafe { (*player).mod_.num_channels == (*player).mod_.num_voices + 1 }
            && note_values_are_zero
            && voice_note_rc == 0
            && mod_note_rc == 0
            && mod_note_changed
            && player_note_rc == 0
            && notes_changed,
    );

    unsafe { capi::set_volume_state(&mut *player, 80, -4) };
    let volume_clamped =
        unsafe { (*player).mod_.musicvolume == 64 && (*player).mod_.sfxvolume == 0 };
    let frequency_ok = unsafe {
        capi::set_frequency_state(&mut *player, 32_000) == 0
            && (*player).playfreq == 32_000
            && capi::set_frequency_state(&mut *player, 44_100) == -1
            && (*player).playfreq == 32_000
            && capi::set_frequency_state(&mut *player, 48_000) == 0
            && (*player).playfreq == 48_000
    };
    unsafe { capi::set_stereo_state(&mut *player, 0) };
    let mono = unsafe { (*player).stereo == 0 };
    unsafe { capi::set_stereo_state(&mut *player, 1) };
    logger.record(
        "modplay state volume, frequency validation, and stereo format",
        volume_clamped && frequency_ok && mono && unsafe { (*player).stereo == 1 },
    );

    let mut pcm = [0xa5u8; 64];
    unsafe { (*player).paused = 1 };
    let paused_mix = unsafe { capi::mix_callback_state(&mut *player, &mut pcm) };
    let paused_output_is_silent = paused_mix == 0 && pcm.iter().all(|sample| *sample == 0);
    unsafe {
        (*player).paused = 0;
        (*player).mod_.mixingbuf = pcm.as_mut_ptr();
        (*player).mod_.mixingbuflen = pcm.len() as i32;
    }
    let player_state = unsafe { capi::mod_player_state(&mut (*player).mod_) };
    let callback_time = capi::mixing_time_state();
    let request_time = capi::request_time_state();
    logger.record(
        "modplay state mixer and timing getters",
        paused_output_is_silent && player_state == 0 && callback_time == 0 && request_time == 0,
    );

    unsafe { (*player).mod_.callback = Some(record_mix_callback) };
    let callback_before = MIX_CALLBACK_COUNT.load(Ordering::Acquire);
    let start_rc = unsafe { capi::start_state(&mut *player) };
    let started = start_rc == 0 && unsafe { (*player).playing != 0 };
    let mixed = started && wait_for_mix_count(callback_before.saturating_add(1));
    let pause_rc = unsafe { capi::pause_state(&mut *player, 1) };
    let paused = pause_rc == 0 && unsafe { (*player).paused == 1 };
    let resume_rc = unsafe { capi::pause_state(&mut *player, 0) };
    let resumed = resume_rc == 0 && unsafe { (*player).paused == 0 };
    let stop_rc = if started {
        unsafe { capi::stop_state(&mut *player) }
    } else {
        -1
    };
    logger.note_i32("state worker start result", start_rc);
    logger.note_u32("state worker mixed", u32::from(mixed));
    logger.note_i32("state pause result", pause_rc);
    logger.note_i32("state resume result", resume_rc);
    logger.note_i32("state worker stop result", stop_rc);
    logger.record(
        "modplay state worker start, pause, resume, and stop",
        started && mixed && paused && resumed && stop_rc == 0 && unsafe { (*player).playing == 0 },
    );

    let unload_rc = unsafe { capi::try_unload_state(&mut *player) };
    unsafe { capi::unload_state(&mut *player) };
    logger.record(
        "modplay state fallible and void unload",
        unload_rc == 0 && unsafe { (*player).playing == 0 && (*player).paused == 0 },
    );
}
