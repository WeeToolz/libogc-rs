use core::arch::global_asm;

pub const GQR2: u32 = 914;
pub const GQR3: u32 = 915;
pub const GQR4: u32 = 916;
pub const GQR5: u32 = 917;
pub const GQR6: u32 = 918;
pub const GQR7: u32 = 919;

pub const GQR_TYPE_F32: u32 = 0;
pub const GQR_TYPE_U8: u32 = 4;
pub const GQR_TYPE_U16: u32 = 5;
pub const GQR_TYPE_S8: u32 = 6;
pub const GQR_TYPE_S16: u32 = 7;

pub const GQR_CAST_U8: u32 = 2;
pub const GQR_CAST_U16: u32 = 3;
pub const GQR_CAST_S8: u32 = 4;
pub const GQR_CAST_S16: u32 = 5;
pub const GQR_SCALE_MASK: u32 = 0x3f << 8;

global_asm!(
    r#"
    .text
    .align 2
    .global __ogc_cast_set_gqr2
__ogc_cast_set_gqr2:
    mtspr 914,3
    blr
    .global __ogc_cast_set_gqr3
__ogc_cast_set_gqr3:
    mtspr 915,3
    blr
    .global __ogc_cast_set_gqr4
__ogc_cast_set_gqr4:
    mtspr 916,3
    blr
    .global __ogc_cast_set_gqr5
__ogc_cast_set_gqr5:
    mtspr 917,3
    blr
    .global __ogc_cast_set_gqr6
__ogc_cast_set_gqr6:
    mtspr 918,3
    blr
    .global __ogc_cast_set_gqr7
__ogc_cast_set_gqr7:
    mtspr 919,3
    blr

    .global __ogc_cast_u8_f32
__ogc_cast_u8_f32:
    # LLVM's integrated assembler does not accept Gekko paired-single mnemonics.
    # Use their D-form instruction words: opcode | FPR | rA | W | I | offset.
    .long 0xe003a000 # psq_l 0,0(3),1,2
    stfs 0,0(4)
    blr
    .global __ogc_cast_u16_f32
__ogc_cast_u16_f32:
    .long 0xe003b000 # psq_l 0,0(3),1,3
    stfs 0,0(4)
    blr
    .global __ogc_cast_s8_f32
__ogc_cast_s8_f32:
    .long 0xe003c000 # psq_l 0,0(3),1,4
    stfs 0,0(4)
    blr
    .global __ogc_cast_s16_f32
__ogc_cast_s16_f32:
    .long 0xe003d000 # psq_l 0,0(3),1,5
    stfs 0,0(4)
    blr

    .global __ogc_cast_f32_u8
__ogc_cast_f32_u8:
    lfs 0,0(3)
    .long 0xf004a000 # psq_st 0,0(4),1,2
    blr
    .global __ogc_cast_f32_u16
__ogc_cast_f32_u16:
    lfs 0,0(3)
    .long 0xf004b000 # psq_st 0,0(4),1,3
    blr
    .global __ogc_cast_f32_s8
__ogc_cast_f32_s8:
    lfs 0,0(3)
    .long 0xf004c000 # psq_st 0,0(4),1,4
    blr
    .global __ogc_cast_f32_s16
__ogc_cast_f32_s16:
    lfs 0,0(3)
    .long 0xf004d000 # psq_st 0,0(4),1,5
    blr
"#
);

unsafe extern "C" {
    fn __ogc_cast_set_gqr2(value: u32);
    fn __ogc_cast_set_gqr3(value: u32);
    fn __ogc_cast_set_gqr4(value: u32);
    fn __ogc_cast_set_gqr5(value: u32);
    fn __ogc_cast_set_gqr6(value: u32);
    fn __ogc_cast_set_gqr7(value: u32);
    fn __ogc_cast_u8_f32(input: *const u8, output: *mut f32);
    fn __ogc_cast_u16_f32(input: *const u16, output: *mut f32);
    fn __ogc_cast_s8_f32(input: *const i8, output: *mut f32);
    fn __ogc_cast_s16_f32(input: *const i16, output: *mut f32);
    fn __ogc_cast_f32_u8(input: *const f32, output: *mut u8);
    fn __ogc_cast_f32_u16(input: *const f32, output: *mut u16);
    fn __ogc_cast_f32_s8(input: *const f32, output: *mut i8);
    fn __ogc_cast_f32_s16(input: *const f32, output: *mut i16);
}

const fn gqr_value(type_: u32, scale: i32) -> u32 {
    let field = (((scale as u32) << 8) & GQR_SCALE_MASK) | type_;
    (field << 16) | field
}

pub fn init() {
    set_gqr2(GQR_TYPE_U8, 0);
    set_gqr3(GQR_TYPE_U16, 0);
    set_gqr4(GQR_TYPE_S8, 0);
    set_gqr5(GQR_TYPE_S16, 0);
}

macro_rules! gqr_setter {
    ($name:ident, $raw:ident) => {
        pub fn $name(type_: u32, scale: i32) {
            unsafe { $raw(gqr_value(type_, scale)) };
        }
    };
}

gqr_setter!(set_gqr2, __ogc_cast_set_gqr2);
gqr_setter!(set_gqr3, __ogc_cast_set_gqr3);
gqr_setter!(set_gqr4, __ogc_cast_set_gqr4);
gqr_setter!(set_gqr5, __ogc_cast_set_gqr5);
gqr_setter!(set_gqr6, __ogc_cast_set_gqr6);
gqr_setter!(set_gqr7, __ogc_cast_set_gqr7);

macro_rules! cast_pair {
    ($name:ident, $input:ty, $output:ty, $raw:ident) => {
        /// # Safety
        /// `input` and `output` must be valid for one value of their respective types.
        pub unsafe fn $name(input: *const $input, output: *mut $output) {
            unsafe { $raw(input, output) };
        }
    };
}

cast_pair!(castu8f32, u8, f32, __ogc_cast_u8_f32);
cast_pair!(castu16f32, u16, f32, __ogc_cast_u16_f32);
cast_pair!(casts8f32, i8, f32, __ogc_cast_s8_f32);
cast_pair!(casts16f32, i16, f32, __ogc_cast_s16_f32);
cast_pair!(castf32u8, f32, u8, __ogc_cast_f32_u8);
cast_pair!(castf32u16, f32, u16, __ogc_cast_f32_u16);
cast_pair!(castf32s8, f32, i8, __ogc_cast_f32_s8);
cast_pair!(castf32s16, f32, i16, __ogc_cast_f32_s16);
