#![cfg_attr(target_arch = "powerpc", no_std)]
#![cfg_attr(target_arch = "powerpc", no_main)]

#[cfg(not(target_arch = "powerpc"))]
fn main() {}

#[cfg(target_arch = "powerpc")]
#[unsafe(no_mangle)]
pub extern "C" fn main(_argc: i32, _argv: *mut *mut u8) -> i32 {
    use core::sync::atomic::Ordering;
    use ogc_system_probe::probe_support::ProbeDisplay;

    const CASES: [DecodeCase; 3] = [
        DecodeCase {
            name: "MPEG Layer I mono",
            bytes: include_bytes!("../../test-data/layer1-mono.mp1"),
            layer: mad_sys::MAD_LAYER_I,
            samplerate: 44_100,
            channels: 1,
            samples_per_frame: 384,
            minimum_frames: 12,
        },
        DecodeCase {
            name: "MPEG Layer II stereo",
            bytes: include_bytes!("../../test-data/layer2-stereo.mp2"),
            layer: mad_sys::MAD_LAYER_II,
            samplerate: 44_100,
            channels: 2,
            samples_per_frame: 1152,
            minimum_frames: 20,
        },
        DecodeCase {
            name: "MPEG Layer III stereo",
            bytes: include_bytes!(env!(
                "LIBOGC_MAD_PROBE_MP3",
                "set LIBOGC_MAD_PROBE_MP3 to external test audio; see docs/dependencies.md"
            )),
            layer: mad_sys::MAD_LAYER_III,
            samplerate: 44_100,
            channels: 2,
            samples_per_frame: 1152,
            minimum_frames: 20,
        },
    ];

    let display = ProbeDisplay::init();
    display.println("libmad-rs MPEG decoder probe");
    if !display.console_ready() {
        loop {
            core::hint::spin_loop();
        }
    }

    let mut all_passed = true;
    for case in CASES {
        let passed = decode_case(case);
        if passed {
            display.println(case.name);
            display.println("  PASS decoded frames and PCM");
        } else {
            display.println(case.name);
            display.println("  FAIL decoder output mismatch");
            all_passed = false;
        }
    }
    PLAYER_FILTER_CALLS.store(0, Ordering::Release);
    mp3player_sys::mp3_player_init();
    let mp3 = CASES[2].bytes;
    let player_started = unsafe {
        mp3player_sys::mp3_player_play_buffer(
            mp3.as_ptr().cast(),
            mp3.len() as i32,
            Some(player_filter),
        ) == 0
    };
    let mut player_finished = false;
    if player_started {
        for _ in 0..600 {
            if !mp3player_sys::is_playing() && PLAYER_FILTER_CALLS.load(Ordering::Acquire) > 0 {
                player_finished = true;
                break;
            }
            display.wait_vsync();
        }
    }
    if mp3player_sys::is_playing() {
        mp3player_sys::mp3_player_stop();
    }
    let player_passed = player_started
        && player_finished
        && PLAYER_FILTER_CALLS.load(Ordering::Acquire) >= CASES[2].minimum_frames;
    display.println(if player_passed {
        "PASS mp3player decoded and played MP3 frames"
    } else {
        "FAIL mp3player MP3 playback"
    });
    all_passed &= player_passed;
    display.println(if all_passed {
        "PASS all MPEG layer decoder checks"
    } else {
        "FAIL one or more MPEG layer decoder checks"
    });

    loop {
        display.wait_vsync();
    }
}

#[cfg(target_arch = "powerpc")]
const INPUT_CAPACITY: usize = 16 * 1024;

#[cfg(target_arch = "powerpc")]
static mut INPUT_BUFFER: [u8; INPUT_CAPACITY] = [0; INPUT_CAPACITY];

#[cfg(target_arch = "powerpc")]
static PLAYER_FILTER_CALLS: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);

#[cfg(target_arch = "powerpc")]
#[derive(Copy, Clone)]
struct DecodeCase {
    name: &'static str,
    bytes: &'static [u8],
    layer: i32,
    samplerate: u32,
    channels: u16,
    samples_per_frame: u16,
    minimum_frames: u32,
}

#[cfg(target_arch = "powerpc")]
struct DecodeState {
    input: *const u8,
    input_len: u32,
    input_sent: bool,
    case: DecodeCase,
    header_frames: u32,
    pcm_frames: u32,
    pcm_samples: u32,
    metadata_matches: bool,
    pcm_nonzero: bool,
    checksum: u32,
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn player_filter(
    _stream: *mut mad_sys::MadStream,
    _frame: *mut mad_sys::MadFrame,
) {
    PLAYER_FILTER_CALLS.fetch_add(1, core::sync::atomic::Ordering::AcqRel);
}

#[cfg(target_arch = "powerpc")]
fn decode_case(case: DecodeCase) -> bool {
    let input_len = case.bytes.len();
    if input_len + mad_sys::MAD_BUFFER_GUARD > INPUT_CAPACITY {
        return false;
    }

    let input = core::ptr::addr_of_mut!(INPUT_BUFFER).cast::<u8>();
    unsafe {
        core::ptr::copy_nonoverlapping(case.bytes.as_ptr(), input, input_len);
        core::ptr::write_bytes(input.add(input_len), 0, mad_sys::MAD_BUFFER_GUARD);
    }

    let mut state = DecodeState {
        input,
        input_len: (input_len + mad_sys::MAD_BUFFER_GUARD) as u32,
        input_sent: false,
        case,
        header_frames: 0,
        pcm_frames: 0,
        pcm_samples: 0,
        metadata_matches: true,
        pcm_nonzero: false,
        checksum: 2_166_136_261,
    };
    let callbacks = mad_sys::MadDecoderCallbacks {
        input: Some(input_callback),
        header: Some(header_callback),
        output: Some(output_callback),
        ..mad_sys::MadDecoderCallbacks::default()
    };
    let mut decoder = mad_sys::MadDecoder::default();
    unsafe {
        mad_sys::decoder_init(
            &mut decoder,
            (&mut state as *mut DecodeState).cast(),
            callbacks,
        );
    }
    let result = unsafe { mad_sys::decoder_run(&mut decoder, mad_sys::MAD_DECODER_MODE_SYNC) };

    result == 0
        && decoder.sync.is_null()
        && state.metadata_matches
        && state.header_frames >= case.minimum_frames
        && state.pcm_frames >= case.minimum_frames
        && state.pcm_samples == state.pcm_frames * case.samples_per_frame as u32
        && state.pcm_nonzero
        && state.checksum != 2_166_136_261
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn input_callback(
    data: *mut core::ffi::c_void,
    stream: *mut mad_sys::MadStream,
) -> i32 {
    let state = unsafe { &mut *data.cast::<DecodeState>() };
    if state.input_sent {
        return mad_sys::MAD_FLOW_STOP;
    }
    state.input_sent = true;
    unsafe { mad_sys::stream_buffer(stream, state.input, state.input_len) };
    mad_sys::MAD_FLOW_CONTINUE
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn header_callback(
    data: *mut core::ffi::c_void,
    header: *const mad_sys::MadHeader,
) -> i32 {
    let state = unsafe { &mut *data.cast::<DecodeState>() };
    let header = unsafe { &*header };
    let channels = if header.mode == mad_sys::MAD_MODE_SINGLE_CHANNEL {
        1
    } else {
        2
    };
    state.metadata_matches &= header.layer == state.case.layer
        && header.samplerate == state.case.samplerate
        && channels == state.case.channels as i32;
    state.header_frames += 1;
    mad_sys::MAD_FLOW_CONTINUE
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn output_callback(
    data: *mut core::ffi::c_void,
    header: *const mad_sys::MadHeader,
    pcm: *mut mad_sys::MadPcm,
) -> i32 {
    let state = unsafe { &mut *data.cast::<DecodeState>() };
    let header = unsafe { &*header };
    let pcm = unsafe { &*pcm };
    state.metadata_matches &= header.layer == state.case.layer
        && pcm.samplerate == state.case.samplerate
        && pcm.channels == state.case.channels
        && pcm.length == state.case.samples_per_frame;
    state.pcm_frames += 1;
    state.pcm_samples += pcm.length as u32;
    for channel in 0..pcm.channels as usize {
        for &sample in &pcm.samples[channel][..pcm.length as usize] {
            state.pcm_nonzero |= sample != 0;
            state.checksum ^= sample as u32;
            state.checksum = state.checksum.wrapping_mul(16_777_619);
        }
    }
    mad_sys::MAD_FLOW_CONTINUE
}

#[cfg(target_arch = "powerpc")]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo<'_>) -> ! {
    loop {
        core::hint::spin_loop();
    }
}
