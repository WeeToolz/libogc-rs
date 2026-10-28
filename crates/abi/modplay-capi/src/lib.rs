#![no_std]

extern crate alloc;
#[cfg(not(target_arch = "powerpc"))]
extern crate std;

use alloc::alloc::alloc_zeroed;
use alloc::alloc::{Layout, dealloc};
#[cfg(target_arch = "powerpc")]
use alloc::vec::Vec;
#[cfg(target_arch = "powerpc")]
use core::ffi::CStr;
use core::ffi::c_void;
use core::ptr;
#[cfg(target_arch = "powerpc")]
use core::sync::atomic::AtomicU32;
use core::sync::atomic::{AtomicBool, Ordering};

pub use modplay::{FREQTAB, FREQTAB_LEN, SEMITONETAB, SEMITONETAB_LEN};
mod wavetab;
#[cfg(target_arch = "powerpc")]
use ogc_audio as audio_backend;
#[cfg(target_arch = "powerpc")]
use ogc_cache::dc_flush_range;
#[cfg(target_arch = "powerpc")]
use ogc_core::mem_virtual_to_physical;
#[cfg(target_arch = "powerpc")]
use ogc_ipc::{FileDescriptor, IPC_OPEN_READ, open_file};
#[cfg(target_arch = "powerpc")]
use ogc_lwp_sys as lwp;
use ogc_sys::{Bool, F32};
#[cfg(target_arch = "powerpc")]
use ogc_system::{SYS_VIDEO_PAL, get_video_mode};

const MOD_ALLOC_HEADER: usize = core::mem::size_of::<usize>();

pub const MAX_VOICES: usize = 32;
const TRUE_BOOL: Bool = 1;
const FALSE_BOOL: Bool = 0;
const SHIFTVALS: [i32; 33] = [
    0, 8, 7, 6, 6, 5, 5, 5, 5, 4, 4, 4, 4, 4, 4, 4, 4, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3, 3,
    3,
];
const BPMTAB_LEN: usize = 224;
const INCTAB_LEN: usize = 4096;
const NTSC_SAMPLE_CLOCK: f32 = 7159090.5 / 2.0;
#[cfg(target_arch = "powerpc")]
const PAL_SAMPLE_CLOCK: f32 = 7093789.2 / 2.0;

static BPMTAB_32K_READY: AtomicBool = AtomicBool::new(false);
static BPMTAB_48K_READY: AtomicBool = AtomicBool::new(false);
static INCTAB_32K_READY: AtomicBool = AtomicBool::new(false);
static INCTAB_48K_READY: AtomicBool = AtomicBool::new(false);

static mut BPMTAB_32K: [u32; BPMTAB_LEN] = [0; BPMTAB_LEN];
static mut BPMTAB_48K: [u32; BPMTAB_LEN] = [0; BPMTAB_LEN];
static mut INCTAB_32K: [u32; INCTAB_LEN] = [0; INCTAB_LEN];
static mut INCTAB_48K: [u32; INCTAB_LEN] = [0; INCTAB_LEN];
static MIXING_TIME_US: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
static REQUEST_TIME_US: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
static THR_RUNNING: AtomicBool = AtomicBool::new(false);
static SND_PLAYING: AtomicBool = AtomicBool::new(false);
static CURRENT_AUDIO: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
static SHIFT_VAL: core::sync::atomic::AtomicU32 = core::sync::atomic::AtomicU32::new(0);
#[cfg(target_arch = "powerpc")]
const PLAYER_STACK_SIZE: usize = 8 * 1024;
#[cfg(any(target_arch = "powerpc", test))]
const SOUND_BUFFER_SIZE: u32 = 5760;
#[cfg(target_arch = "powerpc")]
const PLAYER_PRIORITY: u8 = 80;
#[cfg(target_arch = "powerpc")]
static PLAYER_QUEUE: AtomicU32 = AtomicU32::new(lwp::LWP_TQUEUE_NULL);
#[cfg(target_arch = "powerpc")]
static PLAYER_THREAD: AtomicU32 = AtomicU32::new(lwp::LWP_THREAD_NULL);
#[cfg(target_arch = "powerpc")]
#[repr(C, align(8))]
struct PlayerStack([u8; PLAYER_STACK_SIZE]);
#[cfg(target_arch = "powerpc")]
static mut PLAYER_STACK: PlayerStack = PlayerStack([0; PLAYER_STACK_SIZE]);
#[cfg(target_arch = "powerpc")]
static PLAYER_WORKER_FAILED: AtomicBool = AtomicBool::new(false);
#[repr(align(32))]
#[cfg(target_arch = "powerpc")]
struct AudioBuffers([[u8; SOUND_BUFFER_SIZE as usize]; 2]);

#[cfg(target_arch = "powerpc")]
static mut AUDIO_BUFFER: AudioBuffers = AudioBuffers([[0; SOUND_BUFFER_SIZE as usize]; 2]);
static mut SND_BUFFER: ModSndBuf = ModSndBuf {
    freq: 0,
    fmt: 0,
    chans: 0,
    samples: 0.0,
    usr_data: ptr::null_mut(),
    callback: None,
};

unsafe fn mod_alloc_owned(bytes: &[u8]) -> Option<*mut u8> {
    let total = bytes.len().checked_add(MOD_ALLOC_HEADER)?;
    let layout = Layout::from_size_align(total, core::mem::align_of::<usize>()).ok()?;
    // SAFETY: `layout` is valid, and the returned allocation is checked for null
    // before the header and payload are written within the allocated range.
    unsafe {
        let base = alloc_zeroed(layout);
        if base.is_null() {
            return None;
        }
        (base as *mut usize).write(bytes.len());
        let data = base.add(MOD_ALLOC_HEADER);
        ptr::copy_nonoverlapping(bytes.as_ptr(), data, bytes.len());
        Some(data)
    }
}

unsafe fn mod_free_owned(ptr: *mut u8) {
    if ptr.is_null() {
        return;
    }
    let base = unsafe { ptr.sub(MOD_ALLOC_HEADER) };
    let len = unsafe { (base as *const usize).read() };
    let total = match len.checked_add(MOD_ALLOC_HEADER) {
        Some(total) => total,
        None => return,
    };
    if let Ok(layout) = Layout::from_size_align(total, core::mem::align_of::<usize>()) {
        unsafe {
            dealloc(base, layout);
        }
    }
}

#[repr(C)]
pub struct ModInstr {
    pub name: [i8; 23],
    pub length: u32,
    pub finetune: u8,
    pub volume: u8,
    pub loop_start: u32,
    pub loop_end: u32,
    pub loop_length: u32,
    pub looped: Bool,
    pub data: *mut i8,
}

#[repr(C)]
pub struct Mod {
    pub loaded: Bool,
    pub name: [i8; 21],
    pub instrument: [ModInstr; 31],
    pub num_patterns: i32,
    pub song_length: u8,
    pub ciaa: u8,
    pub song: [u8; 128],
    pub id: [i8; 4],
    pub patterndata: *mut u8,
    pub num_instr: i32,
    pub num_voices: i32,
    pub num_channels: i32,
    pub mixingbuf: *mut u8,
    pub mixingbuflen: i32,
    pub shiftval: i32,
    pub channel_active: [Bool; MAX_VOICES],
    pub patterndelay: i32,
    pub speed: i32,
    pub bpm: i32,
    pub songpos: i32,
    pub patternline: i32,
    pub patternline_jumpto: i32,
    pub patternline_jumpcount: i32,
    pub speedcounter: i32,
    pub freq: i32,
    pub bits: i32,
    pub channels: i32,
    pub playpos: [u32; MAX_VOICES],
    pub instnum: [u8; MAX_VOICES],
    pub chanfreq: [u16; MAX_VOICES],
    pub channote: [u16; MAX_VOICES],
    pub volume: [u8; MAX_VOICES],
    pub effect: [u8; MAX_VOICES],
    pub effectop: [u8; MAX_VOICES],
    pub last_effect: [u8; MAX_VOICES],
    pub portamento_to: [u16; MAX_VOICES],
    pub porta_speed: [u8; MAX_VOICES],
    pub retrigger_counter: [u8; MAX_VOICES],
    pub arp_counter: u8,
    pub sintabpos: [u8; MAX_VOICES],
    pub vib_freq: [u8; MAX_VOICES],
    pub vib_depth: [u8; MAX_VOICES],
    pub vib_basefreq: [u16; MAX_VOICES],
    pub trem_basevol: [u8; MAX_VOICES],
    pub trem_freq: [u8; MAX_VOICES],
    pub trem_depth: [u8; MAX_VOICES],
    pub glissando: [Bool; MAX_VOICES],
    pub trem_wave: [u8; MAX_VOICES],
    pub vib_wave: [u8; MAX_VOICES],
    pub nextinstr: [u8; MAX_VOICES],
    pub nextnote: [u16; MAX_VOICES],
    pub samplespertick: u32,
    pub samplescounter: u32,
    pub modraw: *mut u8,
    pub bpmtab: *mut u32,
    pub inctab: *mut u32,
    pub notebeats: u32,
    pub callback: Option<unsafe extern "C" fn(*mut c_void)>,
    pub musicvolume: u8,
    pub sfxvolume: u8,
    pub set: Bool,
    pub notify: *mut Bool,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub struct ModSndBuf {
    pub freq: u32,
    pub fmt: u16,
    pub chans: u32,
    pub samples: F32,
    pub usr_data: *mut c_void,
    pub callback: Option<unsafe extern "C" fn(*mut c_void, *mut u8, u32)>,
}

#[repr(C)]
pub struct ModPlay {
    pub mod_: Mod,
    pub playing: Bool,
    pub paused: Bool,
    pub bits: Bool,
    pub stereo: Bool,
    pub manual_polling: Bool,
    pub playfreq: u32,
    pub num_sfx_chans: u32,
    pub sound_buf: ModSndBuf,
}

pub struct LayoutInfo {
    pub name: &'static str,
    pub size: usize,
    pub align: usize,
}

pub fn layout_snapshot() -> [LayoutInfo; 4] {
    [
        LayoutInfo {
            name: "MOD_INSTR",
            size: core::mem::size_of::<ModInstr>(),
            align: core::mem::align_of::<ModInstr>(),
        },
        LayoutInfo {
            name: "MOD",
            size: core::mem::size_of::<Mod>(),
            align: core::mem::align_of::<Mod>(),
        },
        LayoutInfo {
            name: "MODSNDBUF",
            size: core::mem::size_of::<ModSndBuf>(),
            align: core::mem::align_of::<ModSndBuf>(),
        },
        LayoutInfo {
            name: "MODPlay",
            size: core::mem::size_of::<ModPlay>(),
            align: core::mem::align_of::<ModPlay>(),
        },
    ]
}

pub static FREQTAB_TABLE: [u16; FREQTAB_LEN] = FREQTAB;

pub static SEMITONETAB_TABLE: [u8; SEMITONETAB_LEN] = SEMITONETAB;

fn mod_alloc_sfx_channels_inner(mod_: &mut Mod, sfxchans: i32) -> i32 {
    if mod_.modraw.is_null() || sfxchans < 0 || !(0..=MAX_VOICES as i32).contains(&mod_.num_voices)
    {
        return -1;
    }

    let Some(total_channels) = mod_.num_voices.checked_add(sfxchans) else {
        return -1;
    };
    if total_channels > MAX_VOICES as i32 {
        mod_.num_channels = mod_.num_voices;
    } else {
        mod_.num_channels = total_channels;
    }
    mod_.shiftval = SHIFTVALS[mod_.num_channels as usize];

    if total_channels > MAX_VOICES as i32 {
        -1
    } else {
        0
    }
}

fn build_bpmtab(freq: i32, out: *mut u32) {
    let mut i = 0usize;
    while i < BPMTAB_LEN {
        let bpm = (i as f32) + 32.0;
        unsafe {
            *out.add(i) = ((freq as f32) / (bpm * 0.4)) as u32;
        }
        i += 1;
    }
}

fn build_inctab(freq: i32, sample_clock: f32, out: *mut u32) {
    let mut i = 0usize;
    while i < INCTAB_LEN {
        let value = if i == 0 {
            0
        } else {
            let rfreq = sample_clock / (i as f32);
            ((rfreq / (freq as f32)) * 262_144.0) as u32
        };
        unsafe {
            *out.add(i) = value;
        }
        i += 1;
    }
}

fn sample_clock_for_tv_mode() -> f32 {
    #[cfg(target_arch = "powerpc")]
    {
        if u32::from(get_video_mode()) == SYS_VIDEO_PAL {
            PAL_SAMPLE_CLOCK
        } else {
            NTSC_SAMPLE_CLOCK
        }
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        NTSC_SAMPLE_CLOCK
    }
}

fn bpmtab_for_freq(freq: i32) -> *mut u32 {
    match freq {
        32_000 => {
            if !BPMTAB_32K_READY.load(Ordering::Acquire) {
                build_bpmtab(freq, ptr::addr_of_mut!(BPMTAB_32K).cast::<u32>());
                BPMTAB_32K_READY.store(true, Ordering::Release);
            }
            ptr::addr_of_mut!(BPMTAB_32K).cast::<u32>()
        }
        48_000 => {
            if !BPMTAB_48K_READY.load(Ordering::Acquire) {
                build_bpmtab(48_000, ptr::addr_of_mut!(BPMTAB_48K).cast::<u32>());
                BPMTAB_48K_READY.store(true, Ordering::Release);
            }
            ptr::addr_of_mut!(BPMTAB_48K).cast::<u32>()
        }
        _ => {
            if !BPMTAB_48K_READY.load(Ordering::Acquire) {
                build_bpmtab(48_000, ptr::addr_of_mut!(BPMTAB_48K).cast::<u32>());
                BPMTAB_48K_READY.store(true, Ordering::Release);
            }
            ptr::addr_of_mut!(BPMTAB_48K).cast::<u32>()
        }
    }
}

fn inctab_for_freq(freq: i32) -> *mut u32 {
    let sample_clock = sample_clock_for_tv_mode();
    match freq {
        32_000 => {
            if !INCTAB_32K_READY.load(Ordering::Acquire) {
                build_inctab(
                    freq,
                    sample_clock,
                    ptr::addr_of_mut!(INCTAB_32K).cast::<u32>(),
                );
                INCTAB_32K_READY.store(true, Ordering::Release);
            }
            ptr::addr_of_mut!(INCTAB_32K).cast::<u32>()
        }
        48_000 => {
            if !INCTAB_48K_READY.load(Ordering::Acquire) {
                build_inctab(
                    48_000,
                    sample_clock,
                    ptr::addr_of_mut!(INCTAB_48K).cast::<u32>(),
                );
                INCTAB_48K_READY.store(true, Ordering::Release);
            }
            ptr::addr_of_mut!(INCTAB_48K).cast::<u32>()
        }
        _ => {
            if !INCTAB_48K_READY.load(Ordering::Acquire) {
                build_inctab(
                    48_000,
                    sample_clock,
                    ptr::addr_of_mut!(INCTAB_48K).cast::<u32>(),
                );
                INCTAB_48K_READY.store(true, Ordering::Release);
            }
            ptr::addr_of_mut!(INCTAB_48K).cast::<u32>()
        }
    }
}

fn copy_i8_bytes<const N: usize>(dst: &mut [i8; N], src: *const u8, len: usize) {
    let mut idx = 0;
    while idx < len {
        dst[idx] = unsafe { *src.add(idx) as i8 };
        idx += 1;
    }
}

fn trim_trailing_spaces(buf: &mut [i8]) {
    let mut idx = buf.len();
    while idx > 0 {
        let current = idx - 1;
        if buf[current] != b' ' as i8 {
            break;
        }
        buf[current] = 0;
        idx = current;
    }
}

fn read_be_u16_times2(mem: *const u8, ofs: &mut usize) -> u32 {
    let high = unsafe { *mem.add(*ofs) } as u32;
    let low = unsafe { *mem.add(*ofs + 1) } as u32;
    *ofs += 2;
    (low | (high << 8)) << 1
}

fn classify_voices(id: &[i8; 4]) -> (i32, i32) {
    const IDS: [([u8; 4], i32); 15] = [
        (*b"M.K.", 4),
        (*b"FLT4", 4),
        (*b"2CHN", 2),
        (*b"6CHN", 6),
        (*b"8CHN", 8),
        (*b"10CH", 10),
        (*b"12CH", 12),
        (*b"14CH", 14),
        (*b"16CH", 16),
        (*b"18CH", 18),
        (*b"20CH", 20),
        (*b"22CH", 22),
        (*b"24CH", 24),
        (*b"26CH", 26),
        (*b"28CH", 28),
    ];
    if id
        .iter()
        .map(|value| *value as u8)
        .eq(b"30CH".iter().copied())
    {
        return (31, 30);
    }
    if id
        .iter()
        .map(|value| *value as u8)
        .eq(b"32CH".iter().copied())
    {
        return (31, 32);
    }
    for (candidate, voices) in IDS {
        if id.iter().map(|value| *value as u8).eq(candidate) {
            return (31, voices);
        }
    }
    (15, 4)
}

fn validate_mod_bytes(bytes: &[u8]) -> bool {
    if bytes.len() < 1084 {
        return false;
    }

    let id = [
        bytes[1080] as i8,
        bytes[1081] as i8,
        bytes[1082] as i8,
        bytes[1083] as i8,
    ];
    let (num_instr, num_voices) = classify_voices(&id);
    let num_instr = num_instr as usize;
    let num_voices = num_voices as usize;
    let song_length_offset = 20 + num_instr * 30;
    let arrangement_offset = song_length_offset + 2;
    let header_len = arrangement_offset + 128 + if num_instr == 15 { 0 } else { 4 };
    if bytes.len() < header_len || !(1..=128).contains(&bytes[song_length_offset]) {
        return false;
    }

    let num_patterns = bytes[arrangement_offset..arrangement_offset + 128]
        .iter()
        .map(|entry| entry & 63)
        .max()
        .unwrap_or(0) as usize
        + 1;
    let Some(pattern_len) = 4usize
        .checked_mul(64)
        .and_then(|len| len.checked_mul(num_voices))
        .and_then(|len| len.checked_mul(num_patterns))
    else {
        return false;
    };
    let Some(sample_offset) = header_len.checked_add(pattern_len) else {
        return false;
    };
    if sample_offset > bytes.len() {
        return false;
    }

    let mut sample_len = 0usize;
    for instrument in 0..num_instr {
        let length_offset = 20 + instrument * 30 + 22;
        let length = u16::from_be_bytes([bytes[length_offset], bytes[length_offset + 1]]) as usize;
        let Some(length) = length.checked_mul(2) else {
            return false;
        };
        let Some(next_len) = sample_len.checked_add(length) else {
            return false;
        };
        sample_len = next_len;
    }

    sample_offset
        .checked_add(sample_len)
        .is_some_and(|required| required <= bytes.len())
}

fn current_pattern_data(mod_: &Mod, patternline: i32, channel: i32) -> *mut u8 {
    let song_index = mod_.song[mod_.songpos as usize] as isize;
    let pattern_offset = song_index * mod_.num_voices as isize * 4 * 64;
    let line_offset = ((patternline & 63) as isize) * 4 * mod_.num_voices as isize;
    let channel_offset = channel as isize * 4;
    unsafe {
        mod_.patterndata
            .offset(pattern_offset + line_offset + channel_offset)
    }
}

unsafe fn mod_set_mod_ptr(mod_: *mut Mod, mem: *mut u8) -> i32 {
    // SAFETY: callers validate the MOD image before entry and provide a valid
    // writable `Mod`. All pointer arithmetic below stays within that image.
    unsafe {
        ptr::write_bytes(mod_, 0, 1);
        let mod_ref = &mut *mod_;
        mod_ref.modraw = mem;
        mod_ref.musicvolume = 0x40;
        mod_ref.sfxvolume = 0x40;
        mod_ref.num_instr = 31;

        copy_i8_bytes(&mut mod_ref.id, mem.add(1080), 4);
        let (num_instr, num_voices) = classify_voices(&mod_ref.id);
        mod_ref.num_instr = num_instr;
        mod_ref.num_voices = num_voices;

        let mut ofs = 0usize;
        copy_i8_bytes(&mut mod_ref.name, mem.add(ofs), 20);
        ofs += 20;
        mod_ref.name[20] = 0;

        let mut instrument_index = 0usize;
        while instrument_index < mod_ref.num_instr as usize {
            let instrument = &mut mod_ref.instrument[instrument_index];
            copy_i8_bytes(&mut instrument.name, mem.add(ofs), 22);
            ofs += 22;
            instrument.name[22] = 0;
            trim_trailing_spaces(&mut instrument.name[..22]);

            instrument.length = read_be_u16_times2(mem, &mut ofs);
            instrument.finetune = ((*mem.add(ofs)).wrapping_add(8)) & 15;
            ofs += 1;
            instrument.volume = (*mem.add(ofs)).min(0x40);
            ofs += 1;
            instrument.loop_start = read_be_u16_times2(mem, &mut ofs);
            let loop_len = read_be_u16_times2(mem, &mut ofs);
            instrument.loop_end = loop_len + instrument.loop_start;
            instrument.looped = TRUE_BOOL;
            if loop_len <= 2 {
                instrument.looped = FALSE_BOOL;
                instrument.loop_start = instrument.length;
                instrument.loop_end = instrument.length;
            }
            instrument.loop_length = instrument.loop_end - instrument.loop_start;
            instrument_index += 1;
        }

        mod_ref.song_length = *mem.add(ofs);
        ofs += 1;
        mod_ref.ciaa = *mem.add(ofs);
        ofs += 1;

        ptr::copy_nonoverlapping(mem.add(ofs), mod_ref.song.as_mut_ptr(), 128);
        ofs += 128;
        mod_ref.num_patterns = 0;
        let mut song_index = 0usize;
        while song_index < 128 {
            mod_ref.song[song_index] &= 63;
            if mod_ref.song[song_index] as i32 > mod_ref.num_patterns {
                mod_ref.num_patterns = mod_ref.song[song_index] as i32;
            }
            song_index += 1;
        }
        mod_ref.num_patterns += 1;

        if mod_ref.num_instr != 15 {
            ofs += 4;
        }

        mod_alloc_sfx_channels_inner(mod_ref, 0);
        mod_ref.patterndata = mem.add(ofs);
        ofs += 4 * 64 * mod_ref.num_voices as usize * mod_ref.num_patterns as usize;

        let mut sample_index = 0usize;
        while sample_index < mod_ref.num_instr as usize {
            let instrument = &mut mod_ref.instrument[sample_index];
            instrument.data = ptr::null_mut();
            if instrument.length != 0 {
                instrument.data = mem.add(ofs) as *mut i8;
                ofs += instrument.length as usize;
            }
            sample_index += 1;
        }

        mod_ref.set = TRUE_BOOL;
        0
    }
}

pub fn mod_set_mod_from_bytes(mod_: &mut Mod, mem: &mut [u8]) -> i32 {
    if !validate_mod_bytes(mem) {
        return -1;
    }
    unsafe { mod_set_mod_ptr(mod_ as *mut Mod, mem.as_mut_ptr()) }
}

#[cfg(target_arch = "powerpc")]
fn close_failed_load(fd: FileDescriptor) -> i32 {
    // The load has already failed. Still close the IOS descriptor before the
    // error reaches the caller so repeated failures cannot exhaust handles.
    match fd.close() {
        Ok(()) | Err(_) => -1,
    }
}

/// # Safety
///
/// `fname` must point to a valid NUL-terminated C string for a readable MOD
/// file path.
pub unsafe fn mod_load_from_path(mod_: &mut Mod, fname: *const i8) -> i32 {
    if fname.is_null() {
        return -1;
    }

    #[cfg(target_arch = "powerpc")]
    {
        let c_path = unsafe { CStr::from_ptr(fname.cast()) };
        let fd = match open_file(c_path, IPC_OPEN_READ) {
            Ok(fd) => fd,
            Err(_) => return -1,
        };
        let file_len = match fd.seek(0, 2) {
            Ok(len) if len >= 0 => len as usize,
            _ => return close_failed_load(fd),
        };
        if fd.seek(0, 0).is_err() {
            return close_failed_load(fd);
        }
        let mut bytes = Vec::new();
        if bytes.try_reserve_exact(file_len).is_err() {
            return close_failed_load(fd);
        }
        bytes.resize(file_len, 0);
        if fd.read_exact(&mut bytes).is_err() {
            return close_failed_load(fd);
        }
        if fd.close().is_err() {
            return -1;
        }
        if !validate_mod_bytes(&bytes) {
            return -1;
        }
        let Some(mem) = (unsafe { mod_alloc_owned(&bytes) }) else {
            return -1;
        };
        let result = unsafe { mod_set_mod_ptr(mod_ as *mut Mod, mem) };
        if result < 0 {
            unsafe {
                mod_free_owned(mem);
                ptr::write_bytes(mod_ as *mut Mod, 0, 1);
            }
            return -1;
        }

        mod_.loaded = TRUE_BOOL;
        0
    }

    #[cfg(not(target_arch = "powerpc"))]
    {
        let path = match unsafe { core::ffi::CStr::from_ptr(fname) }.to_str() {
            Ok(path) => path,
            Err(_) => return -1,
        };
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(_) => return -1,
        };
        if !validate_mod_bytes(&bytes) {
            return -1;
        }
        let Some(mem) = (unsafe { mod_alloc_owned(&bytes) }) else {
            return -1;
        };
        let result = unsafe { mod_set_mod_ptr(mod_ as *mut Mod, mem) };
        if result < 0 {
            unsafe {
                mod_free_owned(mem);
                ptr::write_bytes(mod_ as *mut Mod, 0, 1);
            }
            return -1;
        }

        mod_.loaded = TRUE_BOOL;
        0
    }
}

pub fn mod_free_state(mod_: &mut Mod) {
    mod_.set = FALSE_BOOL;
    if mod_.loaded == 0 {
        return;
    }

    if !mod_.modraw.is_null() {
        unsafe {
            mod_free_owned(mod_.modraw);
        }
    }
    unsafe {
        ptr::write_bytes(mod_ as *mut Mod, 0, 1);
    }
}

pub fn mod_start_state(mod_: &mut Mod) {
    let mut i = MAX_VOICES;
    while i > 0 {
        let idx = i - 1;
        mod_.volume[idx] = 0;
        mod_.playpos[idx] = 0;
        mod_.chanfreq[idx] = 0;
        mod_.instnum[idx] = 0;
        mod_.channote[idx] = 0;
        mod_.sintabpos[idx] = 0;
        mod_.vib_freq[idx] = 0;
        mod_.vib_depth[idx] = 0;
        mod_.last_effect[idx] = 0;
        mod_.glissando[idx] = FALSE_BOOL;
        mod_.trem_wave[idx] = 0;
        mod_.vib_wave[idx] = 0;
        mod_.channel_active[idx] = FALSE_BOOL;
        i -= 1;
    }

    mod_.songpos = 0;
    mod_.patternline = 0;
    mod_.speed = 6;
    mod_.bpm = 125;
    mod_.speedcounter = 0;
    mod_.patterndelay = 0;
    mod_.patternline_jumpto = 0;
    mod_.patternline_jumpcount = 0;
    mod_.bpmtab = bpmtab_for_freq(mod_.freq);
    mod_.inctab = inctab_for_freq(mod_.freq);
    mod_.samplescounter = 0;
    unsafe {
        mod_.samplespertick = *mod_.bpmtab.add((125 - 32) as usize);
    }
}

pub fn trigger_note_on_voice_state(
    mod_: &mut Mod,
    idx: usize,
    instnum: u8,
    freq: u16,
    vol: u8,
) -> i32 {
    if idx >= MAX_VOICES {
        return -1;
    }
    if instnum != 0xff {
        let instrument = instnum as usize;
        if instrument >= mod_.instrument.len() || mod_.instrument[instrument].data.is_null() {
            return -1;
        }
    }

    if instnum != 0xff {
        mod_.channel_active[idx] = TRUE_BOOL;
        mod_.playpos[idx] = 0;
        mod_.instnum[idx] = instnum;
    }
    if freq != 0xffff {
        mod_.chanfreq[idx] = freq;
    }
    if vol <= 0x40 {
        mod_.volume[idx] = vol;
    }
    0
}

fn trigger_pattern_note_state(
    mod_: &mut Mod,
    idx: usize,
    instrument: u8,
    note: u16,
    effect: u8,
) -> bool {
    let instrument_count = (mod_.num_instr.max(0) as usize).min(mod_.instrument.len());
    if idx >= MAX_VOICES || instrument as usize > instrument_count {
        return false;
    }

    let mut triggered = false;
    if instrument != 0 && note != 0 && effect != 3 && effect != 5 {
        mod_.playpos[idx] = 0;
        mod_.instnum[idx] = instrument - 1;
        mod_.sintabpos[idx] = 0;
        triggered = true;
    }
    if note != 0 {
        triggered = true;
        mod_.channote[idx] = note;
        if effect == 3 || effect == 5 {
            mod_.portamento_to[idx] = note;
        } else {
            if effect == 4 || effect == 6 {
                mod_.vib_basefreq[idx] = note;
            }
            mod_.playpos[idx] = 0;
            mod_.chanfreq[idx] = note;
        }
    }
    if instrument != 0 {
        let current = mod_.instnum[idx] as usize;
        if current >= mod_.instrument.len() {
            return false;
        }
        triggered = true;
        mod_.volume[idx] = mod_.instrument[current].volume;
    }
    triggered
}

pub fn mod_trigger_note_state(
    mod_: &mut Mod,
    channel: i32,
    instnum: u8,
    freq: u16,
    vol: u8,
) -> i32 {
    if channel < 0 {
        return -1;
    }
    let Some(channel) = channel.checked_add(mod_.num_voices) else {
        return -1;
    };
    if channel < 0 || channel >= mod_.num_channels || channel >= MAX_VOICES as i32 {
        return -1;
    }
    trigger_note_on_voice_state(mod_, channel as usize, instnum, freq, vol)
}

pub fn mod_effect_handler_state(mod_: &mut Mod) -> u32 {
    let mut retval = 0;
    if mod_.speedcounter == 0 {
        mod_.arp_counter = 0;
    } else {
        mod_.arp_counter += 1;
        if mod_.arp_counter > 3 {
            mod_.arp_counter = 1;
        }
    }

    let mut i = 0usize;
    while i < mod_.num_voices as usize {
        if mod_.effect[i] < 0x10 {
            match mod_.effect[i] {
                0x00 => {
                    if mod_.speedcounter != 0 && mod_.effectop[i] != 0 {
                        let base = FREQTAB[mod_.channote[i] as usize];
                        mod_.chanfreq[i] = match mod_.arp_counter {
                            1 => base + (((mod_.effectop[i] >> 4) & 0x0f) as u16),
                            2 => base + ((mod_.effectop[i] & 0x0f) as u16),
                            3 => base,
                            _ => mod_.chanfreq[i],
                        };
                    }
                }
                0x01 => {
                    if mod_.speedcounter != 0 {
                        mod_.chanfreq[i] = mod_.chanfreq[i].saturating_sub(mod_.effectop[i] as u16);
                    }
                }
                0x02 => {
                    if mod_.speedcounter != 0 {
                        mod_.chanfreq[i] = mod_.chanfreq[i]
                            .saturating_add(mod_.effectop[i] as u16)
                            .min(4095);
                    }
                }
                0x03 => {
                    if mod_.speedcounter != 0 {
                        if mod_.chanfreq[i] < mod_.portamento_to[i] {
                            if (mod_.portamento_to[i] - mod_.chanfreq[i])
                                > mod_.porta_speed[i] as u16
                            {
                                mod_.chanfreq[i] += mod_.porta_speed[i] as u16;
                            } else {
                                mod_.chanfreq[i] = mod_.portamento_to[i];
                            }
                        } else if mod_.chanfreq[i] > mod_.portamento_to[i] {
                            if (mod_.chanfreq[i] - mod_.portamento_to[i])
                                > mod_.porta_speed[i] as u16
                            {
                                mod_.chanfreq[i] -= mod_.porta_speed[i] as u16;
                            } else {
                                mod_.chanfreq[i] = mod_.portamento_to[i];
                            }
                        } else {
                            mod_.channote[i] = mod_.chanfreq[i];
                        }
                    }
                }
                0x04 => {
                    if mod_.speedcounter != 0 {
                        let delta = ((wavetab::WAVETAB[(mod_.vib_wave[i] & 3) as usize]
                            [mod_.sintabpos[i] as usize]
                            as i32)
                            * mod_.vib_depth[i] as i32)
                            >> 7;
                        mod_.chanfreq[i] = (delta + mod_.vib_basefreq[i] as i32) as u16;
                        mod_.sintabpos[i] = mod_.sintabpos[i].wrapping_add(mod_.vib_freq[i]);
                        mod_.sintabpos[i] &= 63;
                    }
                }
                0x05 => {
                    if mod_.speedcounter != 0 {
                        if mod_.chanfreq[i] < mod_.portamento_to[i] {
                            if (mod_.portamento_to[i] - mod_.chanfreq[i])
                                > mod_.porta_speed[i] as u16
                            {
                                mod_.chanfreq[i] += mod_.porta_speed[i] as u16;
                            } else {
                                mod_.chanfreq[i] = mod_.portamento_to[i];
                            }
                        } else if mod_.chanfreq[i] > mod_.portamento_to[i] {
                            if (mod_.chanfreq[i] - mod_.portamento_to[i])
                                > mod_.porta_speed[i] as u16
                            {
                                mod_.chanfreq[i] -= mod_.porta_speed[i] as u16;
                            } else {
                                mod_.chanfreq[i] = mod_.portamento_to[i];
                            }
                        } else {
                            mod_.channote[i] = mod_.chanfreq[i];
                        }
                        if mod_.effectop[i] & 0xf0 != 0 {
                            mod_.volume[i] = mod_.volume[i]
                                .saturating_add((mod_.effectop[i] >> 4) & 0x0f)
                                .min(64);
                        } else if mod_.effectop[i] & 0x0f != 0 {
                            mod_.volume[i] = mod_.volume[i].saturating_sub(mod_.effectop[i] & 0x0f);
                        }
                    }
                }
                0x06 => {
                    if mod_.speedcounter != 0 {
                        let delta = ((wavetab::WAVETAB[(mod_.vib_wave[i] & 3) as usize]
                            [mod_.sintabpos[i] as usize]
                            as i32)
                            * mod_.vib_depth[i] as i32)
                            >> 7;
                        mod_.chanfreq[i] = (delta + mod_.vib_basefreq[i] as i32) as u16;
                        mod_.sintabpos[i] = mod_.sintabpos[i].wrapping_add(mod_.vib_freq[i]);
                        mod_.sintabpos[i] &= 63;
                        if mod_.effectop[i] & 0xf0 != 0 {
                            mod_.volume[i] = mod_.volume[i]
                                .saturating_add((mod_.effectop[i] >> 4) & 0x0f)
                                .min(64);
                        } else if mod_.effectop[i] & 0x0f != 0 {
                            mod_.volume[i] = mod_.volume[i].saturating_sub(mod_.effectop[i] & 0x0f);
                        }
                    }
                }
                0x07 => {
                    if mod_.speedcounter != 0 {
                        let mut v = mod_.trem_basevol[i] as i32;
                        v += ((wavetab::WAVETAB[(mod_.trem_wave[i] & 3) as usize]
                            [mod_.sintabpos[i] as usize] as i32)
                            * (mod_.trem_depth[i] as i32))
                            >> 6;
                        v = v.clamp(0, 64);
                        mod_.sintabpos[i] = mod_.sintabpos[i].wrapping_add(mod_.trem_freq[i]);
                        mod_.sintabpos[i] &= 63;
                        mod_.volume[i] = v as u8;
                    }
                }
                0x0a => {
                    if mod_.speedcounter != 0 {
                        if mod_.effectop[i] & 0xf0 != 0 {
                            mod_.volume[i] = mod_.volume[i]
                                .saturating_add((mod_.effectop[i] >> 4) & 0x0f)
                                .min(64);
                        } else if mod_.effectop[i] & 0x0f != 0 {
                            mod_.volume[i] = mod_.volume[i].saturating_sub(mod_.effectop[i] & 0x0f);
                        }
                    }
                }
                0x0e => match (mod_.effectop[i] >> 4) & 0x0f {
                    0x09 => {
                        if mod_.speedcounter != 0 {
                            mod_.retrigger_counter[i] = mod_.retrigger_counter[i].wrapping_add(1);
                            if mod_.retrigger_counter[i] >= (mod_.effectop[i] & 0x0f) {
                                mod_.retrigger_counter[i] = 0;
                                mod_.playpos[i] = 0;
                                mod_.channel_active[i] = TRUE_BOOL;
                                retval |= 1 << i;
                            }
                        }
                    }
                    0x0c => {
                        if mod_.speedcounter != 0
                            && (mod_.effectop[i] & 0x0f) == mod_.speedcounter as u8
                        {
                            mod_.volume[i] = 0;
                        }
                    }
                    0x0d if mod_.speedcounter != 0
                        && mod_.speedcounter as u8 == (mod_.effectop[i] & 0x0f)
                        && trigger_pattern_note_state(
                            mod_,
                            i,
                            mod_.nextinstr[i],
                            mod_.nextnote[i],
                            mod_.effect[i],
                        ) =>
                    {
                        mod_.channel_active[i] = TRUE_BOOL;
                        retval |= 1 << i;
                    }
                    _ => {}
                },
                _ => {}
            }
        }
        i += 1;
    }

    retval
}

pub fn mod_process_state(mod_: &mut Mod) -> u32 {
    let mut retval = 0;
    let mut do_pattern_break = FALSE_BOOL;
    let mut do_pattern_loop = FALSE_BOOL;
    let mut pattern_data = current_pattern_data(mod_, mod_.patternline, 0);
    let mut i = 0usize;

    while i < mod_.num_voices as usize {
        let note = unsafe {
            (((*pattern_data.add(0) as u16) << 8) & 0x0f00) | *pattern_data.add(1) as u16
        };
        let instrument =
            unsafe { (*pattern_data.add(0) & 0xf0) | ((*pattern_data.add(2) >> 4) & 0x0f) };
        let effect = unsafe { *pattern_data.add(2) & 0x0f };
        let effect_operand = unsafe { *pattern_data.add(3) };
        unsafe { pattern_data = pattern_data.add(4) };

        if ((mod_.last_effect[i] == 0x04) || (mod_.last_effect[i] == 0x06))
            && ((effect != 0x04) && (effect != 0x06))
        {
            mod_.chanfreq[i] = mod_.vib_basefreq[i];
        }
        if (mod_.last_effect[i] == 0x07) && (effect != 0x07) {
            mod_.volume[i] = mod_.trem_basevol[i];
        }

        if mod_.effect[i] == 0x00 && mod_.effectop[i] != 0 && (effect != 0 || effect_operand == 0) {
            mod_.chanfreq[i] = mod_.channote[i];
        }

        mod_.nextinstr[i] = instrument;
        mod_.nextnote[i] = note;

        if !((effect == 0x0e) && ((effect_operand & 0xf0) == 0xd0))
            && trigger_pattern_note_state(mod_, i, mod_.nextinstr[i], mod_.nextnote[i], effect)
        {
            retval |= 1 << i;
            mod_.channel_active[i] = TRUE_BOOL;
        }

        mod_.effect[i] = 0xff;

        match effect {
            0x03 => {
                mod_.effect[i] = effect;
                mod_.effectop[i] = effect_operand;
                if mod_.effectop[i] != 0 {
                    mod_.porta_speed[i] = mod_.effectop[i];
                }
            }
            0x04 => {
                if !((mod_.last_effect[i] == 0x04) || (mod_.last_effect[i] == 0x06)) {
                    mod_.vib_basefreq[i] = mod_.chanfreq[i];
                }
                if effect_operand & 0xf0 != 0 {
                    mod_.vib_freq[i] = (effect_operand >> 4) & 0x0f;
                }
                if effect_operand & 0x0f != 0 {
                    mod_.vib_depth[i] = effect_operand & 0x0f;
                }
                mod_.effect[i] = effect;
                mod_.effectop[i] = effect_operand;
            }
            0x05 => {
                mod_.effect[i] = effect;
                mod_.effectop[i] = effect_operand;
                if mod_.effectop[i] != 0 {
                    mod_.porta_speed[i] = mod_.effectop[i];
                }
            }
            0x06 => {
                if !((mod_.last_effect[i] == 0x04) || (mod_.last_effect[i] == 0x06)) {
                    mod_.vib_basefreq[i] = mod_.chanfreq[i];
                }
                mod_.effect[i] = effect;
                mod_.effectop[i] = effect_operand;
            }
            0x07 => {
                if mod_.last_effect[i] != 0x07 {
                    mod_.trem_basevol[i] = mod_.volume[i];
                }
                if effect_operand & 0xf0 != 0 {
                    mod_.trem_freq[i] = (effect_operand >> 4) & 0x0f;
                }
                if effect_operand & 0x0f != 0 {
                    mod_.trem_depth[i] = effect_operand & 0x0f;
                }
                mod_.effect[i] = effect;
                mod_.effectop[i] = effect_operand;
            }
            0x09 => {
                let ofs = (effect_operand as u32) << 8;
                if ofs > mod_.instrument[mod_.instnum[i] as usize].length {
                    mod_.playpos[i] = mod_.instrument[mod_.instnum[i] as usize].length << 16;
                } else {
                    mod_.playpos[i] = ofs << 16;
                }
            }
            0x0b => {
                if effect_operand < 128 {
                    if !mod_.notify.is_null() {
                        unsafe {
                            *mod_.notify = TRUE_BOOL;
                        }
                    }
                    mod_.songpos = effect_operand as i32;
                    mod_.patternline = 0;
                    return retval;
                }
            }
            0x0c => {
                let mut vol = effect_operand;
                if vol > 64 {
                    vol = 64;
                }
                mod_.volume[i] = vol;
            }
            0x0d => {
                do_pattern_break = TRUE_BOOL;
            }
            0x0e => match (effect_operand >> 4) & 0x0f {
                0x01 => {
                    mod_.chanfreq[i] =
                        mod_.chanfreq[i].saturating_sub((effect_operand & 0x0f) as u16);
                }
                0x02 => {
                    mod_.chanfreq[i] = mod_.chanfreq[i]
                        .saturating_add((effect_operand & 0x0f) as u16)
                        .min(4095);
                }
                0x03 => {
                    if (effect_operand & 0x0f) == 0x00 {
                        mod_.glissando[i] = FALSE_BOOL;
                    } else if (effect_operand & 0x0f) == 0x01 {
                        mod_.glissando[i] = TRUE_BOOL;
                    }
                }
                0x04 => {
                    if (effect_operand & 0x0f) < 8 {
                        mod_.vib_wave[i] = effect_operand & 0x07;
                    }
                }
                0x05 => {
                    mod_.instrument[mod_.instnum[i] as usize].finetune = (effect_operand + 8) & 15;
                }
                0x06 => {
                    if (effect_operand & 0x0f) == 0 {
                        mod_.patternline_jumpto = mod_.patternline;
                    } else {
                        do_pattern_loop = TRUE_BOOL;
                        if mod_.patternline_jumpcount == 0 {
                            mod_.patternline_jumpcount = (effect_operand & 0x0f) as i32;
                        } else if {
                            mod_.patternline_jumpcount -= 1;
                            mod_.patternline_jumpcount
                        } == 0
                        {
                            do_pattern_loop = FALSE_BOOL;
                        }
                    }
                }
                0x07 => {
                    if (effect_operand & 0x0f) < 8 {
                        mod_.trem_wave[i] = effect_operand & 0x07;
                    }
                }
                0x09 => {
                    mod_.retrigger_counter[i] = 0;
                    mod_.effect[i] = effect;
                    mod_.effectop[i] = effect_operand;
                }
                0x0a => {
                    mod_.volume[i] = mod_.volume[i].saturating_add(effect_operand & 0x0f).min(64);
                }
                0x0b => {
                    mod_.volume[i] = mod_.volume[i].saturating_sub(effect_operand & 0x0f);
                    mod_.effect[i] = effect;
                    mod_.effectop[i] = effect_operand;
                }
                0x0c | 0x0d => {
                    mod_.effect[i] = effect;
                    mod_.effectop[i] = effect_operand;
                }
                0x0e => {
                    mod_.patterndelay = mod_.speed * (effect_operand & 0x0f) as i32;
                }
                _ => {}
            },
            0x0f => {
                if effect_operand < 32 {
                    mod_.speed = effect_operand as i32;
                } else {
                    mod_.bpm = effect_operand as i32;
                    mod_.samplespertick =
                        unsafe { *mod_.bpmtab.add((effect_operand - 32) as usize) };
                }
            }
            _ => {
                mod_.effect[i] = effect;
                mod_.effectop[i] = effect_operand;
            }
        }
        mod_.last_effect[i] = effect;
        i += 1;
    }

    if do_pattern_break != FALSE_BOOL {
        mod_.songpos += 1;
        mod_.patternline = 0;
        if mod_.songpos >= mod_.song_length as i32 {
            if !mod_.notify.is_null() {
                unsafe {
                    *mod_.notify = TRUE_BOOL;
                }
            }
            mod_.songpos = 0;
            mod_.patternline = 0;
        }
        mod_.patternline_jumpto = 0;
        mod_.patternline_jumpcount = 0;
    } else if do_pattern_loop != FALSE_BOOL {
        mod_.patternline = mod_.patternline_jumpto;
    } else {
        mod_.patternline += 1;
        if mod_.patternline > 63 {
            mod_.patternline = 0;
            mod_.songpos += 1;
            mod_.patternline_jumpto = 0;
            mod_.patternline_jumpcount = 0;
        }
        if mod_.songpos >= mod_.song_length as i32 {
            if !mod_.notify.is_null() {
                unsafe {
                    *mod_.notify = TRUE_BOOL;
                }
            }
            mod_.songpos = 0;
            mod_.patternline = 0;
        }
    }

    retval
}

const PREFILL_WORD: i16 = 0;

fn mix_mono_16bit_state(mod_: &mut Mod, buf: &mut [i16], num_samples: usize) -> usize {
    let mut idx = 0usize;
    while idx < num_samples {
        buf[idx] = PREFILL_WORD;
        idx += 1;
    }

    let mut voice = 0usize;
    while voice < mod_.num_channels as usize {
        let instnum = mod_.instnum[voice] as usize;
        if mod_.instrument[instnum].data.is_null() || mod_.channel_active[voice] == FALSE_BOOL {
            voice += 1;
            continue;
        }

        let mut noteidx = mod_.chanfreq[voice] as i32;
        noteidx -=
            (mod_.chanfreq[voice] as i32 * 2 * (mod_.instrument[instnum].finetune as i32 - 8))
                / 256;
        let mut incval = unsafe { *mod_.inctab.add(noteidx as usize) };
        if mod_.freq == 32_000 || mod_.freq == 48_000 {
            incval >>= 2;
        }

        let data = mod_.instrument[instnum].data;
        let mut playpos = mod_.playpos[voice];
        let loop_end = mod_.instrument[instnum].loop_end << 16;
        let mut volume = mod_.volume[voice];
        if voice < mod_.num_voices as usize {
            volume = ((volume as i32 * mod_.musicvolume as i32) >> 6) as u8;
        } else {
            volume = ((volume as i32 * mod_.sfxvolume as i32) >> 6) as u8;
        }

        let mut sample_idx = 0usize;
        while sample_idx < num_samples {
            let current = unsafe { *data.add((playpos >> 16) as usize) as i32 };
            let accum =
                buf[sample_idx] as i32 + (((current * volume as i32) >> 6) << mod_.shiftval);
            buf[sample_idx] = accum.clamp(-32768, 32767) as i16;
            playpos = playpos.wrapping_add(incval);
            if playpos >= loop_end {
                if mod_.instrument[instnum].looped != FALSE_BOOL {
                    playpos = playpos.wrapping_sub(mod_.instrument[instnum].loop_length << 16);
                } else {
                    playpos = (mod_.instrument[instnum].loop_end.saturating_sub(1)) << 16;
                    mod_.channel_active[voice] = FALSE_BOOL;
                    break;
                }
            }
            sample_idx += 1;
        }

        mod_.playpos[voice] = playpos;
        voice += 1;
    }

    num_samples
}

fn mix_stereo_16bit_state(mod_: &mut Mod, buf: &mut [i16], num_samples: usize) -> usize {
    let mut idx = 0usize;
    while idx < num_samples * 2 {
        buf[idx] = PREFILL_WORD;
        buf[idx + 1] = PREFILL_WORD;
        idx += 2;
    }

    let mut voice = 0usize;
    while voice < mod_.num_channels as usize {
        let instnum = mod_.instnum[voice] as usize;
        if mod_.instrument[instnum].data.is_null() || mod_.channel_active[voice] == FALSE_BOOL {
            voice += 1;
            continue;
        }

        let lrofs = (((voice as i32 - 1) >> 1) & 1) ^ 1;
        let mut noteidx = mod_.chanfreq[voice] as i32;
        noteidx -=
            (mod_.chanfreq[voice] as i32 * 2 * (mod_.instrument[instnum].finetune as i32 - 8))
                / 256;
        let mut incval = unsafe { *mod_.inctab.add(noteidx as usize) };
        if mod_.freq == 32_000 || mod_.freq == 48_000 {
            incval >>= 2;
        }

        let data = mod_.instrument[instnum].data;
        let mut playpos = mod_.playpos[voice];
        let loop_end = mod_.instrument[instnum].loop_end << 16;
        let mut volume = mod_.volume[voice];
        if voice < mod_.num_voices as usize {
            volume = ((volume as i32 * mod_.musicvolume as i32) >> 6) as u8;
        } else {
            volume = ((volume as i32 * mod_.sfxvolume as i32) >> 6) as u8;
        }

        let mut sample_idx = lrofs as usize;
        while sample_idx < num_samples * 2 {
            let current = unsafe { *data.add((playpos >> 16) as usize) as i32 };
            let accum =
                buf[sample_idx] as i32 + (((current * volume as i32) >> 6) << (mod_.shiftval + 1));
            buf[sample_idx] = accum.clamp(-32768, 32767) as i16;
            playpos = playpos.wrapping_add(incval);
            if playpos >= loop_end {
                if mod_.instrument[instnum].looped != FALSE_BOOL {
                    playpos = playpos.wrapping_sub(mod_.instrument[instnum].loop_length << 16);
                } else {
                    playpos = (mod_.instrument[instnum].loop_end.saturating_sub(1)) << 16;
                    mod_.channel_active[voice] = FALSE_BOOL;
                    break;
                }
            }
            sample_idx += 2;
        }

        mod_.playpos[voice] = playpos;
        voice += 1;
    }

    num_samples
}

pub fn mod_player_state(mod_: &mut Mod) -> u32 {
    let mut retval = 0;
    if mod_.musicvolume > 0x40 {
        mod_.musicvolume = 0x40;
    }
    if mod_.sfxvolume > 0x40 {
        mod_.sfxvolume = 0x40;
    }

    let mixingbuflen = if mod_.mixingbuflen <= 0 {
        0
    } else {
        mod_.mixingbuflen as usize
    };
    if mixingbuflen != 0 && !mod_.mixingbuf.is_null() && mod_.bits == 16 {
        let buf = unsafe {
            core::slice::from_raw_parts_mut(mod_.mixingbuf as *mut i16, mixingbuflen / 2)
        };
        if mod_.channels == 1 {
            let mut remain = buf.len();
            let mut offset = 0usize;
            while remain > 0 {
                let tick_remain = (mod_.samplespertick - mod_.samplescounter) as usize;
                let res = mix_mono_16bit_state(
                    mod_,
                    &mut buf[offset..],
                    core::cmp::min(tick_remain, remain),
                );
                offset += res;
                remain -= res;

                mod_.samplescounter += res as u32;
                if mod_.samplescounter >= mod_.samplespertick {
                    mod_.samplescounter -= mod_.samplespertick;
                    mod_.speedcounter += 1;
                    if mod_.speedcounter >= (mod_.speed + mod_.patterndelay) {
                        mod_.patterndelay = 0;
                        retval |= mod_process_state(mod_);
                        mod_.speedcounter = 0;
                    }
                    retval |= mod_effect_handler_state(mod_);
                }
            }
        } else if mod_.channels == 2 {
            let stereo_samples = buf.len() / 2;
            let mut remain = stereo_samples;
            let mut offset = 0usize;
            while remain > 0 {
                let tick_remain = (mod_.samplespertick - mod_.samplescounter) as usize;
                let res = mix_stereo_16bit_state(
                    mod_,
                    &mut buf[offset..],
                    core::cmp::min(tick_remain, remain),
                );
                offset += res * 2;
                remain -= res;

                mod_.samplescounter += res as u32;
                if mod_.samplescounter >= mod_.samplespertick {
                    mod_.samplescounter -= mod_.samplespertick;
                    mod_.speedcounter += 1;
                    if mod_.speedcounter >= (mod_.speed + mod_.patterndelay) {
                        mod_.patterndelay = 0;
                        retval |= mod_process_state(mod_);
                        mod_.speedcounter = 0;
                    }
                    retval |= mod_effect_handler_state(mod_);
                }
            }
        }
    }

    mod_.notebeats = retval;
    if let Some(callback) = mod_.callback {
        unsafe {
            callback(mod_ as *mut Mod as *mut c_void);
        }
    }

    retval
}

fn update_wave_format_state(modplay: &mut ModPlay) -> bool {
    let was_playing = modplay.playing != 0;
    if was_playing && backend_stop_state() < 0 {
        return false;
    }

    if modplay.stereo != 0 {
        modplay.sound_buf.chans = 2;
        modplay.mod_.channels = 2;
    } else {
        modplay.sound_buf.chans = 1;
        modplay.mod_.channels = 1;
    }

    modplay.sound_buf.freq = modplay.playfreq;
    modplay.mod_.freq = modplay.playfreq as i32;
    modplay.mod_.bits = 16;
    modplay.bits = TRUE_BOOL;
    modplay.sound_buf.fmt = 16;
    modplay.sound_buf.usr_data = modplay as *mut ModPlay as *mut c_void;
    modplay.sound_buf.samples = (modplay.playfreq as f32) / 50.0;
    modplay.sound_buf.callback = Some(mix_callback_export);
    SHIFT_VAL.store(
        u32::from(modplay.sound_buf.chans == 2) + u32::from(modplay.sound_buf.fmt == 16),
        Ordering::Release,
    );

    if was_playing && backend_start_state(&modplay.sound_buf) < 0 {
        modplay.playing = FALSE_BOOL;
        return false;
    }
    true
}

unsafe extern "C" fn mix_callback_export(usrdata: *mut c_void, stream: *mut u8, len: u32) {
    if usrdata.is_null() || stream.is_null() {
        return;
    }
    // SAFETY: the audio backend supplies a valid `ModPlay` user pointer and a
    // writable output buffer of at least `len` bytes for this callback.
    unsafe {
        let modplay = &mut *(usrdata as *mut ModPlay);
        let slice = core::slice::from_raw_parts_mut(stream, len as usize);
        // The callback ABI has no return value. The mask reports internal note
        // triggers, not an error, and `mix_callback_state` applies them directly.
        mix_callback_state(modplay, slice);
    }
}

fn refresh_playback_phase(modplay: &mut ModPlay) {
    modplay.mod_.samplescounter = 0;
    if modplay.mod_.bpmtab.is_null() {
        return;
    }
    let bpm_index = (modplay.mod_.bpm - 32) as usize;
    unsafe {
        modplay.mod_.samplespertick = *modplay.mod_.bpmtab.add(bpm_index);
    }
}

pub fn init_state(modplay: &mut ModPlay) {
    if SND_PLAYING.load(Ordering::Acquire) {
        let rc = backend_stop_state();
        if rc < 0 {
            if rc == -2 {
                modplay.playing = FALSE_BOOL;
            }
            return;
        }
    }
    #[cfg(target_arch = "powerpc")]
    if PLAYER_THREAD.load(Ordering::Acquire) != lwp::LWP_THREAD_NULL {
        let rc = backend_stop_state();
        if rc < 0 {
            if rc == -2 {
                modplay.playing = FALSE_BOOL;
            }
            return;
        }
    }
    unsafe {
        ptr::write_bytes(modplay as *mut ModPlay, 0, 1);
    }
    modplay.playing = FALSE_BOOL;
    modplay.paused = FALSE_BOOL;
    modplay.bits = TRUE_BOOL;
    modplay.stereo = TRUE_BOOL;
    modplay.manual_polling = FALSE_BOOL;
    modplay.playfreq = 48_000;
    modplay.num_sfx_chans = 0;
    if !update_wave_format_state(modplay) {
        modplay.playing = FALSE_BOOL;
    }
    backend_reset_state();
}

pub fn set_frequency_state(modplay: &mut ModPlay, freq: u32) -> i32 {
    if freq == modplay.playfreq {
        return 0;
    }
    if freq == 32_000 || freq == 48_000 {
        let previous = modplay.playfreq;
        modplay.playfreq = freq;
        if !update_wave_format_state(modplay) {
            modplay.playfreq = previous;
            if modplay.playing == 0 && !update_wave_format_state(modplay) {
                return -1;
            }
            return -1;
        }
        if modplay.playing != 0 {
            refresh_playback_phase(modplay);
        }
        return 0;
    }
    -1
}

pub fn set_stereo_state(modplay: &mut ModPlay, stereo: Bool) {
    if stereo == modplay.stereo {
        return;
    }
    let old_stereo = modplay.stereo;
    modplay.stereo = stereo;
    if !update_wave_format_state(modplay) {
        modplay.stereo = old_stereo;
        if modplay.playing == 0 && !update_wave_format_state(modplay) {
            return;
        }
        return;
    }
    if modplay.playing != 0 {
        refresh_playback_phase(modplay);
    }
}

pub fn try_unload_state(modplay: &mut ModPlay) -> i32 {
    if modplay.playing != 0 && stop_state(modplay) < 0 {
        return -1;
    }
    modplay.paused = FALSE_BOOL;
    modplay.playing = FALSE_BOOL;
    mod_free_state(&mut modplay.mod_);
    0
}

pub fn unload_state(modplay: &mut ModPlay) {
    // The historical ABI is void. Callers that need teardown status use
    // `try_unload_state`.
    try_unload_state(modplay);
}

/// # Safety
///
/// `mem` must point to a complete caller-owned MOD image whose backing bytes
/// remain valid until `mod_play_unload` or another module is installed.
pub unsafe fn set_mod_state(modplay: &mut ModPlay, mem: *mut u8) -> i32 {
    if mem.is_null() {
        return -1;
    }
    unsafe {
        if try_unload_state(modplay) < 0 {
            return -1;
        }
        if mod_set_mod_ptr(&mut modplay.mod_ as *mut Mod, mem) == 0 {
            if alloc_sfx_channels_state(modplay, modplay.num_sfx_chans) == 0 {
                0
            } else {
                mod_free_state(&mut modplay.mod_);
                -1
            }
        } else {
            -1
        }
    }
}

pub fn alloc_sfx_channels_state(modplay: &mut ModPlay, sfxchans: u32) -> i32 {
    if modplay.mod_.modraw.is_null() {
        return -1;
    }
    let Ok(sfxchans_i32) = i32::try_from(sfxchans) else {
        return -1;
    };

    if mod_alloc_sfx_channels_inner(&mut modplay.mod_, sfxchans_i32) == 0 {
        modplay.num_sfx_chans = sfxchans;
        0
    } else {
        -1
    }
}

pub fn pause_state(modplay: &mut ModPlay, pause: Bool) -> i32 {
    #[cfg(target_arch = "powerpc")]
    if PLAYER_WORKER_FAILED.load(Ordering::Acquire) {
        let stop_result = stop_state(modplay);
        return if stop_result < 0 { stop_result } else { -1 };
    }
    if modplay.playing == 0 {
        return -1;
    }
    modplay.paused = pause;
    0
}

pub fn trigger_note_state(modplay: &mut ModPlay, chan: u32, inst: u8, freq: u16, vol: u8) -> i32 {
    if modplay.mod_.modraw.is_null() {
        return -1;
    }
    mod_trigger_note_state(&mut modplay.mod_, chan as i32, inst, freq, vol)
}

pub fn set_volume_state(modplay: &mut ModPlay, musicvolume: i32, sfxvolume: i32) {
    modplay.mod_.musicvolume = musicvolume.clamp(0, 64) as u8;
    modplay.mod_.sfxvolume = sfxvolume.clamp(0, 64) as u8;
}

pub fn mix_callback_state(modplay: &mut ModPlay, stream: &mut [u8]) -> u32 {
    if modplay.manual_polling != 0 {
        modplay.mod_.notify = &mut modplay.paused as *mut Bool;
    } else {
        modplay.mod_.notify = ptr::null_mut();
    }

    modplay.mod_.mixingbuf = stream.as_mut_ptr();
    modplay.mod_.mixingbuflen = stream.len() as i32;

    if modplay.paused != 0 {
        let mut i = 0usize;
        while i < stream.len() {
            stream[i] = 0;
            i += 1;
        }
        MIXING_TIME_US.store(0, Ordering::Release);
        0
    } else {
        let retval = mod_player_state(&mut modplay.mod_);
        MIXING_TIME_US.store(stream.len() as u32, Ordering::Release);
        retval
    }
}

pub fn start_state(modplay: &mut ModPlay) -> i32 {
    if modplay.playing != 0 || modplay.mod_.modraw.is_null() {
        return -1;
    }
    mod_start_state(&mut modplay.mod_);
    if backend_start_state(&modplay.sound_buf) < 0 {
        return -1;
    }
    modplay.playing = TRUE_BOOL;
    0
}

pub fn stop_state(modplay: &mut ModPlay) -> i32 {
    if modplay.playing == 0 {
        return -1;
    }
    complete_stop_state(modplay, backend_stop_state())
}

fn complete_stop_state(modplay: &mut ModPlay, backend_result: i32) -> i32 {
    if backend_result == -2 {
        modplay.playing = FALSE_BOOL;
        return -1;
    }
    if backend_result < 0 {
        return -1;
    }
    modplay.playing = FALSE_BOOL;
    0
}

pub fn mixing_time_state() -> u32 {
    if SND_PLAYING.load(Ordering::Acquire) {
        MIXING_TIME_US.load(Ordering::Acquire)
    } else {
        0
    }
}

pub fn request_time_state() -> u32 {
    if THR_RUNNING.load(Ordering::Acquire) {
        REQUEST_TIME_US.load(Ordering::Acquire)
    } else {
        0
    }
}

// Compatibility aliases for the public tests. The implementation uses shorter
// internal state-function names, but the tests and C API compatibility layer
// expect the historical `modplay_*_state` spellings.
pub fn modplay_init_state(modplay: &mut ModPlay) {
    init_state(modplay);
}

pub fn modplay_set_frequency_state(modplay: &mut ModPlay, freq: u32) -> i32 {
    set_frequency_state(modplay, freq)
}

pub fn modplay_set_stereo_state(modplay: &mut ModPlay, stereo: Bool) {
    set_stereo_state(modplay, stereo);
}

pub fn modplay_unload_state(modplay: &mut ModPlay) {
    unload_state(modplay);
}

/// # Safety
///
/// `mem` must satisfy the same lifetime and validity requirements as
/// [`set_mod_state`].
pub unsafe fn modplay_set_mod_state(modplay: &mut ModPlay, mem: *mut u8) -> i32 {
    unsafe { set_mod_state(modplay, mem) }
}

pub fn modplay_alloc_sfx_channels_state(modplay: &mut ModPlay, sfxchans: u32) -> i32 {
    alloc_sfx_channels_state(modplay, sfxchans)
}

pub fn modplay_trigger_note_state(
    modplay: &mut ModPlay,
    chan: u32,
    inst: u8,
    freq: u16,
    vol: u8,
) -> i32 {
    trigger_note_state(modplay, chan, inst, freq, vol)
}

pub fn modplay_set_volume_state(modplay: &mut ModPlay, musicvolume: i32, sfxvolume: i32) {
    set_volume_state(modplay, musicvolume, sfxvolume);
}

pub fn modplay_mix_callback_state(modplay: &mut ModPlay, stream: &mut [u8]) -> u32 {
    mix_callback_state(modplay, stream)
}

fn backend_reset_state() {
    CURRENT_AUDIO.store(0, Ordering::Release);
    SHIFT_VAL.store(0, Ordering::Release);
    MIXING_TIME_US.store(0, Ordering::Release);
    REQUEST_TIME_US.store(0, Ordering::Release);
    THR_RUNNING.store(false, Ordering::Release);
    SND_PLAYING.store(false, Ordering::Release);
    #[cfg(target_arch = "powerpc")]
    PLAYER_WORKER_FAILED.store(false, Ordering::Release);
    unsafe {
        #[cfg(target_arch = "powerpc")]
        ptr::write_bytes(
            core::ptr::addr_of_mut!(AUDIO_BUFFER) as *mut u8,
            0,
            core::mem::size_of::<[[u8; 5760]; 2]>(),
        );
        ptr::write_bytes(
            core::ptr::addr_of_mut!(SND_BUFFER) as *mut u8,
            0,
            core::mem::size_of::<ModSndBuf>(),
        );
    }
}

fn backend_start_state(sndbuf: &ModSndBuf) -> i32 {
    if SND_PLAYING.swap(true, Ordering::AcqRel) {
        return -1;
    }
    CURRENT_AUDIO.store(0, Ordering::Release);
    SHIFT_VAL.store(
        u32::from(sndbuf.chans == 2) + u32::from(sndbuf.fmt == 16),
        Ordering::Release,
    );
    unsafe {
        SND_BUFFER = *sndbuf;
    }
    #[cfg(target_arch = "powerpc")]
    if !unsafe { start_audio_dma(sndbuf) } {
        SND_PLAYING.store(false, Ordering::Release);
        THR_RUNNING.store(false, Ordering::Release);
        return -1;
    }
    1
}

fn backend_stop_state() -> i32 {
    let was_playing = SND_PLAYING.swap(false, Ordering::AcqRel);
    #[cfg(target_arch = "powerpc")]
    let thread = PLAYER_THREAD.load(Ordering::Acquire);
    #[cfg(target_arch = "powerpc")]
    let queue = PLAYER_QUEUE.load(Ordering::Acquire);
    #[cfg(not(target_arch = "powerpc"))]
    if !was_playing {
        return 0;
    }
    CURRENT_AUDIO.store(0, Ordering::Release);
    REQUEST_TIME_US.store(0, Ordering::Release);
    #[cfg(target_arch = "powerpc")]
    {
        let shutdown_result = shutdown_player_worker(
            PlayerWorkerState {
                was_playing,
                has_queue: queue != lwp::LWP_TQUEUE_NULL,
                has_thread: thread != lwp::LWP_THREAD_NULL,
            },
            audio_backend::stop_dma,
            || {
                audio_backend::register_dma_callback(None);
            },
            || {
                if queue != lwp::LWP_TQUEUE_NULL {
                    lwp::thread_signal(queue);
                }
            },
            || unsafe { lwp::join_thread(thread, ptr::null_mut()) == lwp::LWP_SUCCESSFUL },
            || {
                if queue != lwp::LWP_TQUEUE_NULL {
                    lwp::close_queue(queue);
                }
            },
            || PLAYER_WORKER_FAILED.load(Ordering::Acquire),
        );
        let Some(worker_failed) = shutdown_result else {
            // Keep the thread and queue handles live so a later stop call can
            // signal and retry the join without reusing the thread stack.
            return -1;
        };
        if thread != lwp::LWP_THREAD_NULL {
            PLAYER_THREAD.store(lwp::LWP_THREAD_NULL, Ordering::Release);
        }
        if queue != lwp::LWP_TQUEUE_NULL {
            PLAYER_QUEUE.store(lwp::LWP_TQUEUE_NULL, Ordering::Release);
        }
        THR_RUNNING.store(false, Ordering::Release);
        PLAYER_WORKER_FAILED.store(false, Ordering::Release);
        worker_stop_result(true, worker_failed)
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        0
    }
}

#[cfg(any(target_arch = "powerpc", test))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum PlayerWorkerExit {
    Stopped,
    WaitFailed,
}

#[cfg(any(target_arch = "powerpc", test))]
fn player_worker_loop(
    mut is_playing: impl FnMut() -> bool,
    mut wait_for_signal: impl FnMut() -> bool,
    mut mix_next_buffer: impl FnMut(),
) -> PlayerWorkerExit {
    while is_playing() {
        if !wait_for_signal() {
            return PlayerWorkerExit::WaitFailed;
        }
        if is_playing() {
            mix_next_buffer();
        }
    }
    PlayerWorkerExit::Stopped
}

#[cfg(any(target_arch = "powerpc", test))]
fn clear_audio_buffers(buffers: &mut [[u8; SOUND_BUFFER_SIZE as usize]; 2]) {
    buffers[0].fill(0);
    buffers[1].fill(0);
}

#[cfg(any(target_arch = "powerpc", test))]
fn record_worker_wait_failure(playing: &AtomicBool, failed: &AtomicBool) {
    failed.store(true, Ordering::Release);
    playing.store(false, Ordering::Release);
}

#[cfg(any(target_arch = "powerpc", test))]
fn worker_stop_result(cleanup_complete: bool, worker_failed: bool) -> i32 {
    if !cleanup_complete {
        -1
    } else if worker_failed {
        -2
    } else {
        0
    }
}

#[cfg(any(target_arch = "powerpc", test))]
struct PlayerWorkerState {
    was_playing: bool,
    has_queue: bool,
    has_thread: bool,
}

#[cfg(any(target_arch = "powerpc", test))]
fn shutdown_player_worker(
    state: PlayerWorkerState,
    stop_dma: impl FnOnce(),
    remove_dma_callback: impl FnOnce(),
    signal_worker: impl FnOnce(),
    join_worker: impl FnOnce() -> bool,
    close_queue: impl FnOnce(),
    worker_failed_after_join: impl FnOnce() -> bool,
) -> Option<bool> {
    let PlayerWorkerState {
        was_playing,
        has_queue,
        has_thread,
    } = state;
    if was_playing || has_queue || has_thread {
        stop_dma();
        remove_dma_callback();
    }
    if has_queue {
        signal_worker();
    }
    if has_thread && !join_worker() {
        return None;
    }
    if has_queue {
        close_queue();
    }
    Some(worker_failed_after_join())
}

#[cfg(any(target_arch = "powerpc", test))]
fn next_audio_buffer(current: u32) -> u32 {
    (current + 1) & 1
}

#[cfg(target_arch = "powerpc")]
unsafe fn start_audio_dma(sndbuf: &ModSndBuf) -> bool {
    if sndbuf.callback.is_none()
        || PLAYER_THREAD.load(Ordering::Acquire) != lwp::LWP_THREAD_NULL
        || PLAYER_QUEUE.load(Ordering::Acquire) != lwp::LWP_TQUEUE_NULL
    {
        return false;
    }

    lwp::initialize();
    let mut queue = lwp::LWP_TQUEUE_NULL;
    if unsafe { lwp::init_queue(&mut queue) } != lwp::LWP_SUCCESSFUL {
        return false;
    }
    PLAYER_QUEUE.store(queue, Ordering::Release);
    PLAYER_WORKER_FAILED.store(false, Ordering::Release);
    THR_RUNNING.store(true, Ordering::Release);

    audio_backend::init(None);
    let dsp_rate = if sndbuf.freq == 32_000 {
        audio_backend::AI_SAMPLERATE_32KHZ
    } else {
        audio_backend::AI_SAMPLERATE_48KHZ
    };
    audio_backend::set_dsp_sample_rate(dsp_rate as u8);
    unsafe {
        let buffers =
            core::ptr::addr_of_mut!(AUDIO_BUFFER.0) as *mut [u8; SOUND_BUFFER_SIZE as usize];
        clear_audio_buffers(&mut *core::ptr::addr_of_mut!(AUDIO_BUFFER.0));
        dc_flush_range(buffers.cast(), SOUND_BUFFER_SIZE * 2);
        CURRENT_AUDIO.store(0, Ordering::Release);

        let mut thread = lwp::LWP_THREAD_NULL;
        let stack = core::ptr::addr_of_mut!(PLAYER_STACK.0).cast::<u8>().cast();
        let rc = lwp::create_thread(
            &mut thread,
            Some(modplay_player_thread),
            ptr::null_mut(),
            stack,
            PLAYER_STACK_SIZE as u32,
            PLAYER_PRIORITY,
        );
        if rc != lwp::LWP_SUCCESSFUL {
            lwp::close_queue(queue);
            PLAYER_QUEUE.store(lwp::LWP_TQUEUE_NULL, Ordering::Release);
            THR_RUNNING.store(false, Ordering::Release);
            return false;
        }
        PLAYER_THREAD.store(thread, Ordering::Release);

        audio_backend::register_dma_callback(Some(modplay_dma_callback));
        audio_backend::init_dma(mem_virtual_to_physical(buffers as usize), SOUND_BUFFER_SIZE);
        audio_backend::start_dma();
    }
    true
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn modplay_player_thread(_arg: *mut c_void) -> *mut c_void {
    THR_RUNNING.store(true, Ordering::Release);
    let queue = PLAYER_QUEUE.load(Ordering::Acquire);
    let wait_failed = if queue != lwp::LWP_TQUEUE_NULL {
        player_worker_loop(
            || SND_PLAYING.load(Ordering::Acquire),
            || lwp::thread_sleep(queue) == lwp::LWP_SUCCESSFUL,
            || unsafe { mix_next_audio_buffer() },
        ) == PlayerWorkerExit::WaitFailed
    } else {
        true
    };
    if wait_failed {
        record_worker_wait_failure(&SND_PLAYING, &PLAYER_WORKER_FAILED);
    }
    THR_RUNNING.store(false, Ordering::Release);
    ptr::null_mut()
}

#[cfg(target_arch = "powerpc")]
unsafe fn mix_next_audio_buffer() {
    if PLAYER_WORKER_FAILED.load(Ordering::Acquire) {
        return;
    }
    let sndbuf = unsafe { ptr::read_volatile(core::ptr::addr_of!(SND_BUFFER)) };
    let Some(callback) = sndbuf.callback else {
        record_worker_wait_failure(&SND_PLAYING, &PLAYER_WORKER_FAILED);
        return;
    };
    let current = CURRENT_AUDIO.load(Ordering::Acquire) as usize & 1;
    unsafe {
        let buffers =
            core::ptr::addr_of_mut!(AUDIO_BUFFER.0) as *mut [u8; SOUND_BUFFER_SIZE as usize];
        let buffer = &mut *buffers.add(current);
        callback(sndbuf.usr_data, buffer.as_mut_ptr(), SOUND_BUFFER_SIZE);
        dc_flush_range(buffer.as_ptr().cast(), SOUND_BUFFER_SIZE);
    }
}

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn modplay_dma_callback() {
    if !SND_PLAYING.load(Ordering::Acquire) || PLAYER_WORKER_FAILED.load(Ordering::Acquire) {
        return;
    }
    let next = next_audio_buffer(CURRENT_AUDIO.load(Ordering::Acquire)) as usize;
    CURRENT_AUDIO.store(next as u32, Ordering::Release);
    let queue = PLAYER_QUEUE.load(Ordering::Acquire);
    if queue == lwp::LWP_TQUEUE_NULL {
        record_worker_wait_failure(&SND_PLAYING, &PLAYER_WORKER_FAILED);
        return;
    }

    unsafe {
        let buffers =
            core::ptr::addr_of_mut!(AUDIO_BUFFER.0) as *mut [u8; SOUND_BUFFER_SIZE as usize];
        let buffer = &mut *buffers.add(next);
        audio_backend::init_dma(
            mem_virtual_to_physical(buffer.as_ptr() as usize),
            SOUND_BUFFER_SIZE,
        );
        audio_backend::start_dma();
    }
    lwp::thread_signal(queue);
}

pub fn get_note_value(mod_: &Mod, patternline: i32, channel: i32) -> u16 {
    let data = current_pattern_data(mod_, patternline, channel);
    unsafe { (((*data & 0x0f) as u16) << 8) | (*data.add(1) as u16) }
}

pub fn get_instr_value(mod_: &Mod, patternline: i32, channel: i32) -> u8 {
    let data = current_pattern_data(mod_, patternline, channel);
    unsafe { (*data & 0xf0) | ((*data.add(2) >> 4) & 0x0f) }
}

pub fn get_effect_value(mod_: &Mod, patternline: i32, channel: i32) -> u8 {
    let data = current_pattern_data(mod_, patternline, channel);
    unsafe { *data.add(2) & 0x0f }
}

pub fn get_effect_op_value(mod_: &Mod, patternline: i32, channel: i32) -> u8 {
    let data = current_pattern_data(mod_, patternline, channel);
    unsafe { *data.add(3) }
}

/// # Safety
///
/// `mod_` must point to writable storage for a parsed `Mod`. This frees
/// `modraw` only when the module was loaded through `mod_load`; caller-owned
/// `mod_set_mod` buffers are left untouched.
pub unsafe fn mod_free(mod_: *mut Mod) {
    unsafe {
        if mod_.is_null() {
            return;
        }
        mod_free_state(&mut *mod_);
    }
}

/// # Safety
///
/// `mod_` must point to writable storage for a parsed `Mod`.
pub unsafe fn mod_alloc_sfx_channels(mod_: *mut Mod, sfxchans: i32) -> i32 {
    unsafe {
        if mod_.is_null() {
            return -1;
        }
        mod_alloc_sfx_channels_inner(&mut *mod_, sfxchans)
    }
}

/// # Safety
///
/// `mod_` must point to writable storage for a parsed `Mod`.
pub unsafe fn mod_trigger_note(
    mod_: *mut Mod,
    channel: i32,
    instnum: u8,
    freq: u16,
    vol: u8,
) -> i32 {
    unsafe {
        if mod_.is_null() {
            return -1;
        }
        mod_trigger_note_state(&mut *mod_, channel, instnum, freq, vol)
    }
}

/// # Safety
///
/// `mod_` must point to writable storage for a parsed `Mod`. The timing
/// tables are initialized lazily inside this function.
pub unsafe fn mod_start(mod_: *mut Mod) {
    unsafe {
        if mod_.is_null() {
            return;
        }
        mod_start_state(&mut *mod_);
    }
}

/// # Safety
///
/// `mod_` must point to writable storage for a `Mod`, and `mem` must point to a
/// complete MOD image whose backing bytes remain valid for any pointers written
/// into the parsed `Mod`.
pub unsafe fn mod_set_mod(mod_: *mut Mod, mem: *mut u8) -> i32 {
    if mod_.is_null() || mem.is_null() {
        return -1;
    }
    unsafe { mod_set_mod_ptr(mod_, mem) }
}

/// # Safety
///
/// `mod_` must point to writable storage for a `Mod`, and `fname` must point
/// to a NUL-terminated C string naming a readable MOD file.
pub unsafe fn mod_load(mod_: *mut Mod, fname: *const i8) -> i32 {
    unsafe {
        if mod_.is_null() {
            return -1;
        }
        mod_load_from_path(&mut *mod_, fname)
    }
}

/// # Safety
///
/// `mod_` must point to a writable `Mod` whose mixing buffer and timing fields
/// are initialized consistently with the intended playback mode.
pub unsafe fn mod_player(mod_: *mut Mod) -> u32 {
    unsafe {
        if mod_.is_null() {
            return 0;
        }
        mod_player_state(&mut *mod_)
    }
}

/// # Safety
///
/// `mod_` must point to writable storage for a `ModPlay`.
pub unsafe fn mod_play_init(mod_: *mut ModPlay) {
    unsafe {
        if mod_.is_null() {
            return;
        }
        init_state(&mut *mod_);
    }
}

/// # Safety
///
/// `mod_` must point to writable storage for a `ModPlay`.
pub unsafe fn mod_play_set_frequency(mod_: *mut ModPlay, freq: u32) -> i32 {
    unsafe {
        if mod_.is_null() {
            return -1;
        }
        set_frequency_state(&mut *mod_, freq)
    }
}

/// # Safety
///
/// `mod_` must point to writable storage for a `ModPlay`.
pub unsafe fn mod_play_set_stereo(mod_: *mut ModPlay, stereo: Bool) {
    unsafe {
        if mod_.is_null() {
            return;
        }
        set_stereo_state(&mut *mod_, stereo);
    }
}

/// # Safety
///
/// `mod_` must point to writable storage for a `ModPlay`.
pub unsafe fn mod_play_unload(mod_: *mut ModPlay) {
    unsafe {
        if mod_.is_null() {
            return;
        }
        unload_state(&mut *mod_);
    }
}

/// # Safety
///
/// `mod_` must point to writable storage for a `ModPlay`, and `mem` must point
/// to a complete caller-owned MOD image whose backing bytes remain valid until
/// the module is unloaded or replaced.
pub unsafe fn mod_play_set_mod(mod_: *mut ModPlay, mem: *const c_void) -> i32 {
    unsafe {
        if mod_.is_null() {
            return -1;
        }
        set_mod_state(&mut *mod_, mem as *mut u8)
    }
}

/// # Safety
///
/// `mod_` must point to writable storage for a `ModPlay`.
pub unsafe fn mod_play_alloc_sfx_channels(mod_: *mut ModPlay, sfxchans: u32) -> i32 {
    unsafe {
        if mod_.is_null() {
            return -1;
        }
        alloc_sfx_channels_state(&mut *mod_, sfxchans)
    }
}

/// # Safety
///
/// `mod_` must point to writable storage for a `ModPlay`.
pub unsafe fn mod_play_start(mod_: *mut ModPlay) -> i32 {
    unsafe {
        if mod_.is_null() {
            return -1;
        }
        start_state(&mut *mod_)
    }
}

/// # Safety
///
/// `mod_` must point to writable storage for a `ModPlay`.
pub unsafe fn mod_play_stop(mod_: *mut ModPlay) -> i32 {
    unsafe {
        if mod_.is_null() {
            return -1;
        }
        stop_state(&mut *mod_)
    }
}

/// # Safety
///
/// `mod_` must point to writable storage for a `ModPlay`.
pub unsafe fn mod_play_trigger_note(
    mod_: *mut ModPlay,
    chan: u32,
    inst: u8,
    freq: u16,
    vol: u8,
) -> i32 {
    unsafe {
        if mod_.is_null() {
            return -1;
        }
        trigger_note_state(&mut *mod_, chan, inst, freq, vol)
    }
}

/// # Safety
///
/// `mod_` must point to writable storage for a `ModPlay`.
pub unsafe fn mod_play_pause(mod_: *mut ModPlay, pause: Bool) -> i32 {
    unsafe {
        if mod_.is_null() {
            return -1;
        }
        pause_state(&mut *mod_, pause)
    }
}

/// # Safety
///
/// `mod_` must point to writable storage for a `ModPlay`.
pub unsafe fn mod_play_set_volume(mod_: *mut ModPlay, musicvolume: i32, sfxvolume: i32) {
    unsafe {
        if mod_.is_null() {
            return;
        }
        set_volume_state(&mut *mod_, musicvolume, sfxvolume);
    }
}

pub fn mod_play_mixing_time() -> u32 {
    mixing_time_state()
}

pub fn mod_play_request_time() -> u32 {
    request_time_state()
}

/// # Safety
///
/// `mod_` must point to a previously initialized `Mod` whose `patterndata`
/// pointer and song state are valid for pattern lookup.
pub unsafe fn get_note(mod_: *mut Mod, patternline: i32, channel: i32) -> u16 {
    unsafe { get_note_value(&*mod_, patternline, channel) }
}

/// # Safety
///
/// `mod_` must point to a previously initialized `Mod` whose `patterndata`
/// pointer and song state are valid for pattern lookup.
pub unsafe fn get_instr(mod_: *mut Mod, patternline: i32, channel: i32) -> u8 {
    unsafe { get_instr_value(&*mod_, patternline, channel) }
}

/// # Safety
///
/// `mod_` must point to a previously initialized `Mod` whose `patterndata`
/// pointer and song state are valid for pattern lookup.
pub unsafe fn get_effect(mod_: *mut Mod, patternline: i32, channel: i32) -> u8 {
    unsafe { get_effect_value(&*mod_, patternline, channel) }
}

/// # Safety
///
/// `mod_` must point to a previously initialized `Mod` whose `patterndata`
/// pointer and song state are valid for pattern lookup.
pub unsafe fn get_effect_op(mod_: *mut Mod, patternline: i32, channel: i32) -> u8 {
    unsafe { get_effect_op_value(&*mod_, patternline, channel) }
}

#[cfg(test)]
mod tests {
    extern crate std;

    use super::*;
    use core::mem::{MaybeUninit, size_of};
    use std::ffi::CString;
    use std::format;
    use std::fs::{self, OpenOptions};
    use std::io::Write;
    use std::path::PathBuf;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::vec;

    static TEST_LOCK: Mutex<()> = Mutex::new(());
    static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

    fn zeroed_mod() -> Mod {
        unsafe { MaybeUninit::<Mod>::zeroed().assume_init() }
    }

    fn zeroed_modplay() -> ModPlay {
        unsafe { MaybeUninit::<ModPlay>::zeroed().assume_init() }
    }

    fn write_fixture(path: &PathBuf) -> std::io::Result<()> {
        let mut bytes = vec![0u8; 1084 + 1024];
        bytes[1080..1084].copy_from_slice(b"M.K.");
        bytes[950] = 1;
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        if let Err(error) = file.write_all(&bytes) {
            drop(file);
            fs::remove_file(path)?;
            return Err(error);
        }
        Ok(())
    }

    fn unique_fixture() -> std::io::Result<(PathBuf, CString)> {
        for _ in 0..100 {
            let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
            let path = std::env::temp_dir().join(format!(
                "modplay-test-{}-{sequence}.mod",
                std::process::id()
            ));
            let path_text = path.to_str().ok_or_else(|| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "temporary module path is not UTF-8",
                )
            })?;
            let cpath = CString::new(path_text).map_err(|_| {
                std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    "temporary module path contains NUL",
                )
            })?;
            match write_fixture(&path) {
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
                result => return result.map(|()| (path, cpath)),
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "no unique module path",
        ))
    }

    #[test]
    fn fixture_write_rejects_existing_file() -> std::io::Result<()> {
        let (path, _) = unique_fixture()?;
        let result = write_fixture(&path);
        assert_eq!(
            result.unwrap_err().kind(),
            std::io::ErrorKind::AlreadyExists
        );
        fs::remove_file(path)
    }

    #[test]
    fn player_mixes_only_after_a_worker_signal_and_uses_fixed_dma_size() {
        use core::cell::Cell;
        use std::vec::Vec;

        assert_eq!(SOUND_BUFFER_SIZE, 5760);
        assert_eq!(next_audio_buffer(0), 1);
        assert_eq!(next_audio_buffer(1), 0);

        let playing = Cell::new(true);
        let waits = Cell::new(0);
        let mixed_after_waits = std::cell::RefCell::new(Vec::new());
        let exit = player_worker_loop(
            || playing.get(),
            || {
                let wait_count = waits.get() + 1;
                waits.set(wait_count);
                if wait_count == 2 {
                    playing.set(false);
                }
                true
            },
            || mixed_after_waits.borrow_mut().push(waits.get()),
        );

        assert_eq!(waits.get(), 2);
        assert_eq!(*mixed_after_waits.borrow(), [1]);
        assert_eq!(exit, PlayerWorkerExit::Stopped);
    }

    #[test]
    fn player_worker_exits_when_queue_wait_fails() {
        let playing = AtomicBool::new(true);
        let failed = AtomicBool::new(false);
        let mixed = core::cell::Cell::new(false);
        let exit = player_worker_loop(
            || playing.load(Ordering::Acquire),
            || false,
            || mixed.set(true),
        );
        if exit == PlayerWorkerExit::WaitFailed {
            record_worker_wait_failure(&playing, &failed);
        }
        assert_eq!(exit, PlayerWorkerExit::WaitFailed);
        assert!(!mixed.get());
        assert!(!playing.load(Ordering::Acquire));
        assert!(failed.load(Ordering::Acquire));
        assert_eq!(worker_stop_result(true, true), -2);
        assert_eq!(worker_stop_result(false, true), -1);
    }

    #[test]
    fn stop_reports_worker_error_after_cleanup_and_keeps_state_on_join_error() {
        let mut modplay = zeroed_modplay();
        modplay.playing = TRUE_BOOL;
        assert_eq!(complete_stop_state(&mut modplay, -2), -1);
        assert_eq!(modplay.playing, FALSE_BOOL);

        modplay.playing = TRUE_BOOL;
        assert_eq!(complete_stop_state(&mut modplay, -1), -1);
        assert_eq!(modplay.playing, TRUE_BOOL);

        assert_eq!(complete_stop_state(&mut modplay, 0), 0);
        assert_eq!(modplay.playing, FALSE_BOOL);
    }

    #[test]
    fn initial_audio_buffers_are_zeroed_before_dma() {
        let mut buffers = [[0xA5; SOUND_BUFFER_SIZE as usize]; 2];
        clear_audio_buffers(&mut buffers);
        assert!(buffers[0].iter().all(|&byte| byte == 0));
        assert!(buffers[1].iter().all(|&byte| byte == 0));
    }

    #[test]
    fn player_shutdown_stops_dma_signals_joins_then_closes_queue() {
        use std::vec::Vec;

        let operations = std::cell::RefCell::new(Vec::new());
        let joined = shutdown_player_worker(
            PlayerWorkerState {
                was_playing: true,
                has_queue: true,
                has_thread: true,
            },
            || operations.borrow_mut().push("stop DMA"),
            || operations.borrow_mut().push("remove DMA callback"),
            || operations.borrow_mut().push("signal worker"),
            || {
                operations.borrow_mut().push("join worker");
                true
            },
            || operations.borrow_mut().push("close queue"),
            || false,
        );

        assert_eq!(joined, Some(false));
        assert_eq!(
            *operations.borrow(),
            [
                "stop DMA",
                "remove DMA callback",
                "signal worker",
                "join worker",
                "close queue"
            ]
        );
    }

    #[test]
    fn player_shutdown_keeps_queue_open_if_join_fails() {
        use std::vec::Vec;

        let operations = std::cell::RefCell::new(Vec::new());
        let joined = shutdown_player_worker(
            PlayerWorkerState {
                was_playing: true,
                has_queue: true,
                has_thread: true,
            },
            || operations.borrow_mut().push("stop DMA"),
            || operations.borrow_mut().push("remove DMA callback"),
            || operations.borrow_mut().push("signal worker"),
            || {
                operations.borrow_mut().push("join worker");
                false
            },
            || operations.borrow_mut().push("close queue"),
            || false,
        );

        assert_eq!(joined, None);
        assert_eq!(
            *operations.borrow(),
            [
                "stop DMA",
                "remove DMA callback",
                "signal worker",
                "join worker"
            ]
        );
    }

    #[test]
    fn worker_failure_still_stops_dma_and_removes_callback_before_join() {
        use std::vec::Vec;

        let operations = std::cell::RefCell::new(Vec::new());
        let joined = shutdown_player_worker(
            PlayerWorkerState {
                was_playing: false,
                has_queue: true,
                has_thread: true,
            },
            || operations.borrow_mut().push("stop DMA"),
            || operations.borrow_mut().push("remove DMA callback"),
            || operations.borrow_mut().push("signal worker"),
            || {
                operations.borrow_mut().push("join worker");
                true
            },
            || operations.borrow_mut().push("close queue"),
            || false,
        );

        assert_eq!(joined, Some(false));
        assert_eq!(
            *operations.borrow(),
            [
                "stop DMA",
                "remove DMA callback",
                "signal worker",
                "join worker",
                "close queue",
            ]
        );
    }

    #[test]
    fn worker_error_during_join_is_reported_after_join_completes() {
        let failed = AtomicBool::new(false);
        let shutdown_result = shutdown_player_worker(
            PlayerWorkerState {
                was_playing: true,
                has_queue: true,
                has_thread: true,
            },
            || {},
            || {},
            || {},
            || {
                failed.store(true, Ordering::Release);
                true
            },
            || {},
            || failed.load(Ordering::Acquire),
        );

        assert_eq!(shutdown_result, Some(true));
        assert_eq!(worker_stop_result(true, true), -2);
    }

    #[test]
    fn layout_snapshot_and_exports_are_stable() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let layout = layout_snapshot();
        assert_eq!(layout.len(), 4);
        assert_eq!(layout[0].name, "MOD_INSTR");
        assert_eq!(layout[1].name, "MOD");
        assert_eq!(layout[2].name, "MODSNDBUF");
        assert_eq!(layout[3].name, "MODPlay");
        assert!(size_of::<Mod>() > 0);
        let _ = mod_set_mod as unsafe fn(*mut Mod, *mut u8) -> i32;
        let _ = mod_load as unsafe fn(*mut Mod, *const i8) -> i32;
        let _ = mod_free as unsafe fn(*mut Mod);
        let _ = mod_play_init as unsafe fn(*mut ModPlay);
        let _ = mod_play_unload as unsafe fn(*mut ModPlay);
    }

    #[test]
    fn caller_owned_setmod_buffer_survives_free() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let mut mod_ = zeroed_mod();
        let mut fixture = vec![0u8; 1084 + 1024];
        fixture[1080..1084].copy_from_slice(b"M.K.");
        // SAFETY: The output and caller-owned fixture remain live for all calls below.
        assert_eq!(
            unsafe { mod_set_mod(&mut mod_ as *mut Mod, fixture.as_mut_ptr()) },
            0
        );
        assert_eq!(mod_.set, 1);
        assert_eq!(mod_.loaded, 0);
        assert_eq!(mod_.modraw, fixture.as_mut_ptr());
        unsafe { mod_free(&mut mod_ as *mut Mod) };
        assert_eq!(mod_.set, 0);
        assert_eq!(mod_.loaded, 0);
        assert_eq!(mod_.modraw, fixture.as_mut_ptr());
    }

    #[test]
    fn loaded_mod_is_freed_and_cleared_by_mod_free() -> std::io::Result<()> {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let (path, cpath) = unique_fixture()?;
        let mut mod_ = zeroed_mod();
        // SAFETY: The output and NUL-terminated path remain valid for the call.
        let rc = unsafe { mod_load(&mut mod_ as *mut Mod, cpath.as_ptr().cast()) };
        assert_eq!(rc, 0);
        assert_eq!(mod_.loaded, 1);
        assert!(!mod_.modraw.is_null());
        // SAFETY: `mod_` remains a valid unique pointer.
        unsafe { mod_free(&mut mod_ as *mut Mod) };
        assert_eq!(mod_.loaded, 0);
        assert_eq!(mod_.set, 0);
        assert!(mod_.modraw.is_null());
        fs::remove_file(&path)?;
        Ok(())
    }

    #[test]
    fn init_and_unload_reset_public_state() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let mut modplay = zeroed_modplay();
        // SAFETY: `modplay` remains a valid unique pointer for both calls.
        unsafe { mod_play_init(&mut modplay as *mut ModPlay) };
        assert_eq!(modplay.playing, 0);
        assert_eq!(modplay.paused, 0);
        assert_eq!(modplay.playfreq, 48_000);
        assert!(modplay.mod_.modraw.is_null());
        unsafe { mod_play_unload(&mut modplay as *mut ModPlay) };
        assert_eq!(modplay.mod_.set, 0);
        assert_eq!(modplay.mod_.loaded, 0);
        assert!(modplay.mod_.modraw.is_null());
    }

    #[test]
    fn fallible_unload_stops_playback_before_freeing_module() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let mut modplay = zeroed_modplay();
        let mut fixture = vec![0u8; 1084 + 1024];
        fixture[1080..1084].copy_from_slice(b"M.K.");
        init_state(&mut modplay);
        assert_eq!(
            unsafe { set_mod_state(&mut modplay, fixture.as_mut_ptr()) },
            0
        );
        assert_eq!(start_state(&mut modplay), 0);

        assert_eq!(try_unload_state(&mut modplay), 0);
        assert_eq!(modplay.playing, 0);
        assert_eq!(modplay.mod_.set, 0);
        assert!(!SND_PLAYING.load(Ordering::Acquire));
    }

    #[test]
    fn null_and_basic_control_paths_return_expected_values() {
        let _guard = TEST_LOCK
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let mut modplay = zeroed_modplay();
        // SAFETY: These ABI calls explicitly accept null for error-path validation;
        // all non-null pointers below refer to live test values and buffers.
        assert_eq!(
            unsafe { mod_play_set_mod(core::ptr::null_mut(), core::ptr::null()) },
            -1
        );
        assert_eq!(
            unsafe { mod_play_set_frequency(core::ptr::null_mut(), 48_000) },
            -1
        );
        unsafe { mod_play_init(&mut modplay as *mut ModPlay) };
        assert_eq!(
            unsafe { mod_play_set_frequency(&mut modplay as *mut ModPlay, 48_000) },
            0
        );
        unsafe { mod_play_set_stereo(&mut modplay as *mut ModPlay, 1) };
        assert_eq!(modplay.stereo, 1);
        assert_eq!(
            unsafe { mod_play_alloc_sfx_channels(&mut modplay as *mut ModPlay, 2) },
            -1
        );
        let mut fixture = vec![0u8; 1084 + 1024];
        fixture[1080..1084].copy_from_slice(b"M.K.");
        assert_eq!(
            unsafe { mod_play_set_mod(&mut modplay as *mut ModPlay, fixture.as_ptr().cast()) },
            0
        );
        assert_eq!(
            unsafe { mod_play_alloc_sfx_channels(&mut modplay as *mut ModPlay, 2) },
            0
        );
        assert_eq!(unsafe { mod_play_start(&mut modplay as *mut ModPlay) }, 0);
        assert_eq!(
            unsafe { mod_play_pause(&mut modplay as *mut ModPlay, 1) },
            0
        );
        assert_eq!(
            unsafe { mod_play_pause(&mut modplay as *mut ModPlay, 0) },
            0
        );
        assert_eq!(unsafe { mod_play_stop(&mut modplay as *mut ModPlay) }, 0);
        unsafe { mod_play_unload(&mut modplay as *mut ModPlay) };
    }
}
