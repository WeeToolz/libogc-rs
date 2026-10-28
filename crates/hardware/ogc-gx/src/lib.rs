#![no_std]

extern crate alloc;

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

mod copy_scale;
mod fifo_rules;
mod register_state;
use copy_scale::{copy_y_scale, fits_framebuffer, num_xfb_lines, scale_enable_bit, y_scale_factor};
use fifo_rules::fifo_pointers;
use register_state::*;

use alloc::alloc::{Layout, alloc, dealloc};
use alloc::boxed::Box;
use alloc::vec::Vec;
use core::cell::UnsafeCell;
use core::ffi::{CStr, c_char, c_void};
use core::marker::PhantomData;
use core::ptr;
use core::sync::atomic::{AtomicBool, AtomicU32, Ordering, compiler_fence};
use ogc_cache::{dc_flush_range, dc_flush_range_no_sync};
pub use ogc_gu::{GuVector, Mtx, Mtx33, Mtx44};
pub use ogc_gx_sys::GxrModeObj;
use ogc_sysbase::DeviceFile;
#[cfg(not(gamecube))]
use ogc_system::{get_arena2_hi, get_arena2_lo, set_arena2_lo};

pub const GX_FALSE: u8 = 0;
pub const GX_TRUE: u8 = 1;
pub const GX_DISABLE: u8 = 0;
pub const GX_ENABLE: u8 = 1;
pub const GX_CLIP_DISABLE: u8 = 1;
pub const GX_CLIP_ENABLE: u8 = 0;
pub const GX_FIFO_MINSIZE: u32 = 64 * 1024;
pub const GX_FIFO_DEFAULT_SIZE: u32 = 256 * 1024;
pub const GX_FIFO_MAXSIZE: u32 = 1024 * 1024;
pub const GX_FIFO_HIWATERMARK: u32 = 16 * 1024;
pub const GX_FIFO_OBJSIZE: u32 = 128;
pub const GX_PERSPECTIVE: u8 = 0;
pub const GX_ORTHOGRAPHIC: u8 = 1;

pub const GX_FOG_NONE: u8 = 0;
pub const GX_FOG_PERSP_LIN: u8 = 2;
pub const GX_FOG_PERSP_EXP: u8 = 4;
pub const GX_FOG_PERSP_EXP2: u8 = 5;
pub const GX_FOG_PERSP_REVEXP: u8 = 6;
pub const GX_FOG_PERSP_REVEXP2: u8 = 7;
pub const GX_FOG_ORTHO_LIN: u8 = 10;
pub const GX_FOG_ORTHO_EXP: u8 = 12;
pub const GX_FOG_ORTHO_EXP2: u8 = 13;
pub const GX_FOG_ORTHO_REVEXP: u8 = 14;
pub const GX_FOG_ORTHO_REVEXP2: u8 = 15;
pub const GX_FOG_LIN: u8 = GX_FOG_PERSP_LIN;
pub const GX_FOG_EXP: u8 = GX_FOG_PERSP_EXP;
pub const GX_FOG_EXP2: u8 = GX_FOG_PERSP_EXP2;
pub const GX_FOG_REVEXP: u8 = GX_FOG_PERSP_REVEXP;
pub const GX_FOG_REVEXP2: u8 = GX_FOG_PERSP_REVEXP2;
// GX primitive opcodes occupy bits 3..6 of the byte written to the GP FIFO.
// The low three bits select the vertex format.
pub const GX_QUADS: u8 = 0x80;
pub const GX_TRIANGLES: u8 = 0x90;
pub const GX_TRIANGLESTRIP: u8 = 0x98;
pub const GX_TRIANGLEFAN: u8 = 0xA0;
pub const GX_LINES: u8 = 0xA8;
pub const GX_LINESTRIP: u8 = 0xB0;
pub const GX_POINTS: u8 = 0xB8;
pub const GX_VTXFMT0: u8 = 0;
pub const GX_VTXFMT1: u8 = 1;
pub const GX_VTXFMT2: u8 = 2;
pub const GX_VTXFMT3: u8 = 3;
pub const GX_VTXFMT4: u8 = 4;
pub const GX_VTXFMT5: u8 = 5;
pub const GX_VTXFMT6: u8 = 6;
pub const GX_VTXFMT7: u8 = 7;
pub const GX_MAXVTXFMT: u8 = 8;

pub const GX_NONE: u8 = 0;
pub const GX_DIRECT: u8 = 1;
pub const GX_INDEX8: u8 = 2;
pub const GX_INDEX16: u8 = 3;
pub const GX_U8: u32 = 0;
pub const GX_S8: u32 = 1;
pub const GX_U16: u32 = 2;
pub const GX_S16: u32 = 3;
pub const GX_F32: u32 = 4;
pub const GX_RGB565: u32 = 0;
pub const GX_RGB8: u32 = 1;
pub const GX_RGBX8: u32 = 2;
pub const GX_RGBA4: u32 = 3;
pub const GX_RGBA6: u32 = 4;
pub const GX_RGBA8: u32 = 5;
pub const GX_POS_XY: u32 = 0;
pub const GX_POS_XYZ: u32 = 1;
pub const GX_NRM_XYZ: u32 = 0;
pub const GX_NRM_NBT: u32 = 1;
pub const GX_NRM_NBT3: u32 = 2;
pub const GX_CLR_RGB: u32 = 0;
pub const GX_CLR_RGBA: u32 = 1;
pub const GX_TEX_S: u32 = 0;
pub const GX_TEX_ST: u32 = 1;
pub const GX_VA_PTNMTXIDX: u8 = 0;
pub const GX_VA_TEX0MTXIDX: u8 = 1;
pub const GX_VA_TEX1MTXIDX: u8 = 2;
pub const GX_VA_TEX2MTXIDX: u8 = 3;
pub const GX_VA_TEX3MTXIDX: u8 = 4;
pub const GX_VA_TEX4MTXIDX: u8 = 5;
pub const GX_VA_TEX5MTXIDX: u8 = 6;
pub const GX_VA_TEX6MTXIDX: u8 = 7;
pub const GX_VA_TEX7MTXIDX: u8 = 8;
pub const GX_VA_POS: u8 = 9;
pub const GX_VA_NRM: u8 = 10;
pub const GX_VA_CLR0: u8 = 11;
pub const GX_VA_CLR1: u8 = 12;
pub const GX_VA_TEX0: u8 = 13;
pub const GX_VA_TEX1: u8 = 14;
pub const GX_VA_TEX2: u8 = 15;
pub const GX_VA_TEX3: u8 = 16;
pub const GX_VA_TEX4: u8 = 17;
pub const GX_VA_TEX5: u8 = 18;
pub const GX_VA_TEX6: u8 = 19;
pub const GX_VA_TEX7: u8 = 20;
pub const GX_POSMTXARRAY: u8 = 21;
pub const GX_NRMMTXARRAY: u8 = 22;
pub const GX_TEXMTXARRAY: u8 = 23;
pub const GX_LIGHTARRAY: u8 = 24;
pub const GX_VA_NBT: u8 = 25;
pub const GX_VA_MAXATTR: u8 = 26;
pub const GX_VA_NULL: u8 = 0xff;
pub const GX_MAX_VTXDESC_LISTSIZE: usize = GX_VA_MAXATTR as usize + 1;
pub const GX_MAX_VTXATTRFMT_LISTSIZE: usize = GX_VA_MAXATTR as usize + 1;
pub const GX_MT_NULL: u8 = 0;
pub const GX_MT_XF_FLUSH: u8 = 1;
pub const GX_MT_DL_SAVE_CTX: u8 = 2;

pub const GX_PERF0_VERTICES: u32 = 0;
pub const GX_PERF0_CLIP_VTX: u32 = 1;
pub const GX_PERF0_CLIP_CLKS: u32 = 2;
pub const GX_PERF0_XF_WAIT_IN: u32 = 3;
pub const GX_PERF0_XF_WAIT_OUT: u32 = 4;
pub const GX_PERF0_XF_XFRM_CLKS: u32 = 5;
pub const GX_PERF0_XF_LIT_CLKS: u32 = 6;
pub const GX_PERF0_XF_BOT_CLKS: u32 = 7;
pub const GX_PERF0_XF_REGLD_CLKS: u32 = 8;
pub const GX_PERF0_XF_REGRD_CLKS: u32 = 9;
pub const GX_PERF0_CLIP_RATIO: u32 = 10;
pub const GX_PERF0_TRIANGLES: u32 = 11;
pub const GX_PERF0_TRIANGLES_CULLED: u32 = 12;
pub const GX_PERF0_TRIANGLES_PASSED: u32 = 13;
pub const GX_PERF0_TRIANGLES_SCISSORED: u32 = 14;
pub const GX_PERF0_TRIANGLES_0TEX: u32 = 15;
pub const GX_PERF0_TRIANGLES_1TEX: u32 = 16;
pub const GX_PERF0_TRIANGLES_2TEX: u32 = 17;
pub const GX_PERF0_TRIANGLES_3TEX: u32 = 18;
pub const GX_PERF0_TRIANGLES_4TEX: u32 = 19;
pub const GX_PERF0_TRIANGLES_5TEX: u32 = 20;
pub const GX_PERF0_TRIANGLES_6TEX: u32 = 21;
pub const GX_PERF0_TRIANGLES_7TEX: u32 = 22;
pub const GX_PERF0_TRIANGLES_8TEX: u32 = 23;
pub const GX_PERF0_TRIANGLES_0CLR: u32 = 24;
pub const GX_PERF0_TRIANGLES_1CLR: u32 = 25;
pub const GX_PERF0_TRIANGLES_2CLR: u32 = 26;
pub const GX_PERF0_QUAD_0CVG: u32 = 27;
pub const GX_PERF0_QUAD_NON0CVG: u32 = 28;
pub const GX_PERF0_QUAD_1CVG: u32 = 29;
pub const GX_PERF0_QUAD_2CVG: u32 = 30;
pub const GX_PERF0_QUAD_3CVG: u32 = 31;
pub const GX_PERF0_QUAD_4CVG: u32 = 32;
pub const GX_PERF0_AVG_QUAD_CNT: u32 = 33;
pub const GX_PERF0_CLOCKS: u32 = 34;
pub const GX_PERF0_NONE: u32 = 35;
pub const GX_PERF1_TEXELS: u32 = 0;
pub const GX_PERF1_TX_IDLE: u32 = 1;
pub const GX_PERF1_TX_REGS: u32 = 2;
pub const GX_PERF1_TX_MEMSTALL: u32 = 3;
pub const GX_PERF1_TC_CHECK1_2: u32 = 4;
pub const GX_PERF1_TC_CHECK3_4: u32 = 5;
pub const GX_PERF1_TC_CHECK5_6: u32 = 6;
pub const GX_PERF1_TC_CHECK7_8: u32 = 7;
pub const GX_PERF1_TC_MISS: u32 = 8;
pub const GX_PERF1_VC_ELEMQ_FULL: u32 = 9;
pub const GX_PERF1_VC_MISSQ_FULL: u32 = 10;
pub const GX_PERF1_VC_MEMREQ_FULL: u32 = 11;
pub const GX_PERF1_VC_STATUS7: u32 = 12;
pub const GX_PERF1_VC_MISSREP_FULL: u32 = 13;
pub const GX_PERF1_VC_STREAMBUF_LOW: u32 = 14;
pub const GX_PERF1_VC_ALL_STALLS: u32 = 15;
pub const GX_PERF1_VERTICES: u32 = 16;
pub const GX_PERF1_FIFO_REQ: u32 = 17;
pub const GX_PERF1_CALL_REQ: u32 = 18;
pub const GX_PERF1_VC_MISS_REQ: u32 = 19;
pub const GX_PERF1_CP_ALL_REQ: u32 = 20;
pub const GX_PERF1_CLOCKS: u32 = 21;
pub const GX_PERF1_NONE: u32 = 22;
pub const GX_XF_FLUSH_NONE: u8 = 0;
pub const GX_XF_FLUSH_SAFE: u8 = 1;
pub const GX_PNMTX0: u32 = 0;
pub const GX_IDENTITY: u32 = 60;
pub const GX_TO_ZERO: u8 = 0;

pub const GX_MTX3X4: u8 = 0;
pub const GX_MTX2X4: u8 = 1;
pub const GX_NEAR: u8 = 0;
pub const GX_LINEAR: u8 = 1;
pub const GX_NEAR_MIP_NEAR: u8 = 2;
pub const GX_LIN_MIP_NEAR: u8 = 3;
pub const GX_NEAR_MIP_LIN: u8 = 4;
pub const GX_LIN_MIP_LIN: u8 = 5;
pub const GX_ANISO_1: u8 = 0;
pub const GX_ANISO_2: u8 = 1;
pub const GX_ANISO_4: u8 = 2;
pub const GX_MAX_ANISOTROPY: u8 = 3;
pub const GX_TEXCACHE_32K: u8 = 0;
pub const GX_TEXCACHE_128K: u8 = 1;
pub const GX_TEXCACHE_512K: u8 = 2;
pub const GX_TEXCACHE_NONE: u8 = 3;
pub const GX_TF_I4: u8 = 0x0;
pub const GX_TF_I8: u8 = 0x1;
pub const GX_TF_IA4: u8 = 0x2;
pub const GX_TF_IA8: u8 = 0x3;
pub const GX_TF_RGB565: u8 = 0x4;
pub const GX_TF_RGB5A3: u8 = 0x5;
pub const GX_TF_RGBA8: u8 = 0x6;
pub const GX_TF_CI4: u8 = 0x8;
pub const GX_TF_CI8: u8 = 0x9;
pub const GX_TF_CI14: u8 = 0xA;
pub const GX_TF_CMPR: u8 = 0xE;
pub const GX_TF_A8: u8 = 0x27;
pub const GX_TL_IA8: u8 = 0x00;
pub const GX_TL_RGB565: u8 = 0x01;
pub const GX_TL_RGB5A3: u8 = 0x02;
pub const GX_CTF_R4: u8 = 0x20;
pub const GX_CTF_RA4: u8 = 0x22;
pub const GX_CTF_RA8: u8 = 0x23;
pub const GX_CTF_YUVA8: u8 = 0x26;
pub const GX_CTF_A8: u8 = 0x27;
pub const GX_CTF_R8: u8 = 0x28;
pub const GX_CTF_G8: u8 = 0x29;
pub const GX_CTF_B8: u8 = 0x2A;
pub const GX_CTF_RG8: u8 = 0x2B;
pub const GX_CTF_GB8: u8 = 0x2C;
pub const GX_TF_Z8: u8 = 0x11;
pub const GX_TF_Z16: u8 = 0x13;
pub const GX_TF_Z24X8: u8 = 0x16;
pub const GX_CTF_Z4: u8 = 0x30;
pub const GX_CTF_Z8M: u8 = 0x39;
pub const GX_CTF_Z8L: u8 = 0x3A;
pub const GX_CTF_Z16L: u8 = 0x3C;
pub const GX_TLUT_16: u16 = 1;
pub const GX_TLUT_32: u16 = 2;
pub const GX_TLUT_64: u16 = 4;
pub const GX_TLUT_128: u16 = 8;
pub const GX_TLUT_256: u16 = 16;
pub const GX_TLUT_512: u16 = 32;
pub const GX_TLUT_1024: u16 = 64;
pub const GX_TLUT_2048: u16 = 128;
pub const GX_LIGHT0: u16 = 0x001;
pub const GX_LIGHT1: u16 = 0x002;
pub const GX_LIGHT2: u16 = 0x004;
pub const GX_LIGHT3: u16 = 0x008;
pub const GX_LIGHT4: u16 = 0x010;
pub const GX_LIGHT5: u16 = 0x020;
pub const GX_LIGHT6: u16 = 0x040;
pub const GX_LIGHT7: u16 = 0x080;
pub const GX_MAXLIGHT: u16 = 0x100;
pub const GX_LIGHTNULL: u16 = 0x000;

pub const GX_NEVER: u8 = 0;
pub const GX_LESS: u8 = 1;
pub const GX_EQUAL: u8 = 2;
pub const GX_LEQUAL: u8 = 3;
pub const GX_GREATER: u8 = 4;
pub const GX_NEQUAL: u8 = 5;
pub const GX_GEQUAL: u8 = 6;
pub const GX_ALWAYS: u8 = 7;

pub const GX_READ_00: u8 = 0;
pub const GX_READ_FF: u8 = 1;
pub const GX_READ_NONE: u8 = 2;

pub const GX_BM_NONE: u8 = 0;
pub const GX_BM_BLEND: u8 = 1;
pub const GX_BM_LOGIC: u8 = 2;
pub const GX_BM_SUBTRACT: u8 = 3;

pub const GX_BL_ZERO: u8 = 0;
pub const GX_BL_ONE: u8 = 1;
pub const GX_BL_SRCCLR: u8 = 2;
pub const GX_BL_INVSRCCLR: u8 = 3;
pub const GX_BL_SRCALPHA: u8 = 4;
pub const GX_BL_INVSRCALPHA: u8 = 5;
pub const GX_BL_DSTALPHA: u8 = 6;
pub const GX_BL_INVDSTALPHA: u8 = 7;
pub const GX_BL_DSTCLR: u8 = GX_BL_SRCCLR;
pub const GX_BL_INVDSTCLR: u8 = GX_BL_INVSRCCLR;

pub const GX_LO_CLEAR: u8 = 0;
pub const GX_LO_AND: u8 = 1;
pub const GX_LO_REVAND: u8 = 2;
pub const GX_LO_COPY: u8 = 3;
pub const GX_LO_INVAND: u8 = 4;
pub const GX_LO_NOOP: u8 = 5;
pub const GX_LO_XOR: u8 = 6;
pub const GX_LO_OR: u8 = 7;
pub const GX_LO_NOR: u8 = 8;
pub const GX_LO_EQUIV: u8 = 9;
pub const GX_LO_INV: u8 = 10;
pub const GX_LO_REVOR: u8 = 11;
pub const GX_LO_INVCOPY: u8 = 12;
pub const GX_LO_INVOR: u8 = 13;
pub const GX_LO_NAND: u8 = 14;
pub const GX_LO_SET: u8 = 15;

pub const GX_CULL_NONE: u8 = 0;
pub const GX_CULL_FRONT: u8 = 1;
pub const GX_CULL_BACK: u8 = 2;
pub const GX_CULL_ALL: u8 = 3;

pub const GX_PF_RGB8_Z24: u8 = 0;
pub const GX_PF_RGBA6_Z24: u8 = 1;
pub const GX_PF_RGB565_Z16: u8 = 2;
pub const GX_PF_Z24: u8 = 3;
pub const GX_PF_Y8: u8 = 4;
pub const GX_PF_U8: u8 = 5;
pub const GX_PF_V8: u8 = 6;
pub const GX_PF_YUV420: u8 = 7;

pub const GX_CLAMP_NONE: u8 = 0;
pub const GX_CLAMP_TOP: u8 = 1;
pub const GX_CLAMP_BOTTOM: u8 = 2;

pub const GX_GM_1_0: u8 = 0;
pub const GX_GM_1_7: u8 = 1;
pub const GX_GM_2_2: u8 = 2;

pub const GX_COPY_PROGRESSIVE: u8 = 0;
pub const GX_COPY_INTLC_EVEN: u8 = 2;
pub const GX_COPY_INTLC_ODD: u8 = 3;

pub const GX_ZC_LINEAR: u8 = 0;
pub const GX_ZC_NEAR: u8 = 1;
pub const GX_ZC_MID: u8 = 2;
pub const GX_ZC_FAR: u8 = 3;

pub const GX_SRC_REG: u8 = 0;
pub const GX_SRC_VTX: u8 = 1;

pub const GX_DF_NONE: u8 = 0;
pub const GX_DF_SIGNED: u8 = 1;
pub const GX_DF_CLAMP: u8 = 2;

pub const GX_AF_SPEC: u8 = 0;
pub const GX_AF_SPOT: u8 = 1;
pub const GX_AF_NONE: u8 = 2;

pub const GX_COLOR0: u8 = 0;
pub const GX_COLOR1: u8 = 1;
pub const GX_ALPHA0: u8 = 2;
pub const GX_ALPHA1: u8 = 3;
pub const GX_COLOR0A0: u8 = 4;
pub const GX_COLOR1A1: u8 = 5;
pub const GX_COLORZERO: u8 = 6;
pub const GX_ALPHA_BUMP: u8 = 7;
pub const GX_ALPHA_BUMPN: u8 = 8;
pub const GX_COLORNULL: u8 = 0xff;

pub const GX_MODULATE: u8 = 0;
pub const GX_DECAL: u8 = 1;
pub const GX_BLEND: u8 = 2;
pub const GX_REPLACE: u8 = 3;
pub const GX_PASSCLR: u8 = 4;

pub const GX_CC_CPREV: u8 = 0;
pub const GX_CC_APREV: u8 = 1;
pub const GX_CC_C0: u8 = 2;
pub const GX_CC_A0: u8 = 3;
pub const GX_CC_C1: u8 = 4;
pub const GX_CC_A1: u8 = 5;
pub const GX_CC_C2: u8 = 6;
pub const GX_CC_A2: u8 = 7;
pub const GX_CC_TEXC: u8 = 8;
pub const GX_CC_TEXA: u8 = 9;
pub const GX_CC_RASC: u8 = 10;
pub const GX_CC_RASA: u8 = 11;
pub const GX_CC_ONE: u8 = 12;
pub const GX_CC_HALF: u8 = 13;
pub const GX_CC_KONST: u8 = 14;
pub const GX_CC_ZERO: u8 = 15;

pub const GX_CA_APREV: u8 = 0;
pub const GX_CA_A0: u8 = 1;
pub const GX_CA_A1: u8 = 2;
pub const GX_CA_A2: u8 = 3;
pub const GX_CA_TEXA: u8 = 4;
pub const GX_CA_RASA: u8 = 5;
pub const GX_CA_KONST: u8 = 6;
pub const GX_CA_ZERO: u8 = 7;

pub const GX_TEVSTAGE0: u8 = 0;
pub const GX_TEVSTAGE1: u8 = 1;
pub const GX_TEVSTAGE2: u8 = 2;
pub const GX_TEVSTAGE3: u8 = 3;
pub const GX_TEVSTAGE4: u8 = 4;
pub const GX_TEVSTAGE5: u8 = 5;
pub const GX_TEVSTAGE6: u8 = 6;
pub const GX_TEVSTAGE7: u8 = 7;
pub const GX_TEVSTAGE8: u8 = 8;
pub const GX_TEVSTAGE9: u8 = 9;
pub const GX_TEVSTAGE10: u8 = 10;
pub const GX_TEVSTAGE11: u8 = 11;
pub const GX_TEVSTAGE12: u8 = 12;
pub const GX_TEVSTAGE13: u8 = 13;
pub const GX_TEVSTAGE14: u8 = 14;
pub const GX_TEVSTAGE15: u8 = 15;
pub const GX_MAX_TEVSTAGE: u8 = 16;

pub const GX_TEV_ADD: u8 = 0;
pub const GX_TEV_SUB: u8 = 1;
pub const GX_TEV_COMP_R8_GT: u8 = 8;
pub const GX_TEV_COMP_R8_EQ: u8 = 9;
pub const GX_TEV_COMP_GR16_GT: u8 = 10;
pub const GX_TEV_COMP_GR16_EQ: u8 = 11;
pub const GX_TEV_COMP_BGR24_GT: u8 = 12;
pub const GX_TEV_COMP_BGR24_EQ: u8 = 13;
pub const GX_TEV_COMP_RGB8_GT: u8 = 14;
pub const GX_TEV_COMP_RGB8_EQ: u8 = 15;
pub const GX_TEV_COMP_A8_GT: u8 = GX_TEV_COMP_RGB8_GT;
pub const GX_TEV_COMP_A8_EQ: u8 = GX_TEV_COMP_RGB8_EQ;

pub const GX_TB_ZERO: u8 = 0;
pub const GX_TB_ADDHALF: u8 = 1;
pub const GX_TB_SUBHALF: u8 = 2;
pub const GX_CS_SCALE_1: u8 = 0;
pub const GX_CS_SCALE_2: u8 = 1;
pub const GX_CS_SCALE_4: u8 = 2;
pub const GX_CS_DIVIDE_2: u8 = 3;
pub const GX_TEVPREV: u8 = 0;
pub const GX_TEVREG0: u8 = 1;
pub const GX_TEVREG1: u8 = 2;
pub const GX_TEVREG2: u8 = 3;

pub const GX_TEXMAP0: u32 = 0;
pub const GX_TEXMAP1: u32 = 1;
pub const GX_TEXMAP2: u32 = 2;
pub const GX_TEXMAP3: u32 = 3;
pub const GX_TEXMAP4: u32 = 4;
pub const GX_TEXMAP5: u32 = 5;
pub const GX_TEXMAP6: u32 = 6;
pub const GX_TEXMAP7: u32 = 7;
pub const GX_MAX_TEXMAP: u32 = 8;
pub const GX_TEXMAP_NULL: u32 = 0xff;
pub const GX_TEXMAP_DISABLE: u32 = 0x100;

pub const GX_TEV_SWAP0: u8 = 0;
pub const GX_TEV_SWAP1: u8 = 1;
pub const GX_TEV_SWAP2: u8 = 2;
pub const GX_TEV_SWAP3: u8 = 3;
pub const GX_CH_RED: u8 = 0;
pub const GX_CH_GREEN: u8 = 1;
pub const GX_CH_BLUE: u8 = 2;
pub const GX_CH_ALPHA: u8 = 3;

pub const GX_TEXCOORD0: u16 = 0;
pub const GX_TEXCOORD1: u16 = 1;
pub const GX_TEXCOORD2: u16 = 2;
pub const GX_TEXCOORD3: u16 = 3;
pub const GX_TEXCOORD4: u16 = 4;
pub const GX_TEXCOORD5: u16 = 5;
pub const GX_TEXCOORD6: u16 = 6;
pub const GX_TEXCOORD7: u16 = 7;
pub const GX_MAXCOORD: u16 = 8;
pub const GX_TEXCOORDNULL: u16 = 0xff;

pub const GX_TG_MTX3X4: u32 = 0;
pub const GX_TG_MTX2X4: u32 = 1;
pub const GX_TG_BUMP0: u32 = 2;
pub const GX_TG_BUMP1: u32 = 3;
pub const GX_TG_BUMP2: u32 = 4;
pub const GX_TG_BUMP3: u32 = 5;
pub const GX_TG_BUMP4: u32 = 6;
pub const GX_TG_BUMP5: u32 = 7;
pub const GX_TG_BUMP6: u32 = 8;
pub const GX_TG_BUMP7: u32 = 9;
pub const GX_TG_SRTG: u32 = 10;
pub const GX_TG_POS: u32 = 0;
pub const GX_TG_NRM: u32 = 1;
pub const GX_TG_BINRM: u32 = 2;
pub const GX_TG_TANGENT: u32 = 3;
pub const GX_TG_TEX0: u32 = 4;
pub const GX_TG_TEX1: u32 = 5;
pub const GX_TG_TEX2: u32 = 6;
pub const GX_TG_TEX3: u32 = 7;
pub const GX_TG_TEX4: u32 = 8;
pub const GX_TG_TEX5: u32 = 9;
pub const GX_TG_TEX6: u32 = 10;
pub const GX_TG_TEX7: u32 = 11;
pub const GX_TG_TEXCOORD0: u32 = 12;
pub const GX_TG_TEXCOORD1: u32 = 13;
pub const GX_TG_TEXCOORD2: u32 = 14;
pub const GX_TG_TEXCOORD3: u32 = 15;
pub const GX_TG_TEXCOORD4: u32 = 16;
pub const GX_TG_TEXCOORD5: u32 = 17;
pub const GX_TG_TEXCOORD6: u32 = 18;
pub const GX_TG_COLOR0: u32 = 19;
pub const GX_TG_COLOR1: u32 = 20;

pub const GX_DTTMTX0: u32 = 64;
pub const GX_DTTIDENTITY: u32 = 125;
pub const GX_ZT_DISABLE: u8 = 0;
pub const GX_ZT_ADD: u8 = 1;
pub const GX_ZT_REPLACE: u8 = 2;

pub const GX_INDTEXSTAGE0: u8 = 0;
pub const GX_INDTEXSTAGE1: u8 = 1;
pub const GX_INDTEXSTAGE2: u8 = 2;
pub const GX_INDTEXSTAGE3: u8 = 3;
pub const GX_ITF_8: u8 = 0;
pub const GX_ITF_5: u8 = 1;
pub const GX_ITF_4: u8 = 2;
pub const GX_ITF_3: u8 = 3;
pub const GX_ITB_NONE: u8 = 0;
pub const GX_ITB_S: u8 = 1;
pub const GX_ITB_T: u8 = 2;
pub const GX_ITB_ST: u8 = 3;
pub const GX_ITB_U: u8 = 4;
pub const GX_ITB_SU: u8 = 5;
pub const GX_ITB_TU: u8 = 6;
pub const GX_ITB_STU: u8 = 7;
pub const GX_ITM_OFF: u8 = 0;
pub const GX_ITM_0: u8 = 1;
pub const GX_ITM_1: u8 = 2;
pub const GX_ITM_2: u8 = 3;
pub const GX_ITM_S0: u8 = 5;
pub const GX_ITM_S1: u8 = 6;
pub const GX_ITM_S2: u8 = 7;
pub const GX_ITM_T0: u8 = 9;
pub const GX_ITM_T1: u8 = 10;
pub const GX_ITM_T2: u8 = 11;
pub const GX_ITW_OFF: u8 = 0;
pub const GX_ITW_256: u8 = 1;
pub const GX_ITW_128: u8 = 2;
pub const GX_ITW_64: u8 = 3;
pub const GX_ITW_32: u8 = 4;
pub const GX_ITW_16: u8 = 5;
pub const GX_ITW_0: u8 = 6;
pub const GX_ITBA_OFF: u8 = 0;
pub const GX_ITBA_S: u8 = 1;
pub const GX_ITBA_T: u8 = 2;
pub const GX_ITBA_U: u8 = 3;
pub const GX_ITS_1: u8 = 0;
pub const GX_ITS_2: u8 = 1;
pub const GX_ITS_4: u8 = 2;
pub const GX_ITS_8: u8 = 3;
pub const GX_ITS_16: u8 = 4;
pub const GX_ITS_32: u8 = 5;
pub const GX_ITS_64: u8 = 6;
pub const GX_ITS_128: u8 = 7;
pub const GX_ITS_256: u8 = 8;

pub const GX_DA_OFF: u8 = 0;
pub const GX_DA_GENTLE: u8 = 1;
pub const GX_DA_MEDIUM: u8 = 2;
pub const GX_DA_STEEP: u8 = 3;

pub const GX_SP_OFF: u8 = 0;
pub const GX_SP_FLAT: u8 = 1;
pub const GX_SP_COS: u8 = 2;
pub const GX_SP_COS2: u8 = 3;
pub const GX_SP_SHARP: u8 = 4;
pub const GX_SP_RING1: u8 = 5;
pub const GX_SP_RING2: u8 = 6;

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq)]
pub struct GxColor {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq)]
pub struct GxColorS10 {
    pub r: i16,
    pub g: i16,
    pub b: i16,
    pub a: i16,
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq)]
pub struct GxVtxDesc {
    pub attr: u8,
    pub ty: u8,
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq)]
pub struct GxVtxAttrFmt {
    pub vtxattr: u32,
    pub comptype: u32,
    pub compsize: u32,
    pub frac: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Default, Debug, PartialEq, Eq)]
pub struct GxFogAdjTbl {
    pub r: [u16; 10],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct GxFifoObj {
    pub pad: [u8; GX_FIFO_OBJSIZE as usize],
}

impl Default for GxFifoObj {
    fn default() -> Self {
        Self {
            pad: [0; GX_FIFO_OBJSIZE as usize],
        }
    }
}

#[derive(Copy, Clone, Default, Debug, Eq, PartialEq)]
struct FifoSnapshot {
    buf_start: u32,
    buf_end: u32,
    size: u32,
    hi_mark: u32,
    lo_mark: u32,
    rd_ptr: u32,
    wt_ptr: u32,
    rdwt_dst: u32,
    fifo_wrap: u8,
    cpufifo_ready: u8,
    gpfifo_ready: u8,
}

struct FifoAtomicState {
    buf_start: AtomicU32,
    buf_end: AtomicU32,
    size: AtomicU32,
    hi_mark: AtomicU32,
    lo_mark: AtomicU32,
    rd_ptr: AtomicU32,
    wt_ptr: AtomicU32,
    rdwt_dst: AtomicU32,
    flags: AtomicU32,
}

impl FifoAtomicState {
    const fn new() -> Self {
        Self {
            buf_start: AtomicU32::new(0),
            buf_end: AtomicU32::new(0),
            size: AtomicU32::new(0),
            hi_mark: AtomicU32::new(0),
            lo_mark: AtomicU32::new(0),
            rd_ptr: AtomicU32::new(0),
            wt_ptr: AtomicU32::new(0),
            rdwt_dst: AtomicU32::new(0),
            flags: AtomicU32::new(0),
        }
    }
    fn store(&self, s: FifoSnapshot) {
        self.buf_start.store(s.buf_start, Ordering::Relaxed);
        self.buf_end.store(s.buf_end, Ordering::Relaxed);
        self.size.store(s.size, Ordering::Relaxed);
        self.hi_mark.store(s.hi_mark, Ordering::Relaxed);
        self.lo_mark.store(s.lo_mark, Ordering::Relaxed);
        self.rd_ptr.store(s.rd_ptr, Ordering::Relaxed);
        self.wt_ptr.store(s.wt_ptr, Ordering::Relaxed);
        self.rdwt_dst.store(s.rdwt_dst, Ordering::Relaxed);
        self.flags.store(
            u32::from(s.fifo_wrap)
                | (u32::from(s.cpufifo_ready) << 8)
                | (u32::from(s.gpfifo_ready) << 16),
            Ordering::Release,
        );
    }
    fn load(&self) -> FifoSnapshot {
        let f = self.flags.load(Ordering::Acquire);
        FifoSnapshot {
            buf_start: self.buf_start.load(Ordering::Relaxed),
            buf_end: self.buf_end.load(Ordering::Relaxed),
            size: self.size.load(Ordering::Relaxed),
            hi_mark: self.hi_mark.load(Ordering::Relaxed),
            lo_mark: self.lo_mark.load(Ordering::Relaxed),
            rd_ptr: self.rd_ptr.load(Ordering::Relaxed),
            wt_ptr: self.wt_ptr.load(Ordering::Relaxed),
            rdwt_dst: self.rdwt_dst.load(Ordering::Relaxed),
            fifo_wrap: (f & 0xff) as u8,
            cpufifo_ready: ((f >> 8) & 0xff) as u8,
            gpfifo_ready: ((f >> 16) & 0xff) as u8,
        }
    }
}

fn fifo_snapshot(fifo: *const GxFifoObj) -> Option<FifoSnapshot> {
    if fifo.is_null() {
        return None;
    }
    let p = fifo.cast::<u8>();
    let read32 = |off: usize| unsafe { ptr::read_unaligned(p.add(off).cast::<u32>()) };
    Some(FifoSnapshot {
        buf_start: read32(0),
        buf_end: read32(4),
        size: read32(8),
        hi_mark: read32(12),
        lo_mark: read32(16),
        rd_ptr: read32(20),
        wt_ptr: read32(24),
        rdwt_dst: read32(28),
        fifo_wrap: unsafe { p.add(32).read() },
        cpufifo_ready: unsafe { p.add(33).read() },
        gpfifo_ready: unsafe { p.add(34).read() },
    })
}

unsafe fn write_fifo_snapshot(fifo: *mut GxFifoObj, s: FifoSnapshot) {
    if fifo.is_null() {
        return;
    }
    let p = fifo.cast::<u8>();
    for (off, value) in [
        (0, s.buf_start),
        (4, s.buf_end),
        (8, s.size),
        (12, s.hi_mark),
        (16, s.lo_mark),
        (20, s.rd_ptr),
        (24, s.wt_ptr),
        (28, s.rdwt_dst),
    ] {
        unsafe { ptr::write_unaligned(p.add(off).cast::<u32>(), value) };
    }
    unsafe {
        p.add(32).write(s.fifo_wrap);
        p.add(33).write(s.cpufifo_ready);
        p.add(34).write(s.gpfifo_ready);
    }
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct GxTexObj {
    pub val: [u32; 8],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct GxTlutObj {
    pub val: [u32; 3],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct GxTexRegion {
    pub val: [u32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct GxTlutRegion {
    pub val: [u32; 4],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct GxLightObj {
    pub val: [u32; 16],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq)]
pub struct Vtx {
    pub x: f32,
    pub y: f32,
    pub z: f32,
    pub s: u16,
    pub t: u16,
    pub rgba: u32,
}

#[repr(C)]
#[derive(Copy, Clone)]
pub union WGPipe {
    pub u8_: u8,
    pub s8_: i8,
    pub u16_: u16,
    pub s16_: i16,
    pub u32_: u32,
    pub s32_: i32,
    pub f32_: f32,
}

pub type GxBreakPtCallback = Option<unsafe extern "C" fn()>;
pub type GxDrawDoneCallback = Option<unsafe extern "C" fn()>;
pub type GxDrawSyncCallback = Option<unsafe extern "C" fn(token: u16)>;
pub type GxTexRegionCallback =
    Option<unsafe extern "C" fn(obj: *mut GxTexObj, mapid: u8) -> *mut GxTexRegion>;
pub type GxTlutRegionCallback = Option<unsafe extern "C" fn(tlut_name: u32) -> *mut GxTlutRegion>;

static mut TEX_REGION_CB: GxTexRegionCallback = None;
static mut TLUT_REGION_CB: GxTlutRegionCallback = None;
static mut GX_DEFAULT_TEX_REGIONS: [GxTexRegion; 24] = [GxTexRegion { val: [0; 4] }; 24];
static mut GX_DEFAULT_TLUT_REGIONS: [GxTlutRegion; 20] = [GxTlutRegion { val: [0; 4] }; 20];
static GX_GC_TEX_REGION_A: AtomicU32 = AtomicU32::new(0);
static GX_GC_TEX_REGION_B: AtomicU32 = AtomicU32::new(0);

const GP_PIPE_ADDR: usize = 0xCC00_8000;

const GX_CMD_LOAD_XF_REG: u8 = 0x10;
const GX_CMD_LOAD_BP_REG: u8 = 0x61;
const GX_XF_VIEWPORT: u16 = 0x101a;
const GX_XF_PROJECTION: u16 = 0x1020;
const GX_BP_SCISSOR_TL: u8 = 0x20;
const GX_BP_SCISSOR_BR: u8 = 0x21;
const GX_BP_SCISSOR_OFFSET: u8 = 0x59;
const GX_BP_Z_MODE: u8 = 0x40;
const GX_BP_COLOR_MODE: u8 = 0x41;
// TEV order registers start at BP register 0x28. The C GX implementation's
// `3 + stage / 2` slot mapping assigns stage 0 to this register.
const GX_BP_TEV_ORDER: u8 = 0x28;
const GX_BP_PIXEL_ENGINE: u8 = 0x43;
const GX_BP_FIELD_MASK: u8 = 0x44;
const GX_BP_COPY_SRC: u8 = 0x49;
const GX_BP_COPY_SIZE: u8 = 0x4a;
const GX_BP_COPY_DEST: u8 = 0x4d;
const GX_BP_COPY_Y_SCALE: u8 = 0x4e;
const GX_BP_COPY_CLEAR_AR: u8 = 0x4f;
const GX_BP_COPY_CLEAR_GB: u8 = 0x50;
const GX_BP_COPY_CLEAR_Z: u8 = 0x51;
const GX_BP_COPY_CONTROL: u8 = 0x52;
const GX_BP_DRAW_DONE: u8 = 0x45;
const GX_RGBA8_TMEM_EVEN: [u32; 8] = [
    0x0000_0000,
    0x0009_0000,
    0x0002_0000,
    0x000b_0000,
    0x0004_0000,
    0x0009_8000,
    0x0006_0000,
    0x000b_8000,
];
const GX_RGBA8_TMEM_ODD: [u32; 8] = [
    0x0008_0000,
    0x0001_0000,
    0x000a_0000,
    0x0003_0000,
    0x0008_8000,
    0x0005_0000,
    0x000a_8000,
    0x0007_0000,
];
const GX_CP_VCD_LOW: u8 = 0x50;
const GX_CP_VCD_HIGH: u8 = 0x60;
const GX_CP_VAT0: u8 = 0x70;
const GX_CP_VAT1: u8 = 0x80;
const GX_CP_VAT2: u8 = 0x90;
const GX_SCISSOR_BIAS: u32 = 342;
const GX_SCISSOR_MASK: u32 = 0x7ff;
const GX_MAX_EFB_DIMENSION: u16 = 1024;
const GX_CP_BASE: usize = 0xCC00_0000;
const GX_PI_FIFO_BASE: usize = 0xCC00_3000;
const GX_PI_WPAR: u32 = 0x0C00_8000;
const GX_CP_READ_ENABLE: u16 = 1;
const GX_CP_FIFO_INT_HI_ENABLE: u16 = 1 << 2;
const GX_CP_FIFO_INT_LO_ENABLE: u16 = 1 << 3;
const GX_CP_FIFO_LINK: u16 = 1 << 4;
static GX_FIFO_ACTIVE: AtomicBool = AtomicBool::new(false);
static GX_DRAW_DONE: AtomicBool = AtomicBool::new(false);
static GX_WAIT_FINISH_QUEUE: AtomicU32 = AtomicU32::new(0);
static GX_TEX_DIRTY: AtomicBool = AtomicBool::new(false);
static GX_TEX_DIRTY_S: AtomicU32 = AtomicU32::new(0);
static GX_TEX_DIRTY_T: AtomicU32 = AtomicU32::new(0);
static GX_TEX_COORD_MANUAL: AtomicU32 = AtomicU32::new(0);
static GX_SU_SSIZE: [AtomicU32; 8] = [
    AtomicU32::new(0x3000_0000),
    AtomicU32::new(0x3200_0000),
    AtomicU32::new(0x3400_0000),
    AtomicU32::new(0x3600_0000),
    AtomicU32::new(0x3800_0000),
    AtomicU32::new(0x3a00_0000),
    AtomicU32::new(0x3c00_0000),
    AtomicU32::new(0x3e00_0000),
];
static GX_SU_TSIZE: [AtomicU32; 8] = [
    AtomicU32::new(0x3100_0000),
    AtomicU32::new(0x3300_0000),
    AtomicU32::new(0x3500_0000),
    AtomicU32::new(0x3700_0000),
    AtomicU32::new(0x3900_0000),
    AtomicU32::new(0x3b00_0000),
    AtomicU32::new(0x3d00_0000),
    AtomicU32::new(0x3f00_0000),
];

static GX_GEN_MODE: AtomicU32 = AtomicU32::new(0);
static GX_GEN_MODE_DIRTY: AtomicBool = AtomicBool::new(false);
static GX_TEV_IND_MASK: AtomicU32 = AtomicU32::new(0x0f00_00ff);
static GX_TEV_IND_MASK_DIRTY: AtomicBool = AtomicBool::new(false);
static GX_PE_Z_MODE: AtomicU32 = AtomicU32::new(0x4000_0000);
static GX_PE_COLOR_MODE0: AtomicU32 = AtomicU32::new(0x4100_0000);
static GX_PE_COLOR_MODE1: AtomicU32 = AtomicU32::new(0x4200_0000);
static GX_PE_CONTROL: AtomicU32 = AtomicU32::new(0x4300_0000);
static GX_LP_WIDTH: AtomicU32 = AtomicU32::new(0x2200_0000);
static GX_DISP_COPY_TL: AtomicU32 = AtomicU32::new(0x4900_0000);
static GX_DISP_COPY_WH: AtomicU32 = AtomicU32::new(0x4a00_0000);
static GX_DISP_COPY_DST: AtomicU32 = AtomicU32::new(0x4d00_0000);
static GX_DISP_COPY_CONTROL: AtomicU32 = AtomicU32::new(0x5200_0000);
static GX_TEX_COPY_TL: AtomicU32 = AtomicU32::new(0x4900_0000);
static GX_TEX_COPY_WH: AtomicU32 = AtomicU32::new(0x4a00_0000);
static GX_TEX_COPY_DST: AtomicU32 = AtomicU32::new(0x4d00_0000);
static GX_TEX_COPY_CONTROL: AtomicU32 = AtomicU32::new(0x5200_0000);
static GX_TEX_COPY_Z: AtomicBool = AtomicBool::new(false);
static GX_CHAN_AMB_COLOR: [AtomicU32; 2] = [AtomicU32::new(0), AtomicU32::new(0)];
static GX_CHAN_MAT_COLOR: [AtomicU32; 2] = [AtomicU32::new(0), AtomicU32::new(0)];
static GX_CHAN_CONTROL: [AtomicU32; 4] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];
static GX_CHAN_COLOR_DIRTY: AtomicU32 = AtomicU32::new(0);
static GX_CHAN_CONTROL_DIRTY: AtomicU32 = AtomicU32::new(0);
static GX_CHAN_COUNT_DIRTY: AtomicBool = AtomicBool::new(false);
static GX_TEV_COLOR_ENV: [AtomicU32; 16] = [
    AtomicU32::new(0xc000_0000),
    AtomicU32::new(0xc200_0000),
    AtomicU32::new(0xc400_0000),
    AtomicU32::new(0xc600_0000),
    AtomicU32::new(0xc800_0000),
    AtomicU32::new(0xca00_0000),
    AtomicU32::new(0xcc00_0000),
    AtomicU32::new(0xce00_0000),
    AtomicU32::new(0xd000_0000),
    AtomicU32::new(0xd200_0000),
    AtomicU32::new(0xd400_0000),
    AtomicU32::new(0xd600_0000),
    AtomicU32::new(0xd800_0000),
    AtomicU32::new(0xda00_0000),
    AtomicU32::new(0xdc00_0000),
    AtomicU32::new(0xde00_0000),
];
static GX_TEV_ALPHA_ENV: [AtomicU32; 16] = [
    AtomicU32::new(0xc100_0000),
    AtomicU32::new(0xc300_0000),
    AtomicU32::new(0xc500_0000),
    AtomicU32::new(0xc700_0000),
    AtomicU32::new(0xc900_0000),
    AtomicU32::new(0xcb00_0000),
    AtomicU32::new(0xcd00_0000),
    AtomicU32::new(0xcf00_0000),
    AtomicU32::new(0xd100_0000),
    AtomicU32::new(0xd300_0000),
    AtomicU32::new(0xd500_0000),
    AtomicU32::new(0xd700_0000),
    AtomicU32::new(0xd900_0000),
    AtomicU32::new(0xdb00_0000),
    AtomicU32::new(0xdd00_0000),
    AtomicU32::new(0xdf00_0000),
];
static GX_TEV_RAS_ORDER: [AtomicU32; 11] = [
    AtomicU32::new(0x2500_0000),
    AtomicU32::new(0x2600_0000),
    AtomicU32::new(0x2700_0000),
    AtomicU32::new(0x2800_0000),
    AtomicU32::new(0x2900_0000),
    AtomicU32::new(0x2a00_0000),
    AtomicU32::new(0x2b00_0000),
    AtomicU32::new(0x2c00_0000),
    AtomicU32::new(0x2d00_0000),
    AtomicU32::new(0x2e00_0000),
    AtomicU32::new(0x2f00_0000),
];
static GX_TEV_SWAP_MODE_TABLE: [AtomicU32; 8] = [
    AtomicU32::new(0xf600_0000),
    AtomicU32::new(0xf700_0000),
    AtomicU32::new(0xf800_0000),
    AtomicU32::new(0xf900_0000),
    AtomicU32::new(0xfa00_0000),
    AtomicU32::new(0xfb00_0000),
    AtomicU32::new(0xfc00_0000),
    AtomicU32::new(0xfd00_0000),
];
static GX_TEV_TEX_MAP: [AtomicU32; 16] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];
static GX_TEV_TEX_COORD_ENABLE: AtomicU32 = AtomicU32::new(0);
static GX_TEX_COORD_GEN: [AtomicU32; 8] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];
static GX_TEX_COORD_GEN2: [AtomicU32; 8] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];
static GX_TEXGEN_DIRTY_MASK: AtomicU32 = AtomicU32::new(0);
static GX_TEXGEN_COUNT_DIRTY: AtomicBool = AtomicBool::new(false);
static GX_MTX_IDX_LO: AtomicU32 = AtomicU32::new(0);
static GX_MTX_IDX_HI: AtomicU32 = AtomicU32::new(0);
static GX_MTX_IDX_DIRTY: AtomicBool = AtomicBool::new(false);
static GX_VCD_LO: AtomicU32 = AtomicU32::new(0);
static GX_VCD_HI: AtomicU32 = AtomicU32::new(0);
static GX_VCD_NRMS: AtomicU32 = AtomicU32::new(0);
static GX_VCD_CLEAR: AtomicU32 = AtomicU32::new(0x0200);
static GX_VCD_DIRTY: AtomicBool = AtomicBool::new(false);
static GX_VAT0: [AtomicU32; 8] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];
static GX_VAT1: [AtomicU32; 8] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];
static GX_VAT2: [AtomicU32; 8] = [
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
    AtomicU32::new(0),
];
static GX_VAT_DIRTY: AtomicU32 = AtomicU32::new(0);

#[derive(Copy, Clone)]
struct GxShadowSnapshot {
    tex_dirty: bool,
    tex_dirty_s: u32,
    tex_dirty_t: u32,
    tex_coord_manual: u32,
    su_ssize: [u32; 8],
    su_tsize: [u32; 8],
    gen_mode: u32,
    gen_mode_dirty: bool,
    tev_ind_mask: u32,
    tev_ind_mask_dirty: bool,
    pe_z_mode: u32,
    pe_color_mode0: u32,
    pe_color_mode1: u32,
    pe_control: u32,
    lp_width: u32,
    disp_copy_tl: u32,
    disp_copy_wh: u32,
    disp_copy_dst: u32,
    disp_copy_control: u32,
    tex_copy_tl: u32,
    tex_copy_wh: u32,
    tex_copy_dst: u32,
    tex_copy_control: u32,
    tex_copy_z: bool,
    chan_amb_color: [u32; 2],
    chan_mat_color: [u32; 2],
    chan_control: [u32; 4],
    chan_color_dirty: u32,
    chan_control_dirty: u32,
    chan_count_dirty: bool,
    tev_color_env: [u32; 16],
    tev_alpha_env: [u32; 16],
    tev_ras_order: [u32; 11],
    tev_swap_mode_table: [u32; 8],
    tev_tex_map: [u32; 16],
    tev_tex_coord_enable: u32,
    tex_coord_gen: [u32; 8],
    tex_coord_gen2: [u32; 8],
    texgen_dirty_mask: u32,
    texgen_count_dirty: bool,
    mtx_idx_lo: u32,
    mtx_idx_hi: u32,
    mtx_idx_dirty: bool,
    vcd_lo: u32,
    vcd_hi: u32,
    vcd_nrms: u32,
    vcd_clear: u32,
    vcd_dirty: bool,
    vat0: [u32; 8],
    vat1: [u32; 8],
    vat2: [u32; 8],
    vat_dirty: u32,
    xf_flush_safe: u32,
    xf_flush_exp: u32,
    xf_flush_count: u32,
}

fn load_atomic_array<const N: usize>(source: &[AtomicU32; N]) -> [u32; N] {
    core::array::from_fn(|i| source[i].load(Ordering::Relaxed))
}

fn store_atomic_array<const N: usize>(target: &[AtomicU32; N], values: [u32; N]) {
    for (slot, value) in target.iter().zip(values) {
        slot.store(value, Ordering::Relaxed);
    }
}

fn capture_gx_shadow() -> GxShadowSnapshot {
    GxShadowSnapshot {
        tex_dirty: GX_TEX_DIRTY.load(Ordering::Relaxed),
        tex_dirty_s: GX_TEX_DIRTY_S.load(Ordering::Relaxed),
        tex_dirty_t: GX_TEX_DIRTY_T.load(Ordering::Relaxed),
        tex_coord_manual: GX_TEX_COORD_MANUAL.load(Ordering::Relaxed),
        su_ssize: load_atomic_array(&GX_SU_SSIZE),
        su_tsize: load_atomic_array(&GX_SU_TSIZE),
        gen_mode: GX_GEN_MODE.load(Ordering::Relaxed),
        gen_mode_dirty: GX_GEN_MODE_DIRTY.load(Ordering::Relaxed),
        tev_ind_mask: GX_TEV_IND_MASK.load(Ordering::Relaxed),
        tev_ind_mask_dirty: GX_TEV_IND_MASK_DIRTY.load(Ordering::Relaxed),
        pe_z_mode: GX_PE_Z_MODE.load(Ordering::Relaxed),
        pe_color_mode0: GX_PE_COLOR_MODE0.load(Ordering::Relaxed),
        pe_color_mode1: GX_PE_COLOR_MODE1.load(Ordering::Relaxed),
        pe_control: GX_PE_CONTROL.load(Ordering::Relaxed),
        lp_width: GX_LP_WIDTH.load(Ordering::Relaxed),
        disp_copy_tl: GX_DISP_COPY_TL.load(Ordering::Relaxed),
        disp_copy_wh: GX_DISP_COPY_WH.load(Ordering::Relaxed),
        disp_copy_dst: GX_DISP_COPY_DST.load(Ordering::Relaxed),
        disp_copy_control: GX_DISP_COPY_CONTROL.load(Ordering::Relaxed),
        tex_copy_tl: GX_TEX_COPY_TL.load(Ordering::Relaxed),
        tex_copy_wh: GX_TEX_COPY_WH.load(Ordering::Relaxed),
        tex_copy_dst: GX_TEX_COPY_DST.load(Ordering::Relaxed),
        tex_copy_control: GX_TEX_COPY_CONTROL.load(Ordering::Relaxed),
        tex_copy_z: GX_TEX_COPY_Z.load(Ordering::Relaxed),
        chan_amb_color: load_atomic_array(&GX_CHAN_AMB_COLOR),
        chan_mat_color: load_atomic_array(&GX_CHAN_MAT_COLOR),
        chan_control: load_atomic_array(&GX_CHAN_CONTROL),
        chan_color_dirty: GX_CHAN_COLOR_DIRTY.load(Ordering::Relaxed),
        chan_control_dirty: GX_CHAN_CONTROL_DIRTY.load(Ordering::Relaxed),
        chan_count_dirty: GX_CHAN_COUNT_DIRTY.load(Ordering::Relaxed),
        tev_color_env: load_atomic_array(&GX_TEV_COLOR_ENV),
        tev_alpha_env: load_atomic_array(&GX_TEV_ALPHA_ENV),
        tev_ras_order: load_atomic_array(&GX_TEV_RAS_ORDER),
        tev_swap_mode_table: load_atomic_array(&GX_TEV_SWAP_MODE_TABLE),
        tev_tex_map: load_atomic_array(&GX_TEV_TEX_MAP),
        tev_tex_coord_enable: GX_TEV_TEX_COORD_ENABLE.load(Ordering::Relaxed),
        tex_coord_gen: load_atomic_array(&GX_TEX_COORD_GEN),
        tex_coord_gen2: load_atomic_array(&GX_TEX_COORD_GEN2),
        texgen_dirty_mask: GX_TEXGEN_DIRTY_MASK.load(Ordering::Relaxed),
        texgen_count_dirty: GX_TEXGEN_COUNT_DIRTY.load(Ordering::Relaxed),
        mtx_idx_lo: GX_MTX_IDX_LO.load(Ordering::Relaxed),
        mtx_idx_hi: GX_MTX_IDX_HI.load(Ordering::Relaxed),
        mtx_idx_dirty: GX_MTX_IDX_DIRTY.load(Ordering::Relaxed),
        vcd_lo: GX_VCD_LO.load(Ordering::Relaxed),
        vcd_hi: GX_VCD_HI.load(Ordering::Relaxed),
        vcd_nrms: GX_VCD_NRMS.load(Ordering::Relaxed),
        vcd_clear: GX_VCD_CLEAR.load(Ordering::Relaxed),
        vcd_dirty: GX_VCD_DIRTY.load(Ordering::Relaxed),
        vat0: load_atomic_array(&GX_VAT0),
        vat1: load_atomic_array(&GX_VAT1),
        vat2: load_atomic_array(&GX_VAT2),
        vat_dirty: GX_VAT_DIRTY.load(Ordering::Relaxed),
        xf_flush_safe: GX_XF_FLUSH_SAFE_STATE.load(Ordering::Relaxed),
        xf_flush_exp: GX_XF_FLUSH_EXP.load(Ordering::Relaxed),
        xf_flush_count: GX_XF_FLUSH_COUNT.load(Ordering::Relaxed),
    }
}

fn restore_gx_shadow(saved: GxShadowSnapshot) {
    GX_TEX_DIRTY.store(saved.tex_dirty, Ordering::Relaxed);
    GX_TEX_DIRTY_S.store(saved.tex_dirty_s, Ordering::Relaxed);
    GX_TEX_DIRTY_T.store(saved.tex_dirty_t, Ordering::Relaxed);
    GX_TEX_COORD_MANUAL.store(saved.tex_coord_manual, Ordering::Relaxed);
    store_atomic_array(&GX_SU_SSIZE, saved.su_ssize);
    store_atomic_array(&GX_SU_TSIZE, saved.su_tsize);
    GX_GEN_MODE.store(saved.gen_mode, Ordering::Relaxed);
    GX_GEN_MODE_DIRTY.store(saved.gen_mode_dirty, Ordering::Relaxed);
    GX_TEV_IND_MASK.store(saved.tev_ind_mask, Ordering::Relaxed);
    GX_TEV_IND_MASK_DIRTY.store(saved.tev_ind_mask_dirty, Ordering::Relaxed);
    GX_PE_Z_MODE.store(saved.pe_z_mode, Ordering::Relaxed);
    GX_PE_COLOR_MODE0.store(saved.pe_color_mode0, Ordering::Relaxed);
    GX_PE_COLOR_MODE1.store(saved.pe_color_mode1, Ordering::Relaxed);
    GX_PE_CONTROL.store(saved.pe_control, Ordering::Relaxed);
    GX_LP_WIDTH.store(saved.lp_width, Ordering::Relaxed);
    GX_DISP_COPY_TL.store(saved.disp_copy_tl, Ordering::Relaxed);
    GX_DISP_COPY_WH.store(saved.disp_copy_wh, Ordering::Relaxed);
    GX_DISP_COPY_DST.store(saved.disp_copy_dst, Ordering::Relaxed);
    GX_DISP_COPY_CONTROL.store(saved.disp_copy_control, Ordering::Relaxed);
    GX_TEX_COPY_TL.store(saved.tex_copy_tl, Ordering::Relaxed);
    GX_TEX_COPY_WH.store(saved.tex_copy_wh, Ordering::Relaxed);
    GX_TEX_COPY_DST.store(saved.tex_copy_dst, Ordering::Relaxed);
    GX_TEX_COPY_CONTROL.store(saved.tex_copy_control, Ordering::Relaxed);
    GX_TEX_COPY_Z.store(saved.tex_copy_z, Ordering::Relaxed);
    store_atomic_array(&GX_CHAN_AMB_COLOR, saved.chan_amb_color);
    store_atomic_array(&GX_CHAN_MAT_COLOR, saved.chan_mat_color);
    store_atomic_array(&GX_CHAN_CONTROL, saved.chan_control);
    GX_CHAN_COLOR_DIRTY.store(saved.chan_color_dirty, Ordering::Relaxed);
    GX_CHAN_CONTROL_DIRTY.store(saved.chan_control_dirty, Ordering::Relaxed);
    GX_CHAN_COUNT_DIRTY.store(saved.chan_count_dirty, Ordering::Relaxed);
    store_atomic_array(&GX_TEV_COLOR_ENV, saved.tev_color_env);
    store_atomic_array(&GX_TEV_ALPHA_ENV, saved.tev_alpha_env);
    store_atomic_array(&GX_TEV_RAS_ORDER, saved.tev_ras_order);
    store_atomic_array(&GX_TEV_SWAP_MODE_TABLE, saved.tev_swap_mode_table);
    store_atomic_array(&GX_TEV_TEX_MAP, saved.tev_tex_map);
    GX_TEV_TEX_COORD_ENABLE.store(saved.tev_tex_coord_enable, Ordering::Relaxed);
    store_atomic_array(&GX_TEX_COORD_GEN, saved.tex_coord_gen);
    store_atomic_array(&GX_TEX_COORD_GEN2, saved.tex_coord_gen2);
    GX_TEXGEN_DIRTY_MASK.store(saved.texgen_dirty_mask, Ordering::Relaxed);
    GX_TEXGEN_COUNT_DIRTY.store(saved.texgen_count_dirty, Ordering::Relaxed);
    GX_MTX_IDX_LO.store(saved.mtx_idx_lo, Ordering::Relaxed);
    GX_MTX_IDX_HI.store(saved.mtx_idx_hi, Ordering::Relaxed);
    GX_MTX_IDX_DIRTY.store(saved.mtx_idx_dirty, Ordering::Relaxed);
    GX_VCD_LO.store(saved.vcd_lo, Ordering::Relaxed);
    GX_VCD_HI.store(saved.vcd_hi, Ordering::Relaxed);
    GX_VCD_NRMS.store(saved.vcd_nrms, Ordering::Relaxed);
    GX_VCD_CLEAR.store(saved.vcd_clear, Ordering::Relaxed);
    GX_VCD_DIRTY.store(saved.vcd_dirty, Ordering::Relaxed);
    store_atomic_array(&GX_VAT0, saved.vat0);
    store_atomic_array(&GX_VAT1, saved.vat1);
    store_atomic_array(&GX_VAT2, saved.vat2);
    GX_VAT_DIRTY.store(saved.vat_dirty, Ordering::Relaxed);
    GX_XF_FLUSH_SAFE_STATE.store(saved.xf_flush_safe, Ordering::Relaxed);
    GX_XF_FLUSH_EXP.store(saved.xf_flush_exp, Ordering::Relaxed);
    GX_XF_FLUSH_COUNT.store(saved.xf_flush_count, Ordering::Relaxed);
}

struct SavedDlContext(UnsafeCell<Option<GxShadowSnapshot>>);
unsafe impl Sync for SavedDlContext {}
static GX_DL_SAVED_CONTEXT: SavedDlContext = SavedDlContext(UnsafeCell::new(None));

static GX_DL_ACTIVE: AtomicBool = AtomicBool::new(false);
static GX_DL_BASE: AtomicU32 = AtomicU32::new(0);
static GX_DL_SIZE: AtomicU32 = AtomicU32::new(0);
static GX_DL_OFFSET: AtomicU32 = AtomicU32::new(0);
static GX_DL_OVERFLOW: AtomicBool = AtomicBool::new(false);
static GX_DL_SAVED_CPU_FIFO: FifoAtomicState = FifoAtomicState::new();
static GX_DL_SAVED_CPU_READY: AtomicBool = AtomicBool::new(false);
static GX_OVERFLOW_COUNT: AtomicU32 = AtomicU32::new(0);
static GX_OVERFLOW_SUSPENDED: AtomicBool = AtomicBool::new(false);
static GX_CURRENT_THREAD: AtomicU32 = AtomicU32::new(0);
static GX_PERF0_MODE: AtomicU32 = AtomicU32::new(GX_PERF0_NONE);
static GX_PERF1_MODE: AtomicU32 = AtomicU32::new(GX_PERF1_NONE);
static GX_CP_PERF_MODE: AtomicU32 = AtomicU32::new(0);
static GX_CPU_FIFO_STATE: FifoAtomicState = FifoAtomicState::new();
static GX_GP_FIFO_STATE: FifoAtomicState = FifoAtomicState::new();
static GX_CPU_FIFO_READY: AtomicBool = AtomicBool::new(false);
static GX_GP_FIFO_READY: AtomicBool = AtomicBool::new(false);
static GX_CP_GP_LINKED: AtomicBool = AtomicBool::new(false);
static GX_CP_CONTROL: AtomicU32 = AtomicU32::new(0);
static GX_XF_FLUSH_SAFE_STATE: AtomicU32 = AtomicU32::new(0);
static GX_XF_FLUSH_EXP: AtomicU32 = AtomicU32::new(0);
static GX_XF_FLUSH_COUNT: AtomicU32 = AtomicU32::new(1);
static GX_SAVE_DL_CTX: AtomicU32 = AtomicU32::new(1);
static GX_DRAW_DONE_CALLBACK: AtomicU32 = AtomicU32::new(0);
static GX_DRAW_SYNC_CALLBACK: AtomicU32 = AtomicU32::new(0);
static GX_BREAK_PT_CALLBACK: AtomicU32 = AtomicU32::new(0);
static GX_BREAK_POINT: AtomicU32 = AtomicU32::new(0);
// The initialization FIFO object is byte-addressed by libogc, but its public
// storage must be at least 4-byte aligned. A u32 backing array expresses that
// requirement directly without an otherwise-unused alignment wrapper field.
static mut GX_INIT_FIFO_STORAGE: [u32; GX_FIFO_OBJSIZE as usize / core::mem::size_of::<u32>()] =
    [0; GX_FIFO_OBJSIZE as usize / core::mem::size_of::<u32>()];
const GX_PE_BASE: usize = 0xCC00_1000;
const GX_EFB_POKE_BASE: usize = 0xC800_0000;

#[inline]
fn pe_reg(index: usize) -> *mut u16 {
    (GX_PE_BASE + index * core::mem::size_of::<u16>()) as *mut u16
}

#[inline]
fn efb_poke_addr(x: u16, y: u16, z_plane: bool) -> *mut u32 {
    let mut addr =
        GX_EFB_POKE_BASE | ((usize::from(x) & 0x03ff) << 2) | ((usize::from(y) & 0x03ff) << 12);
    if z_plane {
        addr = (addr & !0x00c0_0000) | 0x0040_0000;
    }
    addr as *mut u32
}

#[inline]
fn update_pe_bit(word: u16, bit: u16, enabled: u8) -> u16 {
    (word & !(1 << bit)) | ((u16::from(enabled) & 1) << bit)
}

fn poke_blend_word(mut word: u16, type_: u8, src_fact: u8, dst_fact: u8, op: u8) -> u16 {
    word &= !0x0001;
    if type_ == GX_BM_BLEND || type_ == GX_BM_SUBTRACT {
        word |= 0x0001;
    }
    word &= !0x0800;
    if type_ == GX_BM_SUBTRACT {
        word |= 0x0800;
    }
    word &= !0x0002;
    if type_ == GX_BM_LOGIC {
        word |= 0x0002;
    }
    word = (word & !0xf000) | ((u16::from(op) & 0xf) << 12);
    word = (word & !0x00e0) | ((u16::from(dst_fact) & 0x7) << 5);
    word = (word & !0x0700) | ((u16::from(src_fact) & 0x7) << 8);
    word
}

const GX_PE_FINISH_STATUS: *mut u16 = (GX_PE_BASE + 5 * core::mem::size_of::<u16>()) as *mut u16;
const GX_DRAW_DONE_MAX_POLLS: u32 = 10_000_000;

fn ensure_draw_done_queue() -> u32 {
    let existing = GX_WAIT_FINISH_QUEUE.load(Ordering::Acquire);
    if existing != 0 {
        return existing;
    }
    let mut queue = 0;
    let result = unsafe { ogc_lwp_sys::init_queue(&mut queue) };
    if result != ogc_lwp_sys::LWP_SUCCESSFUL || queue == 0 {
        return 0;
    }
    match GX_WAIT_FINISH_QUEUE.compare_exchange(0, queue, Ordering::AcqRel, Ordering::Acquire) {
        Ok(_) => queue,
        Err(installed) => {
            ogc_lwp_sys::close_queue(queue);
            installed
        }
    }
}

unsafe fn callback0_from_bits(bits: u32) -> Option<unsafe extern "C" fn()> {
    if bits == 0 {
        None
    } else {
        Some(unsafe { core::mem::transmute::<usize, unsafe extern "C" fn()>(bits as usize) })
    }
}

unsafe fn callback_token_from_bits(bits: u32) -> Option<unsafe extern "C" fn(u16)> {
    if bits == 0 {
        None
    } else {
        Some(unsafe { core::mem::transmute::<usize, unsafe extern "C" fn(u16)>(bits as usize) })
    }
}

unsafe extern "C" fn pe_finish_interrupt(id: ogc_irq::KIrqId, _user: *mut c_void) {
    if id != ogc_irq::IRQ_PI_PEFINISH {
        return;
    }
    let status = unsafe { ptr::read_volatile(GX_PE_FINISH_STATUS) };
    unsafe { ptr::write_volatile(GX_PE_FINISH_STATUS, (status & !0x0008) | 0x0008) };
    GX_DRAW_DONE.store(true, Ordering::Release);
    if let Some(cb) = unsafe { callback0_from_bits(GX_DRAW_DONE_CALLBACK.load(Ordering::Acquire)) }
    {
        unsafe { cb() };
    }
    let queue = GX_WAIT_FINISH_QUEUE.load(Ordering::Acquire);
    if queue != 0 {
        ogc_lwp_sys::thread_broadcast(queue);
    }
}

unsafe extern "C" fn pe_token_interrupt(id: ogc_irq::KIrqId, _user: *mut c_void) {
    if id != ogc_irq::IRQ_PI_PETOKEN {
        return;
    }
    let token = unsafe { ptr::read_volatile(pe_reg(7)) };
    if let Some(cb) =
        unsafe { callback_token_from_bits(GX_DRAW_SYNC_CALLBACK.load(Ordering::Acquire)) }
    {
        unsafe { cb(token) };
    }
    let status = unsafe { ptr::read_volatile(GX_PE_FINISH_STATUS) };
    unsafe { ptr::write_volatile(GX_PE_FINISH_STATUS, (status & !0x0004) | 0x0004) };
}

fn set_fifo_interrupt_enable(high: bool, low: bool) {
    let mut control = GX_CP_CONTROL.load(Ordering::Acquire) as u16;
    control &= !(GX_CP_FIFO_INT_HI_ENABLE | GX_CP_FIFO_INT_LO_ENABLE);
    if high {
        control |= GX_CP_FIFO_INT_HI_ENABLE;
    }
    if low {
        control |= GX_CP_FIFO_INT_LO_ENABLE;
    }
    GX_CP_CONTROL.store(u32::from(control), Ordering::Release);
    unsafe { ptr::write_volatile(cp_reg(1), control) };
}

fn handle_fifo_overflow() {
    if GX_OVERFLOW_SUSPENDED.swap(true, Ordering::AcqRel) {
        return;
    }
    GX_OVERFLOW_COUNT.fetch_add(1, Ordering::AcqRel);
    set_fifo_interrupt_enable(false, true);
    let thread = GX_CURRENT_THREAD.load(Ordering::Acquire);
    if thread != 0 {
        let _ = ogc_lwp_sys::suspend_thread(thread);
    }
}

fn handle_fifo_underflow() {
    if !GX_OVERFLOW_SUSPENDED.swap(false, Ordering::AcqRel) {
        return;
    }
    let thread = GX_CURRENT_THREAD.load(Ordering::Acquire);
    if thread != 0 {
        let _ = ogc_lwp_sys::resume_thread(thread);
    }
    set_fifo_interrupt_enable(true, false);
}

unsafe extern "C" fn cp_interrupt(id: ogc_irq::KIrqId, _user: *mut c_void) {
    if id != ogc_irq::IRQ_PI_CP {
        return;
    }
    unsafe { ptr::write_volatile(cp_reg(2), 0x0003) };
    let status = read_cp_u16(0);
    if status & 0x02 != 0 {
        handle_fifo_underflow();
    } else if status & 0x01 != 0 {
        handle_fifo_overflow();
    }
    let control = GX_CP_CONTROL.load(Ordering::Acquire) as u16;
    if status & 0x10 != 0 && control & 0x20 != 0 {
        let new_control = control & !0x20;
        GX_CP_CONTROL.store(u32::from(new_control), Ordering::Release);
        unsafe { ptr::write_volatile(cp_reg(1), new_control) };
        if let Some(cb) =
            unsafe { callback0_from_bits(GX_BREAK_PT_CALLBACK.load(Ordering::Acquire)) }
        {
            unsafe { cb() };
        }
    }
}

fn register_gx_interrupts() -> bool {
    let irq_state = ogc_irq::irq_disable();
    let irqs = [
        ogc_irq::IRQ_PI_PEFINISH,
        ogc_irq::IRQ_PI_PETOKEN,
        ogc_irq::IRQ_PI_CP,
    ];
    if irqs
        .into_iter()
        .any(|irq| ogc_irq::get_handler(irq).is_some())
    {
        ogc_irq::irq_restore(irq_state);
        return false;
    }

    unsafe {
        let _ = ogc_irq::request(
            ogc_irq::IRQ_PI_PEFINISH,
            Some(pe_finish_interrupt),
            ptr::null_mut(),
        );
        let _ = ogc_irq::request(
            ogc_irq::IRQ_PI_PETOKEN,
            Some(pe_token_interrupt),
            ptr::null_mut(),
        );
        let _ = ogc_irq::request(ogc_irq::IRQ_PI_CP, Some(cp_interrupt), ptr::null_mut());
    }
    for irq in irqs {
        ogc_irq::unmask(1 << irq);
    }
    unsafe { ptr::write_volatile(GX_PE_FINISH_STATUS, 0x000f) };
    ogc_irq::irq_restore(irq_state);
    true
}

fn fifo_physical(addr: u32) -> u32 {
    addr & 0x3fff_ffff
}
fn fifo_k0(addr: u32) -> u32 {
    (addr & 0x3fff_ffff) | 0x8000_0000
}

fn fifo_states_linkable(cpu: FifoSnapshot, gp: FifoSnapshot) -> bool {
    cpu.cpufifo_ready != 0
        && gp.gpfifo_ready != 0
        && cpu.buf_start == gp.buf_start
        && cpu.buf_end == gp.buf_end
}

fn display_list_fifo_snapshot_from(
    base: u32,
    size: u32,
    offset: u32,
    overflow: bool,
) -> Option<FifoSnapshot> {
    if base == 0 || size == 0 {
        return None;
    }
    let offset = offset.min(size);
    Some(FifoSnapshot {
        buf_start: base,
        buf_end: base.wrapping_add(size).wrapping_sub(4),
        size,
        hi_mark: 0,
        lo_mark: 0,
        rd_ptr: base,
        wt_ptr: base.wrapping_add(offset),
        rdwt_dst: offset,
        fifo_wrap: overflow as u8,
        cpufifo_ready: 1,
        gpfifo_ready: 0,
    })
}

fn display_list_fifo_snapshot() -> Option<FifoSnapshot> {
    if !GX_DL_ACTIVE.load(Ordering::Acquire) {
        return None;
    }
    display_list_fifo_snapshot_from(
        GX_DL_BASE.load(Ordering::Relaxed),
        GX_DL_SIZE.load(Ordering::Relaxed),
        GX_DL_OFFSET.load(Ordering::Acquire),
        GX_DL_OVERFLOW.load(Ordering::Acquire),
    )
}

fn refresh_fifo_hardware_state() {
    if let Some(cpu) = display_list_fifo_snapshot() {
        GX_CPU_FIFO_STATE.store(cpu);
    } else if GX_CPU_FIFO_READY.load(Ordering::Acquire) {
        let mut cpu = GX_CPU_FIFO_STATE.load();
        let pi = GX_PI_FIFO_BASE as *mut u32;
        let raw = unsafe { ptr::read_volatile(pi.add(5)) };
        cpu.wt_ptr = fifo_k0(raw & 0x1fff_ffe0);
        cpu.fifo_wrap = ((raw & 0x2000_0000) != 0) as u8;
        GX_CPU_FIFO_STATE.store(cpu);
    }
    if GX_GP_FIFO_READY.load(Ordering::Acquire) {
        let mut gp = GX_GP_FIFO_STATE.load();
        gp.rd_ptr = fifo_k0(read_cp_pair(28));
        gp.rdwt_dst = read_cp_pair(24);
        GX_GP_FIFO_STATE.store(gp);
    }
    if GX_CP_GP_LINKED.load(Ordering::Acquire) {
        let mut cpu = GX_CPU_FIFO_STATE.load();
        let mut gp = GX_GP_FIFO_STATE.load();
        cpu.rd_ptr = gp.rd_ptr;
        cpu.rdwt_dst = gp.rdwt_dst;
        gp.wt_ptr = cpu.wt_ptr;
        GX_CPU_FIFO_STATE.store(cpu);
        GX_GP_FIFO_STATE.store(gp);
    } else if GX_CPU_FIFO_READY.load(Ordering::Acquire) {
        let mut cpu = GX_CPU_FIFO_STATE.load();
        let diff = cpu.wt_ptr.wrapping_sub(cpu.rd_ptr);
        cpu.rdwt_dst = if (diff as i32) < 0 {
            diff.wrapping_add(cpu.size)
        } else {
            diff
        };
        GX_CPU_FIFO_STATE.store(cpu);
    }
}

fn program_cpu_fifo(snapshot: FifoSnapshot) {
    let pi = GX_PI_FIFO_BASE as *mut u32;
    unsafe {
        ptr::write_volatile(pi.add(3), fifo_physical(snapshot.buf_start));
        ptr::write_volatile(pi.add(4), fifo_physical(snapshot.buf_end));
        ptr::write_volatile(pi.add(5), fifo_physical(snapshot.wt_ptr) & 0x1fff_ffe0);
    }
}

fn program_gp_fifo(snapshot: FifoSnapshot) {
    unsafe {
        write_cp_u32(16, fifo_physical(snapshot.buf_start));
        write_cp_u32(18, fifo_physical(snapshot.buf_end));
        write_cp_u32(20, snapshot.hi_mark);
        write_cp_u32(22, snapshot.lo_mark);
        write_cp_u32(24, snapshot.rdwt_dst);
        write_cp_u32(26, fifo_physical(snapshot.wt_ptr));
        write_cp_u32(28, fifo_physical(snapshot.rd_ptr));
    }
}

fn update_fifo_link_state() {
    let linked = GX_CPU_FIFO_READY.load(Ordering::Acquire)
        && GX_GP_FIFO_READY.load(Ordering::Acquire)
        && fifo_states_linkable(GX_CPU_FIFO_STATE.load(), GX_GP_FIFO_STATE.load());
    GX_CP_GP_LINKED.store(linked, Ordering::Release);
    let mut control = GX_CP_CONTROL.load(Ordering::Relaxed) as u16;
    control |= GX_CP_READ_ENABLE;
    if linked {
        control |= GX_CP_FIFO_LINK;
    } else {
        control &= !GX_CP_FIFO_LINK;
    }
    GX_CP_CONTROL.store(u32::from(control), Ordering::Release);
    unsafe { ptr::write_volatile(cp_reg(1), control) };
}

/// Initializes an opaque FIFO object around caller-owned storage.
///
/// # Safety
/// `fifo` must be writable for `GX_FIFO_OBJSIZE` bytes. `base` must identify a FIFO buffer.
pub unsafe fn init_fifo_base(fifo: *mut GxFifoObj, base: *mut c_void, size: u32) {
    if fifo.is_null() || base.is_null() || size < GX_FIFO_MINSIZE {
        return;
    }
    let start = base as usize as u32;
    let Some(end) = start.checked_add(size).and_then(|v| v.checked_sub(4)) else {
        return;
    };
    let mut snap = FifoSnapshot {
        buf_start: start,
        buf_end: end,
        size,
        ..FifoSnapshot::default()
    };
    snap.hi_mark = size.wrapping_sub(GX_FIFO_HIWATERMARK);
    snap.lo_mark = (size >> 1) & 0x7fff_ffe0;
    snap.rd_ptr = start;
    snap.wt_ptr = start;
    unsafe { write_fifo_snapshot(fifo, snap) };
}

/// # Safety
/// `fifo` must point to a writable GX FIFO object.
pub unsafe fn init_fifo_limits(fifo: *mut GxFifoObj, hiwatermark: u32, lowatermark: u32) {
    let Some(mut s) = fifo_snapshot(fifo) else {
        return;
    };
    s.hi_mark = hiwatermark;
    s.lo_mark = lowatermark;
    unsafe { write_fifo_snapshot(fifo, s) };
}

/// # Safety
/// `fifo` must point to a writable GX FIFO object.
pub unsafe fn init_fifo_ptrs(fifo: *mut GxFifoObj, rd_ptr: *mut c_void, wt_ptr: *mut c_void) {
    let Some(mut s) = fifo_snapshot(fifo) else {
        return;
    };
    let rd = rd_ptr as usize as u32;
    let wt = wt_ptr as usize as u32;
    let Some((read, write, distance)) = fifo_pointers(s.buf_start, s.size, rd, wt) else {
        return;
    };
    s.rd_ptr = read;
    s.wt_ptr = write;
    s.rdwt_dst = distance;
    unsafe { write_fifo_snapshot(fifo, s) };
}

/// # Safety
/// `fifo` must be readable; non-null outputs must be writable pointers.
pub unsafe fn get_fifo_ptrs(
    fifo: *const GxFifoObj,
    rd_ptr: *mut *mut c_void,
    wt_ptr: *mut *mut c_void,
) {
    let Some(s) = fifo_snapshot(fifo) else {
        return;
    };
    if !rd_ptr.is_null() {
        unsafe { ptr::write(rd_ptr, s.rd_ptr as usize as *mut c_void) };
    }
    if !wt_ptr.is_null() {
        unsafe { ptr::write(wt_ptr, s.wt_ptr as usize as *mut c_void) };
    }
}

/// # Safety
/// A non-null FIFO must contain a valid initialized `GxFifoObj`.
pub unsafe fn set_cpu_fifo(fifo: *mut GxFifoObj) {
    let Some(mut s) = fifo_snapshot(fifo) else {
        GX_CPU_FIFO_READY.store(false, Ordering::Release);
        GX_CP_GP_LINKED.store(false, Ordering::Release);
        return;
    };
    s.cpufifo_ready = 1;
    GX_CPU_FIFO_STATE.store(s);
    GX_CPU_FIFO_READY.store(true, Ordering::Release);
    program_cpu_fifo(s);
    update_fifo_link_state();
}

/// # Safety
/// A non-null FIFO must contain a valid initialized `GxFifoObj`.
pub unsafe fn set_gp_fifo(fifo: *mut GxFifoObj) {
    let Some(mut s) = fifo_snapshot(fifo) else {
        GX_GP_FIFO_READY.store(false, Ordering::Release);
        GX_CP_GP_LINKED.store(false, Ordering::Release);
        let mut c = GX_CP_CONTROL.load(Ordering::Relaxed) as u16;
        c &= !(GX_CP_READ_ENABLE | GX_CP_FIFO_LINK);
        GX_CP_CONTROL.store(u32::from(c), Ordering::Release);
        unsafe { ptr::write_volatile(cp_reg(1), c) };
        return;
    };
    s.gpfifo_ready = 1;
    GX_GP_FIFO_STATE.store(s);
    GX_GP_FIFO_READY.store(true, Ordering::Release);
    program_gp_fifo(s);
    update_fifo_link_state();
}

/// # Safety
/// `fifo` must be writable for a GX FIFO object.
pub unsafe fn get_cpu_fifo(fifo: *mut GxFifoObj) {
    if fifo.is_null() || !GX_CPU_FIFO_READY.load(Ordering::Acquire) {
        return;
    }
    unsafe { flush() };
    refresh_fifo_hardware_state();
    unsafe { write_fifo_snapshot(fifo, GX_CPU_FIFO_STATE.load()) };
}

/// # Safety
/// `fifo` must be writable for a GX FIFO object.
pub unsafe fn get_gp_fifo(fifo: *mut GxFifoObj) {
    if fifo.is_null() || !GX_GP_FIFO_READY.load(Ordering::Acquire) {
        return;
    }
    refresh_fifo_hardware_state();
    unsafe { write_fifo_snapshot(fifo, GX_GP_FIFO_STATE.load()) };
}

/// # Safety
/// A non-null `fifo` must point to a readable, initialized FIFO object. Serialize access with FIFO changes.
pub unsafe fn get_fifo_base(fifo: *const GxFifoObj) -> *mut c_void {
    fifo_snapshot(fifo).map_or(ptr::null_mut(), |s| s.buf_start as usize as *mut c_void)
}
/// # Safety
/// A non-null `fifo` must point to a readable, initialized FIFO object. Serialize access with FIFO changes.
pub unsafe fn get_fifo_size(fifo: *const GxFifoObj) -> u32 {
    fifo_snapshot(fifo).map_or(0, |s| s.size)
}
/// # Safety
/// A non-null `fifo` must point to a readable, initialized FIFO object. Serialize access with FIFO changes.
pub unsafe fn get_fifo_count(fifo: *const GxFifoObj) -> u32 {
    fifo_snapshot(fifo).map_or(0, |s| s.rdwt_dst)
}
/// # Safety
/// A non-null `fifo` must point to a readable, initialized FIFO object. Serialize access with FIFO changes.
pub unsafe fn get_fifo_wrap(fifo: *const GxFifoObj) -> u8 {
    fifo_snapshot(fifo).map_or(0, |s| s.fifo_wrap)
}

fn callback0_bits(cb: GxDrawDoneCallback) -> u32 {
    cb.map_or(0, |f| f as usize as u32)
}
fn break_callback_bits(cb: GxBreakPtCallback) -> u32 {
    cb.map_or(0, |f| f as usize as u32)
}
fn token_callback_bits(cb: GxDrawSyncCallback) -> u32 {
    cb.map_or(0, |f| f as usize as u32)
}

/// C-shaped GX initialization using caller-owned FIFO storage.
///
/// # Safety
/// `base..base+size` must be exclusively owned by GX, writable, 32-byte aligned, and DMA-visible.
pub unsafe fn init(base: *mut c_void, size: u32) -> *mut GxFifoObj {
    let addr = base as usize;
    if base.is_null() || addr & 31 != 0 || size < GX_FIFO_MINSIZE || size & 31 != 0 {
        return ptr::null_mut();
    }
    if [
        ogc_irq::IRQ_PI_PEFINISH,
        ogc_irq::IRQ_PI_PETOKEN,
        ogc_irq::IRQ_PI_CP,
    ]
    .into_iter()
    .any(|irq| ogc_irq::get_handler(irq).is_some())
    {
        return ptr::null_mut();
    }
    if GX_FIFO_ACTIVE
        .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
        .is_err()
    {
        return ptr::null_mut();
    }
    if ensure_draw_done_queue() == 0 {
        GX_FIFO_ACTIVE.store(false, Ordering::Release);
        return ptr::null_mut();
    }
    if !register_gx_interrupts() {
        GX_FIFO_ACTIVE.store(false, Ordering::Release);
        return ptr::null_mut();
    }
    let fifo = core::ptr::addr_of_mut!(GX_INIT_FIFO_STORAGE).cast::<GxFifoObj>();
    unsafe { ptr::write_bytes(fifo.cast::<u8>(), 0, GX_FIFO_OBJSIZE as usize) };
    unsafe {
        init_fifo_base(fifo, base, size);
        set_cpu_fifo(fifo);
        set_gp_fifo(fifo);
        enable_write_gather_pipe();
        initialize_gx_registers();
    }
    GX_CURRENT_THREAD.store(ogc_lwp_sys::get_self(), Ordering::Release);
    fifo
}

pub fn set_draw_done_callback(cb: GxDrawDoneCallback) -> GxDrawDoneCallback {
    let old = GX_DRAW_DONE_CALLBACK.swap(callback0_bits(cb), Ordering::AcqRel);
    unsafe { callback0_from_bits(old) }
}

pub fn set_draw_sync_callback(cb: GxDrawSyncCallback) -> GxDrawSyncCallback {
    let old = GX_DRAW_SYNC_CALLBACK.swap(token_callback_bits(cb), Ordering::AcqRel);
    unsafe { callback_token_from_bits(old) }
}

pub fn set_break_pt_callback(cb: GxBreakPtCallback) -> GxBreakPtCallback {
    let old = GX_BREAK_PT_CALLBACK.swap(break_callback_bits(cb), Ordering::AcqRel);
    unsafe { callback0_from_bits(old) }
}

pub fn set_draw_sync(token: u16) {
    load_bp_reg(0x48, u32::from(token));
    load_bp_reg(0x47, u32::from(token));
    unsafe { flush() };
}

pub fn get_draw_sync() -> u16 {
    unsafe { ptr::read_volatile(pe_reg(7)) }
}

pub fn tex_mode_sync() {
    load_bp_reg(0x63, 0);
}

pub fn set_misc(token: u32, value: u32) {
    if token == u32::from(GX_MT_XF_FLUSH) {
        GX_XF_FLUSH_SAFE_STATE.store(value, Ordering::Relaxed);
        GX_XF_FLUSH_EXP.store(value.leading_zeros() >> 5, Ordering::Relaxed);
        GX_XF_FLUSH_COUNT.store(1, Ordering::Relaxed);
        if value != 0 {
            GX_VCD_DIRTY.store(true, Ordering::Release);
        }
    } else if token == u32::from(GX_MT_DL_SAVE_CTX) {
        GX_SAVE_DL_CTX.store(value & 0xff, Ordering::Relaxed);
    }
}

pub fn enable_break_pt(break_pt: *mut c_void) {
    if break_pt.is_null() {
        return;
    }
    let physical = fifo_physical(break_pt as usize as u32);
    let mut control = GX_CP_CONTROL.load(Ordering::Relaxed) as u16;
    control &= !GX_CP_READ_ENABLE;
    unsafe {
        ptr::write_volatile(cp_reg(1), control);
        ptr::write_volatile(cp_reg(30), physical as u16);
        ptr::write_volatile(cp_reg(31), (physical >> 16) as u16);
    }
    control = (control & !0x22) | 0x22;
    GX_CP_CONTROL.store(u32::from(control), Ordering::Release);
    GX_BREAK_POINT.store(break_pt as usize as u32, Ordering::Release);
    unsafe { ptr::write_volatile(cp_reg(1), control | GX_CP_READ_ENABLE) };
}

pub fn disable_break_pt() {
    let control = (GX_CP_CONTROL.load(Ordering::Relaxed) as u16) & !0x22;
    GX_CP_CONTROL.store(u32::from(control), Ordering::Release);
    GX_BREAK_POINT.store(0, Ordering::Release);
    unsafe { ptr::write_volatile(cp_reg(1), control) };
}

fn send_flush_primitive() {
    let count = GX_XF_FLUSH_COUNT.load(Ordering::Relaxed);
    let words = count.saturating_mul(GX_XF_FLUSH_EXP.load(Ordering::Relaxed));
    pipe_u8(GX_TRIANGLESTRIP);
    pipe_u16(count as u16);
    for _ in 0..words.saturating_add(3) / 4 {
        pipe_u32(0);
    }
    GX_XF_FLUSH_COUNT.store(1, Ordering::Relaxed);
}

/// # Safety
/// `list` must point to a valid GX display list of at least `nbytes` bytes.
pub unsafe fn call_disp_list(list: *mut c_void, nbytes: u32) {
    if list.is_null() || nbytes == 0 || GX_DL_ACTIVE.load(Ordering::Acquire) {
        return;
    }
    flush_pending_gx_state();
    if GX_VCD_CLEAR.load(Ordering::Relaxed) == 0 {
        send_flush_primitive();
    }
    pipe_u8(0x40);
    pipe_u32(fifo_physical(list as usize as u32));
    pipe_u32(nbytes);
}

fn wait_write_gather_empty() {
    loop {
        let wpar: u32;
        unsafe {
            core::arch::asm!("mfspr {out}, 921", out = out(reg) wpar, options(nostack, preserves_flags))
        };
        if wpar & 1 == 0 {
            break;
        }
        core::hint::spin_loop();
    }
}

fn set_write_gather_address(value: u32) {
    unsafe {
        core::arch::asm!("mtspr 921, {value}", value = in(reg) value, options(nostack, preserves_flags));
    }
}

/// Redirects the CPU write-gather FIFO to `ptr` and returns the write-gather aperture.
///
/// # Safety
/// `ptr` must be a writable, DMA-visible, 32-byte-aligned destination.
pub unsafe fn redirect_write_gather_pipe(ptr_: *mut c_void) -> *mut c_void {
    if ptr_.is_null() || (ptr_ as usize & 31) != 0 || GX_DL_ACTIVE.load(Ordering::Acquire) {
        return ptr::null_mut();
    }
    unsafe { flush() };
    wait_write_gather_empty();
    set_write_gather_address(GX_PI_WPAR);
    refresh_fifo_hardware_state();
    let mut cpu = GX_CPU_FIFO_STATE.load();
    if GX_CP_GP_LINKED.swap(false, Ordering::AcqRel) {
        let control = (GX_CP_CONTROL.load(Ordering::Relaxed) as u16) & !GX_CP_FIFO_LINK;
        GX_CP_CONTROL.store(u32::from(control), Ordering::Release);
        unsafe { ptr::write_volatile(cp_reg(1), control) };
    }
    let pi = GX_PI_FIFO_BASE as *mut u32;
    let raw = unsafe { ptr::read_volatile(pi.add(5)) };
    cpu.wt_ptr = fifo_k0(raw & !0x0400_0000);
    GX_CPU_FIFO_STATE.store(cpu);
    unsafe {
        ptr::write_volatile(pi.add(3), 0);
        ptr::write_volatile(pi.add(4), 0x0400_0000);
        ptr::write_volatile(
            pi.add(5),
            ((ptr_ as usize as u32) & 0x3fff_ffe0) & !0x0400_0000,
        );
        core::arch::asm!("sync", options(nostack, preserves_flags));
    }
    0x0c00_8000usize as *mut c_void
}

pub fn restore_write_gather_pipe() {
    if GX_DL_ACTIVE.load(Ordering::Acquire) {
        return;
    }
    for _ in 0..8 {
        unsafe { ptr::write_volatile(GP_PIPE_ADDR as *mut u32, 0) };
    }
    unsafe { core::arch::asm!("sync", options(nostack, preserves_flags)) };
    wait_write_gather_empty();
    set_write_gather_address(GX_PI_WPAR);
    if GX_CPU_FIFO_READY.load(Ordering::Acquire) {
        let cpu = GX_CPU_FIFO_STATE.load();
        program_cpu_fifo(cpu);
        if fifo_states_linkable(cpu, GX_GP_FIFO_STATE.load()) {
            GX_CP_GP_LINKED.store(true, Ordering::Release);
            update_fifo_link_state();
            unsafe { ptr::write_volatile(cp_reg(2), 0x0003) };
        }
    }
}

fn wait_abort_ticks(delay: u64) {
    let start = ogc_lwp_sys::now_ticks();
    while ogc_lwp_sys::now_ticks().wrapping_sub(start) < delay {
        core::hint::spin_loop();
    }
}

pub fn abort_frame() {
    let pi = GX_PI_FIFO_BASE as *mut u32;
    unsafe {
        ptr::write_volatile(pi.add(6), 1);
        core::arch::asm!("sync", options(nostack, preserves_flags));
    }
    wait_abort_ticks(50);
    unsafe {
        ptr::write_volatile(pi.add(6), 0);
        core::arch::asm!("sync", options(nostack, preserves_flags));
    }
    wait_abort_ticks(5);
    if GX_GP_FIFO_READY.load(Ordering::Acquire) {
        let mut gp = GX_GP_FIFO_STATE.load();
        gp.rd_ptr = gp.wt_ptr;
        gp.rdwt_dst = 0;
        GX_GP_FIFO_STATE.store(gp);
        program_gp_fifo(gp);
        if GX_CP_GP_LINKED.load(Ordering::Acquire) {
            let mut cpu = GX_CPU_FIFO_STATE.load();
            cpu.rd_ptr = gp.rd_ptr;
            cpu.wt_ptr = gp.wt_ptr;
            cpu.rdwt_dst = 0;
            GX_CPU_FIFO_STATE.store(cpu);
            program_cpu_fifo(cpu);
        }
        disable_break_pt();
        GX_BREAK_PT_CALLBACK.store(0, Ordering::Release);
        unsafe { ptr::write_volatile(cp_reg(2), 0x0003) };
    }
}

/// Error returned when GX cannot reserve or initialize its MEM2 command FIFO.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum GxFifoError {
    AlreadyActive,
    PeFinishInterruptInUse,
    InvalidSize,
    InsufficientMem2,
}

/// A command FIFO reserved from the low end of MEM2.
///
/// MEM2 arena allocations cannot be returned. Keep one value for the lifetime of GX. Dropping
/// this handle does not stop or release the hardware FIFO.
pub struct GxFifo {
    base: *mut u8,
    size: u32,
    _not_send_sync: PhantomData<*mut ()>,
}

impl GxFifo {
    /// Reserve and start the GX FIFO with the default 256 KiB MEM2 buffer.
    ///
    /// # Safety
    /// See [`GxFifo::initialize_mem2`].
    #[cfg(not(gamecube))]
    pub unsafe fn initialize_default_mem2() -> Result<Self, GxFifoError> {
        unsafe { Self::initialize_mem2(GX_FIFO_DEFAULT_SIZE) }
    }

    /// Reserve and start the GX CPU/GP FIFO in MEM2.
    ///
    /// The size must be 64 KiB through 1 MiB and a multiple of 32 bytes. The allocator advances
    /// MEM2's low arena pointer only after it confirms that the full aligned range fits.
    ///
    /// # Safety
    /// Call once during single-threaded startup, after the MEM2 arena is initialized and before
    /// any GX commands are emitted. No other code may use the allocated range or reprogram the
    /// GX FIFO and write-gather registers while this FIFO is active.
    #[cfg(not(gamecube))]
    pub unsafe fn initialize_mem2(size: u32) -> Result<Self, GxFifoError> {
        if !(GX_FIFO_MINSIZE..=GX_FIFO_MAXSIZE).contains(&size) || size & 31 != 0 {
            return Err(GxFifoError::InvalidSize);
        }
        if [
            ogc_irq::IRQ_PI_PEFINISH,
            ogc_irq::IRQ_PI_PETOKEN,
            ogc_irq::IRQ_PI_CP,
        ]
        .into_iter()
        .any(|irq| ogc_irq::get_handler(irq).is_some())
        {
            return Err(GxFifoError::PeFinishInterruptInUse);
        }
        if GX_FIFO_ACTIVE
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .is_err()
        {
            return Err(GxFifoError::AlreadyActive);
        }

        let arena_lo = get_arena2_lo() as usize;
        let arena_hi = get_arena2_hi() as usize;
        let Some(base) = arena_lo.checked_add(31).map(|value| value & !31) else {
            GX_FIFO_ACTIVE.store(false, Ordering::Release);
            return Err(GxFifoError::InsufficientMem2);
        };
        let Some(end) = base.checked_add(size as usize) else {
            GX_FIFO_ACTIVE.store(false, Ordering::Release);
            return Err(GxFifoError::InsufficientMem2);
        };
        if base > arena_hi
            || end > arena_hi
            || end > u32::MAX as usize
            || fifo_register_state(base, size).is_none()
        {
            GX_FIFO_ACTIVE.store(false, Ordering::Release);
            return Err(GxFifoError::InsufficientMem2);
        }

        unsafe { set_arena2_lo(end as *mut c_void) };
        let base_ptr = base as *mut u8;
        unsafe { ptr::write_bytes(base_ptr, 0, size as usize) };
        unsafe { dc_flush_range_no_sync(base_ptr.cast(), size) };
        unsafe { initialize_fifo_registers(base_ptr, size) };
        unsafe { enable_write_gather_pipe() };
        unsafe { initialize_gx_registers() };
        if ensure_draw_done_queue() == 0 || !register_gx_interrupts() {
            unsafe { set_arena2_lo(arena_lo as *mut c_void) };
            GX_FIFO_ACTIVE.store(false, Ordering::Release);
            return Err(GxFifoError::PeFinishInterruptInUse);
        }

        Ok(Self {
            base: base_ptr,
            size,
            _not_send_sync: PhantomData,
        })
    }

    pub fn size(&self) -> u32 {
        self.size
    }

    pub fn address(&self) -> *mut u8 {
        self.base
    }
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
struct FifoRegisterState {
    physical_base: u32,
    physical_end: u32,
    high_watermark: u32,
    low_watermark: u32,
    physical_read_pointer: u32,
    physical_write_pointer: u32,
    pi_write_pointer: u32,
}

fn fifo_register_state(base: usize, size: u32) -> Option<FifoRegisterState> {
    if base == 0
        || base & 31 != 0
        || !(GX_FIFO_MINSIZE..=GX_FIFO_MAXSIZE).contains(&size)
        || size & 31 != 0
    {
        return None;
    }
    let virtual_end = base.checked_add(size as usize)?.checked_sub(4)?;
    if virtual_end > u32::MAX as usize {
        return None;
    }
    let physical_base = (base as u32) & 0x3fff_ffff;
    let physical_end = (virtual_end as u32) & 0x3fff_ffff;
    Some(FifoRegisterState {
        physical_base,
        physical_end,
        high_watermark: size - GX_FIFO_HIWATERMARK,
        low_watermark: (size >> 1) & 0x7fff_ffe0,
        physical_read_pointer: physical_base,
        physical_write_pointer: physical_base,
        pi_write_pointer: (physical_base & 0x1fff_ffe0) & !0x0400_0000,
    })
}

unsafe fn initialize_fifo_registers(base: *mut u8, size: u32) {
    let Some(regs) = fifo_register_state(base as usize, size) else {
        return;
    };

    // PI's CPU FIFO registers: buffer start, inclusive end, and write pointer.
    unsafe {
        let pi = GX_PI_FIFO_BASE as *mut u32;
        ptr::write_volatile(pi.add(3), regs.physical_base);
        ptr::write_volatile(pi.add(4), regs.physical_end);
        ptr::write_volatile(pi.add(5), regs.pi_write_pointer);
    }

    // CP's GP FIFO registers use low-half then high-half register pairs.
    unsafe {
        write_cp_u32(16, regs.physical_base);
        write_cp_u32(18, regs.physical_end);
        write_cp_u32(20, regs.high_watermark);
        write_cp_u32(22, regs.low_watermark);
        write_cp_u32(24, 0);
        write_cp_u32(26, regs.physical_write_pointer);
        write_cp_u32(28, regs.physical_read_pointer);
        ptr::write_volatile((GX_CP_BASE as *mut u16).add(2), 0x0003);
        ptr::write_volatile(
            (GX_CP_BASE as *mut u16).add(1),
            GX_CP_READ_ENABLE | GX_CP_FIFO_LINK,
        );
    }
    let snapshot = FifoSnapshot {
        buf_start: base as usize as u32,
        buf_end: base as usize as u32 + size - 4,
        size,
        hi_mark: regs.high_watermark,
        lo_mark: regs.low_watermark,
        rd_ptr: base as usize as u32,
        wt_ptr: base as usize as u32,
        rdwt_dst: 0,
        fifo_wrap: 0,
        cpufifo_ready: 1,
        gpfifo_ready: 1,
    };
    GX_CPU_FIFO_STATE.store(snapshot);
    GX_GP_FIFO_STATE.store(snapshot);
    GX_CPU_FIFO_READY.store(true, Ordering::Release);
    GX_GP_FIFO_READY.store(true, Ordering::Release);
    GX_CP_GP_LINKED.store(true, Ordering::Release);
    GX_CP_CONTROL.store(
        u32::from(GX_CP_READ_ENABLE | GX_CP_FIFO_LINK),
        Ordering::Release,
    );
    compiler_fence(Ordering::SeqCst);
}

unsafe fn write_cp_u32(index: usize, value: u32) {
    let cp = GX_CP_BASE as *mut u16;
    unsafe {
        ptr::write_volatile(cp.add(index), value as u16);
        ptr::write_volatile(cp.add(index + 1), (value >> 16) as u16);
    }
}

#[inline]
fn cp_reg(index: usize) -> *mut u16 {
    (GX_CP_BASE as *mut u16).wrapping_add(index)
}

#[inline]
fn read_cp_u16(index: usize) -> u16 {
    unsafe { ptr::read_volatile(cp_reg(index)) }
}

#[inline]
fn read_cp_pair(low: usize) -> u32 {
    u32::from(read_cp_u16(low)) | (u32::from(read_cp_u16(low + 1)) << 16)
}

unsafe fn enable_write_gather_pipe() {
    let hid2: u32;
    unsafe {
        core::arch::asm!("mtspr 921, {wpar}", wpar = in(reg) GX_PI_WPAR, options(nostack));
        core::arch::asm!("mfspr {hid2}, 920", hid2 = out(reg) hid2, options(nostack));
        core::arch::asm!(
            "mtspr 920, {hid2}",
            hid2 = in(reg) hid2 | 0x4000_0000,
            options(nostack)
        );
        core::arch::asm!("sync", options(nostack));
    }
}

/// Framebuffer dimensions and copy settings for the current GX display.
///
/// The caller must keep `xfb` allocated, 32-byte aligned, and addressable by GX for as long as
/// it can be copied to the display.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct FrameConfig {
    pub xfb: *mut c_void,
    pub width: u16,
    pub efb_height: u16,
    pub xfb_height: u16,
    pub copy_filter: GxCopyFilter,
    pub field_rendering: bool,
    pub half_aspect_ratio: bool,
}

#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct GxCopyFilter {
    pub antialias: bool,
    pub sample_pattern: [[u8; 2]; 12],
    pub vertical_enabled: bool,
    pub vertical_coefficients: [u8; 7],
}

impl FrameConfig {
    /// Create a copy configuration for the GX embedded framebuffer and a VI framebuffer.
    pub fn new(xfb: *mut c_void, width: u16, efb_height: u16, xfb_height: u16) -> Option<Self> {
        if xfb.is_null()
            || (xfb as usize) & 31 != 0
            || !valid_copy_width(width, GX_MAX_EFB_DIMENSION)
            || efb_height == 0
            || xfb_height == 0
            || efb_height > GX_MAX_EFB_DIMENSION
            || xfb_height > GX_MAX_EFB_DIMENSION
            || !fits_framebuffer(efb_height, xfb_height)
        {
            return None;
        }
        Some(Self {
            xfb,
            width,
            efb_height,
            xfb_height,
            copy_filter: GxCopyFilter {
                antialias: false,
                sample_pattern: [[0; 2]; 12],
                vertical_enabled: false,
                vertical_coefficients: [0; 7],
            },
            field_rendering: false,
            half_aspect_ratio: false,
        })
    }

    pub fn with_copy_filter(mut self, copy_filter: GxCopyFilter) -> Self {
        self.copy_filter = copy_filter;
        self
    }

    pub fn with_field_mode(mut self, field_rendering: bool, half_aspect_ratio: bool) -> Self {
        self.field_rendering = field_rendering;
        self.half_aspect_ratio = half_aspect_ratio;
        self
    }
}

fn copy_source_rect_words(left: u16, top: u16, width: u16, height: u16) -> Option<[u32; 2]> {
    if width == 0 || height == 0 || width > GX_MAX_EFB_DIMENSION || height > GX_MAX_EFB_DIMENSION {
        return None;
    }
    Some([
        ((GX_BP_COPY_SRC as u32) << 24) | (u32::from(top & 0x3ff) << 10) | u32::from(left & 0x3ff),
        ((GX_BP_COPY_SIZE as u32) << 24)
            | ((u32::from(height - 1) & 0x3ff) << 10)
            | (u32::from(width - 1) & 0x3ff),
    ])
}

fn copy_source_words(width: u16, height: u16) -> Option<[u32; 2]> {
    copy_source_rect_words(0, 0, width, height)
}

fn copy_dest_word(width: u16) -> u32 {
    ((GX_BP_COPY_DEST as u32) << 24) | (u32::from(width) >> 4)
}

fn image_tile_count(fmt: u32, width: u16, height: u16) -> (u32, u32, u32) {
    let (xshift, yshift) = match fmt {
        x if x == GX_TF_I4 as u32
            || x == GX_TF_IA4 as u32
            || x == GX_CTF_R4 as u32
            || x == GX_CTF_RA4 as u32
            || x == GX_CTF_Z4 as u32 =>
        {
            (3, 3)
        }
        x if x == GX_TF_Z8 as u32
            || x == GX_TF_I8 as u32
            || x == GX_CTF_A8 as u32
            || x == GX_CTF_R8 as u32
            || x == GX_CTF_G8 as u32
            || x == GX_CTF_B8 as u32
            || x == GX_CTF_Z8M as u32
            || x == GX_CTF_Z8L as u32 =>
        {
            (3, 2)
        }
        x if x == GX_TF_IA8 as u32
            || x == GX_CTF_RA8 as u32
            || x == GX_CTF_RG8 as u32
            || x == GX_CTF_GB8 as u32
            || x == GX_TF_Z16 as u32
            || x == GX_TF_Z24X8 as u32
            || x == GX_CTF_Z16L as u32
            || x == GX_TF_RGB565 as u32
            || x == GX_TF_RGB5A3 as u32
            || x == GX_TF_RGBA8 as u32 =>
        {
            (2, 2)
        }
        _ => (0, 0),
    };
    let width = u32::from(if width == 0 { 1 } else { width });
    let height = u32::from(if height == 0 { 1 } else { height });
    let xtiles = (width + ((1 << xshift) - 1)) >> xshift;
    let ytiles = (height + ((1 << yshift) - 1)) >> yshift;
    let zplanes = u32::from(fmt == GX_TF_RGBA8 as u32 || fmt == GX_TF_Z24X8 as u32) + 1;
    (xtiles, ytiles, zplanes)
}

fn copy_address_word(xfb: *mut c_void) -> u32 {
    let physical = (xfb as usize as u32) & 0x3fff_ffff;
    ((0x4b_u32) << 24) | ((physical >> 5) & 0x00ff_ffff)
}

fn solid_vcd_low() -> u32 {
    // GX_VA_POS (direct) at bits 9..10 and GX_VA_CLR0 (direct) at bits 13..14.
    (1 << 9) | (1 << 13)
}

fn textured_vcd_high() -> u32 {
    1 // GX_VA_TEX0 uses direct texture coordinates.
}

fn solid_vat_words() -> [u32; 3] {
    // Match HBC C's POS, CLR0, and TEX0 vertex stream for every quad.
    [0x4121_6009, 0x8000_0000, 0]
}

fn solid_channel_control() -> u32 {
    // Disable lighting and take the direct vertex color as the material color.
    // This matches GX_SetChanCtrl(GX_DISABLE, GX_SRC_REG, GX_SRC_VTX, GX_LIGHTNULL, ...).
    1
}

fn modulate_tev_words() -> [u32; 2] {
    // Texture color and alpha multiplied by the white vertex color.
    [0xc008_f8af, 0xc108_f2f0]
}

// These are the BP words emitted by load_tex_obj for the texture object.
fn tex_obj_load_words(
    snapshot: GxTexObjSnapshot,
    map_id: u8,
    region: Option<[u32; 2]>,
) -> Option<[u32; 6]> {
    if snapshot.image_ptr.is_null()
        || snapshot.image_ptr as usize & 31 != 0
        || snapshot.width == 0
        || snapshot.height == 0
        || snapshot.format != GX_TF_RGBA8
        || snapshot.tex_flag & 0x02 == 0
    {
        return None;
    }
    // Wii's default texture-region callback uses region map_id + 8 for RGBA8.
    // An installed callback may instead provide a caller-owned region.
    let [even_region, odd_region] = region.unwrap_or_else(|| {
        let cache_size = 3 << 15 | 3 << 18;
        [
            ((GX_RGBA8_TMEM_EVEN[map_id as usize] >> 5) & 0x7fff) | cache_size,
            ((GX_RGBA8_TMEM_ODD[map_id as usize] >> 5) & 0x7fff) | cache_size,
        ]
    });
    let registers = texture_map_registers(map_id)?;
    Some([
        (u32::from(registers[0]) << 24) | (snapshot.tex_filt & 0x00ff_ffff),
        (u32::from(registers[1]) << 24) | (snapshot.tex_lod & 0x00ff_ffff),
        (u32::from(registers[2]) << 24) | (snapshot.tex_size & 0x00ff_ffff),
        (u32::from(registers[3]) << 24) | (even_region & 0x00ff_ffff),
        (u32::from(registers[4]) << 24) | (odd_region & 0x00ff_ffff),
        (u32::from(registers[5]) << 24) | (snapshot.tex_maddr & 0x00ff_ffff),
    ])
}

fn textured_rect_scale_words(snapshot: GxTexObjSnapshot) -> [u32; 2] {
    texture_coord_scale_words(0, snapshot.width, snapshot.height, snapshot.tex_filt)
}

fn queue_texture_coord_scale(words: [u32; 2]) {
    GX_SU_SSIZE[0].store(words[0], Ordering::Relaxed);
    GX_SU_TSIZE[0].store(words[1], Ordering::Relaxed);
    GX_TEX_DIRTY_S.store(words[0], Ordering::Relaxed);
    GX_TEX_DIRTY_T.store(words[1], Ordering::Relaxed);
    GX_TEX_DIRTY.store(true, Ordering::Release);
}

fn update_su_size_word(current: u32, size: u16) -> u32 {
    (current & !0xffff) | u32::from(size.wrapping_sub(1))
}

fn update_su_flag_word(current: u32, bit: u32, enabled: u8) -> u32 {
    (current & !(1 << bit)) | ((u32::from(enabled) & 1) << bit)
}

fn write_su_word(word: u32) {
    load_bp_reg((word >> 24) as u8, word & 0x00ff_ffff);
}

fn indirect_texture_mask_from(gen_mode: u32, order: u32) -> u32 {
    let count = ((gen_mode >> 16) & 0x7).min(4);
    let mut mask = 0u32;
    let mut stage = 0u32;
    while stage < count {
        let texmap = (order >> (stage * 6)) & 0x7;
        mask |= 1 << texmap;
        stage += 1;
    }
    0x0f00_0000 | mask
}

fn indirect_texture_mask_word() -> u32 {
    indirect_texture_mask_from(
        GX_GEN_MODE.load(Ordering::Relaxed),
        GX_TEV_RAS_ORDER[2].load(Ordering::Relaxed),
    )
}

fn flush_pending_indirect_texture_mask() {
    if !GX_TEV_IND_MASK_DIRTY.swap(false, Ordering::Acquire) {
        return;
    }
    let next = indirect_texture_mask_word();
    let current = GX_TEV_IND_MASK.swap(next, Ordering::AcqRel);
    if current != next {
        write_bp_word(next);
    }
}

fn flush_texture_state() {
    flush_pending_indirect_texture_mask();
    write_bp_word(GX_TEV_IND_MASK.load(Ordering::Relaxed));
}

fn tex_region_ismipmap(region: &GxTexRegion) -> bool {
    region.val[3] != 0 && (region.val[2] as u8) != 0
}

fn tex_region_invalidation_words(region: &GxTexRegion) -> (u32, Option<u32>) {
    let even = region.val[0];
    let odd = region.val[1];
    let clamp_dim = |word: u32, shift: u32| -> i32 { (((word >> shift) & 0x7) as i32 - 1).max(0) };
    let cw_e = clamp_dim(even, 15);
    let ch_e = clamp_dim(even, 18);
    let cw_o = clamp_dim(odd, 15);
    let ch_o = clamp_dim(odd, 18);
    let mipmap = tex_region_ismipmap(region);

    let tmp = cw_e + ch_e;
    let mut size_a = tmp;
    if mipmap {
        size_a += cw_o + ch_o - 2;
    }
    let reg_a = 0x6600_0000 | ((even & 0x7fff) >> 6) | (((size_a as u32) & 0xf) << 9);

    let reg_b = if cw_o != 0 {
        let mut size_b = cw_o + ch_o;
        if mipmap {
            size_b += tmp - 2;
        }
        Some(0x6600_0000 | ((odd & 0x7fff) >> 6) | (((size_b as u32) & 0xf) << 9))
    } else {
        None
    };
    (reg_a, reg_b)
}

fn tlut_region_update_loaded(region: &mut GxTlutRegion, tlut: &GxTlutObjRaw) {
    region.val[1] = (tlut.tlut_fmt & !0x3ff) | (region.val[0] & 0x3ff);
    region.val[2] = tlut.tlut_maddr;
    region.val[3] = (region.val[3] & 0x0000_ffff) | (u32::from(tlut.tlut_nentries) << 16);
}

fn preload_tile_shifts(fmt: u32) -> (u32, u32) {
    match fmt {
        x if x == GX_TF_I4 as u32
            || x == GX_TF_IA4 as u32
            || x == GX_TF_CI4 as u32
            || x == GX_TF_CMPR as u32 =>
        {
            (3, 3)
        }
        x if x == GX_TF_I8 as u32 || x == GX_TF_CI8 as u32 => (3, 2),
        x if x == GX_TF_IA8 as u32
            || x == GX_TF_RGB5A3 as u32
            || x == GX_TF_RGB565 as u32
            || x == GX_TF_CI14 as u32 =>
        {
            (2, 2)
        }
        _ => (0, 0),
    }
}

fn solid_blend_mode() -> u32 {
    // Blend, source alpha, inverse source alpha, clear op, and color/alpha updates enabled.
    1 | (1 << 2) | (5 << 5) | (4 << 8) | (1 << 3) | (1 << 4)
}

fn rectangle_vertices(x: f32, y: f32, width: f32, height: f32, z: f32) -> Option<[[f32; 3]; 4]> {
    if !x.is_finite()
        || !y.is_finite()
        || !width.is_finite()
        || !height.is_finite()
        || !z.is_finite()
        || width <= 0.0
        || height <= 0.0
        || !(x + width).is_finite()
        || !(y + height).is_finite()
    {
        return None;
    }
    Some([
        [x, y + height, z],
        [x + width, y + height, z],
        [x + width, y, z],
        [x, y, z],
    ])
}

fn textured_rectangle_vertices(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    z: f32,
) -> Option<[([f32; 3], [f32; 2]); 4]> {
    let vertices = rectangle_vertices(x, y, width, height, z)?;
    // The y-down rectangle starts at the bottom left. Texture rows start at the top.
    let texcoords = [[0.0, 1.0], [1.0, 1.0], [1.0, 0.0], [0.0, 0.0]];
    Some(core::array::from_fn(|index| {
        (vertices[index], texcoords[index])
    }))
}

fn load_cp_reg(register: u8, value: u32) {
    pipe_u8(0x08);
    pipe_u8(register);
    pipe_u32(value);
}

unsafe fn load_matrix_index(register: u16, value: u32) {
    load_xf_regs(register, &[value]);
}

fn matrix_4x3_words(matrix: &Mtx) -> [u32; 12] {
    let mut words = [0u32; 12];
    let mut out = 0usize;
    for row in matrix {
        for value in row {
            words[out] = value.to_bits();
            out += 1;
        }
    }
    words
}

fn matrix_3x3_words(matrix: &Mtx33) -> [u32; 9] {
    let mut words = [0u32; 9];
    let mut out = 0usize;
    for row in matrix {
        for value in row {
            words[out] = value.to_bits();
            out += 1;
        }
    }
    words
}

fn normal_from_4x3_words(matrix: &Mtx) -> [u32; 9] {
    let mut words = [0u32; 9];
    let mut out = 0usize;
    for row in matrix {
        for value in &row[..3] {
            words[out] = value.to_bits();
            out += 1;
        }
    }
    words
}

fn matrix_2x4_words(matrix: &Mtx) -> [u32; 8] {
    let mut words = [0u32; 8];
    let mut out = 0usize;
    for row in &matrix[..2] {
        for value in row {
            words[out] = value.to_bits();
            out += 1;
        }
    }
    words
}

/// Configure the vertex, channel, TEV, and blend state for HBC-style solid 2D shapes.
///
/// The vertex stream keeps HBC C's direct TEX0 attribute. The solid TEV operation ignores the
/// sampled texture. The caller supplies the orthographic projection matrix.
///
/// # Safety
/// The GX FIFO and CPU write-gather pipe must be initialized. The caller must not change this
/// state while a primitive is in progress.
pub unsafe fn configure_solid_2d(projection: &Mtx44) {
    unsafe { configure_2d_state(projection, false) };
}

/// Configure the vertex, channel, TEV, texture-coordinate, and blend state for textured 2D quads.
///
/// The renderer must load each texture object before drawing its quad. The texture image must be
/// GX tiled RGBA8 data in aligned, flushed memory.
///
/// # Safety
/// The GX FIFO and CPU write-gather pipe must be initialized. The caller must not change this
/// state while a primitive is in progress.
pub unsafe fn configure_textured_2d(projection: &Mtx44) {
    unsafe { configure_2d_state(projection, true) };
}

unsafe fn configure_2d_state(projection: &Mtx44, textured: bool) {
    unsafe {
        let vcd_lo = solid_vcd_low();
        let vcd_hi = textured_vcd_high();
        let vat = solid_vat_words();
        GX_VCD_LO.store(vcd_lo, Ordering::Relaxed);
        GX_VCD_HI.store(vcd_hi, Ordering::Relaxed);
        GX_VCD_NRMS.store(0, Ordering::Relaxed);
        GX_VCD_DIRTY.store(false, Ordering::Relaxed);
        GX_VAT0[0].store(vat[0], Ordering::Relaxed);
        GX_VAT1[0].store(vat[1], Ordering::Relaxed);
        GX_VAT2[0].store(vat[2], Ordering::Relaxed);
        GX_VAT_DIRTY.fetch_and(!1, Ordering::Relaxed);
        load_cp_reg(GX_CP_VCD_LOW, vcd_lo);
        load_cp_reg(GX_CP_VCD_HIGH, vcd_hi);
        load_cp_reg(GX_CP_VAT0, vat[0]);
        load_cp_reg(GX_CP_VAT1, vat[1]);
        load_cp_reg(GX_CP_VAT2, vat[2]);

        let position_identity = [
            1.0_f32.to_bits(),
            0,
            0,
            0,
            0,
            1.0_f32.to_bits(),
            0,
            0,
            0,
            0,
            1.0_f32.to_bits(),
            0,
        ];
        load_xf_regs(0x0000, &position_identity);
        let matrix_index = matrix_index_word();
        GX_MTX_IDX_LO.store(matrix_index, Ordering::Relaxed);
        GX_MTX_IDX_DIRTY.store(false, Ordering::Relaxed);
        load_matrix_index(0x1018, matrix_index);
        load_cp_reg(0x30, matrix_index);
        load_projection_mtx(projection, GX_ORTHOGRAPHIC);
        // INVTXSPEC tells XF which vertex colors and texture coordinates CP sends.
        // XF_NUMCOLORS and XF_NUMTEX configure generated outputs; they do not replace it.
        load_xf_regs(0x1008, &[vertex_spec()]);
        // Match GX_Init's __GX_InitGX call to GX_SetClipMode(GX_CLIP_ENABLE).
        load_xf_regs(0x1005, &[u32::from(GX_CLIP_ENABLE)]);
        load_xf_regs(0x1009, &[1]); // XF_NUMCOLORS must match VCD's direct color 0.

        let gen_mode = solid_generation_mode();
        GX_GEN_MODE.store(gen_mode, Ordering::Relaxed);
        GX_GEN_MODE_DIRTY.store(false, Ordering::Relaxed);
        load_bp_reg(0x00, gen_mode);

        let tev_order = tev_order_word();
        GX_TEV_RAS_ORDER[3].store(tev_order, Ordering::Relaxed);
        load_bp_reg(GX_BP_TEV_ORDER, tev_order);
        load_xf_regs(0x103f, &[TEXTURE_GENERATOR_COUNT]);
        load_xf_regs(POST_IDENTITY_ADDRESS, &position_identity);
        let texgen = textured_texgen_words();
        load_xf_regs(0x1040, &texgen[..1]);
        load_xf_regs(0x1050, &texgen[1..]);
        for (index, word) in identity_swap_words().into_iter().enumerate() {
            GX_TEV_SWAP_MODE_TABLE[index].store(word, Ordering::Relaxed);
            load_bp_reg((word >> 24) as u8, word & 0x00ff_ffff);
        }
        let tev = if textured {
            modulate_tev_words()
        } else {
            pass_color_tev_words()
        };
        GX_TEV_COLOR_ENV[0].store(tev[0], Ordering::Relaxed);
        GX_TEV_ALPHA_ENV[0].store(tev[1], Ordering::Relaxed);
        load_bp_reg(0xc0, tev[0] & 0x00ff_ffff);
        load_bp_reg(0xc1, tev[1] & 0x00ff_ffff);
        load_xf_regs(0x100e, &[solid_channel_control()]);
        load_xf_regs(0x1010, &[solid_channel_control()]);
        load_bp_reg(0xf3, 0x003f_0000); // GX_ALWAYS, GX_AOP_AND, GX_ALWAYS
        let blend = solid_blend_mode();
        GX_PE_COLOR_MODE0.store(
            (u32::from(GX_BP_COLOR_MODE) << 24) | blend,
            Ordering::Relaxed,
        );
        load_bp_reg(GX_BP_COLOR_MODE, blend);
        flush();
    }
}

/// Draw one solid-color rectangle with the configured 2D vertex state.
///
/// The rectangle uses a y-down coordinate convention and four direct vertices. The projection
/// matrix controls the visible coordinate range and clipping.
///
/// # Safety
/// `configure_solid_2d` must have completed and the GX FIFO and write-gather pipe must remain
/// active. No other code may change vertex state during this call.
pub unsafe fn draw_solid_rect(
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    z: f32,
    color: GxColor,
) -> Result<(), GxRectError> {
    let Some(vertices) = rectangle_vertices(x, y, width, height, z) else {
        return Err(GxRectError::InvalidGeometry);
    };
    unsafe { begin(GX_QUADS, GX_VTXFMT0, 4) };
    for [vx, vy, vz] in vertices {
        unsafe {
            position3f32(vx, vy, vz);
            color4u8(color.r, color.g, color.b, color.a);
            tex_coord2f32(0.0, 0.0);
        }
    }
    unsafe { end() };
    Ok(())
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum GxRectError {
    InvalidGeometry,
}

/// Invalidate the GX texture cache after CPU texture data changes.
///
/// # Safety
/// The GX FIFO and CPU write-gather pipe must be initialized.
pub unsafe fn invalidate_texture_cache() {
    unsafe { invalidate_tex_all() };
}

/// Invalidates the complete GX texture cache, matching `GX_InvalidateTexAll`.
///
/// # Safety
/// GX must be initialized and the write-gather pipe must be active.
pub unsafe fn invalidate_tex_all() {
    flush_texture_state();
    write_bp_word(0x6600_1000);
    write_bp_word(0x6600_1100);
    flush_texture_state();
}

/// Invalidates the cache lines described by one GX texture region.
///
/// # Safety
/// `region` must point to a live `GxTexRegion` when non-null and GX must be initialized.
pub unsafe fn invalidate_tex_region(region: *mut GxTexRegion) -> Result<(), GxTextureError> {
    unsafe {
        let Some(region) = region.as_ref() else {
            return Err(GxTextureError::InvalidRegion);
        };

        let (reg_a, reg_b) = tex_region_invalidation_words(region);

        flush_texture_state();
        write_bp_word(reg_a);
        if let Some(word) = reg_b {
            write_bp_word(word);
        }
        flush_texture_state();
        Ok(())
    }
}

/// Loads a TLUT object through the installed TLUT-region callback.
///
/// # Safety
/// `obj` and the callback-returned region must remain valid for the duration of the call.
pub unsafe fn load_tlut(obj: *mut GxTlutObj, tlut_name: u32) -> Result<(), GxTextureError> {
    unsafe {
        if obj.is_null() {
            return Err(GxTextureError::InvalidObject);
        }
        let Some(callback) = TLUT_REGION_CB else {
            return Err(GxTextureError::InvalidRegion);
        };
        let region_ptr = callback(tlut_name);
        let Some(region) = region_ptr.as_mut() else {
            return Err(GxTextureError::InvalidRegion);
        };
        let tlut = &*(obj as *const GxTlutObjRaw);

        flush_texture_state();
        write_bp_word(tlut.tlut_maddr);
        write_bp_word(region.val[0]);
        flush_texture_state();
        tlut_region_update_loaded(region, tlut);
        Ok(())
    }
}

/// Loads a texture object into an explicitly supplied texture region.
///
/// # Safety
/// `obj`, `region`, and any TLUT region returned by the installed callback must be valid.
pub unsafe fn load_tex_obj_preloaded(
    obj: *mut GxTexObj,
    region: *mut GxTexRegion,
    map_id: u8,
) -> Result<(), GxTextureError> {
    unsafe {
        if obj.is_null() {
            return Err(GxTextureError::InvalidObject);
        }
        let Some(registers) = texture_map_registers(map_id) else {
            return Err(GxTextureError::UnsupportedObject);
        };
        let Some(region) = region.as_mut() else {
            return Err(GxTextureError::InvalidRegion);
        };
        let ptr = tex_obj_mut(obj);

        ptr.tex_filt = (ptr.tex_filt & 0x00ff_ffff) | (u32::from(registers[0]) << 24);
        ptr.tex_lod = (ptr.tex_lod & 0x00ff_ffff) | (u32::from(registers[1]) << 24);
        ptr.tex_size = (ptr.tex_size & 0x00ff_ffff) | (u32::from(registers[2]) << 24);
        region.val[0] = (region.val[0] & 0x00ff_ffff) | (u32::from(registers[3]) << 24);
        region.val[1] = (region.val[1] & 0x00ff_ffff) | (u32::from(registers[4]) << 24);
        ptr.tex_maddr = (ptr.tex_maddr & 0x00ff_ffff) | (u32::from(registers[5]) << 24);

        write_bp_word(ptr.tex_filt);
        write_bp_word(ptr.tex_lod);
        write_bp_word(ptr.tex_size);
        write_bp_word(region.val[0]);
        write_bp_word(region.val[1]);
        write_bp_word(ptr.tex_maddr);

        if ptr.tex_flag & 0x02 == 0 {
            let Some(callback) = TLUT_REGION_CB else {
                return Err(GxTextureError::InvalidRegion);
            };
            let tlut_ptr = callback(ptr.tex_tlut);
            let Some(tlut) = tlut_ptr.as_mut() else {
                return Err(GxTextureError::InvalidRegion);
            };
            let tlut_reg = if map_id < 4 {
                0x98 + map_id
            } else {
                0xb4 + map_id
            };
            tlut.val[1] = (tlut.val[1] & 0x00ff_ffff) | (u32::from(tlut_reg) << 24);
            write_bp_word(tlut.val[1]);
        }
        Ok(())
    }
}

/// Preloads an entire texture (and its mip levels) into an explicit preload region.
///
/// # Safety
/// `obj` and `region` must point to live GX objects and GX must be initialized.
pub unsafe fn preload_entire_texture(
    obj: *mut GxTexObj,
    region: *mut GxTexRegion,
) -> Result<(), GxTextureError> {
    unsafe {
        if obj.is_null() {
            return Err(GxTextureError::InvalidObject);
        }
        let Some(region) = region.as_ref() else {
            return Err(GxTextureError::InvalidRegion);
        };
        let ptr = tex_obj_ref(obj);

        let mut reg_a = 0x6000_0000 | (ptr.tex_maddr & 0x00ff_ffff);
        let mut reg_b = 0x6100_0000 | (region.val[0] & 0x7fff);
        let mut reg_c = 0x6200_0000 | (region.val[1] & 0x7fff);
        let mut reg_d = 0x6300_0000
            | (u32::from(ptr.tex_tile_cnt) & 0x7fff)
            | ((u32::from(ptr.tex_tile_type) & 0x3) << 15);
        let fmt = (ptr.tex_size >> 20) & 0xf;

        flush_texture_state();
        write_bp_word(reg_a);
        write_bp_word(reg_b);
        write_bp_word(reg_c);
        write_bp_word(reg_d);

        let mut count = 0u32;
        let width = (ptr.tex_size & 0x3ff) + 1;
        let height = ((ptr.tex_size >> 10) & 0x3ff) + 1;
        if ptr.tex_flag & 0x01 != 0 {
            count = 31 - width.max(height).leading_zeros();
        }

        if count != 0 {
            let mut tmem_even = region.val[0] & 0xffff;
            let mut tmem_odd = region.val[1] & 0xffff;
            let mut maddr = ptr.tex_maddr & 0x00ff_ffff;
            let mut tile_count = u32::from(ptr.tex_tile_cnt);
            let mut level = 0u32;
            while count != 0 {
                if fmt == GX_TF_RGBA8 as u32 {
                    tmem_even = tmem_even.wrapping_add(tile_count);
                    tmem_odd = tmem_odd.wrapping_add(tile_count);
                    maddr = maddr.wrapping_add(tile_count << 1);
                } else {
                    maddr = maddr.wrapping_add(tile_count);
                    if level & 1 != 0 {
                        tmem_odd = tmem_odd.wrapping_add(tile_count);
                    } else {
                        tmem_even = tmem_even.wrapping_add(tile_count);
                    }
                }

                let (te, to) = if level & 1 != 0 {
                    (tmem_odd, tmem_even)
                } else {
                    (tmem_even, tmem_odd)
                };
                // Preserve libogc's source behavior: both mip dimensions are derived from width.
                let mut w = width >> (level + 1);
                let mut h = width >> (level + 1);
                if w == 0 {
                    w = 1;
                }
                if h == 0 {
                    h = 1;
                }
                let (xshift, yshift) = preload_tile_shifts(ptr.tex_fmt);

                reg_a = (reg_a & 0xff00_0000) | (maddr & 0x00ff_ffff);
                reg_b = (reg_b & !0x7fff) | (te & 0x7fff);
                reg_c = (reg_c & !0x7fff) | (to & 0x7fff);
                tile_count =
                    ((w + (1 << xshift) - 1) >> xshift) * ((h + (1 << yshift) - 1) >> yshift);
                reg_d = (reg_d & !0x7fff) | (tile_count & 0x7fff);

                write_bp_word(reg_a);
                write_bp_word(reg_b);
                write_bp_word(reg_c);
                write_bp_word(reg_d);
                level += 1;
                count -= 1;
            }
        }
        flush_texture_state();
        Ok(())
    }
}

/// Load a GX RGBA8 texture object into one of the GX texture-map slots.
///
/// # Safety
/// The mutable object and its aligned image data must remain valid while the GX commands can use
/// them. An installed texture-region callback must return a valid region or null.
pub unsafe fn load_tex_obj(obj: *mut GxTexObj, map_id: u8) -> Result<(), GxTextureError> {
    if unsafe { tex_obj_snapshot(obj) }.is_none() {
        return Err(GxTextureError::InvalidObject);
    }
    if texture_map_registers(map_id).is_none() {
        return Err(GxTextureError::UnsupportedObject);
    }
    let region = unsafe { TEX_REGION_CB };
    let region = match region {
        Some(callback) => {
            let region = unsafe { callback(obj, map_id) };
            if region.is_null() {
                return Err(GxTextureError::InvalidRegion);
            }
            let region = unsafe { &*region };
            Some([region.val[0], region.val[1]])
        }
        None => None,
    };
    // Match GX_LoadTexObj: the callback runs before the loader reads the object fields.
    let snapshot = unsafe { tex_obj_snapshot(obj) }.ok_or(GxTextureError::InvalidObject)?;
    let words =
        tex_obj_load_words(snapshot, map_id, region).ok_or(GxTextureError::UnsupportedObject)?;
    for word in words {
        load_bp_reg((word >> 24) as u8, word & 0x00ff_ffff);
    }
    Ok(())
}

#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub enum GxTextureError {
    InvalidObject,
    InvalidRegion,
    UnsupportedObject,
    InvalidGeometry,
}

/// Draw one textured RGBA8 quad in the configured y-down 2D state.
///
/// # Safety
/// `configure_textured_2d` must have completed and the texture image must remain live until GX
/// has finished reading it. The GX FIFO and write-gather pipe must remain active.
pub unsafe fn draw_textured_rect(
    obj: *mut GxTexObj,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    z: f32,
) -> Result<(), GxTextureError> {
    let Some(vertices) = textured_rectangle_vertices(x, y, width, height, z) else {
        return Err(GxTextureError::InvalidGeometry);
    };
    unsafe { load_tex_obj(obj, 0)? };
    let snapshot = unsafe { tex_obj_snapshot(obj) }.ok_or(GxTextureError::InvalidObject)?;
    queue_texture_coord_scale(textured_rect_scale_words(snapshot));
    unsafe { begin(GX_QUADS, GX_VTXFMT0, 4) };
    for (vertex, texcoord) in vertices {
        unsafe {
            position3f32(vertex[0], vertex[1], vertex[2]);
            color4u8(255, 255, 255, 255);
            tex_coord2f32(texcoord[0], texcoord[1]);
        }
    }
    unsafe { end() };
    Ok(())
}

fn copy_filter_words(filter: GxCopyFilter) -> [u32; 6] {
    let aa_samples = if filter.antialias {
        core::array::from_fn(|register| {
            let sample_start = register * 3;
            let mut value = 0x0100_0000 * (register as u32 + 1);
            for sample in 0..3 {
                let [x, y] = filter.sample_pattern[sample_start + sample];
                value |= u32::from(x & 0x0f) << (sample * 8);
                value |= u32::from(y & 0x0f) << (sample * 8 + 4);
            }
            value
        })
    } else {
        [0x0166_6666, 0x0266_6666, 0x0366_6666, 0x0466_6666]
    };
    let vertical_filter = if filter.vertical_enabled {
        let mut first = 0x5300_0000;
        let mut second = 0x5400_0000;
        for (index, coefficient) in filter.vertical_coefficients.into_iter().enumerate() {
            let shift = if index < 4 {
                index * 6
            } else {
                (index - 4) * 6
            };
            let value = u32::from(coefficient & 0x3f) << shift;
            if index < 4 {
                first |= value;
            } else {
                second |= value;
            }
        }
        [first, second]
    } else {
        [0x5359_5000, 0x5400_0015]
    };
    [
        aa_samples[0],
        aa_samples[1],
        aa_samples[2],
        aa_samples[3],
        vertical_filter[0],
        vertical_filter[1],
    ]
}

fn tmem_config_words() -> [u32; 16] {
    [
        0x8c0d_8000,
        0x900d_c000,
        0x8d0d_8800,
        0x910d_c800,
        0x8e0d_9000,
        0x920d_d000,
        0x8f0d_9800,
        0x930d_d800,
        0xac0d_a000,
        0xb00d_c400,
        0xad0d_a800,
        0xb10d_cc00,
        0xae0d_b000,
        0xb20d_d400,
        0xaf0d_b800,
        0xb30d_dc00,
    ]
}

unsafe extern "C" fn default_tex_region_callback(
    obj: *mut GxTexObj,
    mapid: u8,
) -> *mut GxTexRegion {
    if obj.is_null() || mapid >= 8 {
        return ptr::null_mut();
    }
    let fmt = unsafe { get_tex_obj_fmt(obj) };
    let base = core::ptr::addr_of_mut!(GX_DEFAULT_TEX_REGIONS).cast::<GxTexRegion>();
    if ogc_system::get_hollywood_revision() == 0 {
        let index = if fmt == u32::from(GX_TF_CI4)
            || fmt == u32::from(GX_TF_CI8)
            || fmt == u32::from(GX_TF_CI14)
        {
            (GX_GC_TEX_REGION_B.fetch_add(1, Ordering::AcqRel) & 3) as usize + 8
        } else {
            (GX_GC_TEX_REGION_A.fetch_add(1, Ordering::AcqRel) & 7) as usize
        };
        return unsafe { base.add(index) };
    }

    let mipmap = unsafe { get_tex_obj_mip_map(obj) };
    let index = if (u32::from(GX_TF_CI4)..=u32::from(GX_TF_CI14)).contains(&fmt) {
        usize::from(mapid)
    } else if mipmap != 0 {
        usize::from(mapid) + 16
    } else if fmt == u32::from(GX_TF_CMPR) {
        usize::from(mapid)
    } else {
        usize::from(mapid) + 8
    };
    unsafe { base.add(index) }
}

unsafe extern "C" fn default_tlut_region_callback(tlut_name: u32) -> *mut GxTlutRegion {
    if tlut_name >= 20 {
        return ptr::null_mut();
    }
    unsafe {
        core::ptr::addr_of_mut!(GX_DEFAULT_TLUT_REGIONS)
            .cast::<GxTlutRegion>()
            .add(tlut_name as usize)
    }
}

unsafe fn initialize_default_texture_regions() {
    GX_GC_TEX_REGION_A.store(0, Ordering::Relaxed);
    GX_GC_TEX_REGION_B.store(0, Ordering::Relaxed);
    const REGION_ADDR: [u32; 48] = [
        0x00000000, 0x00010000, 0x00020000, 0x00030000, 0x00040000, 0x00050000, 0x00060000,
        0x00070000, 0x00008000, 0x00018000, 0x00028000, 0x00038000, 0x00048000, 0x00058000,
        0x00068000, 0x00078000, 0x00000000, 0x00090000, 0x00020000, 0x000b0000, 0x00040000,
        0x00098000, 0x00060000, 0x000b8000, 0x00080000, 0x00010000, 0x000a0000, 0x00030000,
        0x00088000, 0x00050000, 0x000a8000, 0x00070000, 0x00000000, 0x00090000, 0x00020000,
        0x000b0000, 0x00040000, 0x00090000, 0x00060000, 0x000b0000, 0x00080000, 0x00010000,
        0x000a0000, 0x00030000, 0x00080000, 0x00050000, 0x000a0000, 0x00070000,
    ];
    let tex = core::ptr::addr_of_mut!(GX_DEFAULT_TEX_REGIONS).cast::<GxTexRegion>();
    let tlut = core::ptr::addr_of_mut!(GX_DEFAULT_TLUT_REGIONS).cast::<GxTlutRegion>();
    if ogc_system::get_hollywood_revision() != 0 {
        for i in 0..8usize {
            unsafe {
                init_tex_cache_region(
                    tex.add(i),
                    GX_FALSE,
                    REGION_ADDR[i],
                    GX_TEXCACHE_32K,
                    REGION_ADDR[i + 8],
                    GX_TEXCACHE_32K,
                );
                init_tex_cache_region(
                    tex.add(i + 8),
                    GX_FALSE,
                    REGION_ADDR[i + 16],
                    GX_TEXCACHE_32K,
                    REGION_ADDR[i + 24],
                    GX_TEXCACHE_32K,
                );
                init_tex_cache_region(
                    tex.add(i + 16),
                    GX_TRUE,
                    REGION_ADDR[i + 32],
                    GX_TEXCACHE_32K,
                    REGION_ADDR[i + 40],
                    GX_TEXCACHE_32K,
                );
            }
        }
    } else {
        for i in 0..8usize {
            let even = (i as u32) << 15;
            unsafe {
                init_tex_cache_region(
                    tex.add(i),
                    GX_FALSE,
                    even,
                    GX_TEXCACHE_32K,
                    even + 0x0008_0000,
                    GX_TEXCACHE_32K,
                )
            };
        }
        for i in 0..4usize {
            let even = (0x08 + ((i as u32) << 1)) << 15;
            let odd = (0x09 + ((i as u32) << 1)) << 15;
            unsafe {
                init_tex_cache_region(
                    tex.add(i + 8),
                    GX_FALSE,
                    even,
                    GX_TEXCACHE_32K,
                    odd,
                    GX_TEXCACHE_32K,
                )
            };
        }
    }
    for i in 0..16usize {
        unsafe {
            init_tlut_region(
                tlut.add(i),
                0x000c_0000 + ((i as u32) << 13),
                GX_TLUT_256 as u8,
            )
        };
    }
    for i in 0..4usize {
        unsafe { init_tlut_region(tlut.add(i + 16), 0x000e_0000 + ((i as u32) << 15), 64) };
    }
    unsafe {
        set_tex_region_callback(Some(default_tex_region_callback));
        set_tlut_region_callback(Some(default_tlut_region_callback));
    }
}

unsafe fn initialize_gx_default_state() {
    let rmode = unsafe { ogc_video_sys::get_preferred_mode(ptr::null_mut()) };
    if rmode.is_null() {
        return;
    }
    let rmode = unsafe { &*rmode };
    let black = GxColor {
        r: 0,
        g: 0,
        b: 0,
        a: 255,
    };
    let white = GxColor {
        r: 255,
        g: 255,
        b: 255,
        a: 255,
    };
    let default_bg = GxColor {
        r: 64,
        g: 64,
        b: 64,
        a: 255,
    };
    let identity: Mtx = [
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ];

    unsafe { set_copy_clear(black, 0x00ff_ffff) };
    for coord in 0..8u8 {
        unsafe {
            set_tex_coord_gen2(
                u16::from(coord),
                GX_TG_MTX2X4,
                GX_TG_TEX0 + u32::from(coord),
                GX_IDENTITY,
                GX_FALSE as u32,
                GX_DTTIDENTITY,
            )
        };
    }
    unsafe {
        set_num_tex_gens(1);
        clear_vtx_desc();
        inv_vtx_cache();
        set_line_width(6, GX_TO_ZERO);
        set_point_size(6, GX_TO_ZERO)
    };
    for coord in 0..8u8 {
        unsafe { enable_tex_offsets(coord, GX_DISABLE, GX_DISABLE) }
    }
    unsafe {
        load_pos_mtx_imm(&identity, GX_PNMTX0);
        load_nrm_mtx_imm(&identity, GX_PNMTX0);
        set_current_mtx(GX_PNMTX0);
        load_tex_mtx_imm(&identity, GX_IDENTITY, GX_MTX3X4);
        load_tex_mtx_imm(&identity, GX_DTTIDENTITY, GX_MTX3X4);
        set_viewport(
            0.0,
            0.0,
            f32::from(rmode.fb_width),
            f32::from(rmode.efb_height),
            0.0,
            1.0,
        );
        set_co_planar(GX_DISABLE);
        set_cull_mode(GX_CULL_BACK);
        set_clip_mode(GX_CLIP_ENABLE);
        set_scissor(0, 0, u32::from(rmode.fb_width), u32::from(rmode.efb_height));
        set_scissor_box_offset(0, 0);
        set_num_chans(0);
        set_chan_ctrl(
            i32::from(GX_COLOR0A0),
            GX_DISABLE,
            GX_SRC_REG,
            GX_SRC_VTX,
            GX_LIGHTNULL as u8,
            GX_DF_NONE,
            GX_AF_NONE,
        );
        set_chan_amb_color(i32::from(GX_COLOR0A0), black);
        set_chan_mat_color(i32::from(GX_COLOR0A0), white);
        set_chan_ctrl(
            i32::from(GX_COLOR1A1),
            GX_DISABLE,
            GX_SRC_REG,
            GX_SRC_VTX,
            GX_LIGHTNULL as u8,
            GX_DF_NONE,
            GX_AF_NONE,
        );
        set_chan_amb_color(i32::from(GX_COLOR1A1), black);
        set_chan_mat_color(i32::from(GX_COLOR1A1), white);
        invalidate_tex_all();
    }
    unsafe { initialize_default_texture_regions() };
    for stage in 0..16u8 {
        let (coord, map, color) = if stage < 8 {
            (stage, u32::from(stage), GX_COLOR0A0)
        } else {
            (GX_TEXCOORDNULL as u8, GX_TEXMAP_NULL, GX_COLORNULL)
        };
        unsafe { set_tev_order(stage, coord, map, color) };
    }
    unsafe {
        set_num_tev_stages(1);
        set_tev_op(0, GX_REPLACE);
        set_alpha_compare(GX_ALWAYS, 0, 0, GX_ALWAYS, 0);
        set_z_texture(GX_ZT_DISABLE, GX_TF_Z8, 0);
    }
    for stage in 0..16u8 {
        unsafe {
            set_tev_k_color_sel(stage, 0x06);
            set_tev_k_alpha_sel(stage, 0x00);
            set_tev_swap_mode(stage, GX_TEV_SWAP0, GX_TEV_SWAP0);
        }
    }
    unsafe {
        set_tev_swap_mode_table(0, GX_CH_RED, GX_CH_GREEN, GX_CH_BLUE, GX_CH_ALPHA);
        set_tev_swap_mode_table(1, GX_CH_RED, GX_CH_RED, GX_CH_RED, GX_CH_ALPHA);
        set_tev_swap_mode_table(2, GX_CH_GREEN, GX_CH_GREEN, GX_CH_GREEN, GX_CH_ALPHA);
        set_tev_swap_mode_table(3, GX_CH_BLUE, GX_CH_BLUE, GX_CH_BLUE, GX_CH_ALPHA);
    }
    for stage in 0..16u8 {
        unsafe { set_tev_direct(stage) }
    }
    unsafe {
        set_num_ind_stages(0);
        for stage in 0..4u8 {
            set_ind_tex_coord_scale(stage, GX_ITS_1, GX_ITS_1);
        }
        set_fog(GX_FOG_NONE, 0.0, 1.0, 0.1, 1.0, black);
        set_fog_range_adj(GX_DISABLE, 0, ptr::null());
        set_blend_mode(GX_BM_NONE, GX_BL_SRCALPHA, GX_BL_INVSRCALPHA, GX_LO_CLEAR);
        set_color_update(GX_ENABLE);
        set_alpha_update(GX_ENABLE);
        set_z_mode(GX_ENABLE, GX_LEQUAL, GX_TRUE);
        set_z_comp_loc(GX_TRUE);
        set_dither(GX_ENABLE);
        set_dst_alpha(GX_DISABLE, 0);
        set_pixel_fmt(GX_PF_RGB8_Z24, GX_ZC_LINEAR);
        set_field_mask(GX_ENABLE, GX_ENABLE);
        let half_aspect = u8::from(rmode.vi_height == rmode.xfb_height.saturating_mul(2));
        set_field_mode(rmode.field_rendering, half_aspect);
        set_copy_clear(default_bg, 0x00ff_ffff);
        set_disp_copy_src(0, 0, rmode.fb_width, rmode.efb_height);
        set_disp_copy_dst(rmode.fb_width, rmode.efb_height);
        set_disp_copy_y_scale(1.0);
        set_copy_clamp(GX_CLAMP_TOP | GX_CLAMP_BOTTOM);
        set_copy_filter(GX_FALSE, ptr::null(), GX_FALSE, ptr::null());
        set_disp_copy_gamma(GX_GM_1_0);
        set_disp_copy_frame2_field(GX_COPY_PROGRESSIVE);
        clear_bounding_box();
        poke_color_update(GX_TRUE);
        poke_alpha_update(GX_TRUE);
        poke_dither(GX_FALSE);
        poke_blend_mode(GX_BM_NONE, GX_BL_ZERO, GX_BL_ONE, GX_LO_SET);
        poke_alpha_mode(GX_ALWAYS, 0);
        poke_alpha_read(GX_READ_FF);
        poke_dst_alpha(GX_DISABLE, 0);
        poke_z_mode(GX_TRUE, GX_ALWAYS, GX_TRUE);
    }
    set_gp_metric(GX_PERF0_NONE, GX_PERF1_NONE);
    clear_gp_metric();
}

unsafe fn initialize_gx_registers() {
    GX_SAVE_DL_CTX.store(1, Ordering::Relaxed);
    // Port the hardware defaults GX_Init establishes before __GX_InitGX.
    unsafe {
        load_cp_reg(0x20, 0);
        load_xf_regs(0x1006, &[0]);
        for register in 0x80..0x88 {
            load_cp_reg(register, 0x8000_0000);
        }
        load_xf_regs(0x1000, &[0x3f]);
        load_xf_regs(0x1012, &[1]);
        load_bp_reg(0x58, 0x0f);
        load_bp_reg(0x23, 0);
        load_bp_reg(0x24, 0);
        load_bp_reg(0x67, 0);
        load_bp_reg(0x0f, 0x00ff);
        for value in tmem_config_words() {
            load_bp_reg((value >> 24) as u8, value & 0x00ff_ffff);
        }
        flush();
        initialize_gx_default_state();
    }
}

fn copy_control_word(clear: bool, y_scale: u16) -> u32 {
    let y_scale_enable = scale_enable_bit(u32::from(y_scale));
    ((GX_BP_COPY_CONTROL as u32) << 24)
        | 0x4003 // Copy enable, clamp top, and clamp bottom.
        | 0x4000 // GX copy command enable.
        | y_scale_enable
        | (u32::from(clear) << 11)
}

/// Configure a basic RGB8/Z24 display copy for HBC-style 2D rendering.
///
/// This emits GX FIFO commands. It does not select or configure a VI mode and it does not create
/// the GP FIFO. The caller must initialize a working GX FIFO and configure the VI before use.
///
/// # Safety
/// GX must be initialized and the GP FIFO must be ready. `config.xfb` must remain valid and
/// 32-byte aligned.
pub unsafe fn configure_frame(config: FrameConfig) {
    unsafe { configure_frame_with_progress(config, |_| {}) };
}

/// Configure a display frame and call `progress` after each GX command group.
///
/// This lets startup diagnostics identify which register writes finish. The callback must return
/// promptly and must not submit GX commands.
///
/// # Safety
/// GX must be initialized and the GP FIFO must be ready. `config.xfb` must remain valid and
/// 32-byte aligned.
pub unsafe fn configure_frame_with_progress(
    config: FrameConfig,
    mut progress: impl FnMut(&'static str),
) {
    progress("GX VIEWPORT CALC START");
    let viewport = viewport_words(
        0.0,
        0.0,
        f32::from(config.width) + 0.05,
        f32::from(config.efb_height) + 0.05,
        0.0,
        1.0,
    );
    progress("GX VIEWPORT CALC DONE");
    unsafe { load_viewport_words_with_progress(&viewport, &mut progress) };
    progress("GX VIEWPORT DONE");

    unsafe {
        set_scissor(0, 0, u32::from(config.width), u32::from(config.efb_height));
    }
    progress("GX SCISSOR DONE");

    unsafe {
        set_scissor_box_offset(0, 0);
    }
    progress("GX SCISSOR OFFSET DONE");

    if let Some(words) = copy_source_words(config.width, config.efb_height) {
        load_bp_reg(GX_BP_COPY_SRC, words[0] & 0x00ff_ffff);
        load_bp_reg(GX_BP_COPY_SIZE, words[1] & 0x00ff_ffff);
    }
    progress("GX COPY SOURCE DONE");

    load_bp_reg(GX_BP_COPY_DEST, copy_dest_word(config.width) & 0x00ff_ffff);
    progress("GX COPY DEST DONE");

    if let Some(y_scale) = copy_y_scale(config.efb_height, config.xfb_height) {
        load_bp_reg(GX_BP_COPY_Y_SCALE, u32::from(y_scale));
    }
    progress("GX COPY SCALE DONE");

    // GX_SetCopyFilter(GX_FALSE, NULL, GX_FALSE, NULL) from libogc. The
    // vertical filter coefficients are required even when AA and vertical
    // filtering are disabled; otherwise the EFB copy can produce black lines.
    for value in copy_filter_words(config.copy_filter) {
        load_bp_reg((value >> 24) as u8, value & 0x00ff_ffff);
    }
    progress("GX COPY FILTER DONE");

    // Match GX_Init's GX_SetZCompLoc(GX_TRUE), including the early-Z control bit.
    load_bp_reg(GX_BP_PIXEL_ENGINE, 0x40);
    progress("GX PIXEL ENGINE DONE");

    // Match GX_Init's GX_SetZMode(GX_ENABLE, GX_LEQUAL, GX_TRUE) baseline.
    load_bp_reg(GX_BP_Z_MODE, 0x17);
    load_bp_reg(GX_BP_COLOR_MODE, 0x1c);
    progress("GX DEPTH AND COLOR MODE DONE");

    // Match GX_SetFieldMask(GX_ENABLE, GX_ENABLE). The display copy uses
    // GX_COPY_PROGRESSIVE, which is encoded as zero in the copy-control word.
    load_bp_reg(GX_BP_FIELD_MASK, 0x03);
    progress("GX FIELD MASK DONE");

    // Match GX_SetFieldMode from HBC/libogc.
    load_bp_reg(
        0x22,
        (u32::from(config.half_aspect_ratio) << 22) & 0x00ff_ffff,
    );
    load_bp_reg(0x0f, 0x00ff);
    load_bp_reg(0x68, u32::from(config.field_rendering));
    load_bp_reg(0x0f, 0x00ff);
    progress("GX FRAME CONFIG DONE");
}

/// Set the background color and depth that a clearing display copy writes to the EFB.
///
/// # Safety
/// GX must be initialized and the GP FIFO must be ready.
pub unsafe fn set_copy_clear(color: GxColor, depth: u32) {
    load_bp_reg(
        GX_BP_COPY_CLEAR_AR,
        (u32::from(color.a) << 8) | u32::from(color.r),
    );
    load_bp_reg(
        GX_BP_COPY_CLEAR_GB,
        (u32::from(color.g) << 8) | u32::from(color.b),
    );
    load_bp_reg(GX_BP_COPY_CLEAR_Z, depth & 0x00ff_ffff);
}

/// Configures display-copy edge clamping.
/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_copy_clamp(clamp: u8) {
    let current = GX_DISP_COPY_CONTROL.load(Ordering::Relaxed);
    GX_DISP_COPY_CONTROL.store(
        update_bits(current, 0x3, u32::from(clamp & 3)),
        Ordering::Relaxed,
    );
}

/// Configures display-copy gamma.
/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_disp_copy_gamma(gamma: u8) {
    let current = GX_DISP_COPY_CONTROL.load(Ordering::Relaxed);
    GX_DISP_COPY_CONTROL.store(
        update_bits(current, 0x180, u32::from(gamma & 3) << 7),
        Ordering::Relaxed,
    );
}

/// Configures sample and vertical copy filters. Null tables are rejected only when the matching feature is enabled.
/// # Safety
/// GX must be initialized and exclusively owned. If `aa` is enabled, `sample_pattern` must contain 12 readable pairs. If `vf` is enabled, `vfilter` must contain 7 readable bytes.
pub unsafe fn set_copy_filter(aa: u8, sample_pattern: *const [u8; 2], vf: u8, vfilter: *const u8) {
    if (aa != 0 && sample_pattern.is_null()) || (vf != 0 && vfilter.is_null()) {
        return;
    }
    let mut filter = GxCopyFilter {
        antialias: aa != 0,
        sample_pattern: [[0; 2]; 12],
        vertical_enabled: vf != 0,
        vertical_coefficients: [0; 7],
    };
    if aa != 0 {
        for (i, sample) in filter.sample_pattern.iter_mut().enumerate() {
            *sample = unsafe { *sample_pattern.add(i) };
        }
    }
    if vf != 0 {
        for (i, coefficient) in filter.vertical_coefficients.iter_mut().enumerate() {
            *coefficient = unsafe { *vfilter.add(i) };
        }
    }
    for word in copy_filter_words(filter) {
        load_bp_reg((word >> 24) as u8, word & 0x00ff_ffff);
    }
}

/// Selects progressive/even/odd field copy behavior.
/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_disp_copy_frame2field(mode: u8) {
    let current = GX_DISP_COPY_CONTROL.load(Ordering::Relaxed);
    GX_DISP_COPY_CONTROL.store(
        update_bits(current, 0x3000, u32::from(mode & 3) << 12),
        Ordering::Relaxed,
    );
}

/// C-shaped snake-case counterpart for `GX_SetDispCopyFrame2Field`.
/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_disp_copy_frame2_field(mode: u8) {
    unsafe { set_disp_copy_frame2field(mode) };
}

/// Returns the C-compatible display-copy Y scale factor. Zero dimensions return 1.0 as a safety fallback.
pub fn get_y_scale_factor(efb_height: u16, xfb_height: u16) -> f32 {
    y_scale_factor(efb_height, xfb_height).unwrap_or(1.0)
}

/// Programs the display-copy Y scale and returns the resulting XFB line count.
/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_disp_copy_y_scale(yscale: f32) -> u32 {
    if !yscale.is_finite() || yscale <= 0.0 {
        return 0;
    }
    let y_scale = ((256.0 / yscale) as u32) & 0x1ff;
    load_bp_reg(GX_BP_COPY_Y_SCALE, y_scale);
    let current = GX_DISP_COPY_CONTROL.load(Ordering::Relaxed);
    GX_DISP_COPY_CONTROL.store(
        update_bits(current, 0x400, scale_enable_bit(y_scale)),
        Ordering::Relaxed,
    );
    let height = ((GX_DISP_COPY_WH.load(Ordering::Relaxed) >> 10) & 0x3ff) as u16 + 1;
    num_xfb_lines(height, y_scale)
}

/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_disp_copy_src(left: u16, top: u16, width: u16, height: u16) {
    let Some([tl, wh]) = copy_source_rect_words(left, top, width, height) else {
        return;
    };
    GX_DISP_COPY_TL.store(tl, Ordering::Relaxed);
    GX_DISP_COPY_WH.store(wh, Ordering::Relaxed);
}

/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_disp_copy_dst(width: u16, _height: u16) {
    GX_DISP_COPY_DST.store(copy_dest_word(width), Ordering::Relaxed);
}

/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_tex_copy_src(left: u16, top: u16, width: u16, height: u16) {
    let Some([tl, wh]) = copy_source_rect_words(left, top, width, height) else {
        return;
    };
    GX_TEX_COPY_TL.store(tl, Ordering::Relaxed);
    GX_TEX_COPY_WH.store(wh, Ordering::Relaxed);
}

/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_tex_copy_dst(width: u16, height: u16, fmt: u32, mipmap: u8) {
    let mut lfmt = (fmt & 0xf) as u8;
    let (xtiles, _ytiles, zplanes) = image_tile_count(fmt, width, height);
    let dst = ((GX_BP_COPY_DEST as u32) << 24) | ((xtiles * zplanes) & 0x3ff);
    GX_TEX_COPY_DST.store(dst, Ordering::Relaxed);
    if fmt == GX_TF_Z16 as u32 {
        lfmt = 11;
    }
    let mut control = GX_TEX_COPY_CONTROL.load(Ordering::Relaxed);
    let mode =
        if fmt == GX_CTF_YUVA8 as u32 || (fmt >= GX_TF_I4 as u32 && fmt < GX_TF_RGB565 as u32) {
            0x18000
        } else {
            0x10000
        };
    control = update_bits(control, 0x18000, mode);
    control = update_bits(control, 0x8, u32::from(lfmt & 8));
    control = update_bits(control, 0x200, u32::from(mipmap & 1) << 9);
    control = update_bits(control, 0x70, u32::from(lfmt & 7) << 4);
    GX_TEX_COPY_CONTROL.store(control, Ordering::Relaxed);
    GX_TEX_COPY_Z.store(fmt & 0x10 != 0, Ordering::Relaxed);
}

fn emit_copy_state(dest: *mut c_void, clear: u8, texture: bool) {
    let mut pe = GX_PE_CONTROL.load(Ordering::Relaxed);
    let zmode = GX_PE_Z_MODE.load(Ordering::Relaxed);
    let cmode = GX_PE_COLOR_MODE0.load(Ordering::Relaxed);
    if clear != 0 {
        load_bp_reg(0x40, (zmode & 0x00ff_fff0) | 0xf);
        load_bp_reg(0x41, cmode & 0x00ff_fffc);
    }
    let mut restore_pe = false;
    if texture && GX_TEX_COPY_Z.load(Ordering::Relaxed) && (pe & 7) != 3 {
        pe = (pe & !7) | 3;
        restore_pe = true;
    }
    if (clear != 0 || (pe & 7) == 3) && pe & 0x40 != 0 {
        pe &= !0x40;
        restore_pe = true;
    }
    if restore_pe {
        load_bp_reg(0x43, pe & 0x00ff_ffff);
    }

    let (tl, wh, dst_word, control_atomic) = if texture {
        (
            &GX_TEX_COPY_TL,
            &GX_TEX_COPY_WH,
            &GX_TEX_COPY_DST,
            &GX_TEX_COPY_CONTROL,
        )
    } else {
        (
            &GX_DISP_COPY_TL,
            &GX_DISP_COPY_WH,
            &GX_DISP_COPY_DST,
            &GX_DISP_COPY_CONTROL,
        )
    };
    for word in [
        tl.load(Ordering::Relaxed),
        wh.load(Ordering::Relaxed),
        dst_word.load(Ordering::Relaxed),
        copy_address_word(dest),
    ] {
        load_bp_reg((word >> 24) as u8, word & 0x00ff_ffff);
    }
    let mut control = control_atomic.load(Ordering::Relaxed);
    control = update_bits(control, 0x800, u32::from(clear != 0) << 11);
    control = update_bits(control, 0x4000, if texture { 0 } else { 0x4000 });
    control = update_bits(control, 0xff00_0000, 0x5200_0000);
    control_atomic.store(control, Ordering::Relaxed);
    load_bp_reg(0x52, control & 0x00ff_ffff);
    if clear != 0 {
        load_bp_reg(0x40, zmode & 0x00ff_ffff);
        load_bp_reg(0x41, cmode & 0x00ff_ffff);
    }
    if restore_pe {
        load_bp_reg(0x43, GX_PE_CONTROL.load(Ordering::Relaxed) & 0x00ff_ffff);
    }
}

/// # Safety
/// GX must be initialized and exclusively owned. A non-null `dest` must be 32-byte aligned and writable for the configured copy size until the GPU copy completes.
pub unsafe fn copy_disp(dest: *mut c_void, clear: u8) {
    if dest.is_null() {
        return;
    }
    emit_copy_state(dest, clear, false);
}

/// # Safety
/// GX must be initialized and exclusively owned. A non-null `dest` must be 32-byte aligned and writable for the configured copy size until the GPU copy completes.
pub unsafe fn copy_tex(dest: *mut c_void, clear: u8) {
    if dest.is_null() {
        return;
    }
    emit_copy_state(dest, clear, true);
}

/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn pix_mode_sync() {
    let word = GX_PE_CONTROL.load(Ordering::Relaxed);
    load_bp_reg((word >> 24) as u8, word & 0x00ff_ffff);
}

/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn clear_bounding_box() {
    load_bp_reg(0x55, 0x0003ff);
    load_bp_reg(0x56, 0x0003ff);
}

/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_field_mask(even_mask: u8, odd_mask: u8) {
    load_bp_reg(
        0x44,
        (u32::from(even_mask & 1) << 1) | u32::from(odd_mask & 1),
    );
}

/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_field_mode(field_mode: u8, half_aspect_ratio: u8) {
    let current = GX_LP_WIDTH.load(Ordering::Relaxed);
    let word = update_bits(current, 0x0040_0000, u32::from(half_aspect_ratio & 1) << 22);
    GX_LP_WIDTH.store(word, Ordering::Relaxed);
    load_bp_reg((word >> 24) as u8, word & 0x00ff_ffff);
    load_bp_reg(0x0f, 0x00ff);
    load_bp_reg(0x68, u32::from(field_mode & 1));
    load_bp_reg(0x0f, 0x00ff);
}

/// Copy the EFB to the configured XFB. When `clear` is true, clear the EFB after the copy.
///
/// # Safety
/// GX must be initialized, a display copy must be configured, and the XFB must remain valid,
/// writable by GX, and 32-byte aligned.
pub unsafe fn copy_display(config: FrameConfig, clear: bool) {
    let z_mode = GX_PE_Z_MODE.load(Ordering::Relaxed);
    let color_mode = GX_PE_COLOR_MODE0.load(Ordering::Relaxed);
    let pixel_engine = GX_PE_CONTROL.load(Ordering::Relaxed);
    let disable_early_z = clear || pixel_engine & 0x7 == 0x3;
    let restore_pixel_engine = disable_early_z && pixel_engine & 0x40 != 0;
    if clear {
        // GX_CopyDisp forces GX_ALWAYS while it clears the EFB.
        load_bp_reg(GX_BP_Z_MODE, (z_mode & !0x0f) | 0x0f);
        load_bp_reg(GX_BP_COLOR_MODE, color_mode & !0x03);
    }
    if restore_pixel_engine {
        // GX_CopyDisp disables early Z compare for clears and YUV pixel formats.
        load_bp_reg(
            GX_BP_PIXEL_ENGINE,
            copy_pixel_engine_word(pixel_engine, true) & 0x00ff_ffff,
        );
    }
    if let Some(words) = copy_source_words(config.width, config.efb_height) {
        load_bp_reg(GX_BP_COPY_SRC, words[0] & 0x00ff_ffff);
        load_bp_reg(GX_BP_COPY_SIZE, words[1] & 0x00ff_ffff);
    }
    load_bp_reg(GX_BP_COPY_DEST, copy_dest_word(config.width) & 0x00ff_ffff);
    load_bp_reg(0x4b, copy_address_word(config.xfb) & 0x00ff_ffff);
    let y_scale = copy_y_scale(config.efb_height, config.xfb_height).unwrap_or(256);
    load_bp_reg(
        GX_BP_COPY_CONTROL,
        copy_control_word(clear, y_scale) & 0x00ff_ffff,
    );
    if clear {
        // GX_CopyDisp restores the shadow state that was active before the copy.
        load_bp_reg(GX_BP_Z_MODE, z_mode & 0x00ff_ffff);
        load_bp_reg(GX_BP_COLOR_MODE, color_mode & 0x00ff_ffff);
    }
    if restore_pixel_engine {
        load_bp_reg(GX_BP_PIXEL_ENGINE, pixel_engine & 0x00ff_ffff);
    }
}

/// Queues the C `GX_SetDrawDone` completion event without waiting.
pub fn set_draw_done() {
    GX_DRAW_DONE.store(false, Ordering::Release);
    unsafe { finish_frame() };
}

/// Blocks until the PE finish interrupt reports completion.
pub fn wait_draw_done() {
    let queue = ensure_draw_done_queue();
    if queue == 0 {
        while !GX_DRAW_DONE.load(Ordering::Acquire) {
            core::hint::spin_loop();
        }
        return;
    }
    let irq_state = ogc_irq::irq_disable();
    while !GX_DRAW_DONE.load(Ordering::Acquire) {
        let _ = ogc_lwp_sys::thread_sleep(queue);
    }
    ogc_irq::irq_restore(irq_state);
}

/// Queues a draw-done event and blocks until it completes.
pub fn draw_done() {
    set_draw_done();
    wait_draw_done();
}

/// Queue a GX draw-done event after all earlier commands.
///
/// This returns after it queues the event. It does not wait for the GPU to finish. A blocking
/// completion wait needs the PE finish interrupt and the LWP synchronization APIs.
///
/// # Safety
/// GX must be initialized and the GP FIFO must be ready.
pub unsafe fn finish_frame() {
    unsafe {
        load_bp_reg(GX_BP_DRAW_DONE, 0x0000_0002);
        flush();
    }
}

/// Queue a GX draw-done event and wait for the PE finish interrupt.
///
/// # Safety
/// GX must be initialized by [`GxFifo::initialize_mem2`] and the GP FIFO must be ready.
pub unsafe fn finish_frame_and_wait() -> bool {
    GX_DRAW_DONE.store(false, Ordering::Release);
    unsafe { ptr::write_volatile(GX_PE_FINISH_STATUS, 0x000f) };
    unsafe { finish_frame() };
    for _ in 0..GX_DRAW_DONE_MAX_POLLS {
        if GX_DRAW_DONE.load(Ordering::Acquire) {
            return true;
        }
        core::hint::spin_loop();
    }
    false
}

/// Set up HBC-style 2D display-copy state and queue the two initial clearing copies used by HBC.
///
/// The caller must separately initialize the GX FIFO and configure the VI framebuffer. This
/// function sets only the GX viewport/scissor/copy registers, then waits for both initial clear
/// copies to complete.
///
/// # Safety
/// GX must be initialized and the GP FIFO must be ready. The framebuffer must remain valid,
/// writable by GX, and 32-byte aligned.
pub unsafe fn initialize_display(config: FrameConfig, background: GxColor) -> bool {
    unsafe { configure_frame(config) };
    unsafe { set_copy_clear(background, 0x00ff_ffff) };
    unsafe { copy_display(config, true) };
    if !unsafe { finish_frame_and_wait() } {
        return false;
    }
    // libogc GX_Init completes the first clearing copy before it queues the
    // second. The first copy clears EFB; the second copy publishes that clear.
    unsafe { copy_display(config, true) };
    unsafe { finish_frame_and_wait() }
}

/// Queue the configured EFB-to-XFB copy for one rendered frame.
///
/// # Safety
/// The display must be configured with `initialize_display`, and GX and its GP FIFO must remain
/// active. The caller must schedule the XFB for VI scanout after the copy completes.
pub unsafe fn present_frame(config: FrameConfig) {
    unsafe { copy_display(config, false) };
    unsafe { finish_frame() };
}

/// Queue the configured EFB-to-XFB copy and wait until GX completes the copy.
///
/// # Safety
/// GX must be initialized, the display copy must be configured, and the XFB must remain valid,
/// writable by GX, and 32-byte aligned.
pub unsafe fn present_frame_and_wait(config: FrameConfig) -> bool {
    unsafe { copy_display(config, false) };
    unsafe { finish_frame_and_wait() }
}

/// Flush pending GX commands through the write-gather FIFO.
///
/// # Safety
/// The CPU write-gather pipe must be enabled and GX must be initialized.
pub unsafe fn flush() {
    flush_pending_gx_state();
    for _ in 0..8 {
        pipe_u32(0);
    }
    // GX_Flush ends with ppcsync() in libogc. A compiler fence only constrains
    // Rust reordering; it does not drain the CPU write-gather pipe.
    unsafe { core::arch::asm!("sync", options(nostack, preserves_flags)) };
}

#[inline]
fn load_xf_regs(register: u16, values: &[u32]) {
    if values.is_empty() || values.len() > u16::MAX as usize {
        return;
    }
    pipe_u8(GX_CMD_LOAD_XF_REG);
    pipe_u32((((values.len() as u32 - 1) & 0xffff) << 16) | register as u32);
    for value in values {
        pipe_u32(*value);
    }
}

#[inline]
fn load_bp_reg(register: u8, value: u32) {
    pipe_u8(GX_CMD_LOAD_BP_REG);
    pipe_u32(((register as u32) << 24) | (value & 0x00ff_ffff));
}

#[inline]
fn write_bp_word(word: u32) {
    load_bp_reg((word >> 24) as u8, word & 0x00ff_ffff);
}

#[inline]
fn tev_color_id(color: u8) -> u8 {
    const MAP: [u8; 9] = [0, 1, 0, 1, 0, 1, 7, 5, 6];
    if color == GX_COLORNULL {
        GX_ALPHA_BUMP
    } else {
        MAP.get(usize::from(color))
            .copied()
            .unwrap_or(GX_ALPHA_BUMP)
    }
}

#[inline]
fn update_bits(word: u32, mask: u32, value: u32) -> u32 {
    (word & !mask) | (value & mask)
}

fn viewport_words(x: f32, y: f32, width: f32, height: f32, near_z: f32, far_z: f32) -> [u32; 6] {
    let x_scale = width * 0.5;
    let y_scale = height * -0.5;
    let z_near = 16_777_215.0 * near_z;
    let z_far = 16_777_215.0 * far_z;
    let z_scale = z_far - z_near;
    let x_origin = x + x_scale + 342.0;
    let y_origin = y + (height * 0.5) + 342.0;
    let z_origin = z_far;
    [
        x_scale.to_bits(),
        y_scale.to_bits(),
        z_scale.to_bits(),
        x_origin.to_bits(),
        y_origin.to_bits(),
        z_origin.to_bits(),
    ]
}

fn scissor_words(x: u32, y: u32, width: u32, height: u32) -> [u32; 2] {
    // BUG FIX: libogc emits scissor words for zero dimensions; u32 subtraction wraps.
    let left = x.wrapping_add(GX_SCISSOR_BIAS) & GX_SCISSOR_MASK;
    let top = y.wrapping_add(GX_SCISSOR_BIAS) & GX_SCISSOR_MASK;
    let right = x
        .wrapping_add(width.wrapping_sub(1))
        .wrapping_add(GX_SCISSOR_BIAS)
        & GX_SCISSOR_MASK;
    let bottom = y
        .wrapping_add(height.wrapping_sub(1))
        .wrapping_add(GX_SCISSOR_BIAS)
        & GX_SCISSOR_MASK;
    [
        ((GX_BP_SCISSOR_TL as u32) << 24) | (left << 12) | top,
        ((GX_BP_SCISSOR_BR as u32) << 24) | (right << 12) | bottom,
    ]
}

fn scissor_offset_word(x: i32, y: i32) -> u32 {
    let x_offset = (x.saturating_add(GX_SCISSOR_BIAS as i32) >> 1) as u32 & 0x3ff;
    let y_offset = (y.saturating_add(GX_SCISSOR_BIAS as i32) >> 1) as u32 & 0x3ff;
    (u32::from(GX_BP_SCISSOR_OFFSET) << 24) | (y_offset << 10) | x_offset
}

/// Sets the GX viewport transform.
///
/// # Safety
/// GX must be initialized and the GP FIFO must be ready to receive commands.
/// Matches `GX_SetViewportJitter`.
///
/// # Safety
/// GX must be initialized before this function writes XF state.
pub unsafe fn set_viewport_jitter(
    x_orig: f32,
    mut y_orig: f32,
    width: f32,
    height: f32,
    near_z: f32,
    far_z: f32,
    field: u32,
) {
    if field == 0 {
        y_orig -= 0.5;
    }
    let x0 = width * 0.5;
    let y0 = -height * 0.5;
    let x1 = x_orig + width * 0.5 + 342.0;
    let y1 = y_orig + height * 0.5 + 342.0;
    let n = 16_777_215.0 * near_z;
    let f = 16_777_215.0 * far_z;
    load_xf_regs(
        0x101a,
        &[
            x0.to_bits(),
            y0.to_bits(),
            (f - n).to_bits(),
            x1.to_bits(),
            y1.to_bits(),
            f.to_bits(),
        ],
    );
}

/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_viewport(x: f32, y: f32, width: f32, height: f32, near_z: f32, far_z: f32) {
    let words = viewport_words(x, y, width, height, near_z, far_z);
    load_xf_regs(GX_XF_VIEWPORT, &words);
}

unsafe fn load_viewport_words_with_progress(
    words: &[u32; 6],
    progress: &mut impl FnMut(&'static str),
) {
    pipe_u8(GX_CMD_LOAD_XF_REG);
    progress("GX VIEWPORT OPCODE SENT");
    pipe_u32((((words.len() as u32 - 1) & 0xffff) << 16) | GX_XF_VIEWPORT as u32);
    progress("GX VIEWPORT ADDRESS SENT");
    const WORD_STAGES: [&str; 6] = [
        "GX VIEWPORT WORD 1 SENT",
        "GX VIEWPORT WORD 2 SENT",
        "GX VIEWPORT WORD 3 SENT",
        "GX VIEWPORT WORD 4 SENT",
        "GX VIEWPORT WORD 5 SENT",
        "GX VIEWPORT WORD 6 SENT",
    ];
    for (word, stage) in words.iter().zip(WORD_STAGES) {
        pipe_u32(*word);
        progress(stage);
    }
}

/// Sets the scissor rectangle.
///
/// # Safety
/// GX must be initialized and the GP FIFO must be ready to receive commands.
pub unsafe fn set_scissor(x: u32, y: u32, width: u32, height: u32) {
    let words = scissor_words(x, y, width, height);
    load_bp_reg(GX_BP_SCISSOR_TL, words[0] & 0x00ff_ffff);
    load_bp_reg(GX_BP_SCISSOR_BR, words[1] & 0x00ff_ffff);
}

/// Set the GX scissor box offset.
///
/// # Safety
/// GX must be initialized and the GP FIFO must be ready to receive commands.
pub unsafe fn set_scissor_box_offset(x: i32, y: i32) {
    let word = scissor_offset_word(x, y);
    load_bp_reg(GX_BP_SCISSOR_OFFSET, word & 0x00ff_ffff);
}

/// Loads a perspective or orthographic projection matrix.
///
/// # Safety
/// GX must be initialized, the matrix must be valid, and the GP FIFO must be ready.
pub unsafe fn load_projection_mtx(matrix: &Mtx44, projection_type: u8) {
    let x = matrix[0];
    let y = matrix[1];
    let z = matrix[2];
    let values = [
        x[0].to_bits(),
        match projection_type {
            GX_PERSPECTIVE => x[2],
            GX_ORTHOGRAPHIC => x[3],
            _ => 0.0,
        }
        .to_bits(),
        y[1].to_bits(),
        match projection_type {
            GX_PERSPECTIVE => y[2],
            GX_ORTHOGRAPHIC => y[3],
            _ => 0.0,
        }
        .to_bits(),
        z[2].to_bits(),
        z[3].to_bits(),
        projection_type as u32,
    ];
    load_xf_regs(GX_XF_PROJECTION, &values);
}

#[repr(C)]
struct GxTexObjRaw {
    tex_filt: u32,
    tex_lod: u32,
    tex_size: u32,
    tex_maddr: u32,
    usr_data: u32,
    tex_fmt: u32,
    tex_tlut: u32,
    tex_tile_cnt: u16,
    tex_tile_type: u8,
    tex_flag: u8,
}

#[repr(C)]
struct GxTlutObjRaw {
    tlut_fmt: u32,
    tlut_maddr: u32,
    tlut_nentries: u16,
    _pad: [u8; 2],
}

#[repr(C)]
struct GxLightObjRaw {
    _pad: [u32; 3],
    col: u32,
    a0: f32,
    a1: f32,
    a2: f32,
    k0: f32,
    k1: f32,
    k2: f32,
    px: f32,
    py: f32,
    pz: f32,
    nx: f32,
    ny: f32,
    nz: f32,
}

fn capture_display_list_bytes(bytes: &[u8]) -> bool {
    if !GX_DL_ACTIVE.load(Ordering::Acquire) {
        return false;
    }
    let offset = GX_DL_OFFSET.load(Ordering::Relaxed);
    let size = GX_DL_SIZE.load(Ordering::Relaxed);
    let Some(end) = offset.checked_add(bytes.len() as u32) else {
        GX_DL_OVERFLOW.store(true, Ordering::Release);
        return true;
    };
    if end > size {
        GX_DL_OVERFLOW.store(true, Ordering::Release);
        return true;
    }
    let base = GX_DL_BASE.load(Ordering::Relaxed) as *mut u8;
    if base.is_null() {
        GX_DL_OVERFLOW.store(true, Ordering::Release);
        return true;
    }
    unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), base.add(offset as usize), bytes.len()) };
    GX_DL_OFFSET.store(end, Ordering::Release);
    true
}

#[inline]
fn pipe_u8(v: u8) {
    if capture_display_list_bytes(&[v]) {
        return;
    }
    unsafe { ptr::write_volatile(GP_PIPE_ADDR as *mut u8, v) };
}

#[inline]
fn pipe_u16(v: u16) {
    if capture_display_list_bytes(&v.to_be_bytes()) {
        return;
    }
    unsafe { ptr::write_volatile(GP_PIPE_ADDR as *mut u16, v) };
}

#[inline]
fn pipe_u32(v: u32) {
    if capture_display_list_bytes(&v.to_be_bytes()) {
        return;
    }
    unsafe { ptr::write_volatile(GP_PIPE_ADDR as *mut u32, v) };
}

#[inline]
fn pipe_f32(v: f32) {
    if capture_display_list_bytes(&v.to_bits().to_be_bytes()) {
        return;
    }
    unsafe { ptr::write_volatile(GP_PIPE_ADDR as *mut f32, v) };
}

#[inline]
unsafe fn tex_obj_mut(obj: *mut GxTexObj) -> &'static mut GxTexObjRaw {
    unsafe { &mut *(obj as *mut GxTexObjRaw) }
}

#[inline]
unsafe fn tex_obj_ref(obj: *const GxTexObj) -> &'static GxTexObjRaw {
    unsafe { &*(obj as *const GxTexObjRaw) }
}

#[inline]
unsafe fn tlut_obj_mut(obj: *mut GxTlutObj) -> &'static mut GxTlutObjRaw {
    unsafe { &mut *(obj as *mut GxTlutObjRaw) }
}

#[inline]
unsafe fn light_obj_mut(obj: *mut GxLightObj) -> &'static mut GxLightObjRaw {
    unsafe { &mut *(obj as *mut GxLightObjRaw) }
}

#[inline]
unsafe fn light_obj_ref(obj: *const GxLightObj) -> &'static GxLightObjRaw {
    unsafe { &*(obj as *const GxLightObjRaw) }
}

#[inline]
fn tex_filter_hw(minfilt: u8) -> u8 {
    match minfilt {
        GX_NEAR => 0x00,
        GX_LINEAR => 0x04,
        GX_NEAR_MIP_NEAR => 0x01,
        GX_LIN_MIP_NEAR => 0x05,
        GX_NEAR_MIP_LIN => 0x02,
        GX_LIN_MIP_LIN => 0x06,
        _ => 0x00,
    }
}

#[inline]
fn tex_size_shifts(fmt: u32) -> (u32, u32) {
    match fmt {
        value
            if value == GX_TF_I4 as u32
                || value == GX_TF_CMPR as u32
                || value == GX_CTF_R4 as u32
                || value == GX_CTF_RA4 as u32
                || value == GX_CTF_Z4 as u32 =>
        {
            (3, 3)
        }
        value
            if value == GX_TF_Z8 as u32
                || value == GX_TF_I8 as u32
                || value == GX_TF_IA4 as u32
                || value == GX_CTF_A8 as u32
                || value == GX_CTF_R8 as u32
                || value == GX_CTF_G8 as u32
                || value == GX_CTF_B8 as u32
                || value == GX_CTF_RG8 as u32
                || value == GX_CTF_GB8 as u32
                || value == GX_CTF_Z8M as u32
                || value == GX_CTF_Z8L as u32 =>
        {
            (3, 2)
        }
        _ => (2, 2),
    }
}

#[inline]
fn pack_tex_cache_size(size: u8) -> u32 {
    let sze = match size {
        GX_TEXCACHE_32K => 3,
        GX_TEXCACHE_128K => 4,
        GX_TEXCACHE_512K => 5,
        _ => 3,
    } as u32;
    ((sze & 0x7) << 15) | ((sze & 0x7) << 18)
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct GxTexObjInit {
    pub wd: u16,
    pub ht: u16,
    pub fmt: u8,
    pub wrap_s: u8,
    pub wrap_t: u8,
    pub mipmap: u8,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct GxTexObjLod {
    pub minfilt: u8,
    pub magfilt: u8,
    pub minlod: f32,
    pub maxlod: f32,
    pub lodbias: f32,
    pub biasclamp: u8,
    pub edgelod: u8,
    pub maxaniso: u8,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct GxTexObjInfo {
    pub image_ptr: *mut c_void,
    pub width: u16,
    pub height: u16,
    pub format: u8,
    pub wrap_s: u8,
    pub wrap_t: u8,
    pub mipmap: u8,
}

/// Optional writable destinations for a texture-object query.
#[derive(Clone, Copy, Debug, Default)]
pub struct GxTexObjOutputs {
    pub image_ptr: *mut *mut c_void,
    pub width: *mut u16,
    pub height: *mut u16,
    pub format: *mut u8,
    pub wrap_s: *mut u8,
    pub wrap_t: *mut u8,
    pub mipmap: *mut u8,
}

/// Indirect-texture settings for one TEV stage.
#[derive(Clone, Copy, Debug, Default)]
pub struct GxTevIndirect {
    pub indtexid: u8,
    pub format: u8,
    pub bias: u8,
    pub mtxid: u8,
    pub wrap_s: u8,
    pub wrap_t: u8,
    pub addprev: u8,
    pub utclod: u8,
    pub alpha: u8,
}

/// Tile and spacing settings for one indirect-texture TEV stage.
#[derive(Clone, Copy, Debug, Default)]
pub struct GxTevIndirectTile {
    pub indtexid: u8,
    pub tilesize_x: u16,
    pub tilesize_y: u16,
    pub tilespacing_x: u16,
    pub tilespacing_y: u16,
    pub indtexfmt: u8,
    pub indtexmtx: u8,
    pub bias_sel: u8,
    pub alpha_sel: u8,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct GxTexObjSnapshot {
    pub image_ptr: *mut c_void,
    pub width: u16,
    pub height: u16,
    pub format: u8,
    pub wrap_s: u8,
    pub wrap_t: u8,
    pub mipmap: u8,
    pub tex_filt: u32,
    pub tex_lod: u32,
    pub tex_tlut: u32,
    pub tex_tile_cnt: u16,
    pub tex_tile_type: u8,
    pub tex_flag: u8,
    pub tex_size: u32,
    pub tex_maddr: u32,
    pub user_data: *mut c_void,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq)]
pub struct GxLightObjSnapshot {
    pub color: GxColor,
    pub position: [f32; 3],
    pub direction: [f32; 3],
    pub attenuation: [f32; 3],
    pub attenuation_coeff: [f32; 3],
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq)]
pub struct GxTlutObjSnapshot {
    pub format: u32,
    pub image_ptr: *mut c_void,
    pub nentries: u16,
    pub tlut_fmt: u32,
    pub tlut_maddr: u32,
}

fn update_vcd_attr(attr: u8, ty: u8) {
    let mut lo = GX_VCD_LO.load(Ordering::Relaxed);
    let mut hi = GX_VCD_HI.load(Ordering::Relaxed);
    match attr {
        GX_VA_PTNMTXIDX..=GX_VA_TEX7MTXIDX => {
            let shift = u32::from(attr);
            lo = (lo & !(1 << shift)) | ((u32::from(ty) & 1) << shift);
        }
        GX_VA_POS => lo = (lo & !0x600) | ((u32::from(ty) & 3) << 9),
        GX_VA_NRM => {
            lo = (lo & !0x1800) | ((u32::from(ty) & 3) << 11);
            GX_VCD_NRMS.store(1, Ordering::Relaxed);
        }
        GX_VA_NBT => {
            lo = (lo & !0x1800) | ((u32::from(ty) & 3) << 11);
            GX_VCD_NRMS.store(2, Ordering::Relaxed);
        }
        GX_VA_CLR0 => lo = (lo & !0x6000) | ((u32::from(ty) & 3) << 13),
        GX_VA_CLR1 => lo = (lo & !0x18000) | ((u32::from(ty) & 3) << 15),
        GX_VA_TEX0..=GX_VA_TEX7 => {
            let shift = u32::from(attr - GX_VA_TEX0) * 2;
            hi = (hi & !(3 << shift)) | ((u32::from(ty) & 3) << shift);
        }
        _ => {}
    }
    GX_VCD_LO.store(lo, Ordering::Relaxed);
    GX_VCD_HI.store(hi, Ordering::Relaxed);
}

fn vcd_vertex_spec_word(lo: u32, hi: u32, nrms: u32) -> u32 {
    let cols = ((lo & 0x6000) != 0) as u32 + ((lo & 0x18000) != 0) as u32;
    let nrms = match nrms {
        1 => 1,
        2 => 2,
        _ => 0,
    };
    let mut texs = 0u32;
    for shift in (0..16).step_by(2) {
        texs += ((hi & (3 << shift)) != 0) as u32;
    }
    (texs << 4) | (nrms << 2) | cols
}

fn set_vat_attr(vtxfmt: u8, vtxattr: u32, comptype: u32, compsize: u32, frac: u32) {
    let vat = usize::from(vtxfmt & 7);
    let mut r0 = GX_VAT0[vat].load(Ordering::Relaxed);
    let mut r1 = GX_VAT1[vat].load(Ordering::Relaxed);
    let mut r2 = GX_VAT2[vat].load(Ordering::Relaxed);
    let mut valid = true;

    match vtxattr as u8 {
        GX_VA_POS if comptype <= GX_POS_XYZ && compsize <= GX_F32 => {
            r0 = (r0 & !0x1) | (comptype & 1);
            r0 = (r0 & !0xe) | ((compsize & 7) << 1);
            r0 = (r0 & !0x1f0) | ((frac & 0x1f) << 4);
        }
        GX_VA_NRM if comptype == GX_NRM_XYZ && matches!(compsize, GX_S8 | GX_S16 | GX_F32) => {
            r0 &= !0x200;
            r0 = (r0 & !0x1c00) | ((compsize & 7) << 10);
            r0 &= !0x8000_0000;
        }
        GX_VA_NBT
            if matches!(comptype, GX_NRM_NBT | GX_NRM_NBT3)
                && matches!(compsize, GX_S8 | GX_S16 | GX_F32) =>
        {
            r0 |= 0x200;
            r0 = (r0 & !0x1c00) | ((compsize & 7) << 10);
            r0 = (r0 & !0x8000_0000)
                | if comptype == GX_NRM_NBT3 {
                    0x8000_0000
                } else {
                    0
                };
        }
        GX_VA_CLR0 if comptype <= GX_CLR_RGBA && compsize <= GX_RGBA8 => {
            r0 = (r0 & !0x2000) | ((comptype & 1) << 13);
            r0 = (r0 & !0x1c000) | ((compsize & 7) << 14);
        }
        GX_VA_CLR1 if comptype <= GX_CLR_RGBA && compsize <= GX_RGBA8 => {
            r0 = (r0 & !0x20000) | ((comptype & 1) << 17);
            r0 = (r0 & !0x1c0000) | ((compsize & 7) << 18);
        }
        GX_VA_TEX0 if comptype <= GX_TEX_ST && compsize <= GX_F32 => {
            r0 = (r0 & !0x200000) | ((comptype & 1) << 21);
            r0 = (r0 & !0x1c00000) | ((compsize & 7) << 22);
            r0 = (r0 & !0x3e000000) | ((frac & 0x1f) << 25);
        }
        GX_VA_TEX1 if comptype <= GX_TEX_ST && compsize <= GX_F32 => {
            r1 = (r1 & !0x1) | (comptype & 1);
            r1 = (r1 & !0xe) | ((compsize & 7) << 1);
            r1 = (r1 & !0x1f0) | ((frac & 0x1f) << 4);
        }
        GX_VA_TEX2 if comptype <= GX_TEX_ST && compsize <= GX_F32 => {
            r1 = (r1 & !0x200) | ((comptype & 1) << 9);
            r1 = (r1 & !0x1c00) | ((compsize & 7) << 10);
            r1 = (r1 & !0x3e000) | ((frac & 0x1f) << 13);
        }
        GX_VA_TEX3 if comptype <= GX_TEX_ST && compsize <= GX_F32 => {
            r1 = (r1 & !0x40000) | ((comptype & 1) << 18);
            r1 = (r1 & !0x380000) | ((compsize & 7) << 19);
            r1 = (r1 & !0x7c00000) | ((frac & 0x1f) << 22);
        }
        GX_VA_TEX4 if comptype <= GX_TEX_ST && compsize <= GX_F32 => {
            r1 = (r1 & !0x8000000) | ((comptype & 1) << 27);
            r1 = (r1 & !0x70000000) | ((compsize & 7) << 28);
            r2 = (r2 & !0x1f) | (frac & 0x1f);
        }
        GX_VA_TEX5 if comptype <= GX_TEX_ST && compsize <= GX_F32 => {
            r2 = (r2 & !0x20) | ((comptype & 1) << 5);
            r2 = (r2 & !0x1c0) | ((compsize & 7) << 6);
            r2 = (r2 & !0x3e00) | ((frac & 0x1f) << 9);
        }
        GX_VA_TEX6 if comptype <= GX_TEX_ST && compsize <= GX_F32 => {
            r2 = (r2 & !0x4000) | ((comptype & 1) << 14);
            r2 = (r2 & !0x38000) | ((compsize & 7) << 15);
            r2 = (r2 & !0x7c0000) | ((frac & 0x1f) << 18);
        }
        GX_VA_TEX7 if comptype <= GX_TEX_ST && compsize <= GX_F32 => {
            r2 = (r2 & !0x800000) | ((comptype & 1) << 23);
            r2 = (r2 & !0x7000000) | ((compsize & 7) << 24);
            r2 = (r2 & !0xf8000000) | ((frac & 0x1f) << 27);
        }
        _ => valid = false,
    }
    if valid && frac != 0 && matches!(vtxattr as u8, GX_VA_POS | GX_VA_TEX0..=GX_VA_TEX7) {
        r0 |= 0x4000_0000;
    }
    GX_VAT0[vat].store(r0, Ordering::Relaxed);
    GX_VAT1[vat].store(r1, Ordering::Relaxed);
    GX_VAT2[vat].store(r2, Ordering::Relaxed);
}

fn flush_pending_vertex_state() {
    if GX_VCD_DIRTY.swap(false, Ordering::Acquire) {
        let lo = GX_VCD_LO.load(Ordering::Relaxed);
        let hi = GX_VCD_HI.load(Ordering::Relaxed);
        load_cp_reg(GX_CP_VCD_LOW, lo);
        load_cp_reg(GX_CP_VCD_HIGH, hi);
        load_xf_regs(
            0x1008,
            &[vcd_vertex_spec_word(
                lo,
                hi,
                GX_VCD_NRMS.load(Ordering::Relaxed),
            )],
        );
    }
    let mask = GX_VAT_DIRTY.swap(0, Ordering::Acquire) & 0xff;
    for vat in 0..8usize {
        if mask & (1 << vat) == 0 {
            continue;
        }
        load_cp_reg(GX_CP_VAT0 + vat as u8, GX_VAT0[vat].load(Ordering::Relaxed));
        load_cp_reg(GX_CP_VAT1 + vat as u8, GX_VAT1[vat].load(Ordering::Relaxed));
        load_cp_reg(GX_CP_VAT2 + vat as u8, GX_VAT2[vat].load(Ordering::Relaxed));
    }
}

fn flush_pending_gx_state() {
    flush_pending_texture_dirty_state();
    flush_pending_gen_mode();
    flush_pending_indirect_texture_mask();
    flush_pending_vertex_state();
    flush_pending_channel_state();
    flush_pending_texgen_state();
}

/// Invalidates the indexed vertex cache.
pub fn inv_vtx_cache() {
    pipe_u8(0x48);
}

/// Clears the current vertex descriptor and defers its CP/XF update until the next draw.
pub fn clear_vtx_desc() {
    GX_VCD_NRMS.store(0, Ordering::Relaxed);
    let clear = GX_VCD_CLEAR.load(Ordering::Relaxed);
    GX_VCD_CLEAR.store((clear & !0x0600) | 0x0200, Ordering::Relaxed);
    GX_VCD_LO.store(0, Ordering::Relaxed);
    GX_VCD_HI.store(0, Ordering::Relaxed);
    GX_VCD_DIRTY.store(true, Ordering::Release);
}

/// Configures an indexed vertex array base and stride.
pub fn set_array(attr: u32, array: *mut c_void, stride: u8) {
    let attr = if attr as u8 == GX_VA_NBT {
        GX_VA_NRM
    } else {
        attr as u8
    };
    if !(GX_VA_POS..=GX_LIGHTARRAY).contains(&attr) {
        return;
    }
    let idx = attr - GX_VA_POS;
    let physical = (array as usize as u32) & 0x3fff_ffff;
    load_cp_reg(0xa0 + idx, physical);
    load_cp_reg(0xb0 + idx, u32::from(stride));
}

pub fn set_vtx_desc(attr: u8, ty: u8) {
    update_vcd_attr(attr, ty);
    GX_VCD_DIRTY.store(true, Ordering::Release);
}

/// # Safety
/// `attr_list` must point to at least `GX_MAX_VTXDESC_LISTSIZE` readable entries or terminate
/// earlier with `GX_VA_NULL`. A null list is rejected as a safety deviation from C UB.
pub unsafe fn set_vtx_descv(attr_list: *const GxVtxDesc) {
    if attr_list.is_null() {
        return;
    }
    for i in 0..GX_MAX_VTXDESC_LISTSIZE {
        let item = unsafe { ptr::read(attr_list.add(i)) };
        if item.attr == GX_VA_NULL {
            break;
        }
        update_vcd_attr(item.attr, item.ty);
    }
    GX_VCD_DIRTY.store(true, Ordering::Release);
}

/// # Safety
/// `attr_list` must point to writable storage for `GX_MAX_VTXDESC_LISTSIZE` entries.
pub unsafe fn get_vtx_descv(attr_list: *mut GxVtxDesc) {
    if attr_list.is_null() {
        return;
    }
    for i in 0..GX_MAX_VTXDESC_LISTSIZE {
        unsafe {
            ptr::write(
                attr_list.add(i),
                GxVtxDesc {
                    attr: GX_VA_NULL,
                    ty: 0,
                },
            )
        };
    }
    let lo = GX_VCD_LO.load(Ordering::Relaxed);
    let hi = GX_VCD_HI.load(Ordering::Relaxed);
    let nrms = GX_VCD_NRMS.load(Ordering::Relaxed);
    let mut out = 0usize;
    let mut push = |attr: u8, ty: u8| {
        if out < GX_MAX_VTXDESC_LISTSIZE {
            unsafe { ptr::write(attr_list.add(out), GxVtxDesc { attr, ty }) };
            out += 1;
        }
    };
    for attr in GX_VA_PTNMTXIDX..=GX_VA_TEX7MTXIDX {
        let shift = u32::from(attr);
        if lo & (1 << shift) != 0 {
            push(attr, ((lo >> shift) & 1) as u8);
        }
    }
    let pos_ty = ((lo >> 9) & 3) as u8;
    if pos_ty != 0 {
        push(GX_VA_POS, pos_ty);
    }
    let nty = ((lo >> 11) & 3) as u8;
    if nty != 0 {
        if nrms == 1 {
            push(GX_VA_NRM, nty);
        } else if nrms == 2 {
            push(GX_VA_NBT, nty);
        }
    }
    for (attr, shift) in [(GX_VA_CLR0, 13), (GX_VA_CLR1, 15)] {
        let ty = ((lo >> shift) & 3) as u8;
        if ty != 0 {
            push(attr, ty);
        }
    }
    for tex in 0..8u8 {
        let ty = ((hi >> (u32::from(tex) * 2)) & 3) as u8;
        if ty != 0 {
            push(GX_VA_TEX0 + tex, ty);
        }
    }
}

pub fn set_vtx_attr_fmt(vtxfmt: u8, vtxattr: u32, comptype: u32, compsize: u32, frac: u32) {
    let fmt = vtxfmt & 7;
    set_vat_attr(fmt, vtxattr, comptype, compsize, frac);
    GX_VAT_DIRTY.fetch_or(1 << fmt, Ordering::AcqRel);
}

/// # Safety
/// `attr_list` must point to at least `GX_MAX_VTXATTRFMT_LISTSIZE` readable entries or terminate
/// earlier with `GX_VA_NULL`. Null is rejected instead of C's null dereference.
pub unsafe fn set_vtx_attr_fmtv(vtxfmt: u8, attr_list: *const GxVtxAttrFmt) {
    if attr_list.is_null() {
        return;
    }
    let fmt = vtxfmt & 7;
    for i in 0..GX_MAX_VTXATTRFMT_LISTSIZE {
        let item = unsafe { ptr::read(attr_list.add(i)) };
        if item.vtxattr as u8 == GX_VA_NULL {
            break;
        }
        set_vat_attr(fmt, item.vtxattr, item.comptype, item.compsize, item.frac);
    }
    GX_VAT_DIRTY.fetch_or(1 << fmt, Ordering::AcqRel);
}

/// Begins recording GX FIFO commands into a caller-provided display-list buffer.
///
/// # Safety
/// The buffer must remain writable for `size` bytes until `end_disp_list`. C requires 32-byte
/// alignment; Rust treats null/misaligned/zero-sized buffers as an overflowed capture rather than
/// programming an invalid FIFO.
pub unsafe fn begin_disp_list(list: *mut c_void, size: u32) {
    flush_pending_gx_state();
    if GX_DL_ACTIVE.swap(true, Ordering::AcqRel) {
        GX_DL_OVERFLOW.store(true, Ordering::Release);
        return;
    }
    if GX_SAVE_DL_CTX.load(Ordering::Acquire) != 0 {
        unsafe { *GX_DL_SAVED_CONTEXT.0.get() = Some(capture_gx_shadow()) };
    } else {
        unsafe { *GX_DL_SAVED_CONTEXT.0.get() = None };
    }
    let addr = list as usize;
    let valid = !list.is_null() && addr & 31 == 0 && size != 0;
    GX_DL_BASE.store(if valid { addr as u32 } else { 0 }, Ordering::Relaxed);
    GX_DL_SIZE.store(if valid { size } else { 0 }, Ordering::Relaxed);
    GX_DL_OFFSET.store(0, Ordering::Relaxed);
    GX_DL_OVERFLOW.store(!valid, Ordering::Release);

    if valid {
        let ready = GX_CPU_FIFO_READY.load(Ordering::Acquire);
        GX_DL_SAVED_CPU_READY.store(ready, Ordering::Relaxed);
        if ready {
            GX_DL_SAVED_CPU_FIFO.store(GX_CPU_FIFO_STATE.load());
        }
        if let Some(dl_fifo) = display_list_fifo_snapshot() {
            GX_CPU_FIFO_STATE.store(dl_fifo);
            GX_CPU_FIFO_READY.store(true, Ordering::Release);
        }
    }
}

fn display_list_padded_size(offset: u32, size: u32) -> Option<u32> {
    let padded = offset.checked_add(31)? & !31;
    (padded <= size).then_some(padded)
}

/// Ends display-list capture and returns the 32-byte-padded byte count, or zero on overflow.
pub fn end_disp_list() -> u32 {
    if !GX_DL_ACTIVE.swap(false, Ordering::AcqRel) {
        return 0;
    }
    let offset = GX_DL_OFFSET.load(Ordering::Relaxed);
    let size = GX_DL_SIZE.load(Ordering::Relaxed);
    let padded = display_list_padded_size(offset, size);
    let overflow = GX_DL_OVERFLOW.load(Ordering::Acquire) || padded.is_none();
    let padded = padded.unwrap_or(0);
    if !overflow && padded > offset {
        let base = GX_DL_BASE.load(Ordering::Relaxed) as *mut u8;
        if !base.is_null() {
            unsafe { ptr::write_bytes(base.add(offset as usize), 0, (padded - offset) as usize) };
        }
    }
    GX_DL_BASE.store(0, Ordering::Relaxed);
    GX_DL_SIZE.store(0, Ordering::Relaxed);
    GX_DL_OFFSET.store(0, Ordering::Relaxed);
    GX_DL_OVERFLOW.store(false, Ordering::Relaxed);

    let cpu_ready = GX_DL_SAVED_CPU_READY.swap(false, Ordering::AcqRel);
    if cpu_ready {
        GX_CPU_FIFO_STATE.store(GX_DL_SAVED_CPU_FIFO.load());
    }
    GX_CPU_FIFO_READY.store(cpu_ready, Ordering::Release);

    let saved = unsafe { core::ptr::replace(GX_DL_SAVED_CONTEXT.0.get(), None) };
    if let Some(saved) = saved {
        restore_gx_shadow(saved);
    }
    if overflow { 0 } else { padded }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
///
/// GX primitives are emitted by their vertex writes; this ABI marker only prevents the
/// compiler from moving later memory operations across the caller's primitive boundary.
pub unsafe fn end() {
    compiler_fence(Ordering::Release);
}

/// Starts a GX primitive by writing its opcode, vertex format, and vertex count to the GP FIFO.
///
/// # Safety
/// GX must be initialized and the caller must provide a valid primitive opcode, vertex format,
/// and vertex count for the active vertex descriptor state.
pub unsafe fn begin(primitive: u8, vtxfmt: u8, nverts: u16) {
    flush_pending_gx_state();
    pipe_u8(primitive | (vtxfmt & 0x07));
    pipe_u16(nverts);
}

fn flush_pending_gen_mode() {
    if GX_GEN_MODE_DIRTY.swap(false, Ordering::Acquire) {
        let word = GX_GEN_MODE.load(Ordering::Relaxed);
        load_bp_reg(0x00, word & 0x00ff_ffff);
        GX_XF_FLUSH_COUNT.store(0, Ordering::Relaxed);
    }
}

fn flush_pending_channel_state() {
    let color_mask = GX_CHAN_COLOR_DIRTY.swap(0, Ordering::Acquire);
    if color_mask & 0x1 != 0 {
        load_xf_regs(0x100a, &[GX_CHAN_AMB_COLOR[0].load(Ordering::Relaxed)]);
    }
    if color_mask & 0x2 != 0 {
        load_xf_regs(0x100b, &[GX_CHAN_AMB_COLOR[1].load(Ordering::Relaxed)]);
    }
    if color_mask & 0x4 != 0 {
        load_xf_regs(0x100c, &[GX_CHAN_MAT_COLOR[0].load(Ordering::Relaxed)]);
    }
    if color_mask & 0x8 != 0 {
        load_xf_regs(0x100d, &[GX_CHAN_MAT_COLOR[1].load(Ordering::Relaxed)]);
    }

    if GX_CHAN_COUNT_DIRTY.swap(false, Ordering::Acquire) {
        load_xf_regs(0x1009, &[(GX_GEN_MODE.load(Ordering::Relaxed) >> 4) & 0x7]);
    }
    let mut control_mask = GX_CHAN_CONTROL_DIRTY.swap(0, Ordering::Acquire) & 0xf;
    let mut index = 0usize;
    while control_mask != 0 {
        if control_mask & 1 != 0 {
            load_xf_regs(
                0x100e + index as u16,
                &[GX_CHAN_CONTROL[index].load(Ordering::Relaxed)],
            );
        }
        control_mask >>= 1;
        index += 1;
    }
}

fn flush_pending_texgen_state() {
    if GX_TEXGEN_COUNT_DIRTY.swap(false, Ordering::Acquire) {
        load_xf_regs(0x103f, &[GX_GEN_MODE.load(Ordering::Relaxed) & 0x0f]);
    }
    let mut mask = GX_TEXGEN_DIRTY_MASK.swap(0, Ordering::Acquire) & 0xff;
    let mut index = 0usize;
    while mask != 0 {
        if mask & 1 != 0 {
            load_xf_regs(
                0x1040 + index as u16,
                &[GX_TEX_COORD_GEN[index].load(Ordering::Relaxed)],
            );
            load_xf_regs(
                0x1050 + index as u16,
                &[GX_TEX_COORD_GEN2[index].load(Ordering::Relaxed)],
            );
        }
        mask >>= 1;
        index += 1;
    }
    if GX_MTX_IDX_DIRTY.swap(false, Ordering::Acquire) {
        let lo = GX_MTX_IDX_LO.load(Ordering::Relaxed);
        let hi = GX_MTX_IDX_HI.load(Ordering::Relaxed);
        unsafe {
            load_cp_reg(0x30, lo);
            load_matrix_index(0x1018, lo);
            load_cp_reg(0x40, hi);
            load_matrix_index(0x1019, hi);
        }
    }
}

fn flush_pending_texture_dirty_state() {
    let pending = GX_TEX_DIRTY.swap(false, Ordering::Acquire);
    let Some([scale_s, scale_t]) = pending_texture_dirty_words(
        pending,
        GX_TEX_DIRTY_S.load(Ordering::Relaxed),
        GX_TEX_DIRTY_T.load(Ordering::Relaxed),
    ) else {
        return;
    };
    load_bp_reg((scale_s >> 24) as u8, scale_s & 0x00ff_ffff);
    load_bp_reg((scale_t >> 24) as u8, scale_t & 0x00ff_ffff);
}

fn pending_texture_dirty_words(dirty: bool, scale_s: u32, scale_t: u32) -> Option<[u32; 2]> {
    dirty.then_some([scale_s, scale_t])
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn position3f32(x: f32, y: f32, z: f32) {
    pipe_f32(x);
    pipe_f32(y);
    pipe_f32(z);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn position3u16(x: u16, y: u16, z: u16) {
    pipe_u16(x);
    pipe_u16(y);
    pipe_u16(z);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn position3s16(x: i16, y: i16, z: i16) {
    unsafe {
        ptr::write_volatile(GP_PIPE_ADDR as *mut i16, x);
        ptr::write_volatile(GP_PIPE_ADDR as *mut i16, y);
        ptr::write_volatile(GP_PIPE_ADDR as *mut i16, z);
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn position3u8(x: u8, y: u8, z: u8) {
    pipe_u8(x);
    pipe_u8(y);
    pipe_u8(z);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn position3s8(x: i8, y: i8, z: i8) {
    unsafe {
        ptr::write_volatile(GP_PIPE_ADDR as *mut i8, x);
        ptr::write_volatile(GP_PIPE_ADDR as *mut i8, y);
        ptr::write_volatile(GP_PIPE_ADDR as *mut i8, z);
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn position2f32(x: f32, y: f32) {
    pipe_f32(x);
    pipe_f32(y);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn position2u16(x: u16, y: u16) {
    pipe_u16(x);
    pipe_u16(y);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn position2s16(x: i16, y: i16) {
    unsafe {
        ptr::write_volatile(GP_PIPE_ADDR as *mut i16, x);
        ptr::write_volatile(GP_PIPE_ADDR as *mut i16, y);
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn position2u8(x: u8, y: u8) {
    pipe_u8(x);
    pipe_u8(y);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn position2s8(x: i8, y: i8) {
    unsafe {
        ptr::write_volatile(GP_PIPE_ADDR as *mut i8, x);
        ptr::write_volatile(GP_PIPE_ADDR as *mut i8, y);
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn position1x8(index: u8) {
    pipe_u8(index);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn position1x16(index: u16) {
    pipe_u16(index);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn normal3f32(nx: f32, ny: f32, nz: f32) {
    pipe_f32(nx);
    pipe_f32(ny);
    pipe_f32(nz);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn normal3s16(nx: i16, ny: i16, nz: i16) {
    unsafe {
        ptr::write_volatile(GP_PIPE_ADDR as *mut i16, nx);
        ptr::write_volatile(GP_PIPE_ADDR as *mut i16, ny);
        ptr::write_volatile(GP_PIPE_ADDR as *mut i16, nz);
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn normal3s8(nx: i8, ny: i8, nz: i8) {
    unsafe {
        ptr::write_volatile(GP_PIPE_ADDR as *mut i8, nx);
        ptr::write_volatile(GP_PIPE_ADDR as *mut i8, ny);
        ptr::write_volatile(GP_PIPE_ADDR as *mut i8, nz);
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn normal1x8(index: u8) {
    pipe_u8(index);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn normal1x16(index: u16) {
    pipe_u16(index);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn color4u8(r: u8, g: u8, b: u8, a: u8) {
    pipe_u8(r);
    pipe_u8(g);
    pipe_u8(b);
    pipe_u8(a);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn color3u8(r: u8, g: u8, b: u8) {
    pipe_u8(r);
    pipe_u8(g);
    pipe_u8(b);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn color3f32(r: f32, g: f32, b: f32) {
    pipe_u8((r * 255.0) as u8);
    pipe_u8((g * 255.0) as u8);
    pipe_u8((b * 255.0) as u8);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn color1u32(clr: u32) {
    pipe_u32(clr);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn color1u16(clr: u16) {
    pipe_u16(clr);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn color1x8(index: u8) {
    pipe_u8(index);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn color1x16(index: u16) {
    pipe_u16(index);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn tex_coord2f32(s: f32, t: f32) {
    pipe_f32(s);
    pipe_f32(t);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn tex_coord2u16(s: u16, t: u16) {
    pipe_u16(s);
    pipe_u16(t);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn tex_coord2s16(s: i16, t: i16) {
    unsafe {
        ptr::write_volatile(GP_PIPE_ADDR as *mut i16, s);
        ptr::write_volatile(GP_PIPE_ADDR as *mut i16, t);
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn tex_coord2u8(s: u8, t: u8) {
    pipe_u8(s);
    pipe_u8(t);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn tex_coord2s8(s: i8, t: i8) {
    unsafe {
        ptr::write_volatile(GP_PIPE_ADDR as *mut i8, s);
        ptr::write_volatile(GP_PIPE_ADDR as *mut i8, t);
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn tex_coord1f32(s: f32) {
    pipe_f32(s);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn tex_coord1u16(s: u16) {
    pipe_u16(s);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn tex_coord1s16(s: i16) {
    unsafe {
        ptr::write_volatile(GP_PIPE_ADDR as *mut i16, s);
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn tex_coord1u8(s: u8) {
    pipe_u8(s);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn tex_coord1s8(s: i8) {
    unsafe {
        ptr::write_volatile(GP_PIPE_ADDR as *mut i8, s);
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn tex_coord1x8(index: u8) {
    pipe_u8(index);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn tex_coord1x16(index: u16) {
    pipe_u16(index);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn matrix_index1x8(index: u8) {
    pipe_u8(index);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn init_tex_obj_from_init(obj: *mut GxTexObj, img_ptr: *mut c_void, init: GxTexObjInit) {
    unsafe {
        if obj.is_null() {
            return;
        }

        let ptr = tex_obj_mut(obj);
        *ptr = GxTexObjRaw {
            tex_filt: 0,
            tex_lod: 0,
            tex_size: 0,
            tex_maddr: 0,
            usr_data: 0,
            tex_fmt: 0,
            tex_tlut: 0,
            tex_tile_cnt: 0,
            tex_tile_type: 0,
            tex_flag: 0,
        };

        ptr.tex_filt = (ptr.tex_filt & !0x03) | ((init.wrap_s & 3) as u32);
        ptr.tex_filt = (ptr.tex_filt & !0x0c) | (((init.wrap_t & 3) as u32) << 2);
        ptr.tex_filt = (ptr.tex_filt & !0x10) | 0x10;

        if init.mipmap != 0 {
            ptr.tex_flag |= 0x01;
            if init.fmt == GX_TF_CI4 || init.fmt == GX_TF_CI8 || init.fmt == GX_TF_CI14 {
                ptr.tex_filt = (ptr.tex_filt & !0xe0) | 0x00a0;
            } else {
                ptr.tex_filt = (ptr.tex_filt & !0xe0) | 0x00c0;
            }
        } else {
            ptr.tex_filt = (ptr.tex_filt & !0xe0) | 0x0080;
        }

        ptr.tex_fmt = init.fmt as u32;
        // BUG FIX: C applies the 10-bit mask after width/height subtraction, including zero.
        ptr.tex_size = (ptr.tex_size & !0x3ff) | ((init.wd.wrapping_sub(1) as u32) & 0x3ff);
        ptr.tex_size =
            (ptr.tex_size & !0xffc00) | (((init.ht.wrapping_sub(1) as u32) & 0x3ff) << 10);
        ptr.tex_size = (ptr.tex_size & !0xf00000) | ((init.fmt as u32) << 20);
        ptr.tex_maddr = (ptr.tex_maddr & !0x00ff_ffff)
            | tex_image_address(ogc_core::mem_virtual_to_physical(img_ptr as usize));

        let (xshift, yshift, tile_type) = match init.fmt {
            GX_TF_I4 | GX_TF_CI4 => (3, 3, 1),
            GX_TF_I8 | GX_TF_IA4 | GX_TF_CI8 => (3, 2, 2),
            GX_TF_IA8 | GX_TF_RGB565 | GX_TF_RGB5A3 | GX_TF_RGBA8 => (2, 2, 2),
            GX_TF_CI14 => (2, 2, 3),
            GX_TF_CMPR => (3, 3, 0),
            _ => (2, 2, 2),
        };

        let nwd = (((init.wd as u32) + (1 << xshift)) - 1) >> xshift;
        let nht = (((init.ht as u32) + (1 << yshift)) - 1) >> yshift;
        ptr.tex_tile_cnt = ((nwd * nht) & 0x7fff) as u16;
        ptr.tex_tile_type = tile_type;
        ptr.tex_flag |= 0x02;
    }
}

/// Initializes a texture object from grouped texture settings.
///
/// # Safety
/// `obj` must be null or point to a writable GX texture object. `img_ptr` is
/// interpreted as the texture image base exactly as libogc does.
pub unsafe fn init_tex_obj(obj: *mut GxTexObj, img_ptr: *mut c_void, init: GxTexObjInit) {
    unsafe { init_tex_obj_from_init(obj, img_ptr, init) };
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn init_tex_obj_ci_from_init(
    obj: *mut GxTexObj,
    img_ptr: *mut c_void,
    init: GxTexObjInit,
    tlut_name: u32,
) {
    unsafe {
        if obj.is_null() {
            return;
        }
        init_tex_obj_from_init(obj, img_ptr, init);
        let ptr = { tex_obj_mut(obj) };
        ptr.tex_flag &= !0x02;
        ptr.tex_tlut = tlut_name;
    }
}

/// Initializes a color-indexed texture object from grouped texture settings.
///
/// # Safety
/// `obj` must be null or point to a writable GX texture object.
pub unsafe fn init_tex_obj_ci(
    obj: *mut GxTexObj,
    img_ptr: *mut c_void,
    init: GxTexObjInit,
    tlut_name: u32,
) {
    unsafe { init_tex_obj_ci_from_init(obj, img_ptr, init, tlut_name) };
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn init_tex_obj_lod_from_params(obj: *mut GxTexObj, params: GxTexObjLod) {
    unsafe {
        let ptr = tex_obj_mut(obj);
        ptr.tex_filt = (ptr.tex_filt & !0x1fe00) | texture_lod_bias_field(params.lodbias);
        ptr.tex_filt = (ptr.tex_filt & !0x10) | ((params.magfilt == GX_LINEAR) as u32) << 4;
        ptr.tex_filt = (ptr.tex_filt & !0xe0) | ((tex_filter_hw(params.minfilt) as u32) << 5);
        ptr.tex_filt = (ptr.tex_filt & !0x100) | (((params.edgelod == 0) as u32) << 8);
        ptr.tex_filt = (ptr.tex_filt & !0x180000) | (((params.maxaniso & 3) as u32) << 19);
        ptr.tex_filt = (ptr.tex_filt & !0x200000) | (((params.biasclamp & 1) as u32) << 21);

        let minlod = params.minlod.clamp(0.0, 10.0);
        let maxlod = params.maxlod.clamp(0.0, 10.0);

        ptr.tex_lod = (ptr.tex_lod & !0xff) | (((16.0 * minlod) as u32) & 0xff);
        ptr.tex_lod = (ptr.tex_lod & !0xff00) | (((16.0 * maxlod) as u32) << 8);
    }
}

/// Sets the texture level-of-detail settings.
///
/// # Safety
/// `obj` must point to a valid writable GX texture object.
pub unsafe fn init_tex_obj_lod(obj: *mut GxTexObj, params: GxTexObjLod) {
    unsafe { init_tex_obj_lod_from_params(obj, params) };
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn init_tex_obj_data(obj: *mut GxTexObj, img_ptr: *mut c_void) {
    let ptr = unsafe { tex_obj_mut(obj) };
    ptr.tex_maddr = (ptr.tex_maddr & !0x00ff_ffff)
        | tex_image_address(ogc_core::mem_virtual_to_physical(img_ptr as usize));
}

#[inline]
fn tex_image_address(physical_addr: u32) -> u32 {
    // BUG FIX: GX stores only the low 24 bits of the 5-bit shifted address.
    (physical_addr >> 5) & 0x00ff_ffff
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn init_tex_obj_tlut(obj: *mut GxTexObj, tlut_name: u32) {
    let ptr = unsafe { tex_obj_mut(obj) };
    ptr.tex_tlut = tlut_name;
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn init_tex_obj_wrap_mode(obj: *mut GxTexObj, wrap_s: u8, wrap_t: u8) {
    let ptr = unsafe { tex_obj_mut(obj) };
    ptr.tex_filt = (ptr.tex_filt & !0x03) | ((wrap_s & 3) as u32);
    ptr.tex_filt = (ptr.tex_filt & !0x0c) | (((wrap_t & 3) as u32) << 2);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn init_tex_obj_filter_mode(obj: *mut GxTexObj, minfilt: u8, magfilt: u8) {
    let ptr = unsafe { tex_obj_mut(obj) };
    ptr.tex_filt = (ptr.tex_filt & !0x10) | ((magfilt == GX_LINEAR) as u32) << 4;
    ptr.tex_filt = (ptr.tex_filt & !0xe0) | ((tex_filter_hw(minfilt) as u32) << 5);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn init_tex_obj_min_lod(obj: *mut GxTexObj, minlod: f32) {
    unsafe {
        let ptr = tex_obj_mut(obj);
        let minlod = minlod.clamp(0.0, 10.0);
        ptr.tex_lod = (ptr.tex_lod & !0xff) | (((16.0 * minlod) as u32) & 0xff);
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn init_tex_obj_max_lod(obj: *mut GxTexObj, maxlod: f32) {
    unsafe {
        let ptr = tex_obj_mut(obj);
        let maxlod = maxlod.clamp(0.0, 10.0);
        ptr.tex_lod = (ptr.tex_lod & !0xff00) | (((16.0 * maxlod) as u32) << 8);
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn init_tex_obj_lod_bias(obj: *mut GxTexObj, lodbias: f32) {
    unsafe {
        let ptr = tex_obj_mut(obj);
        ptr.tex_filt = (ptr.tex_filt & !0x1fe00) | texture_lod_bias_field(lodbias);
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn init_tex_obj_bias_clamp(obj: *mut GxTexObj, biasclamp: u8) {
    let ptr = unsafe { tex_obj_mut(obj) };
    ptr.tex_filt = (ptr.tex_filt & !0x200000) | (((biasclamp & 1) as u32) << 21);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn init_tex_obj_edge_lod(obj: *mut GxTexObj, edgelod: u8) {
    let ptr = unsafe { tex_obj_mut(obj) };
    ptr.tex_filt = (ptr.tex_filt & !0x100) | (((edgelod == 0) as u32) << 8);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn init_tex_obj_max_aniso(obj: *mut GxTexObj, maxaniso: u8) {
    let ptr = unsafe { tex_obj_mut(obj) };
    ptr.tex_filt = (ptr.tex_filt & !0x180000) | (((maxaniso & 3) as u32) << 19);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn init_tex_obj_user_data(obj: *mut GxTexObj, userdata: *mut c_void) {
    let ptr = unsafe { tex_obj_mut(obj) };
    ptr.usr_data = userdata as u32;
}

/// # Safety
/// The caller must ensure `region` points to a valid `GxTexRegion`.
pub unsafe fn init_tex_cache_region(
    region: *mut GxTexRegion,
    is32bmipmap: u8,
    tmem_even: u32,
    size_even: u8,
    tmem_odd: u32,
    size_odd: u8,
) {
    unsafe {
        if region.is_null() {
            return;
        }
        let val = &mut (*region).val;
        val[0] = ((tmem_even >> 5) & 0x7fff) | pack_tex_cache_size(size_even);
        val[1] = ((tmem_odd >> 5) & 0x7fff) | pack_tex_cache_size(size_odd);
        val[2] = is32bmipmap as u32;
        val[3] = 1;
    }
}

/// # Safety
/// The caller must ensure `region` points to a valid `GxTexRegion`.
pub unsafe fn init_tex_preload_region(
    region: *mut GxTexRegion,
    tmem_even: u32,
    size_even: u32,
    tmem_odd: u32,
    size_odd: u32,
) {
    unsafe {
        if region.is_null() {
            return;
        }
        let val = &mut (*region).val;
        val[0] = ((tmem_even >> 5) & 0x7fff) | 0x200000;
        val[1] = (tmem_odd >> 5) & 0x7fff;
        // BUG FIX: GXTexRegion stores these adjacent u16 values in one word.
        val[2] = (((size_even >> 5) & 0xffff) << 16) | ((size_odd >> 5) & 0xffff);
        val[3] = 0;
    }
}

/// # Safety
/// The caller must ensure `region` points to a valid `GxTlutRegion`.
pub unsafe fn init_tlut_region(region: *mut GxTlutRegion, tmem_addr: u32, tlut_sz: u8) {
    unsafe {
        if region.is_null() {
            return;
        }
        let val = &mut (*region).val;
        let tmem_addr = tmem_addr.wrapping_sub(0x80000);
        val[0] = ((tmem_addr >> 9) & 0x3ff) | (((tlut_sz as u32) & 0x3ff) << 10) | (0x65 << 24);
        // BUG FIX: C initializes only tmem_addr_conf and preserves the cached TLUT fields.
    }
}

/// Overrides automatic scaling for a texture coordinate, matching
/// `GX_SetTexCoordScaleManually`.
///
/// # Safety
/// GX must be initialized before enabling manual scaling.
pub unsafe fn set_tex_coord_scale_manually(texcoord: u8, enable: u8, ss: u16, ts: u16) {
    let coord = usize::from(texcoord & 7);
    let mask = 1u32 << coord;
    if enable == 0 {
        GX_TEX_COORD_MANUAL.fetch_and(!mask, Ordering::AcqRel);
        return;
    }
    GX_TEX_COORD_MANUAL.fetch_or(mask, Ordering::AcqRel);

    let s_word = update_su_size_word(GX_SU_SSIZE[coord].load(Ordering::Relaxed), ss);
    let t_word = update_su_size_word(GX_SU_TSIZE[coord].load(Ordering::Relaxed), ts);
    GX_SU_SSIZE[coord].store(s_word, Ordering::Relaxed);
    GX_SU_TSIZE[coord].store(t_word, Ordering::Relaxed);
    write_su_word(s_word);
    write_su_word(t_word);
}

/// Updates cylindrical-wrap state for a texture coordinate.
///
/// # Safety
/// GX must be initialized before this function writes BP state.
pub unsafe fn set_tex_coord_cyl_wrap(texcoord: u8, s_enable: u8, t_enable: u8) {
    let coord = usize::from(texcoord & 7);
    let s_word = update_su_flag_word(GX_SU_SSIZE[coord].load(Ordering::Relaxed), 17, s_enable);
    let t_word = update_su_flag_word(GX_SU_TSIZE[coord].load(Ordering::Relaxed), 17, t_enable);
    GX_SU_SSIZE[coord].store(s_word, Ordering::Relaxed);
    GX_SU_TSIZE[coord].store(t_word, Ordering::Relaxed);
    if GX_TEX_COORD_MANUAL.load(Ordering::Acquire) & (1u32 << coord) != 0 {
        write_su_word(s_word);
        write_su_word(t_word);
    }
}

/// Updates range-bias state for a texture coordinate.
///
/// # Safety
/// GX must be initialized before this function writes BP state.
pub unsafe fn set_tex_coord_bias(texcoord: u8, s_enable: u8, t_enable: u8) {
    let coord = usize::from(texcoord & 7);
    let s_word = update_su_flag_word(GX_SU_SSIZE[coord].load(Ordering::Relaxed), 16, s_enable);
    let t_word = update_su_flag_word(GX_SU_TSIZE[coord].load(Ordering::Relaxed), 16, t_enable);
    GX_SU_SSIZE[coord].store(s_word, Ordering::Relaxed);
    GX_SU_TSIZE[coord].store(t_word, Ordering::Relaxed);
    if GX_TEX_COORD_MANUAL.load(Ordering::Acquire) & (1u32 << coord) != 0 {
        write_su_word(s_word);
        write_su_word(t_word);
    }
}

/// # Safety
/// The caller must ensure callback installation is synchronized with any
/// concurrent GX use.
pub unsafe fn set_tex_region_callback(cb: GxTexRegionCallback) -> GxTexRegionCallback {
    unsafe {
        let prev = TEX_REGION_CB;
        TEX_REGION_CB = cb;
        prev
    }
}

/// # Safety
/// The caller must ensure callback installation is synchronized with any
/// concurrent GX use.
pub unsafe fn set_tlut_region_callback(cb: GxTlutRegionCallback) -> GxTlutRegionCallback {
    unsafe {
        let prev = TLUT_REGION_CB;
        TLUT_REGION_CB = cb;
        prev
    }
}

pub fn get_tex_buffer_size(wd: u16, ht: u16, fmt: u32, mipmap: u8, maxlod: u8) -> u32 {
    let (xshift, yshift) = tex_size_shifts(fmt);
    let bitsize = if fmt == GX_TF_RGBA8 as u32 || fmt == GX_TF_Z24X8 as u32 {
        64
    } else {
        32
    };

    if mipmap != 0 {
        let mut size = 0;
        let mut cnt = maxlod as u32;
        let mut cur_w = wd as u32;
        let mut cur_h = ht as u32;
        while cnt != 0 {
            let xtiles = (cur_w + (1 << xshift) - 1) >> xshift;
            let ytiles = (cur_h + (1 << yshift) - 1) >> yshift;
            size += xtiles * ytiles * bitsize;
            if cur_w == 1 && cur_h == 1 {
                return size;
            }
            cur_w = if cur_w > 1 { cur_w >> 1 } else { 1 };
            cur_h = if cur_h > 1 { cur_h >> 1 } else { 1 };
            cnt -= 1;
        }
        return size;
    }

    let xtiles = (wd as u32 + ((1 << xshift) - 1)) >> xshift;
    let ytiles = (ht as u32 + ((1 << yshift) - 1)) >> yshift;
    xtiles * ytiles * bitsize
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn get_tex_obj_fmt(obj: *const GxTexObj) -> u32 {
    unsafe { tex_obj_snapshot(obj).map_or(0, |snap| snap.format as u32) }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn get_tex_obj_mip_map(obj: *const GxTexObj) -> u32 {
    unsafe { tex_obj_snapshot(obj).map_or(0, |snap| snap.mipmap as u32) }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn get_tex_obj_data(obj: *const GxTexObj) -> *mut c_void {
    unsafe { tex_obj_snapshot(obj).map_or(core::ptr::null_mut(), |snap| snap.image_ptr) }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn get_tex_obj_wrap_s(obj: *const GxTexObj) -> u8 {
    unsafe { tex_obj_snapshot(obj).map_or(0, |snap| snap.wrap_s) }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn get_tex_obj_wrap_t(obj: *const GxTexObj) -> u8 {
    unsafe { tex_obj_snapshot(obj).map_or(0, |snap| snap.wrap_t) }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn get_tex_obj_height(obj: *const GxTexObj) -> u16 {
    unsafe { tex_obj_snapshot(obj).map_or(0, |snap| snap.height) }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn get_tex_obj_width(obj: *const GxTexObj) -> u16 {
    unsafe { tex_obj_snapshot(obj).map_or(0, |snap| snap.width) }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn get_tex_obj_lod(obj: *const GxTexObj, minlod: *mut f32, maxlod: *mut f32) {
    unsafe {
        if let Some(snap) = tex_obj_snapshot(obj) {
            *minlod = (snap.tex_lod & 0xff) as f32 / 16.0;
            *maxlod = ((snap.tex_lod >> 8) & 0xff) as f32 / 16.0;
        } else {
            *minlod = 0.0;
            *maxlod = 0.0;
        }
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn get_tex_obj_filter_mode(obj: *const GxTexObj, minfilt: *mut u8, magfilt: *mut u8) {
    unsafe {
        if let Some(snap) = tex_obj_snapshot(obj) {
            *magfilt = if snap.tex_filt & 0x10 != 0 {
                GX_LINEAR
            } else {
                GX_NEAR
            };
            let hw_filt = ((snap.tex_filt >> 5) & 0x07) as u8;
            let filters = [0x00, 0x04, 0x01, 0x05, 0x02, 0x06, 0x00, 0x00];
            *minfilt = filters
                .iter()
                .position(|&value| value == hw_filt)
                .map(|idx| idx as u8)
                .unwrap_or(GX_NEAR);
        } else {
            *minfilt = GX_NEAR;
            *magfilt = GX_NEAR;
        }
    }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn get_tex_obj_info(obj: *const GxTexObj, out: *mut GxTexObjInfo) {
    unsafe {
        let out = &mut *out;
        if let Some(snap) = tex_obj_snapshot(obj) {
            out.image_ptr = snap.image_ptr;
            out.width = snap.width;
            out.height = snap.height;
            out.format = snap.format;
            out.wrap_s = snap.wrap_s;
            out.wrap_t = snap.wrap_t;
            out.mipmap = snap.mipmap;
        } else {
            *out = GxTexObjInfo::default();
        }
    }
}

/// Copies texture settings into the selected output locations.
///
/// # Safety
/// `obj` must point to a valid GX texture object. Non-null output pointers must
/// be writable. Unlike C, null output pointers are skipped rather than
/// dereferenced.
pub unsafe fn get_tex_obj_all(obj: *const GxTexObj, params: GxTexObjOutputs) {
    let GxTexObjOutputs {
        image_ptr,
        width,
        height,
        format,
        wrap_s,
        wrap_t,
        mipmap,
    } = params;
    let Some(snapshot) = (unsafe { tex_obj_snapshot(obj) }) else {
        return;
    };
    unsafe {
        if !image_ptr.is_null() {
            *image_ptr = snapshot.image_ptr;
        }
        if !width.is_null() {
            *width = snapshot.width;
        }
        if !height.is_null() {
            *height = snapshot.height;
        }
        if !format.is_null() {
            *format = snapshot.format;
        }
        if !wrap_s.is_null() {
            *wrap_s = snapshot.wrap_s;
        }
        if !wrap_t.is_null() {
            *wrap_t = snapshot.wrap_t;
        }
        if !mipmap.is_null() {
            *mipmap = snapshot.mipmap;
        }
    }
}

/// # Safety
/// The caller must ensure `obj` points to a valid GX texture object for the
/// duration of the call.
pub unsafe fn tex_obj_snapshot(obj: *const GxTexObj) -> Option<GxTexObjSnapshot> {
    unsafe {
        if obj.is_null() {
            return None;
        }
        let ptr = tex_obj_ref(obj);
        Some(GxTexObjSnapshot {
            image_ptr: ((ptr.tex_maddr & 0x00ff_ffff) << 5) as usize as *mut c_void,
            width: ((ptr.tex_size & 0x3ff) + 1) as u16,
            height: (((ptr.tex_size & 0xffc00) >> 10) + 1) as u16,
            format: ptr.tex_fmt as u8,
            wrap_s: (ptr.tex_filt & 0x03) as u8,
            wrap_t: ((ptr.tex_filt & 0x0c) >> 2) as u8,
            mipmap: ptr.tex_flag & 0x01,
            tex_filt: ptr.tex_filt,
            tex_lod: ptr.tex_lod,
            tex_tlut: ptr.tex_tlut,
            tex_tile_cnt: ptr.tex_tile_cnt,
            tex_tile_type: ptr.tex_tile_type,
            tex_flag: ptr.tex_flag,
            tex_size: ptr.tex_size,
            tex_maddr: ptr.tex_maddr,
            user_data: ptr.usr_data as usize as *mut c_void,
        })
    }
}

/// # Safety
/// The caller must ensure `obj` points to a valid GX light object for the
/// duration of the call.
pub unsafe fn light_obj_snapshot(obj: *const GxLightObj) -> Option<GxLightObjSnapshot> {
    if obj.is_null() {
        return None;
    }
    let lit = unsafe { light_obj_ref(obj) };
    Some(GxLightObjSnapshot {
        color: GxColor {
            r: (lit.col >> 24) as u8,
            g: (lit.col >> 16) as u8,
            b: (lit.col >> 8) as u8,
            a: lit.col as u8,
        },
        position: [lit.px, lit.py, lit.pz],
        direction: [lit.nx, lit.ny, lit.nz],
        attenuation: [lit.a0, lit.a1, lit.a2],
        attenuation_coeff: [lit.k0, lit.k1, lit.k2],
    })
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn get_tex_obj_user_data(obj: *const GxTexObj) -> *mut c_void {
    unsafe { tex_obj_snapshot(obj).map_or(core::ptr::null_mut(), |snap| snap.user_data) }
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn init_tlut_obj(obj: *mut GxTlutObj, lut: *mut c_void, fmt: u8, entries: u16) {
    unsafe {
        let ptr = tlut_obj_mut(obj);
        *ptr = GxTlutObjRaw {
            tlut_fmt: 0,
            tlut_maddr: 0,
            tlut_nentries: 0,
            _pad: [0; 2],
        };
        ptr.tlut_fmt = (fmt as u32) << 10;
        ptr.tlut_maddr = (ogc_core::mem_virtual_to_physical(lut as usize) >> 5) | (0x64 << 24);
        ptr.tlut_nentries = entries;
    }
}

/// # Safety
/// The caller must ensure `obj` points to a valid GX TLUT object for the
/// duration of the call.
pub unsafe fn tlut_obj_snapshot(obj: *const GxTlutObj) -> Option<GxTlutObjSnapshot> {
    unsafe {
        if obj.is_null() {
            return None;
        }
        let ptr = &*(obj as *const GxTlutObjRaw);
        Some(GxTlutObjSnapshot {
            format: ptr.tlut_fmt >> 10,
            image_ptr: ((ptr.tlut_maddr & 0x00ff_ffff) << 5) as usize as *mut c_void,
            nentries: ptr.tlut_nentries,
            tlut_fmt: ptr.tlut_fmt,
            tlut_maddr: ptr.tlut_maddr,
        })
    }
}

/// Sets the number of generated texture coordinates.
///
/// # Safety
/// GX must be initialized before this function changes generation state.
pub unsafe fn set_num_tex_gens(nr: u32) {
    let current = GX_GEN_MODE.load(Ordering::Relaxed);
    GX_GEN_MODE.store(update_bits(current, 0x0f, nr & 0x0f), Ordering::Relaxed);
    GX_GEN_MODE_DIRTY.store(true, Ordering::Release);
    GX_TEXGEN_COUNT_DIRTY.store(true, Ordering::Release);
}

fn texgen_words(tgen_typ: u32, mut tgen_src: u32, normalize: u32, postmtx: u32) -> (u32, u32) {
    let (vtxrow, stq) = match tgen_src {
        GX_TG_POS => (0, 1),
        GX_TG_NRM => (1, 1),
        GX_TG_BINRM => (3, 1),
        GX_TG_TANGENT => (4, 1),
        GX_TG_COLOR0 | GX_TG_COLOR1 => (2, 0),
        GX_TG_TEX0 => (5, 0),
        GX_TG_TEX1 => (6, 0),
        GX_TG_TEX2 => (7, 0),
        GX_TG_TEX3 => (8, 0),
        GX_TG_TEX4 => (9, 0),
        GX_TG_TEX5 => (10, 0),
        GX_TG_TEX6 => (11, 0),
        GX_TG_TEX7 => (12, 0),
        _ => (5, 0),
    };
    let mut texcoords = 0u32;
    if tgen_typ == GX_TG_MTX3X4 || tgen_typ == GX_TG_MTX2X4 {
        if tgen_typ == GX_TG_MTX3X4 {
            texcoords = 0x02;
        }
        texcoords |= stq << 2;
        texcoords |= vtxrow << 7;
    } else if (GX_TG_BUMP0..=GX_TG_BUMP7).contains(&tgen_typ) {
        tgen_src = tgen_src.wrapping_sub(GX_TG_TEXCOORD0);
        let bump = tgen_typ - GX_TG_BUMP0;
        texcoords = 0x10 | (stq << 2) | (vtxrow << 7) | ((tgen_src & 7) << 12) | ((bump & 7) << 15);
    } else if tgen_typ == GX_TG_SRTG {
        texcoords = if tgen_src == GX_TG_COLOR0 {
            0x20
        } else if tgen_src == GX_TG_COLOR1 {
            0x30
        } else {
            0
        };
        texcoords |= stq << 2;
        texcoords |= 2 << 7;
    }
    let post = postmtx.wrapping_sub(GX_DTTMTX0);
    (texcoords, ((normalize & 1) << 8) | (post & 0x3f))
}

/// Matches `GX_SetTexCoordGen`.
///
/// # Safety
/// GX must be initialized before this function changes XF/CP state.
pub unsafe fn set_tex_coord_gen(texcoord: u16, tgen_typ: u32, tgen_src: u32, mtxsrc: u32) {
    unsafe {
        set_tex_coord_gen2(
            texcoord,
            tgen_typ,
            tgen_src,
            mtxsrc,
            u32::from(GX_FALSE),
            GX_DTTIDENTITY,
        )
    };
}

/// # Safety
/// GX must be initialized and `matrix` must reference a valid 3x4 matrix.
/// Adjusts a render-mode object for horizontal/vertical overscan.
///
/// # Safety
/// `rmin` must be readable and `rmout` writable for a `GxrModeObj`. Null pointers and a zero
/// source XFB height are rejected instead of triggering C undefined behavior.
pub unsafe fn adjust_for_overscan(
    rmin: *const GxrModeObj,
    rmout: *mut GxrModeObj,
    hor: u16,
    ver: u16,
) {
    if rmin.is_null() || rmout.is_null() {
        return;
    }
    let input = unsafe { ptr::read(rmin) };
    if input.xfb_height == 0 {
        return;
    }
    let mut output = input;
    let hor2 = hor.wrapping_mul(2);
    let ver2 = ver.wrapping_mul(2);
    output.fb_width = input.fb_width.wrapping_sub(hor2);
    let efb_trim = (u32::from(input.efb_height) * u32::from(ver2)) / u32::from(input.xfb_height);
    output.efb_height = input.efb_height.wrapping_sub(efb_trim as u16);
    const VI_XFBMODE_SF: u32 = 0;
    const VI_PROGRESSIVE: u32 = 2;
    output.xfb_height = if input.xfb_mode == VI_XFBMODE_SF && input.vi_tv_mode & VI_PROGRESSIVE == 0
    {
        input.xfb_height.wrapping_sub(ver)
    } else {
        input.xfb_height.wrapping_sub(ver2)
    };
    output.vi_width = input.vi_width.wrapping_sub(hor2);
    output.vi_height = if input.vi_tv_mode & VI_PROGRESSIVE != 0 {
        input.vi_height.wrapping_sub(ver.wrapping_mul(4))
    } else {
        input.vi_height.wrapping_sub(ver2)
    };
    output.vi_x_origin = input.vi_x_origin.wrapping_add(hor);
    output.vi_y_origin = input.vi_y_origin.wrapping_add(ver);
    unsafe { ptr::write(rmout, output) };
}

/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn load_pos_mtx_imm(matrix: &Mtx, pnidx: u32) {
    load_xf_regs(((pnidx & 0xff) << 2) as u16, &matrix_4x3_words(matrix));
}

/// # Safety
/// GX must be initialized and `mtxidx` must reference valid indexed matrix data.
pub unsafe fn load_pos_mtx_idx(mtxidx: u16, pnidx: u32) {
    pipe_u8(0x20);
    pipe_u32((u32::from(mtxidx) << 16) | 0xb000 | ((pnidx & 0xff) << 2));
}

/// # Safety
/// GX must be initialized and `matrix` must reference a valid 3x4 matrix.
pub unsafe fn load_nrm_mtx_imm(matrix: &Mtx, pnidx: u32) {
    load_xf_regs(
        0x0400 | ((pnidx as u16).wrapping_mul(3)),
        &normal_from_4x3_words(matrix),
    );
}

/// # Safety
/// GX must be initialized and `matrix` must reference a valid 3x3 matrix.
pub unsafe fn load_nrm_mtx_imm3x3(matrix: &Mtx33, pnidx: u32) {
    load_xf_regs(
        0x0400 | ((pnidx as u16).wrapping_mul(3)),
        &matrix_3x3_words(matrix),
    );
}

/// # Safety
/// GX must be initialized and `mtxidx` must reference valid indexed matrix data.
pub unsafe fn load_nrm_mtx_idx3x3(mtxidx: u16, pnidx: u32) {
    pipe_u8(0x28);
    pipe_u32((u32::from(mtxidx) << 16) | 0x8000 | 0x0400 | ((pnidx & 0xff) * 3));
}

/// # Safety
/// GX must be initialized and `matrix` must reference a valid 3x4 matrix.
pub unsafe fn load_tex_mtx_imm(matrix: &Mtx, mut texidx: u32, mtx_type: u8) {
    let address = if texidx < GX_DTTMTX0 {
        (texidx & 0xff) << 2
    } else {
        texidx -= GX_DTTMTX0;
        0x0500 | ((texidx & 0xff) << 2)
    };
    if mtx_type == GX_MTX2X4 {
        load_xf_regs(address as u16, &matrix_2x4_words(matrix));
    } else {
        load_xf_regs(address as u16, &matrix_4x3_words(matrix));
    }
}

/// # Safety
/// GX must be initialized and `mtxidx` must reference valid indexed matrix data.
pub unsafe fn load_tex_mtx_idx(mtxidx: u16, texidx: u32, mtx_type: u8) {
    let size = if mtx_type == GX_MTX2X4 { 7 } else { 11 };
    let address = if texidx < GX_DTTMTX0 {
        (texidx & 0xff) << 2
    } else {
        0x0500 | (((texidx - GX_DTTMTX0) & 0xff) << 2)
    };
    pipe_u8(0x30);
    pipe_u32((u32::from(mtxidx) << 16) | ((size as u32) << 12) | address);
}

/// Matches `GX_SetCurrentMtx` through the existing deferred matrix-index path.
pub fn set_current_mtx(mtx: u32) {
    let current = GX_MTX_IDX_LO.load(Ordering::Relaxed);
    GX_MTX_IDX_LO.store(update_bits(current, 0x3f, mtx & 0x3f), Ordering::Relaxed);
    GX_MTX_IDX_DIRTY.store(true, Ordering::Release);
}

/// Matches `GX_SetTexCoordGen2`.
///
/// # Safety
/// GX must be initialized before this function changes XF/CP state.
pub unsafe fn set_tex_coord_gen2(
    texcoord: u16,
    tgen_typ: u32,
    tgen_src: u32,
    mtxsrc: u32,
    normalize: u32,
    postmtx: u32,
) {
    if texcoord >= GX_MAXCOORD {
        return;
    }
    let coord = usize::from(texcoord & 7);
    let (word, word2) = texgen_words(tgen_typ, tgen_src, normalize, postmtx);
    GX_TEX_COORD_GEN[coord].store(word, Ordering::Relaxed);
    GX_TEX_COORD_GEN2[coord].store(word2, Ordering::Relaxed);
    let mtx = mtxsrc & 0x3f;
    if coord < 4 {
        let shift = 6 + coord as u32 * 6;
        let current = GX_MTX_IDX_LO.load(Ordering::Relaxed);
        GX_MTX_IDX_LO.store(
            update_bits(current, 0x3f << shift, mtx << shift),
            Ordering::Relaxed,
        );
    } else {
        let shift = (coord as u32 - 4) * 6;
        let current = GX_MTX_IDX_HI.load(Ordering::Relaxed);
        GX_MTX_IDX_HI.store(
            update_bits(current, 0x3f << shift, mtx << shift),
            Ordering::Relaxed,
        );
    }
    GX_TEXGEN_DIRTY_MASK.fetch_or(1u32 << coord, Ordering::Release);
    GX_MTX_IDX_DIRTY.store(true, Ordering::Release);
}

/// Configures Z-texture replacement/addition.
///
/// # Safety
/// GX must be initialized before this function writes BP state.
pub unsafe fn set_z_texture(op: u8, fmt: u8, bias: u32) {
    let format = if fmt == GX_TF_Z8 {
        0
    } else if fmt == GX_TF_Z16 {
        1
    } else {
        2
    };
    write_bp_word(0xf400_0000 | (bias & 0x00ff_ffff));
    write_bp_word(0xf500_0000 | (u32::from(op & 3) << 2) | format);
}

/// Sets a TEV color register, matching `GX_SetTevColor`.
///
/// # Safety
/// GX must be initialized before this function writes BP state.
pub unsafe fn set_tev_color(tev_regid: u8, color: GxColor) {
    let id = tev_regid & 3;
    let ra = (0xe0u32 + u32::from(id) * 2) << 24 | (u32::from(color.a) << 12) | u32::from(color.r);
    let bg = (0xe1u32 + u32::from(id) * 2) << 24 | (u32::from(color.g) << 12) | u32::from(color.b);
    write_bp_word(ra);
    write_bp_word(bg);
    write_bp_word(bg);
    write_bp_word(bg);
}

/// Sets a signed-10 TEV color register, matching `GX_SetTevColorS10`.
///
/// # Safety
/// GX must be initialized before this function writes BP state.
pub unsafe fn set_tev_color_s10(tev_regid: u8, color: GxColorS10) {
    let id = tev_regid & 3;
    let ra = (0xe0u32 + u32::from(id) * 2) << 24
        | (((color.a as u32) & 0x7ff) << 12)
        | ((color.r as u32) & 0x7ff);
    let bg = (0xe1u32 + u32::from(id) * 2) << 24
        | (((color.g as u32) & 0x7ff) << 12)
        | ((color.b as u32) & 0x7ff);
    write_bp_word(ra);
    write_bp_word(bg);
    write_bp_word(bg);
    write_bp_word(bg);
}

/// # Safety
/// GX must be initialized before this function writes BP state.
pub unsafe fn set_tev_k_color(tev_kregid: u8, color: GxColor) {
    let id = tev_kregid & 3;
    let ra = (0xe0u32 + u32::from(id) * 2) << 24
        | (1 << 23)
        | (u32::from(color.a) << 12)
        | u32::from(color.r);
    let bg = (0xe1u32 + u32::from(id) * 2) << 24
        | (1 << 23)
        | (u32::from(color.g) << 12)
        | u32::from(color.b);
    write_bp_word(ra);
    write_bp_word(bg);
    write_bp_word(bg);
    write_bp_word(bg);
}

/// # Safety
/// GX must be initialized before this function writes BP state.
pub unsafe fn set_tev_k_color_s10(tev_kregid: u8, color: GxColorS10) {
    let id = tev_kregid & 3;
    let ra = (0xe0u32 + u32::from(id) * 2) << 24
        | (1 << 23)
        | (((color.a as u32) & 0x7ff) << 12)
        | ((color.r as u32) & 0x7ff);
    let bg = (0xe1u32 + u32::from(id) * 2) << 24
        | (1 << 23)
        | (((color.g as u32) & 0x7ff) << 12)
        | ((color.b as u32) & 0x7ff);
    write_bp_word(ra);
    write_bp_word(bg);
    write_bp_word(bg);
    write_bp_word(bg);
}

/// # Safety
/// GX must be initialized before this function writes BP state.
pub unsafe fn set_tev_color_in(tevstage: u8, a: u8, b: u8, c: u8, d: u8) {
    let stage = usize::from(tevstage & 0x0f);
    let mut word = GX_TEV_COLOR_ENV[stage].load(Ordering::Relaxed);
    word = update_bits(word, 0x0000_f000, u32::from(a & 0x0f) << 12);
    word = update_bits(word, 0x0000_0f00, u32::from(b & 0x0f) << 8);
    word = update_bits(word, 0x0000_00f0, u32::from(c & 0x0f) << 4);
    word = update_bits(word, 0x0000_000f, u32::from(d & 0x0f));
    GX_TEV_COLOR_ENV[stage].store(word, Ordering::Relaxed);
    write_bp_word(word);
}

/// # Safety
/// GX must be initialized before this function writes BP state.
pub unsafe fn set_tev_alpha_in(tevstage: u8, a: u8, b: u8, c: u8, d: u8) {
    let stage = usize::from(tevstage & 0x0f);
    let mut word = GX_TEV_ALPHA_ENV[stage].load(Ordering::Relaxed);
    word = update_bits(word, 0x0000_e000, u32::from(a & 7) << 13);
    word = update_bits(word, 0x0000_1c00, u32::from(b & 7) << 10);
    word = update_bits(word, 0x0000_0380, u32::from(c & 7) << 7);
    word = update_bits(word, 0x0000_0070, u32::from(d & 7) << 4);
    GX_TEV_ALPHA_ENV[stage].store(word, Ordering::Relaxed);
    write_bp_word(word);
}

fn tev_op_word(
    mut word: u32,
    tevop: u8,
    tevbias: u8,
    tevscale: u8,
    clamp: u8,
    tevregid: u8,
) -> u32 {
    word = update_bits(word, 0x0004_0000, u32::from(tevop & 1) << 18);
    if tevop <= GX_TEV_SUB {
        word = update_bits(word, 0x0030_0000, u32::from(tevscale & 3) << 20);
        word = update_bits(word, 0x0003_0000, u32::from(tevbias & 3) << 16);
    } else {
        word = update_bits(word, 0x0030_0000, (u32::from(tevop) << 19) & 0x0030_0000);
        word = update_bits(word, 0x0003_0000, 0x0003_0000);
    }
    word = update_bits(word, 0x0008_0000, u32::from(clamp & 1) << 19);
    update_bits(word, 0x00c0_0000, u32::from(tevregid & 3) << 22)
}

/// # Safety
/// GX must be initialized before this function writes BP state.
pub unsafe fn set_tev_color_op(
    tevstage: u8,
    tevop: u8,
    tevbias: u8,
    tevscale: u8,
    clamp: u8,
    tevregid: u8,
) {
    let stage = usize::from(tevstage & 0x0f);
    let word = tev_op_word(
        GX_TEV_COLOR_ENV[stage].load(Ordering::Relaxed),
        tevop,
        tevbias,
        tevscale,
        clamp,
        tevregid,
    );
    GX_TEV_COLOR_ENV[stage].store(word, Ordering::Relaxed);
    write_bp_word(word);
}

/// # Safety
/// GX must be initialized before this function writes BP state.
pub unsafe fn set_tev_alpha_op(
    tevstage: u8,
    tevop: u8,
    tevbias: u8,
    tevscale: u8,
    clamp: u8,
    tevregid: u8,
) {
    let stage = usize::from(tevstage & 0x0f);
    let word = tev_op_word(
        GX_TEV_ALPHA_ENV[stage].load(Ordering::Relaxed),
        tevop,
        tevbias,
        tevscale,
        clamp,
        tevregid,
    );
    GX_TEV_ALPHA_ENV[stage].store(word, Ordering::Relaxed);
    write_bp_word(word);
}

/// Configures depth testing and depth-buffer updates, matching `GX_SetZMode`.
///
/// # Safety
/// GX must be initialized before this function writes BP state.
pub unsafe fn set_z_mode(enable: u8, func: u8, update_enable: u8) {
    let mut word = GX_PE_Z_MODE.load(Ordering::Relaxed);
    word = update_bits(word, 0x1, u32::from(enable & 1));
    word = update_bits(word, 0x0e, u32::from(func & 7) << 1);
    word = update_bits(word, 0x10, u32::from(update_enable & 1) << 4);
    GX_PE_Z_MODE.store(word, Ordering::Relaxed);
    load_bp_reg((word >> 24) as u8, word & 0x00ff_ffff);
}

/// Selects whether Z comparison occurs before texturing.
/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_z_comp_loc(before_tex: u8) {
    let mut word = GX_PE_CONTROL.load(Ordering::Relaxed);
    word = update_bits(word, 0x40, u32::from(before_tex & 1) << 6);
    GX_PE_CONTROL.store(word, Ordering::Relaxed);
    load_bp_reg((word >> 24) as u8, word & 0x00ff_ffff);
}

/// Sets rasterized line width and texture-offset format.
/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_line_width(width: u8, fmt: u8) {
    let mut word = GX_LP_WIDTH.load(Ordering::Relaxed);
    word = update_bits(word, 0xff, u32::from(width));
    word = update_bits(word, 0x0007_0000, u32::from(fmt & 7) << 16);
    GX_LP_WIDTH.store(word, Ordering::Relaxed);
    load_bp_reg((word >> 24) as u8, word & 0x00ff_ffff);
}

/// Sets rasterized point size and texture-offset format.
/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_point_size(width: u8, fmt: u8) {
    let mut word = GX_LP_WIDTH.load(Ordering::Relaxed);
    word = update_bits(word, 0x0000_ff00, u32::from(width) << 8);
    word = update_bits(word, 0x0038_0000, u32::from(fmt & 7) << 19);
    GX_LP_WIDTH.store(word, Ordering::Relaxed);
    load_bp_reg((word >> 24) as u8, word & 0x00ff_ffff);
}

/// Configures PE blending/logic state, matching `GX_SetBlendMode`.
/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_blend_mode(type_: u8, src_fact: u8, dst_fact: u8, op: u8) {
    let mut word = GX_PE_COLOR_MODE0.load(Ordering::Relaxed);
    word &= !0x1;
    if type_ == GX_BM_BLEND || type_ == GX_BM_SUBTRACT {
        word |= 0x1;
    }
    word &= !0x800;
    if type_ == GX_BM_SUBTRACT {
        word |= 0x800;
    }
    word &= !0x2;
    if type_ == GX_BM_LOGIC {
        word |= 0x2;
    }
    word = update_bits(word, 0x0000_f000, u32::from(op & 0x0f) << 12);
    word = update_bits(word, 0x0000_00e0, u32::from(dst_fact & 7) << 5);
    word = update_bits(word, 0x0000_0700, u32::from(src_fact & 7) << 8);
    GX_PE_COLOR_MODE0.store(word, Ordering::Relaxed);
    load_bp_reg((word >> 24) as u8, word & 0x00ff_ffff);
}

/// Updates the generation-mode cull bits. Invalid values are ignored instead
/// of indexing past libogc's private four-entry lookup table.
/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_cull_mode(mode: u8) {
    const CM_TO_HW: [u32; 4] = [0, 2, 1, 3];
    let Some(hw) = CM_TO_HW.get(mode as usize).copied() else {
        return;
    };
    let current = GX_GEN_MODE.load(Ordering::Relaxed);
    GX_GEN_MODE.store(
        update_bits(current, 0x0000_c000, hw << 14),
        Ordering::Relaxed,
    );
    GX_GEN_MODE_DIRTY.store(true, Ordering::Release);
}

/// Enables or disables co-planar polygon processing.
/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_co_planar(enable: u8) {
    let current = GX_GEN_MODE.load(Ordering::Relaxed);
    let word = update_bits(current, 0x0008_0000, u32::from(enable & 1) << 19);
    GX_GEN_MODE.store(word, Ordering::Relaxed);
    load_bp_reg(0xfe, 0x0008_0000);
    load_bp_reg((word >> 24) as u8, word & 0x00ff_ffff);
}

/// Enables per-coordinate line/point texture offsets.
/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn enable_tex_offsets(coord: u8, line_enable: u8, point_enable: u8) {
    let index = usize::from(coord & 7);
    let mut word = GX_SU_SSIZE[index].load(Ordering::Relaxed);
    word = update_bits(word, 0x0004_0000, u32::from(line_enable & 1) << 18);
    word = update_bits(word, 0x0008_0000, u32::from(point_enable & 1) << 19);
    GX_SU_SSIZE[index].store(word, Ordering::Relaxed);
    load_bp_reg((word >> 24) as u8, word & 0x00ff_ffff);
}

/// Configures XF clipping mode.
/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_clip_mode(mode: u8) {
    load_xf_regs(0x1005, &[u32::from(mode & 1)]);
}

/// Configures alpha-test comparison and combine operation.
/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_alpha_compare(comp0: u8, ref0: u8, aop: u8, comp1: u8, ref1: u8) {
    let value = (u32::from(aop & 3) << 22)
        | (u32::from(comp1 & 7) << 19)
        | (u32::from(comp0 & 7) << 16)
        | (u32::from(ref1) << 8)
        | u32::from(ref0);
    load_bp_reg(0xf3, value);
}

/// Enables or disables RGB writes to the EFB.
fn fog_color_word(color: GxColor) -> u32 {
    0xf200_0000 | (u32::from(color.r) << 16) | (u32::from(color.g) << 8) | u32::from(color.b)
}

/// Matches `GX_SetFog`.
///
/// # Safety
/// GX must be initialized before this function writes BP state.
pub unsafe fn set_fog(
    fog_type: u8,
    start_z: f32,
    end_z: f32,
    near_z: f32,
    far_z: f32,
    color: GxColor,
) {
    let proj = (fog_type >> 3) & 1;
    let (a_f, b_m, b_expn, c) = if proj != 0 {
        if far_z == near_z || end_z == start_z {
            (0.0, 0, 0, 0.0)
        } else {
            let a = 1.0 / (end_z - start_z);
            ((far_z - near_z) * a, 0, 0, (start_z - near_z) * a)
        }
    } else if far_z == near_z || end_z == start_z {
        (0.0, (0.5 * 8_388_638.0) as u32, 1, 0.0)
    } else {
        let a = (far_z * near_z) / ((far_z - near_z) * (end_z - start_z));
        let b = far_z / (far_z - near_z);
        let c = start_z / (end_z - start_z);
        let mut mant = b;
        let mut expn = 1i32;
        while mant > 1.0 {
            mant *= 0.5;
            expn += 1;
        }
        while mant > 0.0 && mant < 0.5 {
            mant *= 2.0;
            expn -= 1;
        }
        let a_f = libm::ldexpf(a, -expn);
        (a_f, (mant * 8_388_638.0) as u32, expn.max(0) as u32, c)
    };
    let a_hex = a_f.to_bits();
    let c_hex = c.to_bits();
    write_bp_word(0xee00_0000 | (a_hex >> 12));
    write_bp_word(0xef00_0000 | (b_m & 0x00ff_ffff));
    write_bp_word(0xf000_0000 | (b_expn & 0x1f));
    write_bp_word(
        0xf100_0000 | (u32::from(fog_type & 7) << 21) | (u32::from(proj) << 20) | (c_hex >> 12),
    );
    write_bp_word(fog_color_word(color));
}

/// Matches `GX_InitFogAdjTable`.
///
/// # Safety
/// `table` and `projection` must be valid non-null pointers. Null pointers are
/// rejected as a safety divergence from C's undefined dereference behavior.
pub unsafe fn init_fog_adj_table(table: *mut GxFogAdjTbl, width: u16, projection: *const Mtx44) {
    unsafe {
        if table.is_null() || projection.is_null() || width == 0 {
            return;
        }
        let matrix = &*projection;
        let (val0, val1) = if matrix[3][3] == 0.0 {
            let val0 = matrix[2][3] / (matrix[2][2] - 1.0);
            (val0, val0 / matrix[0][0])
        } else {
            let val1 = 1.0 / matrix[0][0];
            (val1 * 1.732_05, val1)
        };
        let val2 = val0 * val0;
        let val4 = 2.0 / f32::from(width);
        for i in 0..10 {
            let mut val5 = (i as f32 + 1.0) * 32.0;
            val5 *= val4;
            val5 *= val1;
            val5 = (val5 * val5) / val2;
            let val7 = (libm::sqrtf(val5 + 1.0) * 256.0) as u32;
            (*table).r[i] = (val7 & 0x0fff) as u16;
        }
    }
}

/// Matches `GX_SetFogRangeAdj`.
///
/// # Safety
/// GX must be initialized. When enabled, `table` must be non-null.
pub unsafe fn set_fog_range_adj(enable: u8, center: u16, table: *const GxFogAdjTbl) {
    unsafe {
        if enable != 0 {
            if table.is_null() {
                return;
            }
            let values = &(*table).r;
            for pair in 0..5 {
                let lo = u32::from(values[pair * 2]) & 0x0fff;
                let hi = (u32::from(values[pair * 2 + 1]) & 0x0fff) << 12;
                write_bp_word(((0xe9 + pair as u32) << 24) | hi | lo);
            }
        }
        write_bp_word(
            0xe800_0000
                | (u32::from(enable & 1) << 10)
                | (u32::from(center).wrapping_add(342) & 0x03ff),
        );
    }
}

/// # Safety
/// GX must be initialized before this function writes BP state.
pub unsafe fn set_fog_color(color: GxColor) {
    write_bp_word(fog_color_word(color));
}

/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_color_update(enable: u8) {
    let mut word = GX_PE_COLOR_MODE0.load(Ordering::Relaxed);
    word = update_bits(word, 0x8, u32::from(enable & 1) << 3);
    GX_PE_COLOR_MODE0.store(word, Ordering::Relaxed);
    load_bp_reg((word >> 24) as u8, word & 0x00ff_ffff);
}

/// Enables or disables alpha writes to the EFB.
/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_alpha_update(enable: u8) {
    let mut word = GX_PE_COLOR_MODE0.load(Ordering::Relaxed);
    word = update_bits(word, 0x10, u32::from(enable & 1) << 4);
    GX_PE_COLOR_MODE0.store(word, Ordering::Relaxed);
    load_bp_reg((word >> 24) as u8, word & 0x00ff_ffff);
}

/// Configures EFB color/depth format. Invalid pixel formats are rejected
/// rather than indexing beyond libogc's private mapping table.
/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_pixel_fmt(pix_fmt: u8, z_fmt: u8) {
    const REAL_FMT: [u32; 8] = [0, 1, 2, 3, 4, 4, 4, 5];
    let Some(real_fmt) = REAL_FMT.get(pix_fmt as usize).copied() else {
        return;
    };

    let mut control = GX_PE_CONTROL.load(Ordering::Relaxed);
    control = update_bits(control, 0x7, real_fmt & 7);
    control = update_bits(control, 0x38, u32::from(z_fmt & 7) << 3);
    GX_PE_CONTROL.store(control, Ordering::Relaxed);
    load_bp_reg((control >> 24) as u8, control & 0x00ff_ffff);

    let ms_enable = u32::from(pix_fmt == GX_PF_RGB565_Z16);
    let r#gen = GX_GEN_MODE.load(Ordering::Relaxed);
    GX_GEN_MODE.store(update_bits(r#gen, 0x200, ms_enable << 9), Ordering::Relaxed);
    GX_GEN_MODE_DIRTY.store(true, Ordering::Release);

    if real_fmt == u32::from(GX_PF_Y8) {
        let yuv_fmt = pix_fmt.wrapping_sub(GX_PF_Y8);
        let mut mode1 = GX_PE_COLOR_MODE1.load(Ordering::Relaxed);
        mode1 = update_bits(mode1, 0x0c00, u32::from(yuv_fmt & 3) << 10);
        GX_PE_COLOR_MODE1.store(mode1, Ordering::Relaxed);
        load_bp_reg((mode1 >> 24) as u8, mode1 & 0x00ff_ffff);
    }
}

/// Enables/disables PE dithering.
/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_dither(dither: u8) {
    let mut word = GX_PE_COLOR_MODE0.load(Ordering::Relaxed);
    word = update_bits(word, 0x4, u32::from(dither & 1) << 2);
    GX_PE_COLOR_MODE0.store(word, Ordering::Relaxed);
    load_bp_reg((word >> 24) as u8, word & 0x00ff_ffff);
}

/// Configures destination-alpha override.
/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_dst_alpha(enable: u8, a: u8) {
    let mut word = GX_PE_COLOR_MODE1.load(Ordering::Relaxed);
    word = update_bits(word, 0xff, u32::from(a));
    word = update_bits(word, 0x100, u32::from(enable & 1) << 8);
    GX_PE_COLOR_MODE1.store(word, Ordering::Relaxed);
    load_bp_reg((word >> 24) as u8, word & 0x00ff_ffff);
}

/// Configures one of libogc's five default TEV combiner modes.
///
/// # Safety
/// GX must be initialized before this function writes BP state.
pub unsafe fn set_tev_op(tevstage: u8, mode: u8) {
    let (defcolor, defalpha) = if tevstage == GX_TEVSTAGE0 {
        (GX_CC_RASC, GX_CA_RASA)
    } else {
        (GX_CC_CPREV, GX_CA_APREV)
    };
    match mode {
        GX_MODULATE => {
            unsafe { set_tev_color_in(tevstage, GX_CC_ZERO, GX_CC_TEXC, defcolor, GX_CC_ZERO) };
            unsafe { set_tev_alpha_in(tevstage, GX_CA_ZERO, GX_CA_TEXA, defalpha, GX_CA_ZERO) };
        }
        GX_DECAL => {
            unsafe { set_tev_color_in(tevstage, defcolor, GX_CC_TEXC, GX_CC_TEXA, GX_CC_ZERO) };
            unsafe { set_tev_alpha_in(tevstage, GX_CA_ZERO, GX_CA_ZERO, GX_CA_ZERO, defalpha) };
        }
        GX_BLEND => {
            unsafe { set_tev_color_in(tevstage, defcolor, GX_CC_ONE, GX_CC_TEXC, GX_CC_ZERO) };
            unsafe { set_tev_alpha_in(tevstage, GX_CA_ZERO, GX_CA_TEXA, defalpha, GX_CA_RASA) };
        }
        GX_REPLACE => {
            unsafe { set_tev_color_in(tevstage, GX_CC_ZERO, GX_CC_ZERO, GX_CC_ZERO, GX_CC_TEXC) };
            unsafe { set_tev_alpha_in(tevstage, GX_CA_ZERO, GX_CA_ZERO, GX_CA_ZERO, GX_CA_TEXA) };
        }
        GX_PASSCLR => {
            unsafe { set_tev_color_in(tevstage, GX_CC_ZERO, GX_CC_ZERO, GX_CC_ZERO, defcolor) };
            unsafe { set_tev_alpha_in(tevstage, GX_CA_ZERO, GX_CA_ZERO, GX_CA_ZERO, defalpha) };
        }
        _ => {}
    }
    unsafe {
        set_tev_color_op(
            tevstage,
            GX_TEV_ADD,
            GX_TB_ZERO,
            GX_CS_SCALE_1,
            GX_TRUE,
            GX_TEVPREV,
        )
    };
    unsafe {
        set_tev_alpha_op(
            tevstage,
            GX_TEV_ADD,
            GX_TB_ZERO,
            GX_CS_SCALE_1,
            GX_TRUE,
            GX_TEVPREV,
        )
    };
}

/// # Safety
/// GX must be initialized before this function writes BP state.
pub unsafe fn set_tev_order(tevstage: u8, texcoord: u8, texmap: u32, color: u8) {
    let stage = usize::from(tevstage & 0x0f);
    GX_TEV_TEX_MAP[stage].store(texmap, Ordering::Relaxed);
    let texm_raw = texmap & !GX_TEXMAP_DISABLE;
    let texm = if texm_raw >= GX_MAX_TEXMAP {
        0
    } else {
        texm_raw
    };
    let (texc, enabled) = if texcoord >= 8 {
        (0u32, false)
    } else {
        (u32::from(texcoord), true)
    };
    let bit = 1u32 << stage;
    if enabled {
        GX_TEV_TEX_COORD_ENABLE.fetch_or(bit, Ordering::Relaxed);
    } else {
        GX_TEV_TEX_COORD_ENABLE.fetch_and(!bit, Ordering::Relaxed);
    }
    let reg = 3 + usize::from((tevstage >> 1) & 7);
    let mut word = GX_TEV_RAS_ORDER[reg].load(Ordering::Relaxed);
    let colid = u32::from(tev_color_id(color));
    let lookup = u32::from(texmap != GX_TEXMAP_NULL && (texmap & GX_TEXMAP_DISABLE) == 0);
    if tevstage & 1 != 0 {
        word = update_bits(word, 0x0000_7000, texm << 12);
        word = update_bits(word, 0x0003_8000, texc << 15);
        word = update_bits(word, 0x0038_0000, colid << 19);
        word = update_bits(word, 0x0004_0000, lookup << 18);
    } else {
        word = update_bits(word, 0x0000_0007, texm);
        word = update_bits(word, 0x0000_0038, texc << 3);
        word = update_bits(word, 0x0000_0380, colid << 7);
        word = update_bits(word, 0x0000_0040, lookup << 6);
    }
    GX_TEV_RAS_ORDER[reg].store(word, Ordering::Relaxed);
    write_bp_word(word);
}

/// # Safety
fn packed_channel_color(color: GxColor) -> u32 {
    (u32::from(color.r) << 24)
        | (u32::from(color.g) << 16)
        | (u32::from(color.b) << 8)
        | u32::from(color.a)
}

fn update_channel_color(current: u32, channel: i32, color: GxColor) -> Option<(usize, u32)> {
    let rgb = packed_channel_color(color) & !0xff;
    match channel {
        x if x == i32::from(GX_COLOR0) => Some((0, rgb | (current & 0xff))),
        x if x == i32::from(GX_COLOR1) => Some((1, rgb | (current & 0xff))),
        x if x == i32::from(GX_ALPHA0) || x == i32::from(GX_ALPHA1) => Some((
            (channel - i32::from(GX_ALPHA0)) as usize,
            (current & !0xff) | u32::from(color.a),
        )),
        x if x == i32::from(GX_COLOR0A0) => Some((0, packed_channel_color(color))),
        x if x == i32::from(GX_COLOR1A1) => Some((1, packed_channel_color(color))),
        _ => None,
    }
}

/// Configures one lighting channel, matching `GX_SetChanCtrl`.
/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_chan_ctrl(
    channel: i32,
    enable: u8,
    ambsrc: u8,
    matsrc: u8,
    litmask: u8,
    diff_fn: u8,
    attn_fn: u8,
) {
    let diff = if attn_fn == GX_AF_SPEC {
        GX_DF_NONE
    } else {
        diff_fn
    };
    let value = u32::from(matsrc & 1)
        | (u32::from(enable & 1) << 1)
        | (u32::from(litmask & 0x0f) << 2)
        | (u32::from(ambsrc & 1) << 6)
        | (u32::from(diff & 3) << 7)
        | (u32::from(GX_AF_NONE > attn_fn) << 9)
        | (u32::from(attn_fn > 0) << 10)
        | (u32::from((litmask >> 4) & 0x0f) << 11);
    let reg = (channel as u32 & 3) as usize;
    GX_CHAN_CONTROL[reg].store(value, Ordering::Relaxed);
    GX_CHAN_CONTROL_DIRTY.fetch_or(1 << reg, Ordering::AcqRel);

    if channel == i32::from(GX_COLOR0A0) {
        GX_CHAN_CONTROL[2].store(value, Ordering::Relaxed);
        GX_CHAN_CONTROL_DIRTY.fetch_or(0x5, Ordering::AcqRel);
    } else {
        GX_CHAN_CONTROL[3].store(value, Ordering::Relaxed);
        GX_CHAN_CONTROL_DIRTY.fetch_or(0xa, Ordering::AcqRel);
    }
}

fn set_channel_color(table: &[AtomicU32; 2], dirty_base: u32, channel: i32, color: GxColor) {
    let index = match channel {
        x if x == i32::from(GX_COLOR0)
            || x == i32::from(GX_ALPHA0)
            || x == i32::from(GX_COLOR0A0) =>
        {
            0
        }
        x if x == i32::from(GX_COLOR1)
            || x == i32::from(GX_ALPHA1)
            || x == i32::from(GX_COLOR1A1) =>
        {
            1
        }
        _ => return,
    };
    let current = table[index].load(Ordering::Relaxed);
    let Some((resolved, value)) = update_channel_color(current, channel, color) else {
        return;
    };
    table[resolved].store(value, Ordering::Relaxed);
    GX_CHAN_COLOR_DIRTY.fetch_or(dirty_base << resolved, Ordering::AcqRel);
}

/// Sets a channel ambient color.
/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_chan_amb_color(channel: i32, color: GxColor) {
    set_channel_color(&GX_CHAN_AMB_COLOR, 0x1, channel, color);
}

/// Sets a channel material color.
/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_chan_mat_color(channel: i32, color: GxColor) {
    set_channel_color(&GX_CHAN_MAT_COLOR, 0x4, channel, color);
}

/// Sets the number of active color channels.
/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_num_chans(num: u8) {
    let current = GX_GEN_MODE.load(Ordering::Relaxed);
    GX_GEN_MODE.store(
        update_bits(current, 0x70, u32::from(num & 7) << 4),
        Ordering::Relaxed,
    );
    GX_GEN_MODE_DIRTY.store(true, Ordering::Release);
    GX_CHAN_COUNT_DIRTY.store(true, Ordering::Release);
}

/// GX must be initialized before this function changes generation state.
/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_num_tev_stages(num: u8) {
    let current = GX_GEN_MODE.load(Ordering::Relaxed);
    let word = update_bits(
        current,
        0x0000_3c00,
        u32::from(num.wrapping_sub(1) & 0x0f) << 10,
    );
    GX_GEN_MODE.store(word, Ordering::Relaxed);
    GX_GEN_MODE_DIRTY.store(true, Ordering::Release);
}

/// # Safety
/// GX must be initialized before this function writes BP state.
pub unsafe fn set_tev_k_color_sel(tevstage: u8, sel: u8) {
    let reg = usize::from((tevstage >> 1) & 7);
    let mut word = GX_TEV_SWAP_MODE_TABLE[reg].load(Ordering::Relaxed);
    if tevstage & 1 != 0 {
        word = update_bits(word, 0x0007_c000, u32::from(sel & 0x1f) << 14);
    } else {
        word = update_bits(word, 0x0000_01f0, u32::from(sel & 0x1f) << 4);
    }
    GX_TEV_SWAP_MODE_TABLE[reg].store(word, Ordering::Relaxed);
    write_bp_word(word);
}

/// # Safety
/// GX must be initialized before this function writes BP state.
pub unsafe fn set_tev_k_alpha_sel(tevstage: u8, sel: u8) {
    let reg = usize::from((tevstage >> 1) & 7);
    let mut word = GX_TEV_SWAP_MODE_TABLE[reg].load(Ordering::Relaxed);
    if tevstage & 1 != 0 {
        word = update_bits(word, 0x00f8_0000, u32::from(sel & 0x1f) << 19);
    } else {
        word = update_bits(word, 0x0000_3e00, u32::from(sel & 0x1f) << 9);
    }
    GX_TEV_SWAP_MODE_TABLE[reg].store(word, Ordering::Relaxed);
    write_bp_word(word);
}

/// # Safety
/// GX must be initialized before this function writes BP state.
pub unsafe fn set_tev_swap_mode(tevstage: u8, ras_sel: u8, tex_sel: u8) {
    let stage = usize::from(tevstage & 0x0f);
    let mut word = GX_TEV_ALPHA_ENV[stage].load(Ordering::Relaxed);
    word = update_bits(word, 0x3, u32::from(ras_sel & 3));
    word = update_bits(word, 0xc, u32::from(tex_sel & 3) << 2);
    GX_TEV_ALPHA_ENV[stage].store(word, Ordering::Relaxed);
    write_bp_word(word);
}

/// # Safety
/// GX must be initialized before this function writes BP state.
pub unsafe fn set_tev_swap_mode_table(swapid: u8, r: u8, g: u8, b: u8, a: u8) {
    let reg_a = usize::from((swapid & 3) << 1);
    let reg_b = reg_a + 1;
    let mut wa = GX_TEV_SWAP_MODE_TABLE[reg_a].load(Ordering::Relaxed);
    wa = update_bits(wa, 0x3, u32::from(r & 3));
    wa = update_bits(wa, 0xc, u32::from(g & 3) << 2);
    GX_TEV_SWAP_MODE_TABLE[reg_a].store(wa, Ordering::Relaxed);
    write_bp_word(wa);
    let mut wb = GX_TEV_SWAP_MODE_TABLE[reg_b].load(Ordering::Relaxed);
    wb = update_bits(wb, 0x3, u32::from(b & 3));
    wb = update_bits(wb, 0xc, u32::from(a & 3) << 2);
    GX_TEV_SWAP_MODE_TABLE[reg_b].store(wb, Ordering::Relaxed);
    write_bp_word(wb);
}

/// Configures one TEV stage's indirect-texture operation.
///
/// # Safety
/// GX must be initialized before this function writes BP state.
pub unsafe fn set_tev_indirect(tevstage: u8, params: GxTevIndirect) {
    let GxTevIndirect {
        indtexid,
        format,
        bias,
        mtxid,
        wrap_s,
        wrap_t,
        addprev,
        utclod,
        alpha,
    } = params;
    let word = (0x10u32 + u32::from(tevstage & 0x0f)) << 24
        | u32::from(indtexid & 3)
        | (u32::from(format & 3) << 2)
        | (u32::from(bias & 7) << 4)
        | (u32::from(alpha & 3) << 7)
        | (u32::from(mtxid & 0x0f) << 9)
        | (u32::from(wrap_s & 7) << 13)
        | (u32::from(wrap_t & 7) << 16)
        | (u32::from(utclod & 1) << 19)
        | (u32::from(addprev & 1) << 20);
    write_bp_word(word);
}

/// # Safety
/// GX must be initialized before this function writes BP state.
pub unsafe fn set_tev_direct(tevstage: u8) {
    unsafe {
        set_tev_indirect(
            tevstage,
            GxTevIndirect {
                indtexid: GX_INDTEXSTAGE0,
                format: GX_ITF_8,
                bias: GX_ITB_NONE,
                mtxid: GX_ITM_OFF,
                wrap_s: GX_ITW_OFF,
                wrap_t: GX_ITW_OFF,
                addprev: GX_FALSE,
                utclod: GX_FALSE,
                alpha: GX_ITBA_OFF,
            },
        )
    };
}

/// # Safety
/// GX must be initialized before this function changes generation state.
pub unsafe fn set_num_ind_stages(nstages: u8) {
    let current = GX_GEN_MODE.load(Ordering::Relaxed);
    GX_GEN_MODE.store(
        update_bits(current, 0x0007_0000, u32::from(nstages & 7) << 16),
        Ordering::Relaxed,
    );
    GX_GEN_MODE_DIRTY.store(true, Ordering::Release);
    GX_TEV_IND_MASK_DIRTY.store(true, Ordering::Release);
}

fn ind_matrix_index(indtexmtx: u8) -> u8 {
    if (GX_ITM_0..=GX_ITM_2).contains(&indtexmtx) {
        indtexmtx - GX_ITM_0
    } else if (GX_ITM_S0..=GX_ITM_S2).contains(&indtexmtx) {
        indtexmtx - GX_ITM_S0
    } else if (GX_ITM_T0..=GX_ITM_T2).contains(&indtexmtx) {
        indtexmtx - GX_ITM_T0
    } else {
        GX_ITM_OFF
    }
}

fn ind_matrix_words(indtexmtx: u8, matrix: [[f32; 3]; 2], scale_exp: i8) -> [u32; 3] {
    let index = u32::from(ind_matrix_index(indtexmtx));
    let scale = i32::from(scale_exp) + 17;
    let scale = scale as u32;
    let mut words = [0u32; 3];
    for column in 0..3usize {
        let ma = (matrix[0][column] * 1024.0) as i32 as u32;
        let mb = (matrix[1][column] * 1024.0) as i32 as u32;
        let scale_bits = (scale >> (column * 2)) & 3;
        words[column] = ((0x06 + index * 3 + column as u32) << 24)
            | (scale_bits << 22)
            | ((mb & 0x7ff) << 11)
            | (ma & 0x7ff);
    }
    words
}

/// Loads one indirect texture matrix.
///
/// # Safety
/// `offset_mtx` must point to a readable 2x3 matrix and GX must be initialized.
pub unsafe fn set_ind_tex_matrix(indtexmtx: u8, offset_mtx: *const [[f32; 3]; 2], scale_exp: i8) {
    if offset_mtx.is_null() {
        return;
    }
    let words = ind_matrix_words(indtexmtx, unsafe { *offset_mtx }, scale_exp);
    for word in words {
        write_bp_word(word);
    }
}

/// # Safety
/// GX must be initialized before this function writes BP state.
pub unsafe fn set_tev_ind_bump_st(tevstage: u8, indstage: u8, mtx_sel: u8) {
    let (sel_s, sel_t) = match mtx_sel {
        GX_ITM_0 => (GX_ITM_S0, GX_ITM_T0),
        GX_ITM_1 => (GX_ITM_S1, GX_ITM_T1),
        GX_ITM_2 => (GX_ITM_S2, GX_ITM_T2),
        _ => (GX_ITM_OFF, GX_ITM_OFF),
    };
    unsafe {
        set_tev_indirect(
            tevstage,
            GxTevIndirect {
                indtexid: indstage,
                format: GX_ITF_8,
                bias: GX_ITB_ST,
                mtxid: sel_s,
                wrap_s: GX_ITW_0,
                wrap_t: GX_ITW_0,
                addprev: GX_FALSE,
                utclod: GX_FALSE,
                alpha: GX_ITBA_OFF,
            },
        );
        set_tev_indirect(
            tevstage.wrapping_add(1),
            GxTevIndirect {
                indtexid: indstage,
                format: GX_ITF_8,
                bias: GX_ITB_ST,
                mtxid: sel_t,
                wrap_s: GX_ITW_0,
                wrap_t: GX_ITW_0,
                addprev: GX_TRUE,
                utclod: GX_FALSE,
                alpha: GX_ITBA_OFF,
            },
        );
        set_tev_indirect(
            tevstage.wrapping_add(2),
            GxTevIndirect {
                indtexid: indstage,
                format: GX_ITF_8,
                bias: GX_ITB_NONE,
                mtxid: GX_ITM_OFF,
                wrap_s: GX_ITW_OFF,
                wrap_t: GX_ITW_OFF,
                addprev: GX_TRUE,
                utclod: GX_FALSE,
                alpha: GX_ITBA_OFF,
            },
        );
    }
}

/// # Safety
/// GX must be initialized before this function writes BP state.
pub unsafe fn set_tev_ind_bump_xyz(tevstage: u8, indstage: u8, mtx_sel: u8) {
    unsafe {
        set_tev_indirect(
            tevstage,
            GxTevIndirect {
                indtexid: indstage,
                format: GX_ITF_8,
                bias: GX_ITB_STU,
                mtxid: mtx_sel,
                wrap_s: GX_ITW_OFF,
                wrap_t: GX_ITW_OFF,
                addprev: GX_FALSE,
                utclod: GX_FALSE,
                alpha: GX_ITBA_OFF,
            },
        )
    };
}

/// # Safety
/// GX must be initialized before this function writes BP state.
pub unsafe fn set_tev_ind_repeat(tevstage: u8) {
    unsafe {
        set_tev_indirect(
            tevstage,
            GxTevIndirect {
                indtexid: GX_INDTEXSTAGE0,
                format: GX_ITF_8,
                bias: GX_ITB_NONE,
                mtxid: GX_ITM_OFF,
                wrap_s: GX_ITW_0,
                wrap_t: GX_ITW_0,
                addprev: GX_TRUE,
                utclod: GX_FALSE,
                alpha: GX_ITBA_OFF,
            },
        )
    };
}

/// # Safety
/// GX must be initialized before this function writes BP state.
pub unsafe fn set_ind_tex_coord_scale(indtexid: u8, scale_s: u8, scale_t: u8) {
    let (reg, shift) = match indtexid {
        GX_INDTEXSTAGE0 => (0usize, 0u32),
        GX_INDTEXSTAGE1 => (0usize, 8u32),
        GX_INDTEXSTAGE2 => (1usize, 0u32),
        GX_INDTEXSTAGE3 => (1usize, 8u32),
        _ => return,
    };
    let mut word = GX_TEV_RAS_ORDER[reg].load(Ordering::Relaxed);
    word = update_bits(word, 0x0f << shift, u32::from(scale_s & 0x0f) << shift);
    word = update_bits(
        word,
        0xf0 << shift,
        u32::from(scale_t & 0x0f) << (shift + 4),
    );
    GX_TEV_RAS_ORDER[reg].store(word, Ordering::Relaxed);
    write_bp_word(word);
}

/// # Safety
/// GX must be initialized before this function writes BP state.
pub unsafe fn set_tev_ind_warp(
    tevstage: u8,
    indtexid: u8,
    bias_flag: u8,
    replace_tex: u8,
    mtxid: u8,
) {
    let wrap = if replace_tex != 0 {
        GX_ITW_0
    } else {
        GX_ITW_OFF
    };
    let bias = if bias_flag != 0 {
        GX_ITB_STU
    } else {
        GX_ITB_NONE
    };
    unsafe {
        set_tev_indirect(
            tevstage,
            GxTevIndirect {
                indtexid,
                format: GX_ITF_8,
                bias,
                mtxid,
                wrap_s: wrap,
                wrap_t: wrap,
                addprev: GX_FALSE,
                utclod: GX_FALSE,
                alpha: GX_ITBA_OFF,
            },
        )
    };
}

/// # Safety
/// GX must be initialized before this function writes BP state.
pub unsafe fn set_tev_ind_tile(tevstage: u8, params: GxTevIndirectTile) {
    let GxTevIndirectTile {
        indtexid,
        tilesize_x,
        tilesize_y,
        tilespacing_x,
        tilespacing_y,
        indtexfmt,
        indtexmtx,
        bias_sel,
        alpha_sel,
    } = params;
    fn tile_wrap(size: u16) -> u8 {
        match size {
            16 => GX_ITW_16,
            32 => GX_ITW_32,
            64 => GX_ITW_64,
            128 => GX_ITW_128,
            256 => GX_ITW_256,
            _ => GX_ITW_OFF,
        }
    }
    let matrix = [
        [f32::from(tilespacing_x) / 1024.0, 0.0, 0.0],
        [0.0, f32::from(tilespacing_y) / 1024.0, 0.0],
    ];
    unsafe { set_ind_tex_matrix(indtexmtx, &matrix, 10) };
    unsafe {
        set_tev_indirect(
            tevstage,
            GxTevIndirect {
                indtexid,
                format: indtexfmt,
                bias: bias_sel,
                mtxid: indtexmtx,
                wrap_s: tile_wrap(tilesize_x),
                wrap_t: tile_wrap(tilesize_y),
                addprev: GX_FALSE,
                utclod: GX_TRUE,
                alpha: alpha_sel,
            },
        )
    };
}

/// # Safety
/// GX must be initialized before this function writes BP state.
/// Configures the PE alpha compare used by direct EFB pokes.
fn perf0_program_word(mode: u32) -> Option<(bool, u32)> {
    const XF: [u32; 11] = [
        0x14a, 0x16b, 0x084, 0x0c6, 0x210, 0x252, 0x231, 0x1ad, 0x1ce, 0x021, 0x153,
    ];
    const TRI: [u32; 16] = [
        0x2300ae7f, 0x23008e7f, 0x23009e7f, 0x23001e7f, 0x2300ac3f, 0x2300ac7f, 0x2300acbf,
        0x2300acff, 0x2300ad3f, 0x2300ad7f, 0x2300adbf, 0x2300adff, 0x2300ae3f, 0x2300a27f,
        0x2300a67f, 0x2300aa7f,
    ];
    const QUAD: [u32; 7] = [
        0x2402c0c6, 0x2402c16b, 0x2402c0e7, 0x2402c108, 0x2402c129, 0x2402c14a, 0x2402c1ad,
    ];
    match mode {
        GX_PERF0_VERTICES..=GX_PERF0_CLIP_RATIO => Some((true, XF[mode as usize])),
        GX_PERF0_TRIANGLES..=GX_PERF0_TRIANGLES_2CLR => {
            Some((false, TRI[(mode - GX_PERF0_TRIANGLES) as usize]))
        }
        GX_PERF0_QUAD_0CVG..=GX_PERF0_AVG_QUAD_CNT => {
            Some((false, QUAD[(mode - GX_PERF0_QUAD_0CVG) as usize]))
        }
        GX_PERF0_CLOCKS => Some((true, 0x273)),
        _ => None,
    }
}

fn perf1_bp_word(mode: u32) -> Option<u32> {
    const WORDS: [u32; 9] = [
        0x67000084, 0x67000063, 0x67000129, 0x67000252, 0x67000021, 0x6700014b, 0x6700018d,
        0x670001cf, 0x67000211,
    ];
    match mode {
        GX_PERF1_TEXELS..=GX_PERF1_TC_MISS => Some(WORDS[mode as usize]),
        GX_PERF1_CLOCKS => Some(0x67000042),
        _ => None,
    }
}

pub fn get_overflow_count() -> u32 {
    GX_OVERFLOW_COUNT.load(Ordering::Acquire)
}

pub fn reset_overflow_count() -> u32 {
    GX_OVERFLOW_COUNT.swap(0, Ordering::AcqRel)
}

pub fn get_current_gx_thread() -> u32 {
    GX_CURRENT_THREAD.load(Ordering::Acquire)
}

pub fn set_current_gx_thread() -> u32 {
    GX_CURRENT_THREAD.swap(ogc_lwp_sys::get_self(), Ordering::AcqRel)
}

pub fn set_gp_metric(perf0: u32, perf1: u32) {
    let old0 = GX_PERF0_MODE.load(Ordering::Relaxed);
    if (GX_PERF0_TRIANGLES..GX_PERF0_QUAD_0CVG).contains(&old0) {
        load_bp_reg(0x23, 0);
    } else if (GX_PERF0_QUAD_0CVG..GX_PERF0_CLOCKS).contains(&old0) {
        load_bp_reg(0x24, 0);
    } else if old0 <= GX_PERF0_CLOCKS {
        load_xf_regs(0x1006, &[0]);
    }

    let old1 = GX_PERF1_MODE.load(Ordering::Relaxed);
    if (GX_PERF1_VC_ELEMQ_FULL..GX_PERF1_FIFO_REQ).contains(&old1) {
        let mode = GX_CP_PERF_MODE.load(Ordering::Relaxed) & !0xf0;
        GX_CP_PERF_MODE.store(mode, Ordering::Relaxed);
        load_cp_reg(0x20, mode);
    } else if (GX_PERF1_FIFO_REQ..GX_PERF1_CLOCKS).contains(&old1) {
        unsafe { ptr::write_volatile(cp_reg(3), 0) };
    } else if old1 <= GX_PERF1_CLOCKS {
        load_bp_reg(0x67, 0);
    }

    GX_PERF0_MODE.store(perf0, Ordering::Relaxed);
    if let Some((xf, word)) = perf0_program_word(perf0) {
        if xf {
            load_xf_regs(0x1006, &[word]);
        } else {
            load_bp_reg((word >> 24) as u8, word & 0x00ff_ffff);
        }
    }

    GX_PERF1_MODE.store(perf1, Ordering::Relaxed);
    if let Some(word) = perf1_bp_word(perf1) {
        load_bp_reg((word >> 24) as u8, word & 0x00ff_ffff);
    } else if (GX_PERF1_VC_ELEMQ_FULL..=GX_PERF1_VERTICES).contains(&perf1) {
        const MODES: [u32; 8] = [0x20, 0x30, 0x40, 0x50, 0x60, 0x70, 0x90, 0x80];
        let mode = (GX_CP_PERF_MODE.load(Ordering::Relaxed) & !0xf0)
            | MODES[(perf1 - GX_PERF1_VC_ELEMQ_FULL) as usize];
        GX_CP_PERF_MODE.store(mode, Ordering::Relaxed);
        load_cp_reg(0x20, mode);
    } else if (GX_PERF1_FIFO_REQ..=GX_PERF1_CP_ALL_REQ).contains(&perf1) {
        unsafe { ptr::write_volatile(cp_reg(3), (perf1 - GX_PERF1_FIFO_REQ + 2) as u16) };
    }
}

pub fn clear_gp_metric() {
    unsafe { ptr::write_volatile(cp_reg(2), 4) };
}

pub fn init_xf_ras_metric() {
    load_bp_reg(0x24, 0x02c022);
    load_xf_regs(0x1006, &[0x31000]);
}

/// # Safety
/// Non-null output pointers must each be writable for one `u32`.
pub unsafe fn read_xf_ras_metric(
    xfwaitin: *mut u32,
    xfwaitout: *mut u32,
    rasbusy: *mut u32,
    clks: *mut u32,
) {
    for (out, value) in [
        (rasbusy, read_cp_pair(32)),
        (clks, read_cp_pair(34)),
        (xfwaitin, read_cp_pair(36)),
        (xfwaitout, read_cp_pair(38)),
    ] {
        if !out.is_null() {
            unsafe { ptr::write(out, value) };
        }
    }
}

pub fn read_clks_per_vtx() -> u32 {
    draw_done();
    unsafe {
        ptr::write_volatile(cp_reg(49), 0x1007);
        ptr::write_volatile(cp_reg(48), 0x1007);
    }
    u32::from(read_cp_u16(50) >> 8)
}

pub fn clear_v_cache_metric() {
    load_cp_reg(0, 0);
}

/// # Safety
/// Non-null output pointers must each be writable for one `u32`.
pub unsafe fn read_v_cache_metric(check: *mut u32, miss: *mut u32, stall: *mut u32) {
    for (out, value) in [
        (check, read_cp_pair(40)),
        (miss, read_cp_pair(42)),
        (stall, read_cp_pair(44)),
    ] {
        if !out.is_null() {
            unsafe { ptr::write(out, value) };
        }
    }
}

pub fn set_v_cache_metric(_attr: u32) {
    // libogc intentionally leaves this public API empty.
}

/// # Safety
/// Every non-null output must be writable for one byte.
pub unsafe fn get_gp_status(
    overhi: *mut u8,
    underlow: *mut u8,
    read_idle: *mut u8,
    cmd_idle: *mut u8,
    brkpt: *mut u8,
) {
    let status = read_cp_u16(0);
    for (out, value) in [
        (overhi, (status & 1 != 0) as u8),
        (underlow, (status & 2 != 0) as u8),
        (read_idle, (status & 4 != 0) as u8),
        (cmd_idle, (status & 8 != 0) as u8),
        (brkpt, (status & 16 != 0) as u8),
    ] {
        if !out.is_null() {
            unsafe { ptr::write(out, value) };
        }
    }
}

/// # Safety
/// `cnt0`/`cnt1`, when non-null, must be writable for one `u32`.
pub unsafe fn read_gp_metric(cnt0: *mut u32, _cnt1: *mut u32) {
    if cnt0.is_null() {
        return;
    }
    let reg1 = read_cp_pair(32);
    let reg2 = read_cp_pair(34);
    let mode = GX_PERF0_MODE.load(Ordering::Relaxed);
    let value = if mode == GX_PERF0_CLIP_RATIO {
        reg2.wrapping_mul(1000).checked_div(reg1).unwrap_or(0)
    } else if (GX_PERF0_VERTICES..GX_PERF0_NONE).contains(&mode) {
        reg1
    } else {
        0
    };
    unsafe { ptr::write(cnt0, value) };
    // libogc leaves cnt1 untouched; preserve that observable behavior.
}

/// # Safety
/// Every non-null output must be writable for one `u16`.
pub unsafe fn read_bounding_box(top: *mut u16, bottom: *mut u16, left: *mut u16, right: *mut u16) {
    for (out, value) in [
        (left, unsafe { ptr::read_volatile(pe_reg(8)) }),
        (right, unsafe { ptr::read_volatile(pe_reg(9)) }),
        (top, unsafe { ptr::read_volatile(pe_reg(10)) }),
        (bottom, unsafe { ptr::read_volatile(pe_reg(11)) }),
    ] {
        if !out.is_null() {
            unsafe { ptr::write(out, value) };
        }
    }
}

pub fn poke_alpha_mode(func: u8, threshold: u8) {
    unsafe { ptr::write_volatile(pe_reg(3), (u16::from(func) << 8) | u16::from(threshold)) };
}

/// Configures how EFB alpha is returned by `peek_argb`.
pub fn poke_alpha_read(mode: u8) {
    unsafe { ptr::write_volatile(pe_reg(4), (u16::from(mode) & !0x4) | 0x4) };
}

/// Configures constant destination alpha for direct EFB pokes.
pub fn poke_dst_alpha(enable: u8, alpha: u8) {
    unsafe { ptr::write_volatile(pe_reg(2), ((u16::from(enable) & 1) << 8) | u16::from(alpha)) };
}

pub fn poke_alpha_update(enable: u8) {
    unsafe {
        let word = ptr::read_volatile(pe_reg(1));
        ptr::write_volatile(pe_reg(1), update_pe_bit(word, 4, enable));
    }
}

pub fn poke_color_update(enable: u8) {
    unsafe {
        let word = ptr::read_volatile(pe_reg(1));
        ptr::write_volatile(pe_reg(1), update_pe_bit(word, 3, enable));
    }
}

pub fn poke_dither(enable: u8) {
    unsafe {
        let word = ptr::read_volatile(pe_reg(1));
        ptr::write_volatile(pe_reg(1), update_pe_bit(word, 2, enable));
    }
}

pub fn poke_blend_mode(type_: u8, src_fact: u8, dst_fact: u8, op: u8) {
    unsafe {
        let word = ptr::read_volatile(pe_reg(1));
        ptr::write_volatile(
            pe_reg(1),
            poke_blend_word(word, type_, src_fact, dst_fact, op),
        );
    }
}

pub fn poke_argb(x: u16, y: u16, color: GxColor) {
    let value = (u32::from(color.a) << 24)
        | (u32::from(color.r) << 16)
        | (u32::from(color.g) << 8)
        | u32::from(color.b);
    unsafe { ptr::write_volatile(efb_poke_addr(x, y, false), value) };
}

/// # Safety
/// `color` may be null; unlike C, a null output pointer is rejected.
pub unsafe fn peek_argb(x: u16, y: u16, color: *mut GxColor) {
    if color.is_null() {
        return;
    }
    let value = unsafe { ptr::read_volatile(efb_poke_addr(x, y, false)) };
    unsafe {
        *color = GxColor {
            a: (value >> 24) as u8,
            r: (value >> 16) as u8,
            g: (value >> 8) as u8,
            b: value as u8,
        };
    }
}

pub fn poke_z(x: u16, y: u16, z: u32) {
    unsafe { ptr::write_volatile(efb_poke_addr(x, y, true), z) };
}

/// # Safety
/// `z` may be null; unlike C, a null output pointer is rejected.
pub unsafe fn peek_z(x: u16, y: u16, z: *mut u32) {
    if z.is_null() {
        return;
    }
    unsafe { *z = ptr::read_volatile(efb_poke_addr(x, y, true)) };
}

/// Matches libogc's `GX_PokeZMode`, including its final assignment quirk that
/// leaves only the update-enable bit in the PE register.
pub fn poke_z_mode(comp_enable: u8, func: u8, update_enable: u8) {
    let mut word = u16::from(comp_enable) & 1;
    word = (word & !0x000e) | ((u16::from(func) & 0x7) << 1);
    word = (word & 0x0010) | ((u16::from(update_enable) & 1) << 4);
    unsafe { ptr::write_volatile(pe_reg(0), word) };
}

/// # Safety
/// GX must be initialized. The caller must own the GX state and serialize access to its command stream.
pub unsafe fn set_ind_tex_order(indtexstage: u8, texcoord: u8, texmap: u8) {
    let mut word = GX_TEV_RAS_ORDER[2].load(Ordering::Relaxed);
    let shift = match indtexstage {
        GX_INDTEXSTAGE0 => 0u32,
        GX_INDTEXSTAGE1 => 6u32,
        GX_INDTEXSTAGE2 => 12u32,
        GX_INDTEXSTAGE3 => 18u32,
        _ => return,
    };
    word = update_bits(word, 0x7 << shift, u32::from(texmap & 7) << shift);
    word = update_bits(word, 0x38 << shift, u32::from(texcoord & 7) << (shift + 3));
    GX_TEV_RAS_ORDER[2].store(word, Ordering::Relaxed);
    write_bp_word(word);
    GX_TEV_IND_MASK_DIRTY.store(true, Ordering::Release);
}

fn light_id_index(light_id: u16) -> u32 {
    match light_id {
        GX_LIGHT1 => 1,
        GX_LIGHT2 => 2,
        GX_LIGHT3 => 3,
        GX_LIGHT4 => 4,
        GX_LIGHT5 => 5,
        GX_LIGHT6 => 6,
        GX_LIGHT7 => 7,
        _ => 0,
    }
}

/// Loads a complete light object into the selected XF light register block.
///
/// # Safety
/// The caller must ensure `lit_obj` points to a valid GX light object and GX is initialized.
pub unsafe fn load_light_obj(lit_obj: *const GxLightObj, light_id: u16) {
    let lit = unsafe { light_obj_ref(lit_obj) };
    let reg = 0x0600 | ((light_id_index(light_id) as u16) << 4);
    let values = [
        0,
        0,
        0,
        lit.col,
        lit.a0.to_bits(),
        lit.a1.to_bits(),
        lit.a2.to_bits(),
        lit.k0.to_bits(),
        lit.k1.to_bits(),
        lit.k2.to_bits(),
        lit.px.to_bits(),
        lit.py.to_bits(),
        lit.pz.to_bits(),
        lit.nx.to_bits(),
        lit.ny.to_bits(),
        lit.nz.to_bits(),
    ];
    load_xf_regs(reg, &values);
}

/// Loads an indexed light object command, matching `GX_LoadLightObjIdx`.
///
/// # Safety
/// GX must be initialized and `light_obj_index` must identify a valid indexed light object.
pub unsafe fn load_light_obj_idx(light_obj_index: u32, light_id: u16) {
    let index = light_id_index(light_id);
    let mut reg = 0xf600 | ((index & 0xff) << 4);
    reg = (reg & 0x0000_ffff) | ((light_obj_index & 0xffff) << 16);
    pipe_u8(0x38);
    pipe_u32(reg);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn init_light_pos(lit_obj: *mut GxLightObj, x: f32, y: f32, z: f32) {
    let lit = unsafe { light_obj_mut(lit_obj) };
    lit.px = x;
    lit.py = y;
    lit.pz = z;
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn init_light_color(lit_obj: *mut GxLightObj, col: GxColor) {
    let lit = unsafe { light_obj_mut(lit_obj) };
    lit.col =
        ((col.r as u32) << 24) | ((col.g as u32) << 16) | ((col.b as u32) << 8) | (col.a as u32);
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn init_light_dir(lit_obj: *mut GxLightObj, x: f32, y: f32, z: f32) {
    // BUG FIX: GX_InitLightDir stores the negated vector.
    let lit = unsafe { light_obj_mut(lit_obj) };
    lit.nx = -x;
    lit.ny = -y;
    lit.nz = -z;
}

/// Initializes distance attenuation coefficients exactly as `GX_InitLightDistAttn`.
///
/// # Safety
/// The caller must ensure `lit_obj` points to a valid GX light object.
pub unsafe fn init_light_dist_attn(
    lit_obj: *mut GxLightObj,
    ref_dist: f32,
    ref_brite: f32,
    mut dist_fn: u8,
) {
    if ref_dist < 0.0 || ref_brite < 0.0 || ref_brite >= 1.0 {
        dist_fn = GX_DA_OFF;
    }

    let (k0, k1, k2) = match dist_fn {
        GX_DA_GENTLE => (1.0, (1.0 - ref_brite) / (ref_brite * ref_dist), 0.0),
        GX_DA_MEDIUM => {
            let base = (1.0 - ref_brite) / ref_brite;
            (
                1.0,
                0.5 * base / ref_dist,
                0.5 * base / (ref_dist * ref_dist),
            )
        }
        GX_DA_STEEP => (
            1.0,
            0.0,
            (1.0 - ref_brite) / (ref_brite * ref_dist * ref_dist),
        ),
        _ => (1.0, 0.0, 0.0),
    };
    unsafe { init_light_attn_k(lit_obj, k0, k1, k2) };
}

/// # Safety
/// The caller must ensure `lit_obj` points to a valid GX light object.
pub unsafe fn init_light_attn(
    lit_obj: *mut GxLightObj,
    a0: f32,
    a1: f32,
    a2: f32,
    k0: f32,
    k1: f32,
    k2: f32,
) {
    let lit = unsafe { light_obj_mut(lit_obj) };
    lit.a0 = a0;
    lit.a1 = a1;
    lit.a2 = a2;
    lit.k0 = k0;
    lit.k1 = k1;
    lit.k2 = k2;
}

/// # Safety
/// The caller must ensure `lit_obj` points to a valid GX light object.
pub unsafe fn init_light_attn_a(lit_obj: *mut GxLightObj, a0: f32, a1: f32, a2: f32) {
    let lit = unsafe { light_obj_mut(lit_obj) };
    lit.a0 = a0;
    lit.a1 = a1;
    lit.a2 = a2;
}

/// # Safety
/// The caller must ensure `lit_obj` points to a valid GX light object.
pub unsafe fn init_light_attn_k(lit_obj: *mut GxLightObj, k0: f32, k1: f32, k2: f32) {
    let lit = unsafe { light_obj_mut(lit_obj) };
    lit.k0 = k0;
    lit.k1 = k1;
    lit.k2 = k2;
}

/// # Safety
/// The caller must ensure `lit_obj` points to a valid GX light object.
pub unsafe fn init_light_spot(lit_obj: *mut GxLightObj, cut_off: f32, mut spot_fn: u8) {
    if !(0.0..=90.0).contains(&cut_off) {
        spot_fn = GX_SP_OFF;
    }

    let radians = cut_off * core::f32::consts::PI / 180.0;
    let cr = libm::cosf(radians);
    let one_minus = 1.0 - cr;
    let (a0, a1, a2) = match spot_fn {
        GX_SP_FLAT => (-1000.0 * cr, 1000.0, 0.0),
        GX_SP_COS => (-cr / one_minus, 1.0 / one_minus, 0.0),
        GX_SP_COS2 => (0.0, -cr / one_minus, 1.0 / one_minus),
        GX_SP_SHARP => {
            let d = one_minus * one_minus;
            (cr * (cr - 2.0), 2.0 / d, -1.0 / d)
        }
        GX_SP_RING1 => {
            let d = one_minus * one_minus;
            (-4.0 * cr / d, 4.0 * (1.0 + cr) / d, -4.0 / d)
        }
        GX_SP_RING2 => {
            let d = one_minus * one_minus;
            (1.0 - 2.0 * cr * cr / d, 4.0 * cr / d, -2.0 / d)
        }
        _ => (1.0, 0.0, 0.0),
    };
    unsafe { init_light_attn_a(lit_obj, a0, a1, a2) };
}

/// # Safety
/// The caller must ensure `lit_obj` points to a valid GX light object.
pub unsafe fn init_specular_dir_ha(
    lit_obj: *mut GxLightObj,
    nx: f32,
    ny: f32,
    nz: f32,
    hx: f32,
    hy: f32,
    hz: f32,
) {
    let large_number = if ogc_system::get_hollywood_revision() != 0 {
        -1.0e18_f32
    } else {
        -1_048_576.0_f32
    };
    let lit = unsafe { light_obj_mut(lit_obj) };
    lit.px = nx * large_number;
    lit.py = ny * large_number;
    lit.pz = nz * large_number;
    lit.nx = hx;
    lit.ny = hy;
    lit.nz = hz;
}

/// # Safety
/// The caller must ensure every raw pointer is valid for the required reads and writes.
/// Any in-place aliasing must only be used where the routine explicitly handles it.
pub unsafe fn init_specular_dir(lit_obj: *mut GxLightObj, x: f32, y: f32, z: f32) {
    // BUG FIX: libogc builds the light position and half-angle vector here.
    let large_number = if ogc_system::get_hollywood_revision() != 0 {
        -1.0e18_f32
    } else {
        -1_048_576.0_f32
    };
    let (position, half_angle) = specular_dir_values(x, y, z, large_number);
    let lit = unsafe { light_obj_mut(lit_obj) };
    lit.px = position[0];
    lit.py = position[1];
    lit.pz = position[2];
    lit.nx = half_angle[0];
    lit.ny = half_angle[1];
    lit.nz = half_angle[2];
}

fn specular_dir_values(nx: f32, ny: f32, nz: f32, large_number: f32) -> ([f32; 3], [f32; 3]) {
    let mut half_angle = [-nx, -ny, -nz + 1.0];
    let magnitude = half_angle[0] * half_angle[0]
        + half_angle[1] * half_angle[1]
        + half_angle[2] * half_angle[2];
    if magnitude != 0.0 {
        let inverse_magnitude = 1.0 / libm::sqrtf(magnitude);
        for component in &mut half_angle {
            *component *= inverse_magnitude;
        }
    }
    (
        [nx * large_number, ny * large_number, nz * large_number],
        half_angle,
    )
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    #[test]
    fn texture_state_dirty_words_are_selected_for_flush_before_primitive() {
        let snapshot = GxTexObjSnapshot {
            width: 128,
            height: 48,
            tex_filt: 5,
            ..GxTexObjSnapshot::default()
        };
        let [scale_s, scale_t] = textured_rect_scale_words(snapshot);
        assert_eq!(
            pending_texture_dirty_words(true, scale_s, scale_t),
            Some([0x3001_007f, 0x3101_002f])
        );
        assert_eq!(pending_texture_dirty_words(false, scale_s, scale_t), None);
    }

    #[test]
    fn xf_vertex_spec_matches_the_c_solid_and_textured_vertex_descriptors() {
        assert_eq!(vertex_spec(), 0x11);
    }

    #[test]
    fn solid_channel_control_uses_unlit_vertex_colors() {
        assert_eq!(solid_channel_control(), 1);
    }

    #[test]
    fn owned_tpl_memory_uses_the_memory_parser_and_closes_cleanly() {
        let mut contents = alloc::vec![0; TPL_HDR_DESCR_FIELD];
        contents[..4].copy_from_slice(&TPL_MAGIC.to_ne_bytes());
        contents[TPL_HDR_SIZE_FIELD..TPL_HDR_SIZE_FIELD + 4]
            .copy_from_slice(&(TPL_HDR_DESCR_FIELD as u32).to_ne_bytes());
        let mut tdf = TplFile {
            type_: 0,
            ntextures: 0,
            texdesc: ptr::null_mut(),
            tpl_file: ptr::null_mut(),
        };

        assert_eq!(unsafe { tpl_open_owned_memory(&mut tdf, contents) }, 1);
        assert_eq!(tdf.type_, TPL_FILE_TYPE_OWNED);
        assert_eq!(tdf.ntextures, 0);

        unsafe { tpl_close_tpl_file(&mut tdf) };
        assert_eq!(tdf.ntextures, 0);
        assert!(tdf.texdesc.is_null());
        assert!(tdf.tpl_file.is_null());
    }

    #[test]
    fn owned_tpl_payload_copy_has_cache_alignment() {
        let mut contents = alloc::vec![0; 9];
        contents[1..].copy_from_slice(&[1, 2, 3, 4, 5, 6, 7, 8]);
        let owner = Box::into_raw(Box::new(TplOwnedFile {
            _contents: Some(contents),
            _descriptors: None,
            _images: None,
            _palettes: None,
            file: None,
            file_len: 9,
            buffers: Vec::new(),
        }));
        let mut tdf = TplFile {
            type_: TPL_FILE_TYPE_OWNED,
            ntextures: 0,
            texdesc: ptr::null_mut(),
            tpl_file: owner.cast(),
        };

        let unaligned = unsafe { (*owner)._contents.as_mut().unwrap().as_mut_ptr().add(1) };
        let aligned = unsafe { tpl_align_owned_data(&mut tdf, unaligned, 8) }.unwrap();
        assert_eq!(aligned as usize % TPL_DATA_ALIGNMENT, 0);
        assert_eq!(
            unsafe { core::slice::from_raw_parts(aligned, 8) },
            &[1, 2, 3, 4, 5, 6, 7, 8]
        );
        assert_eq!(unsafe { (*owner).buffers.len() }, 1);
        assert_eq!(
            unsafe { tpl_align_owned_data(&mut tdf, aligned, 8) },
            Some(aligned)
        );
        assert_eq!(unsafe { (*owner).buffers.len() }, 1);

        unsafe { tpl_close_tpl_file(&mut tdf) };
    }

    #[test]
    fn fifo_register_packing_matches_mem2_ring_setup() {
        let regs = fifo_register_state(0x9000_0000, GX_FIFO_MINSIZE).unwrap();
        assert_eq!(regs.physical_base, 0x1000_0000);
        assert_eq!(regs.physical_end, 0x1000_fffc);
        assert_eq!(regs.high_watermark, 0x0000_c000);
        assert_eq!(regs.low_watermark, 0x0000_8000);
        assert_eq!(regs.physical_read_pointer, regs.physical_base);
        assert_eq!(regs.physical_write_pointer, regs.physical_base);
        assert_eq!(regs.pi_write_pointer, regs.physical_base);
    }

    #[test]
    fn fifo_register_packing_rejects_out_of_bounds_layouts() {
        assert_eq!(fifo_register_state(0, GX_FIFO_MINSIZE), None);
        assert_eq!(fifo_register_state(0x9000_0001, GX_FIFO_MINSIZE), None);
        assert_eq!(fifo_register_state(0x9000_0000, GX_FIFO_MINSIZE - 32), None);
        assert_eq!(fifo_register_state(0x9000_0000, GX_FIFO_MAXSIZE + 32), None);
        assert_eq!(fifo_register_state(0xffff_fff0, GX_FIFO_MINSIZE), None);
    }

    #[test]
    fn solid_2d_state_registers_match_libogc_packing() {
        assert_eq!(solid_vcd_low(), 0x0000_2200);
        assert_eq!(solid_vat_words(), [0x4121_6009, 0x8000_0000, 0]);
        assert_eq!(solid_channel_control(), 0x0000_0001);
        assert_eq!(solid_generation_mode(), 0x0000_0011);
        assert_eq!(GX_BP_TEV_ORDER, 0x28);
        assert_eq!(tev_order_word(), 0x40);
        assert_eq!(solid_blend_mode(), 0x0000_04bd);
        assert_eq!(pass_color_tev_words(), [0xc008_fffa, 0xc108_ffd0]);
    }

    #[test]
    fn solid_rectangle_emits_hbc_y_down_vertex_order() {
        assert_eq!(
            rectangle_vertices(10.0, 20.0, 30.0, 40.0, 0.5),
            Some([
                [10.0, 60.0, 0.5],
                [40.0, 60.0, 0.5],
                [40.0, 20.0, 0.5],
                [10.0, 20.0, 0.5],
            ])
        );
        assert_eq!(rectangle_vertices(0.0, 0.0, 0.0, 2.0, 0.0), None);
        assert_eq!(rectangle_vertices(f32::NAN, 0.0, 1.0, 1.0, 0.0), None);
        assert_eq!(rectangle_vertices(f32::MAX, 0.0, f32::MAX, 1.0, 0.0), None);
    }

    #[test]
    fn textured_rectangle_keeps_top_texture_rows_at_the_top() {
        assert_eq!(
            textured_rectangle_vertices(10.0, 20.0, 30.0, 40.0, 0.5),
            Some([
                ([10.0, 60.0, 0.5], [0.0, 1.0]),
                ([40.0, 60.0, 0.5], [1.0, 1.0]),
                ([40.0, 20.0, 0.5], [1.0, 0.0]),
                ([10.0, 20.0, 0.5], [0.0, 0.0]),
            ])
        );
        assert_eq!(textured_rectangle_vertices(0.0, 0.0, 0.0, 2.0, 0.0), None);
    }

    #[test]
    fn viewport_words_match_gx_register_scaling() {
        let words = viewport_words(0.0, 0.0, 640.0, 480.0, 0.0, 1.0);
        assert_eq!(f32::from_bits(words[0]), 320.0);
        assert_eq!(f32::from_bits(words[1]), -240.0);
        assert_eq!(f32::from_bits(words[2]), 16_777_215.0);
        assert_eq!(f32::from_bits(words[3]), 662.0);
        assert_eq!(f32::from_bits(words[4]), 582.0);
        assert_eq!(f32::from_bits(words[5]), 16_777_215.0);
    }

    #[test]
    fn scissor_words_match_libogc_unsigned_coordinates() {
        assert_eq!(
            scissor_words(0, 0, 640, 480),
            [
                0x0200_0000 | (342 << 12) | 342,
                0x0210_0000 | (981 << 12) | 821
            ]
        );
        assert_eq!(
            scissor_words(0, 0, 0, 10),
            [
                (GX_BP_SCISSOR_TL as u32) << 24 | (342 << 12) | 342,
                (GX_BP_SCISSOR_BR as u32) << 24 | (341 << 12) | 351,
            ]
        );
        assert_eq!(
            scissor_words(u32::MAX, u32::MAX, 1, 1),
            [
                0x0200_0000 | (341 << 12) | 341,
                0x0210_0000 | (341 << 12) | 341,
            ]
        );
        assert_eq!(
            scissor_words(2000, 2000, 100, 100),
            [
                0x0200_0000 | (294 << 12) | 294,
                0x0210_0000 | (393 << 12) | 393,
            ]
        );
    }

    #[test]
    fn scissor_offset_matches_libogc_zero_offset() {
        assert_eq!(scissor_offset_word(0, 0), 0x5902_acab);
        assert_eq!(scissor_offset_word(4, -2), 0x5902_a8ad);
    }

    #[test]
    fn frame_copy_registers_encode_dimensions_and_scale() {
        assert_eq!(
            copy_source_words(640, 480),
            Some([0x4900_0000, 0x4a07_7e7f])
        );
        assert_eq!(copy_dest_word(640), 0x4d00_0028);
        assert_eq!(copy_y_scale(480, 480), Some(256));
        assert_eq!(copy_y_scale(240, 480), Some(127));
        assert_eq!(copy_y_scale(480, 360), Some(341));
        assert_eq!(copy_y_scale(480, 240), None);
        assert_eq!(copy_y_scale(0, 240), None);
    }

    #[test]
    fn frame_configuration_rejects_invalid_xfbs_and_dimensions() {
        assert!(FrameConfig::new(core::ptr::null_mut(), 640, 480, 480).is_none());
        assert!(FrameConfig::new(0x8000_0001usize as *mut c_void, 640, 480, 480).is_none());
        assert!(FrameConfig::new(0x8000_0020usize as *mut c_void, 0, 480, 480).is_none());
        assert!(FrameConfig::new(0x8000_0020usize as *mut c_void, 640, 480, 480).is_some());
        assert!(FrameConfig::new(0x8000_0020usize as *mut c_void, 640, 480, 240).is_none());
        assert!(FrameConfig::new(0x8000_0020usize as *mut c_void, 640, 240, 480).is_none());
    }

    #[test]
    fn frame_copy_clears_and_enables_copy_engine() {
        assert_eq!(copy_control_word(false, 256), 0x5200_4003);
        assert_eq!(copy_control_word(true, 256), 0x5200_4803);
        assert_eq!(copy_control_word(false, 128), 0x5200_4403);
        assert_eq!(copy_control_word(false, 341), 0x5200_4403);
        assert_eq!(
            copy_address_word(0x8000_1000usize as *mut c_void),
            0x4b00_0080
        );
    }

    #[test]
    fn copy_filter_defaults_match_libogc() {
        assert_eq!(
            copy_filter_words(GxCopyFilter {
                antialias: false,
                sample_pattern: [[0; 2]; 12],
                vertical_enabled: false,
                vertical_coefficients: [0; 7],
            }),
            [
                0x0166_6666,
                0x0266_6666,
                0x0366_6666,
                0x0466_6666,
                0x5359_5000,
                0x5400_0015,
            ]
        );
    }

    #[test]
    fn copy_filter_matches_hbc_video_mode_vertical_filter() {
        assert_eq!(
            copy_filter_words(GxCopyFilter {
                antialias: false,
                sample_pattern: [[6; 2]; 12],
                vertical_enabled: true,
                vertical_coefficients: [8, 8, 10, 12, 10, 8, 8],
            }),
            [
                0x0166_6666,
                0x0266_6666,
                0x0366_6666,
                0x0466_6666,
                0x5330_a208,
                0x5400_820a,
            ]
        );
    }

    #[test]
    fn wii_tmem_defaults_match_libogc_mode_two() {
        let words = tmem_config_words();
        assert_eq!(words.len(), 16);
        assert_eq!(words[0], 0x8c0d_8000);
        assert_eq!(words[8], 0xac0d_a000);
        assert_eq!(words[15], 0xb30d_dc00);
    }

    #[test]
    fn xf_command_header_encodes_count_and_start_register() {
        assert_eq!(((6_u32 - 1) << 16) | GX_XF_VIEWPORT as u32, 0x0005_101a);
        assert_eq!(((7_u32 - 1) << 16) | GX_XF_PROJECTION as u32, 0x0006_1020);
    }

    unsafe extern "C" fn tex_region_cb(_obj: *mut GxTexObj, _mapid: u8) -> *mut GxTexRegion {
        core::ptr::null_mut()
    }

    unsafe extern "C" fn tlut_region_cb(_name: u32) -> *mut GxTlutRegion {
        core::ptr::null_mut()
    }

    #[test]
    fn texture_object_packers_work() {
        let mut tex = GxTexObj::default();
        unsafe {
            init_tex_obj_from_init(
                &mut tex,
                core::ptr::null_mut(),
                GxTexObjInit {
                    wd: 32,
                    ht: 16,
                    fmt: GX_TF_RGBA8,
                    wrap_s: 1,
                    wrap_t: 2,
                    mipmap: 1,
                },
            );
        }
        unsafe {
            assert_eq!(get_tex_obj_fmt(&tex), GX_TF_RGBA8 as u32);
            assert_eq!(get_tex_obj_wrap_s(&tex), 1);
            assert_eq!(get_tex_obj_wrap_t(&tex), 2);
            assert_eq!(get_tex_obj_mip_map(&tex), 1);
        }
    }

    #[test]
    fn textured_2d_state_encodes_texcoords_and_modulate_tev_stage() {
        assert_eq!(textured_vcd_high(), 1);
        assert_eq!(solid_vat_words(), [0x4121_6009, 0x8000_0000, 0]);
        assert_eq!(textured_texgen_words(), [0x280, 0x3d]);
        assert_eq!(solid_generation_mode(), 0x11);
        assert_eq!(modulate_tev_words(), [0xc008_f8af, 0xc108_f2f0]);
    }

    #[test]
    fn rgba8_texture_load_words_match_the_wii_default_texture_region() {
        let snapshot = GxTexObjSnapshot {
            image_ptr: 0x8000_0020usize as *mut c_void,
            width: 128,
            height: 48,
            format: GX_TF_RGBA8,
            tex_filt: 0x0080,
            tex_lod: 0,
            tex_size: 0x0060_bc7f,
            tex_maddr: 0x0012_3400,
            tex_flag: 0x02,
            ..GxTexObjSnapshot::default()
        };
        let words = tex_obj_load_words(snapshot, 0, None).unwrap();
        assert_eq!(
            words,
            [
                0x8000_0080,
                0x8400_0000,
                0x8860_bc7f,
                0x8c0d_8000,
                0x900d_c000,
                0x9412_3400,
            ]
        );
        assert!(
            words
                .iter()
                .all(|word| !(0x30..=0x3f).contains(&((word >> 24) as u8)))
        );
        assert!(tex_obj_load_words(snapshot, 8, None).is_none());
        assert!(
            tex_obj_load_words(
                GxTexObjSnapshot {
                    format: GX_TF_RGB565,
                    ..snapshot
                },
                0,
                None,
            )
            .is_none()
        );

        assert_eq!(
            tex_obj_load_words(snapshot, 0, Some([0x0012_3456, 0x0065_4321])).unwrap()[3..5],
            [0x8c12_3456, 0x9065_4321]
        );
    }

    #[test]
    fn texture_object_init_masks_zero_dimensions_like_libogc() {
        for (wd, ht) in [(0, 16), (32, 0)] {
            let mut tex = GxTexObj::default();
            unsafe {
                init_tex_obj_from_init(
                    &mut tex,
                    core::ptr::null_mut(),
                    GxTexObjInit {
                        wd,
                        ht,
                        fmt: GX_TF_I4,
                        wrap_s: 0,
                        wrap_t: 0,
                        mipmap: 0,
                    },
                );
                let snapshot = tex_obj_snapshot(&tex).unwrap();
                assert_eq!(snapshot.width, if wd == 0 { 1024 } else { wd });
                assert_eq!(snapshot.height, if ht == 0 { 1024 } else { ht });

                let mut ci_tex = GxTexObj::default();
                init_tex_obj_ci_from_init(
                    &mut ci_tex,
                    core::ptr::null_mut(),
                    GxTexObjInit {
                        wd,
                        ht,
                        fmt: GX_TF_CI8,
                        wrap_s: 0,
                        wrap_t: 0,
                        mipmap: 0,
                    },
                    0x1234,
                );
                let ci_snapshot = tex_obj_snapshot(&ci_tex).unwrap();
                assert_eq!(ci_snapshot.width, if wd == 0 { 1024 } else { wd });
                assert_eq!(ci_snapshot.height, if ht == 0 { 1024 } else { ht });
                assert_eq!(ci_snapshot.tex_tlut, 0x1234);
            }
        }
    }

    #[test]
    fn vcd_vertex_spec_counts_match_libogc() {
        let lo = (GX_DIRECT as u32) << 9 | (GX_DIRECT as u32) << 13;
        let hi = (GX_DIRECT as u32) | ((GX_INDEX8 as u32) << 2);
        assert_eq!(vcd_vertex_spec_word(lo, hi, 0), 0x21);
        assert_eq!(
            vcd_vertex_spec_word(lo | ((GX_DIRECT as u32) << 11), hi, 1),
            0x25
        );
        assert_eq!(
            vcd_vertex_spec_word(lo | ((GX_DIRECT as u32) << 11), hi, 2),
            0x29
        );
    }

    #[test]
    fn display_list_fifo_snapshot_tracks_capture_progress() {
        let fifo = display_list_fifo_snapshot_from(0x8120_0000, 0x100, 0x44, false).unwrap();
        assert_eq!(fifo.buf_start, 0x8120_0000);
        assert_eq!(fifo.buf_end, 0x8120_00fc);
        assert_eq!(fifo.rd_ptr, 0x8120_0000);
        assert_eq!(fifo.wt_ptr, 0x8120_0044);
        assert_eq!(fifo.rdwt_dst, 0x44);
        assert_eq!(fifo.fifo_wrap, 0);
        assert_eq!(display_list_fifo_snapshot_from(0, 0x100, 0, false), None);
    }

    #[test]
    fn fifo_pointer_initialization_preserves_wrapped_read_address() {
        let mut fifo = GxFifoObj::default();
        unsafe {
            init_fifo_base(&mut fifo, 0x8000_0000usize as *mut c_void, 0x10000);
            init_fifo_ptrs(
                &mut fifo,
                0x8000_f000usize as *mut c_void,
                0x8000_1000usize as *mut c_void,
            );
        }
        let snapshot = fifo_snapshot(&fifo).unwrap();
        assert_eq!(snapshot.rd_ptr, 0x8000_f000);
        assert_eq!(snapshot.wt_ptr, 0x8000_1000);
        assert_eq!(snapshot.rdwt_dst, 0x2000);
        unsafe {
            init_fifo_ptrs(
                &mut fifo,
                0x8001_0000usize as *mut c_void,
                0x8000_1000usize as *mut c_void,
            );
        }
        assert_eq!(fifo_snapshot(&fifo), Some(snapshot));
    }

    #[test]
    fn display_list_size_is_cache_line_padded_and_bounded() {
        assert_eq!(display_list_padded_size(0, 32), Some(0));
        assert_eq!(display_list_padded_size(1, 32), Some(32));
        assert_eq!(display_list_padded_size(32, 32), Some(32));
        assert_eq!(display_list_padded_size(33, 64), Some(64));
        assert_eq!(display_list_padded_size(33, 32), None);
        assert_eq!(display_list_padded_size(u32::MAX, u32::MAX), None);
    }

    #[test]
    fn gx_primitive_opcode_values_match_fifo_format() {
        assert_eq!(GX_QUADS, 0x80);
        assert_eq!(GX_TRIANGLES, 0x90);
        assert_eq!(GX_TRIANGLESTRIP, 0x98);
        assert_eq!(GX_POINTS, 0xB8);
        assert_eq!(GX_VTXFMT0, 0);
        assert_eq!(GX_QUADS | (GX_VTXFMT0 & 0x07), 0x80);
    }

    #[test]
    fn texture_buffer_size_matches_libogc_shape() {
        assert_eq!(get_tex_buffer_size(8, 8, GX_TF_I4 as u32, 0, 0), 32);
        assert_eq!(get_tex_buffer_size(16, 8, GX_TF_RGBA8 as u32, 0, 0), 512);
        assert_eq!(get_tex_buffer_size(4, 4, GX_TF_RGBA8 as u32, 1, 2), 128);
        assert_eq!(
            get_tex_buffer_size(4, 4, 0x100 | GX_TF_RGBA8 as u32, 0, 0),
            32
        );
    }

    #[test]
    fn texture_image_address_masks_to_hardware_width() {
        assert_eq!(tex_image_address(0xffff_ffff), 0x00ff_ffff);
        assert_eq!(
            tex_image_address(0x0123_4560),
            (0x0123_4560 >> 5) & 0x00ff_ffff
        );
    }

    #[test]
    fn texture_object_zero_dimensions_match_c_wrapping() {
        let mut tex = GxTexObj::default();
        unsafe {
            init_tex_obj_from_init(
                &mut tex,
                core::ptr::null_mut(),
                GxTexObjInit {
                    wd: 0,
                    ht: 0,
                    fmt: GX_TF_RGB565,
                    wrap_s: 0,
                    wrap_t: 0,
                    mipmap: GX_FALSE,
                },
            );
        }
        let snapshot = unsafe { tex_obj_snapshot(&tex) }.unwrap();
        assert_eq!(snapshot.width, 1024);
        assert_eq!(snapshot.height, 1024);
        assert_eq!(
            snapshot.tex_size,
            ((GX_TF_RGB565 as u32) << 20) | 0x000f_ffff
        );
    }

    #[test]
    fn texture_field_inputs_are_masked_to_c_bit_widths() {
        let mut initialized = GxTexObj::default();
        let mut wrapped = GxTexObj::default();
        let mut lod = GxTexObj::default();
        let mut bias_clamp = GxTexObj::default();
        let mut max_aniso = GxTexObj::default();

        unsafe {
            init_tex_obj(
                &mut initialized,
                core::ptr::null_mut(),
                GxTexObjInit {
                    wd: 8,
                    ht: 8,
                    fmt: GX_TF_I4,
                    wrap_s: u8::MAX,
                    wrap_t: u8::MAX,
                    mipmap: 0,
                },
            );
            init_tex_obj_wrap_mode(&mut wrapped, u8::MAX, u8::MAX);
            init_tex_obj_lod(
                &mut lod,
                GxTexObjLod {
                    minfilt: 0,
                    magfilt: 0,
                    minlod: 0.0,
                    maxlod: 0.0,
                    lodbias: 0.0,
                    biasclamp: u8::MAX,
                    edgelod: 0,
                    maxaniso: u8::MAX,
                },
            );
            init_tex_obj_bias_clamp(&mut bias_clamp, u8::MAX);
            init_tex_obj_max_aniso(&mut max_aniso, u8::MAX);

            assert_eq!(tex_obj_snapshot(&initialized).unwrap().tex_filt, 0x9f);
            assert_eq!(tex_obj_snapshot(&wrapped).unwrap().tex_filt, 0x0f);
            assert_eq!(tex_obj_snapshot(&lod).unwrap().tex_filt, 0x380100);
            assert_eq!(tex_obj_snapshot(&bias_clamp).unwrap().tex_filt, 0x200000);
            assert_eq!(tex_obj_snapshot(&max_aniso).unwrap().tex_filt, 0x180000);
        }
    }

    #[test]
    fn region_initializers_pack_expected_words() {
        let mut tex_region = GxTexRegion::default();
        let mut preload_region = GxTexRegion::default();
        let mut tlut_region = GxTlutRegion {
            val: [0xdead_beef, 0x1234_5678, 0x8765_4321, 0xa5a5_5a5a],
        };
        unsafe {
            init_tex_cache_region(
                &mut tex_region,
                GX_TRUE,
                0x0004_0000,
                GX_TEXCACHE_128K,
                0x0008_0000,
                GX_TEXCACHE_32K,
            );
            init_tex_preload_region(
                &mut preload_region,
                0x0004_0000,
                0x8000,
                0x0008_0000,
                0x4000,
            );
            init_tlut_region(&mut tlut_region, 0x000c_0000, GX_TLUT_256 as u8);
        }

        assert_eq!(
            tex_region.val[0],
            ((0x0004_0000 >> 5) & 0x7fff) | (4 << 15) | (4 << 18)
        );
        assert_eq!(
            tex_region.val[1],
            ((0x0008_0000 >> 5) & 0x7fff) | (3 << 15) | (3 << 18)
        );
        assert_eq!(tex_region.val[2], GX_TRUE as u32);
        assert_eq!(tex_region.val[3], 1);

        assert_eq!(
            preload_region.val[0],
            ((0x0004_0000 >> 5) & 0x7fff) | 0x200000
        );
        assert_eq!(preload_region.val[1], (0x0008_0000 >> 5) & 0x7fff);
        assert_eq!(preload_region.val[2], ((0x8000 >> 5) << 16) | (0x4000 >> 5));
        assert_eq!(preload_region.val[3], 0);

        assert_eq!(
            tlut_region.val[0],
            (((0x000c_0000u32.wrapping_sub(0x80000)) >> 9) & 0x3ff)
                | ((GX_TLUT_256 as u32) << 10)
                | (0x65 << 24)
        );
        assert_eq!(tlut_region.val[1], 0x1234_5678);
        assert_eq!(tlut_region.val[2], 0x8765_4321);
        assert_eq!(tlut_region.val[3], 0xa5a5_5a5a);
    }

    #[test]
    fn indirect_texture_mask_tracks_enabled_indirect_maps() {
        let gen_mode = 3 << 16;
        let order = 2 | (5 << 6) | (2 << 12);
        assert_eq!(indirect_texture_mask_from(gen_mode, order), 0x0f00_0024);
        assert_eq!(indirect_texture_mask_from(0, order), 0x0f00_0000);
    }

    #[test]
    fn texture_region_invalidation_words_match_libogc() {
        let region = GxTexRegion {
            val: [
                0x0000_0000 | (4 << 15) | (4 << 18),
                0x0000_0400 | (3 << 15) | (3 << 18),
                GX_TRUE as u32,
                1,
            ],
        };
        let (even, odd) = tex_region_invalidation_words(&region);
        assert_eq!(even >> 24, 0x66);
        assert_eq!(odd.unwrap_or_default() >> 24, 0x66);
        assert_eq!((even >> 9) & 0xf, 10);
        assert_eq!((odd.unwrap_or_default() >> 9) & 0xf, 8);
    }

    #[test]
    fn loaded_tlut_region_preserves_conf_address() {
        let mut region = GxTlutRegion {
            val: [0x6500_0123, 0, 0, 0x0000_55aa],
        };
        let tlut = GxTlutObjRaw {
            tlut_fmt: 2 << 10,
            tlut_maddr: 0x6400_4567,
            tlut_nentries: 256,
            _pad: [0; 2],
        };
        tlut_region_update_loaded(&mut region, &tlut);
        assert_eq!(region.val[1], (2 << 10) | 0x123);
        assert_eq!(region.val[2], 0x6400_4567);
        assert_eq!(region.val[3], 0x0100_55aa);
    }

    #[test]
    fn preload_tile_shifts_follow_libogc_switch() {
        assert_eq!(preload_tile_shifts(GX_TF_I4 as u32), (3, 3));
        assert_eq!(preload_tile_shifts(GX_TF_CI8 as u32), (3, 2));
        assert_eq!(preload_tile_shifts(GX_TF_RGB565 as u32), (2, 2));
        assert_eq!(preload_tile_shifts(GX_TF_RGBA8 as u32), (0, 0));
    }

    #[test]
    fn texgen_words_match_libogc_encoding() {
        assert_eq!(
            texgen_words(GX_TG_MTX2X4, GX_TG_TEX0, 0, GX_DTTIDENTITY),
            (0x280, 0x3d)
        );
        assert_eq!(
            texgen_words(GX_TG_MTX3X4, GX_TG_POS, 0, GX_DTTIDENTITY),
            (0x06, 0x3d)
        );
        let bump = texgen_words(GX_TG_BUMP2, GX_TG_TEXCOORD1, 1, GX_DTTMTX0);
        assert_eq!(bump.0 & 0x10, 0x10);
        assert_eq!(bump.1, 0x100);
    }

    #[test]
    fn tev_op_shadow_encoding_matches_libogc() {
        let base = 0xc000_0000;
        let add = tev_op_word(
            base,
            GX_TEV_ADD,
            GX_TB_ADDHALF,
            GX_CS_SCALE_2,
            GX_TRUE,
            GX_TEVREG1,
        );
        assert_eq!(add & 0x00ff_0000, 0x0099_0000);

        let compare = tev_op_word(
            base,
            GX_TEV_COMP_RGB8_EQ,
            GX_TB_ZERO,
            GX_CS_SCALE_1,
            GX_TRUE,
            GX_TEVREG2,
        );
        assert_eq!(compare & 0x0003_0000, 0x0003_0000);
        assert_eq!(compare & 0x0030_0000, 0x0030_0000);
        assert_eq!(compare & 0x00c0_0000, 0x00c0_0000);
    }

    #[test]
    fn indirect_matrix_words_match_libogc_layout() {
        let words = ind_matrix_words(GX_ITM_0, [[1.0, 0.5, 0.0], [0.0, -0.5, 1.0]], 0);
        assert_eq!(words[0] >> 24, 0x06);
        assert_eq!(words[1] >> 24, 0x07);
        assert_eq!(words[2] >> 24, 0x08);
        assert_eq!(words[0] & 0x7ff, 0x400);
        assert_eq!((words[1] >> 11) & 0x7ff, 0x600);
        assert_eq!((words[2] >> 11) & 0x7ff, 0x400);
    }

    #[test]
    fn tev_color_id_mapping_matches_libogc_table() {
        assert_eq!(tev_color_id(GX_COLOR0), 0);
        assert_eq!(tev_color_id(GX_ALPHA0), 0);
        assert_eq!(tev_color_id(GX_COLOR1A1), 1);
        assert_eq!(tev_color_id(GX_COLORZERO), 7);
        assert_eq!(tev_color_id(GX_COLORNULL), GX_ALPHA_BUMP);
        assert_eq!(tev_color_id(0xfe), GX_ALPHA_BUMP);
    }

    #[test]
    fn light_attenuation_initializers_match_libogc() {
        let mut light = GxLightObj::default();
        unsafe {
            init_light_attn(&mut light, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
        }
        let snapshot = unsafe { light_obj_snapshot(&light) }.unwrap();
        assert_eq!(snapshot.attenuation, [1.0, 2.0, 3.0]);
        assert_eq!(snapshot.attenuation_coeff, [4.0, 5.0, 6.0]);

        unsafe { init_light_dist_attn(&mut light, 10.0, 0.5, GX_DA_GENTLE) };
        let snapshot = unsafe { light_obj_snapshot(&light) }.unwrap();
        assert_eq!(snapshot.attenuation_coeff, [1.0, 0.1, 0.0]);

        unsafe { init_light_dist_attn(&mut light, -1.0, 0.5, GX_DA_STEEP) };
        let snapshot = unsafe { light_obj_snapshot(&light) }.unwrap();
        assert_eq!(snapshot.attenuation_coeff, [1.0, 0.0, 0.0]);

        unsafe { init_light_spot(&mut light, 90.0, GX_SP_COS) };
        let snapshot = unsafe { light_obj_snapshot(&light) }.unwrap();
        assert!(snapshot.attenuation[0].abs() < 1.0e-6);
        assert!((snapshot.attenuation[1] - 1.0).abs() < 1.0e-6);
        assert_eq!(snapshot.attenuation[2], 0.0);

        unsafe { init_light_spot(&mut light, 91.0, GX_SP_RING2) };
        let snapshot = unsafe { light_obj_snapshot(&light) }.unwrap();
        assert_eq!(snapshot.attenuation, [1.0, 0.0, 0.0]);
    }

    #[test]
    fn light_id_mapping_matches_libogc_default() {
        assert_eq!(light_id_index(GX_LIGHT0), 0);
        assert_eq!(light_id_index(GX_LIGHT3), 3);
        assert_eq!(light_id_index(GX_LIGHT7), 7);
        assert_eq!(light_id_index(GX_LIGHTNULL), 0);
        assert_eq!(light_id_index(0xff), 0);
    }

    #[test]
    fn raster_shadow_updates_match_libogc_bitfields() {
        let mut z = 0x4000_0000u32;
        z = update_bits(z, 0x1, 1);
        z = update_bits(z, 0x0e, 3 << 1);
        z = update_bits(z, 0x10, 1 << 4);
        assert_eq!(z, 0x4000_0017);

        let mut blend = 0x4100_0000u32;
        blend |= 1;
        blend = update_bits(blend, 0x00e0, 5 << 5);
        blend = update_bits(blend, 0x0700, 4 << 8);
        blend = update_bits(blend, 0xf000, 6 << 12);
        assert_eq!(blend, 0x4100_64a1);

        let su = update_bits(update_bits(0x3400_0000, 0x40000, 1 << 18), 0x80000, 1 << 19);
        assert_eq!(su, 0x340c_0000);
    }

    #[test]
    fn pixel_format_mapping_matches_libogc_table() {
        const REAL_FMT: [u32; 8] = [0, 1, 2, 3, 4, 4, 4, 5];
        assert_eq!(REAL_FMT[GX_PF_RGB8_Z24 as usize], 0);
        assert_eq!(REAL_FMT[GX_PF_RGB565_Z16 as usize], 2);
        assert_eq!(REAL_FMT[GX_PF_Y8 as usize], 4);
        assert_eq!(REAL_FMT[GX_PF_U8 as usize], 4);
        assert_eq!(REAL_FMT[GX_PF_V8 as usize], 4);
        assert_eq!(REAL_FMT[GX_PF_YUV420 as usize], 5);
    }

    #[test]
    fn copy_geometry_helpers_match_libogc_encoding() {
        assert_eq!(
            copy_source_rect_words(3, 4, 640, 480),
            Some([0x4900_1003, 0x4a07_7e7f])
        );
        assert_eq!(copy_dest_word(640), 0x4d00_0028);
        assert_eq!(image_tile_count(GX_TF_RGBA8 as u32, 17, 9), (5, 3, 2));
        assert_eq!(image_tile_count(GX_TF_I4 as u32, 17, 9), (3, 2, 1));
    }

    #[test]
    fn xfb_line_helper_matches_copy_scale_rules() {
        assert_eq!(num_xfb_lines(480, 256), 480);
        assert!(get_y_scale_factor(480, 480).is_finite());
        assert_eq!(get_y_scale_factor(0, 480), 1.0);
    }

    #[test]
    fn channel_color_partial_updates_preserve_other_components() {
        let base = 0x1122_3344;
        let color = GxColor {
            r: 0xaa,
            g: 0xbb,
            b: 0xcc,
            a: 0xdd,
        };
        assert_eq!(
            update_channel_color(base, i32::from(GX_COLOR0), color),
            Some((0, 0xaabb_cc44))
        );
        assert_eq!(
            update_channel_color(base, i32::from(GX_ALPHA0), color),
            Some((0, 0x1122_33dd))
        );
        assert_eq!(
            update_channel_color(base, i32::from(GX_COLOR0A0), color),
            Some((0, 0xaabb_ccdd))
        );
        assert_eq!(update_channel_color(base, 99, color), None);
    }

    #[test]
    fn pe_poke_helpers_match_libogc_bitfields() {
        assert_eq!(update_pe_bit(0xffff, 4, 0), 0xffef);
        assert_eq!(update_pe_bit(0, 3, 1), 0x0008);
        assert_eq!(
            poke_blend_word(0, GX_BM_BLEND, GX_BL_SRCALPHA, GX_BL_INVSRCALPHA, GX_LO_XOR),
            0x64a1
        );
        assert_eq!(
            poke_blend_word(0xffff, GX_BM_SUBTRACT, GX_BL_ONE, GX_BL_ZERO, GX_LO_SET),
            0xf91d
        );
    }

    #[test]
    fn efb_poke_address_matches_c_layout() {
        assert_eq!(efb_poke_addr(0, 0, false) as usize, 0xc800_0000);
        assert_eq!(efb_poke_addr(1, 2, false) as usize, 0xc800_2004);
        assert_eq!(efb_poke_addr(1, 2, true) as usize, 0xc840_2004);
        assert_eq!(efb_poke_addr(0x401, 0x402, false) as usize, 0xc800_2004);
    }

    #[test]
    fn su_shadow_updates_match_libogc_bitfields() {
        let base_s = 0x3402_0005;
        assert_eq!(update_su_size_word(base_s, 128), 0x3402_007f);
        assert_eq!(update_su_size_word(base_s, 0), 0x3402_ffff);
        assert_eq!(update_su_flag_word(0x3400_0000, 17, 1), 0x3402_0000);
        assert_eq!(update_su_flag_word(0x3402_0000, 17, 0), 0x3400_0000);
        assert_eq!(update_su_flag_word(0x3400_0000, 16, 1), 0x3401_0000);
    }

    #[test]
    fn matrix_word_helpers_preserve_row_major_c_order() {
        let m: Mtx = [
            [1.0, 2.0, 3.0, 4.0],
            [5.0, 6.0, 7.0, 8.0],
            [9.0, 10.0, 11.0, 12.0],
        ];
        let full = matrix_4x3_words(&m);
        assert_eq!(full[0], 1.0f32.to_bits());
        assert_eq!(full[11], 12.0f32.to_bits());
        let normal = normal_from_4x3_words(&m);
        assert_eq!(
            normal,
            [
                1.0f32.to_bits(),
                2.0f32.to_bits(),
                3.0f32.to_bits(),
                5.0f32.to_bits(),
                6.0f32.to_bits(),
                7.0f32.to_bits(),
                9.0f32.to_bits(),
                10.0f32.to_bits(),
                11.0f32.to_bits(),
            ]
        );
        let two = matrix_2x4_words(&m);
        assert_eq!(two[0], 1.0f32.to_bits());
        assert_eq!(two[7], 8.0f32.to_bits());
    }

    #[test]
    fn fog_color_word_matches_bp_encoding() {
        assert_eq!(
            fog_color_word(GxColor {
                r: 0x12,
                g: 0x34,
                b: 0x56,
                a: 0x78
            }),
            0xf212_3456
        );
    }

    #[test]
    fn specular_direction_matches_libogc_vectors() {
        let (position, half_angle) = specular_dir_values(0.0, 0.0, 0.0, -1_048_576.0);
        assert_eq!(position, [0.0; 3]);
        assert_eq!(half_angle, [0.0, 0.0, 1.0]);

        let (position, half_angle) = specular_dir_values(0.0, 0.0, 1.0, -1_048_576.0);
        assert_eq!(position, [0.0, 0.0, -1_048_576.0]);
        assert_eq!(half_angle, [0.0; 3]);

        let (position, half_angle) = specular_dir_values(1.0, 0.0, 0.0, -1_048_576.0);
        assert_eq!(position, [-1_048_576.0, 0.0, 0.0]);
        assert!((half_angle[0] + core::f32::consts::FRAC_1_SQRT_2).abs() < 1.0e-6);
        assert_eq!(half_angle[1], 0.0);
        assert!((half_angle[2] - core::f32::consts::FRAC_1_SQRT_2).abs() < 1.0e-6);
    }

    #[test]
    fn region_callbacks_are_rust_owned() {
        unsafe {
            assert!(set_tex_region_callback(None).is_none());
            assert!(set_tlut_region_callback(None).is_none());

            assert!(set_tex_region_callback(Some(tex_region_cb)).is_none());
            assert!(set_tlut_region_callback(Some(tlut_region_cb)).is_none());

            assert!(set_tex_region_callback(None).is_some());
            assert!(set_tlut_region_callback(None).is_some());
        }
    }

    #[test]
    fn texture_and_light_snapshots_are_rust_owned() {
        let mut tex = GxTexObj::default();
        let mut light = GxLightObj::default();
        let mut tlut = GxTlutObj::default();
        unsafe {
            init_tex_obj_from_init(
                &mut tex,
                0x1234usize as *mut c_void,
                GxTexObjInit {
                    wd: 16,
                    ht: 32,
                    fmt: GX_TF_RGB565,
                    wrap_s: 0,
                    wrap_t: 1,
                    mipmap: 0,
                },
            );
            init_light_color(
                &mut light,
                GxColor {
                    r: 1,
                    g: 2,
                    b: 3,
                    a: 4,
                },
            );
            init_light_pos(&mut light, 5.0, 6.0, 7.0);
            init_light_dir(&mut light, 1.0, 2.0, 3.0);
            init_tlut_obj(&mut tlut, 0x5678usize as *mut c_void, GX_TF_RGB565, 32);
        }

        let tex_snapshot = unsafe { tex_obj_snapshot(&tex) }.unwrap_or_default();
        assert_eq!(tex_snapshot.width, 16);
        assert_eq!(tex_snapshot.height, 32);
        assert_eq!(tex_snapshot.format, GX_TF_RGB565);
        assert_eq!(tex_snapshot.wrap_s, 0);
        assert_eq!(tex_snapshot.wrap_t, 1);
        assert_eq!(tex_snapshot.mipmap, 0);
        assert_eq!(tex_snapshot.tex_tlut, 0);
        assert_eq!(tex_snapshot.tex_tile_cnt, 32);
        assert_eq!(tex_snapshot.tex_tile_type, 2);
        assert_eq!(tex_snapshot.tex_flag, 2);
        assert_eq!(
            tex_snapshot.tex_size,
            ((16 - 1) as u32) | (((32 - 1) as u32) << 10) | ((GX_TF_RGB565 as u32) << 20)
        );
        assert_eq!(
            tex_snapshot.tex_maddr,
            ogc_core::mem_virtual_to_physical(0x1234usize) >> 5
        );

        let light_snapshot = unsafe { light_obj_snapshot(&light) }.unwrap_or_default();
        assert_eq!(
            light_snapshot.color,
            GxColor {
                r: 1,
                g: 2,
                b: 3,
                a: 4
            }
        );
        assert_eq!(light_snapshot.position, [5.0, 6.0, 7.0]);
        assert_eq!(light_snapshot.direction, [-1.0, -2.0, -3.0]);
        assert_eq!(light_snapshot.attenuation, [0.0, 0.0, 0.0]);
        assert_eq!(light_snapshot.attenuation_coeff, [0.0, 0.0, 0.0]);

        let tlut_snapshot = unsafe { tlut_obj_snapshot(&tlut) }.unwrap_or_default();
        assert_eq!(tlut_snapshot.format, GX_TF_RGB565 as u32);
        assert_eq!(tlut_snapshot.nentries, 32);
        assert_eq!(tlut_snapshot.tlut_fmt, (GX_TF_RGB565 as u32) << 10);
        assert_eq!(
            tlut_snapshot.tlut_maddr,
            ((ogc_core::mem_virtual_to_physical(0x5678usize)) >> 5) | (0x64 << 24)
        );
    }
}

/// Converts a linear RGB565 source buffer into GX 4x4-tiled RGB565 texture data.
///
/// This mirrors `libogc/libogc/texconv.c`'s `make_texture565`.
///
/// The original C routine assumes a fixed source stride of 1024 bytes,
/// equivalent to 512 RGB565 pixels per source row. Width and height are
/// processed in 4-pixel units; partial tiles are ignored.
///
/// # Safety
/// For every complete tile, `src` must be readable at the fixed 1024-byte row
/// stride, and `dst` must be writable for the complete tiled output. The two
/// accessed regions must not overlap. Both pointers must remain valid for the
/// duration of this call.
pub unsafe fn make_texture565(src: *const c_void, dst: *mut c_void, width: i32, height: i32) {
    const SRC_STRIDE_BYTES: usize = 1024;
    const TILE_WIDTH_PIXELS: usize = 4;
    const TILE_HEIGHT_PIXELS: usize = 4;
    const RGB565_BYTES_PER_PIXEL: usize = 2;
    const TILE_ROW_BYTES: usize = TILE_WIDTH_PIXELS * RGB565_BYTES_PER_PIXEL;
    const TILE_BYTES: usize = TILE_HEIGHT_PIXELS * TILE_ROW_BYTES;

    if width <= 0 || height <= 0 {
        return;
    }

    let tiles_x = (width as usize) / TILE_WIDTH_PIXELS;
    let tiles_y = (height as usize) / TILE_HEIGHT_PIXELS;

    let src = src.cast::<u8>();
    let dst = dst.cast::<u8>();

    for tile_y in 0..tiles_y {
        let src_tile_row_base = tile_y * TILE_HEIGHT_PIXELS * SRC_STRIDE_BYTES;

        for tile_x in 0..tiles_x {
            let src_tile_base =
                src_tile_row_base + tile_x * TILE_WIDTH_PIXELS * RGB565_BYTES_PER_PIXEL;
            let dst_tile_base = (tile_y * tiles_x + tile_x) * TILE_BYTES;

            for row in 0..TILE_HEIGHT_PIXELS {
                let src_row = unsafe { src.add(src_tile_base + row * SRC_STRIDE_BYTES) };
                let dst_row = unsafe { dst.add(dst_tile_base + row * TILE_ROW_BYTES) };

                unsafe {
                    core::ptr::copy_nonoverlapping(src_row, dst_row, TILE_ROW_BYTES);
                }
            }
        }
    }
}

const TPL_FILE_TYPE_DISC: i32 = 0;
const TPL_FILE_TYPE_MEM: i32 = 1;
const TPL_FILE_TYPE_OWNED: i32 = 2;
const TPL_DATA_ALIGNMENT: usize = 32;

struct TplOwnedFile {
    _contents: Option<Vec<u8>>,
    _descriptors: Option<Box<[TplDescHeaderDisk]>>,
    _images: Option<Box<[TplImgHeaderDisk]>>,
    _palettes: Option<Box<[TplPalHeaderDisk]>>,
    file: Option<DeviceFile>,
    file_len: usize,
    buffers: Vec<TplAlignedBuffer>,
}

struct TplAlignedBuffer {
    ptr: *mut u8,
    layout: Layout,
}

impl TplAlignedBuffer {
    fn allocate(size: usize) -> Option<Self> {
        let layout = Layout::from_size_align(size.max(1), TPL_DATA_ALIGNMENT).ok()?;
        let ptr = unsafe { alloc(layout) };
        if ptr.is_null() {
            return None;
        }
        Some(Self { ptr, layout })
    }
}

impl Drop for TplAlignedBuffer {
    fn drop(&mut self) {
        unsafe { dealloc(self.ptr, self.layout) };
    }
}

unsafe fn tpl_align_owned_data(tdf: *mut TplFile, data: *mut u8, size: usize) -> Option<*mut u8> {
    let file_type = unsafe { (*tdf).type_ };
    if file_type != TPL_FILE_TYPE_OWNED && file_type != TPL_FILE_TYPE_DISC {
        return Some(data);
    }

    let owner = unsafe { (*tdf).tpl_file.cast::<TplOwnedFile>() };
    if owner.is_null() {
        return None;
    }
    if unsafe { (*owner).buffers.iter().any(|buffer| buffer.ptr == data) } {
        return Some(data);
    }
    let data_addr = data as usize;
    let source_offset = if let Some(contents) = unsafe { (*owner)._contents.as_ref() } {
        let base = contents.as_ptr() as usize;
        let content_len = contents.len();
        let content_end = base.checked_add(content_len)?;
        data_addr.checked_sub(base).filter(|offset| {
            data_addr < content_end
                && offset
                    .checked_add(size)
                    .is_some_and(|end| end <= content_len)
        })
    } else {
        data_addr
            .checked_add(size)
            .filter(|end| *end <= unsafe { (*owner).file_len })
            .map(|_| data_addr)
    };
    let aligned = {
        let offset = source_offset?;
        let aligned = TplAlignedBuffer::allocate(size)?;
        if let Some(file) = unsafe { (*owner).file.as_ref() } {
            file.seek(i32::try_from(offset).ok()?, 0).ok()?;
            let mut loaded = 0;
            while loaded < size {
                let output = unsafe {
                    core::slice::from_raw_parts_mut(aligned.ptr.add(loaded), size - loaded)
                };
                let count = file.read(output).ok()?;
                if count == 0 {
                    return None;
                }
                loaded += count;
            }
        } else {
            let contents = unsafe { (*owner)._contents.as_ref()? };
            let source =
                unsafe { core::slice::from_raw_parts(contents.as_ptr().add(offset), size) };
            unsafe { ptr::copy_nonoverlapping(source.as_ptr(), aligned.ptr, size) };
        }
        aligned
    };
    let aligned_ptr = aligned.ptr;
    unsafe {
        (*owner).buffers.try_reserve(1).ok()?;
        (*owner).buffers.push(aligned);
    }
    Some(aligned_ptr)
}
const TPL_MAGIC: u32 = 0x0020_af30;
const TPL_HDR_NTEXTURE_FIELD: usize = 4;
const TPL_HDR_SIZE_FIELD: usize = 8;
const TPL_HDR_DESCR_FIELD: usize = 12;
const TPL_EINVAL: i32 = -1;

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
struct TplDescHeaderDisk {
    imghead: u32,
    palhead: u32,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
struct TplImgHeaderDisk {
    height: u16,
    width: u16,
    fmt: u32,
    data: u32,
    wraps: u32,
    wrapt: u32,
    minfilter: u32,
    magfilter: u32,
    lodbias: f32,
    edgelod: u8,
    minlod: u8,
    maxlod: u8,
    unpacked: u8,
}

struct TplFileHeaders {
    file: DeviceFile,
    file_len: usize,
    descriptors: Vec<TplDescHeaderDisk>,
    images: Vec<TplImgHeaderDisk>,
    palettes: Vec<TplPalHeaderDisk>,
    has_palettes: Vec<bool>,
}

#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
struct TplPalHeaderDisk {
    nitems: u16,
    unpacked: u8,
    pad: u8,
    fmt: u32,
    data: u32,
}

#[repr(C)]
pub struct TplFile {
    pub type_: i32,
    pub ntextures: i32,
    pub texdesc: *mut c_void,
    pub tpl_file: *mut c_void,
}

#[inline]
unsafe fn tpl_descs(tdf: *mut TplFile) -> *mut TplDescHeaderDisk {
    unsafe { (*tdf).texdesc as *mut TplDescHeaderDisk }
}

#[inline]
unsafe fn tpl_imghead(descs: *mut TplDescHeaderDisk, id: usize) -> *mut TplImgHeaderDisk {
    unsafe { (*descs.add(id)).imghead as usize as *mut TplImgHeaderDisk }
}

#[inline]
unsafe fn tpl_palhead(descs: *mut TplDescHeaderDisk, id: usize) -> *mut TplPalHeaderDisk {
    unsafe { (*descs.add(id)).palhead as usize as *mut TplPalHeaderDisk }
}

#[inline]
fn tpl_get_texture_size(width: u32, height: u32, fmt: u32) -> Option<u32> {
    let (tiles_x, tiles_y, tile_bytes) = match fmt {
        x if x == GX_TF_I4 as u32 || x == GX_TF_CI4 as u32 || x == GX_TF_CMPR as u32 => (
            (width.checked_add(7)? >> 3),
            (height.checked_add(7)? >> 3),
            32,
        ),
        x if x == GX_TF_I8 as u32 || x == GX_TF_IA4 as u32 || x == GX_TF_CI8 as u32 => (
            (width.checked_add(7)? >> 3),
            (height.checked_add(3)? >> 2),
            32,
        ),
        x if x == GX_TF_IA8 as u32
            || x == GX_TF_CI14 as u32
            || x == GX_TF_RGB565 as u32
            || x == GX_TF_RGB5A3 as u32 =>
        {
            (
                (width.checked_add(3)? >> 2),
                (height.checked_add(3)? >> 2),
                32,
            )
        }
        x if x == GX_TF_RGBA8 as u32 => (
            (width.checked_add(3)? >> 2),
            (height.checked_add(3)? >> 2),
            64,
        ),
        _ => return None,
    };
    tiles_x.checked_mul(tiles_y)?.checked_mul(tile_bytes)
}

fn tpl_range_valid(offset: u32, size: usize, len: usize) -> bool {
    (offset as usize)
        .checked_add(size)
        .is_some_and(|end| end <= len)
}

#[inline]
unsafe fn tpl_copy_memory_texture_headers(
    base: *mut u8,
    ntextures: usize,
) -> *mut TplDescHeaderDisk {
    unsafe {
        let descs = base.add(TPL_HDR_DESCR_FIELD) as *mut TplDescHeaderDisk;

        for i in 0..ntextures {
            let desc = descs.add(i);
            let img_offset = ptr::read_unaligned(ptr::addr_of!((*desc).imghead));
            let img = base.add(img_offset as usize) as *mut TplImgHeaderDisk;
            let img_data_offset = ptr::read_unaligned(ptr::addr_of!((*img).data));
            ptr::write_unaligned(
                ptr::addr_of_mut!((*img).data),
                base.add(img_data_offset as usize) as usize as u32,
            );
            ptr::write_unaligned(ptr::addr_of_mut!((*desc).imghead), img as usize as u32);

            let pal_offset = ptr::read_unaligned(ptr::addr_of!((*desc).palhead));
            if pal_offset != 0 {
                let pal = base.add(pal_offset as usize) as *mut TplPalHeaderDisk;
                let pal_data_offset = ptr::read_unaligned(ptr::addr_of!((*pal).data));
                ptr::write_unaligned(
                    ptr::addr_of_mut!((*pal).data),
                    base.add(pal_data_offset as usize) as usize as u32,
                );
                ptr::write_unaligned(ptr::addr_of_mut!((*desc).palhead), pal as usize as u32);
            }
        }

        descs
    }
}

unsafe fn tpl_validate_memory(base: *const u8, len: usize, ntextures: usize) -> bool {
    unsafe {
        let Some(desc_bytes) = ntextures.checked_mul(core::mem::size_of::<TplDescHeaderDisk>())
        else {
            return false;
        };
        if !tpl_range_valid(TPL_HDR_DESCR_FIELD as u32, desc_bytes, len) {
            return false;
        }
        let descs = base.add(TPL_HDR_DESCR_FIELD).cast::<TplDescHeaderDisk>();
        for index in 0..ntextures {
            let desc = descs.add(index);
            let img_offset = ptr::read_unaligned(ptr::addr_of!((*desc).imghead));
            if img_offset == 0
                || !tpl_range_valid(img_offset, core::mem::size_of::<TplImgHeaderDisk>(), len)
            {
                return false;
            }
            let img = base.add(img_offset as usize).cast::<TplImgHeaderDisk>();
            let width = ptr::read_unaligned(ptr::addr_of!((*img).width)) as u32;
            let height = ptr::read_unaligned(ptr::addr_of!((*img).height)) as u32;
            let format = ptr::read_unaligned(ptr::addr_of!((*img).fmt));
            let Some(texture_size) = tpl_get_texture_size(width, height, format) else {
                return false;
            };
            let data_offset = ptr::read_unaligned(ptr::addr_of!((*img).data));
            if data_offset == 0 || !tpl_range_valid(data_offset, texture_size as usize, len) {
                return false;
            }

            let pal_offset = ptr::read_unaligned(ptr::addr_of!((*desc).palhead));
            if pal_offset != 0 {
                if !tpl_range_valid(pal_offset, core::mem::size_of::<TplPalHeaderDisk>(), len) {
                    return false;
                }
                let pal = base.add(pal_offset as usize).cast::<TplPalHeaderDisk>();
                let items = ptr::read_unaligned(ptr::addr_of!((*pal).nitems)) as usize;
                let data_offset = ptr::read_unaligned(ptr::addr_of!((*pal).data));
                let Some(palette_size) = items.checked_mul(core::mem::size_of::<u16>()) else {
                    return false;
                };
                if data_offset == 0 || !tpl_range_valid(data_offset, palette_size, len) {
                    return false;
                }
            }
        }
        true
    }
}

/// Opens a TPL image stored in writable memory.
///
/// # Safety
/// Null pointers are accepted and produce an error. Otherwise, `tdf` must be valid and properly
/// aligned for writes of a `TplFile`, and `memory` must point to a non-overlapping writable
/// allocation of at least `len` bytes containing a complete TPL image. This routine rewrites
/// offsets in that image into pointers, so the allocation must remain at the same address and
/// remain live for every later operation using `tdf`.
pub unsafe fn tpl_open_tpl_from_memory(tdf: *mut TplFile, memory: *mut c_void, len: u32) -> i32 {
    unsafe {
        if tdf.is_null() || memory.is_null() || len < TPL_HDR_DESCR_FIELD as u32 {
            return TPL_EINVAL;
        }

        let base = memory.cast::<u8>();
        if ptr::read_unaligned(base.cast::<u32>()) != TPL_MAGIC
            || ptr::read_unaligned(base.add(TPL_HDR_SIZE_FIELD).cast::<u32>())
                != TPL_HDR_DESCR_FIELD as u32
        {
            return TPL_EINVAL;
        }
        let ntextures = ptr::read_unaligned(base.add(TPL_HDR_NTEXTURE_FIELD).cast::<u32>());
        if ntextures > i32::MAX as u32
            || !tpl_validate_memory(base.cast_const(), len as usize, ntextures as usize)
        {
            return TPL_EINVAL;
        }
        (*tdf).type_ = TPL_FILE_TYPE_MEM;
        (*tdf).tpl_file = ptr::null_mut();
        (*tdf).ntextures = ntextures as i32;
        (*tdf).texdesc = tpl_copy_memory_texture_headers(base, (*tdf).ntextures as usize).cast();

        1
    }
}

/// Opens a TPL image from a file.
///
/// # Safety
/// Null pointers are accepted and produce an error. Otherwise, `tdf` must be valid and properly
/// aligned for writes of a `TplFile`, and `file_name` must point to a valid NUL-terminated string
/// for the duration of the call.
pub unsafe fn tpl_open_tpl_from_file(tdf: *mut TplFile, file_name: *const c_char) -> i32 {
    if tdf.is_null() {
        return TPL_EINVAL;
    }
    if file_name.is_null() {
        return 0;
    }

    let path = unsafe { CStr::from_ptr(file_name) };
    let Some(headers) = read_tpl_file(path) else {
        return TPL_EINVAL;
    };
    // BUG FIX: Keep the file open, retain parsed headers, and load aligned payloads on demand.
    unsafe { tpl_open_file_backing(tdf, headers) }
}

#[cfg(test)]
unsafe fn tpl_open_owned_memory(tdf: *mut TplFile, contents: Vec<u8>) -> i32 {
    unsafe { tpl_open_owned(tdf, contents, None) }
}

#[cfg(test)]
unsafe fn tpl_open_owned(tdf: *mut TplFile, contents: Vec<u8>, file: Option<DeviceFile>) -> i32 {
    let Ok(len) = u32::try_from(contents.len()) else {
        return TPL_EINVAL;
    };
    let mut contents = contents;
    let memory = contents.as_mut_ptr();
    let owner = Box::into_raw(Box::new(TplOwnedFile {
        _contents: Some(contents),
        _descriptors: None,
        _images: None,
        _palettes: None,
        file,
        file_len: len as usize,
        buffers: Vec::new(),
    }));
    let result = unsafe { tpl_open_tpl_from_memory(tdf, memory.cast(), len) };
    if result != 1 {
        unsafe { drop(Box::from_raw(owner)) };
        return result;
    }

    unsafe {
        (*tdf).type_ = TPL_FILE_TYPE_OWNED;
        (*tdf).tpl_file = owner.cast();
    }
    1
}

unsafe fn tpl_open_file_backing(tdf: *mut TplFile, headers: TplFileHeaders) -> i32 {
    let TplFileHeaders {
        file,
        file_len,
        descriptors,
        images,
        palettes,
        has_palettes,
    } = headers;
    let mut images = images.into_boxed_slice();
    let mut palettes = palettes.into_boxed_slice();
    let mut descriptors = descriptors.into_boxed_slice();
    for index in 0..descriptors.len() {
        descriptors[index].imghead = unsafe { images.as_mut_ptr().add(index) as usize as u32 };
        descriptors[index].palhead = if has_palettes[index] {
            unsafe { palettes.as_mut_ptr().add(index) as usize as u32 }
        } else {
            0
        };
    }
    let texture_count = match i32::try_from(descriptors.len()) {
        Ok(count) => count,
        Err(_) => return TPL_EINVAL,
    };
    let descriptor_ptr = descriptors.as_mut_ptr();
    let owner = Box::into_raw(Box::new(TplOwnedFile {
        _contents: None,
        _descriptors: Some(descriptors),
        _images: Some(images),
        _palettes: Some(palettes),
        file: Some(file),
        file_len,
        buffers: Vec::new(),
    }));
    unsafe {
        (*tdf).type_ = TPL_FILE_TYPE_DISC;
        (*tdf).ntextures = texture_count;
        (*tdf).texdesc = descriptor_ptr.cast();
        (*tdf).tpl_file = owner.cast();
    }
    1
}

fn read_tpl_file(path: &CStr) -> Option<TplFileHeaders> {
    let file = DeviceFile::open_read(path)?;
    let file_len = usize::try_from(file.seek(0, 2).ok()?).ok()?;
    if file_len < TPL_HDR_DESCR_FIELD || file_len > i32::MAX as usize {
        return None;
    }
    let mut file_header = [0; TPL_HDR_DESCR_FIELD];
    tpl_read_file_at(&file, 0, &mut file_header)?;
    if u32::from_be_bytes(file_header[..4].try_into().ok()?) != TPL_MAGIC
        || u32::from_be_bytes(
            file_header[TPL_HDR_SIZE_FIELD..TPL_HDR_DESCR_FIELD]
                .try_into()
                .ok()?,
        ) != TPL_HDR_DESCR_FIELD as u32
    {
        return None;
    }
    let ntextures = u32::from_be_bytes(
        file_header[TPL_HDR_NTEXTURE_FIELD..TPL_HDR_SIZE_FIELD]
            .try_into()
            .ok()?,
    ) as usize;
    if ntextures > i32::MAX as usize {
        return None;
    }
    let descriptor_bytes = ntextures.checked_mul(core::mem::size_of::<TplDescHeaderDisk>())?;
    if !tpl_range_valid(TPL_HDR_DESCR_FIELD as u32, descriptor_bytes, file_len) {
        return None;
    }
    let mut descriptor_bytes_buf = Vec::new();
    descriptor_bytes_buf
        .try_reserve_exact(descriptor_bytes)
        .ok()?;
    descriptor_bytes_buf.resize(descriptor_bytes, 0);
    tpl_read_file_at(&file, TPL_HDR_DESCR_FIELD, &mut descriptor_bytes_buf)?;
    let mut descriptors = Vec::new();
    let mut images = Vec::new();
    let mut palettes = Vec::new();
    let mut has_palettes = Vec::new();
    descriptors.try_reserve_exact(ntextures).ok()?;
    images.try_reserve_exact(ntextures).ok()?;
    palettes.try_reserve_exact(ntextures).ok()?;
    has_palettes.try_reserve_exact(ntextures).ok()?;
    for index in 0..ntextures {
        let desc_start = index * core::mem::size_of::<TplDescHeaderDisk>();
        let img_offset = u32::from_be_bytes(
            descriptor_bytes_buf[desc_start..desc_start + 4]
                .try_into()
                .ok()?,
        );
        let pal_offset = u32::from_be_bytes(
            descriptor_bytes_buf[desc_start + 4..desc_start + 8]
                .try_into()
                .ok()?,
        );
        if img_offset == 0
            || !tpl_range_valid(
                img_offset,
                core::mem::size_of::<TplImgHeaderDisk>(),
                file_len,
            )
        {
            return None;
        }
        let image: TplImgHeaderDisk = tpl_read_file_struct(&file, img_offset as usize)?;
        let texture_size =
            tpl_get_texture_size(image.width as u32, image.height as u32, image.fmt)?;
        if image.data == 0 || !tpl_range_valid(image.data, texture_size as usize, file_len) {
            return None;
        }
        let has_palette = pal_offset != 0;
        let palette = if has_palette {
            if !tpl_range_valid(
                pal_offset,
                core::mem::size_of::<TplPalHeaderDisk>(),
                file_len,
            ) {
                return None;
            }
            let palette: TplPalHeaderDisk = tpl_read_file_struct(&file, pal_offset as usize)?;
            let palette_size = (palette.nitems as usize).checked_mul(core::mem::size_of::<u16>())?;
            if palette.data == 0 || !tpl_range_valid(palette.data, palette_size, file_len) {
                return None;
            }
            palette
        } else {
            TplPalHeaderDisk::default()
        };
        descriptors.push(TplDescHeaderDisk::default());
        images.push(image);
        palettes.push(palette);
        has_palettes.push(has_palette);
    }
    Some(TplFileHeaders {
        file,
        file_len,
        descriptors,
        images,
        palettes,
        has_palettes,
    })
}

fn tpl_read_file_at(file: &DeviceFile, offset: usize, output: &mut [u8]) -> Option<()> {
    file.seek(i32::try_from(offset).ok()?, 0).ok()?;
    let mut read = 0;
    while read < output.len() {
        let count = file.read(&mut output[read..]).ok()?;
        if count == 0 {
            return None;
        }
        read += count;
    }
    Some(())
}

fn tpl_read_file_struct<T: Copy + Default>(file: &DeviceFile, offset: usize) -> Option<T> {
    let mut value = T::default();
    let output = unsafe {
        core::slice::from_raw_parts_mut(
            (&mut value as *mut T).cast::<u8>(),
            core::mem::size_of::<T>(),
        )
    };
    tpl_read_file_at(file, offset, output)?;
    Some(value)
}

/// Reads metadata for one texture in an open TPL image.
///
/// # Safety
/// A null `tdf` is accepted and produces an error. Otherwise, it must point to a `TplFile`
/// initialized by a successful TPL open operation, and its backing storage must still be live and
/// unmoved. Each non-null output pointer must be valid and properly aligned for one write of its
/// pointee type.
pub unsafe fn tpl_get_texture_info(
    tdf: *mut TplFile,
    id: i32,
    fmt: *mut u32,
    width: *mut u16,
    height: *mut u16,
) -> i32 {
    unsafe {
        if tdf.is_null() || id < 0 || id >= (*tdf).ntextures || (*tdf).texdesc.is_null() {
            return TPL_EINVAL;
        }

        let descs = tpl_descs(tdf);
        let img = tpl_imghead(descs, id as usize);
        if img.is_null() {
            return TPL_EINVAL;
        }

        if !fmt.is_null() {
            *fmt = (*img).fmt;
        }
        if !width.is_null() {
            *width = (*img).width;
        }
        if !height.is_null() {
            *height = (*img).height;
        }

        0
    }
}

/// Initializes a GX texture object from one texture in an open TPL image.
///
/// # Safety
/// Null input pointers are accepted and produce an error. Otherwise, `tdf` must point to a
/// `TplFile` initialized by a successful TPL open operation, and its backing storage must still be
/// live and unmoved. `tex_obj` must be valid and properly aligned for writes of a `GxTexObj` and
/// must not overlap the descriptor or its backing storage. The texture data must remain live and
/// unmoved while the resulting object is in use by GX.
pub unsafe fn tpl_get_texture(tdf: *mut TplFile, id: i32, tex_obj: *mut GxTexObj) -> i32 {
    unsafe {
        if tdf.is_null()
            || tex_obj.is_null()
            || id < 0
            || id >= (*tdf).ntextures
            || (*tdf).texdesc.is_null()
        {
            return TPL_EINVAL;
        }

        let descs = tpl_descs(tdf);
        let img = tpl_imghead(descs, id as usize);
        if img.is_null() {
            return TPL_EINVAL;
        }

        let Some(size) =
            tpl_get_texture_size((*img).width as u32, (*img).height as u32, (*img).fmt)
        else {
            return TPL_EINVAL;
        };

        let mut b_mipmap = 0;
        let mut biasclamp = GX_DISABLE;
        if (*img).maxlod > 0 {
            b_mipmap = 1;
        }
        if (*img).lodbias > 0.0 {
            biasclamp = GX_ENABLE;
        }

        if (*tdf).type_ == TPL_FILE_TYPE_OWNED || (*tdf).type_ == TPL_FILE_TYPE_DISC {
            let Some(data) =
                tpl_align_owned_data(tdf, (*img).data as usize as *mut u8, size as usize)
            else {
                return TPL_EINVAL;
            };
            (*img).data = data as usize as u32;
        }

        dc_flush_range((*img).data as *const c_void, size);
        init_tex_obj_from_init(
            tex_obj,
            (*img).data as *mut c_void,
            GxTexObjInit {
                wd: (*img).width,
                ht: (*img).height,
                fmt: (*img).fmt as u8,
                wrap_s: (*img).wraps as u8,
                wrap_t: (*img).wrapt as u8,
                mipmap: b_mipmap as u8,
            },
        );

        if b_mipmap != 0 {
            init_tex_obj_lod_from_params(
                tex_obj,
                GxTexObjLod {
                    minfilt: (*img).minfilter as u8,
                    magfilt: (*img).magfilter as u8,
                    minlod: (*img).minlod as f32,
                    maxlod: (*img).maxlod as f32,
                    lodbias: (*img).lodbias,
                    biasclamp,
                    edgelod: biasclamp,
                    maxaniso: (*img).edgelod,
                },
            );
        }

        0
    }
}

/// Initializes GX texture and palette objects from one color-indexed texture in an open TPL image.
///
/// # Safety
/// Null input pointers are accepted and produce an error. Otherwise, `tdf` must point to a
/// `TplFile` initialized by a successful TPL open operation, and its backing storage must still be
/// live and unmoved. `tex_obj` and `tlut_obj` must be valid and properly aligned for writes of
/// their pointee types; neither may overlap the other, the descriptor, or its backing storage. The
/// texture and palette data must remain live and unmoved while the resulting objects are in use by
/// GX.
pub unsafe fn tpl_get_texture_ci(
    tdf: *mut TplFile,
    id: i32,
    tex_obj: *mut GxTexObj,
    tlut_obj: *mut GxTlutObj,
    tluts: u8,
) -> i32 {
    unsafe {
        if tdf.is_null()
            || tex_obj.is_null()
            || tlut_obj.is_null()
            || id < 0
            || id >= (*tdf).ntextures
            || (*tdf).texdesc.is_null()
        {
            return TPL_EINVAL;
        }

        let descs = tpl_descs(tdf);
        let img = tpl_imghead(descs, id as usize);
        let pal = tpl_palhead(descs, id as usize);
        if img.is_null() || pal.is_null() {
            return TPL_EINVAL;
        }

        let Some(size) =
            tpl_get_texture_size((*img).width as u32, (*img).height as u32, (*img).fmt)
        else {
            return TPL_EINVAL;
        };
        let pal_size = (*pal).nitems as usize * core::mem::size_of::<u16>();
        if pal_size == 0 {
            return TPL_EINVAL;
        }

        let mut b_mipmap = 0;
        let mut biasclamp = GX_DISABLE;
        if (*img).maxlod > 0 {
            b_mipmap = 1;
        }
        if (*img).lodbias > 0.0 {
            biasclamp = GX_ENABLE;
        }

        if (*tdf).type_ == TPL_FILE_TYPE_OWNED || (*tdf).type_ == TPL_FILE_TYPE_DISC {
            let Some(img_data) =
                tpl_align_owned_data(tdf, (*img).data as usize as *mut u8, size as usize)
            else {
                return TPL_EINVAL;
            };
            (*img).data = img_data as usize as u32;
            let Some(pal_data) =
                tpl_align_owned_data(tdf, (*pal).data as usize as *mut u8, pal_size)
            else {
                return TPL_EINVAL;
            };
            (*pal).data = pal_data as usize as u32;
        }

        dc_flush_range((*img).data as *const c_void, size);
        dc_flush_range((*pal).data as *const c_void, pal_size as u32);
        init_tlut_obj(
            tlut_obj,
            (*pal).data as *mut c_void,
            (*pal).fmt as u8,
            (*pal).nitems,
        );
        init_tex_obj_ci_from_init(
            tex_obj,
            (*img).data as *mut c_void,
            GxTexObjInit {
                wd: (*img).width,
                ht: (*img).height,
                fmt: (*img).fmt as u8,
                wrap_s: (*img).wraps as u8,
                wrap_t: (*img).wrapt as u8,
                mipmap: b_mipmap as u8,
            },
            tluts as u32,
        );

        if b_mipmap != 0 {
            init_tex_obj_lod_from_params(
                tex_obj,
                GxTexObjLod {
                    minfilt: (*img).minfilter as u8,
                    magfilt: (*img).magfilter as u8,
                    minlod: (*img).minlod as f32,
                    maxlod: (*img).maxlod as f32,
                    lodbias: (*img).lodbias,
                    biasclamp,
                    edgelod: biasclamp,
                    maxaniso: (*img).edgelod,
                },
            );
        }

        0
    }
}

/// Invalidates an open TPL file descriptor.
///
/// # Safety
/// A null pointer is accepted as a no-op. Otherwise, `tdf` must be valid and properly aligned for
/// writes of a `TplFile`, and no other access to that descriptor may occur for the duration of the
/// call. Any GX objects created from it retain raw references to the separate backing TPL storage;
/// the caller must keep that storage live until GX no longer uses those objects.
pub unsafe fn tpl_close_tpl_file(tdf: *mut TplFile) {
    if tdf.is_null() {
        return;
    }

    unsafe {
        if ((*tdf).type_ == TPL_FILE_TYPE_OWNED || (*tdf).type_ == TPL_FILE_TYPE_DISC)
            && !(*tdf).tpl_file.is_null()
        {
            drop(Box::from_raw((*tdf).tpl_file.cast::<TplOwnedFile>()));
        }
        (*tdf).ntextures = 0;
        (*tdf).texdesc = ptr::null_mut();
        (*tdf).tpl_file = ptr::null_mut();
    }
}
