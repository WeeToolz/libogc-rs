#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

pub const GX_FALSE: u8 = 0;
pub const GX_TRUE: u8 = 1;
pub const GX_DISABLE: u8 = 0;
pub const GX_ENABLE: u8 = 1;

pub const GX_CLIP_DISABLE: u8 = 1;
pub const GX_CLIP_ENABLE: u8 = 0;

pub const GX_FIFO_MINSIZE: u32 = 64 * 1024;
pub const GX_FIFO_HIWATERMARK: u32 = 16 * 1024;
pub const GX_FIFO_OBJSIZE: u32 = 128;

pub const GX_PERSPECTIVE: u8 = 0;
pub const GX_ORTHOGRAPHIC: u8 = 1;

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

// Remaining public gx.h constant surface. These aliases preserve libogc's
// spelling so C-oriented code can use the sys crate without renaming values.
pub use GX_MTX2X4 as GX_MTX2x4;
pub use GX_MTX3X4 as GX_MTX3x4;
pub const GX_POINTS: u8 = 0xb8;
pub const GX_LINES: u8 = 0xa8;
pub const GX_LINESTRIP: u8 = 0xb0;
pub const GX_TRIANGLES: u8 = 0x90;
pub const GX_TRIANGLESTRIP: u8 = 0x98;
pub const GX_TRIANGLEFAN: u8 = 0xa0;
pub const GX_QUADS: u8 = 0x80;

pub const GX_PNMTX1: u32 = 3;
pub const GX_PNMTX2: u32 = 6;
pub const GX_PNMTX3: u32 = 9;
pub const GX_PNMTX4: u32 = 12;
pub const GX_PNMTX5: u32 = 15;
pub const GX_PNMTX6: u32 = 18;
pub const GX_PNMTX7: u32 = 21;
pub const GX_PNMTX8: u32 = 24;
pub const GX_PNMTX9: u32 = 27;
pub const GX_TEXMTX0: u32 = 30;
pub const GX_TEXMTX1: u32 = 33;
pub const GX_TEXMTX2: u32 = 36;
pub const GX_TEXMTX3: u32 = 39;
pub const GX_TEXMTX4: u32 = 42;
pub const GX_TEXMTX5: u32 = 45;
pub const GX_TEXMTX6: u32 = 48;
pub const GX_TEXMTX7: u32 = 51;
pub const GX_TEXMTX8: u32 = 54;
pub const GX_TEXMTX9: u32 = 57;
pub const GX_DTTMTX1: u32 = 67;
pub const GX_DTTMTX2: u32 = 70;
pub const GX_DTTMTX3: u32 = 73;
pub const GX_DTTMTX4: u32 = 76;
pub const GX_DTTMTX5: u32 = 79;
pub const GX_DTTMTX6: u32 = 82;
pub const GX_DTTMTX7: u32 = 85;
pub const GX_DTTMTX8: u32 = 88;
pub const GX_DTTMTX9: u32 = 91;
pub const GX_DTTMTX10: u32 = 94;
pub const GX_DTTMTX11: u32 = 97;
pub const GX_DTTMTX12: u32 = 100;
pub const GX_DTTMTX13: u32 = 103;
pub const GX_DTTMTX14: u32 = 106;
pub const GX_DTTMTX15: u32 = 109;
pub const GX_DTTMTX16: u32 = 112;
pub const GX_DTTMTX17: u32 = 115;
pub const GX_DTTMTX18: u32 = 118;
pub const GX_DTTMTX19: u32 = 121;

pub const GX_TLUT_1K: u16 = 64;
pub const GX_TLUT_2K: u16 = 128;
pub const GX_TLUT_4K: u16 = 256;
pub const GX_TLUT_8K: u16 = 512;
pub const GX_TLUT_16K: u16 = 1024;
pub const GX_MAX_ZTEXOP: u8 = 3;
pub use GX_TG_MTX2X4 as GX_TG_MTX2x4;
pub use GX_TG_MTX3X4 as GX_TG_MTX3x4;
pub const GX_CLAMP: u8 = 0;
pub const GX_REPEAT: u8 = 1;
pub const GX_MIRROR: u8 = 2;
pub const GX_MAXTEXWRAPMODE: u8 = 3;
pub const GX_MAX_BLENDMODE: u8 = 4;
pub const GX_TO_SIXTEENTH: u8 = 1;
pub const GX_TO_EIGHTH: u8 = 2;
pub const GX_TO_FOURTH: u8 = 3;
pub const GX_TO_HALF: u8 = 4;
pub const GX_TO_ONE: u8 = 5;
pub const GX_MAX_TEXOFFSET: u8 = 6;
pub const GX_MAX_TEVBIAS: u8 = 3;
pub const GX_TC_LINEAR: u8 = 0;
pub const GX_TC_GE: u8 = 1;
pub const GX_TC_EQ: u8 = 2;
pub const GX_TC_LE: u8 = 3;
pub const GX_MAX_TEVCLAMPMODE: u8 = 4;
pub const GX_MAX_TEVSCALE: u8 = 4;
pub const GX_MAX_TEVREG: u8 = 4;
pub const GX_TEXMAPNULL: u32 = GX_TEXMAP_NULL;
pub const GX_AOP_AND: u8 = 0;
pub const GX_AOP_OR: u8 = 1;
pub const GX_AOP_XOR: u8 = 2;
pub const GX_AOP_XNOR: u8 = 3;
pub const GX_MAX_ALPHAOP: u8 = 4;
pub const GX_KCOLOR0: u8 = 0;
pub const GX_KCOLOR1: u8 = 1;
pub const GX_KCOLOR2: u8 = 2;
pub const GX_KCOLOR3: u8 = 3;
pub const GX_KCOLOR_MAX: u8 = 4;

pub const GX_TEV_KCSEL_1: u8 = 0x00;
pub const GX_TEV_KCSEL_7_8: u8 = 0x01;
pub const GX_TEV_KCSEL_3_4: u8 = 0x02;
pub const GX_TEV_KCSEL_5_8: u8 = 0x03;
pub const GX_TEV_KCSEL_1_2: u8 = 0x04;
pub const GX_TEV_KCSEL_3_8: u8 = 0x05;
pub const GX_TEV_KCSEL_1_4: u8 = 0x06;
pub const GX_TEV_KCSEL_1_8: u8 = 0x07;
pub const GX_TEV_KCSEL_K0: u8 = 0x0c;
pub const GX_TEV_KCSEL_K1: u8 = 0x0d;
pub const GX_TEV_KCSEL_K2: u8 = 0x0e;
pub const GX_TEV_KCSEL_K3: u8 = 0x0f;
pub const GX_TEV_KCSEL_K0_R: u8 = 0x10;
pub const GX_TEV_KCSEL_K1_R: u8 = 0x11;
pub const GX_TEV_KCSEL_K2_R: u8 = 0x12;
pub const GX_TEV_KCSEL_K3_R: u8 = 0x13;
pub const GX_TEV_KCSEL_K0_G: u8 = 0x14;
pub const GX_TEV_KCSEL_K1_G: u8 = 0x15;
pub const GX_TEV_KCSEL_K2_G: u8 = 0x16;
pub const GX_TEV_KCSEL_K3_G: u8 = 0x17;
pub const GX_TEV_KCSEL_K0_B: u8 = 0x18;
pub const GX_TEV_KCSEL_K1_B: u8 = 0x19;
pub const GX_TEV_KCSEL_K2_B: u8 = 0x1a;
pub const GX_TEV_KCSEL_K3_B: u8 = 0x1b;
pub const GX_TEV_KCSEL_K0_A: u8 = 0x1c;
pub const GX_TEV_KCSEL_K1_A: u8 = 0x1d;
pub const GX_TEV_KCSEL_K2_A: u8 = 0x1e;
pub const GX_TEV_KCSEL_K3_A: u8 = 0x1f;
pub const GX_TEV_KASEL_1: u8 = 0x00;
pub const GX_TEV_KASEL_7_8: u8 = 0x01;
pub const GX_TEV_KASEL_3_4: u8 = 0x02;
pub const GX_TEV_KASEL_5_8: u8 = 0x03;
pub const GX_TEV_KASEL_1_2: u8 = 0x04;
pub const GX_TEV_KASEL_3_8: u8 = 0x05;
pub const GX_TEV_KASEL_1_4: u8 = 0x06;
pub const GX_TEV_KASEL_1_8: u8 = 0x07;
pub const GX_TEV_KASEL_K0_R: u8 = 0x10;
pub const GX_TEV_KASEL_K1_R: u8 = 0x11;
pub const GX_TEV_KASEL_K2_R: u8 = 0x12;
pub const GX_TEV_KASEL_K3_R: u8 = 0x13;
pub const GX_TEV_KASEL_K0_G: u8 = 0x14;
pub const GX_TEV_KASEL_K1_G: u8 = 0x15;
pub const GX_TEV_KASEL_K2_G: u8 = 0x16;
pub const GX_TEV_KASEL_K3_G: u8 = 0x17;
pub const GX_TEV_KASEL_K0_B: u8 = 0x18;
pub const GX_TEV_KASEL_K1_B: u8 = 0x19;
pub const GX_TEV_KASEL_K2_B: u8 = 0x1a;
pub const GX_TEV_KASEL_K3_B: u8 = 0x1b;
pub const GX_TEV_KASEL_K0_A: u8 = 0x1c;
pub const GX_TEV_KASEL_K1_A: u8 = 0x1d;
pub const GX_TEV_KASEL_K2_A: u8 = 0x1e;
pub const GX_TEV_KASEL_K3_A: u8 = 0x1f;
pub const GX_MAX_TEVSWAP: u8 = 4;
pub const GX_MAX_INDTEXSTAGE: u8 = 4;
pub const GX_MAX_ITFORMAT: u8 = 4;
pub const GX_MAX_ITBIAS: u8 = 8;
pub const GX_MAX_ITWRAP: u8 = 7;
pub const GX_MAX_ITBALPHA: u8 = 4;
pub const GX_MAX_ITSCALE: u8 = 9;
pub const GX_VC_POS: u32 = 0;
pub const GX_VC_NRM: u32 = 1;
pub const GX_VC_CLR0: u32 = 2;
pub const GX_VC_CLR1: u32 = 3;
pub const GX_VC_TEX0: u32 = 4;
pub const GX_VC_TEX1: u32 = 5;
pub const GX_VC_TEX2: u32 = 6;
pub const GX_VC_TEX3: u32 = 7;
pub const GX_VC_TEX4: u32 = 8;
pub const GX_VC_TEX5: u32 = 9;
pub const GX_VC_TEX6: u32 = 10;
pub const GX_VC_TEX7: u32 = 11;
pub const GX_VC_ALL: u32 = 15;
pub const GX_TLUT0: u32 = 0;
pub const GX_TLUT1: u32 = 1;
pub const GX_TLUT2: u32 = 2;
pub const GX_TLUT3: u32 = 3;
pub const GX_TLUT4: u32 = 4;
pub const GX_TLUT5: u32 = 5;
pub const GX_TLUT6: u32 = 6;
pub const GX_TLUT7: u32 = 7;
pub const GX_TLUT8: u32 = 8;
pub const GX_TLUT9: u32 = 9;
pub const GX_TLUT10: u32 = 10;
pub const GX_TLUT11: u32 = 11;
pub const GX_TLUT12: u32 = 12;
pub const GX_TLUT13: u32 = 13;
pub const GX_TLUT14: u32 = 14;
pub const GX_TLUT15: u32 = 15;
pub const GX_BIGTLUT0: u32 = 16;
pub const GX_BIGTLUT1: u32 = 17;
pub const GX_BIGTLUT2: u32 = 18;
pub const GX_BIGTLUT3: u32 = 19;
pub const GX_MAX_VTXDESC: u8 = GX_VA_MAXATTR;
pub const GX_MAX_VTXATTRFMT: u8 = GX_VA_MAXATTR;
pub const GX_MAX_Z24: u32 = 0x00ff_ffff;

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

/// Opaque texture register region used by the GX texture cache API.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq)]
pub struct GxTexReg {
    pub dummy: [u8; 4],
}

/// Render mode layout declared by `gx_struct.h`.
#[repr(C)]
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub struct GxrModeObj {
    pub vi_tv_mode: u32,
    pub fb_width: u16,
    pub efb_height: u16,
    pub xfb_height: u16,
    pub vi_x_origin: u16,
    pub vi_y_origin: u16,
    pub vi_width: u16,
    pub vi_height: u16,
    pub xfb_mode: u32,
    pub field_rendering: u8,
    pub aa: u8,
    pub sample_pattern: [[u8; 2]; 12],
    pub vfilter: [u8; 7],
}

impl Default for GxFifoObj {
    fn default() -> Self {
        Self {
            pad: [0; GX_FIFO_OBJSIZE as usize],
        }
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

/// Public write-gather pipe pointer declared by `gx.h`.
pub const WG_PIPE: *mut WGPipe = 0xcc00_8000usize as *mut WGPipe;
pub use WG_PIPE as wgPipe;

pub type GxBreakPtCallback = Option<unsafe extern "C" fn()>;
pub type GxDrawDoneCallback = Option<unsafe extern "C" fn()>;
pub type GxDrawSyncCallback = Option<unsafe extern "C" fn(token: u16)>;
pub type GxTexRegionCallback =
    Option<unsafe extern "C" fn(obj: *mut GxTexObj, mapid: u8) -> *mut GxTexRegion>;
pub type GxTlutRegionCallback = Option<unsafe extern "C" fn(tlut_name: u32) -> *mut GxTlutRegion>;

// Exact public C typedef spellings from gx.h. The underlying Rust structs are
// shared so there is only one layout definition to maintain.
pub type GXColor = GxColor;
pub type GXColorS10 = GxColorS10;
pub type GXTexObj = GxTexObj;
pub type GXTlutObj = GxTlutObj;
pub type GXTexRegion = GxTexRegion;
pub type GXTlutRegion = GxTlutRegion;
pub type GXLightObj = GxLightObj;
pub type GXVtxDesc = GxVtxDesc;
pub type GXVtxAttrFmt = GxVtxAttrFmt;
pub type GXFifoObj = GxFifoObj;
pub type GXTexReg = GxTexReg;
pub type GXFogAdjTbl = GxFogAdjTbl;
pub type GXRModeObj = GxrModeObj;
pub type GXBreakPtCallback = GxBreakPtCallback;
pub type GXDrawDoneCallback = GxDrawDoneCallback;
pub type GXDrawSyncCallback = GxDrawSyncCallback;
pub type GXTexRegionCallback = GxTexRegionCallback;
pub type GXTlutRegionCallback = GxTlutRegionCallback;

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    #[test]
    fn structs_have_expected_sizes() {
        assert_eq!(core::mem::size_of::<GxColor>(), 4);
        assert_eq!(core::mem::size_of::<GxColorS10>(), 8);
        assert_eq!(core::mem::size_of::<GxTexObj>(), 32);
        assert_eq!(core::mem::size_of::<GxTlutObj>(), 12);
        assert_eq!(core::mem::size_of::<GxTexRegion>(), 16);
        assert_eq!(core::mem::size_of::<GxTlutRegion>(), 16);
        assert_eq!(core::mem::size_of::<GxLightObj>(), 64);
        assert_eq!(core::mem::size_of::<GxVtxDesc>(), 2);
        assert_eq!(core::mem::size_of::<GxVtxAttrFmt>(), 16);
        assert_eq!(core::mem::size_of::<GxFogAdjTbl>(), 20);
        assert_eq!(core::mem::size_of::<GxFifoObj>(), GX_FIFO_OBJSIZE as usize);
        assert_eq!(core::mem::size_of::<GxTexReg>(), 4);
        assert_eq!(core::mem::size_of::<GxrModeObj>(), 60);
        assert_eq!(core::mem::size_of::<Vtx>(), 20);
    }
}
