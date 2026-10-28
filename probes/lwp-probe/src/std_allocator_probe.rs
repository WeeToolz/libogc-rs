//! Checks actual std System storage and the production reserved memory ranges.
use core::sync::atomic::{AtomicU32, Ordering};
use ogc_system_probe::probe_support::ProbeLogger;
use std::alloc::{GlobalAlloc, Layout, System};
#[unsafe(no_mangle)]
pub static WII_STD_HEAP_SNAPSHOTS: [AtomicU32; 6] = [const { AtomicU32::new(0) }; 6];

fn physical(range: core::ops::Range<usize>) -> core::ops::Range<usize> {
    (range.start & 0x3fff_ffff)..(range.end & 0x3fff_ffff)
}
fn separate(a: &core::ops::Range<usize>, b: &core::ops::Range<usize>) -> bool {
    a.start < a.end && b.start < b.end && (a.end <= b.start || b.end <= a.start)
}

fn worker_allocations(marker: u8) -> bool {
    let layout = Layout::from_size_align(513, 64).expect("fixed worker layout");
    for _ in 0..64 {
        let pointer = unsafe { System.alloc(layout) };
        if pointer.is_null() {
            return false;
        }
        let bytes = unsafe { core::slice::from_raw_parts_mut(pointer, 513) };
        bytes.fill(marker);
        std::thread::yield_now();
        let valid = bytes.iter().all(|&byte| byte == marker);
        unsafe { System.dealloc(pointer, layout) };
        if !valid {
            return false;
        }
    }
    true
}

pub fn run(logger: &mut ProbeLogger) {
    logger.section("STD SYSTEM ALLOCATOR AND MEMORY RANGES");
    let Some(heap) = ogc_std_runtime::heap_region().map(physical) else {
        logger.record("std heap region available", false);
        return;
    };
    let Some(workspace) = ogc_system_sys::lwp_workspace_region().map(physical) else {
        logger.record("LWP workspace region available", false);
        return;
    };
    logger.record(
        "std heap and LWP workspace do not overlap",
        separate(&heap, &workspace),
    );
    let framebuffers = ogc_system_probe::probe_support::framebuffer_regions();
    logger.record(
        "both full framebuffers are outside both reserved heaps",
        framebuffers.is_some_and(|ranges| {
            let [a, b] = ranges.map(physical);
            separate(&a, &b)
                && separate(&a, &heap)
                && separate(&b, &heap)
                && separate(&a, &workspace)
                && separate(&b, &workspace)
        }),
    );
    // Initialize main-thread std metadata used by scoped thread panic handling
    // before measuring worker allocations. Its lazy TLS remains live on main.
    let current = std::thread::current();
    let panic_runtime_ready = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| ())).is_ok();
    logger.record("main panic TLS is initialized", panic_runtime_ready);
    // std initializes worker TLS and its process-wide key on first use. Warm
    // both worker paths, then measure a repeated run for leaked allocations.
    let warmup = std::thread::scope(|scope| {
        let first = std::thread::Builder::new()
            .stack_size(40 * 1024)
            .spawn_scoped(scope, || worker_allocations(0x19));
        let second = std::thread::Builder::new()
            .stack_size(40 * 1024)
            .spawn_scoped(scope, || worker_allocations(0x82));
        match (first, second) {
            (Ok(first), Ok(second)) => {
                first.join().is_ok_and(|ok| ok) && second.join().is_ok_and(|ok| ok)
            }
            (Err(error), _) | (_, Err(error)) => {
                logger.note_i32(
                    "allocator warm-up worker error",
                    error.raw_os_error().unwrap_or(-1),
                );
                false
            }
        }
    });
    logger.record("std worker TLS warm-up completes", warmup);
    let baseline = ogc_std_runtime::heap_used_bytes();
    let vector = vec![0x3cu8; 257];
    let address = vector.as_ptr() as usize & 0x3fff_ffff;
    logger.record(
        "Vec uses the std global MEM2 allocator",
        heap.contains(&address)
            && address + vector.len() <= heap.end
            && vector.iter().all(|&byte| byte == 0x3c)
            && ogc_std_runtime::heap_used_bytes() >= baseline + 257,
    );
    drop(vector);
    logger.record(
        "Vec drop releases its std heap payload",
        ogc_std_runtime::heap_used_bytes() == baseline,
    );
    for alignment in [8, 32, 64, 4096] {
        let layout = Layout::from_size_align(257, alignment).expect("fixed valid layout");
        let pointer = unsafe { System.alloc_zeroed(layout) };
        logger.record(
            "aligned zeroed System allocation succeeds",
            !pointer.is_null() && pointer as usize % alignment == 0,
        );
        if pointer.is_null() {
            continue;
        }
        let address = pointer as usize & 0x3fff_ffff;
        logger.record(
            "System allocation is inside reserved MEM2",
            heap.contains(&address) && address + 257 <= heap.end,
        );
        let bytes = unsafe { core::slice::from_raw_parts_mut(pointer, 257) };
        logger.record(
            "System allocation clears every byte",
            bytes.iter().all(|&b| b == 0),
        );
        bytes.fill(0xa5);
        let huge = ogc_std_runtime::heap_capacity() + 4096;
        let failed = unsafe { System.realloc(pointer, layout, huge) };
        logger.record(
            "System resize failure preserves the original bytes",
            failed.is_null() && bytes.iter().all(|&b| b == 0xa5),
        );
        if !failed.is_null() {
            unsafe {
                System.dealloc(
                    failed,
                    Layout::from_size_align(huge, alignment).expect("valid failure layout"),
                )
            };
            continue;
        }
        let grown = unsafe { System.realloc(pointer, layout, 1024) };
        logger.record(
            "System resize preserves payload",
            !grown.is_null()
                && unsafe { core::slice::from_raw_parts(grown, 257) }
                    .iter()
                    .all(|&b| b == 0xa5),
        );
        if grown.is_null() {
            unsafe { System.dealloc(pointer, layout) };
        } else {
            unsafe {
                System.dealloc(
                    grown,
                    Layout::from_size_align(1024, alignment).expect("valid grown layout"),
                )
            };
        }
    }
    WII_STD_HEAP_SNAPSHOTS[0].store(baseline as u32, Ordering::Release);
    WII_STD_HEAP_SNAPSHOTS[1].store(ogc_std_runtime::heap_used_bytes() as u32, Ordering::Release);
    logger.note_u32("heap baseline", baseline as u32);
    logger.note_u32(
        "heap after direct allocator checks",
        ogc_std_runtime::heap_used_bytes() as u32,
    );
    let concurrent = std::thread::scope(|scope| {
        let mut workers = Vec::new();
        for marker in [0x31u8, 0x72] {
            match std::thread::Builder::new()
                .stack_size(40 * 1024)
                .spawn_scoped(scope, move || worker_allocations(marker))
            {
                Ok(worker) => workers.push(worker),
                Err(error) => {
                    logger.note_i32(
                        "allocator worker creation error",
                        error.raw_os_error().unwrap_or(-1),
                    );
                    logger.record("allocator worker created", false);
                }
            }
        }
        let count = workers.len();
        let mut passed = count == 2;
        for (index, worker) in workers.into_iter().enumerate() {
            passed &= worker.join().is_ok_and(|result| result);
            WII_STD_HEAP_SNAPSHOTS[index + 2]
                .store(ogc_std_runtime::heap_used_bytes() as u32, Ordering::Release);
        }
        passed
    });
    logger.record(
        "concurrent System allocations preserve both worker payloads",
        concurrent,
    );
    drop(current);
    let layout = Layout::from_size_align(ogc_std_runtime::heap_capacity() + 4096, 32)
        .expect("valid oversized layout");
    let failed = unsafe { System.alloc(layout) };
    logger.record("System allocation failure returns null", failed.is_null());
    if !failed.is_null() {
        unsafe { System.dealloc(failed, layout) };
    }
    WII_STD_HEAP_SNAPSHOTS[2].store(ogc_std_runtime::heap_used_bytes() as u32, Ordering::Release);
    logger.note_u32(
        "heap after worker cleanup",
        ogc_std_runtime::heap_used_bytes() as u32,
    );
    WII_STD_HEAP_SNAPSHOTS[5].store(ogc_std_runtime::heap_used_bytes() as u32, Ordering::Release);
    logger.record(
        "System release restores live byte count",
        ogc_std_runtime::heap_used_bytes() == baseline,
    );
}
