//! Polyphase PCM synthesis from `synth.c`.

use crate::tables::synth::D;
use crate::{
    fixed::Fixed,
    frame::{Frame, OPTION_HALFSAMPLERATE},
};

const PCM_CAPACITY: usize = 1152;

/// Persistent polyphase history with the dimensions used by `mad_synth`.
pub type FilterState = [[[[[Fixed; 8]; 16]; 2]; 2]; 2];

/// Synthesized PCM data. Samples use libmad's 28-bit fixed-point format.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pcm {
    pub samplerate: u32,
    pub channels: u16,
    pub length: u16,
    pub samples: [[Fixed; PCM_CAPACITY]; 2],
}

impl Default for Pcm {
    fn default() -> Self {
        Self {
            samplerate: 0,
            channels: 0,
            length: 0,
            samples: [[0; PCM_CAPACITY]; 2],
        }
    }
}

/// Stateful MPEG polyphase synthesis filter.
///
/// The filter dimensions follow `synth.h`: `[channel][even/odd bank][phase
/// parity][subband][history slot]`.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Synth {
    filter: FilterState,
    pub phase: usize,
    pub pcm: Pcm,
}

impl Synth {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the persistent filter history from a C-compatible synth state.
    pub fn set_filter_state(&mut self, filter: FilterState) {
        self.filter = filter;
    }

    /// Return a copy of the persistent filter history for ABI synchronization.
    pub const fn filter_state(&self) -> FilterState {
        self.filter
    }

    /// Clear the filter history without changing phase or PCM metadata.
    pub fn mute(&mut self) {
        self.filter = [[[[[0; 8]; 16]; 2]; 2]; 2];
    }

    /// Synthesize one decoded frame and return the owned PCM buffer.
    pub fn frame(&mut self, frame: &Frame) -> &Pcm {
        let channels = frame.header.channels();
        let slots = frame.header.subband_samples();
        let half = frame.options & OPTION_HALFSAMPLERATE != 0;
        self.pcm.samplerate = if half {
            frame.header.samplerate / 2
        } else {
            frame.header.samplerate
        };
        self.pcm.channels = channels as u16;
        self.pcm.length = (slots * if half { 16 } else { 32 }) as u16;
        self.pcm.samples = [[0; PCM_CAPACITY]; 2];

        for ch in 0..channels {
            let mut phase = self.phase;
            for slot in 0..slots {
                let transformed = dct32(&frame.sbsample[ch][slot]);
                let history_slot = phase >> 1;
                let parity = phase & 1;
                for index in 0..16 {
                    self.filter[ch][0][parity][index][history_slot] = transformed[16 + index];
                    self.filter[ch][1][parity][index][history_slot] = transformed[15 - index];
                }
                let out = polyphase(&self.filter[ch], phase);
                if half {
                    for i in 0..16 {
                        self.pcm.samples[ch][slot * 16 + i] = out[i * 2];
                    }
                } else {
                    self.pcm.samples[ch][slot * 32..slot * 32 + 32].copy_from_slice(&out);
                }
                phase = (phase + 1) & 15;
            }
        }
        self.phase = (self.phase + slots) & 15;
        &self.pcm
    }
}

#[inline]
#[cfg(not(target_arch = "powerpc"))]
const fn preshift_window() -> [[Fixed; 32]; 17] {
    let mut table = [[0; 32]; 17];
    let mut row = 0;
    while row < 17 {
        let mut column = 0;
        while column < 32 {
            let coefficient = D[row][column] as i64;
            let magnitude = if coefficient < 0 {
                -coefficient
            } else {
                coefficient
            };
            let preshifted = ((magnitude + (1 << 13)) >> 14) as Fixed;
            table[row][column] = if coefficient < 0 {
                -preshifted
            } else {
                preshifted
            };
            column += 1;
        }
        row += 1;
    }
    table
}

#[cfg(not(target_arch = "powerpc"))]
const D_SSO: [[Fixed; 32]; 17] = preshift_window();

#[inline]
fn synthesis_window() -> &'static [[Fixed; 32]; 17] {
    #[cfg(target_arch = "powerpc")]
    {
        &D
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        &D_SSO
    }
}

#[inline]
fn dot8_mac(values: &[Fixed; 8], coeffs: &[Fixed; 32], indexes: [usize; 8]) -> u32 {
    let mut accumulator = 0u32;
    for i in 0..8 {
        #[cfg(target_arch = "powerpc")]
        let product = crate::fixed::mul(values[i], coeffs[indexes[i]]);
        #[cfg(not(target_arch = "powerpc"))]
        let product = values[i].wrapping_mul(coeffs[indexes[i]]);
        accumulator = accumulator.wrapping_add(product as u32);
    }
    accumulator
}

#[inline]
fn finish_synthesis_accumulator(accumulator: u32) -> Fixed {
    #[cfg(target_arch = "powerpc")]
    {
        accumulator as Fixed
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        (accumulator as Fixed) >> 2
    }
}

fn polyphase(filter: &[[[[Fixed; 8]; 16]; 2]; 2], phase: usize) -> [Fixed; 32] {
    let window = synthesis_window();
    let pe = phase & !1;
    let po = (phase.wrapping_sub(1) & 15) | 1;
    let fe = &filter[0][phase & 1];
    let fx = &filter[0][(!phase) & 1];
    let fo = &filter[1][(!phase) & 1];
    let mut out = [0; 32];

    let odd = dot8_mac(
        &fx[0],
        &window[0],
        [
            po,
            po + 14,
            po + 12,
            po + 10,
            po + 8,
            po + 6,
            po + 4,
            po + 2,
        ],
    );
    let even = dot8_mac(
        &fe[0],
        &window[0],
        [
            pe,
            pe + 14,
            pe + 12,
            pe + 10,
            pe + 8,
            pe + 6,
            pe + 4,
            pe + 2,
        ],
    );
    out[0] = finish_synthesis_accumulator(odd.wrapping_neg().wrapping_add(even));

    for sb in 1..16 {
        let row: &[Fixed; 32] = &window[sb];
        let odd = dot8_mac(
            &fo[sb - 1],
            row,
            [
                po,
                po + 14,
                po + 12,
                po + 10,
                po + 8,
                po + 6,
                po + 4,
                po + 2,
            ],
        );
        let even = dot8_mac(
            &fe[sb],
            row,
            [
                pe,
                pe + 14,
                pe + 12,
                pe + 10,
                pe + 8,
                pe + 6,
                pe + 4,
                pe + 2,
            ],
        );
        out[sb] = finish_synthesis_accumulator(odd.wrapping_neg().wrapping_add(even));

        // The mirrored half uses the reversed D row and adjacent history bank,
        // as expressed by the pointer arithmetic in synth.c.
        let left = dot8_mac(
            &fe[sb],
            row,
            [
                15 - pe,
                17 - pe,
                19 - pe,
                21 - pe,
                23 - pe,
                25 - pe,
                27 - pe,
                29 - pe,
            ],
        );
        let right = dot8_mac(
            &fo[sb - 1],
            row,
            [
                15 - po,
                17 - po,
                19 - po,
                21 - po,
                23 - po,
                25 - po,
                27 - po,
                29 - po,
            ],
        );
        out[32 - sb] = finish_synthesis_accumulator(left.wrapping_add(right));
    }
    out[16] = finish_synthesis_accumulator(
        dot8_mac(
            &fo[15],
            &window[16],
            [
                po,
                po + 14,
                po + 12,
                po + 10,
                po + 8,
                po + 6,
                po + 4,
                po + 2,
            ],
        )
        .wrapping_neg(),
    );
    out
}

const COSTAB: [Fixed; 31] = [
    0x0ffb10f2u32 as Fixed,
    0x0fec46d2u32 as Fixed,
    0x0fd3aac0u32 as Fixed,
    0x0fb14be8u32 as Fixed,
    0x0f853f7eu32 as Fixed,
    0x0f4fa0abu32 as Fixed,
    0x0f109082u32 as Fixed,
    0x0ec835e8u32 as Fixed,
    0x0e76bd7au32 as Fixed,
    0x0e1c5979u32 as Fixed,
    0x0db941a3u32 as Fixed,
    0x0d4db315u32 as Fixed,
    0x0cd9f024u32 as Fixed,
    0x0c5e4036u32 as Fixed,
    0x0bdaef91u32 as Fixed,
    0x0b504f33u32 as Fixed,
    0x0abeb49au32 as Fixed,
    0x0a267993u32 as Fixed,
    0x0987fbfeu32 as Fixed,
    0x08e39d9du32 as Fixed,
    0x0839c3cdu32 as Fixed,
    0x078ad74eu32 as Fixed,
    0x06d74402u32 as Fixed,
    0x061f78aau32 as Fixed,
    0x0563e69du32 as Fixed,
    0x04a5018cu32 as Fixed,
    0x03e33f2fu32 as Fixed,
    0x031f1708u32 as Fixed,
    0x0259020eu32 as Fixed,
    0x01917a6cu32 as Fixed,
    0x00c8fb30u32 as Fixed,
];

#[inline]
fn configured_mul(x: Fixed, y: Fixed) -> Fixed {
    crate::fixed::mul_configured(x, y)
}

#[inline]
fn configured_dct_shift(x: Fixed) -> Fixed {
    #[cfg(target_arch = "powerpc")]
    {
        x
    }
    #[cfg(not(target_arch = "powerpc"))]
    {
        ((x as i64 + (1 << 11)) >> 12) as Fixed
    }
}

fn dct32(input: &[Fixed; 32]) -> [Fixed; 32] {
    // Port of synth.c's factorized `dct32`. Intermediate additions wrap as
    // 32-bit values. PowerPC keeps the C FPM_PPC Q28 result; the host
    // differential profile applies the OPT_SSO DCT shift.
    let mut t = [0i32; 177];
    let mut out = [0i32; 32];
    t[0] = (input[0]).wrapping_add(input[31]);
    t[16] = configured_mul((input[0]).wrapping_sub(input[31]), COSTAB[0]);
    t[1] = (input[15]).wrapping_add(input[16]);
    t[17] = configured_mul((input[15]).wrapping_sub(input[16]), COSTAB[30]);
    t[41] = (t[16]).wrapping_add(t[17]);
    t[59] = configured_mul((t[16]).wrapping_sub(t[17]), COSTAB[1]);
    t[33] = (t[0]).wrapping_add(t[1]);
    t[50] = configured_mul((t[0]).wrapping_sub(t[1]), COSTAB[1]);
    t[2] = (input[7]).wrapping_add(input[24]);
    t[18] = configured_mul((input[7]).wrapping_sub(input[24]), COSTAB[14]);
    t[3] = (input[8]).wrapping_add(input[23]);
    t[19] = configured_mul((input[8]).wrapping_sub(input[23]), COSTAB[16]);
    t[42] = (t[18]).wrapping_add(t[19]);
    t[60] = configured_mul((t[18]).wrapping_sub(t[19]), COSTAB[29]);
    t[34] = (t[2]).wrapping_add(t[3]);
    t[51] = configured_mul((t[2]).wrapping_sub(t[3]), COSTAB[29]);
    t[4] = (input[3]).wrapping_add(input[28]);
    t[20] = configured_mul((input[3]).wrapping_sub(input[28]), COSTAB[6]);
    t[5] = (input[12]).wrapping_add(input[19]);
    t[21] = configured_mul((input[12]).wrapping_sub(input[19]), COSTAB[24]);
    t[43] = (t[20]).wrapping_add(t[21]);
    t[61] = configured_mul((t[20]).wrapping_sub(t[21]), COSTAB[13]);
    t[35] = (t[4]).wrapping_add(t[5]);
    t[52] = configured_mul((t[4]).wrapping_sub(t[5]), COSTAB[13]);
    t[6] = (input[4]).wrapping_add(input[27]);
    t[22] = configured_mul((input[4]).wrapping_sub(input[27]), COSTAB[8]);
    t[7] = (input[11]).wrapping_add(input[20]);
    t[23] = configured_mul((input[11]).wrapping_sub(input[20]), COSTAB[22]);
    t[44] = (t[22]).wrapping_add(t[23]);
    t[62] = configured_mul((t[22]).wrapping_sub(t[23]), COSTAB[17]);
    t[36] = (t[6]).wrapping_add(t[7]);
    t[53] = configured_mul((t[6]).wrapping_sub(t[7]), COSTAB[17]);
    t[8] = (input[1]).wrapping_add(input[30]);
    t[24] = configured_mul((input[1]).wrapping_sub(input[30]), COSTAB[2]);
    t[9] = (input[14]).wrapping_add(input[17]);
    t[25] = configured_mul((input[14]).wrapping_sub(input[17]), COSTAB[28]);
    t[45] = (t[24]).wrapping_add(t[25]);
    t[63] = configured_mul((t[24]).wrapping_sub(t[25]), COSTAB[5]);
    t[37] = (t[8]).wrapping_add(t[9]);
    t[54] = configured_mul((t[8]).wrapping_sub(t[9]), COSTAB[5]);
    t[10] = (input[6]).wrapping_add(input[25]);
    t[26] = configured_mul((input[6]).wrapping_sub(input[25]), COSTAB[12]);
    t[11] = (input[9]).wrapping_add(input[22]);
    t[27] = configured_mul((input[9]).wrapping_sub(input[22]), COSTAB[18]);
    t[46] = (t[26]).wrapping_add(t[27]);
    t[64] = configured_mul((t[26]).wrapping_sub(t[27]), COSTAB[25]);
    t[38] = (t[10]).wrapping_add(t[11]);
    t[55] = configured_mul((t[10]).wrapping_sub(t[11]), COSTAB[25]);
    t[12] = (input[2]).wrapping_add(input[29]);
    t[28] = configured_mul((input[2]).wrapping_sub(input[29]), COSTAB[4]);
    t[13] = (input[13]).wrapping_add(input[18]);
    t[29] = configured_mul((input[13]).wrapping_sub(input[18]), COSTAB[26]);
    t[47] = (t[28]).wrapping_add(t[29]);
    t[65] = configured_mul((t[28]).wrapping_sub(t[29]), COSTAB[9]);
    t[39] = (t[12]).wrapping_add(t[13]);
    t[56] = configured_mul((t[12]).wrapping_sub(t[13]), COSTAB[9]);
    t[14] = (input[5]).wrapping_add(input[26]);
    t[30] = configured_mul((input[5]).wrapping_sub(input[26]), COSTAB[10]);
    t[15] = (input[10]).wrapping_add(input[21]);
    t[31] = configured_mul((input[10]).wrapping_sub(input[21]), COSTAB[20]);
    t[48] = (t[30]).wrapping_add(t[31]);
    t[66] = configured_mul((t[30]).wrapping_sub(t[31]), COSTAB[21]);
    t[40] = (t[14]).wrapping_add(t[15]);
    t[57] = configured_mul((t[14]).wrapping_sub(t[15]), COSTAB[21]);
    t[69] = (t[33]).wrapping_add(t[34]);
    t[89] = configured_mul((t[33]).wrapping_sub(t[34]), COSTAB[3]);
    t[70] = (t[35]).wrapping_add(t[36]);
    t[90] = configured_mul((t[35]).wrapping_sub(t[36]), COSTAB[27]);
    t[71] = (t[37]).wrapping_add(t[38]);
    t[91] = configured_mul((t[37]).wrapping_sub(t[38]), COSTAB[11]);
    t[72] = (t[39]).wrapping_add(t[40]);
    t[92] = configured_mul((t[39]).wrapping_sub(t[40]), COSTAB[19]);
    t[73] = (t[41]).wrapping_add(t[42]);
    t[94] = configured_mul((t[41]).wrapping_sub(t[42]), COSTAB[3]);
    t[74] = (t[43]).wrapping_add(t[44]);
    t[95] = configured_mul((t[43]).wrapping_sub(t[44]), COSTAB[27]);
    t[75] = (t[45]).wrapping_add(t[46]);
    t[96] = configured_mul((t[45]).wrapping_sub(t[46]), COSTAB[11]);
    t[76] = (t[47]).wrapping_add(t[48]);
    t[97] = configured_mul((t[47]).wrapping_sub(t[48]), COSTAB[19]);
    t[78] = (t[50]).wrapping_add(t[51]);
    t[100] = configured_mul((t[50]).wrapping_sub(t[51]), COSTAB[3]);
    t[79] = (t[52]).wrapping_add(t[53]);
    t[101] = configured_mul((t[52]).wrapping_sub(t[53]), COSTAB[27]);
    t[80] = (t[54]).wrapping_add(t[55]);
    t[102] = configured_mul((t[54]).wrapping_sub(t[55]), COSTAB[11]);
    t[81] = (t[56]).wrapping_add(t[57]);
    t[103] = configured_mul((t[56]).wrapping_sub(t[57]), COSTAB[19]);
    t[83] = (t[59]).wrapping_add(t[60]);
    t[106] = configured_mul((t[59]).wrapping_sub(t[60]), COSTAB[3]);
    t[84] = (t[61]).wrapping_add(t[62]);
    t[107] = configured_mul((t[61]).wrapping_sub(t[62]), COSTAB[27]);
    t[85] = (t[63]).wrapping_add(t[64]);
    t[108] = configured_mul((t[63]).wrapping_sub(t[64]), COSTAB[11]);
    t[86] = (t[65]).wrapping_add(t[66]);
    t[109] = configured_mul((t[65]).wrapping_sub(t[66]), COSTAB[19]);
    t[113] = (t[69]).wrapping_add(t[70]);
    t[114] = (t[71]).wrapping_add(t[72]);
    out[0] = configured_dct_shift((t[113]).wrapping_add(t[114]));
    out[16] = configured_dct_shift(configured_mul((t[113]).wrapping_sub(t[114]), COSTAB[15]));
    t[115] = (t[73]).wrapping_add(t[74]);
    t[116] = (t[75]).wrapping_add(t[76]);
    t[32] = (t[115]).wrapping_add(t[116]);
    out[1] = configured_dct_shift(t[32]);
    t[118] = (t[78]).wrapping_add(t[79]);
    t[119] = (t[80]).wrapping_add(t[81]);
    t[58] = (t[118]).wrapping_add(t[119]);
    out[2] = configured_dct_shift(t[58]);
    t[121] = (t[83]).wrapping_add(t[84]);
    t[122] = (t[85]).wrapping_add(t[86]);
    t[67] = (t[121]).wrapping_add(t[122]);
    t[49] = ((t[67]).wrapping_mul(2)).wrapping_sub(t[32]);
    out[3] = configured_dct_shift(t[49]);
    t[125] = (t[89]).wrapping_add(t[90]);
    t[126] = (t[91]).wrapping_add(t[92]);
    t[93] = (t[125]).wrapping_add(t[126]);
    out[4] = configured_dct_shift(t[93]);
    t[128] = (t[94]).wrapping_add(t[95]);
    t[129] = (t[96]).wrapping_add(t[97]);
    t[98] = (t[128]).wrapping_add(t[129]);
    t[68] = ((t[98]).wrapping_mul(2)).wrapping_sub(t[49]);
    out[5] = configured_dct_shift(t[68]);
    t[132] = (t[100]).wrapping_add(t[101]);
    t[133] = (t[102]).wrapping_add(t[103]);
    t[104] = (t[132]).wrapping_add(t[133]);
    t[82] = ((t[104]).wrapping_mul(2)).wrapping_sub(t[58]);
    out[6] = configured_dct_shift(t[82]);
    t[136] = (t[106]).wrapping_add(t[107]);
    t[137] = (t[108]).wrapping_add(t[109]);
    t[110] = (t[136]).wrapping_add(t[137]);
    t[87] = ((t[110]).wrapping_mul(2)).wrapping_sub(t[67]);
    t[77] = ((t[87]).wrapping_mul(2)).wrapping_sub(t[68]);
    out[7] = configured_dct_shift(t[77]);
    t[141] = configured_mul((t[69]).wrapping_sub(t[70]), COSTAB[7]);
    t[142] = configured_mul((t[71]).wrapping_sub(t[72]), COSTAB[23]);
    t[143] = (t[141]).wrapping_add(t[142]);
    out[8] = configured_dct_shift(t[143]);
    out[24] = configured_dct_shift(
        ((configured_mul((t[141]).wrapping_sub(t[142]), COSTAB[15])).wrapping_mul(2))
            .wrapping_sub(t[143]),
    );
    t[144] = configured_mul((t[73]).wrapping_sub(t[74]), COSTAB[7]);
    t[145] = configured_mul((t[75]).wrapping_sub(t[76]), COSTAB[23]);
    t[146] = (t[144]).wrapping_add(t[145]);
    t[88] = ((t[146]).wrapping_mul(2)).wrapping_sub(t[77]);
    out[9] = configured_dct_shift(t[88]);
    t[148] = configured_mul((t[78]).wrapping_sub(t[79]), COSTAB[7]);
    t[149] = configured_mul((t[80]).wrapping_sub(t[81]), COSTAB[23]);
    t[150] = (t[148]).wrapping_add(t[149]);
    t[105] = ((t[150]).wrapping_mul(2)).wrapping_sub(t[82]);
    out[10] = configured_dct_shift(t[105]);
    t[152] = configured_mul((t[83]).wrapping_sub(t[84]), COSTAB[7]);
    t[153] = configured_mul((t[85]).wrapping_sub(t[86]), COSTAB[23]);
    t[154] = (t[152]).wrapping_add(t[153]);
    t[111] = ((t[154]).wrapping_mul(2)).wrapping_sub(t[87]);
    t[99] = ((t[111]).wrapping_mul(2)).wrapping_sub(t[88]);
    out[11] = configured_dct_shift(t[99]);
    t[157] = configured_mul((t[89]).wrapping_sub(t[90]), COSTAB[7]);
    t[158] = configured_mul((t[91]).wrapping_sub(t[92]), COSTAB[23]);
    t[159] = (t[157]).wrapping_add(t[158]);
    t[127] = ((t[159]).wrapping_mul(2)).wrapping_sub(t[93]);
    out[12] = configured_dct_shift(t[127]);
    t[160] = ((configured_mul((t[125]).wrapping_sub(t[126]), COSTAB[15])).wrapping_mul(2))
        .wrapping_sub(t[127]);
    out[20] = configured_dct_shift(t[160]);
    out[28] = configured_dct_shift(
        ((((configured_mul((t[157]).wrapping_sub(t[158]), COSTAB[15])).wrapping_mul(2))
            .wrapping_sub(t[159]))
        .wrapping_mul(2))
        .wrapping_sub(t[160]),
    );
    t[161] = configured_mul((t[94]).wrapping_sub(t[95]), COSTAB[7]);
    t[162] = configured_mul((t[96]).wrapping_sub(t[97]), COSTAB[23]);
    t[163] = (t[161]).wrapping_add(t[162]);
    t[130] = ((t[163]).wrapping_mul(2)).wrapping_sub(t[98]);
    t[112] = ((t[130]).wrapping_mul(2)).wrapping_sub(t[99]);
    out[13] = configured_dct_shift(t[112]);
    t[164] = ((configured_mul((t[128]).wrapping_sub(t[129]), COSTAB[15])).wrapping_mul(2))
        .wrapping_sub(t[130]);
    t[166] = configured_mul((t[100]).wrapping_sub(t[101]), COSTAB[7]);
    t[167] = configured_mul((t[102]).wrapping_sub(t[103]), COSTAB[23]);
    t[168] = (t[166]).wrapping_add(t[167]);
    t[134] = ((t[168]).wrapping_mul(2)).wrapping_sub(t[104]);
    t[120] = ((t[134]).wrapping_mul(2)).wrapping_sub(t[105]);
    out[14] = configured_dct_shift(t[120]);
    t[135] = ((configured_mul((t[118]).wrapping_sub(t[119]), COSTAB[15])).wrapping_mul(2))
        .wrapping_sub(t[120]);
    out[18] = configured_dct_shift(t[135]);
    t[169] = ((configured_mul((t[132]).wrapping_sub(t[133]), COSTAB[15])).wrapping_mul(2))
        .wrapping_sub(t[134]);
    t[151] = ((t[169]).wrapping_mul(2)).wrapping_sub(t[135]);
    out[22] = configured_dct_shift(t[151]);
    t[170] = ((((configured_mul((t[148]).wrapping_sub(t[149]), COSTAB[15])).wrapping_mul(2))
        .wrapping_sub(t[150]))
    .wrapping_mul(2))
    .wrapping_sub(t[151]);
    out[26] = configured_dct_shift(t[170]);
    out[30] = configured_dct_shift(
        ((((((configured_mul((t[166]).wrapping_sub(t[167]), COSTAB[15])).wrapping_mul(2))
            .wrapping_sub(t[168]))
        .wrapping_mul(2))
        .wrapping_sub(t[169]))
        .wrapping_mul(2))
        .wrapping_sub(t[170]),
    );
    t[171] = configured_mul((t[106]).wrapping_sub(t[107]), COSTAB[7]);
    t[172] = configured_mul((t[108]).wrapping_sub(t[109]), COSTAB[23]);
    t[173] = (t[171]).wrapping_add(t[172]);
    t[138] = ((t[173]).wrapping_mul(2)).wrapping_sub(t[110]);
    t[123] = ((t[138]).wrapping_mul(2)).wrapping_sub(t[111]);
    t[139] = ((configured_mul((t[121]).wrapping_sub(t[122]), COSTAB[15])).wrapping_mul(2))
        .wrapping_sub(t[123]);
    t[117] = ((t[123]).wrapping_mul(2)).wrapping_sub(t[112]);
    out[15] = configured_dct_shift(t[117]);
    t[124] = ((configured_mul((t[115]).wrapping_sub(t[116]), COSTAB[15])).wrapping_mul(2))
        .wrapping_sub(t[117]);
    out[17] = configured_dct_shift(t[124]);
    t[131] = ((t[139]).wrapping_mul(2)).wrapping_sub(t[124]);
    out[19] = configured_dct_shift(t[131]);
    t[140] = ((t[164]).wrapping_mul(2)).wrapping_sub(t[131]);
    out[21] = configured_dct_shift(t[140]);
    t[174] = ((configured_mul((t[136]).wrapping_sub(t[137]), COSTAB[15])).wrapping_mul(2))
        .wrapping_sub(t[138]);
    t[155] = ((t[174]).wrapping_mul(2)).wrapping_sub(t[139]);
    t[147] = ((t[155]).wrapping_mul(2)).wrapping_sub(t[140]);
    out[23] = configured_dct_shift(t[147]);
    t[156] = ((((configured_mul((t[144]).wrapping_sub(t[145]), COSTAB[15])).wrapping_mul(2))
        .wrapping_sub(t[146]))
    .wrapping_mul(2))
    .wrapping_sub(t[147]);
    out[25] = configured_dct_shift(t[156]);
    t[175] = ((((configured_mul((t[152]).wrapping_sub(t[153]), COSTAB[15])).wrapping_mul(2))
        .wrapping_sub(t[154]))
    .wrapping_mul(2))
    .wrapping_sub(t[155]);
    t[165] = ((t[175]).wrapping_mul(2)).wrapping_sub(t[156]);
    out[27] = configured_dct_shift(t[165]);
    t[176] = ((((((configured_mul((t[161]).wrapping_sub(t[162]), COSTAB[15])).wrapping_mul(2))
        .wrapping_sub(t[163]))
    .wrapping_mul(2))
    .wrapping_sub(t[164]))
    .wrapping_mul(2))
    .wrapping_sub(t[165]);
    out[29] = configured_dct_shift(t[176]);
    out[31] = configured_dct_shift(
        ((((((((configured_mul((t[171]).wrapping_sub(t[172]), COSTAB[15])).wrapping_mul(2))
            .wrapping_sub(t[173]))
        .wrapping_mul(2))
        .wrapping_sub(t[174]))
        .wrapping_mul(2))
        .wrapping_sub(t[175]))
        .wrapping_mul(2))
        .wrapping_sub(t[176]),
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::frame::{Header, Mode};

    #[test]
    fn silence_stays_silent_and_advances_phase() {
        let mut synth = Synth::new();
        let frame = Frame {
            header: Header {
                samplerate: 44_100,
                mode: Mode::Stereo,
                ..Header::default()
            },
            ..Frame::default()
        };
        let pcm = synth.frame(&frame).clone();
        assert_eq!(pcm.length, 1152);
        assert_eq!(pcm.channels, 2);
        assert!(pcm.samples.iter().flatten().all(|&sample| sample == 0));
        assert_eq!(synth.phase, 4);
    }

    #[test]
    fn half_rate_halves_sample_rate_and_output_length() {
        let mut synth = Synth::new();
        let frame = Frame {
            header: Header {
                samplerate: 48_000,
                ..Header::default()
            },
            options: OPTION_HALFSAMPLERATE,
            ..Frame::default()
        };
        let pcm = synth.frame(&frame);
        assert_eq!((pcm.samplerate, pcm.channels, pcm.length), (24_000, 1, 576));
    }
    #[test]
    fn half_rate_selects_every_second_full_rate_sample() {
        let mut input = Frame::default();
        input.header.samplerate = 32_000;
        input.sbsample[0][0][0] = crate::fixed::ONE / 2;
        let mut full_synth = Synth::new();
        let full = full_synth.frame(&input).clone();
        assert!(full.samples[0][..32].iter().any(|&sample| sample != 0));

        input.options = OPTION_HALFSAMPLERATE;
        let mut half_synth = Synth::new();
        let half = half_synth.frame(&input).clone();
        for i in 0..16 {
            assert_eq!(half.samples[0][i], full.samples[0][i * 2]);
        }
    }

    #[test]
    fn mirrored_polyphase_pairs_filter_values_in_c_order() {
        let sb = 3;
        for phase in [4, 5] {
            let mut filter = [[[[0; 8]; 16]; 2]; 2];
            let fo_bank = (!phase) & 1;
            for (index, value) in filter[1][fo_bank][sb - 1].iter_mut().enumerate() {
                *value = (index as i32 + 1) * (1 << 24);
            }
            let actual = polyphase(&filter, phase);
            let po = (phase.wrapping_sub(1) & 15) | 1;
            let expected = finish_synthesis_accumulator(dot8_mac(
                &filter[1][fo_bank][sb - 1],
                &synthesis_window()[sb],
                [
                    15 - po,
                    17 - po,
                    19 - po,
                    21 - po,
                    23 - po,
                    25 - po,
                    27 - po,
                    29 - po,
                ],
            ));
            assert_eq!(actual[32 - sb], expected);
        }
    }

    #[cfg(not(target_arch = "powerpc"))]
    #[test]
    fn fpm_default_multiply_uses_split_operand_rounding() {
        assert_eq!(
            configured_mul(crate::fixed::ONE, crate::fixed::ONE),
            crate::fixed::ONE
        );
        assert_eq!(
            configured_mul(-crate::fixed::ONE, crate::fixed::ONE),
            -crate::fixed::ONE
        );
        assert_eq!(configured_mul(1, crate::fixed::ONE), 0);
        assert_eq!(configured_mul(1_234_567, 2_345_678), 10_836);
        assert_eq!(configured_mul(-1_234_567, 2_345_678), -10_836);
    }

    #[cfg(not(target_arch = "powerpc"))]
    #[test]
    fn sso_window_is_preshifted_to_q14_with_c_rounding() {
        assert_eq!(D_SSO[0][8], 18_760);
        assert_eq!(D_SSO[0][7], -9_372);
        assert_eq!(D_SSO[0][1], -7);
        assert_eq!(D_SSO[3][1], -10);
    }

    #[cfg(not(target_arch = "powerpc"))]
    #[test]
    fn polyphase_sso_accumulator_wraps_in_32_bits_and_truncates() {
        assert_eq!(finish_synthesis_accumulator(3), 0);
        assert_eq!(finish_synthesis_accumulator(4), 1);
        assert_eq!(finish_synthesis_accumulator(u32::MAX - 3), -1);

        let values = [Fixed::MAX; 8];
        let indexes = [8; 8];
        let wrapped = dot8_mac(&values, &synthesis_window()[0], indexes);
        assert_eq!(wrapped, 0xfffd_b5c0);
        assert_eq!(finish_synthesis_accumulator(wrapped), -37_520);
    }

    #[cfg(not(target_arch = "powerpc"))]
    #[test]
    fn factorized_dct_stores_q16_shifted_samples() {
        let mut input = [1 << 20; 32];
        let output = dct32(&input);
        assert_eq!(output[0], 8_192);
        assert!(output[1..].iter().all(|&sample| sample == 0));

        input.fill(0);
        assert_eq!(dct32(&input), [0; 32]);
    }
}
