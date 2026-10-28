#![no_std]

#[cfg(not(target_arch = "powerpc"))]
compile_error!(
    "This crate targets Wii/GameCube PowerPC only. Use the powerpc-nintendo-none target."
);

use core::ffi::c_void;

#[cfg(target_arch = "powerpc")]
#[cfg(not(gamecube))]
use core::ptr::{read_volatile, write_volatile};

pub const LC_BASEPREFIX: u32 = 0xe000;
pub const LC_BASE: usize = (LC_BASEPREFIX as usize) << 16;

const CACHE_LINE_SIZE: usize = 32;
const MSR_EE: u32 = 1 << 15;
const MSR_FP: u32 = 1 << 13;
#[cfg(not(gamecube))]
const L2_FLUSH_RANGES: [(usize, u32); 2] = [(0x8000_0000, 0x0180_0000), (0x9000_0000, 0x0400_0000)];

macro_rules! cache_range_op {
    ($start:expr_2021, $len:expr_2021, $mnemonic:literal) => {{
        let mut addr = $start as usize;
        let count = line_count($start, $len);

        for _ in 0..count {
            #[cfg(target_arch = "powerpc")]
            unsafe {
                core::arch::asm!(concat!($mnemonic, " 0, {addr}"), addr = in(reg) addr, options(nostack));
            }

            addr = addr.wrapping_add(CACHE_LINE_SIZE);
        }
    }};
}

#[inline(always)]
fn ppc_sync() {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        core::arch::asm!("sync", options(nostack));
    }
}

#[inline(always)]
fn ppc_isync() {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        core::arch::asm!("isync", options(nostack));
    }
}

#[cfg(all(target_arch = "powerpc", not(gamecube)))]
#[inline(always)]
fn ppc_eieio() {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        core::arch::asm!("eieio", options(nostack));
    }
}

#[cfg(target_arch = "powerpc")]
#[inline(always)]
fn ppc_nop() {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        core::arch::asm!("nop", options(nostack));
    }
}

#[inline(always)]
fn ppc_mfmsr() -> u32 {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        let mut value: u32 = 0;
        core::arch::asm!("mfmsr {value}", value = out(reg) value, options(nostack));
        value
    }

    #[cfg(not(target_arch = "powerpc"))]
    {
        0
    }
}

#[inline(always)]
fn ppc_mtmsr(value: u32) {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        core::arch::asm!("mtmsr {value}", value = in(reg) value, options(nostack));
    }

    #[cfg(not(target_arch = "powerpc"))]
    {
        let _ = value;
    }
}

#[inline(always)]
fn ppc_mfspr_hid0() -> u32 {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        let mut value: u32 = 0;
        core::arch::asm!("mfspr {value}, 1008", value = out(reg) value, options(nostack));
        value
    }

    #[cfg(not(target_arch = "powerpc"))]
    {
        0
    }
}

#[inline(always)]
fn ppc_mfspr_hid2() -> u32 {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        let mut value: u32 = 0;
        core::arch::asm!("mfspr {value}, 920", value = out(reg) value, options(nostack));
        value
    }

    #[cfg(not(target_arch = "powerpc"))]
    {
        0
    }
}

#[cfg(all(target_arch = "powerpc", not(gamecube)))]
#[inline(always)]
fn ppc_mfspr_hid4() -> u32 {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        let mut value: u32 = 0;
        core::arch::asm!("mfspr {value}, 1011", value = out(reg) value, options(nostack));
        value
    }

    #[cfg(not(target_arch = "powerpc"))]
    {
        0
    }
}

#[cfg(all(target_arch = "powerpc", not(gamecube)))]
#[inline(always)]
fn ppc_mfspr_l2cr() -> u32 {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        let mut value: u32 = 0;
        core::arch::asm!("mfspr {value}, 1017", value = out(reg) value, options(nostack));
        value
    }

    #[cfg(not(target_arch = "powerpc"))]
    {
        0
    }
}

#[inline(always)]
fn ppc_mtspr_hid0(value: u32) {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        core::arch::asm!("mtspr 1008, {value}", value = in(reg) value, options(nostack));
    }

    #[cfg(not(target_arch = "powerpc"))]
    {
        let _ = value;
    }
}

#[inline(always)]
fn ppc_mtspr_hid2(value: u32) {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        core::arch::asm!("mtspr 920, {value}", value = in(reg) value, options(nostack));
    }

    #[cfg(not(target_arch = "powerpc"))]
    {
        let _ = value;
    }
}

#[cfg(all(target_arch = "powerpc", not(gamecube)))]
#[inline(always)]
fn ppc_mtspr_hid4(value: u32) {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        core::arch::asm!("mtspr 1011, {value}", value = in(reg) value, options(nostack));
    }

    #[cfg(not(target_arch = "powerpc"))]
    {
        let _ = value;
    }
}

#[cfg(all(target_arch = "powerpc", not(gamecube)))]
#[inline(always)]
fn ppc_mtspr_l2cr(value: u32) {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        core::arch::asm!("mtspr 1017, {value}", value = in(reg) value, options(nostack));
    }

    #[cfg(not(target_arch = "powerpc"))]
    {
        let _ = value;
    }
}

#[inline(always)]
fn ppc_mtspr_dmau(value: u32) {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        core::arch::asm!("mtspr 922, {value}", value = in(reg) value, options(nostack));
    }

    #[cfg(not(target_arch = "powerpc"))]
    {
        let _ = value;
    }
}

#[inline(always)]
fn ppc_mtspr_dmal(value: u32) {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        core::arch::asm!("mtspr 923, {value}", value = in(reg) value, options(nostack));
    }

    #[cfg(not(target_arch = "powerpc"))]
    {
        let _ = value;
    }
}

#[inline(always)]
fn lock_by_msr() -> u32 {
    let msr = ppc_mfmsr();
    ppc_mtmsr(msr & !(MSR_EE | MSR_FP));
    ogc_sys::msr_lock_state(msr)
}

#[inline(always)]
fn unlock_by_msr(state: u32) {
    ppc_mtmsr(ogc_sys::msr_unlock_value(ppc_mfmsr(), state));
}

#[inline(always)]
fn line_count(start: *const c_void, len: u32) -> usize {
    let start = start as usize;
    let aligned = start & !(CACHE_LINE_SIZE - 1);
    let end = start
        .wrapping_add(len as usize)
        .wrapping_add(CACHE_LINE_SIZE - 1);
    (end - aligned) >> 5
}

#[cfg(all(target_arch = "powerpc", not(gamecube)))]
fn write_u32_uncached(addr: u32, value: u32) {
    let ptr = (0xc0000000u32 | addr) as *mut u32;
    unsafe {
        write_volatile(ptr, value);
    }
    ppc_eieio();
}

pub fn dc_enable() {
    let mut hid0 = ppc_mfspr_hid0();
    hid0 |= 0x4000;
    ppc_mtspr_hid0(hid0);
    ppc_isync();
}

pub fn dc_disable() {
    let mut hid0 = ppc_mfspr_hid0();
    hid0 &= !0x4000;
    ppc_mtspr_hid0(hid0);
    ppc_isync();
}

pub fn dc_freeze() {
    let mut hid0 = ppc_mfspr_hid0();
    hid0 |= 0x1000;
    ppc_mtspr_hid0(hid0);
    ppc_isync();
}

pub fn dc_unfreeze() {
    let mut hid0 = ppc_mfspr_hid0();
    hid0 &= !0x1000;
    ppc_mtspr_hid0(hid0);
    ppc_isync();
}

pub fn dc_flash_invalidate() {
    let mut hid0 = ppc_mfspr_hid0();
    hid0 |= 0x0400;
    ppc_mtspr_hid0(hid0);
    ppc_isync();
}

pub fn sync() {
    ppc_sync();
}

/// # Safety
///
/// `startaddress` and `len` must match the cache-line alignment rules in the
/// libogc header.
pub unsafe fn dc_invalidate_range(startaddress: *const c_void, len: u32) {
    cache_range_op!(startaddress, len, "dcbi");
}

/// # Safety
///
/// `startaddress` and `len` must match the cache-line alignment rules in the
/// libogc header.
pub unsafe fn dc_flush_range(startaddress: *const c_void, len: u32) {
    cache_range_op!(startaddress, len, "dcbf");
    sync();
}

/// # Safety
///
/// `startaddress` and `len` must match the cache-line alignment rules in the
/// libogc header.
pub unsafe fn dc_store_range(startaddress: *const c_void, len: u32) {
    cache_range_op!(startaddress, len, "dcbst");
    sync();
}

/// # Safety
///
/// `startaddress` and `len` must match the cache-line alignment rules in the
/// libogc header.
pub unsafe fn dc_flush_range_no_sync(startaddress: *const c_void, len: u32) {
    cache_range_op!(startaddress, len, "dcbf");
}

/// # Safety
///
/// `startaddress` and `len` must match the cache-line alignment rules in the
/// libogc header.
pub unsafe fn dc_store_range_no_sync(startaddress: *const c_void, len: u32) {
    cache_range_op!(startaddress, len, "dcbst");
}

/// # Safety
///
/// `startaddress` and `len` must match the cache-line alignment rules in the
/// libogc header.
pub unsafe fn dc_zero_range(startaddress: *const c_void, len: u32) {
    cache_range_op!(startaddress, len, "dcbz");
}

/// # Safety
///
/// `startaddress` and `len` must match the cache-line alignment rules in the
/// libogc header.
pub unsafe fn dc_touch_range(startaddress: *const c_void, len: u32) {
    cache_range_op!(startaddress, len, "dcbt");
}

pub fn ic_sync() {
    ppc_isync();
}

pub fn ic_flash_invalidate() {
    let mut hid0 = ppc_mfspr_hid0();
    hid0 |= 0x0800;
    ppc_mtspr_hid0(hid0);
    ppc_isync();
}

pub fn ic_enable() {
    let mut hid0 = ppc_mfspr_hid0();
    hid0 |= 0x8000;
    ppc_mtspr_hid0(hid0);
    ppc_isync();
}

pub fn ic_disable() {
    let mut hid0 = ppc_mfspr_hid0();
    hid0 &= !0x8000;
    ppc_mtspr_hid0(hid0);
    ppc_isync();
}

pub fn ic_freeze() {
    let mut hid0 = ppc_mfspr_hid0();
    hid0 |= 0x2000;
    ppc_mtspr_hid0(hid0);
    ppc_isync();
}

pub fn ic_unfreeze() {
    let mut hid0 = ppc_mfspr_hid0();
    hid0 &= !0x2000;
    ppc_mtspr_hid0(hid0);
    ppc_isync();
}

/// # Safety
///
/// `startaddress` and `len` must match the cache-line alignment rules in the
/// libogc header.
pub unsafe fn ic_block_invalidate(_startaddress: *const c_void) {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        core::arch::asm!("icbi 0, {addr}", addr = in(reg) _startaddress as usize, options(nostack));
    }
}

/// # Safety
///
/// `startaddress` and `len` must match the cache-line alignment rules in the
/// libogc header.
pub unsafe fn ic_invalidate_range(startaddress: *const c_void, len: u32) {
    cache_range_op!(startaddress, len, "icbi");
}

pub fn lc_enable() {
    let state = lock_by_msr();
    lc_enable_inner();
    unlock_by_msr(state);
}

pub fn lc_disable() {
    cache_range_op!(0x_e000_0000 as *const c_void, 512 * 32, "dcbi");
    let mut hid2 = ppc_mfspr_hid2();
    hid2 &= !(0x10000000);
    ppc_mtspr_hid2(hid2);
}

/// # Safety
///
/// `dst`/`src` must be valid for `count` cache lines and obey cache-locking
/// rules.
pub unsafe fn lc_load_blocks(_dst: *mut c_void, _src: *mut c_void, _count: u32) {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        let dst = _dst as usize;
        let src = _src as usize;
        let count = _count as usize;
        core::arch::asm!(
            "extrwi {tmp}, {count}, 5, 25",
            "clrlwi {src}, {src}, 4",
            "or {tmp}, {tmp}, {src}",
            "mtspr 922, {tmp}",
            "clrlslwi {tmp}, {count}, 30, 2",
            "or {tmp}, {tmp}, {dst}",
            "ori {tmp}, {tmp}, 0x0012",
            "mtspr 923, {tmp}",
            tmp = lateout(reg) _,
            src = inout(reg) src => _,
            count = in(reg) count,
            dst = in(reg) dst,
            options(nostack),
        );
    }
}

/// # Safety
///
/// `dst`/`src` must be valid for `count` cache lines and obey cache-locking
/// rules.
pub unsafe fn lc_store_blocks(_dst: *mut c_void, _src: *mut c_void, _count: u32) {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        let dst = _dst as usize;
        let src = _src as usize;
        let count = _count as usize;
        core::arch::asm!(
            "extrwi {tmp}, {count}, 5, 25",
            "clrlwi {src}, {src}, 4",
            "or {tmp}, {tmp}, {dst}",
            "mtspr 922, {tmp}",
            "clrlslwi {tmp}, {count}, 30, 2",
            "or {tmp}, {tmp}, {src}",
            "ori {tmp}, {tmp}, 0x0002",
            "mtspr 923, {tmp}",
            tmp = lateout(reg) _,
            src = inout(reg) src => _,
            count = in(reg) count,
            dst = in(reg) dst,
            options(nostack),
        );
    }
}

/// # Safety
///
/// `dst`/`src` must be valid for the requested transfer.
pub unsafe fn lc_load_data(dst: *mut c_void, src: *mut c_void, count: u32) -> u32 {
    unsafe {
        if (count as i32) <= 0 {
            return 0;
        }

        let mut cnt = (count + 31) >> 5;
        let blocks = (cnt + 127) >> 7;
        let mut dst = dst;
        let mut src = src;

        while cnt != 0 {
            if cnt < 0x80 {
                lc_load_blocks(dst, src, cnt);
                break;
            }

            lc_load_blocks(dst, src, 0);
            cnt -= 128;
            dst = (dst as usize + 4096) as *mut c_void;
            src = (src as usize + 4096) as *mut c_void;
        }

        blocks
    }
}

/// # Safety
///
/// `dst`/`src` must be valid for the requested transfer.
pub unsafe fn lc_store_data(dst: *mut c_void, src: *mut c_void, count: u32) -> u32 {
    unsafe {
        if (count as i32) <= 0 {
            return 0;
        }

        let mut cnt = (count + 31) >> 5;
        let blocks = (cnt + 127) >> 7;
        let mut dst = dst;
        let mut src = src;

        while cnt != 0 {
            if cnt < 0x80 {
                lc_store_blocks(dst, src, cnt);
                break;
            }

            lc_store_blocks(dst, src, 0);
            cnt -= 128;
            dst = (dst as usize + 4096) as *mut c_void;
            src = (src as usize + 4096) as *mut c_void;
        }

        blocks
    }
}

pub fn lc_queue_length() -> u32 {
    (ppc_mfspr_hid2() >> 4) & 0x0f
}

pub fn lc_queue_wait(len: u32) -> u32 {
    let len = len.wrapping_add(1);

    while ((ppc_mfspr_hid2() >> 4) & 0x0f) >= len {}

    len
}

pub fn lc_flush_queue() {
    ppc_mtspr_dmau(0);
    ppc_mtspr_dmal(1);
    ppc_sync();
}

/// # Safety
///
/// `addr` must be aligned and valid for the allocated cache blocks.
pub unsafe fn lc_alloc(addr: *mut c_void, bytes: u32) {
    let cnt = bytes >> 5;
    if (ppc_mfspr_hid2() & 0x10000000) == 0 {
        let state = lock_by_msr();
        lc_enable_inner();
        unlock_by_msr(state);
    }

    unsafe {
        lc_alloc_tags(true, addr, cnt);
    }
}

/// # Safety
///
/// `addr` must be aligned and valid for the allocated cache blocks.
pub unsafe fn lc_alloc_no_invalidate(addr: *mut c_void, bytes: u32) {
    let cnt = bytes >> 5;
    if (ppc_mfspr_hid2() & 0x10000000) == 0 {
        let state = lock_by_msr();
        lc_enable_inner();
        unlock_by_msr(state);
    }

    unsafe {
        lc_alloc_tags(false, addr, cnt);
    }
}

/// # Safety
///
/// `addr` must be aligned and valid for the allocated cache blocks.
pub unsafe fn lc_alloc_one_tag(_invalidate: bool, _addr: *mut c_void) {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        if _invalidate {
            core::arch::asm!("dcbi 0, {addr}", addr = in(reg) _addr as usize, options(nostack));
        }
        core::arch::asm!("dcbzl 0, {addr}", addr = in(reg) _addr as usize, options(nostack));
    }
}

/// # Safety
///
/// `addr` must be aligned and valid for the allocated cache blocks.
pub unsafe fn lc_alloc_tags(_invalidate: bool, _addr: *mut c_void, _count: u32) {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        let mut addr = _addr as usize;
        if _count == 0 {
            return;
        }

        if _invalidate {
            for _ in 0.._count {
                core::arch::asm!("dcbi 0, {addr}", addr = in(reg) addr, options(nostack));
                core::arch::asm!("dcbzl 0, {addr}", addr = in(reg) addr, options(nostack));
                addr = addr.wrapping_add(CACHE_LINE_SIZE);
            }
        } else {
            for _ in 0.._count {
                core::arch::asm!("dcbzl 0, {addr}", addr = in(reg) addr, options(nostack));
                addr = addr.wrapping_add(CACHE_LINE_SIZE);
            }
        }
    }
}

#[cfg(not(gamecube))]
pub fn l2_enhance() {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        let state = lock_by_msr();
        let hid4 = ppc_mfspr_hid4();

        if (hid4 & 0x8000_0000) != 0 {
            for (start, len) in L2_FLUSH_RANGES {
                dc_flush_range_no_sync(start as *const c_void, len);
            }

            l2_global_invalidate();

            let mut hid4 = ppc_mfspr_hid4();
            hid4 |= 0x2420_0000;
            ppc_mtspr_hid4(hid4);
            l2_enable_inner();

            let stub = 0x8000_1800 as *mut u32;
            if read_volatile(stub.add(1)) == 0x5354_5542
                && read_volatile(stub.add(2)) == 0x4841_5858
            {
                let mut scan = stub.add(3);
                while (scan as usize) < 0x8000_3000 {
                    let word = read_volatile(scan);
                    if (word & 0xFC1F_FFFF) == 0x7C13_FBA6 {
                        write_u32_uncached(scan as u32, 0x6000_0000);
                        break;
                    }
                    scan = scan.add(1);
                }
            }
        }

        unlock_by_msr(state);
    }
}

#[cfg(not(gamecube))]
#[inline(always)]
fn l2_enable_inner() {
    ppc_sync();
    let mut l2cr = ppc_mfspr_l2cr();
    l2cr |= 0x8000_0000;
    l2cr &= !0x0000_0e00;
    ppc_mtspr_l2cr(l2cr);
    ppc_sync();
}

#[cfg(not(gamecube))]
#[inline(always)]
fn l2_global_invalidate() {
    l2_disable_inner();
    let mut l2cr = ppc_mfspr_l2cr();
    l2cr |= 0x0020_0000;
    ppc_mtspr_l2cr(l2cr);
    while (ppc_mfspr_l2cr() & 0x8000_0000) != 0 {}
    let mut l2cr = ppc_mfspr_l2cr();
    l2cr &= !0x0000_0e00;
    ppc_mtspr_l2cr(l2cr);
    while (ppc_mfspr_l2cr() & 0x8000_0000) != 0 {}
}

#[cfg(not(gamecube))]
#[inline(always)]
fn l2_disable_inner() {
    ppc_sync();
    let mut l2cr = ppc_mfspr_l2cr();
    l2cr &= !0x8000_0000;
    ppc_mtspr_l2cr(l2cr);
    ppc_sync();
}

fn lc_enable_inner() {
    #[cfg(target_arch = "powerpc")]
    unsafe {
        let mut msr = ppc_mfmsr();
        msr |= 0x1000;
        ppc_mtmsr(msr);
        ppc_sync();

        let mut addr = 0x8000_0000usize;
        for _ in 0..1024 {
            core::arch::asm!(
                "dcbt 0, {addr}",
                "dcbst 0, {addr}",
                addr = in(reg) addr,
                options(nostack),
            );
            addr = addr.wrapping_add(CACHE_LINE_SIZE);
        }

        let mut hid2 = ppc_mfspr_hid2();
        hid2 |= 0x100f_0000;
        ppc_mtspr_hid2(hid2);

        for _ in 0..12 {
            ppc_nop();
        }

        core::arch::asm!("mtspr 543, {value}", value = in(reg) 0xe000_0002usize, options(nostack));
        core::arch::asm!("mtspr 542, {value}", value = in(reg) 0xe000_01feusize, options(nostack));
        ppc_isync();

        let mut addr = 0xe000_0000usize;
        for _ in 0..512 {
            core::arch::asm!("dcbzl 0, {addr}", addr = in(reg) addr, options(nostack));
            addr = addr.wrapping_add(CACHE_LINE_SIZE);
        }

        for _ in 0..12 {
            ppc_nop();
        }
    }
}

pub const fn lc_get_base() -> *mut c_void {
    LC_BASE as *mut c_void
}

#[cfg(all(test, target_arch = "powerpc"))]
mod tests {
    use super::*;

    #[test]
    fn helper_functions_exist_without_backend() {
        let _ = dc_enable as fn();
        let _ = dc_disable as fn();
        let _ = ic_sync as fn();
        let _ = lc_enable as fn();
        let _ = lc_disable as fn();
        let _ = lc_queue_length as fn() -> u32;
        let _ = lc_queue_wait as fn(u32) -> u32;
        let _ = lc_get_base as fn() -> *mut c_void;
        #[cfg(not(gamecube))]
        let _ = l2_enhance as fn();
    }

    #[cfg(not(gamecube))]
    #[test]
    fn l2_enhance_flushes_both_c_memory_ranges() {
        assert_eq!(
            L2_FLUSH_RANGES,
            [(0x8000_0000, 0x0180_0000), (0x9000_0000, 0x0400_0000)]
        );
    }
}
