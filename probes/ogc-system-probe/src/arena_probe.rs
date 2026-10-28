use core::ffi::c_void;

use ogc_system::{self as system};

use crate::probe_support::ProbeLogger;

pub fn run(logger: &mut ProbeLogger) {
    logger.section("ARENA PROBE");

    let original = system::snapshot_arenas();
    logger.note_hex32("arena1 lo", original.arena1_lo as usize as u32);
    logger.note_hex32("arena1 hi", original.arena1_hi as usize as u32);
    logger.note_hex32("arena2 lo", original.arena2_lo as usize as u32);
    logger.note_hex32("arena2 hi", original.arena2_hi as usize as u32);

    logger.record("arena1 low alloc math", test_arena1(original.arena1_lo));
    logger.record("arena2 low alloc math", test_arena2(original.arena2_lo));

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
}

fn test_arena1(original_lo: *mut c_void) -> bool {
    let (expected_ptr, expected_lo) =
        system::arena_low_alloc_from(original_lo as usize, 0x120, 0x20);
    let ptr = system::alloc_arena1_mem_lo(0x120, 0x20) as usize;
    let new_lo = system::get_arena1_lo() as usize;
    ptr == expected_ptr && new_lo == expected_lo
}

fn test_arena2(original_lo: *mut c_void) -> bool {
    let (expected_ptr, expected_lo) =
        system::arena_low_alloc_from(original_lo as usize, 0x180, 0x40);
    let ptr = system::alloc_arena2_mem_lo(0x180, 0x40) as usize;
    let new_lo = system::get_arena2_lo() as usize;
    ptr == expected_ptr && new_lo == expected_lo
}
