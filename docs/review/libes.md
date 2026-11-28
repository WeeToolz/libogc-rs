# libes Review

See the [2026-11-19 Dolphin results](../../DEVELOPMENT_RESULTS.md#2026-11-19-dolphin-api-evidence)
for the current ES probe scope. The results include system menu metadata and
content from a private Dolphin NAND, fixture-based crypto checks, and selected
error paths. Physical Wii validation remains open.

Status: the explicit initialization and `get_title_id` error-precedence fixes remain in place. The previously open Wii `es:` filesystem mismatch is now implemented in this pass; independent re-review and hardware validation remain. Comparison scope: `ogc-es` and `ogc-es-sys` against `C-sources/libogc/libogc/es.c` and `C-sources/libogc/gc/ogc/es.h`.

## Confirmed mismatches

These findings describe the original review. See the follow-up sections below
for the fixes and later review results.

- **The explicit initialization contract is not preserved.** In C, `__ES_Init` creates the heap and opens `/dev/es`; public calls such as `ES_GetTitleID` return `ES_ENOTINIT` while the descriptor is closed (`libogc/es.c:124-152,179-190`). In Rust, the API entry points call `ensure_open` themselves (for example `imp.rs:407-419,438-456`), which opens the device and heap on first use. A caller that omits `init()` therefore gets an operation instead of `ES_ENOTINIT`. This affects the sys API broadly; `ensure_open(state)` appears throughout the implementation.

- **Historical finding, fixed in the current pass — Rust ES initialization omitted the Wii `es:` filesystem device registration.** C calls `__ES_InitFS()` from `__ES_Init` and `__ES_DeinitFS()` from `__ES_Close` (`libogc/es.c:124-152` and `:154-169`); on Wii these add and remove the `es:` devoptab (`libogc/es.c:1287-1293`). Rust `init()` only calls `ensure_open` (`ogc-es-sys/src/imp.rs:407-412`), and `close()` only closes the IPC descriptor (`:414-425`). The C implementation also provides the corresponding `es:` file operations in `es.c` (`:_ES_decodepath` onward), while neither Rust crate exposes that filesystem behavior. Programs that rely on paths such as `es:/...` do not get C behavior.

## BUG FIX handoff: explicit initialization contract

Implementer: `worker_c`. Independent reviewers: `/root/reviewer_2` and `/root`.

`init()` remains the only operation that creates the ES heap and opens `/dev/es`. Public ES operations now check the existing descriptor and return `ES_ENOTINIT` if it is absent, instead of opening the device on first use. This preserves the C behavior after startup, `close()`, and `reset()` until the caller invokes `init()` again. The `ogc-es` safe crate re-exports the sys functions, so this change covers both ledger crates. At that stage, the separate Wii `es:` filesystem registration and path API mismatch remained open; the 2026-10-02 implementation below supersedes it.

Added a regression test that calls `get_title_id` before explicit initialization and expects `ES_ENOTINIT`. `cargo check -p ogc-es-sys --target powerpc-unknown-linux-gnu --tests` passed, which compiles the PowerPC test code. Tests were not executed. `cargo fmt -p ogc-es-sys -- --check` reported a formatting difference in the pre-existing crate-level compile-error line in `src/lib.rs`; this fix did not change that file. No hardware verification was performed.

## Independent review: explicit initialization contract

Reviewer: `/root`. Implementer: `worker_c`.

The fix restores the C initialization guard for calls with otherwise valid arguments: `init()` is the only path that opens `/dev/es`, and the public operations use `require_initialized` before accessing the descriptor. This matches C's `__ES_Init`, `__ES_Close`, and `ES_GetTitleID` state checks (`libogc/es.c:124–169, 179–190`).

This review found one mismatch: Rust `get_title_id` checked `title_id.is_null()` before `require_initialized`, so an uninitialized call with a null output returned `ES_EINVAL`. C checks `__es_fd < 0` first and returns `ES_ENOTINIT` for the same call. The follow-up fix is recorded below. Other input-validation checks in other operations were not exhaustively compared. At that stage, the separate Wii `es:` filesystem mismatch remained open; the 2026-10-02 implementation below supersedes it.

## BUG FIX handoff: `get_title_id` error precedence

Implementer: `worker_c`. Independent reviewer: `/root/worker_a` (accepted).

`get_title_id` now checks the open-descriptor state before checking whether the output pointer is null. This matches C `ES_GetTitleID`, which returns `ES_ENOTINIT` before using or validating the output pointer when `__es_fd` is closed (`libogc/es.c:179-190`). A regression test checks `get_title_id(null)` before initialization and expects `ES_ENOTINIT`. This fix is scoped to `get_title_id`; no broader error-precedence claim is made.

Independent review confirmed that C checks `__es_fd < 0` before `!titleID`, and Rust's `require_initialized` now precedes the null check. The regression test exercises the uninitialized/null case and expects `ES_ENOTINIT`. The updated control flow preserves `ES_EINVAL` for a null pointer after initialization. No mismatch was found in this fix. The PowerPC test code compiled with `cargo check -p ogc-es -p ogc-es-sys --target powerpc-unknown-linux-gnu --tests`; tests were not executed. No hardware verification was performed.

## Uncertain differences

- **FIXED: successful ioctl return values are preserved.** C ES wrappers return the raw `IOS_IoctlvFormat` result, including nonnegative values. Rust now does the same for scalar-output and direct ioctlv wrappers instead of normalizing successful results to zero. This also preserves positive byte/count-style results if IOS supplies them.

## Public API and layout

- `ogc-es` is currently a `no_std` re-export of `ogc-es-sys` (`ogc-es/src/lib.rs`); it adds no higher-level wrapper behavior.
- C header API functions are represented by Rust `snake_case` functions and Rust pointer types. The different spelling is an idiomatic Rust API choice; no missing declared ES operation was identified in the function inventory.
- The packed Rust records mirror the C ES records. The Rust PowerPC test asserts key sizes, including `Tikview` 216, `Tik` 356, `TmdContent` 36, `Tmd` 164, and `TmdView` 92 (`ogc-es-sys/src/imp.rs` test module). No confirmed layout mismatch was found in the inspected declarations.

## Intentional Rust additions

- `patch_ahbprot_reset` is a Rust-only helper for restoring the HBC AHBPROT check. It is documented as using HBC's Wii-only patch sequence and does not correspond to a public libogc ES API (`ogc-es-sys/src/lib.rs:30-68`).

  The HBC reference advances a `u16` pointer by `0x4000` elements, covering
  0x8000 bytes. The Rust scan now covers that range; previously it covered
  only half. Rust requires the complete signature to fit within the range,
  while the C loop can read up to six halfwords past its end. This bounds the
  read, but leaves signatures starting in those final six halfwords unmatched.
  The memory patch and cache flush still require target verification.
  On 2026-11-28, the direct-DOL Dolphin attempt exited with `SIGABRT` after the
  guard section began and before the AHBPROT state was recorded. DOL SHA-256:
  `d9da5fb80863067c659f466275821142e1a3903a0ee2d38432b715b110b816f5`. The
  probe had not called the patch helper; its preceding `has_ahbprot` register
  read failed in this context. The helper can write MEM2 protection and ES
  module memory, so do not call it from a direct-DOL probe. Its error and patch
  paths remain unverified until a qualified HBC-context fixture and recovery
  path are available.
- Rust provides `ImportBootParams` as a parameter record for `import_boot`, while the C API uses a flat argument list. This is an interface-shape convenience; the fields map to the C arguments.
- Rust adds defensive pointer, alignment, size, and certificate-list validation around several raw calls. These checks intentionally return libogc ES error constants before issuing an ioctl; no adverse mismatch was confirmed from those checks in this review.

## Independent `es:` devoptab ABI re-review (2026-10-03)

**Historical finding, superseded by commit `33dbfe6` and the target-header check below.** Reviewer: `/root/reviewer_d`. Implementer of native registration fix: `worker_c`, commit `6bddd92`.

The native registration and populated callback slots pass. The `seek_r` width mismatch in this audit was fixed by `33dbfe6`; the earlier `st_ino` mismatch claim was incorrect for the target. The installed devkitPPC `machine/_types.h` defines `__ino_t` as `__uint32_t`, and the target `Stat.st_ino` is 32-bit, matching Rust's `Stat.st_ino: u32`.

The shared `ftruncate_r` callback type remains a separate ABI mismatch in the reviewed commit: C `sys/iosupport.h:59` uses target 64-bit `off_t`, while `FtruncateFn` in that commit takes `i32`. ES leaves the slot empty, but this shared callback type affects devices that register it. The current shared worktree contains an uncommitted widening to `i64`; it is outside commit `33dbfe6` and is not accepted by this review.

Validation for the earlier audit: `cargo check -p ogc-es -p ogc-es-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled test code; tests did not run. No hardware verification was performed.


## 2026-10-02 `es:` filesystem parity implementation

The previously open filesystem finding is implemented against current `libogc/es.c`. `init()` now registers the read-only `es:` device after `/dev/es` is available, and `close()` removes the device before closing the ES descriptor. `reset()` intentionally keeps C's separate behavior and does not de-register the device.

The implementation is centralized in `ogc-es-sys/src/imp/filesystem.rs` and reuses `ogc-sysbase::Devoptab`, `Reent`, `Stat`, device registration, and shared errno constants. It supports all four C path forms: current-title content index, current-title content ID, explicit-title content index, and explicit-title content ID. The decoder resolves the current title, loads and validates the stored TMD, and selects the matching `TmdContent`.

The devoptab provides C's open, close, read, seek, fstat, and stat callbacks and leaves write/mutation/directory callbacks absent. Opens are read-only. Current-title content uses `open_content`; explicit-title content obtains aligned ticket views and uses `open_title_content`. Each open file has its own LWP mutex and lazily allocated 64 KiB, 32-byte-aligned bounce buffer for unaligned reads. Aligned reads go directly to ES. `stat` metadata follows C's content-ID inode, title-derived device/uid, shared-content group-read bit, content size, 512-byte block count, and 16 KiB block size rules.

The low-level `read_content` wrapper was also corrected to return the nonnegative IOS result, which is the number of bytes read in C. Previously Rust normalized successful reads to zero, which would make the filesystem read path unable to advance or detect short reads.

Focused target-side tests cover the documented path forms, malformed/overflowing paths, and C stat synthesis rules. No Wii hardware verification is claimed in this handoff.

## Independent re-review: 2026-10-03

**Result: NEEDS_CHANGES.** Reviewer: `reviewer_e`. Implementer: `worker_c`. Implementation commit reviewed: `c5173e2` (`Complete ES and MP3 player parity`). Scope: `ogc-es` and `ogc-es-sys`, especially the added Wii `es:` filesystem and `ES_ReadContent` return value.

Reviewed C `C-sources/libogc/libogc/es.c` (`__ES_Init`, `__ES_Close`, `ES_OpenContent`, `ES_OpenTitleContent`, `ES_ReadContent`, `_ES_decodepath`, `_ES_open_r`, `_ES_close_r`, `_ES_read_r`, `_ES_seek_r`, `_ES_fillstat`, `_ES_fstat_r`, `_ES_stat_r`, `__ES_InitFS`, and `__ES_DeinitFS`; lines 124–169, 805–829, 960–1293) and declarations in `C-sources/libogc/gc/ogc/es.h` (lines 256–304). Reviewed Rust `crates/core/ogc-es-sys/src/imp.rs` (`init`, `close`, `read_content`, `open_title_content`, and ES metadata wrappers) and `crates/core/ogc-es-sys/src/imp/filesystem.rs` (`parse_path`, `decode_path`, callbacks, `fill_stat`, and registration; lines 79–565), plus `crates/core/ogc-sysbase/src/lib.rs` device registration and dispatch (lines 296–390, 538–628).

The callback behavior inspected matches the C implementation for the supported paths, content selection, read-only opens, per-file close/read/seek handling, and stat synthesis. The `read_content` return-value change also matches C: `ES_ReadContent` returns the `IOS_IoctlvFormat` result directly (`es.c:818–829`), and Rust now returns `rc` directly (`ogc-es-sys/src/imp.rs:2125–2147`).

One confirmed integration mismatch remains. C registers `dotab_es` through libogc `AddDevice` and removes it through `RemoveDevice` (`C-sources/libogc/libogc/es.c:1287–1292`), making the device available through the C devoptab filesystem. Rust `init_fs`/`deinit_fs` instead call the Rust-only `sysbase::add_device`/`remove_device` (`ogc-es-sys/src/imp/filesystem.rs:555–565`). Those functions mutate only `DEVICE_REGISTRY` (`ogc-sysbase/src/lib.rs:575–594, 615–628`), which Rust `DeviceFile` resolves through `device_op_tab` (`ogc-sysbase/src/lib.rs:538–555`). They do not call libogc `AddDevice`/`RemoveDevice` or update the C devoptab table. Therefore Rust callers using `DeviceFile` can resolve `es:`, but C callers using normal newlib/libogc path APIs do not receive the registration that C `__ES_InitFS` provides. This leaves the claimed C filesystem-registration parity incomplete.

The new regression tests in `filesystem.rs:571–632` cover parser cases and a stat helper only; they do not exercise registration visibility through the C devoptab API or the content-read return count. `cargo check -p ogc-es -p ogc-es-sys --target powerpc-unknown-linux-gnu --tests` passed. This compiled the target test code; tests were not run. No hardware verification was performed. The review note is updated; ledger changes are left to the coordinating reviewer.

## BUG FIX handoff: native `es:` devoptab registration

Implementer: `worker_c`. Independent reviewer: assigned by the coordinating review thread. This follow-up addresses the remaining registration mismatch above.

`init_fs()` now registers a libogc-compatible `es` devoptab with C `AddDevice` when C `FindDevice` does not find it. The callbacks are the same callbacks used by the Rust `DeviceFile` registry. C `RemoveDevice("es:")` is called from `deinit_fs()`, matching `__ES_DeinitFS`; Rust's own `es:` registry is still maintained for Rust path handling. The separate bare-name descriptor is required because C's `devoptab_t` uses the bare name (`"es"`), while the Rust registry compares colon-terminated names.

Added a regression test for the bare C name and for matching open, read, and stat callbacks across both descriptors. `cargo check -p ogc-es -p ogc-es-sys --target powerpc-unknown-linux-gnu --tests` passed and compiled the test code. Tests were not executed. `cargo fmt -p ogc-es-sys -- --check` reports formatting differences in pre-existing code in `filesystem.rs` and `lib.rs`; the new test has been formatted. No hardware verification was performed. Independent re-review is required; the coordinator manages the ledger.

## Independent re-review — native `es:` devoptab registration

**Result: NEEDS_CHANGES.** Reviewer: `/root/reviewer_e`. Implementer: `/root/worker_c`. Implementation commit: `6bddd92`.

The native registration and removal path now follows C: C `__ES_InitFS`/`__ES_DeinitFS` call `AddDevice`/`RemoveDevice` (`C-sources/libogc/libogc/es.c:124–169, 1287–1293`), and Rust registers the native descriptor during initialization and removes it on close (`ogc-es-sys/src/imp.rs:414–434`, `imp/filesystem.rs:563–623`).

The registered seek callback still has an ABI mismatch. C `_ES_seek_r` takes and returns `off_t` (`es.c:1185–1203`), which is 64-bit on this target. Rust registers `es_seek` with 32-bit `i32` input and output through `SeekFn` (`ogc-sysbase/src/lib.rs:148–150`; `filesystem.rs:434`). A newlib caller can pass or receive a truncated offset or call the function with the wrong ABI. The regression test verifies callback identity between Rust descriptors, but not compatibility with C's callback type.

The PowerPC test target compiled; tests were not run. `git diff --check` passed; `rustfmt --check` reported pre-existing formatting differences. No hardware verification was performed. The registration fix passes its bounded scope, but ES remains open for the seek ABI mismatch and earlier findings.

## Independent confirmation of seek callback ABI finding (2026-10-03)

Reviewer: `reviewer_b`. The open `es_seek` mismatch is a callback ABI difference. C devkitPPC declares devoptab `seek_r` with `off_t` input and return, which is 64-bit on this target (`sys/iosupport.h:43`; `machine/_types.h:6-8`; `sys/types.h:154-156`). Rust registers an `i32 -> i32` callback through `SeekFn` (`crates/core/ogc-sysbase/src/lib.rs:148-150`; `crates/core/ogc-es-sys/src/imp/filesystem.rs:434-459,528-536`). Both callback layers map negative results to `EINVAL`, but the underlying C `ES_SeekContent` takes 32-bit values; its internal width does not make the outer callback ABI compatible. No build or tests were run; no hardware verification was performed.

## Fix pass: newlib `seek_r` offset width

**BUG FIX.** Implementer: `worker_c`. Independent re-review: needed. The shared Rust `SeekFn` now uses 64-bit `i64` input and return values, matching devkitPPC's 64-bit `off_t`. ES `es_seek` has that callback signature, then narrows the offset to the 32-bit `ES_SeekContent` argument as C does. The ISO9660 callback also uses the corrected shared signature; its tracked file offset is now 64-bit, matching its C file-state field. `DeviceFile::seek` keeps its existing i32 convenience signature, and `seek_off_t` exposes the full callback width.

The stated inode-width concern does not reproduce with the installed devkitPPC headers. `machine/_types.h` defines `__ino_t` as `__uint32_t`, and `sys/stat.h` uses `ino_t` for `st_ino`; Rust `Stat.st_ino` is already `u32`. A target C compile-time check confirms `sizeof(off_t) == 8`, `sizeof(ino_t) == sizeof(st_ino) == 4`, `offsetof(st_ino) == 4`, and `sizeof(struct stat) == 96`. Added Rust tests verify the seek callback type and `Stat` layout, and the metadata test checks a full 32-bit inode value.

Validation: `powerpc-eabi-gcc -std=gnu11 -fsyntax-only` passed static assertions against the installed devkitPPC headers. `cargo check -p ogc-es -p ogc-es-sys -p iso9660 --target powerpc-unknown-linux-gnu --tests` and scoped `git diff --check` passed. Test code compiled but tests did not run. `cargo fmt -p ogc-sysbase -p ogc-es-sys -p iso9660 -- --check` reports existing formatting differences across these crates; the added callback test is formatted. No hardware verification was performed. Other ES filesystem behavior findings remain separate.

## Independent re-review of shared `off_t` callback fix `33dbfe6`

**Result: NEEDS CHANGES for shared devoptab ABI parity.** Reviewer: `/root/reviewer_f`; implementer: `worker_c`.

The `seek_r` fix passes. Target `sys/iosupport.h:43` uses 64-bit `off_t`; Rust `SeekFn` is now `i64` input/output. ES narrows to its 32-bit `ES_SeekContent` argument and widens the result, matching C `_ES_seek_r` (`C-sources/libogc/libogc/es.c:1185-1203,827-831`). ISO9660's callback and tracked offset are also 64-bit, matching `_ISO9660_seek_r` (`C-sources/libogc/libiso9660/iso9660.c:530-567`). The devoptab field order and pointer size do not change. The earlier inode concern is not a target mismatch: `machine/_types.h:13` defines target `__ino_t` as `__uint32_t`; Rust's `Stat.st_ino` is `u32`, and the target C layout check in the handoff matches its size and offset.

However, the same commit leaves `FtruncateFn` at `i32` while C `sys/iosupport.h:59` takes `off_t` for `ftruncate_r`'s length. The worktree now has an uncommitted `i64` widening, but that change is outside `33dbfe6` and needs its own review. ES does not populate this slot; the mismatch is in the shared sysbase callback type. ES initialization and filesystem parity findings remain separate.

Validation: PowerPC checks for `ogc-es`, `ogc-es-sys`, and `iso9660` passed on the current worktree and compiled tests; tests did not run. `git diff --check` passed. The check was not an isolated checkout of `33dbfe6`. No hardware verification was performed.

## 2026-10-05 devoptab registration fix handoff

Implementer: `/root`. `ogc-sysbase` registration helpers now use newlib's `AddDevice`, `FindDevice`, and `RemoveDevice` while preserving their Rust registry for `DeviceFile`; therefore `ogc-es-sys::init_fs` and `deinit_fs` now register and remove `es:` in the same native device table used by C path APIs. The sysbase handoff and PowerPC test-code compilation are recorded in `libipc-core.md`. Independent static re-review is needed. Other ES ABI, return-value, and initialization differences remain open. No hardware verification was performed.

## Independent static re-review: `ogc-es` / `ogc-es-sys` (2026-10-05)

**Result: NEEDS CHANGES for ES filesystem device registration.** Reviewer: `worker_f`. Implementer: `/root` for the current Rust-owned devoptab table and `worker_c` for the ES filesystem callbacks. Reviewed the current registration and callback code, including commits `33dbfe6`, `9334164`, `5419995`, and `7001d38` where they apply. `ogc-es` is a re-export of `ogc-es-sys` (`crates/core/ogc-es/src/lib.rs`), so it adds no separate runtime behavior.

The seek callback-width fix passes its bounded C comparison. The installed target header declares `devoptab_t.seek_r` with 64-bit `off_t` (`${DEVKITPRO}/devkitPPC/powerpc-eabi/include/sys/iosupport.h:43`; `machine/_types.h:6-8`). Rust's shared `SeekFn` and `es_seek` now use `i64` at the devoptab boundary, then narrow to the 32-bit `ES_SeekContent` argument and widen the result (`crates/core/ogc-sysbase/src/lib.rs:180-181`; `crates/core/ogc-es-sys/src/imp/filesystem.rs:426-446`). C `_ES_seek_r` has that same outer `off_t` signature and passes the value to `ES_SeekContent` (`../../C-sources/libogc/libogc/es.c:1185-1203, 827-831`).

The current native-registration handoff does not match the C device name and therefore cannot register a usable `es:` path. C's `dotab_es` stores the bare name `"es"`, then registers it with `AddDevice` and removes it with `RemoveDevice("es:")` (`../../C-sources/libogc/libogc/es.c:1264-1266, 1287-1293`). Rust stores `ES_DEVICE_NAME = b"es:\0"` directly as `Devoptab.name` (`crates/core/ogc-es-sys/src/imp/filesystem.rs:19, 521-523`). The Rust `FindDevice` path matcher expects the colon after the bare device name, or a single digit followed by a colon (`crates/core/ogc-sysbase/src/lib.rs:568-576, 584-603`). With the current name `"es:"`, `FindDevice("es:")` has no delimiter after the stored name, and `FindDevice("es:/...")` sees `/` where it expects `:` or a digit-plus-colon. Both lookups fail. Consequently `GetDeviceOpTab` cannot resolve normal `es:` paths; `deinit_fs` returns early without calling `RemoveDevice`, so `close()` can leave the descriptor in `devoptab_list` after closing `/dev/es` (`filesystem.rs:568-575`; `imp.rs:433-449`). The current test that expects the colon in `Devoptab.name` codifies this mismatch (`filesystem.rs:216-227`).

### Accepted safety deviations

- Rust bounds the stored TMD size and validates its signed layout before reading content entries (`filesystem.rs:196-214`), where C passes the IOS-reported size into its fixed stack buffer without an explicit size check (`es.c:1001-1035`). This prevents an out-of-bounds read.
- Rust checks ticket-view count and fetch results and reports an error before opening a title's content (`filesystem.rs:105-131`). C ignores these two results before attempting the open (`es.c:1088-1096`). Returning an error on failed prerequisites is a safe failure.
- Rust's `FileGuard` releases the per-file mutex on callback error paths. C's close/read error branches can return before unlocking (`es.c:1145-1180`). Avoiding a permanently held mutex is accepted as a safety correction.

### Uncovered scope and validation limits

The init guard and `get_title_id` error precedence remain corrected in the current source (`imp.rs:416-429, 463-481` versus C `es.c:124-152, 179-192`). The ES ioctl wrappers were re-audited in the 2026-10-07 follow-up. Rust now preserves the raw nonnegative IOS result throughout the direct wrapper surface, matching C. The checked C tree has the ES `devoptab_t` use but not the newlib `AddDevice` implementation. The installed header verifies callback declarations and `STD_MAX`, not every registration-table semantic in the Rust replacement. The earlier target checks and layout assertions compiled test code, but tests did not run. I did not run a fresh target build or final application link after the Rust-owned table changes. No hardware verification was performed.

## Independent re-review — ES devoptab name fix (2026-10-05)

**Result: the bounded `es:` registration-name mismatch is fixed; broader ES parity remains open.** Reviewer: `worker_e`. Implementer: `worker_f`, commit `9289c94` (`Fix ES devoptab registration name`). I inspected the complete commit diff and compared registration, lookup, and removal with C.

C defines `dotab_es.name` as the bare string `"es"`, calls `AddDevice(&dotab_es)` from `__ES_InitFS`, and calls `RemoveDevice("es:")` from `__ES_DeinitFS` (`../../C-sources/libogc/libogc/es.c:1264-1285,1287-1293`; initialization and close call these hooks at `:124-152,154-169`). Rust now stores the bare NUL-terminated name in `ES_DEVICE_NAME` and keeps the colon-terminated lookup key separately in `ES_DEVICE_PATH` (`crates/core/ogc-es-sys/src/imp/filesystem.rs:19-20,521-523`). `init_fs` uses `FindDevice("es:")` before registering the descriptor; `deinit_fs` finds and removes that same path (`filesystem.rs:557-579`). This matches C's name/path distinction. The current sysbase path matcher requires the exact registered name followed by `:` (`crates/core/ogc-sysbase/src/lib.rs:571-578`), so it resolves `es:` and `es:/...` through the bare `es` entry. The sysbase regression covers both forms and registration/removal lookup behavior (`ogc-sysbase/src/lib.rs:693-761`).

The ES-specific test checks the stored bare name, separate `es:` path, and populated open/close/read/seek/stat callback slots (`filesystem.rs:589-605`). It did not execute `init_fs`/`deinit_fs` against the global devoptab table. At this review, the prior colon-name mismatch was fixed, but a duplicate-registration edge remained: Rust returned `ES_EINVAL` when `init_fs` found an existing `es:` entry, while C called `AddDevice` unconditionally (`es.c:1287-1290`). The installed devkitPPC `libsysbase.a` `AddDevice` disassembly (`0x3c-0x9c`) showed that it replaces a same-name entry. Commit `a6019c0` addresses this duplicate-registration edge as recorded in the independent re-review below. The full ES crate remains open for initialization, ioctl return-value, and other findings recorded above.

Validation: `cargo check -p ogc-es -p ogc-es-sys --target powerpc-unknown-linux-gnu --tests` passed and compiled test code; tests were not run. `cargo fmt -p ogc-es-sys -- --check` passed. I did not run a final application link or hardware verification.

## Independent re-review — duplicate ES devoptab registration (2026-10-05)

**Result: PASS for commit `a6019c0`.** Reviewer: `worker_f`. Implementer: `worker_e`. This commit resolves the duplicate-name mismatch recorded in the prior ES registration review.

C's `__ES_InitFS` calls `AddDevice(&dotab_es)` without first rejecting an existing `es` entry (`../../C-sources/libogc/libogc/es.c:1264-1266,1287-1289`). In the installed devkitPPC `libsysbase.a` `AddDevice` implementation, the table scan selects the first null slot, same-name entry, or `stdnull` entry; the same-name case replaces that slot in place. C deinitialization later calls `RemoveDevice("es:")` (`es.c:1291-1293`), and `RemoveDevice` clears the selected slot to null. The checked-in libogc tree does not include this newlib implementation; these table details are confirmed from the installed target library.

Rust `init_fs` now always calls `sysbase::add_device` and propagates a negative registration result (`crates/core/ogc-es-sys/src/imp/filesystem.rs:557-564`). Rust `AddDevice` uses the same-name entry at its current index and returns that index (`crates/core/ogc-sysbase/src/lib.rs:613-635`). `deinit_fs` removes the entry through the `es:` path (`filesystem.rs:567-578`), and Rust `RemoveDevice` clears the selected slot to null (`ogc-sysbase/src/lib.rs:602-608`). The new regression installs an existing bare-name `es` descriptor, verifies lookup, calls `init_fs`, checks the same slot now points to `ES_DEVOPTAB`, then calls `deinit_fs` and confirms lookup fails (`filesystem.rs:584-618,639-658`). This covers replacement and cleanup against the Rust-owned devoptab table used by the production path.

Rust returns the actual AddDevice error if the registry has no available slot; C's ES initializer discards that result. Error propagation is an accepted safety correction and does not affect the tested successful replacement behavior. `cargo check -p ogc-es -p ogc-es-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled target test code; tests did not run. `rustfmt --check` on `filesystem.rs` and its scoped diff check passed. No final application link or hardware verification was performed. Other ES findings and uncovered scope remain open.

## 2026-10-06 source-status reconciliation

The current source has no remaining confirmed non-safety ES mismatch in the named blocker set. Explicit initialization, `get_title_id` error precedence, raw successful ioctl return values, and the `es:` filesystem registration semantics are fixed. `ogc-es` and `ogc-es-sys` remain `needs_re_review` only because target/runtime evidence has not been rerun in this environment; runtime or independent review can reopen a blocker if it demonstrates a concrete mismatch.

## Independent re-review — ES raw IOS return values (2026-10-07)

**Result: PASSED for the current ES return-value fix.** Reviewer: `/root` in a new review thread. C `ES_GetTitleID`, scalar count getters, and direct ioctlv wrappers return the raw nonnegative `IOS_IoctlvFormat` result (`../../C-sources/libogc/libogc/es.c:187–190, 286–303, 311–326`). Rust now preserves that result through scalar output helpers and direct ioctlv paths in `crates/core/ogc-es-sys/src/imp.rs`; it writes output values only after nonnegative completion. No mismatch was found in the current change. PowerPC target test code compiled with `RUSTFLAGS='-Aunsafe-op-in-unsafe-fn'`; tests were not run. Existing ES filesystem and ABI scope has no confirmed open parity finding after the earlier fixes recorded above. No hardware verification was performed.

## IOS launch ES error precedence — 2026-11-26

C checks the ES descriptor before validating arguments in
`ES_GetNumTicketViews`, `ES_GetTicketViews`, `ES_LaunchTitle`, and
`ES_LaunchTitleBackground`. Rust checked arguments first. These four Rust
entry points now check the descriptor first. In Dolphin, the baseline probe
failed the closed-ES precedence check; after the change, the four calls return
`ES_ENOTINIT` with invalid arguments while ES is closed and `ES_EINVAL` for
those arguments while ES is open. The same run reloaded IOS 58 and passed 16
checks. Evidence is in
`../dolphin-runs/2026-11-26-es-argument-precedence-red/` and
`../dolphin-runs/2026-11-26-es-argument-precedence-green/`.

This covers the four named calls and the tested invalid arguments. Other ES
entry points and other validation combinations require separate review.
Independent review and physical Wii validation remain pending.

The same descriptor-first comparison was extended to 16 title enumeration,
TMD, title-content, consumption, and shared-content entry points. The Dolphin
baseline failed the closed-service check and passed the open-service check;
after the change both passed, as did the IOS 58 reload. The grouped probe
checks every one of the 16 return values. The final run reports 18 checks and
no failures; see `../dolphin-runs/2026-11-26-es-inventory-precedence-red/` and
`../dolphin-runs/2026-11-26-es-inventory-precedence-green/`. This only covers
the tested invalid inputs. Independent review, other ES calls, and physical Wii
validation remain open.

The descriptor-first check was also moved ahead of argument validation for 24
ES operations covering DI views, identification, ticket and title operations,
content I/O, cryptography, device information, and boot import. The probe uses
invalid arguments that are rejected before an IOS request; it does not write to
NAND. Dolphin's baseline failed the grouped closed-service check. The fixed
run passed both grouped checks, the earlier ES checks, and IOS 58 reload: 20
checks passed. See `../dolphin-runs/2026-11-26-es-operation-precedence-red/`
and `../dolphin-runs/2026-11-26-es-operation-precedence-green/`. This is not a
successful-operation test for these APIs. Independent review and physical Wii
validation remain pending.

## ES close preserves state after IPC rejection — 2026-11-27

The independent lifecycle review found that `close_state` removed the ES
descriptor and filesystem registration before IPC accepted the close request.
The IOS replacement gate can reject that request while the old IOS still owns
the descriptor. Rust removes `es:` before it waits for IPC, as C does. If IPC
rejects the request before IOS replies, Rust restores `es:` and keeps the
descriptor. If IOS replies with a close error, Rust clears local state as C
does. This is a Rust safety difference from C's unguarded submission sequence.

The lifecycle regression tests cover cleanup order, IPC rejection, IOS reply
error, and success. PowerPC test code compiles with `cargo check --tests`; the
target test binary cannot link with the installed host linker, so these
assertions have not run. If IPC rejects close and `es:` registration cannot be
restored, Rust keeps the descriptor, returns the registration error, and leaves
`es:` absent until a later `init` succeeds. Runtime concurrency behavior
remains unverified. This fix does not add a lock around the full reload
sequence; that source-level gap remains open.

## 2026-11-26 follow-up: operation order and lifecycle

Reference: C `7d4b8b3e7b84`; Rust base `ae51d7e19a13` plus local changes.
The `ogc-es` facade re-exports `ogc-es-sys`; review the production paths in
`src/imp.rs`, the filesystem implementation, and their C equivalents in
`libogc/es.c`. The existing filesystem audit remains applicable. Review the
service check, argument checks, vector direction and size, result propagation,
output copyback, and cleanup separately; normal success alone does not prove
error precedence.

| Area | Source comparison and result |
| --- | --- |
| Initialization, close, reset (`es.c:124-177`) | Register on each successful init; remove only when a descriptor is open; close and discard the descriptor even if device removal fails; reset invalidates the descriptor. Keep Rust registry-error reporting rather than discard registry errors as C does. |
| UID (`es.c:194-205`) | After a nonnegative IOS result, call ISFS deinitialize; call initialize only if deinitialize fails. Return the original IOS result. The existing ISFS error-reporting difference is retained. |
| Catalog, ticket views, stored metadata, contents, shared contents | Compare descriptor guard, counts, layout sizes, pointer/alignment checks and ioctl vector directions; preserve bounded Rust validation. Existing success/invalid-argument coverage is not exhaustive. |
| Consumption (`es.c:434-455`) | Copy the zero-initialized local count back after submission, even on IOS failure. Use an explicit null/zero-length omitted vector. Host tests cover error copyback; Dolphin covers zero-count and descriptor rejection. |
| DI views (`es.c:457-498`) | Validate TMD length before alignment and preserve omitted null vectors. C `ES_DiGetTMDViewSize` tests `!size` on a local initialized to zero, so its request is unreachable. Intentionally retain the working Rust request and output-pointer check; copy the local value on all submitted results, as in the intended C path. Dolphin tests the valid System Menu view and no-current-title error. |
| Identify / DI verify (`es.c:576-650`) | Release owned allocations before close/init, preserve the original IOS result (including positive values), and copy Identify key output only on success. Use explicit omitted CRL vectors. Reject truncated headers and null ticket views for safety; initialize owned output memory before borrowing it. |
| Install, reimport, export, delete, launch and boot (`es.c:~500-809,~860-875`) | Compare dispatch, vector widths and guards. Preserve explicit null vectors for omitted CRLs/boot fields and reject undersized signatures. Successful privileged or destructive operations are not established by this run. |
| Content I/O (`es.c:805-850`) | Check descriptor before the ticket-view pointer in `open_title_content`; reject null view for safety. Dolphin opens/reads/seeks/closes System Menu content and tests closed-state precedence. |
| Crypto, signing, device information and certificate helpers | Existing general probe covers layouts, bounded outputs, AES-CBC vector and invalid inputs. Successful signing and verification coverage and allowed buffer aliasing need explicit follow-up; the Rust borrowed-slice API must not assume that overlapping C inputs are valid. |
| `es:` devoptab | Keep the existing filesystem audit and tests; lifecycle registration is corrected here. Exact ES-heap allocation failure behavior differs where the typed Rust IPC path uses its own allocation/staging strategy. Do not claim exact allocator exhaustion parity without a test. |

Additional intentional safety differences include null-output checks and
bounds checks before reading TMD/CRL signatures. The shared ISFS implementation
reports errors that C ISFS discards; do not change that separate contract to
force ES parity. A complete matrix for all malformed combinations, aliasing,
allocation failures and successful privileged operations has not been proved.

Verification: host `xtask` tests (161 passed), core target group (25 packages),
Dolphin `general_probe` (94 passed) and `ios_reload_probe` (21 passed). All ten
captured pages passed. See [fresh evidence](../dolphin-runs/2026-11-26-es-lifecycle-linux/README.md).
Stock Dolphin deliberately denies direct PowerPC DI verification in
`Source/Core/Core/IOS/ES/ES.cpp:805-808`. Host tests cover release-before-reopen
and preservation of the original result, and the UID deinitialize/initialize
condition. Successful privileged IOS operations require separate suitable
fixtures or emulator support. The package status remains `needs_re_review`;
this change does not promote all APIs to verified. Physical Wii evidence
remains explicitly pending and is separate from emulator completion.

### Shared IOS filesystem descriptor

The IOS IPC crate owns the `/dev/fs` descriptor and its open and close
operations. The ISFS API and the ES UID completion path use the same state.
This removes the ES ABI crate's dependency on the filesystem service crate
and retains the UID result and cleanup order described above.

On Dolphin 2609a, `runtime_probe` passed 889 checks with zero failures and
50 valid pages. The checks cover ISFS deinitialization and reinitialization,
closed-service errors, live file operations, and ES UID rejection when ES
is closed. The host UID completion test covers callback order for simulated
IOS success and failure. Successful privileged UID changes were outside
this run.

## Safety-limited ES operations — 2026-11-27

The current-tree `runtime_probe` passed 908 checks with zero failures. Seven
new closed-service checks cover `add_title_start`, `add_content_data`,
`add_ticket`, `export_title_init`, `export_content_data`,
`reimport_title_init`, and `delete_ticket`. Each returns `ES_ENOTINIT` before
it checks or reads valid but incomplete arguments while ES is closed. The
checks issue no IOS request. See the [run record](../dolphin-runs/2026-11-27-es-closed-service-guards/README.md).

The `ios_reload_probe` checks `import_boot` with null, zero-length inputs while
ES is closed and after it is open. The calls return the expected service-state
or argument errors before they submit an IOS request. The inventory records
this narrow guard evidence as `invalid-arguments`; valid boot import remains
unverified.

These checks establish closed-service error precedence only. The successful
and IOS-error behavior of title/content installation and deletion,
export-session state, UID changes, `ES_Reset`, and the Rust-only
`patch_ahbprot_reset` remains unverified. Do not test successful boot import,
title mutation, or UID changes until the probe has a disposable, known-good
title fixture and a qualified recovery path. The AHBPROT helper writes the
MEM2 protection register and may patch ES code, so its runtime check needs an
environment with an explicit reset path. These APIs remain open.


## IOS replacement and ES close safety — 2026-11-27

`FileDescriptor::close_with_status` consumes the handle and returns it only
when IPC rejects the request before IOS replies. ES removes its filesystem
registration before the request and restores it only when it existed before
close. `reopen_after_verify` does not reopen ES while a rejected close left the
descriptor active. A lifecycle guard now covers direct launch and the full
reload sequence, so a competing call receives `IPC_EQUEUEFULL` before it can
change ES or STM state.

PowerPC test code compiles. `cargo xtask test-host` and strict Clippy pass.
The corrected Dolphin 2609a run passes 132/132 checks. A competing LWP calls
`ogc_es_sys::close()` while the replacement scope is active. The call returns
`IPC_EQUEUEFULL`; ES remains usable after the scope ends and closes normally
afterward. The probe also verifies that its worker stops IPC before it releases the
replacement scope. Physical Wii behavior and independent concurrency review
remain open. See the
[`ios-replacement-es-close` run record](../dolphin-runs/2026-11-27-ios-replacement-es-close/README.md).
