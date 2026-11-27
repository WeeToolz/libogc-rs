use core::ffi::c_void;

use ogc_system::{self as system};
use ogc_system_sys as raw;

use crate::probe_support::ProbeLogger;
use ogc_alloc::ArenaAllocator;

const FREE_BYTES_HEAP_SIZE: usize = 4096;

#[repr(align(32))]
struct FreeBytesHeap([u8; FREE_BYTES_HEAP_SIZE]);

static mut FREE_BYTES_HEAP: FreeBytesHeap = FreeBytesHeap([0; FREE_BYTES_HEAP_SIZE]);
static FREE_BYTES_ALLOCATOR: ArenaAllocator = ArenaAllocator::new();

pub fn run(logger: &mut ProbeLogger) {
    logger.section("ARENA PROBE");

    let original = system::snapshot_arenas();
    logger.note_hex32("arena1 lo", original.arena1_lo as usize as u32);
    logger.note_hex32("arena1 hi", original.arena1_hi as usize as u32);
    logger.note_hex32("arena2 lo", original.arena2_lo as usize as u32);
    logger.note_hex32("arena2 hi", original.arena2_hi as usize as u32);

    logger.record("arena1 bounds and size roundtrip", probe_arena1_bounds());
    logger.record("arena2 bounds and size roundtrip", probe_arena2_bounds());
    logger.record(
        "arena1 size matches bounds",
        system::get_arena1_size()
            == (original.arena1_hi as u32).wrapping_sub(original.arena1_lo as u32),
    );
    logger.record(
        "arena2 size matches bounds",
        system::get_arena2_size()
            == (original.arena2_hi as u32).wrapping_sub(original.arena2_lo as u32),
    );
    logger.record("arena1 low alloc math", test_arena1(original.arena1_lo));
    logger.record("arena2 low alloc math", test_arena2(original.arena2_lo));
    logger.record(
        "allocator free bytes track allocate and free",
        probe_free_bytes(),
    );

    // SAFETY: these are the arena bounds captured at probe entry, before the
    // probe's temporary allocations changed them.
    unsafe {
        system::set_arena1_lo(original.arena1_lo);
        system::set_arena2_lo(original.arena2_lo);
    }
    let restored = system::snapshot_arenas();
    logger.record(
        "arena bounds restored",
        restored.arena1_lo == original.arena1_lo && restored.arena2_lo == original.arena2_lo,
    );
    logger.record("raw arena1 bounds and allocation", probe_raw_arena1());
    logger.record(
        "raw and ABI arena2 bounds and allocation",
        probe_raw_arena2(),
    );
}

fn probe_free_bytes() -> bool {
    // SAFETY: This private, aligned static range belongs only to this probe
    // allocator and remains valid for the allocator's lifetime.
    let initialized = unsafe {
        FREE_BYTES_ALLOCATOR.initialize_region(
            core::ptr::addr_of_mut!(FREE_BYTES_HEAP.0).cast::<u8>(),
            FREE_BYTES_HEAP_SIZE,
        )
    };
    if initialized.is_err() {
        return false;
    }

    let before = FREE_BYTES_ALLOCATOR.free_bytes();
    let allocation = FREE_BYTES_ALLOCATOR.allocate(64, 32);
    let after_allocate = FREE_BYTES_ALLOCATOR.free_bytes();
    if allocation.is_null() {
        return false;
    }
    // SAFETY: `allocation` is live and returned by this allocator with the
    // same size and alignment passed to allocate.
    unsafe { FREE_BYTES_ALLOCATOR.deallocate(allocation, 64, 32) };
    let after_free = FREE_BYTES_ALLOCATOR.free_bytes();

    before > 0 && after_allocate < before && after_free == before
}

fn probe_arena1_bounds() -> bool {
    let before = (system::get_arena1_lo(), system::get_arena1_hi());
    let checked = check_bounds(
        system::get_arena1_lo,
        system::get_arena1_hi,
        |value| unsafe { system::set_arena1_lo(value) },
        |value| unsafe { system::set_arena1_hi(value) },
        system::get_arena1_size,
    );
    checked && before == (system::get_arena1_lo(), system::get_arena1_hi())
}

fn probe_arena2_bounds() -> bool {
    let before = (system::get_arena2_lo(), system::get_arena2_hi());
    let checked = check_bounds(
        system::get_arena2_lo,
        system::get_arena2_hi,
        |value| unsafe { system::set_arena2_lo(value) },
        |value| unsafe { system::set_arena2_hi(value) },
        system::get_arena2_size,
    );
    checked && before == (system::get_arena2_lo(), system::get_arena2_hi())
}

// The temporary bounds stay inside the original free arena. No memory is
// allocated or accessed before the original bounds are restored.
fn check_bounds(
    get_lo: impl Fn() -> *mut c_void,
    get_hi: impl Fn() -> *mut c_void,
    set_lo: impl Fn(*mut c_void),
    set_hi: impl Fn(*mut c_void),
    get_size: impl Fn() -> u32,
) -> bool {
    let lo = get_lo() as usize;
    let hi = get_hi() as usize;
    if hi.checked_sub(lo).is_none_or(|size| size < 0x1000) {
        return false;
    }
    let new_lo = lo + 0x100;
    let new_hi = hi - 0x100;
    set_lo(new_lo as *mut c_void);
    set_hi(new_hi as *mut c_void);
    let changed = get_lo() as usize == new_lo
        && get_hi() as usize == new_hi
        && get_size() == (new_hi - new_lo) as u32;
    set_lo(lo as *mut c_void);
    set_hi(hi as *mut c_void);
    let restored = get_lo() as usize == lo && get_hi() as usize == hi;
    changed && restored
}

fn test_arena1(original_lo: *mut c_void) -> bool {
    let expected_ptr = (original_lo as usize).div_ceil(0x20) * 0x20;
    let expected_lo = expected_ptr + 0x120;
    let ptr = system::alloc_arena1_mem_lo(0x120, 0x20) as usize;
    let new_lo = system::get_arena1_lo() as usize;
    let next_ptr = system::alloc_arena1_mem_lo(0x31, 0x100) as usize;
    let expected_next = expected_lo.div_ceil(0x100) * 0x100;
    ptr == expected_ptr
        && new_lo == expected_lo
        && next_ptr == expected_next
        && system::get_arena1_lo() as usize == expected_next + 0x100
}

fn test_arena2(original_lo: *mut c_void) -> bool {
    let expected_ptr = (original_lo as usize).div_ceil(0x40) * 0x40;
    let expected_lo = expected_ptr + 0x180;
    let ptr = system::alloc_arena2_mem_lo(0x180, 0x40) as usize;
    let new_lo = system::get_arena2_lo() as usize;
    let next_ptr = system::alloc_arena2_mem_lo(0x31, 0x100) as usize;
    let expected_next = expected_lo.div_ceil(0x100) * 0x100;
    ptr == expected_ptr
        && new_lo == expected_lo
        && next_ptr == expected_next
        && system::get_arena2_lo() as usize == expected_next + 0x100
}

// These checks reserve only unused arena space. They do not access the
// reserved memory, and restore both original bounds before returning.
fn probe_raw_arena1() -> bool {
    // SAFETY: The probe serializes arena changes and restores the saved bounds.
    unsafe {
        let lo = raw::sys_get_arena1_lo();
        let hi = raw::sys_get_arena1_hi();
        let bounds = check_bounds(
            || raw::sys_get_arena1_lo(),
            || raw::sys_get_arena1_hi(),
            |value| raw::sys_set_arena1_lo(value),
            |value| raw::sys_set_arena1_hi(value),
            || raw::sys_get_arena1_size(),
        );
        let expected = (lo as usize).div_ceil(32) * 32;
        let allocation = raw::sys_alloc_arena1_mem_lo(0x31, 32) as usize;
        let next = raw::sys_get_arena1_lo() as usize;
        let invalid = raw::sys_alloc_arena1_mem_lo(1, 3).is_null()
            && raw::sys_get_arena1_lo() as usize == next;
        raw::sys_set_arena1_lo(lo);
        raw::sys_set_arena1_hi(hi);
        bounds
            && allocation == expected
            && next == expected + 0x40
            && invalid
            && raw::sys_get_arena1_lo() == lo
            && raw::sys_get_arena1_hi() == hi
    }
}

fn probe_raw_arena2() -> bool {
    // SAFETY: The probe serializes arena changes and restores the saved bounds.
    unsafe {
        let lo = raw::sys_get_arena2_lo();
        let hi = raw::sys_get_arena2_hi();
        let raw_bounds = check_bounds(
            || raw::sys_get_arena2_lo(),
            || raw::sys_get_arena2_hi(),
            |value| raw::sys_set_arena2_lo(value),
            |value| raw::sys_set_arena2_hi(value),
            || raw::sys_get_arena2_size(),
        );
        let abi_bounds = check_bounds(
            || raw::abi_sys_get_arena2_lo(),
            || raw::abi_sys_get_arena2_hi(),
            |value| raw::abi_sys_set_arena2_lo(value),
            |value| raw::abi_sys_set_arena2_hi(value),
            || raw::abi_sys_get_arena2_size(),
        );
        let expected = (lo as usize).div_ceil(64) * 64;
        let direct = raw::sys_alloc_arena2_mem_lo(0x31, 64) as usize;
        let direct_next = raw::sys_get_arena2_lo() as usize;
        let invalid = raw::sys_alloc_arena2_mem_lo(0, 64).is_null()
            && raw::sys_get_arena2_lo() as usize == direct_next;
        raw::sys_set_arena2_lo(lo);
        let abi = raw::abi_sys_alloc_arena2_mem_lo(0x31, 64) as usize;
        let abi_next = raw::abi_sys_get_arena2_lo() as usize;
        raw::abi_sys_set_arena2_lo(lo);
        raw::abi_sys_set_arena2_hi(hi);
        raw_bounds
            && abi_bounds
            && direct == expected
            && direct_next == expected + 0x40
            && invalid
            && abi == expected
            && abi_next == expected + 0x40
            && raw::sys_get_arena2_lo() == lo
            && raw::sys_get_arena2_hi() == hi
    }
}
