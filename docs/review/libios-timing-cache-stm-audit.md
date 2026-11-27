# IOS timing, cache, and STM audit

Reviewer: `/root/reviewer_6`  
Scope: independent source audit of the remaining timing, cache, and STM questions in `docs/review/libios.md`. The preferred-version zero-size TMD fix is excluded.

## Confirmed mismatches

### Current source status — 2026-11-27

The launch-polling timing mismatch below is historical and is fixed in the current source. C's Wii timebase is `PPC_BUS_CLOCK / 4 = 60,750,000` ticks per second; `udelay(1000)` waits for 60,750 ticks. Rust reads the same PowerPC timebase and waits for that tick interval after the same unsuccessful version read and before each IPC readiness check (`crates/core/ogc-ios-sys/src/lib.rs:40-43,87-106,150-159`; C `gc/tuxedo/ppc/clock.h:5-13,85-88`, `tuxedo/tick.c:135-143`, and `libogc/ios.c:290-312`). Target-gated regression coverage checks the interval boundary and counter wraparound. This is a source and helper-test result; elapsed timing and the full reload remain unverified on physical Wii.

The ACR restore order and the shared STM event-hook lifecycle also match the recorded fixes below. The 2026-11-27 source reconciliation below corrects the earlier cache note: launch polling reads the uncached alias in both C and Rust. Rust intentionally returns required STM initialization errors under the Real Implementation Policy, while pinned C can return success when STM setup fails. STM injected failure cleanup, real IOS replacement failures, the full-reload concurrency gap, and physical Wii behavior remain open.

- **Historical launch polling delay finding; fixed in current source.** C waits for the version word with `udelay(1000)` after each unsuccessful read (`../../C-sources/libogc/libogc/ios.c:290-291`). Its IPC readiness loop also calls `udelay(1000)` before each register check (`ios.c:307-312`). `udelay` measures elapsed time from `gettime()` until at least the requested microseconds have passed (`../../C-sources/libogc/libogc/timesupp.c:66-77`). The earlier Rust implementation used fixed `spin_loop` counts. Rust now waits for the same 60,750 timebase ticks per millisecond before each unsuccessful version poll and each IPC readiness check (`crates/core/ogc-ios-sys/src/lib.rs:40-43,100-106,150-170`). Target-gated helper tests cover interval boundaries and counter wraparound; elapsed timing on Wii remains unverified.
### Historical STM wrapper findings — shared lifecycle now used

The earlier claims that `ogc-ios-sys` omitted event-hook setup/release and
closed STM descriptors directly are superseded by the 2026-10-05 shared
lifecycle change. Current IOS startup calls `ogc_stm::init`; shutdown calls
`ogc_stm::close` (`crates/core/ogc-ios-sys/src/lib.rs:227,239`). The shared
implementation registers and releases the event hook
(`crates/core/ogc-stm/src/lib.rs:214,244`).

- **STM initialization failure behavior:** C `__STM_Init` returns `0` when either `IOS_Open` fails (`../../C-sources/libogc/libogc/stm.c:87-92`). Rust propagates the STM open error through subsystem initialization, where it can replace an earlier ES error. This is a deliberate safety difference: Rust reports that required STM setup failed instead of returning success.
- **Partial-open cleanup/order:** If C opens `/dev/stm/immediate` but fails to open `/dev/stm/eventhook`, it keeps the immediate descriptor until later shutdown (`stm.c:87-92,103-120`). Rust closes it at once and retains it only if that close fails (`crates/core/ogc-stm/src/lib.rs:347`). This changes close timing and preserves cleanup failure state for retry. C also sends `__STM_ReleaseEventHook` during close after unsuccessful initialization; Rust sends release only when its shared lifecycle records a registered hook. Injected cleanup failure and physical behavior remain unverified.

## Cache behavior compared

- **No invalidation-range mismatch confirmed for the version address.** C `IOS_GetVersion` and `IOS_GetRevision` invalidate eight bytes at `0x80003140` before reading a word (`../../C-sources/libogc/libogc/ios.c:185-204`). `PPCDCacheInvalidate` rounds the start down to a cache-line boundary and computes all intersecting lines (`../../C-sources/libogc/tuxedo/cache.c:8-23,29`). The address is 32-byte aligned and the eight-byte range remains within one line. Rust invalidates that address with one `dcbi` and then executes `sync` before its volatile word load (`crates/core/ogc-ios-sys/src/lib.rs:63-75`). Both invalidate the one containing line; Rust adds an ordering instruction. Launch-polling cache access is reconciled in the 2026-11-27 source note below.

## 2026-11-27 source reconciliation — version-word cache access

The current launch-polling code does not call `ios_read_version_word`. It reads and clears `0x80003140` through the uncached alias at `0xC0000000` plus the physical address (`crates/core/ogc-ios-sys/src/lib.rs:140-161`). The C `write32` and `read32` helpers use the same uncached alias (`../../C-sources/libogc/gc/ogc/machine/processor.h:116-129`), including the launch poll (`../../C-sources/libogc/libogc/ios.c:271,290`). The earlier statement that Rust invalidates the cache line on each poll was stale and is withdrawn.

The separate Rust `get_version` and `get_revision` functions use `ios_read_version_word`, which issues `dcbi` and reads the cached address (`crates/core/ogc-ios-sys/src/lib.rs:58-65,420-454`). C `IOS_GetVersion` and `IOS_GetRevision` call `DCInvalidateRange(0x80003140, 8)` and then read that address (`../../C-sources/libogc/libogc/ios.c:185-204`). The range is aligned and contained in one 32-byte line, so both invalidate the same line. This source comparison finds no cache invalidation mismatch for the version query or launch polling. It does not establish physical cache behavior; Wii validation remains open.

## Validation and limits

This was a source-only comparison. No build, tests, or hardware verification were performed. The review inspected the exact current Rust worktree and C source paths listed above. The addendum is separate from `docs/review/libios.md` to avoid changing the shared review note.

## Independent re-review — timing and STM findings

Result: **NEEDS_CHANGES** for the recorded timing and STM scope.

Reviewer: `/root/reviewer_d`. Implementer: the Rust implementation under review originated in commits `b2db513` (launch path), `f20cc9c` (STM path), and `d87d3c5` (subsystem result handling), authored as `notOrrytrout`; later STM cleanup behavior is in `45b3ca8`. No new implementation was made in this review. I checked current Rust against C `ios.c`, `timesupp.c`, and `stm.c` directly.

### Re-verified confirmed mismatches

### 2026-11-27 correction — retain required STM initialization errors

An earlier change ignored negative STM initialization results so the outer
return matched pinned C. A source-review finding rejects that behavior under
the Real Implementation Policy: it reports successful subsystem
initialization after a required STM device open or event-hook operation fails.
The current `initialization_result` preserves the last negative subsystem
result, so a real STM failure remains visible. This is an intentional safety
deviation from C's outer error behavior. The review of STM partial-open
cleanup and close-error handling remains open. Current evidence: the target
test code compiles; all 9 `ios_logic.rs` tests pass in a host wrapper; the full
host suite and strict Clippy pass with the documented external fixtures.

- **Historical launch polling cadence finding; fixed in current source.** The fixed `spin_loop` counts described in the older re-review were replaced by the timebase wait documented above. Physical elapsed timing remains unverified.
- **STM init failure behavior differs by policy.** Rust routes IOS startup through `ogc_stm::init` and attempts both required device opens and event-hook registration. It returns a negative failure when setup fails. Pinned C returns zero for failed device opens and ignores the event-hook submission result (`stm.c:87-100`). Rust retains the error because reporting successful setup after required operations fail is unsafe under the repository policy. The public result and STM state can therefore differ on failure; verify both error and cleanup paths.
- **The IOS STM success path includes event-hook registration and release.** `stm_init_powerpc` calls `ogc_stm::init`, which opens both descriptors, sets initialized state, and submits `IOCTL_STM_EVENTHOOK`; `stm_close_powerpc` calls `ogc_stm::close`, which releases a registered hook before descriptor close. This matches the successful C lifecycle. Remaining failure-path differences are listed next.
- **Failed second-open cleanup occurs at a different time and has different state.** When C opens the immediate descriptor but event-hook open fails, `__STM_Init` returns zero and leaves the immediate descriptor stored (`stm.c:87-92`); later `__STM_Close` attempts release first, then closes and clears the stored descriptors (`stm.c:103-120`). Rust explicitly closes the immediate descriptor as soon as the event-hook open fails, returns the event-hook error, and retains the immediate fd only if that close fails (`crates/core/ogc-ios-sys/src/lib.rs:177-186`). The descriptor lifetime, return value, and eventual close/release flow differ. On ordinary close, Rust also preserves a descriptor after close failure for a later retry (`lib.rs:198-215`), while C clears the stored fd after each close attempt (`stm.c:109-117`); this is a deliberate retry/safety behavior but is not C state parity.

### Additional launch ordering difference

C reinstalls and unmasks the ACR interrupt before `__IPC_Reinitialize` and the post-launch version query (`ios.c:318-325`). Rust calls `ogc_ipc::reinitialize()` before requesting and unmasking the handler, and does so only after the version query (`crates/core/ogc-ios-sys/src/lib.rs:142-159`). This changes interrupt availability during reinitialization and version validation. Both paths restore the handler on version mismatch, but they do so at different points.

### Cache behavior and validation

For `get_version` and `get_revision`, C invalidates the eight-byte version range through `PPCDCacheInvalidate` and Rust issues `dcbi` for the aligned version address (`ios.c:185-204`; cache helper `../../C-sources/libogc/tuxedo/cache.c:8-30`; Rust `crates/core/ogc-ios-sys/src/lib.rs:58-75`). Both invalidate the same cache line. Launch polling uses the uncached alias in both implementations, as recorded in the source reconciliation above. Source comparison found no cache-operation mismatch; physical cache behavior remains unverified.

`cargo check -p ogc-ios -p ogc-ios-sys --tests --target powerpc-unknown-linux-gnu` passed, which confirms the target test code compiled; tests did not run. No hardware verification was performed. The confirmed timing and STM differences above remain open for implementation review.

## Independent re-review — launch timing and neighboring IOS/STM behavior

**Result: PASSED for the 1 ms timing and Wii target gate; IOS remains open.** Reviewer: `/root/reviewer_d`. Timing commits: `87b7f3f` and `4a75075`; target-gate fix is also recorded in the handoff above.

Rust's launch poll uses 60,750 ticks for 1 ms at 60.75 MHz, with the same checks and delay placement as C `udelay(1000)` (`crates/core/ogc-ios-sys/src/lib.rs:40–43, 100–110, 150–159`; `C-sources/libogc/libogc/ios.c:290–312`, `timesupp.c:66–77`). Both perform up to 401 IPC checks. The timing helper regression covers boundary and wraparound cases. The Wii-only target gate matches C's `HW_RVL` guard.

The remaining confirmed differences are broader than the timing change. C restores/unmasks the ACR handler before IPC reinitialization and the version query, while Rust reinitializes IPC and queries the version before restoring/unmasking (`ios.c:318–325`; Rust `ogc-ios-sys/src/lib.rs:142–174`). C STM initialization converts either open failure to success (`stm.c:87–92`), while Rust propagates negative open errors (`ios-sys/src/lib.rs:185–197`), which can change IOS startup error precedence. C successful STM init registers an event hook and close releases it (`stm.c:98–108, 123–153`); Rust's IOS path only opens and closes descriptors (`ios-sys/src/lib.rs:177–230`). Partial-open cleanup and close-error precedence also differ: Rust closes the immediate descriptor after second-open failure and retains failed-close descriptors; C leaves the immediate descriptor for shutdown and clears both descriptor fields even after close errors (`stm.c:87–120`; Rust `ios-sys/src/lib.rs:189–227`).

Cache invalidation before each Rust version poll remains a source difference from C's uninvalidated `read32`; the runtime effect remains uncertain. `cargo check -p ogc-ios -p ogc-ios-sys --target powerpc-unknown-linux-gnu --tests` passed and compiled target tests, which did not run. The relevant commit diff checks passed. No hardware verification was performed.

## Fix pass: STM initialization open-error return

**BUG FIX.** Implementer: `worker_c`. Independent re-review: needed. When either STM `open_file` call fails, Rust now returns zero, matching C `__STM_Init`. It preserves the current partial-open cleanup after the event-hook open failure; cleanup timing is a separate finding. This prevents an STM open error from replacing an earlier ES initialization error. Added focused target coverage for both open failure results and ES error precedence.

`cargo check -p ogc-ios-sys --target powerpc-unknown-linux-gnu --tests` and scoped `git diff --check` passed. Test code compiled but did not run. No hardware verification was performed. Event-hook registration and partial-open cleanup findings remain open.

## Independent re-review — ACR ordering and STM open failure result

Reviewer: `/root`. Implementer: `worker_c`. Commit `088d798` passes its bounded comparison: C requests and unmasks the ACR interrupt before `__IPC_Reinitialize` and `IOS_GetVersion` (`../../C-sources/libogc/libogc/ios.c:318-325`); Rust now uses the same order in `launch_ios_from_es` (`crates/core/ogc-ios-sys/src/lib.rs:168-184`).

Commit `ec99c62` needs changes. It maps either failed STM `open_file` call to zero through `stm_open_failure_result`, which matches C `__STM_Init` (`../../C-sources/libogc/libogc/stm.c:80-100`). However, C's return value hides a required device-open failure. The Rust port must not report successful initialization after an open fails; this conflicts with the repository Real Implementation Policy. Preserve the actual failure in Rust and keep useful ES error precedence without converting the failed STM initialization to success. The tests cover the zero mapping, so they currently lock in this unsafe success behavior. The STM event-hook and partial-open differences recorded above also remain open.

This was a static source review. I did not run builds or tests. No hardware verification was performed.

## 2026-10-05 fix handoff: share STM lifecycle with IOS startup

Implementer: `/root`. `ogc-ios-sys` now calls the same `ogc-stm` state used by the public STM API for initialization and shutdown. This removes duplicate STM descriptors, installs the real event-hook callback during IOS startup, and releases the hook before closing descriptors during IOS shutdown. The dependency cycle is removed by making `ogc-stm` read the Hollywood revision through the low-level `ogc-sys` accessor instead of the `ogc-system` wrapper. STM device opens now run in C order: eventhook is not opened if immediate open fails. Open errors propagate to IOS initialization, because returning success when required STM setup fails violates the repository's real implementation policy. Event-hook registration failure closes the descriptors and returns the ioctl error. Failed cleanup retains the descriptor in STM state so a later close can retry it.

Added PowerPC regression coverage for immediate-open short-circuit, returned open errors, event-hook failure cleanup, and descriptor retention after cleanup failure. `cargo check -p ogc-stm --tests --target powerpc-unknown-linux-gnu` and `cargo check -p ogc-ios-sys --tests --target powerpc-unknown-linux-gnu` passed; tests were compiled but not run. The `ogc-system-sys` library check passed. Its test target remains blocked by existing test-module errors; tests did not compile or run. Static review of the changed dependency graph and C ordering is needed. IOS timing/cache differences and the STM target-availability review remain open. No hardware verification was performed.

## 2026-10-06 source-status reconciliation

The named non-safety IOS blocker is closed in the current source. The independent review above records that commit `088d798` matches C's ACR handler restore/unmask ordering before IPC reinitialization and the version query. IOS startup uses the shared `ogc-stm` lifecycle. Source comparison found no version-word cache-operation mismatch; physical cache behavior remains unverified. The deliberate propagation of required STM open and event-hook failures is retained by repository policy. `ogc-ios` and `ogc-ios-sys` therefore remain `needs_re_review`.

## Independent re-review — IOS cache invalidation sequence (2026-10-07)

**Result: PASSED for `ogc-ios` and `ogc-ios-sys`.** Reviewer: `/root` in a new review thread. The current source removes the `sync` instruction after `dcbi` in `ios_read_version_word`. This matches the supplied `PPCDCacheInvalidate`, whose implementation loops over covered cache lines and issues `dcbi` without a trailing synchronization (`../../C-sources/libogc/tuxedo/cache.c:8–30`). The version-word address and covered range remain aligned as described in the earlier source comparison. The previously recorded launch ordering and shared STM lifecycle fixes remain present. PowerPC target test code compiled with `RUSTFLAGS='-Aunsafe-op-in-unsafe-fn'`; tests were not run. Runtime IOS reload behavior was not tested.

## IPC ready-bit condition — 2026-11-26

C `__IOS_LaunchNewIOS` leaves the IPC startup wait when either bit in
`HW_IPC_PPC_CTRL_REGS` is set. Rust previously required both bits. The Rust
condition now accepts either bit. A host test checks neither bit, an unrelated
bit, each ready bit alone, and both bits; it failed on the former condition.
Dolphin 2609a then reloaded IOS 58 and passed all 12 checks, including IPC
interrupt masking and a subsequent `/dev/fs` open and close. The result is in
`../dolphin-runs/2026-11-26-ios-ready-any-bit/`. This Dolphin run does not show
which ready-bit combinations occurred. Independent review and physical Wii
verification remain pending.

## IOS ticket-view alignment — 2026-11-26

C allocates the IOS launch ticket-view array with 32-byte alignment. Rust's
`Tikview` is packed, so its previous array had no alignment guarantee; ES
requires at least 4-byte alignment. Rust now uses a 32-byte aligned wrapper
for this array and reuses that wrapper for the existing aligned TMD buffer.
A host layout test failed for an unaligned wrapper and passes with the new
wrapper. Dolphin 2609a reloaded IOS 58 and passed all 12 checks with this
change; see `../dolphin-runs/2026-11-26-ios-ticket-alignment/`. This does not
prove a failure occurred with the previous stack placement. Independent source
review and physical Wii validation remain pending.

## IOS lifecycle API evidence — 2026-11-27

The `ios_reload_probe` now calls the public Rust subsystem shutdown and
initialization APIs, the startup loader, invalid-version launch, and same-version
reload. Dolphin 2609a passed 112 checks. The startup loader returns `0`, which
matches the pinned C weak default because its IOS launch body is disabled with
`#if 0`; this result does not prove a startup IOS launch. The invalid launch
returns `IOS_EBADVERSION`. The reload returns `0`, leaves IOS 58 active, and
allows a fresh `/dev/fs` open and close. See
`../dolphin-runs/2026-11-27-ios-lifecycle-parity/README.md`.

The Rust STM wrapper continues to return required STM open and event-hook
errors. Pinned C maps STM device-open failures to `0`, which can report
subsystem initialization success when the required devices did not open. Rust
also cleans up a partially opened device instead of retaining it until a later
shutdown. These are deliberate safety differences: Rust preserves the actual
failure and avoids keeping incomplete STM state. The public subsystem return
code can therefore differ from C when STM setup fails. The normal Dolphin run
checks the success path; injected STM open failures and physical Wii behavior
remain unverified.

## STM close callback lock wait — 2026-11-27

The saved `stm_api_probe` run from 2026-11-20 timed out while its deferred
event-hook callback was in `event_hook_handler` waiting for `STM_LOCK`
(`docs/dolphin-runs/2026-11-20-ogc-stm-lifecycle-apis/stm_api_probe/timeout-diagnostic.json`).
`close_with_operations` previously took that lock before setting
`STM_HOOK_RELEASING`. Shutdown now sets the release flag first, so a deferred
callback that preempts the lock owner can exit instead of spinning while the
owner waits to resume. A target regression test covers the callback path with
the lock held and release flag set.

The updated `stm_api_probe` DOL built with SHA-256
`d28a11020097ef6f31e83f5a5f34f1574b4b91ad1e4e25ea9abe004d1c6750cc`.
`cargo xtask clippy` passed. PowerPC test code compiled with
`RUSTFLAGS='-Aunsafe-op-in-unsafe-fn -Astatic-mut-refs'`; this only checks the
test code and does not run it. Dolphin could not run the updated probe: the
installed `/Applications/Dolphin.app/Contents/MacOS/Dolphin` exited with
`SIGABRT` on `--version`. The saved timeout is evidence of the old behavior;
the fix remains unverified at runtime. Keep the STM close probe and physical
Wii behavior open until Dolphin can run it.

## Replacement cleanup on an unexpected IOS version

`launch_ios_from_es` now finishes the active IOS replacement after the new
version word is published and IPC is reinitialized, before it returns
`IOS_EMISMATCH` for a version that differs from the request
(`crates/core/ogc-ios-sys/src/lib.rs`). `IosReplacement::finish` still requires
a fresh IOS reply before it cancels abandoned requests. If that reply fails,
it retains the old request records because IOS may still access them. This
preserves buffer ownership and callback rules while avoiding stale requests
after a successful restart into an unexpected version.

Host tests run the shared helper called by production and confirm cleanup runs
before matching and mismatching version results. The focused helper tests pass
2/2. These tests cover result ordering; they do not simulate an actual IOS
version mismatch. The PowerPC Dolphin probe build also compiles this helper.
`cargo xtask clippy` passed with the repository MP3 fixtures. Dolphin
`ios_reload_probe` passed on the normal IOS 58 path; it does not force an
unexpected IOS version. Physical Wii and failed-handshake tests remain open.


## Independent IOS/IPC/ES lifecycle review — 2026-11-27

The independent review confirmed that IOS startup and shutdown use the shared
`ogc-stm` lifecycle and identified the earlier wrapper-omission findings as
stale. It confirmed the intentional STM initialization error difference and
found no request-retirement or deferred callback ownership mismatch. The review
also found a cross-crate safety gap: `IosReplacement` rejects IPC calls during
its launch window, but does not serialize the full `reload_ios` lifecycle or
protect ES descriptor state mutations that happen before a rejected IPC call.
See [`libipc-core.md`](libipc-core.md#independent-ipcioses-lifecycle-review--2026-11-27).
The concurrent path remains unverified at runtime.
