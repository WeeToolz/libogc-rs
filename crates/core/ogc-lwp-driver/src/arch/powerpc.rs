use core::arch::{asm, global_asm};

#[inline]
pub(crate) fn write_decrementer(ticks: u32) {
    unsafe {
        asm!("mtspr 22, {ticks}", ticks = in(reg) ticks, options(nostack));
    }
}

/// PowerPC execution context using the Tuxedo/libogc field ordering.
///
/// `pc` and `msr` are restored through SRR0/SRR1 by `rfi`. The context switch
/// preserves the architectural GPR, FPR, paired-single, FPSCR, CR, LR, CTR,
/// XER, and GQR state using the same public layout as Tuxedo/libogc.
#[repr(C, align(32))]
#[derive(Copy, Clone)]
pub struct Context {
    pub pc: u32,
    pub msr: u32,
    pub cr: u32,
    pub lr: u32,
    pub ctr: u32,
    pub xer: u32,
    pub gpr: [u32; 32],
    pub fpscr: u64,
    pub fpr: [u64; 32],
    pub gqr: [u32; 8],
    pub ps: [u64; 32],
}

impl Context {
    pub const fn zeroed() -> Self {
        Self {
            pc: 0,
            msr: 0,
            cr: 0,
            lr: 0,
            ctr: 0,
            xer: 0,
            gpr: [0; 32],
            fpscr: 0,
            fpr: [0; 32],
            gqr: [0; 8],
            ps: [0; 32],
        }
    }
}

const _: [(); 0x2c0] = [(); core::mem::size_of::<Context>()];

// Context offsets match gc/tuxedo/ppc/context.h:
// pc=0x000, msr=0x004, cr=0x008, lr=0x00c, ctr=0x010,
// xer=0x014, gpr=0x018, fpscr=0x098, fpr=0x0a0,
// gqr=0x1a0, ps=0x1c0, size=0x2c0.
global_asm!(
    r#"
    # Match C-sources/libogc/tuxedo/exception_asm.S, which saves and restores
    # all 32 paired-single registers with psq_st and psq_l. LLVM integrated
    # assembly rejects these Gekko mnemonics, so these are their D-form words:
    # opcode | FPR | rA | W | I | 12-bit displacement.

    .section .text.__ogc_lwp_context_switch,"ax",@progbits
    .balign 4
    .globl __ogc_lwp_context_switch
    .type __ogc_lwp_context_switch,@function
__ogc_lwp_context_switch:
    stw     0,24(3)
    mr      0,12
    mr      12,3

    stw     1,28(12)
    stw     2,32(12)
    stmw    3,36(12)
    stw     0,72(12)

    mflr    0
    stw     0,0(12)
    stw     0,12(12)
    mfmsr   0
    stw     0,4(12)
    mfcr    0
    stw     0,8(12)
    mfctr   0
    stw     0,16(12)
    mfxer   0
    stw     0,20(12)

    mfmsr   11
    ori     0,11,0x2000
    mtmsr   0
    isync
    stfd    0,160(12)
    mffs    0
    stfd    0,152(12)
    stfd    1,168(12)
    stfd    2,176(12)
    stfd    3,184(12)
    stfd    4,192(12)
    stfd    5,200(12)
    stfd    6,208(12)
    stfd    7,216(12)
    stfd    8,224(12)
    stfd    9,232(12)
    stfd    10,240(12)
    stfd    11,248(12)
    stfd    12,256(12)
    stfd    13,264(12)
    stfd    14,272(12)
    stfd    15,280(12)
    stfd    16,288(12)
    stfd    17,296(12)
    stfd    18,304(12)
    stfd    19,312(12)
    stfd    20,320(12)
    stfd    21,328(12)
    stfd    22,336(12)
    stfd    23,344(12)
    stfd    24,352(12)
    stfd    25,360(12)
    stfd    26,368(12)
    stfd    27,376(12)
    stfd    28,384(12)
    stfd    29,392(12)
    stfd    30,400(12)
    stfd    31,408(12)
    mfspr   0,912
    stw     0,416(12)
    mfspr   0,913
    stw     0,420(12)
    mfspr   0,914
    stw     0,424(12)
    mfspr   0,915
    stw     0,428(12)
    mfspr   0,916
    stw     0,432(12)
    mfspr   0,917
    stw     0,436(12)
    mfspr   0,918
    stw     0,440(12)
    mfspr   0,919
    stw     0,444(12)
    .long 0xf00c01c0 # psq_st 0,448(12),0,0
    .long 0xf02c01c8 # psq_st 1,456(12),0,0
    .long 0xf04c01d0 # psq_st 2,464(12),0,0
    .long 0xf06c01d8 # psq_st 3,472(12),0,0
    .long 0xf08c01e0 # psq_st 4,480(12),0,0
    .long 0xf0ac01e8 # psq_st 5,488(12),0,0
    .long 0xf0cc01f0 # psq_st 6,496(12),0,0
    .long 0xf0ec01f8 # psq_st 7,504(12),0,0
    .long 0xf10c0200 # psq_st 8,512(12),0,0
    .long 0xf12c0208 # psq_st 9,520(12),0,0
    .long 0xf14c0210 # psq_st 10,528(12),0,0
    .long 0xf16c0218 # psq_st 11,536(12),0,0
    .long 0xf18c0220 # psq_st 12,544(12),0,0
    .long 0xf1ac0228 # psq_st 13,552(12),0,0
    .long 0xf1cc0230 # psq_st 14,560(12),0,0
    .long 0xf1ec0238 # psq_st 15,568(12),0,0
    .long 0xf20c0240 # psq_st 16,576(12),0,0
    .long 0xf22c0248 # psq_st 17,584(12),0,0
    .long 0xf24c0250 # psq_st 18,592(12),0,0
    .long 0xf26c0258 # psq_st 19,600(12),0,0
    .long 0xf28c0260 # psq_st 20,608(12),0,0
    .long 0xf2ac0268 # psq_st 21,616(12),0,0
    .long 0xf2cc0270 # psq_st 22,624(12),0,0
    .long 0xf2ec0278 # psq_st 23,632(12),0,0
    .long 0xf30c0280 # psq_st 24,640(12),0,0
    .long 0xf32c0288 # psq_st 25,648(12),0,0
    .long 0xf34c0290 # psq_st 26,656(12),0,0
    .long 0xf36c0298 # psq_st 27,664(12),0,0
    .long 0xf38c02a0 # psq_st 28,672(12),0,0
    .long 0xf3ac02a8 # psq_st 29,680(12),0,0
    .long 0xf3cc02b0 # psq_st 30,688(12),0,0
    .long 0xf3ec02b8 # psq_st 31,696(12),0,0

    # r5 optionally points at a caller-owned PPCContext mirror. All caller
    # registers are already saved, so r6-r9 are scratch here.
    cmpwi   5,0
    beq     2f
    li      6,176
    mr      7,12
    mr      8,5
1:
    lwz     9,0(7)
    stw     9,0(8)
    addi    7,7,4
    addi    8,8,4
    addic.  6,6,-1
    bne     1b
2:
    mr      12,4

    lwz     0,4(12)
    mtsrr1  0
    lwz     0,0(12)
    mtsrr0  0

    lwz     0,416(12)
    mtspr   912,0
    lwz     0,420(12)
    mtspr   913,0
    lwz     0,424(12)
    mtspr   914,0
    lwz     0,428(12)
    mtspr   915,0
    lwz     0,432(12)
    mtspr   916,0
    lwz     0,436(12)
    mtspr   917,0
    lwz     0,440(12)
    mtspr   918,0
    lwz     0,444(12)
    mtspr   919,0
    lfd     0,152(12)
    mtfsf   255,0
    .long 0xe00c01c0 # psq_l 0,448(12),0,0
    lfd     0,160(12)
    .long 0xe02c01c8 # psq_l 1,456(12),0,0
    lfd     1,168(12)
    .long 0xe04c01d0 # psq_l 2,464(12),0,0
    lfd     2,176(12)
    .long 0xe06c01d8 # psq_l 3,472(12),0,0
    lfd     3,184(12)
    .long 0xe08c01e0 # psq_l 4,480(12),0,0
    lfd     4,192(12)
    .long 0xe0ac01e8 # psq_l 5,488(12),0,0
    lfd     5,200(12)
    .long 0xe0cc01f0 # psq_l 6,496(12),0,0
    lfd     6,208(12)
    .long 0xe0ec01f8 # psq_l 7,504(12),0,0
    lfd     7,216(12)
    .long 0xe10c0200 # psq_l 8,512(12),0,0
    lfd     8,224(12)
    .long 0xe12c0208 # psq_l 9,520(12),0,0
    lfd     9,232(12)
    .long 0xe14c0210 # psq_l 10,528(12),0,0
    lfd     10,240(12)
    .long 0xe16c0218 # psq_l 11,536(12),0,0
    lfd     11,248(12)
    .long 0xe18c0220 # psq_l 12,544(12),0,0
    lfd     12,256(12)
    .long 0xe1ac0228 # psq_l 13,552(12),0,0
    lfd     13,264(12)
    .long 0xe1cc0230 # psq_l 14,560(12),0,0
    lfd     14,272(12)
    .long 0xe1ec0238 # psq_l 15,568(12),0,0
    lfd     15,280(12)
    .long 0xe20c0240 # psq_l 16,576(12),0,0
    lfd     16,288(12)
    .long 0xe22c0248 # psq_l 17,584(12),0,0
    lfd     17,296(12)
    .long 0xe24c0250 # psq_l 18,592(12),0,0
    lfd     18,304(12)
    .long 0xe26c0258 # psq_l 19,600(12),0,0
    lfd     19,312(12)
    .long 0xe28c0260 # psq_l 20,608(12),0,0
    lfd     20,320(12)
    .long 0xe2ac0268 # psq_l 21,616(12),0,0
    lfd     21,328(12)
    .long 0xe2cc0270 # psq_l 22,624(12),0,0
    lfd     22,336(12)
    .long 0xe2ec0278 # psq_l 23,632(12),0,0
    lfd     23,344(12)
    .long 0xe30c0280 # psq_l 24,640(12),0,0
    lfd     24,352(12)
    .long 0xe32c0288 # psq_l 25,648(12),0,0
    lfd     25,360(12)
    .long 0xe34c0290 # psq_l 26,656(12),0,0
    lfd     26,368(12)
    .long 0xe36c0298 # psq_l 27,664(12),0,0
    lfd     27,376(12)
    .long 0xe38c02a0 # psq_l 28,672(12),0,0
    lfd     28,384(12)
    .long 0xe3ac02a8 # psq_l 29,680(12),0,0
    lfd     29,392(12)
    .long 0xe3cc02b0 # psq_l 30,688(12),0,0
    lfd     30,400(12)
    .long 0xe3ec02b8 # psq_l 31,696(12),0,0
    lfd     31,408(12)

    # A reservation belongs to the processor, not the saved context. Clear it
    # so lwarx/stwcx. sequences cannot accidentally span two LWP threads.
    li      0,4
    stwcx.  0,1,0

    lwz     0,8(12)
    mtcrf   255,0
    lwz     0,12(12)
    mtlr    0
    lwz     0,16(12)
    mtctr   0
    lwz     0,20(12)
    mtxer   0

    lmw     13,76(12)
    lwz     0,24(12)
    lwz     1,28(12)
    lwz     2,32(12)
    lwz     3,36(12)
    lwz     4,40(12)
    lwz     5,44(12)
    lwz     6,48(12)
    lwz     7,52(12)
    lwz     8,56(12)
    lwz     9,60(12)
    lwz     10,64(12)
    lwz     11,68(12)
    lwz     12,72(12)
    rfi
    .size __ogc_lwp_context_switch,.-__ogc_lwp_context_switch
"#
);

unsafe extern "C" {
    fn __ogc_lwp_context_switch(old: *mut Context, new: *const Context, mirror: *mut u8);
}

#[inline(always)]
pub(crate) unsafe fn context_switch(old: *mut Context, new: *const Context, mirror: *mut u8) {
    unsafe { __ogc_lwp_context_switch(old, new, mirror) };
}

#[inline(always)]
pub(crate) fn current_r2() -> u32 {
    let value: u32;
    unsafe { asm!("mr {out}, 2", out = out(reg) value, options(nostack, nomem)) };
    value
}

#[inline(always)]
pub(crate) fn current_r13() -> u32 {
    let value: u32;
    unsafe { asm!("mr {out}, 13", out = out(reg) value, options(nostack, nomem)) };
    value
}

#[inline(always)]
pub(crate) fn current_msr() -> u32 {
    let value: u32;
    unsafe { asm!("mfmsr {out}", out = out(reg) value, options(nostack, nomem)) };
    value
}

const MSR_EE: u32 = 1 << 15;
const MSR_RI: u32 = 1 << 1;

#[inline(always)]
pub(crate) fn in_exception() -> bool {
    current_msr() & MSR_RI == 0
}

#[inline(always)]
pub(crate) fn disable_interrupts() -> u32 {
    let msr = current_msr();
    let masked = msr & !MSR_EE;
    unsafe {
        asm!(
            "mtmsr {masked}",
            "isync",
            masked = in(reg) masked,
            options(nostack)
        );
    }
    msr & MSR_EE
}

#[inline(always)]
pub(crate) fn restore_interrupts(state: u32) {
    let restored = (current_msr() & !MSR_EE) | (state & MSR_EE);
    unsafe {
        asm!(
            "mtmsr {restored}",
            "isync",
            restored = in(reg) restored,
            options(nostack)
        );
    }
}

#[inline(always)]
pub(crate) fn enable_interrupts() {
    restore_interrupts(MSR_EE);
}

#[inline(always)]
pub(crate) fn read_timebase() -> u64 {
    ogc_core::powerpc::read_timebase()
}
