# IOS parity review

Scope: public declarations in `crates/core/ogc-ios/src/lib.rs` and `crates/core/ogc-ios-sys/src/lib.rs`, compared with `C-sources/libogc/gc/ogc/ios.h` and `C-sources/libogc/libogc/ios.c`. This review covers the exported API and the PowerPC implementation. It does not compare transitive ES, STM, IPC, or IRQ implementations.

Status: the preferred-version TMD size and launch-polling fixes passed static review. The 2026-10-07 follow-up also reconciled version-word cache invalidation with the supplied C cache primitive. Target timing and STM behavior remain open.

## Confirmed mismatches

- **Fixed and independently accepted:** the Rust preferred-version scan previously skipped a stored TMD when its reported size was zero. C skips only a size greater than 4096 (`C-sources/libogc/libogc/ios.c:163-170`), so C calls `ES_GetStoredTMD` with zero. The C `tmd_size < 0` check is ineffective because `tmd_size` is unsigned. Commits `0157ac5` and `dcbc20f` correct this behavior; re-review evidence is below.

## Differences that need target validation

- **FIXED:** IOS launch polling uses the timebase-backed one-millisecond wait, matching C's `udelay(1000)` cadence closely enough at the source contract level. Target timing still needs runtime verification.
- **FIXED:** `ios_read_version_word` issues one `dcbi` for the 32-byte-aligned version address before reading the word. The supplied C `PPCDCacheInvalidate` computes covered cache lines and issues `dcbi` for each one; the 8-byte IOS range occupies exactly one cache line. Rust no longer adds the extra `sync` that was absent from the C primitive. Target IOS reload behavior still needs execution testing.
- Rust represents the STM initialization in terms of opening `/dev/stm/immediate` and `/dev/stm/eventhook` and retains descriptors for close/retry (`ogc-ios-sys/src/lib.rs:165-218`). C calls `__STM_Init` and `__STM_Close` (`ios.c:79-87,94-108`). The higher-level order and error precedence match, but this review did not trace the STM implementation to verify descriptor and retry behavior in all error cases.

## Parity observed

- Header declarations map to the Rust re-exports: initialize/shutdown, startup load, IOS launch, preferred version, reload, version, revision, and revision components (`ios.h:42-53`; `ogc-ios/src/lib.rs:6-12`). Rust also exposes the four IOS error constants from the header (`ios.h:31-34`; `ogc-ios-sys/src/lib.rs:30-33`). The Rust symbol spellings are idiomatic wrappers around the C names.
- Subsystem initialization attempts ES and STM and keeps the later negative result; shutdown calls STM then ES and also keeps the later negative result (`ios.c:67-108`; `ogc-ios-sys/src/lib.rs:274-307`).
- Preferred-version selection uses IOS titles 28 through 61, checks the TMD title words, and prefers IOS 58 according to the same rule (`ios.c:156-175`; `ogc-ios-sys/src/ios_logic.rs:1-24` and `ogc-ios-sys/src/lib.rs:221-270`). Rust's full title-word check matches the C high/low word comparisons.
- Launch accepts versions 3 through 255, allows zero ticket views, rejects more than four, launches the same IOS title, waits for startup, reinitializes IPC, and returns `IOS_EMISMATCH` when the loaded version differs (`ios.c:224-332`; `ogc-ios-sys/src/lib.rs:101-163`). Both restore/unmask the ACR IRQ before checking the loaded version.
- Reload follows shutdown, ES init, conditional launch, and subsystem init; later negative results replace earlier ones (`ios.c:361-404`; `ogc-ios-sys/src/lib.rs:376-425`). Startup IOS load returns zero in both versions (`ios.c:335-359`; `ogc-ios-sys/src/lib.rs:349-355`).

## Intentional Rust additions

- The sys crate exports `ogc_ios_sys::Bool` (`ogc-ios-sys/src/lib.rs:28`); the C header has no corresponding IOS type (`ios.h:28-56`).
- The sys crate has host-side simulation paths and atomics for mock results and cached IOS version state (`ogc-ios-sys/src/lib.rs:35-58,274-375,428-450`). C exposes no matching host simulation API. These paths support the Rust crate's build/test setup and are not part of the PowerPC C parity claim.

## BUG FIX handoff: preferred-version TMD size

Implementer: `worker_c`. Independent reviewer: `/root/worker_a`.

The preferred-version scan now skips a stored TMD only when the size request fails or its size is greater than 4096. A zero size reaches `ES_GetStoredTMD`, matching the C scan's unsigned size check and call sequence. A regression test covers size zero, the 4096-byte limit, and 4097 bytes. The `ogc-ios` crate re-exports `ogc-ios-sys`; the implementation is in the sys crate.

`cargo check -p ogc-ios -p ogc-ios-sys --target powerpc-unknown-linux-gnu --tests` passed. This compiles the PowerPC test code, but the tests were not executed. `git diff --check` passed for the fix files.

Independent static review compared commits `0157ac5` and `dcbc20f` with `ios.c:156-175` and the ES size API. C stores the size in `u32`, rejects values greater than 4096, and passes zero or any supported size to `ES_GetStoredTMD`. The Rust predicate `size <= 4096` preserves that range, including zero; the 4096-byte aligned Rust buffer also covers every accepted size. The regression test covers zero, 4096, and 4097 at the predicate. It does not mock the ES call, so the zero-size call-through was confirmed by inspecting `preferred_version_powerpc`. No mismatch remains in this finding. The review did not check timing, cache, STM behavior, or hardware.

No hardware verification was performed.

## Implementer handoff: launch polling delay

Implementer: `/root/worker_b`. Commit: `87b7f3f` (`Match IOS launch polling delays`). Rust now waits until one millisecond has elapsed on the existing Wii timebase after each unsuccessful IOS version-word read, and before each IPC readiness check. This follows C's order and keeps the `counter <= MAX_IPC_RETRIES` limit (401 checks). A helper test covers the elapsed-time boundary and timebase wraparound.

Validation: `cargo check -p ogc-ios-sys --tests --target powerpc-unknown-linux-gnu` passed; target test code compiled but was not run. `git diff --check` passed. `cargo fmt -p ogc-ios-sys -- --check` still reports pre-existing formatting differences in the crate and `ios_logic.rs`; the changed test formatting was corrected without formatting unrelated files. No hardware verification was performed.

This handoff covers only the launch polling delay. Independent static re-review is needed. ACR handler restoration ordering and STM lifecycle findings remain separate open work.
