#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use ogc_audio_sys as backend;

pub const AI_STREAM_STOP: u32 = backend::AI_STREAM_STOP;
pub const AI_STREAM_START: u32 = backend::AI_STREAM_START;
pub const AI_SAMPLERATE_32KHZ: u32 = backend::AI_SAMPLERATE_32KHZ;
pub const AI_SAMPLERATE_48KHZ: u32 = backend::AI_SAMPLERATE_48KHZ;

pub type AidCallback = backend::AidCallback;
pub type AisCallback = backend::AisCallback;

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

pub fn init(stack: Option<&mut [u8]>) {
    backend::audio_init(stack.map_or(core::ptr::null_mut(), |s| s.as_mut_ptr()));
}

pub fn snapshot() -> AudioSnapshot {
    let snap = backend::snapshot();
    AudioSnapshot {
        initialized: snap.initialized,
        callback_stack: snap.callback_stack,
        stream_vol_left: snap.stream_vol_left,
        stream_vol_right: snap.stream_vol_right,
        stream_sample_rate: snap.stream_sample_rate,
        stream_trigger: snap.stream_trigger,
        stream_sample_count: snap.stream_sample_count,
        stream_play_state: snap.stream_play_state,
        dsp_sample_rate: snap.dsp_sample_rate,
        dma_start_addr: snap.dma_start_addr,
        dma_length: snap.dma_length,
        dma_bytes_left: snap.dma_bytes_left,
        dma_enable: snap.dma_enable,
        stream_callback_registered: snap.stream_callback_registered,
        dma_callback_registered: snap.dma_callback_registered,
        stream_callback: snap.stream_callback,
        dma_callback: snap.dma_callback,
    }
}

pub fn get_dma_bytes_left() -> u32 {
    backend::audio_get_dma_bytes_left()
}

pub fn get_dma_enable_flag() -> u32 {
    backend::audio_get_dma_enable_flag() as u32
}

pub fn get_dma_length() -> u32 {
    backend::audio_get_dma_length()
}

pub fn get_dma_start_addr() -> u32 {
    backend::audio_get_dma_start_addr()
}

pub fn get_dsp_sample_rate() -> u32 {
    backend::audio_get_dsp_sample_rate()
}

pub fn get_stream_play_state() -> u32 {
    backend::audio_get_stream_play_state()
}

pub fn get_stream_sample_rate() -> u32 {
    backend::audio_get_stream_sample_rate()
}

pub fn get_stream_vol_left() -> u8 {
    backend::audio_get_stream_vol_left()
}

pub fn get_stream_vol_right() -> u8 {
    backend::audio_get_stream_vol_right()
}

pub fn init_dma(start_addr: u32, length: u32) {
    backend::audio_init_dma(start_addr, length);
}

pub fn register_dma_callback(callback: AidCallback) -> AidCallback {
    backend::audio_register_dma_callback(callback)
}

pub fn register_stream_callback(callback: AisCallback) -> AisCallback {
    backend::audio_register_stream_callback(callback)
}

pub fn reset_stream_sample_cnt() {
    backend::audio_reset_stream_sample_cnt();
}

pub fn set_dsp_sample_rate(rate: u8) {
    backend::audio_set_dsp_sample_rate(rate);
}

pub fn set_stream_play_state(state: u32) {
    backend::audio_set_stream_play_state(state);
}

pub fn set_stream_sample_rate(rate: u32) {
    backend::audio_set_stream_sample_rate(rate);
}

pub fn set_stream_trigger(cnt: u32) {
    backend::audio_set_stream_trigger(cnt);
}

pub fn set_stream_vol_left(vol: u8) {
    backend::audio_set_stream_vol_left(vol);
}

pub fn set_stream_vol_right(vol: u8) {
    backend::audio_set_stream_vol_right(vol);
}

pub fn start_dma() {
    backend::audio_start_dma();
}

pub fn stop_dma() {
    backend::audio_stop_dma();
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    #[test]
    fn helper_functions_exist_without_backend() {
        let _ = init;
        let _ = set_stream_vol_left;
        let _ = set_stream_vol_right;
        let _ = set_stream_sample_rate;
        let _ = start_dma;
        let _ = stop_dma;
        let _ = get_stream_play_state;
    }

    #[test]
    fn exported_constants_have_expected_relationships() {
        assert_ne!(AI_SAMPLERATE_32KHZ, AI_SAMPLERATE_48KHZ);
        assert_ne!(AI_STREAM_START, AI_STREAM_STOP);
    }

    #[test]
    fn snapshot_is_owned_locally() {
        let _ = snapshot as fn() -> AudioSnapshot;
    }

    #[test]
    fn audio_snapshot_is_owned_locally() {
        let snap = AudioSnapshot {
            stream_sample_rate: AI_SAMPLERATE_32KHZ,
            ..AudioSnapshot::default()
        };
        assert_eq!(snap.stream_sample_rate, AI_SAMPLERATE_32KHZ);
    }
}
