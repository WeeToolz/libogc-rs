#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::mem::{align_of, offset_of, size_of};

use modplay_capi::{Mod, ModInstr, ModPlay, ModSndBuf};
use ogc_sys::Bool;

#[repr(C)]
struct LayoutEntry {
    name: [u8; 64],
    value: u32,
}

impl LayoutEntry {
    const fn new(name: &str, value: usize) -> Self {
        Self {
            name: name_bytes(name),
            value: value as u32,
        }
    }
}

const fn name_bytes(name: &str) -> [u8; 64] {
    let src = name.as_bytes();
    let mut dst = [0_u8; 64];
    let mut idx = 0;
    while idx < src.len() {
        dst[idx] = src[idx];
        idx += 1;
    }
    dst
}

#[cfg_attr(target_vendor = "apple", unsafe(link_section = "__DATA,.layout"))]
#[cfg_attr(not(target_vendor = "apple"), unsafe(link_section = ".layout"))]
#[used]
static MODPLAY_LAYOUT: [LayoutEntry; 96] = [
    LayoutEntry::new("BOOL.size", size_of::<Bool>()),
    LayoutEntry::new("BOOL.align", align_of::<Bool>()),
    LayoutEntry::new("MOD_INSTR.size", size_of::<ModInstr>()),
    LayoutEntry::new("MOD_INSTR.align", align_of::<ModInstr>()),
    LayoutEntry::new("MOD_INSTR.offset.name", offset_of!(ModInstr, name)),
    LayoutEntry::new("MOD_INSTR.offset.length", offset_of!(ModInstr, length)),
    LayoutEntry::new("MOD_INSTR.offset.finetune", offset_of!(ModInstr, finetune)),
    LayoutEntry::new("MOD_INSTR.offset.volume", offset_of!(ModInstr, volume)),
    LayoutEntry::new(
        "MOD_INSTR.offset.loop_start",
        offset_of!(ModInstr, loop_start),
    ),
    LayoutEntry::new("MOD_INSTR.offset.loop_end", offset_of!(ModInstr, loop_end)),
    LayoutEntry::new(
        "MOD_INSTR.offset.loop_length",
        offset_of!(ModInstr, loop_length),
    ),
    LayoutEntry::new("MOD_INSTR.offset.looped", offset_of!(ModInstr, looped)),
    LayoutEntry::new("MOD_INSTR.offset.data", offset_of!(ModInstr, data)),
    LayoutEntry::new("MOD.size", size_of::<Mod>()),
    LayoutEntry::new("MOD.align", align_of::<Mod>()),
    LayoutEntry::new("MOD.offset.loaded", offset_of!(Mod, loaded)),
    LayoutEntry::new("MOD.offset.name", offset_of!(Mod, name)),
    LayoutEntry::new("MOD.offset.instrument", offset_of!(Mod, instrument)),
    LayoutEntry::new("MOD.offset.num_patterns", offset_of!(Mod, num_patterns)),
    LayoutEntry::new("MOD.offset.song_length", offset_of!(Mod, song_length)),
    LayoutEntry::new("MOD.offset.ciaa", offset_of!(Mod, ciaa)),
    LayoutEntry::new("MOD.offset.song", offset_of!(Mod, song)),
    LayoutEntry::new("MOD.offset.id", offset_of!(Mod, id)),
    LayoutEntry::new("MOD.offset.patterndata", offset_of!(Mod, patterndata)),
    LayoutEntry::new("MOD.offset.num_instr", offset_of!(Mod, num_instr)),
    LayoutEntry::new("MOD.offset.num_voices", offset_of!(Mod, num_voices)),
    LayoutEntry::new("MOD.offset.num_channels", offset_of!(Mod, num_channels)),
    LayoutEntry::new("MOD.offset.mixingbuf", offset_of!(Mod, mixingbuf)),
    LayoutEntry::new("MOD.offset.mixingbuflen", offset_of!(Mod, mixingbuflen)),
    LayoutEntry::new("MOD.offset.shiftval", offset_of!(Mod, shiftval)),
    LayoutEntry::new("MOD.offset.channel_active", offset_of!(Mod, channel_active)),
    LayoutEntry::new("MOD.offset.patterndelay", offset_of!(Mod, patterndelay)),
    LayoutEntry::new("MOD.offset.speed", offset_of!(Mod, speed)),
    LayoutEntry::new("MOD.offset.bpm", offset_of!(Mod, bpm)),
    LayoutEntry::new("MOD.offset.songpos", offset_of!(Mod, songpos)),
    LayoutEntry::new("MOD.offset.patternline", offset_of!(Mod, patternline)),
    LayoutEntry::new(
        "MOD.offset.patternline_jumpto",
        offset_of!(Mod, patternline_jumpto),
    ),
    LayoutEntry::new(
        "MOD.offset.patternline_jumpcount",
        offset_of!(Mod, patternline_jumpcount),
    ),
    LayoutEntry::new("MOD.offset.speedcounter", offset_of!(Mod, speedcounter)),
    LayoutEntry::new("MOD.offset.freq", offset_of!(Mod, freq)),
    LayoutEntry::new("MOD.offset.bits", offset_of!(Mod, bits)),
    LayoutEntry::new("MOD.offset.channels", offset_of!(Mod, channels)),
    LayoutEntry::new("MOD.offset.playpos", offset_of!(Mod, playpos)),
    LayoutEntry::new("MOD.offset.instnum", offset_of!(Mod, instnum)),
    LayoutEntry::new("MOD.offset.chanfreq", offset_of!(Mod, chanfreq)),
    LayoutEntry::new("MOD.offset.channote", offset_of!(Mod, channote)),
    LayoutEntry::new("MOD.offset.volume", offset_of!(Mod, volume)),
    LayoutEntry::new("MOD.offset.effect", offset_of!(Mod, effect)),
    LayoutEntry::new("MOD.offset.effectop", offset_of!(Mod, effectop)),
    LayoutEntry::new("MOD.offset.last_effect", offset_of!(Mod, last_effect)),
    LayoutEntry::new("MOD.offset.portamento_to", offset_of!(Mod, portamento_to)),
    LayoutEntry::new("MOD.offset.porta_speed", offset_of!(Mod, porta_speed)),
    LayoutEntry::new(
        "MOD.offset.retrigger_counter",
        offset_of!(Mod, retrigger_counter),
    ),
    LayoutEntry::new("MOD.offset.arp_counter", offset_of!(Mod, arp_counter)),
    LayoutEntry::new("MOD.offset.sintabpos", offset_of!(Mod, sintabpos)),
    LayoutEntry::new("MOD.offset.vib_freq", offset_of!(Mod, vib_freq)),
    LayoutEntry::new("MOD.offset.vib_depth", offset_of!(Mod, vib_depth)),
    LayoutEntry::new("MOD.offset.vib_basefreq", offset_of!(Mod, vib_basefreq)),
    LayoutEntry::new("MOD.offset.trem_basevol", offset_of!(Mod, trem_basevol)),
    LayoutEntry::new("MOD.offset.trem_freq", offset_of!(Mod, trem_freq)),
    LayoutEntry::new("MOD.offset.trem_depth", offset_of!(Mod, trem_depth)),
    LayoutEntry::new("MOD.offset.glissando", offset_of!(Mod, glissando)),
    LayoutEntry::new("MOD.offset.trem_wave", offset_of!(Mod, trem_wave)),
    LayoutEntry::new("MOD.offset.vib_wave", offset_of!(Mod, vib_wave)),
    LayoutEntry::new("MOD.offset.nextinstr", offset_of!(Mod, nextinstr)),
    LayoutEntry::new("MOD.offset.nextnote", offset_of!(Mod, nextnote)),
    LayoutEntry::new("MOD.offset.samplespertick", offset_of!(Mod, samplespertick)),
    LayoutEntry::new("MOD.offset.samplescounter", offset_of!(Mod, samplescounter)),
    LayoutEntry::new("MOD.offset.modraw", offset_of!(Mod, modraw)),
    LayoutEntry::new("MOD.offset.bpmtab", offset_of!(Mod, bpmtab)),
    LayoutEntry::new("MOD.offset.inctab", offset_of!(Mod, inctab)),
    LayoutEntry::new("MOD.offset.notebeats", offset_of!(Mod, notebeats)),
    LayoutEntry::new("MOD.offset.callback", offset_of!(Mod, callback)),
    LayoutEntry::new("MOD.offset.musicvolume", offset_of!(Mod, musicvolume)),
    LayoutEntry::new("MOD.offset.sfxvolume", offset_of!(Mod, sfxvolume)),
    LayoutEntry::new("MOD.offset.set", offset_of!(Mod, set)),
    LayoutEntry::new("MOD.offset.notify", offset_of!(Mod, notify)),
    LayoutEntry::new("MODSNDBUF.size", size_of::<ModSndBuf>()),
    LayoutEntry::new("MODSNDBUF.align", align_of::<ModSndBuf>()),
    LayoutEntry::new("MODSNDBUF.offset.freq", offset_of!(ModSndBuf, freq)),
    LayoutEntry::new("MODSNDBUF.offset.fmt", offset_of!(ModSndBuf, fmt)),
    LayoutEntry::new("MODSNDBUF.offset.chans", offset_of!(ModSndBuf, chans)),
    LayoutEntry::new("MODSNDBUF.offset.samples", offset_of!(ModSndBuf, samples)),
    LayoutEntry::new("MODSNDBUF.offset.usr_data", offset_of!(ModSndBuf, usr_data)),
    LayoutEntry::new("MODSNDBUF.offset.callback", offset_of!(ModSndBuf, callback)),
    LayoutEntry::new("MODPlay.size", size_of::<ModPlay>()),
    LayoutEntry::new("MODPlay.align", align_of::<ModPlay>()),
    LayoutEntry::new("MODPlay.offset.mod", offset_of!(ModPlay, mod_)),
    LayoutEntry::new("MODPlay.offset.playing", offset_of!(ModPlay, playing)),
    LayoutEntry::new("MODPlay.offset.paused", offset_of!(ModPlay, paused)),
    LayoutEntry::new("MODPlay.offset.bits", offset_of!(ModPlay, bits)),
    LayoutEntry::new("MODPlay.offset.stereo", offset_of!(ModPlay, stereo)),
    LayoutEntry::new(
        "MODPlay.offset.manual_polling",
        offset_of!(ModPlay, manual_polling),
    ),
    LayoutEntry::new("MODPlay.offset.playfreq", offset_of!(ModPlay, playfreq)),
    LayoutEntry::new(
        "MODPlay.offset.numSFXChans",
        offset_of!(ModPlay, num_sfx_chans),
    ),
    LayoutEntry::new("MODPlay.offset.soundBuf", offset_of!(ModPlay, sound_buf)),
];
