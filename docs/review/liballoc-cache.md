# Allocation and Cache Review

Date: 2026-10-01

Status: static source comparison is complete for `ogc-alloc` and `ogc-cache`. The MEM2 arena-boundary contract mismatch remains open. The interrupt-safety fix is implemented and awaits independent review. Allocator host tests and the PowerPC target check passed; no hardware check was run.

## Scope

- Rust allocator: `crates/core/ogc-alloc/src/lib.rs`
- Rust cache: `crates/core/ogc-cache/src/lib.rs`
- C cache API: `C-sources/libogc/gc/ogc/cache.h`
- C cache implementation: `C-sources/libogc/libogc/cache.c`, `cache_asm.S`, `C-sources/libogc/tuxedo/cache.c`, and `C-sources/libogc/gc/tuxedo/ppc/cache.h`
- Closest C allocator reference: `C-sources/libogc/gc/ogc/lwp_heap.h`, `C-sources/libogc/libogc/lwp_heap.c`, `lwp_heap.inl`, `C-sources/libogc/libogc/sbrk.c`, and arena accessors in `system.h` / `system.c`.

Libogc does not expose the same bounded Rust `GlobalAlloc` API. The LWP heap is the closest explicit allocator implementation. Newlib `malloc` uses `_sbrk_r` and arena boundaries, so that path is included for the Wii reservation comparison.

## Rust API inventory and C comparison

### `ogc-alloc`

All production public APIs and constants are in `crates/core/ogc-alloc/src/lib.rs`:

- `ArenaAllocator`, `new`, `initialize_region`, `allocate`, `deallocate`, `capacity`, and `used_bytes` (lines 48-167). `Default` is at lines 394-398. The allocator uses a 16-byte aligned boundary-tag block header, a doubly linked free list, first-fit search, block splitting, and adjacent free-block coalescing (lines 169-392). C LWP heap has a distinct representation and algorithm: `heap_block` boundary flags plus links in free blocks (`lwp_heap.h:21-27`), initialization and page-size validation (`lwp_heap.c:9-38`), first-fit allocation with page-size rounding (`40-96`), coalescing free (`98-159`), and heap statistics (`161-195`). The Rust API allows each allocation its own power-of-two alignment and returns a raw pointer; C `__lwp_heap_allocate` uses the heap's fixed `pg_size`, and C `__lwp_heap_free` reports success/failure. These are design differences, not missing translations of a public C ABI.
- `InitError` variants (lines 410-415) and the default heap/reserve constants (418-420) are Rust-only.
- `initialize_wii` (lines 422-452) takes caller-supplied arena bounds and returns the end of its bounded region. C arena getters/setters are declared in `system.h:400-416`; the Wii arena-2 accessors and allocation helper are implemented in `system.c:1252-1316`. Newlib's Wii `_sbrk_r` moves `SYS_GetArena2Lo()` while it allocates from MEM2 (`sbrk.c:25-69`).
- `GlobalAlloc` implementation (lines 454-464) is Rust-only; C provides `malloc`/`free` through newlib `_sbrk_r`, not this bounded allocator API.
- `align_up` (lines 466-470) is private. Allocator host tests exercise lock and IRQ-state ordering, alignments, reuse, coalescing, exhaustion, accounting, and bad inputs. All 6 tests passed. The PowerPC-gated Wii cap helper test was not run on the host.

### `ogc-cache`

Rust public cache API inventory in `crates/core/ogc-cache/src/lib.rs`, matched below to `cache.h` and the C implementation:

| Rust API and lines | C counterpart and source | Review result |
|---|---|---|
| `LC_BASEPREFIX`, `LC_BASE` (11-12), `lc_get_base` (762-764) | `LC_BASEPREFIX`, `LC_BASE`, `LCGetBase` (`cache.h:46-47,336`) | Values and address match. Rust function replaces the C macro. |
| `dc_enable`, `dc_disable`, `dc_freeze`, `dc_unfreeze`, `dc_flash_invalidate` (273-306) | `DCEnable`, `DCDisable`, `DCFreeze`, `DCUnfreeze`, `DCFlashInvalidate` (`cache.h:60-105`; assembly `cache_asm.S:34-72`) | HID0 bits, read/modify/write, and `isync` match. |
| `sync` (308-310), `ic_sync` (370-372) | `PPCSync`, `ICSync` (`gc/tuxedo/ppc/intrinsics.h`; `cache.h:217-220`) | Rust emits `sync` / `isync`, consistent with C inline wrappers. |
| `dc_invalidate_range` (312-318), `dc_flush_range` (320-327), `dc_store_range` (329-336), no-sync variants (338-352), `dc_zero_range` (354-360), `dc_touch_range` (362-368) | `DCInvalidateRange`, `DCFlushRange`, `DCStoreRange`, `DCFlushRangeNoSync`, `DCStoreRangeNoSync`, `DCZeroRange`, `DCTouchRange` (`cache.h:119-206`) | Rust uses the same line count formula and 32-byte stepping as `tuxedo/cache.c:8-30`. Flush/store sync behavior matches `cache.h:134-168`. |
| `ic_flash_invalidate`, `ic_enable`, `ic_disable`, `ic_freeze`, `ic_unfreeze` (374-407) | `ICFlashInvalidate`, `ICEnable`, `ICDisable`, `ICFreeze`, `ICUnfreeze` (`cache.h:232-274`; assembly `cache_asm.S:74-112`) | HID0 bits and `isync` match. |
| `ic_block_invalidate` (409-418), `ic_invalidate_range` (420-426) | `ICBlockInvalidate`, `ICInvalidateRange` (`cache.h:287-304`) | `icbi` operation and line walking match C inline/helper implementation. Rust uses `*const c_void` where the C declarations use `void *`; this is a Rust const-correctness choice. |
| `lc_enable`, `lc_disable` (428-439) | `LCEnable` (`cache.c:40-47`, `cache_asm.S:179-231`), `LCDisable` (`cache_asm.S:233-244`) | Initialization, interrupt masking intent, cache clearing, and HID2 bit operation match the reference. |
| `lc_load_blocks`, `lc_store_blocks` (441-495) | `LCLoadBlocks`, `LCStoreBlocks` (`cache.h:324-325`, `cache_asm.S:273-295`) | DMA register bit assembly and direction fields match. |
| `lc_load_data`, `lc_store_data` (497-555) | `LCLoadData`, `LCStoreData` (`cache.c:49-91`) | 32-byte rounding, 128-line chunking, pointer increments, and returned queue count match. |
| `lc_queue_length`, `lc_queue_wait`, `lc_flush_queue` (557-573) | `LCQueueLength`, `LCQueueWait`, `LCFlushQueue` (`cache.c:93-111`) | HID2 field, wait threshold, DMA registers, and final `sync` match. |
| `lc_alloc`, `lc_alloc_no_invalidate` (575-605) | `LCAlloc`, `LCAllocNoInvalidate` (`cache.c:113-139`) | Byte-to-line conversion, conditional LC enable, and invalidate selection match. |
| `lc_alloc_one_tag`, `lc_alloc_tags` (607-644) | `LCAllocOneTag`, `LCAllocTags` (`cache_asm.S:246-271`) | Zero-count handling, optional `dcbi`, `dcbz_l`, and 32-byte stepping match. |
| `l2_enhance` (646-682), private L2 helpers (684-717) | `L2Enhance` (`cache.h:306-320`, `cache.c:140-175`), L2 assembly (`cache_asm.S:114-177`) | Both ranges are flushed, the same HID4 bits are set, L2 is invalidated/re-enabled, and the HBC stub scan uses the same signature and opcode mask. Rust uses volatile stub reads and an uncached volatile write. |

Rust-private helper coverage: `cache_range_op!` and `line_count` (lines 22-36, 254-262); PPC SPR/MSR helpers (38-250); `write_u32_uncached` (264-271); `lc_enable_inner` (719-760). These were traced to the corresponding C inline assembly and/or cache assembly. Rust constants `MSR_EE`, `MSR_FP`, and `L2_FLUSH_RANGES` (15-20) are helper state, not C public API. Rust-only target-gated helper tests are at lines 766-789 and were not run.

## Confirmed mismatch

`initialize_wii` is documented as reserving MEM2 (`lib.rs:422-426`) and the module documentation says the Wii initializer reserves by advancing arena2's low boundary (`lib.rs:3-7`). The function only initializes the allocator over `arena_low..end` and returns `end` (`lib.rs:428-451`); it does not update `SYS_ARENA2_LO` or call `SYS_SetArena2Lo`. C's arena reservation changes the global boundary through the setter (`system.c:1252-1269`), and `_sbrk_r` relies on that boundary when it allocates MEM2 (`sbrk.c:27-48`). Unless the caller separately advances the boundary using the returned address, the region remains available to another allocator. This is a confirmed behavior/documentation mismatch; whether caller-side boundary update is intentionally required is not clear from this crate API.

## Interrupt-safety fix

The original Rust allocator used only an `AtomicBool` spin lock (`lib.rs:382-390`). C's LWP heap saves and disables interrupts before heap mutation and restores the saved state on success and error exits (`lwp_heap.c:16-35,53-93,107-156`). Rust now saves and disables external interrupt delivery before entering the spin loop, then releases the lock before restoring the saved state. The guard restores prior state on all normal and early-return exits from protected operations (`ogc-alloc/src/lib.rs`). This prevents an interrupt from re-entering allocation while interrupted code owns the lock, and it preserves already-disabled caller state.

**BUG FIX.** `/root/worker_a` added the PowerPC `ogc-irq-sys` dependency and saved-state guard behavior, plus a unit test that checks lock release precedes restoration and that the saved state is passed through. This fix needs independent static review. Other allocator findings remain open, including the separate MEM2 arena-boundary contract mismatch above.

## Rust-only additions and uncertain differences

- The bounded coalescing allocator, `GlobalAlloc` adapter, per-request alignment, byte accounting, default MEM2 limits, and `initialize_wii` helper are Rust-only API. The nearest C facilities (`lwp_heap` and `_sbrk_r`) have different allocation contracts, so their algorithm differences are not reported as parity defects.
- `initialize_wii`'s end-pointer return suggests caller-managed arena boundary adjustment may be deliberate. No call site of this helper was found in the reviewed Rust crate tree. The required reservation protocol is therefore uncertain.
- Rust cache range methods are `unsafe`; C declares ordinary C functions. This strengthens the Rust call contract without changing the hardware operation.
- `L2Enhance` is conditional on `HW_RVL` in C (`cache.h:318-320`, `cache.c:140`) but Rust exposes `l2_enhance` unconditionally on its PowerPC-only crate (`lib.rs:646`). On non-Wii PowerPC this function compiles to an empty body because the implementation body is target-architecture-gated but not console-gated. This is an API availability difference; callers cannot cause L2 writes on non-PowerPC via this function, but the target distinction is not encoded.

## Completeness check and limits

Completeness was checked by enumerating every top-level `pub` declaration in both Rust source files, tracing all private cache helpers used by those declarations, and matching the cache names against all declarations in `cache.h` plus definitions in `cache.c`, `cache_asm.S`, and `tuxedo/cache.c`. For allocation, all exported Rust items and their full allocator paths were read, then compared with every public LWP heap declaration and implementation plus Wii arena and `_sbrk_r` reservation behavior. No other Rust source module exists under either crate's `src` directory. This is a source review only: no compiler, tests, emulator, or console were run. `ogc-cache` is crate-gated to PowerPC (`lib.rs:3-4`), and target-specific behavior was inspected as source rather than executed.
