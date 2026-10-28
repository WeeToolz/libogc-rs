#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::ffi::c_void;
use core::sync::atomic::{AtomicPtr, AtomicU32, AtomicUsize, Ordering};

#[cfg(target_arch = "powerpc")]
use core::sync::atomic::AtomicBool;
#[cfg(target_arch = "powerpc")]
use ogc_sys::{install_irq_exception_vector, set_irq_dispatch_callback};

pub use u32;

pub type KIrqId = u32;
pub type KIrqHandlerFn = Option<unsafe extern "C" fn(id: KIrqId, user: *mut c_void)>;

pub const IRQ_PI_ERROR: KIrqId = 0;
pub const IRQ_PI_DEBUG: KIrqId = 1;
pub const IRQ_MEM0: KIrqId = 2;
pub const IRQ_MEM1: KIrqId = 3;
pub const IRQ_MEM2: KIrqId = 4;
pub const IRQ_MEM3: KIrqId = 5;
pub const IRQ_MEMADDRESS: KIrqId = 6;
pub const IRQ_PI_RSW: KIrqId = 7;
pub const IRQ_PI_VI: KIrqId = 8;
pub const IRQ_PI_PETOKEN: KIrqId = 9;
pub const IRQ_PI_PEFINISH: KIrqId = 10;
pub const IRQ_PI_HSP: KIrqId = 11;
pub const IRQ_DSP_ARAM: KIrqId = 12;
pub const IRQ_DSP_DSP: KIrqId = 13;
pub const IRQ_AI: KIrqId = 14;
pub const IRQ_EXI0_EXI: KIrqId = 15;
pub const IRQ_EXI0_TC: KIrqId = 16;
pub const IRQ_EXI0_EXT: KIrqId = 17;
pub const IRQ_EXI1_EXI: KIrqId = 18;
pub const IRQ_EXI1_TC: KIrqId = 19;
pub const IRQ_EXI1_EXT: KIrqId = 20;
pub const IRQ_EXI2_EXI: KIrqId = 21;
pub const IRQ_EXI2_TC: KIrqId = 22;
pub const IRQ_PI_SI: KIrqId = 23;
pub const IRQ_PI_DI: KIrqId = 24;
pub const IRQ_DSP_AI: KIrqId = 25;
pub const IRQ_PI_CP: KIrqId = 26;
const IRQ_PI_ACR_VALUE: KIrqId = 27;
#[cfg(not(gamecube))]
pub const IRQ_PI_ACR: KIrqId = IRQ_PI_ACR_VALUE;

#[cfg(gamecube)]
const IRQ_COUNT: usize = (IRQ_PI_CP as usize) + 1;
#[cfg(not(gamecube))]
const IRQ_COUNT: usize = (IRQ_PI_ACR_VALUE as usize) + 1;

#[cfg(target_arch = "powerpc")]
const IM_MEM0: u32 = 1 << IRQ_MEM0;
#[cfg(target_arch = "powerpc")]
const IM_MEM1: u32 = 1 << IRQ_MEM1;
#[cfg(target_arch = "powerpc")]
const IM_MEM2: u32 = 1 << IRQ_MEM2;
#[cfg(target_arch = "powerpc")]
const IM_MEM3: u32 = 1 << IRQ_MEM3;
#[cfg(target_arch = "powerpc")]
const IM_MEMADDRESS: u32 = 1 << IRQ_MEMADDRESS;
#[cfg(target_arch = "powerpc")]
const IM_MEM: u32 = IM_MEM0 | IM_MEM1 | IM_MEM2 | IM_MEM3 | IM_MEMADDRESS;

#[cfg(target_arch = "powerpc")]
const IM_DSP_AI: u32 = 1 << IRQ_DSP_AI;
#[cfg(target_arch = "powerpc")]
const IM_DSP_ARAM: u32 = 1 << IRQ_DSP_ARAM;
#[cfg(target_arch = "powerpc")]
const IM_DSP_DSP: u32 = 1 << IRQ_DSP_DSP;
#[cfg(target_arch = "powerpc")]
const IM_DSP: u32 = IM_DSP_AI | IM_DSP_ARAM | IM_DSP_DSP;

#[cfg(target_arch = "powerpc")]
const IM_AI: u32 = 1 << IRQ_AI;

#[cfg(target_arch = "powerpc")]
const IM_EXI0_EXI: u32 = 1 << IRQ_EXI0_EXI;
#[cfg(target_arch = "powerpc")]
const IM_EXI0_TC: u32 = 1 << IRQ_EXI0_TC;
#[cfg(target_arch = "powerpc")]
const IM_EXI0_EXT: u32 = 1 << IRQ_EXI0_EXT;
#[cfg(target_arch = "powerpc")]
const IM_EXI0: u32 = IM_EXI0_EXI | IM_EXI0_TC | IM_EXI0_EXT;
#[cfg(target_arch = "powerpc")]
const IM_EXI1_EXI: u32 = 1 << IRQ_EXI1_EXI;
#[cfg(target_arch = "powerpc")]
const IM_EXI1_TC: u32 = 1 << IRQ_EXI1_TC;
#[cfg(target_arch = "powerpc")]
const IM_EXI1_EXT: u32 = 1 << IRQ_EXI1_EXT;
#[cfg(target_arch = "powerpc")]
const IM_EXI1: u32 = IM_EXI1_EXI | IM_EXI1_TC | IM_EXI1_EXT;
#[cfg(target_arch = "powerpc")]
const IM_EXI2_EXI: u32 = 1 << IRQ_EXI2_EXI;
#[cfg(target_arch = "powerpc")]
const IM_EXI2_TC: u32 = 1 << IRQ_EXI2_TC;
#[cfg(target_arch = "powerpc")]
const IM_EXI2: u32 = IM_EXI2_EXI | IM_EXI2_TC;

#[cfg(target_arch = "powerpc")]
const IM_PI_ERROR: u32 = 1 << IRQ_PI_ERROR;
#[cfg(target_arch = "powerpc")]
const IM_PI_DEBUG: u32 = 1 << IRQ_PI_DEBUG;
#[cfg(target_arch = "powerpc")]
const IM_PI_RSW: u32 = 1 << IRQ_PI_RSW;
#[cfg(target_arch = "powerpc")]
const IM_PI_VI: u32 = 1 << IRQ_PI_VI;
#[cfg(target_arch = "powerpc")]
const IM_PI_PETOKEN: u32 = 1 << IRQ_PI_PETOKEN;
#[cfg(target_arch = "powerpc")]
const IM_PI_PEFINISH: u32 = 1 << IRQ_PI_PEFINISH;
#[cfg(target_arch = "powerpc")]
const IM_PI_HSP: u32 = 1 << IRQ_PI_HSP;
#[cfg(target_arch = "powerpc")]
const IM_PI_SI: u32 = 1 << IRQ_PI_SI;
#[cfg(target_arch = "powerpc")]
const IM_PI_DI: u32 = 1 << IRQ_PI_DI;
#[cfg(target_arch = "powerpc")]
const IM_PI_CP: u32 = 1 << IRQ_PI_CP;
#[cfg(target_arch = "powerpc")]
const IM_PI_ACR_BIT: u32 = 1 << IRQ_PI_ACR_VALUE;
#[cfg(target_arch = "powerpc")]
const IM_PI_BASE: u32 = IM_PI_ERROR
    | IM_PI_DEBUG
    | IM_PI_RSW
    | IM_PI_VI
    | IM_PI_PETOKEN
    | IM_PI_PEFINISH
    | IM_PI_HSP
    | IM_PI_SI
    | IM_PI_DI
    | IM_PI_CP;
#[cfg(all(target_arch = "powerpc", gamecube))]
const IM_PI: u32 = IM_PI_BASE;
#[cfg(all(target_arch = "powerpc", not(gamecube)))]
const IM_PI: u32 = IM_PI_BASE | IM_PI_ACR_BIT;

#[cfg(any(target_arch = "powerpc", test))]
const MSR_EE: u32 = 1 << 15;

#[cfg(any(target_arch = "powerpc", test))]
const fn msr_irq_saved_state(msr: u32) -> u32 {
    msr & MSR_EE
}

#[cfg(any(target_arch = "powerpc", test))]
const fn msr_irq_lock_value(msr: u32) -> u32 {
    msr & !MSR_EE
}

#[cfg(any(target_arch = "powerpc", test))]
const fn msr_irq_unlock_value(current: u32, state: u32) -> u32 {
    (current & !MSR_EE) | state
}

static IRQ_MASK: AtomicU32 = AtomicU32::new(0);
static IRQ_HANDLERS: [AtomicUsize; IRQ_COUNT] = [const { AtomicUsize::new(0) }; IRQ_COUNT];
static IRQ_USERS: [AtomicPtr<c_void>; IRQ_COUNT] =
    [const { AtomicPtr::new(core::ptr::null_mut()) }; IRQ_COUNT];
#[cfg(target_arch = "powerpc")]
static HOLLYWOOD_REVISION: AtomicU32 = AtomicU32::new(u32::MAX);
#[cfg(target_arch = "powerpc")]
static IRQ_RUNTIME_INITIALIZED: AtomicBool = AtomicBool::new(false);

#[cfg(target_arch = "powerpc")]
unsafe extern "C" fn irq_dispatch_callback() -> u32 {
    k_irq_dispatch_pending() as u32
}

#[cfg(target_arch = "powerpc")]
fn ensure_irq_dispatch_callback_registered() {
    set_irq_dispatch_callback(Some(irq_dispatch_callback));
}

#[cfg(not(target_arch = "powerpc"))]
fn ensure_irq_dispatch_callback_registered() {}

#[cfg(target_arch = "powerpc")]
fn ensure_irq_runtime_initialized() {
    if IRQ_RUNTIME_INITIALIZED.swap(true, Ordering::AcqRel) {
        return;
    }

    unsafe {
        install_irq_exception_vector();
        set_irq_dispatch_callback(Some(irq_dispatch_callback));

        // Match libogc's KIrqInit baseline before individual handlers/masks
        // start mutating device state.
        write_pi(1, 0);
        write_mi(14, 0);
        write_dsp(5, read_dsp(5) & !0x01f8);
        write_ai(0, read_ai(0) & !0x0000_002c);
        write_exi(0, read_exi(0) & !0x0000_2c0f);
        write_exi(5, read_exi(5) & !0x0000_0c0f);
        write_exi(10, read_exi(10) & !0x0000_000f);
        write_pi(1, 0x0000_00f0);
        apply_stored_irq_mask(&IRQ_MASK, |mask| modify_hw_masks(0, mask));

        // Enable external interrupts once the vector and dispatch callback are
        // in place so device IRQs can reach the registered handler table.
        let msr = ppc_mfmsr();
        ppc_mtmsr(msr | MSR_EE);
    }
}

#[cfg(target_arch = "powerpc")]
fn reset_irq_runtime_initialized(initialized: &AtomicBool) {
    initialized.store(false, Ordering::Release);
}

#[cfg(target_arch = "powerpc")]
fn apply_stored_irq_mask(mask: &AtomicU32, apply: impl FnOnce(u32)) {
    apply(mask.load(Ordering::Acquire));
}

#[cfg(not(target_arch = "powerpc"))]
fn ensure_irq_runtime_initialized() {}

#[inline]
fn index(id: KIrqId) -> Option<usize> {
    let wii = {
        #[cfg(target_arch = "powerpc")]
        {
            is_wii()
        }
        #[cfg(not(target_arch = "powerpc"))]
        {
            true
        }
    };
    index_for_console(id, wii)
}

#[inline]
fn index_for_console(id: KIrqId, wii: bool) -> Option<usize> {
    if id == IRQ_PI_ACR_VALUE && !wii {
        return None;
    }
    let idx = id as usize;
    (idx < IRQ_COUNT).then_some(idx)
}

#[cfg(target_arch = "powerpc")]
fn is_wii() -> bool {
    const UNKNOWN_REVISION: u32 = u32::MAX;
    let revision = HOLLYWOOD_REVISION.load(Ordering::Acquire);
    if revision != UNKNOWN_REVISION {
        return revision != 0;
    }

    let revision = unsafe { ogc_sys::sys_get_hollywood_revision() };
    HOLLYWOOD_REVISION.store(revision, Ordering::Release);
    revision != 0
}

#[cfg(target_arch = "powerpc")]
fn platform_irq_mask(mask: u32) -> u32 {
    // BUG FIX: C exposes the ACR interrupt only on Wii, but this shared target
    // also runs on GameCube. Do not enable its reserved PI bit on GameCube.
    platform_irq_mask_for_console(mask, is_wii())
}

#[cfg(target_arch = "powerpc")]
fn platform_irq_mask_for_console(mask: u32, wii: bool) -> u32 {
    if wii { mask } else { mask & !IM_PI_ACR_BIT }
}

#[inline]
fn encode_handler(handler: KIrqHandlerFn) -> usize {
    handler.map_or(0, |f| f as usize)
}

#[inline]
fn decode_handler(raw: usize) -> KIrqHandlerFn {
    if raw == 0 {
        None
    } else {
        Some(unsafe {
            core::mem::transmute::<usize, unsafe extern "C" fn(KIrqId, *mut c_void)>(raw)
        })
    }
}

#[cfg(all(test, target_arch = "powerpc"))]
fn stored_user(id: KIrqId) -> Option<*mut c_void> {
    let idx = index(id)?;
    Some(IRQ_USERS[idx].load(Ordering::Acquire))
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn ppc_mfmsr() -> u32 {
    unsafe {
        let value: u32;
        core::arch::asm!("mfmsr {0}", out(reg) value, options(nostack));
        value
    }
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn ppc_mtmsr(value: u32) {
    unsafe {
        core::arch::asm!("mtmsr {0}", in(reg) value, options(nostack));
    }
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn lock_by_msr() -> u32 {
    unsafe {
        let msr = ppc_mfmsr();
        ppc_mtmsr(msr_irq_lock_value(msr));
        msr_irq_saved_state(msr)
    }
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn unlock_by_msr(state: u32) {
    unsafe {
        let msr = ppc_mfmsr();
        ppc_mtmsr(msr_irq_unlock_value(msr, state));
    }
}

/// Disable external interrupt delivery and return its previous MSR state.
///
/// This matches the save/restore contract of libogc's `IRQ_Disable`.
pub fn k_irq_disable_all() -> u32 {
    unsafe { lock_by_msr() }
}

/// Restore external interrupt delivery from a state returned by
/// [`k_irq_disable_all`].
pub fn k_irq_restore_all(state: u32) {
    unsafe { unlock_by_msr(state) }
}

#[cfg(target_arch = "powerpc")]
const PI_REG_BASE: usize = 0xCC00_3000;
#[cfg(target_arch = "powerpc")]
const MI_REG_BASE: usize = 0xCC00_4000;
#[cfg(target_arch = "powerpc")]
const DSP_REG_BASE: usize = 0xCC00_5000;
#[cfg(target_arch = "powerpc")]
const EXI_REG_BASE: usize = 0xCD00_6800;
#[cfg(target_arch = "powerpc")]
const AI_REG_BASE: usize = 0xCD00_6C00;

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn pi_reg(index: usize) -> *mut u32 {
    unsafe { (PI_REG_BASE as *mut u32).add(index) }
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn mi_reg(index: usize) -> *mut u16 {
    unsafe { (MI_REG_BASE as *mut u16).add(index) }
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn dsp_reg(index: usize) -> *mut u16 {
    unsafe { (DSP_REG_BASE as *mut u16).add(index) }
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn exi_reg(index: usize) -> *mut u32 {
    unsafe { (EXI_REG_BASE as *mut u32).add(index) }
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn ai_reg(index: usize) -> *mut u32 {
    unsafe { (AI_REG_BASE as *mut u32).add(index) }
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn read_pi(index: usize) -> u32 {
    unsafe { core::ptr::read_volatile(pi_reg(index)) }
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn write_pi(index: usize, value: u32) {
    unsafe {
        core::ptr::write_volatile(pi_reg(index), value);
    }
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn read_mi(index: usize) -> u16 {
    unsafe { core::ptr::read_volatile(mi_reg(index)) }
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn write_mi(index: usize, value: u16) {
    unsafe {
        core::ptr::write_volatile(mi_reg(index), value);
    }
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn read_dsp(index: usize) -> u16 {
    unsafe { core::ptr::read_volatile(dsp_reg(index)) }
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn write_dsp(index: usize, value: u16) {
    unsafe {
        core::ptr::write_volatile(dsp_reg(index), value);
    }
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn read_exi(index: usize) -> u32 {
    unsafe { core::ptr::read_volatile(exi_reg(index)) }
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn write_exi(index: usize, value: u32) {
    unsafe {
        core::ptr::write_volatile(exi_reg(index), value);
    }
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn read_ai(index: usize) -> u32 {
    unsafe { core::ptr::read_volatile(ai_reg(index)) }
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn write_ai(index: usize, value: u32) {
    unsafe {
        core::ptr::write_volatile(ai_reg(index), value);
    }
}

#[cfg(target_arch = "powerpc")]
#[inline]
fn encode_mem(mask: u32) -> u16 {
    ((mask >> IRQ_MEM0) & 0x1f) as u16
}

#[cfg(target_arch = "powerpc")]
#[inline]
fn encode_dsp(mask: u32) -> u16 {
    let mut out = 0u16;
    if (mask & IM_DSP_ARAM) != 0 {
        out |= 1 << 6;
    }
    if (mask & IM_DSP_DSP) != 0 {
        out |= 1 << 8;
    }
    if (mask & IM_DSP_AI) != 0 {
        out |= 1 << 4;
    }
    out
}

#[cfg(target_arch = "powerpc")]
#[inline]
fn encode_ai(mask: u32) -> u32 {
    let mut out = 0u32;
    if (mask & IM_AI) != 0 {
        out |= 1 << 2;
    }
    out
}

#[cfg(target_arch = "powerpc")]
#[inline]
fn encode_exi(mask: u32) -> u32 {
    let mut out = 0u32;
    if (mask & IM_EXI0_EXI) != 0 {
        out |= 1 << 0;
    }
    if (mask & IM_EXI0_TC) != 0 {
        out |= 1 << 2;
    }
    if (mask & IM_EXI0_EXT) != 0 {
        out |= 1 << 10;
    }
    out
}

#[cfg(target_arch = "powerpc")]
#[inline]
fn encode_pi(mask: u32) -> u32 {
    let mut out = 0u32;
    if (mask & IM_PI_ERROR) != 0 {
        out |= 1 << 0;
    }
    if (mask & IM_PI_DEBUG) != 0 {
        out |= 1 << 12;
    }
    if (mask & IM_PI_RSW) != 0 {
        out |= 1 << 1;
    }
    if (mask & IM_PI_VI) != 0 {
        out |= 1 << 8;
    }
    if (mask & IM_PI_PETOKEN) != 0 {
        out |= 1 << 9;
    }
    if (mask & IM_PI_PEFINISH) != 0 {
        out |= 1 << 10;
    }
    if (mask & IM_PI_HSP) != 0 {
        out |= 1 << 13;
    }
    if (mask & IM_PI_SI) != 0 {
        out |= 1 << 3;
    }
    if (mask & IM_PI_DI) != 0 {
        out |= 1 << 2;
    }
    if (mask & IM_PI_CP) != 0 {
        out |= 1 << 11;
    }
    #[cfg(not(gamecube))]
    if (mask & IM_PI_ACR_BIT) != 0 {
        out |= 1 << 14;
    }
    out
}

#[cfg(target_arch = "powerpc")]
unsafe fn modify_hw_masks(clr_mask: u32, set_mask: u32) {
    unsafe {
        let mod_mask = clr_mask ^ set_mask;
        let clr_mask = clr_mask & mod_mask;
        let set_mask = set_mask & mod_mask;

        if (mod_mask & IM_MEM) != 0 {
            let mut reg = read_mi(14);
            reg &= !encode_mem(clr_mask);
            reg |= encode_mem(set_mask);
            write_mi(14, reg);
        }

        if (mod_mask & IM_DSP) != 0 {
            let mut reg = read_dsp(5) & !0x00A8;
            reg &= !encode_dsp(clr_mask);
            reg |= encode_dsp(set_mask);
            write_dsp(5, reg);
        }

        if (mod_mask & IM_AI) != 0 {
            let mut reg = read_ai(0) & !0x0000_0008;
            reg &= !encode_ai(clr_mask);
            reg |= encode_ai(set_mask);
            write_ai(0, reg);
        }

        if (mod_mask & IM_EXI0) != 0 {
            let mut reg = read_exi(0) & !0x0000_280A;
            reg &= !encode_exi(clr_mask);
            reg |= encode_exi(set_mask);
            write_exi(0, reg);
        }

        if (mod_mask & IM_EXI1) != 0 {
            let mut reg = read_exi(5) & !0x0000_080A;
            reg &= !encode_exi(clr_mask << 3);
            reg |= encode_exi(set_mask << 3);
            write_exi(5, reg);
        }

        if (mod_mask & IM_EXI2) != 0 {
            let mut reg = read_exi(10) & !0x0000_000A;
            reg &= !encode_exi((clr_mask << 6) & !IM_EXI0_EXT);
            reg |= encode_exi((set_mask << 6) & !IM_EXI0_EXT);
            write_exi(10, reg);
        }

        if (mod_mask & IM_PI) != 0 {
            let mut reg = read_pi(1);
            reg &= !encode_pi(clr_mask);
            reg |= encode_pi(set_mask);
            write_pi(1, reg);
        }
    }
}

#[cfg(target_arch = "powerpc")]
#[inline]
unsafe fn decode_exi_cause(exi_cause: u32) -> Option<KIrqId> {
    let cause = exi_cause & (exi_cause << 1);
    if (cause & (1 << 1)) != 0 {
        return Some(IRQ_EXI0_EXI);
    }
    if (cause & (1 << 3)) != 0 {
        return Some(IRQ_EXI0_TC);
    }
    if (cause & (1 << 11)) != 0 {
        return Some(IRQ_EXI0_EXT);
    }
    None
}

#[cfg(target_arch = "powerpc")]
unsafe fn pending_cause() -> Option<KIrqId> {
    unsafe {
        let pi_cause = read_pi(0) & read_pi(1);

        if (pi_cause & (1 << 0)) != 0 {
            return Some(IRQ_PI_ERROR);
        }
        if (pi_cause & (1 << 12)) != 0 {
            return Some(IRQ_PI_DEBUG);
        }
        if (pi_cause & (1 << 7)) != 0 {
            let mi_cause = (read_mi(14) & read_mi(15)) & 0x1f;
            if mi_cause != 0 {
                return Some(mi_cause.trailing_zeros() + IRQ_MEM0);
            }
        }
        if (pi_cause & (1 << 1)) != 0 {
            return Some(IRQ_PI_RSW);
        }
        if (pi_cause & (1 << 8)) != 0 {
            return Some(IRQ_PI_VI);
        }
        if (pi_cause & (1 << 9)) != 0 {
            return Some(IRQ_PI_PETOKEN);
        }
        if (pi_cause & (1 << 10)) != 0 {
            return Some(IRQ_PI_PEFINISH);
        }
        if (pi_cause & (1 << 13)) != 0 {
            return Some(IRQ_PI_HSP);
        }

        let mut has_dsp_ai = false;
        if (pi_cause & (1 << 6)) != 0 {
            let dsp_cause = read_dsp(5);
            let dsp_cause = dsp_cause & (dsp_cause >> 1);
            if (dsp_cause & (1 << 5)) != 0 {
                return Some(IRQ_DSP_ARAM);
            }
            if (dsp_cause & (1 << 7)) != 0 {
                return Some(IRQ_DSP_DSP);
            }
            if (dsp_cause & (1 << 3)) != 0 {
                has_dsp_ai = true;
            }
        }

        if (pi_cause & (1 << 5)) != 0 {
            let ai_cause = read_ai(0);
            let ai_cause = ai_cause & (ai_cause << 1);
            if (ai_cause & (1 << 3)) != 0 {
                return Some(IRQ_AI);
            }
        }

        if (pi_cause & (1 << 4)) != 0 {
            if let Some(cause) = decode_exi_cause(read_exi(0)) {
                return Some(cause);
            }
            if let Some(cause) = decode_exi_cause(read_exi(5)) {
                return Some(cause + (IRQ_EXI1_EXI - IRQ_EXI0_EXI));
            }
            if let Some(cause) = decode_exi_cause(read_exi(10) & !(1 << 11)) {
                return Some(cause + (IRQ_EXI2_EXI - IRQ_EXI0_EXI));
            }
        }

        if (pi_cause & (1 << 3)) != 0 {
            return Some(IRQ_PI_SI);
        }
        if (pi_cause & (1 << 2)) != 0 {
            return Some(IRQ_PI_DI);
        }
        if has_dsp_ai {
            return Some(IRQ_DSP_AI);
        }
        if (pi_cause & (1 << 11)) != 0 {
            return Some(IRQ_PI_CP);
        }
        #[cfg(not(gamecube))]
        if is_wii() && (pi_cause & (1 << 14)) != 0 {
            return Some(IRQ_PI_ACR_VALUE);
        }

        None
    }
}

#[cfg(target_arch = "powerpc")]
fn dispatch_irq(id: KIrqId) -> bool {
    let Some(idx) = index(id) else {
        return false;
    };
    let handler = decode_handler(IRQ_HANDLERS[idx].load(Ordering::Acquire));
    let user = IRQ_USERS[idx].load(Ordering::Acquire);
    if let Some(handler) = handler {
        unsafe {
            handler(id, user);
        }
        true
    } else {
        false
    }
}

pub fn k_irq_enable(mask: u32) {
    ensure_irq_runtime_initialized();
    ensure_irq_dispatch_callback_registered();
    #[cfg(target_arch = "powerpc")]
    unsafe {
        let state = lock_by_msr();
        let mask = platform_irq_mask(mask);
        modify_hw_masks(0, mask);
        unlock_by_msr(state);
        IRQ_MASK.fetch_or(mask, Ordering::AcqRel);
    }
    #[cfg(not(target_arch = "powerpc"))]
    IRQ_MASK.fetch_or(mask, Ordering::AcqRel);
}

pub fn k_irq_disable(mask: u32) {
    ensure_irq_runtime_initialized();
    #[cfg(target_arch = "powerpc")]
    unsafe {
        let state = lock_by_msr();
        let mask = platform_irq_mask(mask);
        modify_hw_masks(mask, 0);
        unlock_by_msr(state);
        IRQ_MASK.fetch_and(!mask, Ordering::AcqRel);
    }
    #[cfg(not(target_arch = "powerpc"))]
    IRQ_MASK.fetch_and(!mask, Ordering::AcqRel);
}

pub fn k_irq_get(id: KIrqId) -> KIrqHandlerFn {
    let idx = index(id)?;
    decode_handler(IRQ_HANDLERS[idx].load(Ordering::Acquire))
}

pub fn k_irq_set(id: KIrqId, fn_: KIrqHandlerFn, user: *mut c_void) -> KIrqHandlerFn {
    let idx = index(id)?;

    ensure_irq_runtime_initialized();
    ensure_irq_dispatch_callback_registered();

    #[cfg(target_arch = "powerpc")]
    unsafe {
        let state = lock_by_msr();
        let previous = IRQ_HANDLERS[idx].swap(encode_handler(fn_), Ordering::AcqRel);
        IRQ_USERS[idx].store(user, Ordering::Release);
        unlock_by_msr(state);
        decode_handler(previous)
    }

    #[cfg(not(target_arch = "powerpc"))]
    {
        let previous = IRQ_HANDLERS[idx].swap(encode_handler(fn_), Ordering::AcqRel);
        IRQ_USERS[idx].store(user, Ordering::Release);
        decode_handler(previous)
    }
}

pub fn mask() -> u32 {
    IRQ_MASK.load(Ordering::Acquire)
}

/// Dispatches one currently pending hardware IRQ using the registered handler table.
///
/// On PowerPC this uses the same hardware cause decoder as the installed IRQ
/// exception vector, without polling IOS or synthesizing completion.
pub fn k_irq_dispatch_pending() -> bool {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        if let Some(id) = pending_cause() {
            return dispatch_irq(id);
        }
    }
    false
}

/// Restore the external interrupt vector bytes that were present before the
/// first IRQ runtime initialization.
pub fn k_irq_restore_exception_vector() {
    ogc_sys::restore_irq_exception_vector();
    #[cfg(target_arch = "powerpc")]
    reset_irq_runtime_initialized(&IRQ_RUNTIME_INITIALIZED);
}

/// Restore every exception vector installed by the core Rust runtime.
///
/// This covers the external interrupt vector and the LWP decrementer vector.
/// General exception vectors installed explicitly by debugger clients are not
/// core runtime state and remain owned by those clients.
pub fn k_irq_restore_runtime_exception_vectors() {
    ogc_sys::restore_decrementer_exception_vector();
    k_irq_restore_exception_vector();
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    unsafe extern "C" fn sample_handler(_id: KIrqId, _user: *mut c_void) {}
    const SAMPLE_HANDLER: KIrqHandlerFn = Some(sample_handler);

    fn handler_slot(handler: KIrqHandlerFn) -> usize {
        encode_handler(handler)
    }

    #[test]
    fn gets_none_for_unset_handler() {
        assert_eq!(handler_slot(k_irq_get(IRQ_MEM0)), 0);
    }

    #[test]
    fn set_returns_previous_and_stores_new_handler() {
        assert_eq!(
            handler_slot(k_irq_set(
                IRQ_PI_ERROR,
                SAMPLE_HANDLER,
                core::ptr::null_mut()
            )),
            0
        );
        assert_eq!(
            handler_slot(k_irq_get(IRQ_PI_ERROR)),
            handler_slot(SAMPLE_HANDLER)
        );
        assert_eq!(
            handler_slot(k_irq_set(IRQ_PI_ERROR, None, core::ptr::null_mut())),
            handler_slot(SAMPLE_HANDLER)
        );
        assert_eq!(handler_slot(k_irq_get(IRQ_PI_ERROR)), 0);
    }

    #[test]
    fn invalid_irq_id_is_rejected_safely() {
        let invalid = IRQ_COUNT as u32;
        assert_eq!(handler_slot(k_irq_get(invalid)), 0);
        assert_eq!(
            handler_slot(k_irq_set(invalid, SAMPLE_HANDLER, core::ptr::null_mut())),
            0
        );
    }

    #[test]
    fn enable_and_disable_update_mask_bits() {
        IRQ_MASK.store(0, Ordering::Release);
        k_irq_enable(0b1010);
        assert_eq!(mask(), 0b1010);
        k_irq_disable(0b0010);
        assert_eq!(mask(), 0b1000);
    }

    #[test]
    fn acr_irq_mask_is_wii_only() {
        assert_eq!(
            platform_irq_mask_for_console(IM_PI_ACR_BIT, true),
            IM_PI_ACR_BIT
        );
        assert_eq!(platform_irq_mask_for_console(IM_PI_ACR_BIT, false), 0);
    }

    #[test]
    fn acr_irq_id_is_rejected_on_gamecube() {
        assert_eq!(index_for_console(IRQ_PI_ACR_VALUE, false), None);
        #[cfg(not(gamecube))]
        assert_eq!(
            index_for_console(IRQ_PI_ACR_VALUE, true),
            Some(IRQ_PI_ACR_VALUE as usize)
        );
        #[cfg(gamecube)]
        assert_eq!(index_for_console(IRQ_PI_ACR_VALUE, true), None);
        assert_eq!(
            index_for_console(IRQ_PI_CP, false),
            Some(IRQ_PI_CP as usize)
        );
    }

    #[test]
    fn user_pointer_is_stored() {
        let marker = 0x1234usize as *mut c_void;
        let _ = k_irq_set(IRQ_PI_DEBUG, SAMPLE_HANDLER, marker);
        assert_eq!(stored_user(IRQ_PI_DEBUG), Some(marker));
    }

    #[test]
    fn host_dispatch_without_pending_irq_returns_false() {
        assert!(!k_irq_dispatch_pending());
    }

    #[test]
    fn restoring_the_vector_allows_runtime_reinitialization() {
        let initialized = AtomicBool::new(true);
        reset_irq_runtime_initialized(&initialized);
        assert!(!initialized.load(Ordering::Acquire));
    }

    #[test]
    fn runtime_reinitialization_reapplies_enabled_irq_mask() {
        let mask = AtomicU32::new(0b1000);
        let mut applied = 0;
        apply_stored_irq_mask(&mask, |value| applied = value);
        assert_eq!(applied, 0b1000);
    }
}

#[cfg(test)]
mod msr_state_tests {
    use super::*;

    #[test]
    fn irq_disable_restore_preserves_the_saved_ee_state() {
        let other_bits = 0x1234_5678 & !MSR_EE;
        assert_eq!(msr_irq_saved_state(other_bits), 0);
        assert_eq!(msr_irq_lock_value(other_bits | MSR_EE), other_bits);
        assert_eq!(msr_irq_saved_state(other_bits | MSR_EE), MSR_EE);
        assert_eq!(
            msr_irq_unlock_value(other_bits, MSR_EE),
            other_bits | MSR_EE
        );
        assert_eq!(msr_irq_unlock_value(other_bits | MSR_EE, 0), other_bits);
    }
}
