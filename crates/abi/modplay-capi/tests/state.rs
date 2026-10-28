use modplay_capi::{
    FREQTAB, Mod, ModPlay, mod_alloc_sfx_channels, mod_effect_handler_state, mod_free,
    mod_free_state, mod_load, mod_load_from_path, mod_play_alloc_sfx_channels, mod_play_init,
    mod_play_mixing_time, mod_play_pause, mod_play_request_time, mod_play_set_frequency,
    mod_play_set_mod, mod_play_set_stereo, mod_play_set_volume, mod_play_start, mod_play_stop,
    mod_play_trigger_note, mod_play_unload, mod_player, mod_process_state, mod_set_mod,
    mod_set_mod_from_bytes, mod_start, mod_trigger_note, modplay_alloc_sfx_channels_state,
    modplay_init_state, modplay_mix_callback_state, modplay_set_frequency_state,
    modplay_set_mod_state, modplay_set_stereo_state, modplay_set_volume_state,
    modplay_trigger_note_state, modplay_unload_state, start_state, stop_state,
};
use ogc_sys::Bool;
use std::ffi::CString;
use std::ffi::c_void;
use std::io::Write;
use std::mem::ManuallyDrop;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};

static PLAYER_CALLBACK_RAN: AtomicBool = AtomicBool::new(false);
static TEMP_FILE_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn create_module_file(
    bytes: &[u8],
) -> Result<(std::path::PathBuf, CString), Box<dyn std::error::Error>> {
    for _ in 0..100 {
        let sequence = TEMP_FILE_SEQUENCE.fetch_add(1, Ordering::Relaxed);
        let path =
            std::env::temp_dir().join(format!("modplay-{}-{sequence}.mod", std::process::id()));
        let path_text = path.to_str().ok_or("temporary module path is not UTF-8")?;
        let c_path = CString::new(path_text)?;
        match std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)
        {
            Ok(mut file) => {
                if let Err(error) = file.write_all(bytes) {
                    drop(file);
                    std::fs::remove_file(&path)?;
                    return Err(error.into());
                }
                return Ok((path, c_path));
            }
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => return Err(error.into()),
        }
    }
    Err("could not allocate a unique temporary module path".into())
}

unsafe extern "C" fn player_callback(_: *mut core::ffi::c_void) {
    PLAYER_CALLBACK_RAN.store(true, Ordering::SeqCst);
}

fn alloc_bytes(bytes: &[u8]) -> (*mut u8, usize) {
    let mut buf = vec![0u8; bytes.len()];
    buf.copy_from_slice(bytes);
    let mut buf = ManuallyDrop::new(buf);
    (buf.as_mut_ptr(), buf.len())
}

unsafe fn free_bytes(ptr: *mut u8, len: usize) {
    if ptr.is_null() {
        return;
    }
    unsafe {
        drop(Vec::from_raw_parts(ptr, len, len));
    }
}

fn write_be_u16_words(buf: &mut [u8], offset: usize, word_value: u16) {
    let [high, low] = word_value.to_be_bytes();
    buf[offset] = high;
    buf[offset + 1] = low;
}

fn pattern_base_offset(num_instr: usize, num_voices: usize) -> usize {
    let mut offset = 20 + (num_instr * 30) + 1 + 1 + 128;
    if num_instr != 15 {
        offset += 4;
    }
    let _ = num_voices;
    offset
}

fn build_mod_with_id(id: [u8; 4], voices: usize) -> Vec<u8> {
    let num_instr = 31usize;
    let num_patterns = 2usize;
    let pattern_bytes = 4 * 64 * voices * num_patterns;
    let sample_bytes = 8usize + 4usize;
    let total = pattern_base_offset(num_instr, voices) + pattern_bytes + sample_bytes;
    let mut bytes = vec![0u8; total];

    bytes[..20].copy_from_slice(b"TEST MOD SONG NAME!!");
    let mut inst0_name = [b' '; 22];
    inst0_name[..11].copy_from_slice(b"BASS SAMPLE");
    bytes[20..42].copy_from_slice(&inst0_name);
    write_be_u16_words(&mut bytes, 42, 4);
    bytes[44] = 0x0f;
    bytes[45] = 0x7f;
    write_be_u16_words(&mut bytes, 46, 1);
    write_be_u16_words(&mut bytes, 48, 2);

    let inst1 = 50usize;
    let mut inst1_name = [b' '; 22];
    inst1_name[..8].copy_from_slice(b"NO LOOP ");
    bytes[inst1..inst1 + 22].copy_from_slice(&inst1_name);
    write_be_u16_words(&mut bytes, inst1 + 22, 2);
    bytes[inst1 + 24] = 0x00;
    bytes[inst1 + 25] = 0x20;
    write_be_u16_words(&mut bytes, inst1 + 26, 0);
    write_be_u16_words(&mut bytes, inst1 + 28, 1);

    let song_len_offset = 20 + (num_instr * 30);
    bytes[song_len_offset] = 2;
    bytes[song_len_offset + 1] = 0x7f;
    bytes[song_len_offset + 2] = 0;
    bytes[song_len_offset + 3] = 65;

    let id_offset = song_len_offset + 2 + 128;
    bytes[id_offset..id_offset + 4].copy_from_slice(&id);

    let patterns_offset = pattern_base_offset(num_instr, voices);
    let line0_ch0 = patterns_offset;
    bytes[line0_ch0] = 0x11;
    bytes[line0_ch0 + 1] = 0x23;
    bytes[line0_ch0 + 2] = 0x45;
    bytes[line0_ch0 + 3] = 0x67;

    let line1_ch2 = patterns_offset + (4 * voices) + (2 * 4);
    bytes[line1_ch2] = 0xab;
    bytes[line1_ch2 + 1] = 0xcd;
    bytes[line1_ch2 + 2] = 0xef;
    bytes[line1_ch2 + 3] = 0x01;

    let pattern1_base = patterns_offset + (4 * 64 * voices);
    bytes[pattern1_base] = 0x20;
    bytes[pattern1_base + 1] = 0x10;
    bytes[pattern1_base + 2] = 0x30;
    bytes[pattern1_base + 3] = 0x40;

    let sample_offset = patterns_offset + pattern_bytes;
    for idx in 0..8 {
        bytes[sample_offset + idx] = idx as u8;
    }
    for idx in 0..4 {
        bytes[sample_offset + 8 + idx] = (idx as u8) + 8;
    }

    bytes
}

fn build_minimal_mod() -> Vec<u8> {
    let num_instr = 31usize;
    let voices = 4usize;
    let pattern_bytes = 4 * 64 * voices;
    let total = 20 + (num_instr * 30) + 1 + 1 + 128 + 4 + pattern_bytes + 4;
    let mut bytes = vec![0u8; total];
    bytes[..20].copy_from_slice(b"STATE MANAGEMENT!!! ");
    bytes[20 + (num_instr * 30)] = 1;
    bytes[20 + (num_instr * 30) + 1] = 0;
    bytes[20 + (num_instr * 30) + 2] = 0;
    bytes[20 + (num_instr * 30) + 3] = 0;
    bytes[20 + (num_instr * 30) + 1 + 1 + 128..20 + (num_instr * 30) + 1 + 1 + 128 + 4]
        .copy_from_slice(b"M.K.");
    bytes
}

#[test]
fn alloc_sfx_channels_clamps_and_sets_shiftval() {
    let mut bytes = build_minimal_mod();
    let mut parsed = unsafe { core::mem::zeroed::<Mod>() };

    assert_eq!(mod_set_mod_from_bytes(&mut parsed, &mut bytes), 0);
    // SAFETY: All pointers below refer to live, initialized test values and buffers.
    assert_eq!(unsafe { mod_alloc_sfx_channels(&mut parsed, 0) }, 0);
    assert_eq!(parsed.num_channels, 4);
    assert_eq!(parsed.shiftval, 6);

    assert_eq!(unsafe { mod_alloc_sfx_channels(&mut parsed, 2) }, 0);
    assert_eq!(parsed.num_channels, 6);
    assert_eq!(parsed.shiftval, 5);

    assert_eq!(unsafe { mod_alloc_sfx_channels(&mut parsed, -1) }, -1);
    assert_eq!(parsed.num_channels, 6);

    assert_eq!(unsafe { mod_alloc_sfx_channels(&mut parsed, 30) }, -1);
    assert_eq!(parsed.num_channels, 4);
    assert_eq!(parsed.shiftval, 6);
}

#[test]
fn free_state_clears_runtime_fields() {
    let mut bytes = build_minimal_mod();
    let mut parsed = unsafe { core::mem::zeroed::<Mod>() };

    assert_eq!(mod_set_mod_from_bytes(&mut parsed, &mut bytes), 0);
    assert_eq!(parsed.set, 1 as Bool);
    mod_free_state(&mut parsed);

    assert_eq!(parsed.set, 0);
    assert_eq!(parsed.loaded, 0);
    assert_eq!(parsed.num_instr, 31);
    assert_eq!(parsed.modraw, bytes.as_mut_ptr());
    assert!(!parsed.patterndata.is_null());
    assert_eq!(parsed.musicvolume, 0x40);
    assert_eq!(parsed.sfxvolume, 0x40);
}

#[test]
fn mod_free_releases_loaded_buffer_and_tolerates_repeated_free()
-> Result<(), Box<dyn std::error::Error>> {
    let bytes = build_minimal_mod();
    let (path, c_path) = create_module_file(&bytes)?;
    let mut parsed = unsafe { core::mem::zeroed::<Mod>() };

    // SAFETY: The output and NUL-terminated path remain valid for the call.
    assert_eq!(unsafe { mod_load(&mut parsed, c_path.as_ptr().cast()) }, 0);
    assert_eq!(parsed.loaded, 1 as Bool);
    assert!(!parsed.modraw.is_null());

    // SAFETY: `parsed` remains a valid unique pointer for both calls.
    unsafe { mod_free(&mut parsed) };
    assert_eq!(parsed.loaded, 0);
    assert_eq!(parsed.set, 0);
    assert!(parsed.modraw.is_null());
    assert!(parsed.patterndata.is_null());

    unsafe { mod_free(&mut parsed) };
    assert_eq!(parsed.loaded, 0);
    assert!(parsed.modraw.is_null());

    std::fs::remove_file(path)?;
    Ok(())
}

#[test]
fn mod_free_preserves_caller_owned_mod_set_mod_buffer() {
    let mut bytes = build_minimal_mod();
    let mut parsed = unsafe { core::mem::zeroed::<Mod>() };
    let ptr = bytes.as_mut_ptr();

    // SAFETY: `ptr` refers to the live module buffer for the duration of this test.
    assert_eq!(unsafe { mod_set_mod(&mut parsed, ptr) }, 0);
    assert_eq!(parsed.loaded, 0);
    assert_eq!(parsed.modraw, ptr);

    unsafe { mod_free(&mut parsed) };
    assert_eq!(parsed.loaded, 0);
    assert_eq!(parsed.set, 0);
    assert_eq!(parsed.modraw, ptr);
    assert!(!parsed.patterndata.is_null());

    unsafe { mod_free(&mut parsed) };
    assert_eq!(parsed.modraw, ptr);
}

#[test]
fn mod_load_reads_file_and_marks_owned_buffer() -> Result<(), Box<dyn std::error::Error>> {
    let bytes = build_mod_with_id(*b"M.K.", 4);
    let (path, c_path) = create_module_file(&bytes)?;
    let mut parsed = unsafe { core::mem::zeroed::<Mod>() };

    // SAFETY: The output and NUL-terminated path remain valid for the call.
    assert_eq!(
        unsafe { mod_load_from_path(&mut parsed, c_path.as_ptr().cast()) },
        0
    );
    assert_eq!(parsed.loaded, 1 as Bool);
    assert!(!parsed.modraw.is_null());
    assert_eq!(parsed.num_voices, 4);
    assert_eq!(parsed.song_length, 2);

    mod_free_state(&mut parsed);
    assert_eq!(parsed.loaded, 0);
    assert!(parsed.modraw.is_null());

    std::fs::remove_file(path)?;
    Ok(())
}

#[test]
fn modplay_state_helpers_update_embedded_mod() {
    let mut handle = unsafe { core::mem::zeroed::<ModPlay>() };
    modplay_init_state(&mut handle);
    let handle_ptr = (&mut handle as *mut ModPlay) as *mut c_void;

    assert_eq!(handle.playfreq, 48_000);
    assert_eq!(handle.stereo, 1 as Bool);
    assert_eq!(handle.sound_buf.freq, 48_000);
    assert_eq!(handle.sound_buf.chans, 2);
    assert_eq!(handle.sound_buf.fmt, 16);
    assert!(handle.sound_buf.callback.is_some());
    assert_eq!(handle.sound_buf.usr_data, handle_ptr);
    assert_eq!(handle.mod_.freq, 48_000);
    assert_eq!(handle.mod_.channels, 2);
    assert_eq!(handle.mod_.bits, 16);
    assert_eq!(handle.sound_buf.samples, 960.0);

    assert_eq!(modplay_set_frequency_state(&mut handle, 32_000), 0);
    assert_eq!(handle.playfreq, 32_000);
    assert_eq!(handle.sound_buf.freq, 32_000);
    assert_eq!(handle.mod_.freq, 32_000);
    assert_eq!(handle.sound_buf.samples, 640.0);
    assert!(handle.sound_buf.callback.is_some());

    modplay_set_stereo_state(&mut handle, 0);
    assert_eq!(handle.stereo, 0);
    assert_eq!(handle.sound_buf.chans, 1);
    assert_eq!(handle.mod_.channels, 1);
    assert!(handle.sound_buf.callback.is_some());

    modplay_set_volume_state(&mut handle, 99, -5);
    assert_eq!(handle.mod_.musicvolume, 64);
    assert_eq!(handle.mod_.sfxvolume, 0);

    modplay_unload_state(&mut handle);
    assert_eq!(handle.playing, 0);
    assert_eq!(handle.paused, 0);
    assert_eq!(handle.mod_.loaded, 0);
}

#[test]
fn modplay_load_alloc_and_trigger_route_through_mod_state() {
    let mut handle = unsafe { core::mem::zeroed::<ModPlay>() };
    modplay_init_state(&mut handle);
    let bytes = build_mod_with_id(*b"M.K.", 4);
    let (mem, mem_len) = alloc_bytes(&bytes);
    // SAFETY: `mem` refers to the allocated module image until it is freed below.
    assert_eq!(unsafe { modplay_set_mod_state(&mut handle, mem) }, 0);
    assert_eq!(handle.mod_.loaded, 0);
    assert_eq!(handle.mod_.num_voices, 4);

    assert_eq!(modplay_set_frequency_state(&mut handle, 48_000), 0);
    assert_eq!(
        modplay_trigger_note_state(&mut handle, 0, 0, 0x123, 0x20),
        -1
    );

    assert_eq!(modplay_alloc_sfx_channels_state(&mut handle, 1), 0);
    assert_eq!(
        modplay_trigger_note_state(&mut handle, 0, 0, 0x123, 0x20),
        0
    );
    assert_eq!(handle.mod_.playpos[handle.mod_.num_voices as usize], 0);
    modplay_unload_state(&mut handle);
    unsafe {
        free_bytes(mem, mem_len);
    }
}

#[test]
fn modplay_public_wrappers_update_state() {
    let mut handle = unsafe { core::mem::zeroed::<ModPlay>() };
    let bytes = build_mod_with_id(*b"M.K.", 4);
    let (mem, mem_len) = alloc_bytes(&bytes);
    // SAFETY: All pointers below refer to the live handle and module buffer.
    unsafe { mod_play_init(&mut handle) };
    assert_eq!(handle.playfreq, 48_000);
    assert_eq!(unsafe { mod_play_set_frequency(&mut handle, 32_000) }, 0);
    unsafe { mod_play_set_stereo(&mut handle, 0) };
    unsafe { mod_play_set_volume(&mut handle, 70, -4) };
    assert_eq!(
        unsafe { mod_play_set_mod(&mut handle, mem as *const c_void) },
        0
    );
    assert_eq!(unsafe { mod_play_alloc_sfx_channels(&mut handle, 1) }, 0);
    assert_eq!(unsafe { mod_play_start(&mut handle) }, 0);
    assert_eq!(handle.playing, 1 as Bool);
    assert!(handle.sound_buf.callback.is_some());
    assert_eq!(handle.sound_buf.freq, 32_000);
    assert_eq!(handle.sound_buf.chans, 1);
    assert_eq!(handle.sound_buf.fmt, 16);
    assert_eq!(unsafe { mod_play_pause(&mut handle, 1) }, 0);
    assert_eq!(
        unsafe { mod_play_trigger_note(&mut handle, 0, 0, 0x123, 0x20) },
        0
    );
    assert_eq!(unsafe { mod_play_stop(&mut handle) }, 0);
    assert_eq!(handle.playing, 0);
    unsafe { mod_play_unload(&mut handle) };
    unsafe {
        free_bytes(mem, mem_len);
    }
}

#[test]
fn modplay_active_format_changes_reset_playback_phase() {
    let mut handle = unsafe { core::mem::zeroed::<ModPlay>() };
    let bytes = build_mod_with_id(*b"M.K.", 4);
    let (mem, mem_len) = alloc_bytes(&bytes);
    // SAFETY: All pointers below refer to the live handle and module buffer.
    unsafe { mod_play_init(&mut handle) };
    unsafe { mod_play_set_stereo(&mut handle, 0) };
    assert_eq!(
        unsafe { mod_play_set_mod(&mut handle, mem as *const c_void) },
        0
    );
    assert_eq!(unsafe { mod_play_start(&mut handle) }, 0);

    handle.mod_.samplescounter = 17;
    handle.mod_.samplespertick = 999;
    assert_eq!(unsafe { mod_play_set_frequency(&mut handle, 32_000) }, 0);
    assert_eq!(handle.mod_.samplescounter, 0);
    assert_eq!(handle.mod_.samplespertick, unsafe {
        *handle.mod_.bpmtab.add((handle.mod_.bpm - 32) as usize)
    });

    handle.mod_.samplescounter = 19;
    handle.mod_.samplespertick = 777;
    unsafe { mod_play_set_stereo(&mut handle, 1) };
    assert_eq!(handle.mod_.samplescounter, 0);
    assert_eq!(handle.mod_.samplespertick, unsafe {
        *handle.mod_.bpmtab.add((handle.mod_.bpm - 32) as usize)
    });

    unsafe { mod_play_unload(&mut handle) };
    unsafe {
        free_bytes(mem, mem_len);
    }
}

#[test]
fn modplay_unload_preserves_caller_owned_set_mod_buffer() {
    let mut handle = unsafe { core::mem::zeroed::<ModPlay>() };
    let bytes = build_mod_with_id(*b"M.K.", 4);
    let (mem, mem_len) = alloc_bytes(&bytes);

    // SAFETY: All pointers below refer to the live handle and module buffer.
    unsafe { mod_play_init(&mut handle) };
    assert_eq!(
        unsafe { mod_play_set_mod(&mut handle, mem as *const c_void) },
        0
    );
    assert_eq!(handle.mod_.loaded, 0);
    assert_eq!(handle.mod_.modraw, mem);

    unsafe { mod_play_unload(&mut handle) };
    assert_eq!(handle.playing, 0);
    assert_eq!(handle.paused, 0);
    assert_eq!(handle.mod_.loaded, 0);
    assert_eq!(handle.mod_.set, 0);
    assert_eq!(handle.mod_.modraw, mem);

    unsafe { mod_play_unload(&mut handle) };
    assert_eq!(handle.mod_.modraw, mem);
    unsafe {
        free_bytes(mem, mem_len);
    }
}

#[test]
fn modplay_mix_callback_respects_manual_polling_and_pause() {
    let mut handle = unsafe { core::mem::zeroed::<ModPlay>() };
    modplay_init_state(&mut handle);
    handle.manual_polling = 1;
    handle.paused = 1;

    let mut stream = [9u8, 8, 7, 6];
    let retval = modplay_mix_callback_state(&mut handle, &mut stream);

    assert_eq!(retval, 0);
    assert_eq!(stream, [0, 0, 0, 0]);
    assert_eq!(handle.mod_.notify, &mut handle.paused as *mut Bool);
    assert_eq!(handle.mod_.mixingbuf, stream.as_mut_ptr());
    assert_eq!(handle.mod_.mixingbuflen, stream.len() as i32);

    handle.manual_polling = 0;
    handle.paused = 0;
    handle.playing = 1;
    handle.mod_.samplespertick = 1;
    handle.mod_.speed = 6;
    handle.mod_.bpm = 125;
    let retval = modplay_mix_callback_state(&mut handle, &mut stream);
    assert_eq!(retval, 0);
    assert!(handle.mod_.notify.is_null());
}

#[test]
fn modplay_debug_timing_accessors_stay_zero_initially() {
    assert_eq!(mod_play_mixing_time(), 0);
    assert_eq!(mod_play_request_time(), 0);
}

#[test]
fn restart_preserves_pause_after_stop() {
    let mut handle = unsafe { core::mem::zeroed::<ModPlay>() };
    let bytes = build_mod_with_id(*b"M.K.", 4);
    let (mem, mem_len) = alloc_bytes(&bytes);
    modplay_init_state(&mut handle);
    assert_eq!(unsafe { modplay_set_mod_state(&mut handle, mem) }, 0);
    assert_eq!(start_state(&mut handle), 0);
    assert_eq!(unsafe { mod_play_pause(&mut handle, 1) }, 0);
    assert_eq!(stop_state(&mut handle), 0);

    assert_eq!(start_state(&mut handle), 0);
    assert_eq!(handle.paused, 1);
    assert_eq!(stop_state(&mut handle), 0);
    unsafe { free_bytes(mem, mem_len) };
}

#[test]
fn mod_start_resets_runtime_state_and_sets_timing_tables() {
    let mut bytes = build_minimal_mod();
    let mut parsed = unsafe { core::mem::zeroed::<Mod>() };

    assert_eq!(mod_set_mod_from_bytes(&mut parsed, &mut bytes), 0);
    parsed.freq = 48_000;
    parsed.speed = 99;
    parsed.bpm = 222;
    parsed.songpos = 7;
    parsed.patternline = 33;
    parsed.speedcounter = 17;
    parsed.patterndelay = 5;
    parsed.patternline_jumpto = 11;
    parsed.patternline_jumpcount = 3;
    parsed.volume[0] = 12;
    parsed.playpos[0] = 99;
    parsed.chanfreq[0] = 55;
    parsed.instnum[0] = 9;
    parsed.channote[0] = 10;
    parsed.sintabpos[0] = 4;
    parsed.vib_freq[0] = 3;
    parsed.vib_depth[0] = 2;
    parsed.last_effect[0] = 1;
    parsed.glissando[0] = 1;
    parsed.trem_wave[0] = 2;
    parsed.vib_wave[0] = 3;
    parsed.channel_active[0] = 1;

    // SAFETY: `parsed` is a live, validated module value.
    unsafe { mod_start(&mut parsed) };

    assert_eq!(parsed.songpos, 0);
    assert_eq!(parsed.patternline, 0);
    assert_eq!(parsed.speed, 6);
    assert_eq!(parsed.bpm, 125);
    assert_eq!(parsed.speedcounter, 0);
    assert_eq!(parsed.patterndelay, 0);
    assert_eq!(parsed.patternline_jumpto, 0);
    assert_eq!(parsed.patternline_jumpcount, 0);
    assert_eq!(parsed.samplescounter, 0);
    assert!(!parsed.bpmtab.is_null());
    assert!(!parsed.inctab.is_null());
    assert_eq!(parsed.volume[0], 0);
    assert_eq!(parsed.playpos[0], 0);
    assert_eq!(parsed.chanfreq[0], 0);
    assert_eq!(parsed.instnum[0], 0);
    assert_eq!(parsed.channote[0], 0);
    assert_eq!(parsed.sintabpos[0], 0);
    assert_eq!(parsed.vib_freq[0], 0);
    assert_eq!(parsed.vib_depth[0], 0);
    assert_eq!(parsed.last_effect[0], 0);
    assert_eq!(parsed.glissando[0], 0);
    assert_eq!(parsed.trem_wave[0], 0);
    assert_eq!(parsed.vib_wave[0], 0);
    assert_eq!(parsed.channel_active[0], 0);
    assert_eq!(parsed.samplespertick, unsafe {
        *parsed.bpmtab.add((125 - 32) as usize)
    });
}

#[test]
fn trigger_note_updates_sfx_voice_state() {
    let mut bytes = build_mod_with_id(*b"M.K.", 4);
    let mut parsed = unsafe { core::mem::zeroed::<Mod>() };

    assert_eq!(mod_set_mod_from_bytes(&mut parsed, &mut bytes), 0);
    // SAFETY: `parsed` is a live, validated module value.
    assert_eq!(unsafe { mod_alloc_sfx_channels(&mut parsed, 1) }, 0);

    assert_eq!(
        unsafe { mod_trigger_note(&mut parsed, 0, 0, 0x123, 0x20) },
        0
    );

    let voice = parsed.num_voices as usize;
    assert_eq!(parsed.playpos[voice], 0);
    assert_eq!(parsed.instnum[voice], 0);
    assert_eq!(parsed.chanfreq[voice], 0x123);
    assert_eq!(parsed.volume[voice], 0x20);

    assert_eq!(
        unsafe { mod_trigger_note(&mut parsed, 0, 0xff, 0x234, 0x10) },
        0
    );
    assert_eq!(parsed.instnum[voice], 0);
    assert_eq!(parsed.chanfreq[voice], 0x234);
    assert_eq!(parsed.volume[voice], 0x10);
}

#[test]
fn effect_handler_applies_arpeggio_on_tick() {
    let mut bytes = build_minimal_mod();
    let mut parsed = unsafe { core::mem::zeroed::<Mod>() };

    assert_eq!(mod_set_mod_from_bytes(&mut parsed, &mut bytes), 0);
    parsed.num_voices = 1;
    parsed.effect[0] = 0x00;
    parsed.effectop[0] = 0x12;
    parsed.channote[0] = 1;
    parsed.speedcounter = 1;

    let retval = mod_effect_handler_state(&mut parsed);

    assert_eq!(retval, 0);
    assert_eq!(parsed.chanfreq[0], FREQTAB[1] + 1);
    assert_eq!(parsed.arp_counter, 1);
}

#[test]
fn effect_handler_clamps_slides_and_handles_negative_vibrato() {
    let mut bytes = build_minimal_mod();
    let mut parsed = unsafe { core::mem::zeroed::<Mod>() };

    assert_eq!(mod_set_mod_from_bytes(&mut parsed, &mut bytes), 0);
    parsed.num_voices = 1;
    parsed.speedcounter = 1;
    parsed.effect[0] = 0x01;
    parsed.effectop[0] = 2;
    parsed.chanfreq[0] = 1;
    mod_effect_handler_state(&mut parsed);
    assert_eq!(parsed.chanfreq[0], 0);

    parsed.effect[0] = 0x0a;
    parsed.effectop[0] = 5;
    parsed.volume[0] = 3;
    mod_effect_handler_state(&mut parsed);
    assert_eq!(parsed.volume[0], 0);

    parsed.effect[0] = 0x04;
    parsed.vib_wave[0] = 0;
    parsed.sintabpos[0] = 33;
    parsed.vib_depth[0] = 8;
    parsed.vib_basefreq[0] = 500;
    mod_effect_handler_state(&mut parsed);
    assert_eq!(parsed.chanfreq[0], 498);
}

#[test]
fn process_uses_pattern_note_semantics() {
    let mut bytes = build_mod_with_id(*b"M.K.", 4);
    let pattern_offset = 20 + (31 * 30) + 1 + 1 + 128 + 4;
    bytes[pattern_offset..pattern_offset + 4].copy_from_slice(&[0x01, 0x23, 0x10, 0x00]);
    let mut parsed = unsafe { core::mem::zeroed::<Mod>() };

    assert_eq!(mod_set_mod_from_bytes(&mut parsed, &mut bytes), 0);
    let changed = mod_process_state(&mut parsed);

    assert_eq!(changed & 1, 1);
    assert_eq!(parsed.instnum[0], 0);
    assert_eq!(parsed.channote[0], 0x123);
    assert_eq!(parsed.chanfreq[0], 0x123);
    assert_eq!(parsed.volume[0], 0x40);
    assert_eq!(parsed.channel_active[0], 1);
}

#[test]
fn process_state_handles_pattern_break() {
    let mut bytes = build_mod_with_id(*b"M.K.", 4);
    let pattern_offset = 20 + (31 * 30) + 1 + 1 + 128 + 4;
    bytes[pattern_offset + 2] = 0x4d;
    let mut parsed = unsafe { core::mem::zeroed::<Mod>() };

    assert_eq!(mod_set_mod_from_bytes(&mut parsed, &mut bytes), 0);
    let _ = mod_process_state(&mut parsed);

    assert_eq!(parsed.songpos, 1);
    assert_eq!(parsed.patternline, 0);
    assert_eq!(parsed.patternline_jumpto, 0);
    assert_eq!(parsed.patternline_jumpcount, 0);
}

#[test]
fn mod_player_mixes_samples_and_invokes_callback() {
    let mut bytes = build_mod_with_id(*b"M.K.", 4);
    let num_instr = 31usize;
    let voices = 4usize;
    let pattern_bytes = 4 * 64 * voices * 2;
    let sample_offset = 20 + (num_instr * 30) + 1 + 1 + 128 + 4 + pattern_bytes;
    bytes[sample_offset] = 0x40;

    let mut parsed = unsafe { core::mem::zeroed::<Mod>() };
    let mut mixingbuf = [0i16; 2];

    assert_eq!(mod_set_mod_from_bytes(&mut parsed, &mut bytes), 0);
    // SAFETY: `parsed` and its mixing buffer remain live for both calls.
    unsafe { mod_start(&mut parsed) };
    parsed.volume[0] = 0x40;
    parsed.channel_active[0] = 1;
    parsed.instnum[0] = 0;
    parsed.chanfreq[0] = 0;
    parsed.playpos[0] = 0;
    parsed.bits = 16;
    parsed.channels = 1;
    parsed.mixingbuf = mixingbuf.as_mut_ptr() as *mut u8;
    parsed.mixingbuflen = (mixingbuf.len() * core::mem::size_of::<i16>()) as i32;
    parsed.samplespertick = 100;
    parsed.samplescounter = 0;
    parsed.callback = Some(player_callback);
    PLAYER_CALLBACK_RAN.store(false, Ordering::SeqCst);

    let retval = unsafe { mod_player(&mut parsed) };

    assert_eq!(retval, 0);
    assert_eq!(parsed.notebeats, 0);
    assert_eq!(parsed.samplescounter, 2);
    assert_eq!(mixingbuf[0], 0x1000);
    assert_eq!(mixingbuf[1], 0x1000);
    assert!(PLAYER_CALLBACK_RAN.load(Ordering::SeqCst));
}
