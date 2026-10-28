#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::sync::atomic::{AtomicBool, Ordering};

#[cfg(target_arch = "powerpc")]
use ogc_irq::{self as irq, IRQ_AI, IRQ_DSP_AI};
#[cfg(target_arch = "powerpc")]
core::arch::global_asm!(
    ".section .text\n\
     .align 2\n\
     .global ogc_audio_call_on_stack\n\
     .type ogc_audio_call_on_stack,@function\n\
     ogc_audio_call_on_stack:\n\
     mflr 0\n\
     stw 0,4(1)\n\
     stwu 1,-24(1)\n\
     stw 30,16(1)\n\
     stw 31,20(1)\n\
     mr 30,1\n\
     mr 31,4\n\
     mr 1,3\n\
     subi 1,1,8\n\
     mtlr 31\n\
     blrl\n\
     mr 1,30\n\
     lwz 30,16(1)\n\
     lwz 31,20(1)\n\
     addi 1,1,24\n\
     lwz 0,4(1)\n\
     mtlr 0\n\
     blr\n\
     .size ogc_audio_call_on_stack, .-ogc_audio_call_on_stack"
);

#[cfg(target_arch = "powerpc")]
unsafe extern "C" {
    fn ogc_audio_call_on_stack(stack: *mut u8, callback: unsafe extern "C" fn());
}

#[cfg(target_arch = "powerpc")]
unsafe fn run_dma_callback(stack: *mut u8, callback: unsafe extern "C" fn()) {
    if stack.is_null() {
        unsafe { callback() };
    } else {
        unsafe { ogc_audio_call_on_stack(stack, callback) };
    }
}

pub const AI_STREAM_STOP: u32 = 0x0000_0000;
pub const AI_STREAM_START: u32 = 0x0000_0001;
pub const AI_SAMPLERATE_32KHZ: u32 = 0x0000_0000;
pub const AI_SAMPLERATE_48KHZ: u32 = 0x0000_0001;

fn stream_start_needs_src_init(playstate: u32, next_state: u32, streamrate: u32) -> bool {
    playstate != next_state && next_state == AI_STREAM_START && streamrate == AI_SAMPLERATE_32KHZ
}

pub type AidCallback = Option<unsafe extern "C" fn()>;
pub type AisCallback = Option<unsafe extern "C" fn(smp_cnt: u32)>;

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct AudioSnapshot {
    pub initialized: bool,
    pub callback_stack: *mut u8,
    pub stream_vol_left: u8,
    pub stream_vol_right: u8,
    pub stream_sample_rate: u32,
    pub stream_trigger: u32,
    pub stream_sample_count: u32,
    pub stream_play_state: u32,
    pub dsp_sample_rate: u8,
    pub dma_start_addr: u32,
    pub dma_length: u32,
    pub dma_bytes_left: u32,
    pub dma_enable: bool,
    pub stream_callback_registered: bool,
    pub dma_callback_registered: bool,
    pub stream_callback: usize,
    pub dma_callback: usize,
}

#[derive(Copy, Clone)]
struct AudioState {
    inited: bool,
    callback_stack: *mut u8,
    stream_callback: AisCallback,
    dma_callback: AidCallback,
    #[cfg(not(target_arch = "powerpc"))]
    stream_vol_left: u8,
    #[cfg(not(target_arch = "powerpc"))]
    stream_vol_right: u8,
    #[cfg(not(target_arch = "powerpc"))]
    stream_sample_rate: u32,
    #[cfg(not(target_arch = "powerpc"))]
    stream_trigger: u32,
    #[cfg(not(target_arch = "powerpc"))]
    stream_sample_count: u32,
    #[cfg(not(target_arch = "powerpc"))]
    stream_play_state: u32,
    #[cfg(not(target_arch = "powerpc"))]
    dsp_sample_rate: u8,
    #[cfg(not(target_arch = "powerpc"))]
    dma_start_addr: u32,
    #[cfg(not(target_arch = "powerpc"))]
    dma_length: u32,
    #[cfg(not(target_arch = "powerpc"))]
    dma_bytes_left: u32,
    #[cfg(not(target_arch = "powerpc"))]
    dma_enable: bool,
}

impl AudioState {
    const fn new() -> Self {
        Self {
            inited: false,
            callback_stack: core::ptr::null_mut(),
            stream_callback: None,
            dma_callback: None,
            #[cfg(not(target_arch = "powerpc"))]
            stream_vol_left: 0,
            #[cfg(not(target_arch = "powerpc"))]
            stream_vol_right: 0,
            #[cfg(not(target_arch = "powerpc"))]
            stream_sample_rate: AI_SAMPLERATE_48KHZ,
            #[cfg(not(target_arch = "powerpc"))]
            stream_trigger: 0,
            #[cfg(not(target_arch = "powerpc"))]
            stream_sample_count: 0,
            #[cfg(not(target_arch = "powerpc"))]
            stream_play_state: AI_STREAM_STOP,
            #[cfg(not(target_arch = "powerpc"))]
            dsp_sample_rate: AI_SAMPLERATE_48KHZ as u8,
            #[cfg(not(target_arch = "powerpc"))]
            dma_start_addr: 0,
            #[cfg(not(target_arch = "powerpc"))]
            dma_length: 0,
            #[cfg(not(target_arch = "powerpc"))]
            dma_bytes_left: 0,
            #[cfg(not(target_arch = "powerpc"))]
            dma_enable: false,
        }
    }
}

static AUDIO_LOCK: AtomicBool = AtomicBool::new(false);
#[cfg(target_arch = "powerpc")]
static DMA_CALLBACK_ACTIVE: AtomicBool = AtomicBool::new(false);
static mut AUDIO_STATE: AudioState = AudioState::new();

fn lock_state() {
    while AUDIO_LOCK
        .compare_exchange(false, true, Ordering::Acquire, Ordering::Relaxed)
        .is_err()
    {
        core::hint::spin_loop();
    }
}

fn unlock_state() {
    AUDIO_LOCK.store(false, Ordering::Release);
}

#[cfg(target_arch = "powerpc")]
unsafe fn disable_interrupts() -> u32 {
    let msr: u32;
    unsafe {
        core::arch::asm!("mfmsr {0}", out(reg) msr, options(nostack));
        core::arch::asm!("mtmsr {0}", in(reg) (msr & !(1 << 15)), options(nostack));
    }
    msr & (1 << 15)
}

#[cfg(target_arch = "powerpc")]
unsafe fn restore_interrupts(state: u32) {
    let msr: u32;
    unsafe {
        core::arch::asm!("mfmsr {0}", out(reg) msr, options(nostack));
        core::arch::asm!("mtmsr {0}", in(reg) ((msr & !(1 << 15)) | state), options(nostack));
    }
}

fn with_state<R>(f: impl FnOnce(&mut AudioState) -> R) -> R {
    #[cfg(target_arch = "powerpc")]
    let interrupt_state = unsafe { disable_interrupts() };
    lock_state();
    // SAFETY: `AUDIO_LOCK` serializes all access to `AUDIO_STATE`, so this
    // creates the only mutable reference for the duration of the callback.
    let out = unsafe { f(&mut *core::ptr::addr_of_mut!(AUDIO_STATE)) };
    unlock_state();
    #[cfg(target_arch = "powerpc")]
    unsafe {
        restore_interrupts(interrupt_state);
    }
    out
}

const AI_REG_BASE_GAMECUBE: usize = 0xCC00_6C00;
const AI_REG_BASE_WII: usize = 0xCD00_6C00;
#[cfg(target_arch = "powerpc")]
const DSP_REG_BASE: usize = 0xCC00_5000;
#[cfg(target_arch = "powerpc")]
const AI_CONTROL: usize = 0;
#[cfg(target_arch = "powerpc")]
const AI_STREAM_VOL: usize = 1;
#[cfg(target_arch = "powerpc")]
const AI_SAMPLE_COUNT: usize = 2;
#[cfg(target_arch = "powerpc")]
const AI_INT_TIMING: usize = 3;
#[cfg(target_arch = "powerpc")]
const AI_PSTAT: u32 = 0x01;
#[cfg(target_arch = "powerpc")]
const AI_AISFR: u32 = 0x02;
#[cfg(target_arch = "powerpc")]
const AI_AIINTMSK: u32 = 0x04;
#[cfg(target_arch = "powerpc")]
const AI_AIINT: u32 = 0x08;
#[cfg(target_arch = "powerpc")]
const AI_AIINTVLD: u32 = 0x10;
#[cfg(target_arch = "powerpc")]
const AI_SCRESET: u32 = 0x20;
#[cfg(target_arch = "powerpc")]
const AI_DMAFR: u32 = 0x40;
#[cfg(target_arch = "powerpc")]
const DSPCR_DSPINT: u16 = 0x0080;
#[cfg(target_arch = "powerpc")]
const DSPCR_ARINT: u16 = 0x0020;
#[cfg(target_arch = "powerpc")]
const DSPCR_AIINT: u16 = 0x0008;

fn ai_reg_base(is_wii: bool) -> usize {
    if is_wii {
        AI_REG_BASE_WII
    } else {
        AI_REG_BASE_GAMECUBE
    }
}

#[cfg(target_arch = "powerpc")]
fn ai_reg(index: usize) -> *mut u32 {
    let is_wii = ogc_core::powerpc::hollywood_revision() != 0;
    (ai_reg_base(is_wii) as *mut u32).wrapping_add(index)
}

#[cfg(target_arch = "powerpc")]
fn dsp_reg(index: usize) -> *mut u16 {
    (DSP_REG_BASE as *mut u16).wrapping_add(index)
}

#[cfg(target_arch = "powerpc")]
fn read_ai(index: usize) -> u32 {
    unsafe { core::ptr::read_volatile(ai_reg(index)) }
}

#[cfg(target_arch = "powerpc")]
fn write_ai(index: usize, value: u32) {
    unsafe { core::ptr::write_volatile(ai_reg(index), value) };
}

#[cfg(target_arch = "powerpc")]
fn update_ai(index: usize, f: impl FnOnce(u32) -> u32) {
    let value = read_ai(index);
    write_ai(index, f(value));
}

#[cfg(target_arch = "powerpc")]
fn ai_src_init() {
    const BUFFER_NS: u64 = 3_000;
    const BOUND_32_KHZ_NS: u64 = 31_524;
    const BOUND_48_KHZ_NS: u64 = 42_024;
    const MIN_WAIT_NS: u64 = 42_000;
    const MAX_WAIT_NS: u64 = 63_000;
    let is_wii = ogc_core::powerpc::hollywood_revision() != 0;
    let sample_count_mask = if is_wii { 0x7fff_ffff } else { u32::MAX };

    let ticks_from_ns =
        |nanoseconds: u64| nanoseconds * ogc_lwp_driver::TICKS_PER_SECOND / 1_000_000_000;
    let bound_32_khz = ticks_from_ns(BOUND_32_KHZ_NS);
    let bound_48_khz = ticks_from_ns(BOUND_48_KHZ_NS);
    let buffer = ticks_from_ns(BUFFER_NS);

    let (time2, wait) = loop {
        update_ai(AI_CONTROL, |value| {
            (value | AI_SCRESET) & !AI_AISFR | AI_PSTAT
        });
        let mut sample_count = mask_sample_count(read_ai(AI_SAMPLE_COUNT), sample_count_mask);
        while sample_count == mask_sample_count(read_ai(AI_SAMPLE_COUNT), sample_count_mask) {
            core::hint::spin_loop();
        }

        let time1 = ogc_lwp_driver::now_ticks();
        update_ai(AI_CONTROL, |value| value | AI_AISFR | AI_PSTAT);
        sample_count = mask_sample_count(read_ai(AI_SAMPLE_COUNT), sample_count_mask);
        while sample_count == mask_sample_count(read_ai(AI_SAMPLE_COUNT), sample_count_mask) {
            core::hint::spin_loop();
        }

        let time2 = ogc_lwp_driver::now_ticks();
        let elapsed = tick_delta(time1, time2);
        update_ai(AI_CONTROL, |value| value & !(AI_AISFR | AI_PSTAT));

        if let Some(wait) = src_settle_wait(
            elapsed,
            bound_32_khz,
            bound_48_khz,
            buffer,
            ticks_from_ns(MIN_WAIT_NS),
            ticks_from_ns(MAX_WAIT_NS),
        ) {
            break (time2, wait);
        }
    };

    while tick_delta(time2, ogc_lwp_driver::now_ticks()) < wait {
        core::hint::spin_loop();
    }
}

#[cfg(target_arch = "powerpc")]
fn mask_sample_count(value: u32, mask: u32) -> u32 {
    value & mask
}

#[cfg(target_arch = "powerpc")]
fn tick_delta(start: u64, end: u64) -> u64 {
    if end < start {
        u64::MAX.wrapping_sub(start).wrapping_add(end)
    } else {
        end - start
    }
}

#[cfg(target_arch = "powerpc")]
fn src_settle_wait(
    elapsed: u64,
    bound_32_khz: u64,
    bound_48_khz: u64,
    buffer: u64,
    min_wait: u64,
    max_wait: u64,
) -> Option<u64> {
    if elapsed > bound_32_khz.saturating_sub(buffer)
        && elapsed < bound_32_khz.saturating_add(buffer)
    {
        (elapsed < bound_48_khz.saturating_sub(buffer)).then_some(max_wait)
    } else {
        Some(min_wait)
    }
}

#[cfg(target_arch = "powerpc")]
fn read_dsp(index: usize) -> u16 {
    unsafe { core::ptr::read_volatile(dsp_reg(index)) }
}

#[cfg(target_arch = "powerpc")]
fn write_dsp(index: usize, value: u16) {
    unsafe { core::ptr::write_volatile(dsp_reg(index), value) };
}

#[cfg(target_arch = "powerpc")]
fn update_dsp(index: usize, f: impl FnOnce(u16) -> u16) {
    let value = read_dsp(index);
    write_dsp(index, f(value));
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn dma_irq_handler(_id: u32, _user: *mut core::ffi::c_void) {
    update_dsp(5, |value| {
        (value & !(DSPCR_DSPINT | DSPCR_ARINT)) | DSPCR_AIINT
    });

    let (callback, callback_stack) = with_state(|state| (state.dma_callback, state.callback_stack));
    if let Some(callback) = callback
        && DMA_CALLBACK_ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_ok()
    {
        unsafe { run_dma_callback(callback_stack, callback) };
        DMA_CALLBACK_ACTIVE.store(false, Ordering::Release);
    }
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn stream_irq_handler(_id: u32, _user: *mut core::ffi::c_void) {
    let sample_count = read_ai(AI_SAMPLE_COUNT);
    let callback = with_state(|state| state.stream_callback);
    if let Some(callback) = callback {
        unsafe { callback(sample_count) };
    }
    update_ai(AI_CONTROL, |value| value | AI_AIINT);
}

#[cfg(target_arch = "powerpc")]
fn powerpc_snapshot() -> AudioSnapshot {
    let state = with_state(|state| {
        (
            state.inited,
            state.callback_stack,
            state.stream_callback,
            state.dma_callback,
        )
    });

    AudioSnapshot {
        initialized: state.0,
        callback_stack: state.1,
        stream_vol_left: audio_get_stream_vol_left(),
        stream_vol_right: audio_get_stream_vol_right(),
        stream_sample_rate: audio_get_stream_sample_rate(),
        stream_trigger: read_ai(AI_INT_TIMING),
        stream_sample_count: read_ai(AI_SAMPLE_COUNT),
        stream_play_state: audio_get_stream_play_state(),
        dsp_sample_rate: audio_get_dsp_sample_rate() as u8,
        dma_start_addr: audio_get_dma_start_addr(),
        dma_length: audio_get_dma_length(),
        dma_bytes_left: audio_get_dma_bytes_left(),
        dma_enable: audio_get_dma_enable_flag() != 0,
        stream_callback_registered: state.2.is_some(),
        dma_callback_registered: state.3.is_some(),
        stream_callback: state.2.map_or(0, |cb| cb as usize),
        dma_callback: state.3.map_or(0, |cb| cb as usize),
    }
}

pub fn snapshot() -> AudioSnapshot {
    #[cfg(target_arch = "powerpc")]
    {
        powerpc_snapshot()
    }

    #[cfg(not(target_arch = "powerpc"))]
    with_state(|state| AudioSnapshot {
        initialized: state.inited,
        callback_stack: state.callback_stack,
        stream_vol_left: state.stream_vol_left,
        stream_vol_right: state.stream_vol_right,
        stream_sample_rate: state.stream_sample_rate,
        stream_trigger: state.stream_trigger,
        stream_sample_count: state.stream_sample_count,
        stream_play_state: state.stream_play_state,
        dsp_sample_rate: state.dsp_sample_rate,
        dma_start_addr: state.dma_start_addr,
        dma_length: state.dma_length,
        dma_bytes_left: state.dma_bytes_left,
        dma_enable: state.dma_enable,
        stream_callback_registered: state.stream_callback.is_some(),
        dma_callback_registered: state.dma_callback.is_some(),
        stream_callback: state.stream_callback.map_or(0, |cb| cb as usize),
        dma_callback: state.dma_callback.map_or(0, |cb| cb as usize),
    })
}

#[cfg(all(test, target_arch = "powerpc"))]
fn reset_state(stack: *mut u8) {
    with_state(|state| {
        *state = AudioState::new();
        state.callback_stack = stack;
    });
}

pub fn audio_register_stream_callback(callback: AisCallback) -> AisCallback {
    with_state(|state| {
        let old = state.stream_callback;
        state.stream_callback = callback;
        old
    })
}

pub fn audio_init(stack: *mut u8) {
    #[cfg(target_arch = "powerpc")]
    {
        let should_init = with_state(|state| {
            let was_inited = state.inited;
            if !was_inited {
                *state = AudioState::new();
                state.callback_stack = stack;
                state.inited = true;
            }
            !was_inited
        });
        if should_init {
            update_ai(AI_CONTROL, |value| {
                (value & !(AI_AIINTVLD | AI_AIINTMSK | AI_PSTAT)) | AI_SCRESET
            });
            write_ai(AI_STREAM_VOL, 0);
            write_ai(AI_INT_TIMING, 0);
            if audio_get_dsp_sample_rate() == AI_SAMPLERATE_48KHZ {
                let irq_state = unsafe { disable_interrupts() };
                update_ai(AI_CONTROL, |value| value & !AI_DMAFR);
                ai_src_init();
                update_ai(AI_CONTROL, |value| value | AI_DMAFR);
                unsafe { restore_interrupts(irq_state) };
            }
            unsafe {
                irq::request(IRQ_DSP_AI, Some(dma_irq_handler), core::ptr::null_mut());
            }
            irq::enable(1 << IRQ_DSP_AI);
            if ogc_core::powerpc::hollywood_revision() == 0 {
                // BUG FIX: C installs this GameCube handler. Without it, the
                // registered stream callback never runs and the AI interrupt
                // remains pending.
                unsafe {
                    irq::request(IRQ_AI, Some(stream_irq_handler), core::ptr::null_mut());
                }
                irq::enable(1 << IRQ_AI);
            }
        }
    }

    #[cfg(not(target_arch = "powerpc"))]
    with_state(|state| {
        if !state.inited {
            *state = AudioState::new();
            state.callback_stack = stack;
            state.inited = true;
        }
    });
}

pub fn audio_set_stream_vol_left(vol: u8) {
    #[cfg(target_arch = "powerpc")]
    {
        update_ai(AI_STREAM_VOL, |value| {
            (value & !0x0000_00ff) | u32::from(vol)
        });
    }

    #[cfg(not(target_arch = "powerpc"))]
    with_state(|state| {
        state.stream_vol_left = vol;
    });
}

pub fn audio_get_stream_vol_left() -> u8 {
    #[cfg(target_arch = "powerpc")]
    {
        (read_ai(AI_STREAM_VOL) & 0xff) as u8
    }

    #[cfg(not(target_arch = "powerpc"))]
    with_state(|state| state.stream_vol_left)
}

pub fn audio_set_stream_vol_right(vol: u8) {
    #[cfg(target_arch = "powerpc")]
    {
        update_ai(AI_STREAM_VOL, |value| {
            (value & !0x0000_ff00) | (u32::from(vol) << 8)
        });
    }

    #[cfg(not(target_arch = "powerpc"))]
    with_state(|state| {
        state.stream_vol_right = vol;
    });
}

pub fn audio_get_stream_vol_right() -> u8 {
    #[cfg(target_arch = "powerpc")]
    {
        ((read_ai(AI_STREAM_VOL) >> 8) & 0xff) as u8
    }

    #[cfg(not(target_arch = "powerpc"))]
    with_state(|state| state.stream_vol_right)
}

pub fn audio_set_stream_sample_rate(rate: u32) {
    #[cfg(target_arch = "powerpc")]
    {
        update_ai(AI_CONTROL, |value| {
            (value & !AI_AISFR) | ((rate & AI_SAMPLERATE_48KHZ) << 1)
        });
    }

    #[cfg(not(target_arch = "powerpc"))]
    with_state(|state| {
        state.stream_sample_rate = rate & AI_SAMPLERATE_48KHZ;
    });
}

pub fn audio_get_stream_sample_rate() -> u32 {
    #[cfg(target_arch = "powerpc")]
    {
        (read_ai(AI_CONTROL) >> 1) & 1
    }

    #[cfg(not(target_arch = "powerpc"))]
    with_state(|state| state.stream_sample_rate)
}

pub fn audio_register_dma_callback(callback: AidCallback) -> AidCallback {
    with_state(|state| {
        let old = state.dma_callback;
        state.dma_callback = callback;
        old
    })
}

pub fn audio_init_dma(startaddr: u32, len: u32) {
    #[cfg(target_arch = "powerpc")]
    {
        // BUG FIX: C disables interrupts around the three related DSP
        // register updates so an interrupt cannot observe a partial transfer.
        let _interrupt_guard = ogc_sys::InterruptGuard::new();
        update_dsp(24, |value| {
            (value & !0x1fff) | (((startaddr >> 16) as u16) & 0x1fff)
        });
        update_dsp(25, |value| (value & !0xffe0) | (startaddr as u16));
        update_dsp(27, |value| {
            (value & !0x7fff) | (((len >> 5) as u16) & 0x7fff)
        });
    }

    #[cfg(not(target_arch = "powerpc"))]
    with_state(|state| {
        state.dma_start_addr = startaddr;
        state.dma_length = len;
    });
}

pub fn audio_start_dma() {
    #[cfg(target_arch = "powerpc")]
    {
        update_dsp(27, |value| value | 0x8000);
    }

    #[cfg(not(target_arch = "powerpc"))]
    with_state(|state| {
        state.dma_enable = true;
    });
}

pub fn audio_stop_dma() {
    #[cfg(target_arch = "powerpc")]
    {
        update_dsp(27, |value| value & !0x8000);
    }

    #[cfg(not(target_arch = "powerpc"))]
    with_state(|state| {
        state.dma_enable = false;
    });
}

pub fn audio_get_dma_enable_flag() -> u16 {
    #[cfg(target_arch = "powerpc")]
    {
        (read_dsp(27) >> 15) & 1
    }

    #[cfg(not(target_arch = "powerpc"))]
    with_state(|state| state.dma_enable as u16)
}

pub fn audio_get_dma_bytes_left() -> u32 {
    #[cfg(target_arch = "powerpc")]
    {
        u32::from(read_dsp(29) & 0x7fff) << 5
    }

    #[cfg(not(target_arch = "powerpc"))]
    with_state(|state| state.dma_bytes_left)
}

pub fn audio_get_dma_length() -> u32 {
    #[cfg(target_arch = "powerpc")]
    {
        u32::from(read_dsp(27) & 0x7fff) << 5
    }

    #[cfg(not(target_arch = "powerpc"))]
    with_state(|state| state.dma_length)
}

pub fn audio_get_dma_start_addr() -> u32 {
    #[cfg(target_arch = "powerpc")]
    {
        (u32::from(read_dsp(24) & 0x1fff) << 16) | u32::from(read_dsp(25) & 0xffe0)
    }

    #[cfg(not(target_arch = "powerpc"))]
    with_state(|state| state.dma_start_addr)
}

pub fn audio_set_stream_trigger(cnt: u32) {
    #[cfg(target_arch = "powerpc")]
    {
        write_ai(AI_INT_TIMING, cnt);
    }

    #[cfg(not(target_arch = "powerpc"))]
    with_state(|state| {
        state.stream_trigger = cnt;
    });
}

pub fn audio_reset_stream_sample_cnt() {
    #[cfg(target_arch = "powerpc")]
    {
        update_ai(AI_CONTROL, |value| (value & !AI_SCRESET) | AI_SCRESET);
    }

    #[cfg(not(target_arch = "powerpc"))]
    with_state(|state| {
        state.stream_sample_count = 0;
    });
}

pub fn audio_set_dsp_sample_rate(rate: u8) {
    #[cfg(target_arch = "powerpc")]
    {
        if audio_get_dsp_sample_rate() as u8 == rate {
            return;
        }
        update_ai(AI_CONTROL, |value| value & !AI_DMAFR);
        if rate == AI_SAMPLERATE_32KHZ as u8 {
            let irq_state = unsafe { disable_interrupts() };
            ai_src_init();
            update_ai(AI_CONTROL, |value| value | AI_DMAFR);
            unsafe { restore_interrupts(irq_state) };
        }
    }

    #[cfg(not(target_arch = "powerpc"))]
    with_state(|state| {
        state.dsp_sample_rate = if rate == AI_SAMPLERATE_32KHZ as u8 {
            AI_SAMPLERATE_32KHZ as u8
        } else {
            AI_SAMPLERATE_48KHZ as u8
        };
    });
}

pub fn audio_get_dsp_sample_rate() -> u32 {
    #[cfg(target_arch = "powerpc")]
    {
        ((read_ai(AI_CONTROL) >> 6) & 1) ^ 1
    }

    #[cfg(not(target_arch = "powerpc"))]
    with_state(|state| state.dsp_sample_rate as u32)
}

pub fn audio_set_stream_play_state(state_value: u32) {
    #[cfg(target_arch = "powerpc")]
    {
        let playstate = audio_get_stream_play_state();
        let streamrate = audio_get_stream_sample_rate();
        if stream_start_needs_src_init(playstate, state_value, streamrate) {
            let volright = audio_get_stream_vol_right();
            audio_set_stream_vol_right(0);
            let volleft = audio_get_stream_vol_left();
            audio_set_stream_vol_left(0);

            let irq_state = unsafe { disable_interrupts() };
            ai_src_init();
            update_ai(AI_CONTROL, |value| (value & !AI_SCRESET) | AI_SCRESET);
            update_ai(AI_CONTROL, |value| value | AI_PSTAT);
            unsafe { restore_interrupts(irq_state) };

            audio_set_stream_vol_right(volright);
            audio_set_stream_vol_left(volleft);
        } else {
            update_ai(AI_CONTROL, |value| {
                (value & !AI_PSTAT) | (state_value & AI_PSTAT)
            });
        }
    }

    #[cfg(not(target_arch = "powerpc"))]
    with_state(|state| {
        let next_state = state_value & AI_STREAM_START;
        let initialize_src = stream_start_needs_src_init(
            state.stream_play_state,
            next_state,
            state.stream_sample_rate,
        );

        state.stream_play_state = next_state;
        if initialize_src {
            state.stream_sample_count = 0;
        }
    });
}

pub fn audio_get_stream_play_state() -> u32 {
    #[cfg(target_arch = "powerpc")]
    {
        read_ai(AI_CONTROL) & AI_PSTAT
    }

    #[cfg(not(target_arch = "powerpc"))]
    with_state(|state| state.stream_play_state)
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;
    use core::sync::atomic::AtomicUsize;

    unsafe extern "C" fn stream_cb(_: u32) {}
    unsafe extern "C" fn dma_cb() {}

    #[repr(align(16))]
    struct TestStack([u8; 1024]);

    static mut TEST_CALLBACK_STACK: TestStack = TestStack([0; 1024]);
    static TEST_CALLBACK_STACK_POINTER: AtomicUsize = AtomicUsize::new(0);

    unsafe extern "C" fn record_callback_stack_pointer() {
        let stack_pointer: usize;
        unsafe {
            core::arch::asm!(
                "mr {stack_pointer}, 1",
                stack_pointer = out(reg) stack_pointer,
                options(nomem, nostack, preserves_flags)
            );
        }
        TEST_CALLBACK_STACK_POINTER.store(stack_pointer, Ordering::Release);
    }

    fn reset_for_test() {
        reset_state(core::ptr::null_mut());
    }

    #[test]
    fn src_timing_selects_the_c_wait_for_each_sample_rate() {
        assert_eq!(tick_delta(u64::MAX, 0), 0);
        assert_eq!(
            src_settle_wait(1_000, 1_000, 1_300, 100, 100, 200),
            Some(200)
        );
        assert_eq!(
            src_settle_wait(1_300, 1_000, 1_300, 100, 100, 200),
            Some(100)
        );
    }

    #[test]
    fn sample_counter_mask_matches_console_register_layout() {
        assert_eq!(mask_sample_count(0x8000_0001, 0x7fff_ffff), 1);
        assert_eq!(mask_sample_count(0x8000_0001, u32::MAX), 0x8000_0001);
    }

    #[test]
    fn audio_register_base_matches_console_model() {
        assert_eq!(ai_reg_base(false), 0xCC00_6C00);
        assert_eq!(ai_reg_base(true), 0xCD00_6C00);
    }

    #[test]
    fn dma_callback_uses_the_stack_passed_to_audio_init() {
        let stack = unsafe { core::ptr::addr_of_mut!(TEST_CALLBACK_STACK.0).cast::<u8>() };
        let stack_end = unsafe { stack.add(1024) };
        TEST_CALLBACK_STACK_POINTER.store(0, Ordering::Release);
        unsafe { run_dma_callback(stack_end, record_callback_stack_pointer) };

        let callback_stack_pointer = TEST_CALLBACK_STACK_POINTER.load(Ordering::Acquire);
        assert!(callback_stack_pointer >= stack as usize);
        assert!(callback_stack_pointer < stack_end as usize);
    }

    #[test]
    fn init_resets_state_and_preserves_stack_pointer() {
        reset_for_test();
        audio_set_stream_vol_left(99);
        audio_set_stream_vol_right(88);
        audio_set_stream_sample_rate(AI_SAMPLERATE_32KHZ);
        audio_set_dsp_sample_rate(AI_SAMPLERATE_32KHZ as u8);
        audio_init(0x1234usize as *mut u8);

        assert_eq!(audio_get_stream_vol_left(), 0);
        assert_eq!(audio_get_stream_vol_right(), 0);
        assert_eq!(audio_get_stream_sample_rate(), AI_SAMPLERATE_48KHZ);
        assert_eq!(audio_get_dsp_sample_rate(), AI_SAMPLERATE_48KHZ);

        audio_set_stream_vol_left(7);
        audio_init(core::ptr::null_mut());
        assert_eq!(audio_get_stream_vol_left(), 7);
    }

    #[test]
    fn callbacks_and_dma_state_round_trip() {
        reset_for_test();
        audio_init(core::ptr::null_mut());

        assert!(audio_register_stream_callback(Some(stream_cb)).is_none());
        let old_stream = audio_register_stream_callback(None);
        assert!(matches!(old_stream, Some(cb) if cb as *const () == stream_cb as *const ()));

        assert!(audio_register_dma_callback(Some(dma_cb)).is_none());
        let old_dma = audio_register_dma_callback(None);
        assert!(matches!(old_dma, Some(cb) if cb as *const () == dma_cb as *const ()));

        audio_init_dma(0x8000, 0x200);
        assert_eq!(audio_get_dma_start_addr(), 0x8000);
        assert_eq!(audio_get_dma_length(), 0x200);
        assert_eq!(audio_get_dma_bytes_left(), 0);
        assert_eq!(audio_get_dma_enable_flag(), 0);

        audio_start_dma();
        assert_eq!(audio_get_dma_enable_flag(), 1);
        assert_eq!(audio_get_dma_bytes_left(), 0);

        audio_stop_dma();
        assert_eq!(audio_get_dma_enable_flag(), 0);

        audio_start_dma();
        audio_init_dma(0x9000, 0x300);
        assert_eq!(audio_get_dma_start_addr(), 0x9000);
        assert_eq!(audio_get_dma_length(), 0x300);
        assert_eq!(audio_get_dma_enable_flag(), 1);

        audio_set_stream_trigger(123);
        audio_reset_stream_sample_cnt();
        audio_set_stream_play_state(AI_STREAM_START);
        assert_eq!(audio_get_stream_play_state(), AI_STREAM_START);
    }

    #[test]
    fn sample_rate_setters_mask_to_libogc_bit_width() {
        reset_for_test();

        audio_set_stream_sample_rate(0xffff_fffe);
        audio_set_dsp_sample_rate(0xff);

        assert_eq!(audio_get_stream_sample_rate(), AI_SAMPLERATE_32KHZ);
        assert_eq!(audio_get_dsp_sample_rate(), AI_SAMPLERATE_48KHZ);

        let snap = snapshot();
        assert_eq!(snap.stream_sample_rate, AI_SAMPLERATE_32KHZ);
        assert_eq!(snap.dsp_sample_rate, AI_SAMPLERATE_48KHZ as u8);
    }

    #[test]
    fn snapshot_reflects_current_state() {
        reset_for_test();
        audio_init(0x1234usize as *mut u8);
        audio_set_stream_vol_left(11);
        audio_set_stream_vol_right(22);
        audio_set_stream_sample_rate(AI_SAMPLERATE_32KHZ);
        audio_set_stream_trigger(33);
        audio_init_dma(0x8000, 0x200);
        audio_start_dma();
        audio_set_dsp_sample_rate(AI_SAMPLERATE_32KHZ as u8);
        audio_set_stream_play_state(AI_STREAM_START);
        audio_register_stream_callback(Some(stream_cb));
        audio_register_dma_callback(Some(dma_cb));

        let snap = snapshot();
        assert!(snap.initialized);
        assert_eq!(snap.callback_stack, 0x1234usize as *mut u8);
        assert_eq!(snap.stream_vol_left, 11);
        assert_eq!(snap.stream_vol_right, 22);
        assert_eq!(snap.stream_sample_rate, AI_SAMPLERATE_32KHZ);
        assert_eq!(snap.stream_trigger, 33);
        assert_eq!(snap.dma_start_addr, 0x8000);
        assert_eq!(snap.dma_length, 0x200);
        assert_eq!(snap.dma_bytes_left, 0);
        assert!(snap.dma_enable);
        assert_eq!(snap.dsp_sample_rate, AI_SAMPLERATE_32KHZ as u8);
        assert_eq!(snap.stream_play_state, AI_STREAM_START);
        assert!(snap.stream_callback_registered);
        assert!(snap.dma_callback_registered);
        assert_ne!(snap.stream_callback, 0);
        assert_ne!(snap.dma_callback, 0);
    }

    // BUG FIX: Test the shared transition rule without reading a host-only
    // sample-count field from the PowerPC state model.
    #[test]
    fn stream_src_init_only_runs_when_starting_at_32khz() {
        assert!(!stream_start_needs_src_init(
            AI_STREAM_STOP,
            AI_STREAM_START,
            AI_SAMPLERATE_48KHZ
        ));
        assert!(stream_start_needs_src_init(
            AI_STREAM_STOP,
            AI_STREAM_START,
            AI_SAMPLERATE_32KHZ
        ));
        assert!(!stream_start_needs_src_init(
            AI_STREAM_START,
            AI_STREAM_START,
            AI_SAMPLERATE_32KHZ
        ));
    }
}
