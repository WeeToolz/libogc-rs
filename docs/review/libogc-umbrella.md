# ogc compatibility umbrella static review

Reviewer: `thread-current/reviewer_4`  
Pass: first-pass static review  
Scope: public module re-exports, umbrella dependency surface, and platform guards. This pass does not review the subsystem implementations behind each re-export.

## Confirmed mismatches

### The umbrella omits the memory-card subsystem

- C behavior: `../../C-sources/libogc/gc/gccore.h:39-66` unconditionally includes `ogc/card.h` as part of the core public umbrella.
- Rust behavior: `crates/compatibility/ogc/Cargo.toml:8-42` has no `ogc-card` dependency, and `crates/compatibility/ogc/src/lib.rs:6-39` has no `card` re-export. A separate `ogc-card` crate exists at `crates/compatibility/ogc-card/`.
- Result: a consumer using the `ogc` umbrella cannot reach the Rust memory-card API through the umbrella, unlike a C consumer including `gccore.h`.

### Wii-only modules are exported for GameCube targets

- C behavior: `gccore.h:39-66` exposes common GameCube/Wii interfaces, then lines 68-82 place IPC, SHA, AES, ES, STM, IOS, USB, ISFS, CONF, USB storage, and Wii launch headers inside `#if defined(HW_RVL)`.
- Rust behavior: `ogc/src/lib.rs:3-4` only rejects non-PowerPC architectures. It unconditionally re-exports `aes`, `es`, `ios`, `ipc`, `stm`, `usb`, `usbstorage`, `conf`, `wiilaunch`, and related modules at lines 6-39, with no Wii/GameCube hardware feature guard in the crate manifest or source.
- Result: on a GameCube PowerPC target, the Rust umbrella exposes modules that the C umbrella excludes unless `HW_RVL` is defined. The current guard distinguishes PowerPC from non-PowerPC but not Wii from GameCube.

## Coverage and limits

The C umbrella also includes `arqueue.h`, `arqmgr.h`, `cast.h`, `color.h`, `consol.h`, `gx_struct.h`, `pad.h`, `tpl.h`, `video_types.h`, and `texconv.h` (`gccore.h:39-66`). The Rust umbrella re-exports some related APIs under subsystem crates, such as `input` and `video`, but does not provide a one-to-one set of C header/module names. I did not record these as separate bugs where this workspace has no direct crate counterpart. This review does not compare individual subsystem APIs.

## Review state

The crate remains open with confirmed umbrella-surface mismatches: the missing memory-card API, cast utilities, and color constants, plus Wii-only modules exposed on GameCube PowerPC. No implementation files were changed. No build, test, or hardware verification was performed, as instructed.

## Additional include and export inventory

C's common include group (`gccore.h:39-66`) has Rust umbrella coverage through `dsp`, `aram`, `audio`, `cache`, `dvd`, `exi`, `gu`, `gx`, `si`, `irq`, `lwp`, `system`, `video`, `usbgecko`, and `input` (for PAD). `ogc-aram` includes the ARQ and ARQ manager APIs under `aram::arq`; `ogc-video` includes console facilities; `ogc-gx` includes TPL and `make_texture565` (the `texconv.h` helper). C's GX structs and video mode types are represented by Rust's GX/video modules. These are namespace/API-shape differences, not missing whole subsystem implementations in the checked surface.

The complete C umbrella macro/API surface also exposes two additional groups not found in the Rust workspace search:

- `color.h:7-26` defines 20 packed YUV color constants (`COLOR_BLACK` through `COLOR_MEDGRAY`). There are no matching public Rust constants.
- `cast.h:6-22,33-88,97-151,159-233` defines GQR register/type/cast constants, `CAST_Init`, and GQR setters; it also supplies typed integer/float conversion inline helpers. No corresponding public Rust cast module, constants, or helpers were found. These are explicit C umbrella API omissions.

`gccore.h:87-98` exposes the RNC error constants and attribute macros. Rust's `ogc::core::sys` contains the same RNC error values (`ogc-core/src/lib.rs:55-59`); alignment/packing are expressed with Rust representation attributes rather than preprocessor macros, so those macros do not have a direct Rust module export.

Rust's `ogc/src/lib.rs:6-39` has no `cfg` per re-export. In addition to the Wii-only APIs gated by `HW_RVL` in C (`gccore.h:68-82`), it also exports `wiisd` unconditionally (`lib.rs:39`); the C Wii internal SD interface lives in `gc/sdcard/wiisd_io.h`, which is separate from the GameCube/Wii common umbrella. The PowerPC-only guard at `lib.rs:3-4` cannot distinguish GameCube from Wii. Other Rust-only umbrella additions such as `bt_sync`, `gcsd`, `network`, and `wd` are not listed as C omissions because they extend the surface rather than replace an included C header.

## 2026-10-02 umbrella/API-surface implementation

The confirmed umbrella findings are addressed in the current source.

- `ogc-card` is now a dependency and is re-exported as `ogc::card`, restoring the memory-card subsystem present in `gccore.h`.
- `ogc::cast` implements the public GQR constants, initialization/setters, and integer/f32 paired-single cast helpers from `ogc/cast.h`.
- `ogc::color` contains all 20 packed YUV constants from `ogc/color.h`; the constants are also re-exported at the umbrella root.
- Wii-only umbrella modules are cfg-gated from the GameCube target. Common GameCube/Wii modules remain visible.

The C headers `arqueue.h`, `arqmgr.h`, `consol.h`, `pad.h`, `tpl.h`, `gx_struct.h`, `video_types.h`, and `texconv.h` continue to map into existing Rust subsystem modules rather than receiving duplicate facade crates: ARQ lives under `aram::arq`, console/video types under `video`, PAD under `input`, and TPL/texture helpers and GX structures under `gx`. This is retained as a namespace-shape difference, not a missing implementation.


## 2026-10-02 compiled-list verification follow-up

The compiled baseline contains the previously implemented memory-card export, cast/GQR helpers, all 20 C color constants, and Wii/GameCube platform gating. A final direct-name gap was found during re-audit: C `gccore.h` includes `ogc/isfs.h`, while Rust exposed the same ISFS implementation only as `ogc::filesystem`. The facade now also re-exports that crate as `ogc::isfs`; `filesystem` remains as the existing Rust-facing name.

The other C umbrella headers continue to map into existing subsystem modules without duplicate implementations: ARQ under `aram::arq`, mutex/message/semaphore under `lwp`, console/video types under `video`, PAD under `input`, and TPL/GX structures/texture helpers under `gx`. No additional missing whole subsystem was found in this `gccore.h` recheck.

## Independent re-review of umbrella fixes (2026-10-03)

Reviewer: `/root/reviewer_f`
Implementer: `notOrrytrout` (commit author/committer)
Fix commits: `966aef9`, `59a3105`, `e694fd2`
Result: **NEEDS_CHANGES** for GameCube target gating. The card, cast, color, and direct ISFS facade additions pass the bounded source comparison below.

### Passed API additions

- C `gc/gccore.h:45` includes `ogc/card.h` in its common header group. Rust adds `ogc-card` at `crates/compatibility/ogc/Cargo.toml:15` and exports it as `card` at `src/lib.rs:13` without a Wii-only guard, matching the common C availability.
- C `gc/ogc/cast.h:6-22,32-88` defines GQR numbers, type/cast selectors, and the scale mask; lines 33-50 initialize GQR2-5, and lines 54-88 repeat the type/scale field into both halves for setters 2-7. Rust `crates/compatibility/ogc/src/cast.rs:3-24,93-111` has the same values and selector mapping; `:116-124` initializes only 2-5; `:98-100,126-136` builds and writes the same repeated field to setters 2-7. Its paired-single helpers use the corresponding selectors and W=1 (`:32-88`), matching C's helper assembly in `cast.h:97-233`. The Rust facade uses lower-case Rust function names and output pointers instead of C's inline return-value helper spelling; the conversion operations themselves match.
- C `gc/ogc/color.h:7-26` defines 20 packed YUV constants. Rust `crates/compatibility/ogc/src/color.rs:1-20` defines the same 20 names and numeric values, and `src/lib.rs:60-62` exports the module and constants at the umbrella root.
- C `gc/gccore.h:68-82` includes ISFS only under `HW_RVL`. The Rust `isfs` alias added by `59a3105` is gated by `#[cfg(not(gamecube))]` at `src/lib.rs:26-27`, like the existing `filesystem` alias at `:24-25`.

### Remaining GameCube target mismatch

C `gc/gccore.h:68-82` excludes IPC and other Wii-only headers when `HW_RVL` is absent. Rust gates the public `ipc` re-export at `crates/compatibility/ogc/src/lib.rs:34-35`, but `ogc-ipc` remains an unconditional dependency in `crates/compatibility/ogc/Cargo.toml:35`. Cargo therefore still builds that dependency for the GameCube facade target. In the actual GameCube-target check, `crates/core/ogc-ipc/src/lib.rs:1124-1134` compiles `install_interrupt_handler` using `irq::IRQ_PI_ACR` and `irq::IM_PI_ACR`, while `crates/core/ogc-irq/src/lib.rs:17-20` correctly removes those Wii-only symbols under `cfg(gamecube)`. The check fails with E0425 for both names before it can check the full `ogc` crate. The manifest must avoid building the Wii-only dependency for this target, or the dependency itself must fully guard its Wii-only path, before the target-gating fix matches the C availability.

This is a concrete target build failure, not only an export-name difference. `e694fd2` attempted to make the STM dependency target-conditional with `target_env = "gamecube"`, but the custom GameCube target declares `env = "eabi"`; subsequent source now has STM unconditional again at `Cargo.toml:42`. The other Wii-only dependencies, including IPC, are also unconditional. Current `src/lib.rs` uses the custom cfg for public re-exports, but this does not remove dependencies from Cargo's build graph.

### Validation and coverage

- Direct C comparison: `gc/gccore.h`, `gc/ogc/cast.h`, and `gc/ogc/color.h`, with current Rust facade and manifest sources.
- `cargo check -p ogc` for `powerpc-nintendo-none.json` completed with the nightly JSON-target/build-std invocation. It emitted `unexpected_cfgs` warnings for the custom `gamecube` cfg because target-specific rustflags omit the check-cfg declaration.
- The same check for `powerpc-nintendo-gamecube.json` failed during dependency compilation at `ogc-ipc` with E0425 for `IRQ_PI_ACR` and `IM_PI_ACR`. This is the concrete remaining mismatch above.
- No umbrella-specific regression tests were found for the card re-export, GQR conversion helpers, color values, ISFS alias, or GameCube facade gating. No tests were run; the Wii target was checked, and the GameCube target check failed. No hardware verification was performed.

At the time of the preceding re-review, the crate remained open pending correction of the GameCube dependency/build path and an independent re-review. The current re-review below verifies the implementation changes since that report.

## Independent re-review: GameCube system entry gates (2026-10-03)

Reviewer: `/root/reviewer_b`
Implementer: `worker_c` (commit author `notOrrytrout`)
Implementation commit: `4521b52` (`Gate Wii system entry paths on GameCube`)
Result: **PASSED** for the recorded GameCube build-path finding.

I checked the system entry changes against C `libogc/system.c`. C's `SYS_PreMain` performs IOS startup, STM registration, CONF initialization, and Wii initialization only under `#if defined(HW_RVL)` (`system.c:999-1015`), then runs the common boot-time setup. Rust `ogc-system-sys::sys_pre_main` now compiles the Wii startup path only under `#[cfg(not(gamecube))]` and still calls common `set_boot_time()` (`crates/core/ogc-system-sys/src/lib.rs:1722-1740`). Thus the GameCube target skips Wii-only startup and retains common boot-time setup.

C has separate reset implementations: the `HW_DOL` implementation is GameCube behavior, while the `HW_RVL` implementation handles Wii reset/menu/power paths (`system.c:1040-1080,1082-1170`). Rust now excludes `wii_reset_system` on GameCube and dispatches directly to `gamecube_reset_system`; the Wii-capable target retains the Hollywood revision dispatch (`crates/core/ogc-system-sys/src/lib.rs:1934-1999,2025-2100`). This matches the target-specific C path and leaves runtime dispatch available on the combined Wii target.

The umbrella's Wii-only public re-exports remain guarded by `#[cfg(not(gamecube))]` (`crates/compatibility/ogc/src/lib.rs:6-58`). Its manifest still has unconditional dependencies for some Wii-only crates, including `ogc-ipc`, but these are not public umbrella re-exports. The relevant IPC ACR implementation is itself gated from GameCube (`crates/core/ogc-ipc/src/lib.rs:1128`), while the IOS facade and implementation use crate-level `#![cfg(not(gamecube))]` (`crates/core/ogc-ios/src/lib.rs:2`, `crates/core/ogc-ios-sys/src/lib.rs:2`). The prior GameCube `ogc-ipc` unresolved ACR-symbol build failure no longer reproduces.

Validation: nightly `cargo check -p ogc` passed for both `targets/powerpc-nintendo-gamecube.json` and `targets/powerpc-nintendo-none.json`, with `-Z build-std=core,alloc -Z json-target-spec`. I set `RUSTC` to the nightly compiler because the shell's default `cargo` resolves `rustc` to stable. Both checks emitted existing `unexpected_cfgs` warnings for the custom `gamecube` cfg. These were compile checks only; no tests ran and no hardware verification was performed.

This pass accepts the implementation of the recorded target gating/build-path mismatch. The earlier C surface additions were independently accepted by reviewer_f, and the include inventory found no additional omissions. Together these reviews resolve the confirmed umbrella findings; subsystem implementation findings remain outside this umbrella scope.

## GameCube build fix handoff — IOS API availability

Implementer: `worker_c`. Status: fixed and independently re-reviewed by reviewer_b on 2026-10-03.

C compiles the complete IOS implementation only under `#if defined(HW_RVL)` (`../../C-sources/libogc/libogc/ios.c:31` through its closing guard). Rust previously compiled `ogc-ios-sys` and its public `ogc-ios` facade for GameCube even though `ogc::ios` was hidden there. The raw IOS implementation uses the Wii-only ACR interrupt constants, so the hidden facade re-export did not prevent dependency compilation. Both Rust crates now use `#![cfg(not(gamecube))]`, matching C's Wii-only API availability.

Validation: nightly `cargo check -p ogc-ios -p ogc-ios-sys` passed for both GameCube and Wii target JSONs using `-Z build-std=core,alloc -Z json-target-spec`. At that revision, the full GameCube `cargo check -p ogc` progressed past these IOS crates, then failed in `ogc-system-sys` because its separate IOS call sites were not yet GameCube-gated (`lib.rs:1728-1729,1990`). Commit `4521b52` later gated those startup and reset call sites; reviewer_b confirmed the GameCube and Wii umbrella checks now pass. The checks reported existing `unexpected_cfgs` warnings for `cfg(gamecube)`. No tests or hardware verification were performed.

## Independent follow-up: remaining `gccore.h` inventory

- Result: **NO ADDITIONAL INVENTORY FINDINGS**. At the time of this follow-up, the GameCube dependency build mismatch remained open; the later gates in `4521b52` resolve it, as independently checked by reviewer_b below.
- Reviewer: `/root/reviewer_e`
- Implementer of prior API additions: `notOrrytrout` (`966aef9`, `59a3105`); the earlier GameCube gate was implemented in `e694fd2`.
- Scope: direct `gccore.h` includes and their current Rust facade destinations, after the card/cast/color/ISFS additions.

I rechecked the complete include groups in C `../../C-sources/libogc/gc/gccore.h:39-82` against `crates/compatibility/ogc/src/lib.rs:6-62` and its manifest. Every common include now has a Rust public path: DSP, ARAM and ARQ (`dsp`, `aram`, `aram::arq`); audio, cache, card, cast and color (`audio`, `cache`, `card`, `cast`, `color`); console and video (`video`); DVD, EXI, GU, GX, SI and IRQ; LWP mutex/message/semaphore APIs (`lwp`); PAD (`input`); TPL, GX structs, video types and texture conversion (`gx`/`video`); system and USB Gecko. The Wii-only include group also has corresponding `aes`, `es`, `ios`, `ipc`, `sha`, `stm`, `usb`, `isfs`, `conf`, `usbstorage`, and `wiilaunch` facade paths. The `RNC_*` constants and `ATTRIBUTE_ALIGN`/`ATTRIBUTE_PACKED` concepts are represented as already documented above.

This audit found no additional whole-header omission or unsupported direct C include name after the prior additions. The nested mappings are not identical Rust module names, but the relevant public modules/functions are present: `ogc-aram` declares `pub mod arq` (`crates/hardware/ogc-aram/src/lib.rs:47`); LWP re-exports raw synchronization types (`crates/core/ogc-lwp/src/lib.rs:8-13`); video contains public console APIs (`crates/hardware/ogc-video/src/lib.rs:657-681,878-899`); GX contains `make_texture565` (`crates/hardware/ogc-gx/src/lib.rs:3193`); and `ogc-core` re-exports RNC constants at its crate root (`crates/core/ogc-core/src/lib.rs:87-96`).

The facade also exposes extra modules (`bt_sync`, `di`, `gcsd`, `network`, `wd`, and `wiisd`) that are not direct `gccore.h` includes. These are additive Rust crate APIs, not omissions or replacements for an included C header. The `filesystem` name is also an additional alias to the Wii-gated ISFS implementation. C's `HW_RVL` group is gated in Rust exports with `#[cfg(not(gamecube))]` (`ogc/src/lib.rs:6-7,14-18,21-27,32-44,47-58`); all common C groups remain unconditional. This confirms the public export inventory statically but does not establish that the whole GameCube facade builds.

### Validation

I ran the nightly GameCube target compile command with the checked-in target JSON and build-std settings. Compilation still fails in the unconditional `ogc-ipc` dependency because it uses the Wii-only `irq::IRQ_PI_ACR` and `irq::IM_PI_ACR`, which `ogc-irq` gates out for `cfg(gamecube)` (`crates/core/ogc-ipc/src/lib.rs:1124-1134`; `crates/core/ogc-irq/src/lib.rs:17-20`). This reproduces the existing open finding in the preceding re-review; it is not an additional inventory mismatch. The source inventory was inspected statically. No tests were run, and no hardware verification was performed.
