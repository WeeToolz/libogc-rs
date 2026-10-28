#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::ffi::c_void;

pub use ogc_irq_sys as sys;
pub use sys::{
    IRQ_AI, IRQ_DSP_AI, IRQ_DSP_ARAM, IRQ_DSP_DSP, IRQ_EXI0_EXI, IRQ_EXI0_EXT, IRQ_EXI0_TC,
    IRQ_EXI1_EXI, IRQ_EXI1_EXT, IRQ_EXI1_TC, IRQ_EXI2_EXI, IRQ_EXI2_TC, IRQ_MEM0, IRQ_MEM1,
    IRQ_MEM2, IRQ_MEM3, IRQ_MEMADDRESS, IRQ_PI_CP, IRQ_PI_DEBUG, IRQ_PI_DI, IRQ_PI_ERROR,
    IRQ_PI_HSP, IRQ_PI_PEFINISH, IRQ_PI_PETOKEN, IRQ_PI_RSW, IRQ_PI_SI, IRQ_PI_VI, KIrqHandlerFn,
    KIrqId,
};

#[cfg(not(gamecube))]
pub use sys::IRQ_PI_ACR;
#[cfg(not(gamecube))]
pub const IM_PI_ACR: u32 = 1 << IRQ_PI_ACR;

/// # Safety
///
/// `user` must remain valid for as long as the handler may be invoked.
pub unsafe fn request(id: KIrqId, handler: KIrqHandlerFn, user: *mut c_void) -> KIrqHandlerFn {
    sys::k_irq_set(id, handler, user)
}

pub fn free(id: KIrqId) -> KIrqHandlerFn {
    sys::k_irq_set(id, None, core::ptr::null_mut())
}

pub fn get_handler(id: KIrqId) -> KIrqHandlerFn {
    sys::k_irq_get(id)
}

pub fn enable(mask: u32) {
    sys::k_irq_enable(mask)
}

pub fn disable(mask: u32) {
    sys::k_irq_disable(mask)
}

/// Disable external IRQ delivery and return the previous state.
///
/// Pass the returned value to [`irq_restore`] to restore the previous state.
pub fn irq_disable() -> u32 {
    sys::k_irq_disable_all()
}

/// Restore external IRQ delivery from a value returned by [`irq_disable`].
pub fn irq_restore(state: u32) {
    sys::k_irq_restore_all(state)
}

pub fn mask(mask: u32) {
    disable(mask);
}

pub fn unmask(mask: u32) {
    enable(mask);
}

pub fn clear(id: KIrqId) -> KIrqHandlerFn {
    free(id)
}

pub fn dispatch_pending() -> bool {
    sys::k_irq_dispatch_pending()
}

/// Restore the external interrupt vector that was present before the IRQ
/// runtime first installed its handler.
pub fn restore_exception_vector() {
    sys::k_irq_restore_exception_vector()
}

/// Restore every exception vector installed by the core Rust runtime.
///
/// Use this before transferring control to an application that owns its own
/// exception setup, matching libogc's `__exception_closeall` handoff role.
pub fn restore_runtime_exception_vectors() {
    sys::k_irq_restore_runtime_exception_vectors()
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    #[test]
    fn helper_functions_exist_without_backend() {
        let _ = request as unsafe fn(KIrqId, KIrqHandlerFn, *mut c_void) -> KIrqHandlerFn;
        let _ = free as fn(KIrqId) -> KIrqHandlerFn;
        let _ = get_handler as fn(KIrqId) -> KIrqHandlerFn;
        let _ = enable as fn(u32);
        let _ = disable as fn(u32);
        let _ = irq_disable as fn() -> u32;
        let _ = irq_restore as fn(u32);
        let _ = mask as fn(u32);
        let _ = unmask as fn(u32);
        let _ = clear as fn(KIrqId) -> KIrqHandlerFn;
        let _ = dispatch_pending as fn() -> bool;
        let _ = restore_exception_vector as fn();
        let _ = restore_runtime_exception_vectors as fn();
    }
}
