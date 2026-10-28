# IRQ wrapper parity review

Status: the `IRQ_Disable`/`IRQ_Restore` wrappers, GameCube ACR runtime rejection, and Wii/GameCube ACR cfg split have passed independent static review. Target checks pass for both console targets. Tests do not compile for the bare-metal targets because their `test` crate is unavailable; no tests ran. No hardware verification was done.

## Scope and source map

Reviewed the full wrapper source `crates/core/ogc-irq/src/lib.rs` (lines 1–80) and its manifest. The dependency `crates/core/ogc-irq-sys/src/lib.rs` was already reviewed as the low-level binding, but its public wrapper call paths and corresponding cause/mask logic were followed here (lines 14–48, 145–233, 387–471, 473–729).

The public C wrapper API is in `../../C-sources/libogc/gc/ogc/irq.h` (lines 49–125). Its `KIrq*` types and declarations are in `../../C-sources/libogc/gc/tuxedo/interrupt.h` (lines 8–103); hardware implementation and interrupt cause/mask behavior are in `../../C-sources/libogc/tuxedo/interrupt.c` (lines 8–245). The exception vector setup path is `../../C-sources/libogc/tuxedo/common_crt0.S` (lines 62 onward), `../../C-sources/libogc/tuxedo/interrupt_asm.S` (entry, register save/restore, and dispatch call), and `../../C-sources/libogc/tuxedo/exception.c` plus `gc/tuxedo/ppc/exception.h`. The C time/decrementer path enters `KIrqDispatch` and calls `KTickTaskRun` (`interrupt.c:96–110`; implementation in `tuxedo/tick.c:74 onward`). Rust's wrapper delegates to `ogc-irq-sys`; the IRQ exception vector and registered dispatcher are in `crates/abi/ogc-sys/src/lib.rs` and are also connected from `ogc-irq-sys/src/lib.rs:12, 145–184`.

## Historical API mismatch, resolved by target cfg split

- C defines `IM_PI_ACR` and includes `IRQ_PI_ACR` in the `KIrqId` enum only under `__wii__` (`gc/tuxedo/interrupt.h:47–52, 84–87`). This initially differed from Rust's single shared PowerPC build. Commit `a573618` added the explicit `cfg(gamecube)` target selection, and the current Rust wrapper and sys crate gate the public ACR constants with `#[cfg(not(gamecube))]` (`crates/core/ogc-irq/src/lib.rs:17–20`; `ogc-irq-sys/src/lib.rs:46–53`). The current Nintendo-none target is the Wii mode; `powerpc-nintendo-gamecube.json` enables `cfg(gamecube)` through `.cargo/config.toml`. Thus the public constants are present for the Wii target and absent for GameCube.

No unintended mismatch was confirmed for the wrapper functions that have C equivalents.

### BUG FIX: add IRQ disable and restore wrappers

`IRQ_Disable` and `IRQ_Restore` were missing from the Rust public wrapper. C saves the MSR EE bit, clears EE to disable external IRQ delivery, and later restores EE from the saved level (`gc/ogc/irq.h:95–115`; `gc/tuxedo/ppc/intrinsics.h:121–132`). Added `irq_disable() -> u32` and `irq_restore(level)` to `ogc-irq`; these delegate to the low-level MSR save/restore helpers in `ogc-irq-sys`. The implementation saves and restores the prior EE state while preserving unrelated current MSR bits. C's lock helper also clears FP, but its unlock helper does not restore the previous FP state. Rust leaves FP unchanged to avoid this side effect. Regression coverage checks the saved EE state and both enabled/disabled restore cases. Implementer: `/root/worker_a`.

Independent re-review: `worker_b` compared the complete IRQ change in `d7dae19` with C `IRQ_Disable`, `IRQ_Restore`, `PPCIrqLockByMsr`, and `PPCIrqUnlockByMsr`. The Rust code saves the EE bit, clears EE, and restores EE from the saved value while preserving the other current MSR bits. The regression tests cover both initial EE states. Rust leaves FP unchanged. C clears FP in the lock helper but does not restore its prior value; the C public contract describes disabling IRQ delivery and does not document an FP state change. The Rust behavior is an intentional correction of that side effect. The separate GameCube ACR exposure finding remains open, so both IRQ ledger rows remain `pending`/`finding`.

## API mapping and behavior

| Rust wrapper | C API / behavior |
| --- | --- |
| `request(id, handler, user)` (`ogc-irq/src/lib.rs:19–24`) | `IRQ_Request` calls `KIrqSet` and returns the previous callback (`gc/ogc/irq.h:57–68`; `tuxedo/interrupt.c:235–245`). Rust adds an `unsafe` contract that `user` outlive possible callbacks. |
| `free(id)` (`lib.rs:26–28`) | `IRQ_Free` calls `KIrqSet(id, NULL, NULL)` and returns the previous callback (`irq.h:71–80`). |
| `get_handler(id)` (`lib.rs:30–32`) | `IRQ_GetHandler` calls `KIrqGet` (`irq.h:83–92`; `interrupt.c:230–233`). |
| `enable(mask)` / `unmask(mask)` (`lib.rs:34–48`) | `__UnmaskIrq` / `KIrqEnable` (`irq.h:122–125`; `interrupt.c:220–223`). |
| `disable(mask)` / `mask(mask)` (`lib.rs:38–44`) | `__MaskIrq` / `KIrqDisable` (`irq.h:117–120`; `interrupt.c:225–228`). |
| `irq_disable()` / `irq_restore(level)` | `IRQ_Disable` / `IRQ_Restore`; save and restore the MSR EE bit (`irq.h:95–115`; `intrinsics.h:121–132`). |
| `clear(id)` (`lib.rs:50–52`) | `KIrqClear`, equivalent to `KIrqSet(id, NULL, NULL)` (`gc/tuxedo/interrupt.h:100–103`). |
| Re-exported `KIrqId`, handler type, and named IDs (`lib.rs:8–15`) | C enum order and callback ABI in `gc/tuxedo/interrupt.h:56–90`; numeric values agree for common IDs. The Wii-only ACR exception is the mismatch listed above. |

Rust additionally exposes `dispatch_pending()` and `restore_exception_vector()` (`ogc-irq/src/lib.rs:54–62`). These have no corresponding `ogc/irq.h` API. They are runtime controls: the former runs the low-level pending cause decoder and handler table, while the latter calls the Rust vector restore helper and resets Rust runtime-initialization state (`ogc-irq-sys/src/lib.rs:709–729`).

The original review found no `IRQ_Disable` / `IRQ_Restore` wrapper. The later `irq_disable()` and `irq_restore(level)` additions above provide this behavior (`ogc-irq/src/lib.rs:44–52`; C `gc/ogc/irq.h:95–115`).

## Call-path comparison

- **Registration state:** C stores `(fn,user)` pairs in `s_irqCookies[IRQ_MAX]`, and `KIrqSet` swaps the pair with external interrupts masked (`tuxedo/interrupt.c:8–13, 235–245`). Rust stores handlers and user pointers in atomics and masks external interrupts around the two updates on PowerPC (`ogc-irq-sys/src/lib.rs:136–139, 682–703`). The wrapper returns the previous handler in both cases. Invalid IDs are unchecked in C and may index outside the array; Rust validates IDs and returns `None`/no change (`ogc-irq-sys/src/lib.rs:200–204, 677–703`). This is Rust bounds hardening.
- **Mask operations:** The wrapper forwards masks unchanged to the Rust backend, where `k_irq_enable`/`k_irq_disable` serialize register edits with MSR external IRQ masking and update a software mask (`ogc-irq-sys/src/lib.rs:648–675`). C `KIrqEnable`/`KIrqDisable` call `KIrqModify`, which saves/restores MSR and updates the corresponding MI, DSP, AI, EXI, and PI registers (`tuxedo/interrupt.c:112–228`). Register encodings, preserved bits, and update order match on supported IRQs (`interrupt.c:161–217`; Rust `ogc-irq-sys/src/lib.rs:387–527`). Rust also stores/applies enabled masks during lazy initialization (`ogc-irq-sys/src/lib.rs:158–184, 193–195`); C initializes the baseline in `KIrqInit` during startup (`interrupt.c:18–33`).
- **Cause priority:** Rust's `pending_cause` mirrors `KIrqGetCause`'s priority over PI error/debug, memory, PI RSW/VI/token/finish/HSP, DSP/AI, EXI, SI/DI, DSP AI, CP, and Wii ACR (`tuxedo/interrupt.c:35–94`; Rust `ogc-irq-sys/src/lib.rs:531–629`). The MI trailing-zero selection matches C's `__builtin_ffs - 1`. EXI channel offsets and removal of the EXI2 external cause match.
- **Callback dispatch:** C `KIrqDispatch` calls `KTickTaskRun` for decrementer exceptions and otherwise invokes the selected IRQ callback with `(id,user)` (`tuxedo/interrupt.c:96–110`). Rust routes external IRQs from its installed exception vector through a registered callback to `k_irq_dispatch_pending` (`ogc-irq-sys/src/lib.rs:145–153, 713–720`); its decrementer path uses separate dispatch hooks in `ogc-sys`. This is a different internal split with the same intended distinct IRQ/decrementer routing. The C exception assembly and Rust exception-vector runtime were not compared instruction-for-instruction.

## Rust-only and safer behavior

- Rust's low-level interrupt functions lazily install and initialize its exception runtime the first time `set` or `enable` is called (`ogc-irq-sys/src/lib.rs:158–184, 648–695`). C performs `KIrqInit` during startup (`tuxedo/common_crt0.S:62 onward`; `tuxedo/interrupt.c:18–33`). This shifts initialization timing and is Rust-runtime-specific.
- Rust validates IRQ IDs before indexing; C's `KIrqGet`/`KIrqSet` index directly (`interrupt.c:230–245`; Rust `ogc-irq-sys/src/lib.rs:200–204, 677–703`).
- Rust exports `mask`/`unmask`, `clear`, `dispatch_pending`, and vector restoration convenience APIs. C has `__MaskIrq`, `__UnmaskIrq`, and `KIrqClear` macros/inlines, but no manual pending-dispatch or restore-vector API in `irq.h`.
- The Rust handler callback type is `unsafe extern "C" fn(KIrqId, *mut c_void)` and represents null as `Option`; this matches C's nullable function pointer ABI (`interrupt.h:90`).

## Uncertain or unchecked

- No hardware IRQ was triggered. Register write-one-to-clear effects, priority under simultaneous causes, and edge timing were not verified on Wii/GameCube.
- The C path's `PPCExcptIrqHandler` assembly, exception table installation, and restoration behavior were not compared instruction-by-instruction against Rust's generated exception stubs. Only the call topology was checked (`tuxedo/interrupt_asm.S`, `tuxedo/exception.c`, `ogc-sys/src/lib.rs`, `ogc-irq-sys/src/lib.rs:145–184`).
- Rust lazy initialization uses `IRQ_RUNTIME_INITIALIZED.swap(true)` before the setup writes complete (`ogc-irq-sys/src/lib.rs:158–184`). A concurrent first-time `set`/`enable` can return while the first caller is still initializing hardware. This race was noted, but was not reproduced or classified as a confirmed mismatch in this wrapper review.
- Rust target selection is now explicit: public ACR names and code exist only when `cfg(gamecube)` is false, while runtime get/set still rejects IRQ 27 if a Wii-mode build runs on a GameCube (`ogc-irq-sys/src/lib.rs:224–245,733–740`).
- `IRQ_Disable`/`IRQ_Restore` wrapper parity passed independent review; see the re-review section below.
- The wrapper test checks that public function signatures exist (`ogc-irq/src/lib.rs:76–96`). The low-level MSR state transformation regression test is present in `ogc-irq-sys/src/lib.rs`, but could not run: the PowerPC target has no `test` crate and the host build is rejected by the crate's PowerPC-only compile guard. PowerPC target `cargo check` passed. No hardware verification was performed.

### Independent re-review of IRQ disable and restore

Reviewer: `worker_b`. Implementer: `/root/worker_a`. Implementation commit: `d7dae19` (the IRQ changes in this commit; its subject refers to a separate ES task).

The added wrappers pass independent static review. C `IRQ_Disable` returns `PPCIrqLockByMsr()` and `IRQ_Restore` calls `PPCIrqUnlockByMsr(level)` (`../../C-sources/libogc/gc/ogc/irq.h:95–115`; `../../C-sources/libogc/gc/tuxedo/ppc/intrinsics.h:121–132`). The C lock saves only `MSR_EE` and clears `MSR_EE|MSR_FP`; unlock reads the current MSR, clears `MSR_EE`, then restores the saved EE bit. Rust `irq_disable` / `irq_restore` delegate to `k_irq_disable_all` / `k_irq_restore_all` (`crates/core/ogc-irq/src/lib.rs:44–52`), which read the current MSR, save only EE, and restore EE while preserving all other current bits (`crates/core/ogc-irq-sys/src/lib.rs:285–312`). This matches the IRQ enable state contract. Rust leaves FP unchanged, an intentional correction of C's undocumented FP side effect; no unrelated MSR bits are overwritten.

Coverage includes public wrapper signature checks and pure state transformation assertions for both EE states (`ogc-irq/src/lib.rs:76–96`; `ogc-irq-sys/src/lib.rs:845–865`). The tests do not execute privileged MSR instructions, but directly check the transformation logic used by those instructions. `cargo check -p ogc-irq -p ogc-irq-sys --tests --target powerpc-unknown-linux-gnu` passed; this compiled tests but did not run them. `rustfmt --edition 2021 --check crates/core/ogc-irq/src/lib.rs crates/core/ogc-irq-sys/src/lib.rs` reported pre-existing formatting differences at the crate-level `compile_error!` attributes and `decode_handler`; no implementation changes were made during re-review. No hardware verification was performed.

### BUG FIX: reject the ACR IRQ ID on GameCube

C includes `IRQ_PI_ACR` in `KIrqId` and defines `IM_PI_ACR` only for Wii (`../../C-sources/libogc/gc/tuxedo/interrupt.h:47–52, 84–88`). The Rust workspace has one shared PowerPC target and selects the hardware at runtime, so its public constants remain visible in both console builds. The low-level getter and setter now validate ID 27 against the detected console before indexing the handler table. On GameCube, both return `None`; on Wii, IRQ 27 remains valid. The `ogc-irq` wrapper delegates to these paths. The mask path already filtered the ACR bit on GameCube.

Regression coverage checks that the shared ID-validation helper rejects IRQ 27 for GameCube while accepting it for Wii and preserving a common IRQ on GameCube (`ogc-irq-sys/src/lib.rs` test `acr_irq_id_is_rejected_on_gamecube`). The test compiles for PowerPC but cannot run in this environment. Exact compile-time visibility of `IM_PI_ACR` and `IRQ_PI_ACR` cannot match C until the project adds explicit Wii/GameCube build modes; this fix changes runtime behavior and does not add such a configuration system. Implementer: `/root/worker_c`.

### Independent re-review of GameCube ACR runtime rejection

Reviewer: `reviewer a` (primary reviewer). Implementer: `/root/worker_c`. Implementation commit: `be72ab9`.

C includes IRQ 27 in its enum only for Wii (`../../C-sources/libogc/gc/tuxedo/interrupt.h:84–88`). Rust `index_for_console` now rejects ID 27 when the detected console is GameCube, and `k_irq_get` / `k_irq_set` return without accessing the handler slot (`crates/core/ogc-irq-sys/src/lib.rs:224–240, 716–725`). Wii keeps the ID. The ACR mask was already filtered for GameCube. This runtime correction matches the hardware-specific behavior checked in C. The public constants still differ at compile time, so the broader finding remains open; no additional implementation change is requested for this fix.

The PowerPC target check compiled the regression tests; they did not run. No hardware verification was performed.

### Independent re-review: Wii/GameCube ACR cfg and runtime behavior

- Result: **PASSED** for the `a573618` target cfg correction and the `be72ab9` runtime rejection. This finding is resolved at source level.
- Reviewer: `/root/reviewer_e`
- Implementer: `notOrrytrout`
- Commits: `a573618` (`Apply parity and target cfg patches`), including the IRQ cfg split; `be72ab9` (`Reject ACR IRQ registration on GameCube`), adding runtime ID validation.

C `interrupt.h` puts `IM_PI_ACR` and the `IRQ_PI_ACR` enum value under `__wii__`, and excludes them in the GameCube mode (`../../C-sources/libogc/gc/tuxedo/interrupt.h:47-52,84-88`). C's handler storage is sized by the per-mode `IRQ_MAX` enum sentinel (`../../C-sources/libogc/tuxedo/interrupt.c:13`). Rust now reproduces the public API split: `ogc-irq-sys` only exports `IRQ_PI_ACR` outside `cfg(gamecube)`, sizes its handler/user arrays to 27 entries for GameCube (`IRQ_PI_CP + 1`) and 28 for Wii (`IRQ_PI_ACR_VALUE + 1`), and the wrapper only exports `IRQ_PI_ACR`/`IM_PI_ACR` in Wii mode (`crates/core/ogc-irq-sys/src/lib.rs:46-53,159-162`; `crates/core/ogc-irq/src/lib.rs:17-20`). The checked-in GameCube target has EABI env and `.cargo/config.toml` selects `cfg(gamecube)` for it; the Nintendo-none target has no GameCube cfg and is the Wii build (`targets/powerpc-nintendo-gamecube.json`; `.cargo/config.toml`; `README.md:15`).

The register behavior also follows the C guards. C's Wii `IM_PI` includes the ACR mask and `KIrqEncodePi` maps it to PI bit 14; C GameCube omits both (`interrupt.h:47-52`; `tuxedo/interrupt.c:142-158`). Rust composes `IM_PI` with the ACR bit only outside GameCube cfg, and compiles the bit-14 PI encoder branch only in that mode (`ogc-irq-sys/src/lib.rs:124-139,489-525`). A Wii-mode binary additionally filters the requested ACR mask by runtime console detection (`:247-274`), so it does not enable the reserved GameCube bit. C's pending decoder checks PI cause bit 14 only under `__wii__` (`interrupt.c:89-91`); Rust compiles the corresponding check only outside GameCube cfg and also requires runtime Wii detection (`ogc-irq-sys/src/lib.rs:600-685`). The other PI bits, mask encodings, and cause priority are unchanged by this fix and remain aligned with the reviewed C logic above (`interrupt.c:44-94,142-158`; Rust `:489-525,600-685`).

Runtime ID rejection from `be72ab9` remains active. `index` checks the detected console and calls `index_for_console`; that helper rejects ID 27 on GameCube and then bounds-checks against the build's `IRQ_COUNT` (`ogc-irq-sys/src/lib.rs:223-245`). Both getter and setter use this index before reading or writing the table (`:733-758`). On a Wii-mode build ID 27 is accepted only when runtime detection reports Wii; on a GameCube build the 27-entry storage bound rejects it even if the helper is given `wii = true`. This matches C's per-console table size and guard behavior. The exact public constants are now compile-time gated, so the earlier compile-time mismatch is resolved.

The existing `acr_irq_id_is_rejected_on_gamecube` regression covers ID rejection and the build-specific helper branches (`ogc-irq-sys/src/lib.rs:849-860`), but is not hardware-backed. `cargo check -p ogc-irq -p ogc-irq-sys` passed for both actual targets: `powerpc-nintendo-none.json` and `powerpc-nintendo-gamecube.json`, using nightly build-std. Both checks emit `unexpected_cfgs` warnings for `gamecube` because the target-specific rustflags do not include the global check-cfg declaration; the cfg selection itself is applied. `cargo check ... --tests` was attempted for both targets and failed in each because these no-OS target specs do not provide the `test` crate (`E0463: can't find crate for test`). Therefore the tests neither compiled nor ran on either target. No hardware verification was performed. The unrelated GameCube `ogc-ipc` dependency failure is not caused by the IRQ cfg change and is outside this IRQ result.
