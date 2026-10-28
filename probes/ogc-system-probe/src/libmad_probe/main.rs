#![cfg_attr(target_arch = "powerpc", no_std)]
#![cfg_attr(target_arch = "powerpc", no_main)]

#[cfg(not(target_arch = "powerpc"))]
fn main() {}

#[cfg(target_arch = "powerpc")]
#[unsafe(no_mangle)]
pub extern "C" fn main(_argc: i32, _argv: *mut *mut u8) -> i32 {
    use libmad_rs::{
        fixed::Fixed,
        frame::{Frame, Header, Layer, Mode, OPTION_HALFSAMPLERATE},
        synth::Synth,
    };
    use ogc_system_probe::probe_support::{ProbeDisplay, ProbeLogger};

    // Golden values for the current C-like DCT/SSO synthesis path. Update
    // these only when the reference comparison confirms a synthesis change.
    const FULL_RATE_HASH: u32 = 897_898_644;
    const HALF_RATE_HASH: u32 = 1_929_954_053;

    let mut logger = ProbeLogger::new(ProbeDisplay::init());
    logger.section("LIBMAD-RS SYNTHESIS");
    if !logger.display().console_ready() {
        loop {
            core::hint::spin_loop();
        }
    }

    let mut frame = Frame {
        header: Header {
            layer: Layer::III,
            mode: Mode::Stereo,
            samplerate: 44_100,
            ..Header::default()
        },
        ..Frame::default()
    };
    for channel in 0..2 {
        for slot in 0..36 {
            for band in 0..32 {
                let value = ((channel * 997 + slot * 101 + band * 37) % 2001) as i32 - 1000;
                frame.sbsample[channel][slot][band] = value << 12;
            }
        }
    }

    let mut synth = Synth::new();
    let full = synth.frame(&frame);
    let full_hash = checksum(
        &full.samples[0][..full.length as usize],
        &full.samples[1][..full.length as usize],
    );
    let full_metadata = full.samplerate == 44_100 && full.channels == 2 && full.length == 1152;
    let full_phase = synth.phase == 4;
    let full_state = synth.filter_state() != [[[[[0 as Fixed; 8]; 16]; 2]; 2]; 2];
    let full_checksum = full_hash == FULL_RATE_HASH;

    frame.options = OPTION_HALFSAMPLERATE;
    let half = synth.frame(&frame);
    let half_hash = checksum(
        &half.samples[0][..half.length as usize],
        &half.samples[1][..half.length as usize],
    );
    let half_metadata = half.samplerate == 22_050 && half.channels == 2 && half.length == 576;
    let half_phase = synth.phase == 8;
    let half_checksum = half_hash == HALF_RATE_HASH;

    logger.record("full-rate PCM metadata", full_metadata);
    logger.record("full-rate synthesis phase", full_phase);
    logger.record("full-rate filter history", full_state);
    logger.record("full-rate PCM checksum", full_checksum);
    logger.record("half-rate PCM metadata", half_metadata);
    logger.record("half-rate synthesis phase", half_phase);
    logger.record("half-rate PCM checksum", half_checksum);
    logger.summary();

    loop {
        logger.display().wait_vsync();
    }
}

#[cfg(target_arch = "powerpc")]
fn checksum(left: &[Fixed], right: &[Fixed]) -> u32 {
    let mut hash = 2_166_136_261u32;
    for &sample in left.iter().chain(right) {
        hash ^= sample as u32;
        hash = hash.wrapping_mul(16_777_619);
    }
    hash
}

#[cfg(target_arch = "powerpc")]
use libmad_rs::fixed::Fixed;

#[cfg(target_arch = "powerpc")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
