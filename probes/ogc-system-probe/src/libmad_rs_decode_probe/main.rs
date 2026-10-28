#![cfg_attr(target_arch = "powerpc", no_std)]
#![cfg_attr(target_arch = "powerpc", no_main)]

#[cfg(not(target_arch = "powerpc"))]
fn main() {}

#[cfg(target_arch = "powerpc")]
extern crate alloc;

#[cfg(target_arch = "powerpc")]
use libmad_rs::fixed::Fixed;

#[cfg(target_arch = "powerpc")]
#[path = "../libmad_rs_pcm_playback.rs"]
mod pcm_playback;

#[cfg(target_arch = "powerpc")]
#[unsafe(no_mangle)]
pub extern "C" fn main(_argc: i32, _argv: *mut *mut u8) -> i32 {
    use ogc_system_probe::probe_support::ProbeDisplay;

    let display = ProbeDisplay::init();
    display.println("pure Rust libmad-rs MP3 probe R3");
    display.println("REF C-sources/libmad-c FPM_DEFAULT");
    if !display.console_ready() {
        loop {
            core::hint::spin_loop();
        }
    }

    // Keep the sizeable decode path in a separate stack frame. The runtime
    // must initialize video and print diagnostics before entering it.
    run_decode_probe(&display);
    loop {
        display.wait_vsync();
    }
}

#[cfg(target_arch = "powerpc")]
#[inline(never)]
fn run_decode_probe(display: &ogc_system_probe::probe_support::ProbeDisplay) {
    use alloc::{boxed::Box, vec::Vec};
    use libmad_rs::{
        frame::{self, FLAG_INCOMPLETE, Frame, Layer, Mode},
        layer::decode::Layer3Decoder,
        stream::{Error, MAD_BUFFER_GUARD, Stream},
        synth::Synth,
    };

    const INPUT: &[u8] = include_bytes!(env!(
        "LIBOGC_MAD_REFERENCE_MP3",
        "set LIBOGC_MAD_REFERENCE_MP3 to the external reference fixture; see docs/dependencies.md"
    ));
    const EXPECTED_FRAMES: u32 = 116;
    const EXPECTED_RATE: u32 = 44_100;
    const EXPECTED_BITRATE: u32 = 128_000;
    const EXPECTED_PCM_SAMPLES: u32 = 267_264;
    const PLAYBACK_SAMPLE_FRAMES: usize = EXPECTED_RATE as usize;
    // The upstream C reference and its 32-bit rounding-expression check
    // agree on this fixture. The old libogc fork uses a different IMDCT;
    // its d9986cab/a06b9960 hashes are not goldens for the upstream port.
    const EXPECTED_PCM_HASH: u32 = 0x4475_b651;
    const EXPECTED_SUBBAND_HASH: u32 = 0x33fa_3da0;

    let mut input = Vec::with_capacity(INPUT.len() + MAD_BUFFER_GUARD);
    input.extend_from_slice(INPUT);
    input.resize(INPUT.len() + MAD_BUFFER_GUARD, 0);

    let mut stream = Stream::new();
    stream.buffer(&input);
    // Keep the large codec state off the Wii application stack. The frame,
    // overlap/reservoir state, and synthesis filter history exceed 30 KiB.
    let mut frame = Box::new(Frame::default());
    let mut layer3 = Box::new(Layer3Decoder::new());
    let mut synth = Box::new(Synth::new());
    let mut interleaved_pcm = Vec::with_capacity(PLAYBACK_SAMPLE_FRAMES * 2);
    let mut frames = 0_u32;
    let mut recoverable_errors = 0_u32;
    let mut samples = 0_u32;
    let mut checksum = 2_166_136_261_u32;
    let mut subband_checksum = 2_166_136_261_u32;
    let mut metadata_matches = true;
    let mut nonzero_pcm = false;

    loop {
        let header = match frame::decode_header(&mut stream) {
            Ok(header) => header,
            Err(Error::BufferLength) => break,
            Err(error) if error.is_recoverable() => {
                recoverable_errors += 1;
                continue;
            }
            Err(_) => {
                metadata_matches = false;
                break;
            }
        };
        frame.header = header;
        frame.options = stream.options;
        frame.header.flags &= !FLAG_INCOMPLETE;
        metadata_matches &= header.layer == Layer::III
            && header.mode == Mode::JointStereo
            && header.samplerate == EXPECTED_RATE
            && header.bitrate == EXPECTED_BITRATE
            && header.channels() == 2;

        match layer3.decode(&mut stream, &mut frame) {
            Ok(()) => {}
            Err(error) if error.is_recoverable() => {
                recoverable_errors += 1;
                continue;
            }
            Err(_) => {
                metadata_matches = false;
                break;
            }
        }

        for channel in 0..2 {
            for slot in 0..36 {
                for band in 0..32 {
                    subband_checksum ^= frame.sbsample[channel][slot][band] as u32;
                    subband_checksum = subband_checksum.wrapping_mul(16_777_619);
                }
            }
        }

        let pcm = synth.frame(&frame);
        metadata_matches &=
            pcm.samplerate == EXPECTED_RATE && pcm.channels == 2 && pcm.length == 1_152;
        for channel in 0..2 {
            for &sample in &pcm.samples[channel][..pcm.length as usize] {
                nonzero_pcm |= sample != 0;
                checksum ^= sample as u32;
                checksum = checksum.wrapping_mul(16_777_619);
                samples += 1;
            }
        }
        for sample_index in 0..pcm.length as usize {
            if interleaved_pcm.len() / 2 < PLAYBACK_SAMPLE_FRAMES {
                for channel in 0..2 {
                    interleaved_pcm.push(to_i16(pcm.samples[channel][sample_index]));
                }
            }
        }
        frames += 1;
    }

    let decoded_ok = metadata_matches
        && frames == EXPECTED_FRAMES
        && samples == EXPECTED_PCM_SAMPLES
        && nonzero_pcm
        && subband_checksum == EXPECTED_SUBBAND_HASH
        && checksum == EXPECTED_PCM_HASH
        && recoverable_errors == 1
        && stream.error == Error::BufferLength;
    display.println(&alloc::format!(
        "FRAMES {frames}/{EXPECTED_FRAMES} ERR {recoverable_errors}"
    ));
    display.println(&alloc::format!(
        "PCM {samples}/{EXPECTED_PCM_SAMPLES} FNV {checksum:08x}"
    ));
    display.println(&alloc::format!("SUBBAND FNV {subband_checksum:08x}"));
    display.println(if metadata_matches {
        "META 44.1k STEREO 128k PASS"
    } else {
        "META FAIL"
    });
    display.println(if nonzero_pcm {
        "PCM NONZERO PASS"
    } else {
        "PCM NONZERO FAIL"
    });
    if decoded_ok {
        display.println("PASS MP3 FRAME, SUBBAND, AND PCM HASHES");
    } else {
        display.println("FAIL pure Rust MP3 decode or PCM parity");
    }

    let playback_passed = if decoded_ok {
        let playback =
            pcm_playback::play_pcm_stereo_i16(interleaved_pcm, EXPECTED_RATE, 6 * 60_750_000);
        display.println(&alloc::format!(
            "ASND sub {} dma {} voice {}",
            playback.submit_status,
            playback.dma_callbacks,
            playback.voice_callbacks
        ));
        display.println(&alloc::format!(
            "ASND status {} frames {} timeout {}",
            playback.final_voice_status,
            playback.sample_frames,
            playback.timed_out
        ));
        playback.passed() && playback.sample_frames == PLAYBACK_SAMPLE_FRAMES
    } else {
        display.println("ASND SKIP: decode check failed");
        false
    };
    display.println(if playback_passed {
        "PASS decoded PCM ASND playback"
    } else {
        "FAIL decoded PCM ASND playback"
    });
    display.println(if decoded_ok && playback_passed {
        "PASS pure Rust libmad-rs end-to-end MP3 probe"
    } else {
        "FAIL pure Rust libmad-rs end-to-end MP3 probe"
    });
}

#[cfg(target_arch = "powerpc")]
fn to_i16(sample: Fixed) -> i16 {
    if sample >= libmad_rs::fixed::ONE {
        i16::MAX
    } else if sample <= -libmad_rs::fixed::ONE {
        -i16::MAX
    } else {
        (sample >> 13) as i16
    }
}

#[cfg(target_arch = "powerpc")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
