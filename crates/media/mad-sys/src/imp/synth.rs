//! C-compatible synthesis entry points backed by the pure Rust decoder.

use super::abi::{MadFrame, MadHeader, MadSynth};
use libmad_rs::{
    frame::{Emphasis, Frame, Header, Layer, Mode},
    synth::{FilterState, Synth},
    timer::Timer,
};

/// Convert the public C frame layout to the pure Rust synthesis input.
///
/// The ABI sample array and Rust frame use the same channel, slot, and
/// subband order. This copy occurs after the decoder filter callback so
/// callback edits are preserved.
pub(crate) fn frame_to_core(frame: &MadFrame) -> Frame {
    Frame {
        header: header_to_core(&frame.header),
        options: frame.options as u32,
        sbsample: frame.sbsample,
    }
}

fn header_to_core(header: &MadHeader) -> Header {
    Header {
        layer: match header.layer {
            1 => Layer::I,
            2 => Layer::II,
            3 => Layer::III,
            _ => Layer::Unknown,
        },
        mode: match header.mode {
            0 => Mode::SingleChannel,
            1 => Mode::DualChannel,
            2 => Mode::JointStereo,
            _ => Mode::Stereo,
        },
        mode_extension: header.mode_extension as u8,
        emphasis: match header.emphasis {
            0 => Emphasis::None,
            1 => Emphasis::FiftyFifteenUs,
            2 => Emphasis::Reserved,
            _ => Emphasis::CcittJ17,
        },
        bitrate: header.bitrate,
        samplerate: header.samplerate,
        crc_check: header.crc_check,
        crc_target: header.crc_target,
        flags: header.flags as u32,
        private_bits: header.private_bits as u32,
        duration: Timer {
            seconds: i64::from(header.duration.seconds),
            fraction: u64::from(header.duration.fraction),
        },
    }
}

fn sync_state(synth: &mut Synth, abi_synth: &MadSynth) {
    let filter: FilterState = abi_synth.filter;
    synth.set_filter_state(filter);
    synth.phase = (abi_synth.phase & 15) as usize;
}

fn export_state(synth: &Synth, abi_synth: &mut MadSynth) {
    abi_synth.filter = synth.filter_state();
    abi_synth.phase = synth.phase as u32;
    abi_synth.pcm.samplerate = synth.pcm.samplerate;
    abi_synth.pcm.channels = synth.pcm.channels;
    abi_synth.pcm.length = synth.pcm.length;
    abi_synth.pcm.samples = synth.pcm.samples;
}

/// Initialize a C-compatible synthesis state.
///
/// # Safety
/// `synth` must be null or point to writable `MadSynth` storage.
pub unsafe fn synth_init(synth: *mut MadSynth) {
    if synth.is_null() {
        return;
    }
    let core = Synth::new();
    let output = unsafe { &mut *synth };
    *output = MadSynth::default();
    export_state(&core, output);
}

/// Clear only the synthesis filter history, preserving phase and PCM metadata.
///
/// # Safety
/// `synth` must be null or point to writable `MadSynth` storage.
pub unsafe fn synth_mute(synth: *mut MadSynth) {
    if synth.is_null() {
        return;
    }
    let output = unsafe { &mut *synth };
    let mut core = Synth::new();
    sync_state(&mut core, output);
    core.mute();
    output.filter = core.filter_state();
}

/// Synthesize one decoded ABI frame with the pure Rust polyphase filter.
///
/// # Safety
/// `synth` must be null or point to writable `MadSynth` storage. `frame` must
/// be null or point to readable `MadFrame` storage for the duration of the call.
pub unsafe fn synth_frame(synth: *mut MadSynth, frame: *const MadFrame) {
    if synth.is_null() || frame.is_null() {
        return;
    }
    let abi_synth = unsafe { &mut *synth };
    let core_frame = frame_to_core(unsafe { &*frame });
    let mut core = Synth::new();
    sync_state(&mut core, abi_synth);
    core.frame(&core_frame);
    export_state(&core, abi_synth);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::imp::abi::{MAD_LAYER_III, MAD_MODE_STEREO, MAD_OPTION_HALFSAMPLERATE};

    #[test]
    fn synth_uses_abi_subband_samples_and_persists_core_filter_state() {
        let mut frame = MadFrame::default();
        frame.header.layer = MAD_LAYER_III;
        frame.header.mode = MAD_MODE_STEREO;
        frame.header.samplerate = 44_100;
        for slot in 0..36 {
            for band in 0..32 {
                frame.sbsample[0][slot][band] = 1 << 27;
                frame.sbsample[1][slot][band] = 1 << 26;
            }
        }
        let mut synth = MadSynth::default();
        unsafe {
            synth_init(&mut synth);
            synth_frame(&mut synth, &frame);
        }
        assert_eq!(synth.pcm.samplerate, 44_100);
        assert_eq!(synth.pcm.channels, 2);
        assert_eq!(synth.pcm.length, 1152);
        assert_eq!(synth.phase, 4);
        assert_ne!(synth.filter, [[[[[0; 8]; 16]; 2]; 2]; 2]);

        frame.options = MAD_OPTION_HALFSAMPLERATE;
        unsafe { synth_frame(&mut synth, &frame) };
        assert_eq!(synth.pcm.samplerate, 22_050);
        assert_eq!(synth.pcm.length, 576);
        assert_eq!(synth.phase, 8);
    }

    #[test]
    fn filter_changes_survive_frame_conversion() {
        let mut frame = MadFrame::default();
        frame.header.layer = MAD_LAYER_III;
        frame.options = 0;
        frame.sbsample[0][0][0] = libmad_rs::fixed::Fixed::MAX / 8;
        let core = frame_to_core(&frame);
        assert_eq!(core.sbsample[0][0][0], libmad_rs::fixed::Fixed::MAX / 8);
        assert_eq!(core.options, 0);
    }

    #[test]
    fn changed_filter_samples_change_pcm() {
        let mut original = MadFrame::default();
        original.header.layer = MAD_LAYER_III;
        original.header.mode = MAD_MODE_STEREO;
        original.header.samplerate = 44_100;
        let mut filtered = original;
        // This is the value a post-decode filter callback can write. The
        // synthesis conversion must consume the changed ABI frame directly.
        filtered.sbsample[0][0][0] = libmad_rs::fixed::Fixed::MAX / 2;

        let mut original_synth = MadSynth::default();
        let mut filtered_synth = MadSynth::default();
        unsafe {
            synth_init(&mut original_synth);
            synth_init(&mut filtered_synth);
            synth_frame(&mut original_synth, &original);
            synth_frame(&mut filtered_synth, &filtered);
        }
        assert_ne!(original_synth.pcm.samples, filtered_synth.pcm.samples);
    }
}
