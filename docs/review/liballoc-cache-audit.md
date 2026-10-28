# Allocator and cache finding audit

Reviewer: `/root/reviewer_4`  
Date: 2026-10-01  
Implementer: `/root` (the original review note does not identify a separate implementer; no implementation fix was reviewed).  
Scope: independent static verification of the open `ogc-alloc` and `ogc-cache` findings in `liballoc-cache.md`. No builds, tests, or hardware checks were run.

## `ogc-alloc`: arena boundary and interrupt safety

The arena-boundary finding is confirmed as an API/documentation contract mismatch. Rust `initialize_wii` says it reserves the low MEM2 arena for Rust allocations and says the new boundary is the allocator end (`crates/core/ogc-alloc/src/lib.rs:422-426`). It initializes only the local `ArenaAllocator` from `low` through `end`, then returns `end` (`:428-451`). It does not call `SYS_SetArena2Lo` or mutate a shared arena boundary. C's `SYS_AllocArena2MemLo` advances the global low boundary with `SYS_SetArena2Lo` (`C-sources/libogc/libogc/system.c:1305-1315`), while `_sbrk_r` reads and advances `SYS_GetArena2Lo` for MEM2 allocations (`C-sources/libogc/libogc/sbrk.c:27-48`). Therefore, unless callers separately set the boundary from the returned pointer, C/newlib users can still treat the allocator's range as available. The return value could mean that boundary management is intentionally delegated; the current “reserve” documentation does not state that required caller step, so this remains a valid contract mismatch, not proof that every call site overlaps allocations.

The interrupt-safety concern is also confirmed, conditionally. Rust's allocator lock repeatedly spins on an `AtomicBool` until it acquires the lock (`lib.rs:382-390`). The closest C LWP heap protects initialization and every allocate/free mutation with `_CPU_ISR_Disable` and restores the prior interrupt state on its exits (`C-sources/libogc/libogc/lwp_heap.c:16-35,53-95,107-158`). If an interrupt handler calls the Rust allocator after interrupting code that holds this lock, the handler cannot wait for the interrupted code to release it. The deadlock risk applies only if allocator use from interrupt context is possible; no such call site was established in this audit. This finding is about the Rust allocator lock and does not establish a defect in `ogc-cache`.

## `ogc-cache`: findings checked

The cache crate itself does not use the allocator's `AtomicBool` lock for cache mutations. Its `lc_enable` masks interrupts through `lock_by_msr`, runs the cache initialization body, then restores the saved MSR (`crates/core/ogc-cache/src/lib.rs:428-432`); the C `LCEnable` wrapper calls the assembly helper that saves the original MSR and performs cache setup (`C-sources/libogc/libogc/cache.c:40-47`, `cache_asm.S:179-231`). I found no cache interrupt-safety mismatch corresponding to the allocator spin-lock concern.

The pre-existing `L2Enhance` target-availability mismatch is confirmed. C declares and defines `L2Enhance` only under `HW_RVL` (`C-sources/libogc/gc/ogc/cache.h:318-320`, `libogc/cache.c:140-141`). Rust exposes `l2_enhance` on the PowerPC-only crate without a Wii/`HW_RVL` cfg (`crates/core/ogc-cache/src/lib.rs:646-682`, as inventoried in `liballoc-cache.md`). Thus it is exported for non-Wii PowerPC builds even though the C API is absent there. Keep `ogc-cache` open for this API surface mismatch; do not attribute the allocator lock finding to it.

## Result

- `ogc-alloc`: both documented concerns are upheld, with the stated call-context and caller-boundary qualifications. Keep pending/finding.
- `ogc-cache`: allocator interrupt-safety concern does not apply to this crate; the independent `L2Enhance` target-gating mismatch is upheld. Keep pending/finding.
- Validation: static source inspection only. No tests were run or compiled. No hardware verification was performed.

## Independent `ogc-cache` target-availability review

**Result: NEEDS_CHANGES; `ogc-cache` remains open.** Reviewer: `reviewer_e`. No implementer fix for this cache finding is present in the reviewed source; the current GameCube profile registration came from `e694fd2` by `notOrrytrout`. This section covers only `ogc-cache`; it makes no new disposition for the separate `ogc-alloc` findings above.

C declares `L2Enhance` only inside `#ifdef HW_RVL` (`../../C-sources/libogc/gc/ogc/cache.h:306–320`) and defines the function under the same guard (`../../C-sources/libogc/libogc/cache.c:140–141`). The Rust `l2_enhance` public function has no `cfg(gamecube)` or equivalent target gate (`crates/core/ogc-cache/src/lib.rs:646–682`). The repository's GameCube target profile sets `--cfg=gamecube` (`.cargo/config.toml:26–32`, target spec `targets/powerpc-nintendo-gamecube.json`), but that cfg has no effect on this function. The umbrella crate also re-exports the cache module without a platform gate (`crates/compatibility/ogc/src/lib.rs:8–13`). Thus both a direct `ogc-cache` user and a GameCube user of `ogc::cache` can call `l2_enhance`, although the C declaration and definition are unavailable when `HW_RVL` is not defined. This confirms the API-availability mismatch independently of the prior audit.

Validation: `cargo check -p ogc-cache` passed for the actual GameCube JSON target using nightly `build-std=core,alloc`. This confirms the current GameCube configuration still compiles the crate with its public method. The crate has unit tests (`src/lib.rs:770,782`), but tests were not built or run. No hardware verification was performed. The ledger is unchanged; only this audit note was edited.

## Fix handoff: Wii-only `l2_enhance` API

**BUG FIX.** Implementer: `worker_a`. Independent re-review is needed; the review thread will handle it.

Rust now gates the public `l2_enhance` function and its API-availability test with `#[cfg(not(gamecube))]`. This matches C's `#ifdef HW_RVL`: the repository's Wii profile has no `gamecube` cfg, while its GameCube profile sets it. The umbrella crate can continue to export the shared cache module; the Wii-only function is absent from that module on GameCube builds. The PowerPC test helper includes a function-pointer reference on Wii builds so the API remains covered when target tests are compiled.

Validation: `cargo check -p ogc-cache --target powerpc-unknown-linux-gnu --tests` passed, compiling the Wii API-reference test. The actual GameCube target check passed with nightly Cargo and `build-std=core,alloc`, confirming the crate compiles after the API and L2-only implementation are cfg-gated. A GameCube `--tests` build was attempted but could not find the target `test` crate, so that test binary was not compiled. No tests were run and no hardware verification was performed.

## Fix handoff: Arena 2 boundary reservation contract

**BUG FIX.** Implementer: `worker_a`. Independent re-review is needed; the review thread will handle it.

`ogc-alloc` cannot call `ogc-system::set_arena2_lo` directly because `ogc-system-sys` depends on `ogc-alloc`. `initialize_wii` now requires a caller-supplied setter callback. It invokes that callback with the heap end only after the allocator initializes successfully. This makes advancing the shared Arena 2 low boundary part of the initializer contract without adding a dependency cycle. The caller must pass the current arena bounds and a callback that updates the real shared boundary before any other allocator uses the range. Regression coverage checks that the callback receives the end pointer and runs after allocator initialization.

Validation: `cargo test -p ogc-alloc` passed all 6 host tests. `cargo check -p ogc-alloc --target powerpc-unknown-linux-gnu --tests` passed, compiling the PowerPC-gated initializer regression; target tests were not run. `git diff --check` passed. No hardware verification was performed.

## Independent re-review of grouped fix `bd602f7`

**Result: accepted for the reviewed findings.** Reviewer: `reviewer_d`. Implementer: `worker_a` (commit author `notOrrytrout`). Reviewed the full source diff in `crates/core/ogc-alloc/src/lib.rs` and `crates/core/ogc-cache/src/lib.rs`, plus the grouped commit and its validation notes.

### `ogc-alloc`: Arena 2 boundary publication

In C, `SYS_AllocArena2MemLo` reads the shared low boundary, computes the aligned allocation end, and publishes it with `SYS_SetArena2Lo` before returning (`../../C-sources/libogc/libogc/system.c:1305-1315`). Newlib also reads and changes that same global boundary (`../../C-sources/libogc/libogc/sbrk.c:27-68`). Before this fix, Rust initialized only its local allocator and returned the end pointer. The fix now calls the required `set_arena2_lo` callback with that end after `initialize_region` succeeds (`crates/core/ogc-alloc/src/lib.rs:466-505`). The function's safety contract states that the callback must update the shared boundary before other allocators use the range (`:474-479`). This provides the C-equivalent publication operation while avoiding a crate dependency cycle. The callback is not fallible, and it is not called if bounds validation or allocator initialization fails.

There is no production call site for `initialize_wii` in the repository, so this review confirms the helper's behavior and caller contract. A future caller must pass a setter that actually updates the shared boundary; a no-op callback would violate the documented safety contract.

The earlier interrupt re-entry concern is still resolved in the current code: Rust disables external interrupts before acquiring its spin lock and unlocks before restoring the saved IRQ state (`crates/core/ogc-alloc/src/lib.rs:388-397,402-410,449-457`). This follows the C heap's `_CPU_ISR_Disable`/restore critical sections (`../../C-sources/libogc/libogc/lwp_heap.c:9-38,40-95,98-159`).

### `ogc-cache`: Wii-only L2 API

C exposes `L2Enhance` only under `HW_RVL` in both declaration and definition (`../../C-sources/libogc/gc/ogc/cache.h:306-320`, `../../C-sources/libogc/libogc/cache.c:140-141`). The fix gates Rust `l2_enhance` and its L2-only helpers with `#[cfg(not(gamecube))]` (`crates/core/ogc-cache/src/lib.rs:649-717`). The repository's GameCube target profile sets `--cfg=gamecube` (`.cargo/config.toml:26-32`); its Wii target does not. The public function and implementation are therefore excluded from the repository's GameCube build and retained in the Wii build, matching the C API availability. The remaining body is unchanged by this commit.

### Validation and disposition

`cargo test -p ogc-alloc` ran and passed all 6 host tests. `cargo check -p ogc-alloc --target powerpc-unknown-linux-gnu --tests` passed; this compiled the PowerPC-only initializer test but did not run it. `cargo check -p ogc-cache --target powerpc-unknown-linux-gnu --tests` passed. Nightly `cargo check -p ogc-cache` passed for both `targets/powerpc-nintendo-none.json` and `targets/powerpc-nintendo-gamecube.json` with `-Z json-target-spec -Z build-std=core,alloc`. The custom target checks emitted `unexpected_cfgs` warnings for `gamecube`; no compile errors occurred. Tests were not run for `ogc-cache`. No hardware verification was performed. The reviewed arena-boundary and L2 API findings are resolved at the crate implementation and documented caller-contract level; no remaining mismatch was found in these changes.

## Independent `ogc-alloc` follow-up

**Result: NEEDS_CHANGES for the arena-boundary contract; the interrupt re-entry finding is resolved by the current fix.** Reviewer: `reviewer_e`. Relevant implementers: `notOrrytrout` for the allocator and `initialize_wii` (`111be89`), and `worker_a` / commit author `notOrrytrout` for the interrupt-mask fix (`3f9c2ce`). This section covers only `ogc-alloc`; it does not revise the separate cache result above.

### Arena boundary: operation difference confirmed, runtime impact conditional

Current Rust module documentation says `initialize_wii` reserves a range by advancing Arena 2's low boundary (`crates/core/ogc-alloc/src/lib.rs:3–7`); the function docs say the current low boundary is the allocation start and refer to a new low boundary (`:463–467`). The function itself only initializes the local allocator for `low..end` and returns `end` (`:469–492`). It does not call `SYS_SetArena2Lo`, nor does it obtain the bounds internally. The crate has no production call site for `initialize_wii`; the only other call is its unit test (`:603–618`).

C provides the shared Arena 2 getters/setters under `HW_RVL` (`../../C-sources/libogc/gc/ogc/system.h:410–416`), and `SYS_AllocArena2MemLo` obtains the current low pointer and advances it through `SYS_SetArena2Lo` before returning its allocation (`../../C-sources/libogc/libogc/system.c:1252–1315`). Newlib's `_sbrk_r` also uses the global `SYS_GetArena2Lo`/`SYS_SetArena2Lo` pair for MEM2 allocation and keeps a lower barrier once it transitions into MEM2 (`../../C-sources/libogc/libogc/sbrk.c:25–68`).

So the operation mismatch is confirmed: calling Rust `initialize_wii` alone does not reserve its range from the C/newlib arena. A caller may deliberately apply the returned pointer with `SYS_SetArena2Lo`; because there is no production caller and the API returns the boundary, the sources do not prove that any current runtime use overlaps. That caller-managed protocol is not documented by the helper, while its documentation claims the helper performs the reservation. Keep the contract finding open; describe actual overlap as conditional on a caller using the helper without separately excluding the range from other allocators.

### Interrupt re-entry: original concern resolved for external IRQs

C's LWP heap disables interrupts around initialization and each allocate/free mutation, restoring state on normal and error exits (`../../C-sources/libogc/libogc/lwp_heap.c:9–38,40–95,98–159`). `_CPU_ISR_Disable` maps to `PPCIrqLockByMsr` (`../../C-sources/libogc/gc/ogc/machine/processor.h:83–89`), which saves MSR.EE and clears MSR.EE and MSR.FP (`../../C-sources/libogc/gc/tuxedo/ppc/intrinsics.h:121–132`).

The current Rust allocator uses the same protected lock path for allocator state operations: `lock` disables external interrupt delivery before attempting the atomic spin lock (`crates/core/ogc-alloc/src/lib.rs:385–393,433–449`), and the guard releases the lock before restoring the saved EE state (`:402–410`). `k_irq_disable_all` and `k_irq_restore_all` use `mfmsr`/`mtmsr`, save the prior EE bit, clear EE before lock acquisition, and restore that saved bit afterward (`crates/core/ogc-irq-sys/src/lib.rs:141–157,296–335`). This prevents a maskable external IRQ from re-entering the allocator while interrupted code holds the lock and preserves an already-disabled EE state. The old deadlock scenario is therefore no longer supported for ordinary external IRQ delivery. C also clears FP temporarily; the Rust helper does not, but FP is not interrupt delivery and no floating-point operation occurs in the allocator critical sections, so this does not establish the earlier re-entry deadlock.

The added helper test verifies that interrupt-state acquisition runs before taking the lock and that unlock precedes restoration (`ogc-alloc/src/lib.rs:521–538`). It does not exercise real PowerPC interrupt delivery; the static code path is consistent with C's IRQ-lock behavior for the external interrupt bit.

### Validation and disposition

`cargo test -p ogc-alloc` ran and passed all 6 host unit tests. `cargo check -p ogc-alloc --target powerpc-unknown-linux-gnu` passed, and `cargo check -p ogc-alloc --target powerpc-unknown-linux-gnu --tests` passed (target test code compiled; it was not executed). These checks do not validate the global arena boundary at runtime. No hardware verification was performed. Keep `ogc-alloc` open for the arena-boundary API/documentation contract; the prior conditional interrupt spin-lock finding is resolved by `3f9c2ce`.

## Final reconciliation of grouped fix `bd602f7`

Reviewer: `/root/reviewer_f`. Implementer: `worker_a` (commit author `notOrrytrout`). This section resolves the conflicting bounded dispositions above using the current callback implementation and its documented example.

### `ogc-alloc`: NEEDS_CHANGES for C/newlib boundary publication

Commit `bd602f7` correctly invokes the required setter callback with the heap end after allocator initialization, and its safety contract requires the callback to update the shared Arena 2 boundary (`crates/core/ogc-alloc/src/lib.rs:466–506`). However, the documented example `ogc_system::set_arena2_lo` stores into Rust atomics: it calls `sys::sys_set_arena2_lo` (`crates/core/ogc-system/src/lib.rs:949–951`), which writes private `SYS_ARENA2_LO` (`crates/core/ogc-system-sys/src/lib.rs:186, 3706–3707`). The source contains no call to C `SYS_SetArena2Lo` and no reference to C `__sysarena2lo` in those paths.

C `SYS_SetArena2Lo` writes `__sysarena2lo` (`C-sources/libogc/libogc/system.c:1252–1259`), and `_sbrk_r` reads and updates that shared boundary through `SYS_GetArena2Lo`/`SYS_SetArena2Lo` (`sbrk.c:29–48, 55–62`). An arbitrary callback could call the C setter, but the repository has no production caller proving that it does. Thus the callback order and unsafe contract are sound, but the documented integration does not establish C/newlib boundary publication. Keep `ogc-alloc` open. The earlier reviewer_e text that described pre-fix code and undocumented caller responsibility is stale; the missing shared C-global publication concern remains valid. Reviewer_d's bounded pass was based on the caller contract, but independent inspection of the named setter example shows it does not update C's global.

### `ogc-cache`: PASSED

The `L2Enhance` availability finding is resolved by the same grouped fix. C declares and defines it only under `HW_RVL` (`cache.h:306–320`; `cache.c:140–141`). Rust gates the function and its L2-only helpers with `#[cfg(not(gamecube))]` (`ogc-cache/src/lib.rs:649–717`), matching the repository's Wii and GameCube profiles. The current source contains no other recorded confirmed cache mismatch. This supersedes the earlier reviewer_e `NEEDS_CHANGES` section, which examined source before the gate was added.

Validation for reviewer_f's adjudication was static source inspection; builds and tests were not rerun. The prior reviewer_d report records PowerPC checks passing with target tests compiled but not run, and the two custom target cache checks passing. No hardware verification was performed.

## BUG FIX handoff: publish the allocator boundary to C/newlib

Implementer: `/root/worker_a`. Independent re-review is needed; the review thread handles it. `ogc-system-sys::sys_set_arena2_lo` now updates its Rust mirror and calls the existing C `SYS_SetArena2Lo` symbol on Wii. This uses the public function in `C-sources/libogc/libogc/system.c`; it does not declare or write `__sysarena2lo`, which is private in that C file. The Arena 2 helper still accepts a setter callback, so `ogc-alloc` does not gain a Rust dependency on `ogc-system-sys` and the existing crate dependency direction has no cycle. The documented callback `ogc_system::set_arena2_lo` now publishes to the C/newlib boundary through this path.

The target ABI is checked at compile time against `unsafe extern "C" fn(*mut c_void)`, and the PowerPC `ogc-system-sys` library check passes. The final Wii application must link libogc's C `SYS_SetArena2Lo` implementation; this Cargo workspace does not build that C translation unit, so the PowerPC check does not prove final symbol resolution. `cargo check -p ogc-system-sys --tests --target powerpc-unknown-linux-gnu` remains blocked by existing test-module errors (PowerPC tests reference host-only statics, missing `Vec` imports, and unrelated unsafe calls); test code did not compile and tests did not run. The higher-level `cargo check -p ogc-system --target powerpc-unknown-linux-gnu` is also blocked by existing `ogc-sysbase` `seek_r` i32/i64 type mismatches. A GameCube-configured PowerPC library check passed with `RUSTFLAGS='--cfg=gamecube'` (it emitted existing cfg/dead-code warnings); the Wii-only C symbol is not referenced in that build. `git diff --check` passed. No hardware verification was performed.

### Independent re-review of allocator boundary publication fix `ad5155a`

**Result: NEEDS CHANGES.** Reviewer: `/root/reviewer_e`; implementer: `/root/worker_a`.

The new call reaches the right C function when linked: C `SYS_SetArena2Lo` masks interrupts and writes `__sysarena2lo`, which newlib `_sbrk_r` reads and updates (`C-sources/libogc/libogc/system.c:1252-1259`; `sbrk.c:27-68`). Rust declares and calls it with the correct Wii-only ABI (`crates/core/ogc-system-sys/src/lib.rs:744-749,3718-3723`). Two issues prevent accepting the fix: first, the standalone app build/link configuration does not show that it supplies libogc's C implementation; the attempted storage-probe link stopped on other unresolved C symbols, so it did not establish this symbol's resolution. Second, Rust updates its atomic mirror before calling the C IRQ-protected setter, leaving an interval where Rust sees the new boundary while C/newlib still sees the old one. No guard or documented single-threaded initialization contract was found to exclude a C/newlib allocation during that interval.

Validation: `cargo xtask build-storage-probe` was attempted but failed at final link on unresolved `FindDevice`, `AddDevice`, and `RemoveDevice`; that failure does not isolate `SYS_SetArena2Lo`. The implementer reports a PowerPC library check passed, but that only type-checks. The test target did not compile; tests did not run. No hardware verification was performed.

## BUG FIX handoff: synchronize Arena 2 boundary publication

Implementer: `/root/worker_a`. Independent reviewer: pending. On Wii, `sys_set_arena2_lo` now disables external interrupts, calls C `SYS_SetArena2Lo` first, updates the Rust atomic mirror second, and restores the previous interrupt state. This keeps maskable interrupt-driven allocation from observing the interval between the two updates and preserves an already-disabled interrupt state. The C setter remains responsible for its own nested IRQ save/restore. A focused test checks that interrupt masking surrounds C publication and the subsequent Rust mirror update, in that order.

The final link's resolution of the C `SYS_SetArena2Lo` symbol remains unproven. This change does not resolve that separate integration uncertainty. `cargo check -p ogc-system-sys --target powerpc-unknown-linux-gnu` passed. `cargo check -p ogc-system-sys --tests --target powerpc-unknown-linux-gnu` failed on existing test-module issues, including PowerPC-only tests referencing host-only statics, missing `Vec` imports, and unrelated unsafe-call and callback-ABI errors; tests did not run. `git diff --check` passed. No hardware verification was performed.

### Independent re-review of Arena 2 publication (`3a18c8a` and current HEAD)

**Result: NEEDS CHANGES.** Reviewer: `/root/worker_b`. Implementer: `/root/worker_a`.

The IRQ ordering helper added in `3a18c8a` is internally correct for the Rust mirror: it disables IRQs, stores the new value, then restores the saved state (`crates/core/ogc-system-sys/src/lib.rs:3786–3810`). However, the later commit `e2495b9` removed the C `SYS_SetArena2Lo` call. Current Wii `sys_set_arena2_lo` only updates `SYS_ARENA2_LO`; it does not update C `__sysarena2lo` (`lib.rs:3786–3798`). C `SYS_SetArena2Lo` writes that global (`../../C-sources/libogc/libogc/system.c:1252–1259`), and `_sbrk_r` reads and updates it for MEM2 allocation (`../../C-sources/libogc/libogc/sbrk.c:27–68`). Therefore the documented `ogc-alloc::initialize_wii` callback, when passed `ogc_system::set_arena2_lo`, still does not reserve the same range from C/newlib. The bounded finding is not fixed at current HEAD. Keep `ogc-alloc` open until the real shared boundary is updated or the contract is changed and proven safe for callers.

No checks or tests were run during this independent pass. The earlier handoff records a PowerPC library check passing, target test compilation failing on existing errors, and tests not running; those results do not test the removed C update. No hardware verification was performed.

## 2026-10-05 fix handoff: restore C/newlib Arena 2 boundary update

Implementer: `/root`. `ogc-system-sys::sys_set_arena2_lo` now calls C `SYS_SetArena2Lo` while interrupts are masked, then updates the Rust mirror, then restores the saved interrupt state. This makes the `ogc-alloc::initialize_wii` callback exclude the Rust heap from both Rust and C/newlib Arena 2 allocations. The C call is Wii-only; the GameCube path keeps the Rust mirror behavior. PowerPC checks for `ogc-system-sys` and `ogc-system` passed. `cargo check -p ogc-system-sys --tests --target powerpc-unknown-linux-gnu` remains blocked by existing test-module compile errors; tests did not compile or run. The Cargo workspace does not link the C libogc translation unit, so resolution of `SYS_SetArena2Lo` remains to be confirmed by an application link. Independent static re-review is needed. No hardware verification was performed.

## Independent re-review — current Arena 2 boundary handoff (2026-10-05)

Reviewer: `worker_e`. The allocator callback order is correct within the Rust runtime: `initialize_wii` validates and initializes its local heap, then calls the supplied setter with the heap end (`crates/core/ogc-alloc/src/lib.rs:481-505`). The documented callback is `ogc_system::set_arena2_lo` (`:471-479`; `crates/core/ogc-system/src/lib.rs:964-966`).

The current setter no longer calls C `SYS_SetArena2Lo`. It masks external interrupts and updates only Rust's `SYS_ARENA2_LO` atomic (`crates/core/ogc-system-sys/src/lib.rs:3786-3800`). Rust's Arena 2 getters and allocator use this same atomic (`:3778-3780,3829-3859`), so the Rust-owned boundary is internally consistent. C `SYS_SetArena2Lo` writes C's `__sysarena2lo` (`../../C-sources/libogc/libogc/system.c:1252-1259`), and C `_sbrk_r` reads and updates that boundary for MEM2 allocations (`sbrk.c:27-68`). The current callback therefore does not establish the handoff's claim that C/newlib will avoid the Rust heap if those C allocation paths are linked into the same runtime. No production call site for `initialize_wii` is present, so this review does not establish an actual overlapping allocation.

Result: **NEEDS CHANGES for the documented C/newlib reservation contract; runtime overlap remains unproven.** The earlier source-symbol resolution concern is stale because the current implementation removed the C call. The remaining question is whether the Rust runtime must interoperate with C/newlib's separate arena state or explicitly limits the allocator to a Rust-only runtime. `cargo check -p ogc-alloc --target powerpc-unknown-linux-gnu --tests` passed and compiled target test code; tests were not run. No hardware verification was performed.

## 2026-10-05 Arena2 handoff completion under the Rust-only runtime policy

The earlier review language about updating a separately linked libogc `__sysarena2lo` is superseded by the repository rule that the C tree is reference-only and Rust crates must not depend on unresolved libogc C symbols. `ogc-alloc::initialize_wii` still publishes the reserved heap end through its mandatory setter callback after successful allocator initialization. The intended production callback is `ogc_system::set_arena2_lo`.

The current `ogc-system-sys` implementation provides the `SYS_GetArena2Lo`, `SYS_SetArena2Lo`, `SYS_GetArena2Hi`, `SYS_SetArena2Hi`, `SYS_GetArena2Size`, and `SYS_AllocArena2MemLo` C ABI symbols itself on Wii. These exports and the Rust APIs use the same IRQ-guarded Arena2 atomics. Therefore a linked newlib `_sbrk_r` that calls the public `SYS_*Arena2*` ABI resolves to the Rust-owned symbols and observes the same boundary; no libogc C implementation or private `__sysarena2lo` symbol is required. This satisfies the allocator handoff contract within the Rust-owned runtime.

The allocator interrupt critical-section fix remains present. Independent re-review should verify the callback contract, the Rust-owned C ABI exports, and final-link symbol ownership. No hardware validation is claimed by this handoff.
