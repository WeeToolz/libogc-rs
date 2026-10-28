//! Check synthesis state across changes between frames.

use libmad_rs::{
    fixed::ONE,
    frame::{FLAG_LSF_EXT, Frame, Layer, Mode, OPTION_HALFSAMPLERATE},
    synth::Synth,
};

fn signal_frame(layer: Layer, mode: Mode, lsf: bool) -> Frame {
    let mut frame = Frame::default();
    frame.header.layer = layer;
    frame.header.mode = mode;
    frame.header.samplerate = if lsf { 24_000 } else { 48_000 };
    if lsf {
        frame.header.flags |= FLAG_LSF_EXT;
    }
    for channel in 0..frame.header.channels() {
        for slot in 0..frame.header.subband_samples() {
            frame.sbsample[channel][slot][0] = ONE / (channel as i32 + 4);
            frame.sbsample[channel][slot][7] = if slot % 2 == 0 { ONE / 8 } else { -ONE / 8 };
        }
    }
    frame
}

#[test]
fn full_half_full_transitions_preserve_history_and_select_the_correct_samples() {
    for (layer, lsf, full_length) in [
        (Layer::I, false, 384),
        (Layer::II, false, 1152),
        (Layer::III, false, 1152),
        (Layer::III, true, 576),
    ] {
        for mode in [Mode::SingleChannel, Mode::Stereo] {
            let mut reference = Synth::new();
            let mut switched = Synth::new();
            let mut frame = signal_frame(layer, mode, lsf);
            for half in [false, true, false, true, true, false] {
                frame.options = 0;
                let full = reference.frame(&frame).clone();
                assert!(full.samples.iter().flatten().any(|&sample| sample != 0));
                frame.options = if half { OPTION_HALFSAMPLERATE } else { 0 };
                let pcm = switched.frame(&frame);
                let stride = if half { 2 } else { 1 };
                assert_eq!(usize::from(pcm.length), full_length / stride);
                assert_eq!(pcm.channels as usize, frame.header.channels());
                assert_eq!(pcm.samplerate, frame.header.samplerate / stride as u32);
                for channel in 0..usize::from(pcm.channels) {
                    for index in 0..usize::from(pcm.length) {
                        assert_eq!(
                            pcm.samples[channel][index],
                            full.samples[channel][index * stride]
                        );
                    }
                    assert!(
                        pcm.samples[channel][usize::from(pcm.length)..]
                            .iter()
                            .all(|&sample| sample == 0)
                    );
                }
                assert_eq!(switched.filter_state(), reference.filter_state());
                assert_eq!(switched.phase, reference.phase);
            }
        }
    }
}

#[test]
fn stereo_to_mono_clears_unused_pcm_and_keeps_right_channel_history() {
    let mut synth = Synth::new();
    let mut frame = signal_frame(Layer::III, Mode::Stereo, false);
    let first = synth.frame(&frame).clone();
    assert!(first.samples[1].iter().any(|&sample| sample != 0));
    let right_history = synth.filter_state()[1];
    frame.header.mode = Mode::SingleChannel;
    frame.options = OPTION_HALFSAMPLERATE;
    let pcm = synth.frame(&frame);
    assert_eq!((pcm.channels, pcm.length, pcm.samplerate), (1, 576, 24_000));
    assert!(pcm.samples[1].iter().all(|&sample| sample == 0));
    assert!(pcm.samples[0][576..].iter().all(|&sample| sample == 0));
    assert_eq!(synth.filter_state()[1], right_history);
    assert_eq!(synth.phase, 8);
}

#[test]
fn mute_keeps_phase_and_pcm_but_clears_history_before_next_frame() {
    let mut synth = Synth::new();
    let frame = signal_frame(Layer::III, Mode::Stereo, false);
    let before = synth.frame(&frame).clone();
    assert!(before.samples.iter().flatten().any(|&sample| sample != 0));
    assert_ne!(synth.filter_state(), Synth::new().filter_state());
    let phase = synth.phase;
    synth.mute();
    assert_eq!(synth.phase, phase);
    assert_eq!(synth.pcm, before);
    assert_eq!(synth.filter_state(), [[[[[0; 8]; 16]; 2]; 2]; 2]);
    let mut silence = frame;
    silence.mute();
    let pcm = synth.frame(&silence);
    assert!(pcm.samples.iter().flatten().all(|&sample| sample == 0));
    assert_eq!(
        (pcm.channels, pcm.length, pcm.samplerate),
        (2, 1152, 48_000)
    );
    assert_eq!(synth.phase, 8);
}
