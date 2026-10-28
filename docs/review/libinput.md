# Safe PAD wrapper review

Status: static source review complete for `ogc-input`, including its calls into the previously reviewed `ogc-input-sys`. No compile, test, or hardware run was done.

## Scope and source map

Reviewed every production line in `crates/services/ogc-input/src/lib.rs` (lines 1–108) and its target-gated signature/layout tests (lines 109–147). Reviewed the checked-in raw binding API and the relevant call paths in `crates/services/ogc-input-sys/src/lib.rs`, including constants/types (lines 14–124), reset/read/recalibration and motor logic (lines 1072–1104, 1205–1455), scan/accessor paths (lines 1448–1573), and diagnostics (lines 1106–1203, 1575 onward). The raw binding's prior report is `docs/review/libpad.md`.

The C API and data layout are declared in `../../C-sources/libogc/gc/ogc/pad.h` (lines 6–89); implementation is `../../C-sources/libogc/libogc/pad.c` (lines 457–813 for sampling, initialization, reads, reset/recalibration, sync, spec, motor, clamp, scan, and accessors). This review focuses on the safe wrapper and confirms its dependencies against those raw C semantics. Low-level SI transaction scheduling and PAD status decoding were reviewed in the existing `libpad.md` scope; they are not re-audited here beyond wrapper call effects.

## Confirmed mismatches

No new unintended mismatch was found in the safe wrappers. The one intentional difference inherited from `ogc-input-sys` is `PAD_Recalibrate`: C's checked-in implementation is an empty critical section that returns 1 without changing calibration state (`pad.c:625–633`), while Rust performs the calibration/reset path (`ogc-input-sys/src/lib.rs:1329–1345`). The safe `recalibrate(mask)` wrapper forwards to that Rust behavior (`ogc-input/src/lib.rs:39–41`). This is an intentional fix to the no-op C implementation and is already recorded in `docs/review/libpad.md`.

## Wrapper API mapping

The wrapper re-exports all raw binding items at `ogc-input/src/lib.rs:11–20`, then provides fixed-size/status-safe entry points. Every direct wrapper passes through to its corresponding `ogc-input-sys` operation:

| Safe wrapper | Raw Rust call | C equivalent |
| --- | --- | --- |
| `init()` (`lib.rs:22–24`) | `pad_init()` | `PAD_Init` (`pad.h:64`; `pad.c:479–506`) |
| `sync()` (`lib.rs:26–28`) | `pad_sync()` | `PAD_Sync` (`pad.h:65`; `pad.c:635–643`) |
| `read(&mut [PadStatus; 4])` (`lib.rs:30–33`) | `pad_read(ptr)` | `PAD_Read(PADStatus *)` (`pad.h:66`; `pad.c:508–599`) |
| `reset(mask)` / `recalibrate(mask)` (`lib.rs:35–41`) | `pad_reset` / `pad_recalibrate` | `PAD_Reset` / `PAD_Recalibrate` (`pad.h:67–68`; `pad.c:601–633`) |
| `clamp(&mut [PadStatus; 4])` (`lib.rs:43–46`) | `pad_clamp(ptr)` | `PAD_Clamp(PADStatus *)` (`pad.h:69`; `pad.c:693–705`) |
| `control_motor(chan, cmd)` (`lib.rs:48–50`) | `pad_control_motor` | `PAD_ControlMotor` (`pad.h:70`; `pad.c:657–676`) |
| `set_spec(spec)` (`lib.rs:52–54`) | `pad_set_spec` | `PAD_SetSpec` (`pad.h:71`; `pad.c:645–655`) |
| `scan_pads()` (`lib.rs:56–58`) | `pad_scan_pads()` | `PAD_ScanPads` (`pad.h:73`; `pad.c:707–758`) |
| `buttons_up/down/held(pad)` (`lib.rs:65–75`) | matching `pad_buttons_*` | matching C accessors (`pad.h:75–77`; `pad.c:761–777`) |
| stick and trigger getters (`lib.rs:77–99`) | matching `pad_*` getters | C `PAD_SubStick*`, `PAD_Stick*`, `PAD_Trigger*` (`pad.h:79–86`; `pad.c:779–813`) |
| `set_sampling_callback(cb)` (`lib.rs:101–103`) | `pad_set_sampling_callback` | `PAD_SetSamplingCallback` (`pad.h:89`; `pad.c:678–691`) |

`PadStatus` and its constants come through the `sys` re-exports (`lib.rs:11–20`). C defines `PADStatus` as 12 bytes with 2-byte alignment and four channels (`pad.h:6–10, 46–57`). The wrapper uses a `[PadStatus; PAD_CHANMAX as usize]` reference for `read` and `clamp`, so safe callers provide the four entries required by C's loops (`pad.c:519–594, 697–704`). The target-gated wrapper test asserts the Rust structure size/alignment (`ogc-input/src/lib.rs:113–117`); it was not run.

## Call-path behavior

- **Read and clamp buffer safety:** The C API accepts an unconstrained pointer; `PAD_Read` and `PAD_Clamp` access four entries (`pad.c:508–599, 693–705`). The safe wrapper's fixed array prevents a short buffer or null pointer from reaching the raw functions (`ogc-input/src/lib.rs:30–33, 43–46`). It preserves the raw return value from `PAD_Read`. No copy or state change is added by these two wrappers.
- **Reset and recalibration:** `reset(mask)` forwards unchanged to `pad_reset`, which follows C `PAD_Reset`'s mask/pending/reset logic (`pad.c:601–623`; raw Rust `ogc-input-sys/src/lib.rs:1314–1327`). `recalibrate(mask)` forwards to Rust's intentional implementation change described above. It does not preserve C's empty-body result semantics because that body is treated as a C bug fix.
- **Scan and accessors:** `scan_pads()` forwards to the raw helper that reads PAD state and updates the per-channel key state; its connected mask and button transition behavior follow the C path at `pad.c:707–758`. The getters forward their channel index unchanged; the raw Rust functions bounds-check `0..4` and return zero for invalid/disconnected channels, matching C's documented range checks while avoiding out-of-range indexing (`pad.c:761–813`; `ogc-input-sys/src/lib.rs:1503–1573`).
- **Control motor, spec, sync, init, and callback registration:** Wrappers do not modify arguments or results (`ogc-input/src/lib.rs:22–28, 35–54, 101–103`). The raw operations preserve the C command/spec/reset semantics (`pad.c:479–506, 635–691`). Rust's initialized-state locking and SI registration machinery are implementation details in the already reviewed sys crate.

## Rust-only API and behavior

- `scan_pads_with_status(&mut [PadStatus; 4])` (`ogc-input/src/lib.rs:60–63`) is not in `pad.h`. It performs one read and returns the same decoded samples used to update button/accessor state (`ogc-input-sys/src/lib.rs:1453–1501`). This gives diagnostics status without consuming a second SI response.
- `set_debug_hook` (`ogc-input/src/lib.rs:105–107`) forwards the Rust diagnostic hook. C PAD exposes no corresponding public hook (`pad.h:64–89`). The `diagnostics` Cargo feature enables bounded trace support through the dependency feature (`ogc-input/Cargo.toml`); the wrapper also re-exports raw trace types/helpers (`lib.rs:11–20`).
- `read` and `clamp` are safe Rust wrappers because their references encode the four-entry extent. Other wrappers expose the same scalar arguments as C. Raw pointer operations remain accessible through the public `sys` re-export (`ogc-input/src/lib.rs:11–12`), so callers can still use the raw API when they need C-compatible pointer behavior.
- Rust applies the real reset/calibration path in `recalibrate`; C's current empty `PAD_Recalibrate` is a no-op (`pad.c:625–633`). `docs/review/libpad.md` records this deliberate bug fix.
- Rust's sampling callback is invoked by its polling handler, while the checked-in C `__pad_samplinghandler` is empty (`pad.c:457–459`; raw Rust behavior is covered in `libpad.md`). This is another existing Rust addition rather than a wrapper mismatch.

## Uncertain or unchecked

- No hardware was used. Controller polling, motor behavior, reset callbacks, SI timing, and callback reentrancy remain outside this wrapper review.
- The existing `libpad.md` note covers the raw state machine and its C comparison. This pass did not independently repeat every status-spec decoding and origin-calibration path; those are not claimed as newly re-reviewed here.
- The wrapper tests only assert the status layout and callable API signatures (`ogc-input/src/lib.rs:109–147`). No compile or tests were run for this scope, per instruction.

## Independent static review update (2026-10-03)

Reviewer: `/root/reviewer_e`. The safe wrapper itself preserves the arguments and return values for its direct PAD forwarding functions. During the call-path check, I found one inherited `ogc-input-sys` mismatch in sampling callback registration. This is within the behavior exposed by `ogc-input::set_sampling_callback`, so the wrapper crate cannot be accepted as fully matching C while it forwards this behavior.

### Finding: callback state changes only when SI registration succeeds

C `PAD_SetSamplingCallback` saves the prior callback, assigns `__pad_samplingcallback = cb` unconditionally, then attempts `SI_RegisterPollingHandler` or `SI_UnregisterPollingHandler`, and returns the prior callback (`../../C-sources/libogc/libogc/pad.c:678-691`). The SI operations can fail: registration returns `0` when all four handler slots are occupied (`../../C-sources/libogc/libogc/si.c:701-726`), and unregistration returns `0` when the handler is absent (`:728-746`).

Rust `pad_set_sampling_callback` (`crates/services/ogc-input-sys/src/lib.rs:1400-1412`) assigns `PAD_SAMPLING_CALLBACK = cb` only if the SI operation returns nonzero. Therefore, when registration fails because all SI polling-handler slots are full, C changes the PAD callback state but Rust leaves the old callback in place. When unregistration fails because the PAD handler is absent, C still clears its callback but Rust keeps the previous callback. The safe wrapper at `crates/services/ogc-input/src/lib.rs:101-103` forwards directly, so callers observe this mismatch through callback behavior. Recommend matching C's unconditional PAD callback state update while preserving the SI handler operation's own return status internally.

This is a confirmed static mismatch in the Rust dependency behavior, not a defect in the safe wrapper's forwarding logic. The finding overlaps the existing `ogc-input-sys` review scope (`docs/review/libpad.md`) and should be routed to its implementation owner. No source implementation was changed, and no compile, test, or hardware run was performed for this update.

## Independent re-review — sampling callback state

Reviewer: `/root/reviewer_f`. This confirms the state update mismatch above. C stores `__pad_samplingcallback = cb` before calling SI register/unregister and ignores the SI return (`C-sources/libogc/libogc/pad.c:678–691`). SI can return zero when all four handler slots are full or when the handler to remove is absent (`si.c:701–746`). Rust stores `PAD_SAMPLING_CALLBACK` only when SI returns nonzero (`crates/services/ogc-input-sys/src/lib.rs:1400–1412`). Therefore, after a failed registration or removal, a later setter call returns a different previous callback in Rust than in C. The safe API returns this value unchanged (`ogc-input/src/lib.rs:101–103`).

Rust also invokes the stored callback from its sampling handler, while C's `__PADSamplingHandler` is empty (`pad.c:457–459`; Rust `ogc-input-sys/src/lib.rs:690–695`). That is a separate Rust addition recorded in the prior note. `cargo check -p ogc-input --tests --target powerpc-unknown-linux-gnu` passed and compiled tests; tests were not run. No hardware verification was performed.

### Broader sampling callback and SI registration audit

Reviewer: `/root/reviewer_f`. Rust publishes SI handler state before storing the new PAD callback. C stores the callback first, then registers or unregisters the SI handler and ignores that result (`C-sources/libogc/libogc/pad.c:678-691`). Rust registers/unregisters first and changes `PAD_SAMPLING_CALLBACK` only on success (`crates/services/ogc-input-sys/src/lib.rs:1400-1412`). SI registration mutates the handler slot and enables polling before returning (`crates/hardware/ogc-si-sys/src/lib.rs:1392-1416`); unlike C's interrupt-masked slot update (`C-sources/libogc/libogc/si.c:701-746`), Rust does not guard the slot mutation and callback store as one operation. A pending polling interrupt can therefore observe the old callback or no callback during registration. C's PAD sampling handler body is empty (`pad.c:457-459`); Rust callback invocation is a deliberate Rust-only extension documented in `docs/review/libpad.md:27`, and it runs inline in the gated RDST interrupt path (`ogc-input-sys/src/lib.rs:690-695`; `ogc-si-sys/src/lib.rs:851-884`). No callback-order regression test was found.

Reviewer_f ran no validation. The recorded PowerPC `cargo check --tests` compiled test code but did not run tests. No hardware verification was performed.

## BUG FIX handoff: store PAD sampling callback after failed SI operations

Implementer: `/root/worker_a`. Independent re-review is needed; the review thread handles it. Rust now stores the requested PAD sampling callback regardless of the result from SI polling-handler registration or removal, matching C's state update behavior. The SI handler operation still runs, but its failure does not prevent the PAD callback state from changing.

Added focused regression coverage for failed registration and failed removal. Each case checks the previous callback return value and the new stored state. The broader SI interrupt-guard concern and Rust-only callback invocation remain separate documented differences. `cargo check -p ogc-input-sys --target powerpc-unknown-linux-gnu --lib` and `cargo check -p ogc-input-sys --tests --target powerpc-unknown-linux-gnu` passed. The test target compiled; tests were not run. `git diff --check` passed. No hardware verification was performed.

### Independent re-review of failed-SI callback state fix `0fd06ca`

**Result: PASS.** Reviewer: `/root/reviewer_e`; implementer: `worker_a`.

C stores the requested callback before SI register/unregister, ignores the SI result, and returns the previous callback (`C-sources/libogc/libogc/pad.c:678-691`). Rust now also ignores SI failure, stores the requested callback, and returns the previous callback (`crates/services/ogc-input-sys/src/lib.rs:1400-1429`), matching failed registration and removal behavior. Although the assignment order differs, Rust holds `PAD_LOCK`'s `InterruptGuard` across the complete setter operation (`lib.rs:237-257,1401`); the nested SI guard does not expose an interval in which the IRQ handler can observe stale callback state (`crates/hardware/ogc-si-sys/src/lib.rs:1450-1461`). The two added tests cover failed registration and failed removal, including prior/new callback state.

Validation: `git diff 0fd06ca^ 0fd06ca --check` passed. PowerPC library and test-target checks passed according to the handoff; test code compiled, but tests did not run. Reviewer_e ran `cargo xtask review-status`, which passed. No hardware verification was performed. The Rust-only callback invocation remains a documented extension (`docs/review/libpad.md:27`); the setter state and ordering findings above are resolved.
