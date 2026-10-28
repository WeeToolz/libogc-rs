#![no_std]

#[cfg(all(
    feature = "wii-std",
    not(all(
        target_arch = "powerpc",
        target_vendor = "nintendo",
        target_os = "none",
        not(gamecube)
    ))
))]
compile_error!("wii-std requires the Wii PowerPC EABI target");

#[cfg(feature = "wii-std")]
static STD_STARTUP_RESULT: core::sync::atomic::AtomicI32 =
    core::sync::atomic::AtomicI32::new(i32::MIN);

#[cfg(feature = "wii-std")]
static STD_RETURN_RESULT: core::sync::atomic::AtomicI32 =
    core::sync::atomic::AtomicI32::new(i32::MIN);
#[cfg(feature = "wii-std")]
static STARTUP_ARGS_READY: core::sync::atomic::AtomicBool =
    core::sync::atomic::AtomicBool::new(false);
#[cfg(feature = "wii-std")]
static STARTUP_ARGC: core::sync::atomic::AtomicI32 = core::sync::atomic::AtomicI32::new(0);
#[cfg(feature = "wii-std")]
static STARTUP_ARGV: core::sync::atomic::AtomicPtr<*mut u8> =
    core::sync::atomic::AtomicPtr::new(core::ptr::null_mut());

/// Read the checked std startup result. None means startup has not called it.
#[cfg(feature = "wii-std")]
pub fn std_startup_result() -> Option<i32> {
    let result = STD_STARTUP_RESULT.load(core::sync::atomic::Ordering::Acquire);
    (result != i32::MIN).then_some(result)
}

/// Read the sheltered loader vector after startup publishes it. The bytes and
/// vector remain valid for the application lifetime and must not be modified.
#[cfg(feature = "wii-std")]
pub fn startup_arguments() -> Option<(i32, *mut *mut u8)> {
    if !STARTUP_ARGS_READY.load(Ordering::Acquire) {
        return None;
    }
    Some((
        STARTUP_ARGC.load(Ordering::Relaxed),
        STARTUP_ARGV.load(Ordering::Relaxed),
    ))
}

/// Read the final runtime cleanup result. None means main has not returned yet.
#[cfg(feature = "wii-std")]
pub fn std_return_result() -> Option<i32> {
    let result = STD_RETURN_RESULT.load(Ordering::Acquire);
    (result != i32::MIN).then_some(result)
}

#[cfg(target_arch = "powerpc")]
use core::sync::atomic::AtomicU8;
use core::sync::atomic::{AtomicUsize, Ordering};

const MSR_EE: u32 = 1 << 15;
const MSR_FP: u32 = 1 << 13;
const MSR_RI: u32 = 1 << 1;
const MSR_LOCK_MASK: u32 = MSR_EE | MSR_FP;

/// Set the processor bits required for normal runtime interrupt handling.
pub const fn runtime_msr_value(msr: u32) -> u32 {
    msr | MSR_EE | MSR_RI
}

/// Enable external interrupts and recoverability after system setup installs
/// the runtime vectors and scheduler.
///
/// # Safety
/// Install valid exception vectors and initialize the scheduler before this
/// call. The current runtime must be ready to receive external interrupts.
#[cfg(target_arch = "powerpc")]
pub unsafe fn enable_runtime_interrupts() {
    let msr: u32;
    unsafe {
        core::arch::asm!("mfmsr {0}", out(reg) msr, options(nostack));
        core::arch::asm!(
            "mtmsr {0}",
            "isync",
            in(reg) runtime_msr_value(msr),
            options(nostack)
        );
    }
}

/// Read the Hollywood revision from the low memory value written by the Wii.
///
/// # Safety
///
/// This function accesses platform low memory and is valid only on supported
/// Wii/GameCube PowerPC hardware.
pub unsafe fn sys_get_hollywood_revision() -> u32 {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        const HOLLYWOOD_REVISION_ADDR: usize = 0x8000_3138;
        core::arch::asm!(
            "dcbi 0, {addr}",
            addr = in(reg) HOLLYWOOD_REVISION_ADDR,
            options(nostack)
        );
        core::arch::asm!("sync", options(nostack));
        core::ptr::read_volatile(HOLLYWOOD_REVISION_ADDR as *const u32)
    }

    #[cfg(not(target_arch = "powerpc"))]
    {
        0
    }
}

/// Return the interrupt and floating point bits that a critical section must restore.
pub const fn msr_lock_state(msr: u32) -> u32 {
    msr & MSR_LOCK_MASK
}

/// Restore saved interrupt and floating point bits while preserving current unrelated MSR bits.
pub const fn msr_unlock_value(current: u32, state: u32) -> u32 {
    (current & !MSR_LOCK_MASK) | (state & MSR_LOCK_MASK)
}

/// Disable external interrupts for the lifetime of the guard, then restore
/// their previous state without changing other current MSR bits.
#[cfg(target_arch = "powerpc")]
pub struct InterruptGuard(u32);

#[cfg(target_arch = "powerpc")]
impl InterruptGuard {
    pub fn new() -> Self {
        let msr: u32;
        unsafe {
            core::arch::asm!("mfmsr {0}", out(reg) msr, options(nostack, preserves_flags));
            core::arch::asm!(
                "mtmsr {0}",
                "isync",
                in(reg) (msr & !MSR_EE),
                options(nostack, preserves_flags)
            );
        }
        Self(msr & MSR_EE)
    }
}

#[cfg(target_arch = "powerpc")]
impl Default for InterruptGuard {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(target_arch = "powerpc")]
impl Drop for InterruptGuard {
    fn drop(&mut self) {
        let current_msr: u32;
        unsafe {
            core::arch::asm!(
                "mfmsr {0}",
                out(reg) current_msr,
                options(nostack, preserves_flags)
            );
            core::arch::asm!(
                "mtmsr {0}",
                "isync",
                in(reg) ((current_msr & !MSR_EE) | self.0),
                options(nostack, preserves_flags)
            );
        }
    }
}

#[cfg(any(target_arch = "powerpc", test))]
fn snapshot_vector_bytes(source: &[u8], backup: &mut [u8]) {
    backup.copy_from_slice(source);
}

#[cfg(any(target_arch = "powerpc", test))]
fn restore_vector_bytes(backup: &[u8], destination: &mut [u8]) {
    destination.copy_from_slice(backup);
}

#[cfg(target_arch = "powerpc")]
core::arch::global_asm!(
    r#"
    .section .init
    .globl _start
_start:
    b .Lactual_start
    .ascii "_arg"
    .balign 4
    .globl __ogc_startup_argv
__ogc_startup_argv:
    .space 6*4

    # Dolphin identifies a standalone Wii DOL by this HID4 instruction. Keep
    # it after the Homebrew Channel argv block and before the branch target so
    # startup never executes it. Real Wii startup behavior is unchanged.
    .globl __ogc_wii_hid4_marker
__ogc_wii_hid4_marker:
    .long 0x7c13fba6

.Lactual_start:
    # Rust emits floating-point instructions for ordinary f32 operations.
    # DOL startup does not guarantee that the loader enabled the FPU.
    mfmsr 12
    ori 12, 12, 0x2000
    mtmsr 12
    isync
    lis 1, __stack_top@h
    ori 1, 1, __stack_top@l
    bl _start_rust
1:
    b 1b
"#
);

#[cfg(target_arch = "powerpc")]
core::arch::global_asm!(
    r#"
    .section .text.__ogc_decrementer_vector_stub,"ax",@progbits
    .balign 4
    .globl __ogc_decrementer_vector_stub_start
__ogc_decrementer_vector_stub_start:
    mtsprg0 11
    mtsprg1 12
    stw 1, __ogc_interrupt_saved_sp@l(0)
    mfsrr0 11
    stw 11, __ogc_interrupt_saved_srr0@l(0)
    mfsrr1 11
    stw 11, __ogc_interrupt_saved_srr1@l(0)
    mfmsr 11
    ori 11, 11, 0x2030
    lis 12, __ogc_decrementer_common_handler@h
    ori 12, 12, __ogc_decrementer_common_handler@l
    mtsrr0 12
    mtsrr1 11
    rfi
    .globl __ogc_decrementer_vector_stub_end
__ogc_decrementer_vector_stub_end:

    .section .text.__ogc_decrementer_common_handler,"ax",@progbits
    .globl __ogc_decrementer_common_handler
__ogc_decrementer_common_handler:
    lis 1, __ogc_decrementer_exception_stack@ha
    addi 1, 1, __ogc_decrementer_exception_stack@l
    addi 1, 1, 16384
    stwu 1, -704(1)
    stw 0, 24(1)
    lis 12, __ogc_interrupt_saved_sp@ha
    lwz 12, __ogc_interrupt_saved_sp@l(12)
    stw 12, 28(1)
    stw 2, 32(1)
    stw 3, 36(1)
    stw 4, 40(1)
    stw 5, 44(1)
    stw 6, 48(1)
    stw 7, 52(1)
    stw 8, 56(1)
    stw 9, 60(1)
    stw 10, 64(1)
    mfsprg0 11
    mfsprg1 12
    stw 11, 68(1)
    stw 12, 72(1)
    stw 13, 76(1)
    stmw 14, 80(1)
    lis 12, __ogc_interrupt_saved_srr0@ha
    lwz 12, __ogc_interrupt_saved_srr0@l(12)
    stw 12, 0(1)
    lis 12, __ogc_interrupt_saved_srr1@ha
    lwz 12, __ogc_interrupt_saved_srr1@l(12)
    stw 12, 4(1)
    mfcr 12
    stw 12, 8(1)
    mflr 12
    stw 12, 12(1)
    mfctr 12
    stw 12, 16(1)
    mfxer 12
    stw 12, 20(1)
    .rept 32
    stfd \+, 160+8*\+(1)
    .endr
    mffs 0
    stfd 0, 152(1)
    mfspr 0, 912
    stw 0, 416(1)
    mfspr 0, 913
    stw 0, 420(1)
    mfspr 0, 914
    stw 0, 424(1)
    mfspr 0, 915
    stw 0, 428(1)
    mfspr 0, 916
    stw 0, 432(1)
    mfspr 0, 917
    stw 0, 436(1)
    mfspr 0, 918
    stw 0, 440(1)
    mfspr 0, 919
    stw 0, 444(1)
    .set __ogc_ps_reg, 0
    .rept 32
    .long (0xf00101c0 + (__ogc_ps_reg * 0x200008))
    .set __ogc_ps_reg, __ogc_ps_reg + 1
    .endr
    # Keep the ABI linkage and argument area below the saved PPCContext.
    # Callees write their LR at 4(sp); the context MSR must stay intact.
    stwu 1, -64(1)
    bl __ogc_sys_decrementer_dispatch
    addi 3, 1, 64
    bl __ogc_sys_interrupt_return
    mr 12, 3
    b __ogc_restore_interrupt_context

    .section .text.__ogc_restore_interrupt_context,"ax",@progbits
    .globl __ogc_restore_interrupt_context
__ogc_restore_interrupt_context:
    mr 12, 3
    lwz 0, 4(12)
    mtsrr1 0
    lwz 0, 0(12)
    mtsrr0 0
    lwz 0, 416(12)
    mtspr 912, 0
    lwz 0, 420(12)
    mtspr 913, 0
    lwz 0, 424(12)
    mtspr 914, 0
    lwz 0, 428(12)
    mtspr 915, 0
    lwz 0, 432(12)
    mtspr 916, 0
    lwz 0, 436(12)
    mtspr 917, 0
    lwz 0, 440(12)
    mtspr 918, 0
    lwz 0, 444(12)
    mtspr 919, 0
    lfd 0, 152(12)
    mtfsf 255, 0
    .set __ogc_ps_reg, 0
    .rept 32
    .long (0xe00c01c0 + (__ogc_ps_reg * 0x200008))
    lfd \+, 160+8*\+(12)
    .set __ogc_ps_reg, __ogc_ps_reg + 1
    .endr
    lmw 14, 80(12)
    # Do not carry an lwarx reservation across an interrupt or thread switch.
    li 0, 4
    stwcx. 0, 1, 0
    lwz 0, 8(12)
    mtcr 0
    lwz 0, 12(12)
    mtlr 0
    lwz 0, 16(12)
    mtctr 0
    lwz 0, 20(12)
    mtxer 0
    .rept 12
    lwz \+, 24+4*\+(12)
    .endr
    lwz 13, 76(12)
    lwz 12, 72(12)
    rfi

    .section .bss.__ogc_decrementer_state,"aw",@nobits
    .balign 32
    .globl __ogc_decrementer_exception_stack
__ogc_decrementer_exception_stack:
    .space 0x4000
    # Match C's __ppc_excpt_buf PC/MSR and GPR1 fields. These words
    # precede the reset vector at 0x100 and are accessible with IR/DR off.
    # Never use the end of PPCContext as scratch: it overlaps DSI code.
    # SPRG2/3 remain untouched throughout Rust interrupt dispatch.
    .set __ogc_interrupt_saved_sp, 0x800000ec
    .set __ogc_interrupt_saved_srr0, 0x800000d0
    .set __ogc_interrupt_saved_srr1, 0x800000d4
    .globl __ogc_interrupt_saved_sp
    .globl __ogc_interrupt_saved_srr0
    .globl __ogc_interrupt_saved_srr1
"#
);

#[cfg(target_arch = "powerpc")]
core::arch::global_asm!(
    r#"
    .section .sdata,"aw"
    .balign 4
    .globl __system_argv
__system_argv:
    .long __ogc_startup_argv
    .balign 4
    .globl __argvArena1Lo
__argvArena1Lo:
    .long 0xdeadbeef
"#
);

#[cfg(target_arch = "powerpc")]
core::arch::global_asm!(
    r#"
    .section .text.__ogc_irq_vector_stub,"ax",@progbits
    .globl __ogc_irq_vector_stub_start
__ogc_irq_vector_stub_start:
    mtsprg0 11
    mtsprg1 12
    stw 1, __ogc_interrupt_saved_sp@l(0)
    mfsrr0 11
    stw 11, __ogc_interrupt_saved_srr0@l(0)
    mfsrr1 11
    stw 11, __ogc_interrupt_saved_srr1@l(0)
    mfmsr 11
    ori 11, 11, 0x2030
    lis 12, __ogc_irq_common_handler@h
    ori 12, 12, __ogc_irq_common_handler@l
    mtsrr0 12
    mtsrr1 11
    rfi
    .globl __ogc_irq_vector_stub_end
__ogc_irq_vector_stub_end:

    .section .text.__ogc_irq_common_handler,"ax",@progbits
    .globl __ogc_irq_common_handler
__ogc_irq_common_handler:
    lis 1, __ogc_decrementer_exception_stack@ha
    addi 1, 1, __ogc_decrementer_exception_stack@l
    addi 1, 1, 16384
    stwu 1, -704(1)
    stw 0, 24(1)
    lis 12, __ogc_interrupt_saved_sp@ha
    lwz 12, __ogc_interrupt_saved_sp@l(12)
    stw 12, 28(1)
    stw 2, 32(1)
    stw 3, 36(1)
    stw 4, 40(1)
    stw 5, 44(1)
    stw 6, 48(1)
    stw 7, 52(1)
    stw 8, 56(1)
    stw 9, 60(1)
    stw 10, 64(1)
    mfsprg0 11
    mfsprg1 12
    stw 11, 68(1)
    stw 12, 72(1)
    stw 13, 76(1)
    stmw 14, 80(1)
    lis 12, __ogc_interrupt_saved_srr0@ha
    lwz 12, __ogc_interrupt_saved_srr0@l(12)
    stw 12, 0(1)
    lis 12, __ogc_interrupt_saved_srr1@ha
    lwz 12, __ogc_interrupt_saved_srr1@l(12)
    stw 12, 4(1)
    mfcr 12
    stw 12, 8(1)
    mflr 12
    stw 12, 12(1)
    mfctr 12
    stw 12, 16(1)
    mfxer 12
    stw 12, 20(1)
    .rept 32
    stfd \+, 160+8*\+(1)
    .endr
    mffs 0
    stfd 0, 152(1)
    mfspr 0, 912
    stw 0, 416(1)
    mfspr 0, 913
    stw 0, 420(1)
    mfspr 0, 914
    stw 0, 424(1)
    mfspr 0, 915
    stw 0, 428(1)
    mfspr 0, 916
    stw 0, 432(1)
    mfspr 0, 917
    stw 0, 436(1)
    mfspr 0, 918
    stw 0, 440(1)
    mfspr 0, 919
    stw 0, 444(1)
    .set __ogc_ps_reg, 0
    .rept 32
    .long (0xf00101c0 + (__ogc_ps_reg * 0x200008))
    .set __ogc_ps_reg, __ogc_ps_reg + 1
    .endr
    # Do not expose the saved context as the caller linkage area.
    stwu 1, -64(1)
    bl __ogc_sys_irq_dispatch
    addi 3, 1, 64
    bl __ogc_sys_interrupt_return
    b __ogc_restore_interrupt_context
"#
);

#[cfg(target_arch = "powerpc")]
core::arch::global_asm!(
    r#"
    .section .text.__ogc_general_exception_vector_stub,"ax",@progbits
    .balign 4
    .globl __ogc_general_exception_vector_stub_start
__ogc_general_exception_vector_stub_start:
    mtsprg0 11
    mtsprg1 12
    stw 1, __ogc_interrupt_saved_sp@l(0)
    mfsrr0 11
    stw 11, __ogc_interrupt_saved_srr0@l(0)
    mfsrr1 12
    stw 12, __ogc_interrupt_saved_srr1@l(0)
    ori 12, 12, 0x2030
    mtsrr1 12
    li 11, 0
    lis 12, __ogc_general_exception_common_handler@h
    ori 12, 12, __ogc_general_exception_common_handler@l
    mtsrr0 12
    rfi
    .globl __ogc_general_exception_vector_stub_end
__ogc_general_exception_vector_stub_end:

    .section .text.__ogc_general_exception_common_handler,"ax",@progbits
    .globl __ogc_general_exception_common_handler
__ogc_general_exception_common_handler:
    lis 1, __ogc_decrementer_exception_stack@ha
    addi 1, 1, __ogc_decrementer_exception_stack@l
    addi 1, 1, 16384
    # PPCContext is 704 bytes. Keep one extra word for the exception ID.
    stwu 1, -708(1)
    stw 11, 704(1)
    stw 0, 24(1)
    lis 12, __ogc_interrupt_saved_sp@ha
    lwz 12, __ogc_interrupt_saved_sp@l(12)
    stw 12, 28(1)
    stw 2, 32(1)
    stw 3, 36(1)
    stw 4, 40(1)
    stw 5, 44(1)
    stw 6, 48(1)
    stw 7, 52(1)
    stw 8, 56(1)
    stw 9, 60(1)
    stw 10, 64(1)
    mfsprg0 11
    mfsprg1 12
    stw 11, 68(1)
    stw 12, 72(1)
    stw 13, 76(1)
    stmw 14, 80(1)
    lis 12, __ogc_interrupt_saved_srr0@ha
    lwz 12, __ogc_interrupt_saved_srr0@l(12)
    stw 12, 0(1)
    lis 12, __ogc_interrupt_saved_srr1@ha
    lwz 12, __ogc_interrupt_saved_srr1@l(12)
    stw 12, 4(1)
    mfcr 12
    stw 12, 8(1)
    mflr 12
    stw 12, 12(1)
    mfctr 12
    stw 12, 16(1)
    mfxer 12
    stw 12, 20(1)
    .rept 32
    stfd \+, 160+8*\+(1)
    .endr
    mffs 0
    stfd 0, 152(1)
    mfspr 0, 912
    stw 0, 416(1)
    mfspr 0, 913
    stw 0, 420(1)
    mfspr 0, 914
    stw 0, 424(1)
    mfspr 0, 915
    stw 0, 428(1)
    mfspr 0, 916
    stw 0, 432(1)
    mfspr 0, 917
    stw 0, 436(1)
    mfspr 0, 918
    stw 0, 440(1)
    mfspr 0, 919
    stw 0, 444(1)
    .set __ogc_exc_ps_reg, 0
    .rept 32
    .long (0xf00101c0 + (__ogc_exc_ps_reg * 0x200008))
    .set __ogc_exc_ps_reg, __ogc_exc_ps_reg + 1
    .endr
    # Keep ABI linkage below the saved context and extra exception-ID word.
    stwu 1, -64(1)
    lwz 3, 768(1)
    addi 4, 1, 64
    bl __ogc_sys_general_exception_dispatch
    addi 3, 1, 64
    b __ogc_restore_interrupt_context
"#
);

pub type Bool = u32;
pub type F32 = f32;
pub type F64 = f64;

pub type IrqDispatchCallback = unsafe extern "C" fn() -> u32;
pub type DecrementerDispatchCallback = unsafe extern "C" fn();
pub type GeneralExceptionCallback = unsafe extern "C" fn(u32, *mut u32);
pub type InterruptReturnCallback = unsafe extern "C" fn(*const u32) -> *const u32;

// The external interrupt vector is at 0x500. PowerPC exception vectors are
// spaced 0x100 bytes apart. The stub below is 14 instructions (56 bytes).
// Keep only the bytes that install_irq_exception_vector overwrites.
#[cfg(any(target_arch = "powerpc", test))]
const IRQ_VECTOR_STUB_LEN: usize = 14 * 4;
#[cfg(any(target_arch = "powerpc", test))]
#[cfg(target_arch = "powerpc")]
const DECREMENTER_VECTOR_STUB_LEN: usize = 14 * 4;
#[cfg(any(target_arch = "powerpc", test))]
#[cfg(target_arch = "powerpc")]
const DECREMENTER_VECTOR_SLOT_LEN: usize = 0x100;
#[cfg(target_arch = "powerpc")]
static IRQ_VECTOR_STATE: AtomicU8 = AtomicU8::new(0); // 0: unsaved, 1: installed, 2: restored
#[cfg(target_arch = "powerpc")]
static IRQ_VECTOR_BACKUP: [AtomicU8; IRQ_VECTOR_STUB_LEN] =
    [const { AtomicU8::new(0) }; IRQ_VECTOR_STUB_LEN];
#[cfg(target_arch = "powerpc")]
static DECREMENTER_VECTOR_STATE: AtomicU8 = AtomicU8::new(0); // 0: unsaved, 1: installed, 2: restored
#[cfg(target_arch = "powerpc")]
static DECREMENTER_VECTOR_BACKUP: [AtomicU8; DECREMENTER_VECTOR_STUB_LEN] =
    [const { AtomicU8::new(0) }; DECREMENTER_VECTOR_STUB_LEN];
static IRQ_DISPATCH_CALLBACK: AtomicUsize = AtomicUsize::new(0);
static DECREMENTER_DISPATCH_CALLBACK: AtomicUsize = AtomicUsize::new(0);
static GENERAL_EXCEPTION_CALLBACK: AtomicUsize = AtomicUsize::new(0);
static INTERRUPT_RETURN_CALLBACK: AtomicUsize = AtomicUsize::new(0);
static EXIT_CALLBACK: AtomicUsize = AtomicUsize::new(0);
/// Nonzero if a process exit transfer returned to the runtime.
#[unsafe(no_mangle)]
pub static __ogc_exit_callback_returned: AtomicUsize = AtomicUsize::new(0);

/// Install the process exit handler used after the application `main` returns.
///
/// The system runtime sets this during `SYS_Init`. The handler must not return.
pub fn set_exit_callback(callback: Option<extern "C" fn(i32) -> !>) {
    EXIT_CALLBACK.store(
        callback.map_or(0, |callback| callback as usize),
        Ordering::Release,
    );
}

/// Return whether memory contains the Homebrew Channel reload-stub signature.
pub const fn has_hbc_reload_stub_signature(signature_high: u32, signature_low: u32) -> bool {
    signature_high == 0x5354_5542 && signature_low == 0x4841_5858
}

/// Check for the Homebrew Channel reload stub at its documented low-memory address.
#[cfg(target_arch = "powerpc")]
pub fn hbc_reload_stub_present() -> bool {
    const STUB_SIGNATURE: *const u32 = 0x8000_1804 as *const u32;
    let high = unsafe { core::ptr::read_volatile(STUB_SIGNATURE) };
    let low = unsafe { core::ptr::read_volatile(STUB_SIGNATURE.add(1)) };
    has_hbc_reload_stub_signature(high, low)
}

#[cfg(target_arch = "powerpc")]
fn exit_after_main(status: i32) -> ! {
    let callback = EXIT_CALLBACK.load(Ordering::Acquire);
    if callback != 0 {
        // Treat the indirect call as returning at this boundary. A loader or
        // emulator can return through LR even though the registered Rust
        // callback has a diverging signature. Keep a real failure path after
        // the call instead of allowing execution to enter the next function.
        let callback: extern "C" fn(i32) = unsafe { core::mem::transmute(callback) };
        callback(status);
        __ogc_exit_callback_returned.store(1, Ordering::Release);
    }
    loop {
        core::hint::spin_loop();
    }
}

#[cfg(target_arch = "powerpc")]
const IRQ_VECTOR_ADDR: usize = 0x8000_0500;
#[cfg(target_arch = "powerpc")]
const CACHE_LINE_SIZE: usize = 32;

#[repr(transparent)]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
pub struct ErrorCode(i32);

impl ErrorCode {
    pub const fn new(code: i32) -> Self {
        Self(code)
    }

    pub const fn raw(self) -> i32 {
        self.0
    }
}

impl core::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "libogc error {}", self.0)
    }
}

pub type Result<T> = core::result::Result<T, ErrorCode>;

pub fn result_from_status(status: i32) -> Result<i32> {
    if status < 0 {
        Err(ErrorCode(status))
    } else {
        Ok(status)
    }
}

pub fn unit_from_status(status: i32) -> Result<()> {
    result_from_status(status).map(|_| ())
}

pub fn bool_from_status(status: i32) -> Result<bool> {
    result_from_status(status).map(|value| value != 0)
}

pub fn unit_from_bool_status(status: i32) -> Result<()> {
    match status {
        value if value < 0 => Err(ErrorCode(value)),
        0 => Err(ErrorCode(-1)),
        _ => Ok(()),
    }
}

pub const BYTE_ORDER_BIG_ENDIAN: u32 = 1234;
pub const BYTE_ORDER_LITTLE_ENDIAN: u32 = 3412;
pub const TARGET_BYTE_ORDER: u32 = BYTE_ORDER_BIG_ENDIAN;

#[cfg(target_arch = "powerpc")]
unsafe extern "Rust" {
    static mut __bss_start: u8;
    static mut __bss_end: u8;
    static mut __ogc_startup_argv: BootArgv;
    static mut __argvArena1Lo: usize;
    static __ogc_irq_vector_stub_start: u8;
    static __ogc_irq_vector_stub_end: u8;
}

#[cfg(any(target_arch = "powerpc", test))]
const STARTUP_ARGV_MAGIC: u32 = 0x5f61_7267;
#[cfg(any(target_arch = "powerpc", test))]
const MAX_STARTUP_ARGUMENT_BYTES: usize = 4096;
#[cfg(any(target_arch = "powerpc", test))]
const MAX_STARTUP_ARGUMENTS: usize = 64;

#[cfg(any(target_arch = "powerpc", test))]
#[repr(C)]
struct BootArgv {
    magic: u32,
    command_line: *mut u8,
    length: i32,
    argc: i32,
    argv: *mut *mut u8,
    end_argv: *mut *mut u8,
}

/// Copy HBC's argument buffer to arena memory before BSS is cleared.
#[cfg(any(target_arch = "powerpc", test))]
unsafe fn shelter_argv_into(args: *mut BootArgv, dest: *mut u8, capacity: usize) -> Option<usize> {
    let args = unsafe { &mut *args };
    args.argc = 0;
    args.argv = core::ptr::null_mut();
    args.end_argv = core::ptr::null_mut();
    if args.magic != STARTUP_ARGV_MAGIC || args.command_line.is_null() {
        return None;
    }
    let len = usize::try_from(args.length).ok()?;
    if len == 0 || len > MAX_STARTUP_ARGUMENT_BYTES || len > capacity {
        return None;
    }
    let source = unsafe { core::slice::from_raw_parts(args.command_line, len) };
    if source[len - 1] != 0 {
        return None;
    }
    let mut count = 0usize;
    let mut pos = 0usize;
    while pos < len {
        while pos < len && source[pos] == 0 {
            pos += 1;
        }
        if pos == len {
            break;
        }
        count += 1;
        if count > MAX_STARTUP_ARGUMENTS {
            return None;
        }
        while pos < len && source[pos] != 0 {
            pos += 1;
        }
    }

    let alignment = core::mem::align_of::<*mut u8>();
    let base = dest as usize;
    let argv_start = base.checked_add(len)?.checked_add(alignment - 1)? & !(alignment - 1);
    let argv_end = argv_start.checked_add((count + 1).checked_mul(alignment)?)?;
    if argv_end > base.checked_add(capacity)? {
        return None;
    }
    unsafe { core::ptr::copy(args.command_line, dest, len) };
    let argv = argv_start as *mut *mut u8;
    let copied = unsafe { core::slice::from_raw_parts(dest, len) };
    let mut index = 0usize;
    let mut pos = 0usize;
    while pos < len {
        while pos < len && copied[pos] == 0 {
            pos += 1;
        }
        if pos == len {
            break;
        }
        unsafe { argv.add(index).write(dest.add(pos)) };
        index += 1;
        while pos < len && copied[pos] != 0 {
            pos += 1;
        }
    }
    unsafe { argv.add(index).write(core::ptr::null_mut()) };
    args.command_line = dest;
    args.argc = count as i32;
    args.argv = argv;
    args.end_argv = unsafe { argv.add(index + 1) };
    Some(argv_end)
}

#[cfg(target_arch = "powerpc")]
unsafe fn shelter_startup_argv() -> (i32, *mut *mut u8) {
    let base = (core::ptr::addr_of!(__bss_end) as usize + 31) & !31;
    let capacity = 0x8180_0000usize.saturating_sub(base);
    let args = core::ptr::addr_of_mut!(__ogc_startup_argv);
    if let Some(end) = unsafe { shelter_argv_into(args, base as *mut u8, capacity) } {
        unsafe { core::ptr::addr_of_mut!(__argvArena1Lo).write(end) };
        let args = unsafe { &*args };
        (args.argc, args.argv)
    } else {
        // An absent loader header means no arguments. A recognized but invalid
        // header must fail checked std preparation before main can run.
        #[cfg(feature = "wii-std")]
        if unsafe { (*args).magic } == STARTUP_ARGV_MAGIC {
            return (-1, core::ptr::null_mut());
        }
        (0, core::ptr::null_mut())
    }
}

#[cfg(target_arch = "powerpc")]
#[unsafe(no_mangle)]
pub extern "C" fn _start_rust() -> ! {
    let (argc, argv) = unsafe { shelter_startup_argv() };
    unsafe {
        let mut cursor = core::ptr::addr_of_mut!(__bss_start);
        let end = core::ptr::addr_of_mut!(__bss_end);
        while cursor < end {
            core::ptr::write_volatile(cursor, 0);
            cursor = cursor.add(1);
        }
    }

    install_irq_exception_vector();

    #[cfg(feature = "wii-std")]
    {
        STARTUP_ARGC.store(argc, Ordering::Relaxed);
        STARTUP_ARGV.store(argv, Ordering::Relaxed);
        STARTUP_ARGS_READY.store(true, Ordering::Release);
        unsafe extern "C" {
            fn __wii_std_initialize() -> i32;
        }
        // SAFETY: The selected std backend supplies this startup symbol. It
        // initializes system services and the heap before Rust main uses TLS.
        let result = unsafe { __wii_std_initialize() };
        STD_STARTUP_RESULT.store(result, core::sync::atomic::Ordering::Release);
        if result != 0 {
            // Retain the diagnostic and device state. Never enter main or
            // reset the console after failed runtime preparation.
            loop {
                core::hint::spin_loop();
            }
        }
    }

    unsafe extern "C" {
        fn main(argc: i32, argv: *mut *mut u8) -> i32;
    }

    #[cfg(ogc_dev)]
    {
        unsafe extern "C" {
            fn __ogc_dev_begin(argc: i32, argv: *mut *mut u8) -> i32;
        }
        // SAFETY: the development runtime receives the sheltered entry vector.
        let result = unsafe { __ogc_dev_begin(argc, argv) };
        if result != 0 {
            exit_after_main(result);
        }
    }
    let status = unsafe { main(argc, argv) };
    #[cfg(ogc_dev)]
    {
        unsafe extern "C" {
            fn __ogc_dev_end(status: i32);
        }
        // SAFETY: the development runtime records the application's real result.
        unsafe { __ogc_dev_end(status) };
    }
    #[cfg(feature = "wii-std")]
    {
        unsafe extern "C" {
            fn __wii_std_runtime_return(status: i32) -> i32;
        }
        // SAFETY: The provider completes initial-thread cleanup after both a
        // compiler-generated Rust main and a C entry that used std services.
        let result = unsafe { __wii_std_runtime_return(status) };
        STD_RETURN_RESULT.store(result, Ordering::Release);
        if result != 0 {
            // Keep failed cleanup observable and do not reset the device.
            loop {
                core::hint::spin_loop();
            }
        }
    }
    exit_after_main(status)
}

#[cfg(target_arch = "powerpc")]
/// # Safety
///
/// `dest` and `src` must be valid for `n` bytes and must not overlap.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcpy(
    dest: *mut core::ffi::c_void,
    src: *const core::ffi::c_void,
    n: usize,
) -> *mut core::ffi::c_void {
    if n == 0 || core::ptr::eq(dest.cast_const(), src) {
        return dest;
    }

    unsafe {
        let mut index = 0usize;
        let dest = dest.cast::<u8>();
        let src = src.cast::<u8>();
        while index < n {
            core::ptr::write_volatile(dest.add(index), core::ptr::read_volatile(src.add(index)));
            index += 1;
        }
    }
    dest
}

#[cfg(target_arch = "powerpc")]
/// # Safety
///
/// `dest` and `src` must be valid for `n` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memmove(
    dest: *mut core::ffi::c_void,
    src: *const core::ffi::c_void,
    n: usize,
) -> *mut core::ffi::c_void {
    if n == 0 || core::ptr::eq(dest.cast_const(), src) {
        return dest;
    }

    unsafe {
        let dest = dest.cast::<u8>();
        let src = src.cast::<u8>();
        if (dest as usize) < (src as usize) {
            let mut index = 0usize;
            while index < n {
                core::ptr::write_volatile(
                    dest.add(index),
                    core::ptr::read_volatile(src.add(index)),
                );
                index += 1;
            }
        } else {
            let mut index = n;
            while index != 0 {
                index -= 1;
                core::ptr::write_volatile(
                    dest.add(index),
                    core::ptr::read_volatile(src.add(index)),
                );
            }
        }
    }
    dest
}

#[cfg(target_arch = "powerpc")]
/// # Safety
///
/// `dest` must be valid for `n` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memset(
    dest: *mut core::ffi::c_void,
    value: i32,
    n: usize,
) -> *mut core::ffi::c_void {
    if n == 0 {
        return dest;
    }

    unsafe {
        let mut index = 0usize;
        let dest = dest.cast::<u8>();
        let value = value as u8;
        while index < n {
            core::ptr::write_volatile(dest.add(index), value);
            index += 1;
        }
    }
    dest
}

#[cfg(target_arch = "powerpc")]
/// # Safety
///
/// `lhs` and `rhs` must be valid for `n` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn memcmp(
    lhs: *const core::ffi::c_void,
    rhs: *const core::ffi::c_void,
    n: usize,
) -> i32 {
    let left = unsafe { core::slice::from_raw_parts(lhs.cast::<u8>(), n) };
    let right = unsafe { core::slice::from_raw_parts(rhs.cast::<u8>(), n) };

    for (left, right) in left.iter().zip(right.iter()) {
        if left != right {
            return i32::from(*left) - i32::from(*right);
        }
    }
    0
}

#[cfg(target_arch = "powerpc")]
/// # Safety
///
/// `ptr` must point to a valid NUL-terminated C string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn strlen(ptr: *const u8) -> usize {
    if ptr.is_null() {
        return 0;
    }

    let mut len = 0usize;
    loop {
        let byte = unsafe { *ptr.add(len) };
        if byte == 0 {
            return len;
        }
        len += 1;
    }
}

pub fn set_irq_dispatch_callback(callback: Option<IrqDispatchCallback>) {
    let raw = callback.map_or(0, |cb| cb as usize);
    IRQ_DISPATCH_CALLBACK.store(raw, Ordering::Release);
}

pub fn set_decrementer_dispatch_callback(callback: Option<DecrementerDispatchCallback>) {
    DECREMENTER_DISPATCH_CALLBACK.store(callback.map_or(0, |cb| cb as usize), Ordering::Release);
}

/// Register the shared callback used by general PowerPC exception vectors.
///
/// `frame` points to a writable 0x2c0-byte `PPCContext`-layout frame. The
/// callback may edit it before exception return.
pub fn set_general_exception_callback(callback: Option<GeneralExceptionCallback>) {
    GENERAL_EXCEPTION_CALLBACK.store(callback.map_or(0, |cb| cb as usize), Ordering::Release);
}

#[unsafe(no_mangle)]
/// Dispatch a general exception to the registered handler.
///
/// # Safety
///
/// `frame` must point to a writable, live 0x2c0-byte frame with the
/// `PPCContext` layout used by the exception vector. The registered callback
/// may read and modify all fields in that frame before exception return.
pub unsafe extern "C" fn __ogc_sys_general_exception_dispatch(exid: u32, frame: *mut u32) {
    let raw = GENERAL_EXCEPTION_CALLBACK.load(Ordering::Acquire);
    if raw == 0 {
        return;
    }
    let callback: GeneralExceptionCallback = unsafe { core::mem::transmute(raw) };
    unsafe { callback(exid, frame) };
}

/// Register the scheduler hook that selects a context before exception return.
/// The pointer addresses a `PPCContext`-layout frame saved by the exception stub.
pub fn set_interrupt_return_callback(callback: Option<InterruptReturnCallback>) {
    INTERRUPT_RETURN_CALLBACK.store(callback.map_or(0, |cb| cb as usize), Ordering::Release);
}

#[unsafe(no_mangle)]
/// Select a saved exception frame before the assembly stub restores it.
///
/// # Safety
///
/// `frame` must point to a live `PPCContext`-layout frame. Any registered
/// callback must return a pointer to a live, readable frame of that layout.
pub unsafe extern "C" fn __ogc_sys_interrupt_return(frame: *const u32) -> *const u32 {
    let raw = INTERRUPT_RETURN_CALLBACK.load(Ordering::Acquire);
    if raw == 0 {
        return frame;
    }
    let callback: InterruptReturnCallback = unsafe { core::mem::transmute(raw) };
    unsafe { callback(frame) }
}

#[unsafe(no_mangle)]
pub extern "C" fn __ogc_sys_decrementer_dispatch() {
    let raw = DECREMENTER_DISPATCH_CALLBACK.load(Ordering::Acquire);
    if raw != 0 {
        let callback: DecrementerDispatchCallback = unsafe { core::mem::transmute(raw) };
        unsafe { callback() };
    }
}

/// Install the shared general-exception vector for one PowerPC exception ID.
///
/// IRQ (5), FPU-unavailable (8), and decrementer (9) have specialized runtime
/// vectors and are rejected here. This function intentionally does not install
/// a callback; users must register one with [`set_general_exception_callback`].
pub fn install_general_exception_vector(exid: u32) -> bool {
    if exid == 0 || exid > 19 || matches!(exid, 5 | 8 | 9) {
        return false;
    }

    #[cfg(target_arch = "powerpc")]
    unsafe {
        unsafe extern "Rust" {
            static __ogc_general_exception_vector_stub_start: u8;
            static __ogc_general_exception_vector_stub_end: u8;
        }
        const EXID_INSTRUCTION_INDEX: usize = 9;
        const VECTOR_SLOT_LEN: usize = 0x100;
        let start = core::ptr::addr_of!(__ogc_general_exception_vector_stub_start);
        let end = core::ptr::addr_of!(__ogc_general_exception_vector_stub_end);
        let len = end as usize - start as usize;
        if len > VECTOR_SLOT_LEN {
            return false;
        }
        let dest = (0x8000_0000usize + exid as usize * VECTOR_SLOT_LEN) as *mut u8;
        let msr = ppc_mfmsr();
        ppc_mtmsr(msr & !MSR_EE);
        core::ptr::copy_nonoverlapping(start, dest, len);
        // `li r11, imm` is `addi r11,r0,imm` (0x39600000 | imm16).
        let patch = dest.cast::<u32>().add(EXID_INSTRUCTION_INDEX);
        core::ptr::write_volatile(patch, 0x3960_0000 | (exid & 0xffff));
        dc_flush_range(dest.cast_const(), len);
        ic_invalidate_range(dest.cast_const(), len);
        ppc_mtmsr(msr);
    }

    true
}

/// Install the PowerPC decrementer exception vector used by LWP deadline service.
pub fn install_decrementer_exception_vector() {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        unsafe extern "Rust" {
            static __ogc_decrementer_vector_stub_start: u8;
            static __ogc_decrementer_vector_stub_end: u8;
        }
        const VECTOR_ADDR: usize = 0x8000_0900;
        let start = core::ptr::addr_of!(__ogc_decrementer_vector_stub_start);
        let end = core::ptr::addr_of!(__ogc_decrementer_vector_stub_end);
        let len = end as usize - start as usize;
        assert!(len == DECREMENTER_VECTOR_STUB_LEN);
        assert!(len <= DECREMENTER_VECTOR_SLOT_LEN);
        let dest = VECTOR_ADDR as *mut u8;
        let msr = ppc_mfmsr();
        ppc_mtmsr(msr & !MSR_EE);
        let state = DECREMENTER_VECTOR_STATE.load(Ordering::Acquire);
        if state == 1 {
            ppc_mtmsr(msr);
            return;
        }
        if state == 0 {
            let mut original = [0; DECREMENTER_VECTOR_STUB_LEN];
            for (offset, byte) in original.iter_mut().enumerate() {
                *byte = core::ptr::read_volatile(dest.add(offset));
            }
            let mut backup = [0; DECREMENTER_VECTOR_STUB_LEN];
            snapshot_vector_bytes(&original, &mut backup);
            for (saved, byte) in DECREMENTER_VECTOR_BACKUP.iter().zip(backup) {
                saved.store(byte, Ordering::Relaxed);
            }
        }
        core::ptr::copy_nonoverlapping(start, dest, len);
        dc_flush_range(dest.cast_const(), len);
        ic_invalidate_range(dest.cast_const(), len);
        DECREMENTER_VECTOR_STATE.store(1, Ordering::Release);
        ppc_mtmsr(msr);
    }
}

/// Restore the decrementer vector bytes that were present before the first
/// Rust LWP deadline-vector installation. Repeated calls are safe. A later
/// install reuses the original snapshot instead of saving Rust's own stub.
pub fn restore_decrementer_exception_vector() {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        const VECTOR_ADDR: usize = 0x8000_0900;
        let msr = ppc_mfmsr();
        ppc_mtmsr(msr & !MSR_EE);
        if DECREMENTER_VECTOR_STATE.load(Ordering::Acquire) != 1 {
            ppc_mtmsr(msr);
            return;
        }

        let dest = VECTOR_ADDR as *mut u8;
        let mut backup = [0; DECREMENTER_VECTOR_STUB_LEN];
        for (byte, saved) in backup.iter_mut().zip(DECREMENTER_VECTOR_BACKUP.iter()) {
            *byte = saved.load(Ordering::Relaxed);
        }
        let mut restored = [0; DECREMENTER_VECTOR_STUB_LEN];
        restore_vector_bytes(&backup, &mut restored);
        for (offset, byte) in restored.iter().enumerate() {
            core::ptr::write_volatile(dest.add(offset), *byte);
        }
        dc_flush_range(dest.cast_const(), DECREMENTER_VECTOR_STUB_LEN);
        ic_invalidate_range(dest.cast_const(), DECREMENTER_VECTOR_STUB_LEN);
        DECREMENTER_VECTOR_STATE.store(2, Ordering::Release);
        ppc_mtmsr(msr);
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn __ogc_sys_irq_dispatch() -> u32 {
    let raw = IRQ_DISPATCH_CALLBACK.load(Ordering::Acquire);
    if raw == 0 {
        return 0;
    }
    let callback: IrqDispatchCallback = unsafe { core::mem::transmute(raw) };
    unsafe { callback() }
}

pub fn install_irq_exception_vector() {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        let start = core::ptr::addr_of!(__ogc_irq_vector_stub_start);
        let end = core::ptr::addr_of!(__ogc_irq_vector_stub_end);
        let len = end as usize - start as usize;
        assert!(len == IRQ_VECTOR_STUB_LEN);
        let dest = IRQ_VECTOR_ADDR as *mut u8;

        let msr = ppc_mfmsr();
        ppc_mtmsr(msr & !MSR_EE);
        let state = IRQ_VECTOR_STATE.load(Ordering::Acquire);
        if state == 1 {
            ppc_mtmsr(msr);
            return;
        }
        if state == 0 {
            let mut original = [0; IRQ_VECTOR_STUB_LEN];
            for (offset, byte) in original.iter_mut().enumerate() {
                *byte = core::ptr::read_volatile(dest.add(offset));
            }
            let mut backup = [0; IRQ_VECTOR_STUB_LEN];
            snapshot_vector_bytes(&original, &mut backup);
            for (saved, byte) in IRQ_VECTOR_BACKUP.iter().zip(backup) {
                saved.store(byte, Ordering::Relaxed);
            }
        }
        core::ptr::copy_nonoverlapping(start, dest, len);
        dc_flush_range(dest.cast_const(), len);
        ic_invalidate_range(dest.cast_const(), len);
        IRQ_VECTOR_STATE.store(1, Ordering::Release);
        ppc_mtmsr(msr);
    }
}

/// Restore the external interrupt vector bytes that were present before the
/// first Rust IRQ vector installation. Repeated calls are safe. A later install
/// reuses the original snapshot instead of saving Rust's own stub.
pub fn restore_irq_exception_vector() {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        let msr = ppc_mfmsr();
        ppc_mtmsr(msr & !MSR_EE);
        if IRQ_VECTOR_STATE.load(Ordering::Acquire) != 1 {
            ppc_mtmsr(msr);
            return;
        }

        let dest = IRQ_VECTOR_ADDR as *mut u8;
        let mut backup = [0; IRQ_VECTOR_STUB_LEN];
        for (byte, saved) in backup.iter_mut().zip(IRQ_VECTOR_BACKUP.iter()) {
            *byte = saved.load(Ordering::Relaxed);
        }
        let mut restored = [0; IRQ_VECTOR_STUB_LEN];
        restore_vector_bytes(&backup, &mut restored);
        for (offset, byte) in restored.iter().enumerate() {
            core::ptr::write_volatile(dest.add(offset), *byte);
        }
        dc_flush_range(dest.cast_const(), IRQ_VECTOR_STUB_LEN);
        ic_invalidate_range(dest.cast_const(), IRQ_VECTOR_STUB_LEN);
        IRQ_VECTOR_STATE.store(2, Ordering::Release);
        ppc_mtmsr(msr);
    }
}

#[cfg(target_arch = "powerpc")]
unsafe fn ppc_mfmsr() -> u32 {
    let value: u32;
    // SAFETY: This helper is PowerPC-only and its caller is responsible for
    // ensuring that reading MSR is valid in the current execution context.
    unsafe {
        core::arch::asm!("mfmsr {0}", out(reg) value, options(nostack, preserves_flags));
    }
    value
}

#[cfg(target_arch = "powerpc")]
unsafe fn ppc_mtmsr(value: u32) {
    // SAFETY: The caller supplies the MSR value and guarantees that changing
    // processor state here is valid. `isync` serializes the new state.
    unsafe {
        core::arch::asm!("mtmsr {0}", in(reg) value, options(nostack, preserves_flags));
        core::arch::asm!("isync", options(nostack, preserves_flags));
    }
}

#[cfg(target_arch = "powerpc")]
unsafe fn dc_flush_range(start: *const u8, len: usize) {
    if len == 0 {
        return;
    }
    let mut addr = (start as usize) & !(CACHE_LINE_SIZE - 1);
    let end = (start as usize)
        .wrapping_add(len)
        .wrapping_add(CACHE_LINE_SIZE - 1)
        & !(CACHE_LINE_SIZE - 1);
    // SAFETY: The caller guarantees that the supplied address range is valid
    // for cache maintenance. Addresses are rounded to cache-line boundaries.
    unsafe {
        while addr < end {
            core::arch::asm!("dcbf 0, {addr}", addr = in(reg) addr, options(nostack));
            addr = addr.wrapping_add(CACHE_LINE_SIZE);
        }
        core::arch::asm!("sync", options(nostack));
    }
}

#[cfg(target_arch = "powerpc")]
unsafe fn ic_invalidate_range(start: *const u8, len: usize) {
    if len == 0 {
        return;
    }
    let mut addr = (start as usize) & !(CACHE_LINE_SIZE - 1);
    let end = (start as usize)
        .wrapping_add(len)
        .wrapping_add(CACHE_LINE_SIZE - 1)
        & !(CACHE_LINE_SIZE - 1);
    // SAFETY: The caller guarantees that the supplied address range is valid
    // for cache maintenance. `sync` and `isync` order the invalidation before
    // subsequent instruction fetches.
    unsafe {
        while addr < end {
            core::arch::asm!("icbi 0, {addr}", addr = in(reg) addr, options(nostack));
            addr = addr.wrapping_add(CACHE_LINE_SIZE);
        }
        core::arch::asm!("sync", options(nostack));
        core::arch::asm!("isync", options(nostack));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_snapshot_restores_exact_original_bytes_after_reinstall() {
        let original = [0xA5; IRQ_VECTOR_STUB_LEN];
        let rust_stub = [0x5A; IRQ_VECTOR_STUB_LEN];
        let mut saved = [0; IRQ_VECTOR_STUB_LEN];
        let mut live_vector = original;

        snapshot_vector_bytes(&live_vector, &mut saved);
        live_vector = rust_stub;
        restore_vector_bytes(&saved, &mut live_vector);
        assert_eq!(live_vector, original);

        // A later install must use the first snapshot, not replace it with the
        // Rust stub that was in the vector between restore and reinstall.
        live_vector = rust_stub;
        restore_vector_bytes(&saved, &mut live_vector);
        assert_eq!(live_vector, original);
    }

    #[cfg(target_arch = "powerpc")]
    #[test]
    fn decrementer_vector_stub_fits_its_reserved_slot() {
        unsafe extern "Rust" {
            static __ogc_decrementer_vector_stub_start: u8;
            static __ogc_decrementer_vector_stub_end: u8;
        }
        let start = core::ptr::addr_of!(__ogc_decrementer_vector_stub_start) as usize;
        let end = core::ptr::addr_of!(__ogc_decrementer_vector_stub_end) as usize;
        let len = end - start;

        assert_eq!(len, DECREMENTER_VECTOR_STUB_LEN);
        assert_eq!(len % 4, 0);
        assert!(len <= DECREMENTER_VECTOR_SLOT_LEN);
    }

    #[cfg(target_arch = "powerpc")]
    #[test]
    fn interrupt_scratch_uses_c_context_fields_before_reset_vector() {
        unsafe extern "Rust" {
            static __ogc_interrupt_saved_sp: u8;
            static __ogc_interrupt_saved_srr0: u8;
            static __ogc_interrupt_saved_srr1: u8;
        }
        // Fixed C ABI expectations; inspect the actual assembly symbols.
        assert_eq!(
            core::ptr::addr_of!(__ogc_interrupt_saved_srr0) as usize,
            0x8000_00d0
        );
        assert_eq!(
            core::ptr::addr_of!(__ogc_interrupt_saved_srr1) as usize,
            0x8000_00d4
        );
        assert_eq!(
            core::ptr::addr_of!(__ogc_interrupt_saved_sp) as usize,
            0x8000_00ec
        );
    }

    #[test]
    fn critical_section_restores_only_saved_interrupt_and_fp_bits() {
        let unrelated = 0x1234_5678 & !MSR_LOCK_MASK;
        let originally_enabled = MSR_EE | MSR_FP | unrelated;
        let state = msr_lock_state(originally_enabled);
        assert_eq!(state, MSR_LOCK_MASK);
        assert_eq!(msr_unlock_value(unrelated, state), originally_enabled);

        let originally_disabled = unrelated;
        let state = msr_lock_state(originally_disabled);
        assert_eq!(state, 0);
        assert_eq!(
            msr_unlock_value(MSR_LOCK_MASK | unrelated, state),
            unrelated
        );
    }

    #[test]
    fn runtime_interrupt_setup_enables_ee_and_ri_and_preserves_other_bits() {
        let unrelated = 0x1234_5678 & !(MSR_EE | MSR_RI);
        assert_eq!(runtime_msr_value(unrelated), unrelated | MSR_EE | MSR_RI);
        assert_eq!(runtime_msr_value(MSR_EE | MSR_RI), MSR_EE | MSR_RI);
    }

    #[test]
    fn detects_only_the_hbc_reload_stub_signature() {
        assert!(has_hbc_reload_stub_signature(0x5354_5542, 0x4841_5858));
        assert!(!has_hbc_reload_stub_signature(0, 0));
        assert!(!has_hbc_reload_stub_signature(0x5354_5542, 0x4841_5859));
        assert!(!has_hbc_reload_stub_signature(0x5354_5543, 0x4841_5858));
    }

    #[test]
    fn exit_callback_can_be_registered_and_cleared() {
        extern "C" fn exit_handler(_status: i32) -> ! {
            loop {
                core::hint::spin_loop();
            }
        }

        set_exit_callback(Some(exit_handler));
        assert_eq!(
            EXIT_CALLBACK.load(Ordering::Acquire),
            exit_handler as *const () as usize
        );
        set_exit_callback(None);
        assert_eq!(EXIT_CALLBACK.load(Ordering::Acquire), 0);
    }

    #[test]
    fn shelters_overlapping_hbc_arguments_before_bss_clear() {
        let mut memory = [0u8; 256];
        let aligned = (memory.as_mut_ptr() as usize + 7) & !7;
        let dest = aligned as *mut u8;
        let source = unsafe { dest.add(8) };
        let command = b"--report=10.0.0.238:4298\0--run-id=42\0\0";
        unsafe { core::ptr::copy_nonoverlapping(command.as_ptr(), source, command.len()) };
        let mut args = BootArgv {
            magic: STARTUP_ARGV_MAGIC,
            command_line: source,
            length: command.len() as i32,
            argc: 0,
            argv: core::ptr::null_mut(),
            end_argv: core::ptr::null_mut(),
        };
        let end = unsafe { shelter_argv_into(&mut args, dest, 240) }.unwrap();
        assert_eq!(args.argc, 2);
        assert_eq!(args.command_line, dest);
        assert_eq!(args.end_argv as usize, end);
        assert_eq!(
            unsafe {
                core::ffi::CStr::from_ptr(*args.argv.cast::<*const core::ffi::c_char>()).to_bytes()
            },
            b"--report=10.0.0.238:4298"
        );
        assert_eq!(
            unsafe {
                core::ffi::CStr::from_ptr(*args.argv.add(1).cast::<*const core::ffi::c_char>())
                    .to_bytes()
            },
            b"--run-id=42"
        );
        assert!(unsafe { (*args.argv.add(2)).is_null() });
    }

    #[test]
    fn sheltered_arguments_preserve_non_utf8_bytes() {
        let mut source = [0xff, b'x', 0, 0x80, 0];
        let mut memory = [0xa5u8; 128];
        let mut args = BootArgv {
            magic: STARTUP_ARGV_MAGIC,
            command_line: source.as_mut_ptr(),
            length: source.len() as i32,
            argc: 0,
            argv: core::ptr::null_mut(),
            end_argv: core::ptr::null_mut(),
        };
        assert!(
            unsafe { shelter_argv_into(&mut args, memory.as_mut_ptr(), memory.len()) }.is_some()
        );
        source.fill(0);
        assert_eq!(args.argc, 2);
        assert_eq!(
            unsafe { core::ffi::CStr::from_ptr((*args.argv).cast()).to_bytes() },
            &[0xff, b'x']
        );
        assert_eq!(
            unsafe { core::ffi::CStr::from_ptr((*args.argv.add(1)).cast()).to_bytes() },
            &[0x80]
        );
        assert!(unsafe { (*args.argv.add(2)).is_null() });
    }

    #[test]
    fn malformed_or_oversized_arguments_leave_destination_unchanged() {
        let mut source = [b'x', 0];
        let mut memory = [0xa5u8; 128];
        for (magic, length, capacity) in [
            (0, 2, 128),
            (STARTUP_ARGV_MAGIC, -1, 128),
            (STARTUP_ARGV_MAGIC, 1, 128),
            (STARTUP_ARGV_MAGIC, 2, 2),
        ] {
            let mut args = BootArgv {
                magic,
                command_line: source.as_mut_ptr(),
                length,
                argc: 99,
                argv: memory.as_mut_ptr().cast(),
                end_argv: memory.as_mut_ptr().cast(),
            };
            assert_eq!(
                unsafe { shelter_argv_into(&mut args, memory.as_mut_ptr(), capacity) },
                None
            );
            assert_eq!(memory, [0xa5; 128]);
            assert_eq!(args.argc, 0);
            assert!(args.argv.is_null());
            assert!(args.end_argv.is_null());
        }
    }

    #[test]
    fn status_helpers_convert_c_return_codes() {
        assert_eq!(result_from_status(0), Ok(0));
        assert_eq!(result_from_status(7), Ok(7));
        assert_eq!(result_from_status(-3), Err(ErrorCode::new(-3)));
        assert_eq!(unit_from_status(1), Ok(()));
        assert_eq!(bool_from_status(0), Ok(false));
        assert_eq!(bool_from_status(1), Ok(true));
        assert_eq!(unit_from_bool_status(0), Err(ErrorCode::new(-1)));
        assert_eq!(unit_from_bool_status(1), Ok(()));
        assert_eq!(unit_from_bool_status(-2), Err(ErrorCode::new(-2)));
    }

    unsafe extern "C" fn sample_irq_callback() -> u32 {
        1
    }

    #[test]
    fn irq_dispatch_callback_round_trips() {
        set_irq_dispatch_callback(Some(sample_irq_callback));
        assert_eq!(__ogc_sys_irq_dispatch(), 1);
        set_irq_dispatch_callback(None);
        assert_eq!(__ogc_sys_irq_dispatch(), 0);
    }
}
