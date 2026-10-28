# Static C parity review: EXI fix pass

Date: 2026-10-01

Status: All recorded EXI fixes passed independent static re-review. Hardware verification is outside scope.

## Scope

- `crates/hardware/ogc-exi/src/lib.rs`
- `crates/hardware/ogc-exi-sys/src/lib.rs`
- C references: `../../C-sources/libogc/libogc/exi.c` and `../../C-sources/libogc/gc/ogc/exi.h`

## Fixes

- **BUG FIX — SelectSD device ID validation and cache.** `ExiBus` now stores the last successful device 0 ID. `get_id` validates device presence, returns the cached ID when valid, and stores a new ID only after a successful transfer and a final presence check. `select_sd` calls `get_id` for unattached device 0 before selection. Presence removal or an external interrupt clears the cache. See `crates/hardware/ogc-exi-sys/src/lib.rs:66-88`, `:660-701`, `:756-786`, and `:1004-1071`.
- **BUG FIX — Attach requires a successful probe.** `attach` now calls `probe` before setting the attached state and checks presence again while it holds the EXI lock. See `crates/hardware/ogc-exi-sys/src/lib.rs:1074-1097`.
- **BUG FIX — Deselect checks device 0 after selection.** `deselect` checks device 0 presence after it clears selection and returns failure when the check fails. See `crates/hardware/ogc-exi-sys/src/lib.rs:792-822`.
- **BUG FIX — GameCube register base.** PowerPC EXI register access now selects `0xCC006800` when `sys_get_hollywood_revision()` identifies GameCube/DOL, and `0xCD006800` for Wii/RVL. See `crates/hardware/ogc-exi-sys/src/lib.rs:146-176`.
- **BUG FIX — Empty immediate transfer.** `imm_ex` returns 0 when `n_len` is zero, as C does. See `crates/hardware/ogc-exi-sys/src/lib.rs:939-965`.
- **BUG FIX — Initialization probe reset.** `init` now calls `probe_reset` after it releases the EXI state lock and installs handlers, matching C `__exi_init` startup probes and serial-port ID read. `probe_reset` clears both cached IDs before probing. See `crates/hardware/ogc-exi-sys/src/lib.rs:492-541`.

## Regression checks

Added source-level tests for the model-to-register-base choice, device presence and pending external interrupt rules, SelectSD ID-check conditions, zero-length `imm_ex`, and the Deselect result. The target permits compile checks, but it does not permit safe execution of the hardware-facing PowerPC tests in this host environment.

Commands run:

- `cargo fmt -p ogc-exi-sys`
- `cargo check -p ogc-exi-sys --target powerpc-unknown-linux-gnu --tests` — passed with one pre-existing function-item cast warning in a test helper.
- `cargo check -p ogc-exi --target powerpc-unknown-linux-gnu --tests` — passed. The previous check reported a temporary dead-code warning for the probe helper. The helper is now used.
- `git diff --check` — passed after the final source changes.
- `cargo xtask review-status` — blocked by an unrelated active ledger row at line 18 (`ogc-alloc` has no owner); the EXI rows are valid and already say `re_review`.

No hardware behavior was tested.

## Follow-up debounce fix

Implementer: `worker_c`. Independent reviewer: `/root/worker_a` (accepted).

Rust now keeps a per-channel first-detection timebase value for un-attached device 0 probes. It reports absence until 20 microseconds have elapsed, matching C's `diff_usec(last_exi_idtime[nChn], time) + 10 < 30` check. The threshold uses 40.5 MHz for GameCube and 60.75 MHz for Wii. A missing device or pending external interrupt resets the timestamp; the external interrupt handler and `probe_reset` also clear it. Attached devices skip the debounce, matching C's attached branch. Regression assertions cover each platform threshold and timebase wraparound.

Independent review compared the complete debounce change with C `__exi_probe` (`exi.c:164-207`) and its `EXI_ProbeReset` and external-interrupt paths (`exi.c:719-729, 821-832`). The Rust threshold is 810 GameCube timebase ticks and 1,215 Wii ticks, matching 20 microseconds at the respective 40.5 MHz and 60.75 MHz timebases. The C condition accepts at 20 microseconds because `PPCTicksToUs` truncates and C checks whether the result is below 20; Rust's rounded-up tick threshold has the same boundary. `wrapping_sub` handles timebase rollover. Rust resets the start time when the device is absent or an external interrupt is pending, in the external-interrupt handler, on `probe_reset`, and during `init` through `ExiBus::new`. Attached devices bypass the delay, as C does. No mismatch was found.

The regression tests cover both platform thresholds and rollover. Reset lifecycle paths were checked against C by source inspection; tests do not independently exercise each reset path. This handoff covers the probe insertion debounce only. Other EXI fixes and findings remain governed by their separate review records. No hardware verification is claimed.

## Validation boundary

The source changes and compile checks do not establish register, interrupt, hot-removal, or transfer behavior on hardware. All recorded changes passed independent static re-review; no hardware verification is claimed.

## Independent re-review — `/root/exi_independent_review`

Reviewer: `/root/exi_independent_review`

Implementer: `/root/di_review` (commit `b9f5d7b`)

Compared the changed EXI paths with `../../C-sources/libogc/libogc/exi.c` and `../../C-sources/libogc/gc/ogc/exi.h`. The register base selection matches C's GameCube/Wii compile-time split. The empty `EXI_ImmEx` result, SelectSD ID validation, attach probe requirement, deselect presence result, and initialization probe reset have corresponding changes. The startup sequence resets probe state, checks channels 0 and 1, then reads channel 0 device 2 as C does.

### Finding at the time of the initial independent review (resolved by the follow-up above)

**Confirmed parity gap — probe insertion debounce.** C `__exi_probe` (`exi.c:164-207`) records `last_exi_idtime` on first detection and returns absent until its elapsed-time calculation reaches the approximately 20 microsecond threshold. Rust `device_zero_probe_locked` (`crates/hardware/ogc-exi-sys/src/lib.rs:660-682`) reported present as soon as `EXI_EXT_BIT` was set and `EXI_EXT_IRQ` was clear. Worker_c implemented this missing debounce in commit `00a3df5`; worker_a independently compared it with C and accepted the fix. See “Follow-up debounce fix” above.

No source changes were made during this review. No tests were run; this was a static comparison.
