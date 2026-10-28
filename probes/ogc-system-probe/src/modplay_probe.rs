use core::ffi::c_void;
use core::mem::MaybeUninit;
use core::sync::atomic::{AtomicU32, Ordering};

use modplay_capi::{Mod, ModPlay};
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
