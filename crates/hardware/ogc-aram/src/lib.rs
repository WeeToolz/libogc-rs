#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::ffi::c_void;
use core::ptr::{read_volatile, write_volatile};

use ogc_cache::{dc_flush_range, dc_invalidate_range};
use ogc_irq as irq;
use ogc_irq::IRQ_DSP_ARAM;

pub use ogc_sys::Bool;

pub const AR_MRAMTOARAM: u32 = 0;
pub const AR_ARAMTOMRAM: u32 = 1;

pub const AR_ARAMINTALL: u32 = 0;
pub const AR_ARAMINTUSER: u32 = 1;
pub const AR_ARAMEXPANSION: u32 = 2;

const DSP_REG_BASE: usize = 0xCC00_5000;

const REG_DSP_CONTROL: usize = 5;
const REG_AR_MODE: usize = 9;
const REG_AR_REFRESH: usize = 11;
const REG_AR_INFO: usize = 13;

const REG_DMA_MMADDR_H: usize = 16;
const REG_DMA_MMADDR_L: usize = 17;
const REG_DMA_ARADDR_H: usize = 18;
const REG_DMA_ARADDR_L: usize = 19;
const REG_DMA_LEN_H_DIR: usize = 20;
const REG_DMA_LEN_L: usize = 21;

const DSPCR_DSPDMA: u32 = 0x0200;
const DSPCR_DSPINT: u32 = 0x0080;
const DSPCR_AIINT: u32 = 0x0008;
const DSPCR_ARINT: u32 = 0x0020;

const ARAM_BASE: u32 = 0x4000;
const INTERNAL_ARAM_SIZE: u32 = 0x0100_0000;
const ZERO_CHUNK_LEN: u32 = 2048;

pub type ArCallback = Option<unsafe extern "C" fn()>;

pub mod arq;

#[derive(Copy, Clone, Default)]
struct ArState {
    callback: ArCallback,
    init_flag: Bool,
    stack_pointer: u32,
    free_blocks: u32,
    block_len: *mut u32,
    internal_size: u32,
    expansion_size: u32,
    size: u32,
}

static mut AR_STATE: ArState = ArState {
    callback: None,
    init_flag: 0,
    stack_pointer: 0,
    free_blocks: 0,
    block_len: core::ptr::null_mut(),
    internal_size: 0,
    expansion_size: 0,
    size: 0,
};

#[repr(align(32))]
struct Words8([u32; 8]);

#[repr(align(32))]
struct ZeroBuf([u8; ZERO_CHUNK_LEN as usize]);

static mut TEST_DATA: Words8 = Words8([0; 8]);
static mut DUMMY_DATA: Words8 = Words8([0; 8]);
static mut BUFFER: Words8 = Words8([0; 8]);
static mut ZERO_BUFFER: ZeroBuf = ZeroBuf([0; ZERO_CHUNK_LEN as usize]);

#[inline]
fn state_ptr() -> *mut ArState {
    core::ptr::addr_of_mut!(AR_STATE)
}

#[inline]
fn test_data_ptr() -> *mut Words8 {
    core::ptr::addr_of_mut!(TEST_DATA)
}

#[inline]
fn dummy_data_ptr() -> *mut Words8 {
    core::ptr::addr_of_mut!(DUMMY_DATA)
}

#[inline]
fn buffer_ptr() -> *mut Words8 {
    core::ptr::addr_of_mut!(BUFFER)
}

#[inline]
fn zero_buffer_ptr() -> *mut ZeroBuf {
    core::ptr::addr_of_mut!(ZERO_BUFFER)
}

#[inline]
unsafe fn words8_data_ptr(ptr: *mut Words8) -> *mut c_void {
    unsafe { core::ptr::addr_of_mut!((*ptr).0).cast::<c_void>() }
}

#[inline]
unsafe fn words8_words_ptr(ptr: *mut Words8) -> *mut u32 {
    unsafe { core::ptr::addr_of_mut!((*ptr).0).cast::<u32>() }
}

#[inline]
unsafe fn words8_fill(ptr: *mut Words8, value: u32) {
    let words = unsafe { words8_words_ptr(ptr) };

    for i in 0..8usize {
        unsafe {
            words.add(i).write(value);
        }
    }
}

#[inline]
unsafe fn words8_first(ptr: *mut Words8) -> u32 {
    unsafe { words8_words_ptr(ptr).read() }
}

#[inline]
unsafe fn zero_buffer_data_ptr() -> *mut c_void {
    unsafe { core::ptr::addr_of_mut!((*zero_buffer_ptr()).0).cast::<c_void>() }
}

#[inline]
unsafe fn zero_buffer_fill_zero() {
    unsafe {
        core::ptr::write_bytes(
            core::ptr::addr_of_mut!((*zero_buffer_ptr()).0).cast::<u8>(),
            0,
            ZERO_CHUNK_LEN as usize,
        );
    }
}

#[cfg(target_arch = "powerpc")]
unsafe fn ppc_mfmsr() -> u32 {
    let value: u32;
    unsafe {
        core::arch::asm!("mfmsr {0}", out(reg) value, options(nostack));
    }
    value
}

#[cfg(target_arch = "powerpc")]
unsafe fn ppc_mtmsr(value: u32) {
    unsafe {
        core::arch::asm!("mtmsr {0}", in(reg) value, options(nostack));
    }
}

#[cfg(target_arch = "powerpc")]
unsafe fn lock_by_msr() -> u32 {
    let msr = unsafe { ppc_mfmsr() };
    unsafe {
        ppc_mtmsr(msr & !(1 << 15));
    }
    msr & (1 << 15)
}

#[cfg(not(target_arch = "powerpc"))]
unsafe fn lock_by_msr() -> u32 {
    0
}

#[cfg(target_arch = "powerpc")]
unsafe fn unlock_by_msr(state: u32) {
    let msr = unsafe { ppc_mfmsr() } & !(1 << 15);
    unsafe {
        ppc_mtmsr(msr | state);
    }
}

#[cfg(not(target_arch = "powerpc"))]
unsafe fn unlock_by_msr(_state: u32) {}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
unsafe fn ppc_nop() {
    unsafe {
        core::arch::asm!("nop", options(nostack, nomem));
    }
}

#[cfg(not(target_arch = "powerpc"))]
#[inline(always)]
unsafe fn ppc_nop() {}

#[inline]
fn reg(index: usize) -> *mut u16 {
    (DSP_REG_BASE as *mut u16).wrapping_add(index)
}

#[inline]
unsafe fn read(index: usize) -> u32 {
    unsafe { u32::from(read_volatile(reg(index))) }
}

#[inline]
unsafe fn write(index: usize, value: u32) {
    unsafe {
        write_volatile(reg(index), value as u16);
    }
}

fn with_state<T>(f: impl FnOnce(&mut ArState) -> T) -> T {
    unsafe {
        let level = lock_by_msr();
        let out = f(&mut *state_ptr());
        unlock_by_msr(level);
        out
    }
}

fn clear_interrupt() {
    let cause = unsafe { read(REG_DSP_CONTROL) } & !(DSPCR_DSPINT | DSPCR_AIINT);

    unsafe {
        write(REG_DSP_CONTROL, cause | DSPCR_ARINT);
    }
}

fn wait_dma() {
    while unsafe { read(REG_DSP_CONTROL) } & DSPCR_DSPDMA != 0 {
        core::hint::spin_loop();
    }
}

#[inline]
unsafe fn program_dma_registers(dir: u32, memaddr: u32, aramaddr: u32, len: u32) {
    let reg16 = unsafe { read(REG_DMA_MMADDR_H) };
    unsafe {
        write(REG_DMA_MMADDR_H, (reg16 & !0x03ff) | (memaddr >> 16));
    }

    let reg17 = unsafe { read(REG_DMA_MMADDR_L) };
    unsafe {
        write(REG_DMA_MMADDR_L, (reg17 & !0xffe0) | (memaddr & 0xffff));
    }

    let reg18 = unsafe { read(REG_DMA_ARADDR_H) };
    unsafe {
        write(REG_DMA_ARADDR_H, (reg18 & !0x03ff) | (aramaddr >> 16));
    }

    let reg19 = unsafe { read(REG_DMA_ARADDR_L) };
    unsafe {
        write(REG_DMA_ARADDR_L, (reg19 & !0xffe0) | (aramaddr & 0xffff));
    }

    let reg20 = unsafe { read(REG_DMA_LEN_H_DIR) };
    unsafe {
        write(REG_DMA_LEN_H_DIR, (reg20 & !0x8000) | (dir << 15));
    }

    let reg20 = unsafe { read(REG_DMA_LEN_H_DIR) };
    unsafe {
        write(REG_DMA_LEN_H_DIR, (reg20 & !0x03ff) | (len >> 16));
    }

    let reg21 = unsafe { read(REG_DMA_LEN_L) };
    unsafe {
        write(REG_DMA_LEN_L, (reg21 & !0xffe0) | (len & 0xffff));
    }
}

fn read_dma(memaddr: u32, aramaddr: u32, len: u32) {
    unsafe {
        program_dma_registers(AR_ARAMTOMRAM, memaddr, aramaddr, len);
    }

    wait_dma();
    clear_interrupt();
}

fn write_dma(memaddr: u32, aramaddr: u32, len: u32) {
    unsafe {
        program_dma_registers(AR_MRAMTOARAM, memaddr, aramaddr, len);
    }

    wait_dma();
    clear_interrupt();
}

fn clear_area(aramaddr: u32, len: u32) {
    while unsafe { read(REG_AR_REFRESH) } & 0x0001 == 0 {
        core::hint::spin_loop();
    }

    unsafe {
        zero_buffer_fill_zero();
        dc_flush_range(zero_buffer_data_ptr(), 2048);
    }

    let mut curraddr = aramaddr;
    let endaddr = aramaddr.wrapping_add(len);
    let mut currlen = ZERO_CHUNK_LEN;

    while curraddr < endaddr {
        let remaining = endaddr - curraddr;

        if remaining < currlen {
            currlen = (remaining + 31) & !31;
        }

        write_dma(unsafe { zero_buffer_data_ptr() as u32 }, curraddr, currlen);

        curraddr = curraddr.wrapping_add(currlen);
    }
}

unsafe fn prepare_buffer_for_read() {
    unsafe {
        words8_fill(buffer_ptr(), 0);
        dc_flush_range(words8_data_ptr(buffer_ptr()), 32);
    }
}

unsafe fn read_buffer_word_from_aram(aramaddr: u32) -> u32 {
    unsafe {
        prepare_buffer_for_read();

        read_dma(words8_data_ptr(buffer_ptr()) as u32, aramaddr, 32);

        ppc_nop();

        words8_first(buffer_ptr())
    }
}

fn check_size() {
    while unsafe { read(REG_AR_REFRESH) } & 0x0001 == 0 {
        core::hint::spin_loop();
    }

    let mut arsize = INTERNAL_ARAM_SIZE;
    let mut arszflag = 0x03;
    let state = unsafe { &mut *state_ptr() };

    state.size = INTERNAL_ARAM_SIZE;
    state.internal_size = INTERNAL_ARAM_SIZE;
    state.expansion_size = 0;

    unsafe {
        write(REG_AR_MODE, (read(REG_AR_MODE) & !0x3f) | 0x23);

        words8_fill(test_data_ptr(), 0xBAD1_BAD0);
        words8_fill(dummy_data_ptr(), 0xDEAD_BEEF);

        dc_flush_range(words8_data_ptr(test_data_ptr()), 32);
        dc_flush_range(words8_data_ptr(dummy_data_ptr()), 32);
    }

    unsafe {
        write_dma(words8_data_ptr(test_data_ptr()) as u32, 0x0100_0000, 32);

        write_dma(words8_data_ptr(test_data_ptr()) as u32, 0x0120_0000, 32);

        write_dma(words8_data_ptr(test_data_ptr()) as u32, 0x0200_0000, 32);

        write_dma(words8_data_ptr(test_data_ptr()) as u32, 0x0100_0200, 32);

        write_dma(words8_data_ptr(test_data_ptr()) as u32, 0x0140_0000, 32);

        words8_fill(buffer_ptr(), 0);
        dc_flush_range(words8_data_ptr(buffer_ptr()), 32);

        write_dma(words8_data_ptr(dummy_data_ptr()) as u32, 0x0100_0000, 32);
    }

    unsafe {
        dc_invalidate_range(words8_data_ptr(buffer_ptr()), 32);

        read_dma(words8_data_ptr(buffer_ptr()) as u32, 0x0100_0000, 32);

        ppc_nop();
    }

    let dummy_word = unsafe { words8_first(dummy_data_ptr()) };
    let buffer_word = unsafe { words8_first(buffer_ptr()) };

    if buffer_word == dummy_word {
        let buffer_word = unsafe { read_buffer_word_from_aram(0x0120_0000) };

        if buffer_word == dummy_word {
            state.expansion_size = 0x0020_0000;
            arsize += 0x0020_0000;

            unsafe {
                write(REG_AR_MODE, (read(REG_AR_MODE) & !0x3f) | arszflag);
            }
        } else {
            let buffer_word = unsafe { read_buffer_word_from_aram(0x0200_0000) };

            if buffer_word == dummy_word {
                state.expansion_size = 0x0040_0000;
                arsize += 0x0040_0000;
                arszflag |= 0x08;
            } else {
                let buffer_word = unsafe { read_buffer_word_from_aram(0x0140_0000) };

                if buffer_word == dummy_word {
                    state.expansion_size = 0x0100_0000;
                    arsize += 0x0100_0000;
                    arszflag |= 0x18;
                } else {
                    state.expansion_size = 0x0200_0000;
                    arsize += 0x0200_0000;
                    arszflag |= 0x20;
                }
            }

            unsafe {
                write(REG_AR_MODE, (read(REG_AR_MODE) & !0x3f) | arszflag);
            }
        }
    }

    unsafe {
        write_volatile(0x8000_00d0 as *mut u32, arsize);
    }

    state.size = arsize;
}

unsafe extern "C" fn handler(_irq: u32, _ctx: *mut c_void) {
    let callback = with_state(|state| {
        clear_interrupt();
        state.callback
    });

    if let Some(cb) = callback {
        unsafe {
            cb();
        }
    }
}

fn is_aram_handler(candidate: irq::KIrqHandlerFn) -> bool {
    candidate.is_some_and(|candidate| candidate as *const () == handler as *const ())
}

pub fn register_callback(callback: ArCallback) -> ArCallback {
    with_state(|state| {
        let old = state.callback;
        state.callback = callback;
        old
    })
}

pub fn get_dma_status() -> u32 {
    unsafe {
        let level = lock_by_msr();
        let ret = ((read(REG_DSP_CONTROL) & DSPCR_DSPDMA) == DSPCR_DSPDMA) as u32;
        unlock_by_msr(level);
        ret
    }
}

pub fn init(stack_idx_array: *mut u32, num_entries: u32) -> u32 {
    unsafe {
        let level = lock_by_msr();
        let state = &mut *state_ptr();

        if state.init_flag != 0 {
            unlock_by_msr(level);
            return ARAM_BASE;
        }

        state.callback = None;

        let installed_handler = irq::get_handler(IRQ_DSP_ARAM);
        if installed_handler.is_some() && !is_aram_handler(installed_handler) {
            unlock_by_msr(level);
            return 0;
        }
        let previous_handler = irq::request(IRQ_DSP_ARAM, Some(handler), core::ptr::null_mut());
        if previous_handler.is_some() && !is_aram_handler(previous_handler) {
            // Interrupts are disabled above, so this can only occur if the IRQ
            // backend changed independently. Restore its handler and leave ARAM
            // uninitialized instead of taking ownership from another driver.
            irq::request(IRQ_DSP_ARAM, previous_handler, core::ptr::null_mut());
            unlock_by_msr(level);
            return 0;
        }
        irq::unmask(1 << IRQ_DSP_ARAM);

        state.stack_pointer = ARAM_BASE;
        state.free_blocks = num_entries;
        state.block_len = stack_idx_array;

        let reg13 = read(REG_AR_INFO);
        write(REG_AR_INFO, (reg13 & !0xff) | (reg13 & 0xff));

        check_size();

        state.init_flag = 1;

        let ret = state.stack_pointer;
        unlock_by_msr(level);
        ret
    }
}

pub fn start_dma(dir: u32, memaddr: u32, aramaddr: u32, len: u32) {
    unsafe {
        let level = lock_by_msr();
        program_dma_registers(dir, memaddr, aramaddr, len);
        unlock_by_msr(level);
    }
}

pub fn start_dma_read(memaddr: u32, aramaddr: u32, len: u32) {
    start_dma(AR_ARAMTOMRAM, memaddr, aramaddr, len);
}

pub fn start_dma_write(memaddr: u32, aramaddr: u32, len: u32) {
    start_dma(AR_MRAMTOARAM, memaddr, aramaddr, len);
}

pub fn alloc(len: u32) -> u32 {
    with_state(|state| {
        let curraddr = state.stack_pointer;

        state.stack_pointer = state.stack_pointer.wrapping_add(len);

        unsafe {
            *state.block_len = len;
            state.block_len = state.block_len.add(1);
        }

        state.free_blocks = state.free_blocks.wrapping_sub(1);

        curraddr
    })
}

pub fn free(len: Option<&mut u32>) -> u32 {
    with_state(|state| {
        unsafe {
            state.block_len = state.block_len.sub(1);

            if let Some(out) = len {
                *out = *state.block_len;
            }

            state.stack_pointer = state.stack_pointer.wrapping_sub(*state.block_len);
        }

        state.free_blocks = state.free_blocks.wrapping_add(1);

        state.stack_pointer
    })
}

pub fn clear(flag: u32) {
    unsafe {
        let state = &mut *state_ptr();

        match flag {
            AR_ARAMINTALL if state.internal_size != 0 => {
                clear_area(0, state.internal_size);
            }

            AR_ARAMINTUSER if state.internal_size != 0 => {
                clear_area(ARAM_BASE, state.internal_size - ARAM_BASE);
            }

            AR_ARAMEXPANSION if state.internal_size != 0 && state.expansion_size != 0 => {
                clear_area(state.internal_size, state.expansion_size);
            }

            _ => {}
        }
    }
}

pub fn check_init() -> Bool {
    unsafe {
        let level = lock_by_msr();
        let ret = (*state_ptr()).init_flag;
        unlock_by_msr(level);
        ret
    }
}

pub fn reset() {
    unsafe {
        let level = lock_by_msr();
        (*state_ptr()).init_flag = 0;
        unlock_by_msr(level);
    }
}

pub fn get_size() -> u32 {
    unsafe {
        let level = lock_by_msr();
        let ret = (*state_ptr()).size;
        unlock_by_msr(level);
        ret
    }
}

pub fn get_base_address() -> u32 {
    ARAM_BASE
}

pub fn get_internal_size() -> u32 {
    unsafe {
        let level = lock_by_msr();
        let ret = (*state_ptr()).internal_size;
        unlock_by_msr(level);
        ret
    }
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    #[test]
    fn constants_match_header() {
        assert_eq!(AR_MRAMTOARAM, 0);
        assert_eq!(AR_ARAMTOMRAM, 1);
        assert_eq!(AR_ARAMINTALL, 0);
        assert_eq!(AR_ARAMINTUSER, 1);
        assert_eq!(AR_ARAMEXPANSION, 2);

        let _ = start_dma_read as fn(u32, u32, u32);
        let _ = start_dma_write as fn(u32, u32, u32);
    }

    #[test]
    fn callback_uses_c_abi() {
        unsafe extern "C" fn callback() {}

        let _callback: ArCallback = Some(callback);
    }

    #[test]
    fn init_accepts_its_own_irq_handler_but_rejects_other_owners() {
        unsafe extern "C" fn other_handler(_irq: u32, _ctx: *mut c_void) {}

        assert!(!is_aram_handler(None));
        assert!(is_aram_handler(Some(handler)));
        assert!(!is_aram_handler(Some(other_handler)));
    }
}
