# Static C Parity Review

Status: no confirmed non-safety parity blocker remains in the recorded scope; independent re-review pending.

## Scope

- `crates/services/ogc-di/src/lib.rs`
- `crates/services/ogc-di/src/retry.rs`
- `../../C-sources/libogc/libdi/di.c`
- `../../C-sources/libogc/gc/di/di.h`

This review covers the Rust-owned DI implementation and its `DiscInterface` callbacks. It does not cover `ogc-dvd` or `ogc-dvd-sys`. The Rust crate exports the legacy `DI_*` C ABI. Hardware behavior was not checked.

## Confirmed parity findings

The four findings below describe the pre-fix code in commit `76e588a`. The fix pass changes are recorded after the list.

1. **The Rust interface startup callback returns before mount finishes.** C's `diio_Startup` starts mount and waits while `DVD_INIT` is set, with a 15-second timeout (`../../C-sources/libogc/libdi/di.c:916-937`). Rust's `diio_startup` calls asynchronous `mount()` and immediately checks whether status is `DVD_READY` (`crates/services/ogc-di/src/lib.rs:1261-1264`). `mount()` sets `DVD_INIT | DVD_NO_DISC` before it queues the first ioctl (`:836-867`). Therefore the Rust callback returns false while its successful mount is still in progress. The C callback returns true after the mount reaches ready.

2. **Mount with no disc follows a different state flow.** C checks the cover register before it starts the callback chain. On an ioctl error or no inserted disc, it sets `DVD_NO_DISC` and returns (`../../C-sources/libogc/libdi/di.c:288-302`). Rust submits `DVD_GETCOVER` for every mount request and moves to `MOUNT_WAIT_FOR_COVER` when the response says no disc (`crates/services/ogc-di/src/lib.rs:507-517, 836-852`). Rust can therefore keep an active wait-for-cover request where C mount has already returned with `DVD_NO_DISC`. This also affects startup behavior.

3. **The `DiscInterface` shutdown callback closes more state than C.** C's callback only calls `DI_StopMotor` and returns true (`../../C-sources/libogc/libdi/di.c:971-975`). Rust also closes the partition and DI file descriptor, resets its read mode and status, and returns false when no file descriptor is open (`crates/services/ogc-di/src/lib.rs:1279-1292`). Callers can observe that later DI operations fail until Rust `init()` reopens the device, unlike the C interface callback.

4. **The `DiscInterface` clear-status callback has different results and effects.** C always returns true and performs no ioctl (`../../C-sources/libogc/libdi/di.c:966-969`). Rust calls `DVD_GET_ERROR` and returns false if that request fails (`crates/services/ogc-di/src/lib.rs:1275-1277`). This is a confirmed behavioral difference; whether the Rust behavior is preferable is a separate design choice.

## Intentional Rust differences and C defects avoided

- C provides a `writeSectors` callback that returns true without writing data (`../../C-sources/libogc/libdi/di.c:961-964`). Rust deliberately returns false because the DVD interface does not advertise write capability and reporting success without a write violates the repository real-implementation policy. This is an accepted safety/real-implementation deviation.
- Rust `init()` opens `/dev/di` without requiring AHBPROT. C returns success before opening the device when AHBPROT is absent (`../../C-sources/libogc/libdi/di.c:255-278`; `crates/services/ogc-di/src/lib.rs:788-805`). Rust's behavior supports the IOS transport without direct register access.
- Rust checks null pointers through safe references or explicit pointer checks. C omits checks in some functions, including `DI_GetCoverRegister` and `DI_ReadDiscID` (`../../C-sources/libogc/libdi/di.c:730-746, 896-913`).
- Rust's cache range test accepts a read whose exclusive end equals the cache end (`crates/services/ogc-di/src/lib.rs:463-500`). C uses a strict `<` test and misses that valid boundary hit (`../../C-sources/libogc/libdi/di.c:218-239`). Rust's result is the correct range behavior.
- **Fixed, needs independent re-review:** Rust `DI_ReadDVDAsync` now submits the active D0/A8 command directly through the Rust-owned IOS async transport, passes the caller callback through to IOS, uses the caller buffer as callback user data, and bypasses the synchronous sector cache. Unknown mode tries D0 submission first and A8 only if D0 submission fails, matching C. Both wrapper and helper flow use one mutex/locked-state serialization point.

## Validation limits

The fix pass added regression tests for mount precheck decisions, startup waiting and no-disc return behavior, motor-only shutdown, and unconditional clear-status success. `cargo check -p ogc-di --target powerpc-unknown-linux-gnu --tests` passed. `cargo test -p ogc-di --target powerpc-unknown-linux-gnu --no-run` compiled the crate and tests but could not link because no PowerPC Linux linker is installed; the available macOS linker rejects the PowerPC Linux linker options. The tests were not run.

This pass did not test IOS ioctl behavior, callbacks on hardware, cache coherence, DI registers, media handling, or timing on a Wii. No hardware verification is claimed.

## Fix pass changes

- **BUG FIX:** `diio_startup` now waits for `DVD_INIT` to clear, up to the C timeout of 15 seconds, and then returns whether the interface is ready (`crates/services/ogc-di/src/lib.rs:1326-1342`).
- **BUG FIX:** `mount` checks the cover register before it starts the asynchronous mount sequence. No disc or a cover-register error sets `DVD_NO_DISC` and does not queue a wait-for-cover request (`crates/services/ogc-di/src/lib.rs:878-928`).
- **BUG FIX:** `diio_shutdown` now attempts only to stop the motor and returns true. It leaves the handle and partition state open (`crates/services/ogc-di/src/lib.rs:1360-1373`).
- **BUG FIX:** `diio_clear_status` now returns true without issuing `DVD_GET_ERROR` (`crates/services/ogc-di/src/lib.rs:1355-1359`).
- A separate reviewer must compare these changes with C and inspect the final diff. The intentional read-only interface difference above remains recorded.

## Independent fix re-review

I compared fix commit `a147f30` with the C callbacks and inspected its complete DI source diff. The four fixes match the recorded C behavior: startup waits for initialization to finish and returns readiness; mount checks the cover register before queuing the asynchronous chain; shutdown only attempts the motor-stop operation and succeeds; clear-status returns true without an ioctl. The regression checks cover these paths, and the target check compiled the test code. No confirmed mismatch remains in those four fixes.

The crate remains open. The note still records that the asynchronous read callback path has not been established as behaviorally equivalent. This section predates the legacy ABI export and write callback fixes recorded later in this note.

## Additional confirmed mismatch: async read submission before initialization

Reviewer: `/root`. The Rust asynchronous read submission path does not preserve C's initialization error timing. C `DI_ReadDVDAsync` returns `-ENXIO` immediately when `di_fd < 0` and does not submit a callback (`../../C-sources/libogc/libdi/di.c:51,554-556`). Rust's initial `DiState` has `fd: None` (`crates/services/ogc-di/src/lib.rs:101-132`), but `read_dvd_async` validates the buffer and callback, reserves a slot, and queues `async_read_main` without checking the DI descriptor (`:1119-1175`). The worker later calls `read_dvd`; `current_fd` then returns `ENXIO` for the absent descriptor (`:320-322,1092-1106`). If the deferred queue accepts the request, Rust returns `0` to the submitter and later invokes the callback with `-ENXIO`; C returns `-ENXIO` synchronously without invoking the callback. This is an observable immediate-return and callback mismatch.

The existing async test checks only conversion of synchronous read results into callback statuses (`:1412-1416`). It does not cover submission before initialization. No tests were run for this review; static comparison only. No hardware verification was performed. This finding keeps `ogc-di` open.

## Independent re-review: NEEDS_CHANGES

Reviewer: reviewer_d. Implementer of the previously accepted DiscInterface callback fixes: notOrrytrout, commit `a147f30` (`fix(di): match disc interface callbacks to libogc`). The async submission mismatch was recorded by `/root` in docs-only commit `84dddc5`; no implementation fix for it is present in the current source.

I verified the uninitialized async mismatch directly. C `DI_ReadDVDAsync` checks `di_fd < 0` first and immediately returns `-ENXIO`; only if a descriptor exists does it look for `DI_ReadDVDAsyncptr`, call it, and return its submission result (`../../C-sources/libogc/libdi/di.c:554-565`). Rust starts with `DiState.fd == None` (`crates/services/ogc-di/src/lib.rs:101-132`), but `read_dvd_async` validates the buffer/callback, reserves a request slot, and queues `async_read_main` without checking `fd` (`:1119-1175`). The worker later invokes `read_dvd`, whose `current_fd` returns `ENXIO` when the handle is absent (`:320-322,1071-1073`); it releases the slot and invokes the callback with `-ENXIO` (`:1079-1108`). Thus, for valid arguments before init, C returns `-ENXIO` synchronously and does not call the callback, while Rust returns `0` if queuing succeeds and later calls the callback with `-ENXIO`. Invalid arguments also expose the ordering difference: C returns `-ENXIO` before validating them, while Rust returns `EINVAL` before checking initialization.

There is a related not-yet-mounted state mismatch. C leaves `DI_ReadDVDAsyncptr` null until the mount callback chain completes successfully and installs `_DI_ReadDVD_Check_Async` (`di.c:430-438`); with an open descriptor but no successful mount, `DI_ReadDVDAsync` returns `-1` without invoking a callback (`:554-565`). Rust does not gate submission on `DVD_READY` or a selected read mode. A valid request with an open descriptor can enter the deferred queue; `async_read_main` calls the normal `read_dvd` path, and `read_dvd_selected` attempts D0/A8 when `ReadMode::Unknown` (`crates/services/ogc-di/src/lib.rs:425-464,1079-1108`). This creates a later IO/callback path where C has not yet enabled async reads. Both pre-init and pre-mount requests need to preserve C's synchronous submission error and no-callback behavior if this API is intended as the C equivalent.

The async callback path after successful setup has intentionally different mechanics: C passes the caller callback directly to `IOS_IoctlAsync` through `_DI_ReadDVD_Check_Async` and returns the IOS submission result converted from 1 to 0 (`di.c:70-112,353-374`), while Rust executes a synchronous read on its deferred worker and then calls the supplied `Ipccallback`, converting read success 0 to callback status 1 (`ogc-di/src/lib.rs:73-79,1079-1108,1111-1175`). The timing and submission behavior differ, and the following cache-path comparison shows another result difference.

The worker also routes async reads through Rust's sector-cache path. C `DI_ReadDVDAsync` calls the active async function pointer directly (`di.c:554-565`), and `_DI_ReadDVD_Check_Async` dispatches straight to the D0/A8 async ioctl helpers without calling `ReadBlockFromCache` (`:353-374`). Rust `async_read_main` calls `read_dvd`, which calls `read_from_cache_or_drive` (`ogc-di/src/lib.rs:1092-1095,1071-1073`); with the default cache enabled, this can satisfy the callback from cached bytes or issue a 26-block cache fill before copying the requested sector range (`:466-501`). Therefore an async read after setup can avoid an IOS read in Rust when the requested blocks are cached, while C submits its async ioctl directly to the caller's buffer. This is another observable async-path difference; the existing code comment describes a “real DI read” but does not establish C-equivalent IOS submission.

The legacy C ABI remains absent, as the module itself documents (`ogc-di/src/lib.rs:3-8`). `gc/di/di.h` declares `di_fd`, `__io_wiidvd`, and legacy symbols such as `DI_Init`, `DI_Close`, `DI_ReadDVD`, and `DI_ReadDVDAsync` (`../../C-sources/libogc/gc/di/di.h:90-126`). Rust exposes Rust-named functions such as `init`, `close`, and `read_dvd_async`, plus `WII_DVD_INTERFACE`, and does not declare these as `#[no_mangle]`/`export_name` C symbols (`ogc-di/src/lib.rs:851-899,956-975,1071-1175,1397-1406`). The C ABI question therefore remains open and is separate from the async submission mismatch.

Validation: `cargo check -p ogc-di --tests --target powerpc-unknown-linux-gnu` passed, which compiled the crate and its target-gated test code. No tests ran. The current target tests check callback status conversion but do not test pre-init or pre-mount async submission (`ogc-di/src/lib.rs:1408-1416`). No hardware verification was performed. Result: `NEEDS_CHANGES`; the async pre-init and pre-mount submission differences remain, and the legacy `DI_*` ABI remains unimplemented/open.

## BUG FIX handoff: async read submission state checks

Implementer: `worker_c`. The coordinating implementation thread will mark the finding for independent re-review; reviewer assignment and review remain with the review thread.

`read_dvd_async` now checks state before it validates arguments or queues work. If `/dev/di` is not open, it returns `ENXIO` immediately. If the descriptor is open but status does not contain `DVD_READY`, it returns `-1` immediately. Both checks run while holding the DI state lock, so close or mount state changes cannot pass between the readiness check and queuing. The request cannot acquire a callback slot on either error path. This matches C's `DI_ReadDVDAsync`, which checks `di_fd` first and returns `-1` if `DI_ReadDVDAsyncptr` is not installed (`libdi/di.c:554–565`).

Added target-gated regression coverage for the pre-init and open-but-unmounted cases, including the C error precedence over invalid arguments and an unchanged callback counter. `cargo check -p ogc-di --target powerpc-unknown-linux-gnu --tests` passed and compiled the test code. Tests were not run. `cargo fmt -p ogc-di -- --check` reports existing formatting differences in the compile-error declaration and `audio_stream`; no formatter was applied to avoid unrelated changes. No hardware verification was performed.

The async worker still uses Rust's deferred synchronous read and cache path, while C submits D0/A8 directly through IOS async calls. That transport and cache behavior remains open. The crate also still lacks the legacy `DI_*` C ABI described above. This fix does not address either finding.

## Independent re-review of async submission state fix (2026-10-03)

**Result: NEEDS_CHANGES.** Reviewer: `/root/reviewer_e`. Implementer: `worker_c`. Fix commit: `3bfbbef` (`BUG FIX: check DI async read submission state`). I reviewed the complete commit diff for `crates/services/ogc-di/src/lib.rs` and its test/note additions. The pre-init and pre-mount error-order fix passes static comparison for the cases it claims to fix, but the crate still has known async-path mismatches and the new ready-state argument validation has a confirmed errno mismatch.

### Fix behavior that passes

C `DI_ReadDVDAsync` first returns `-ENXIO` when `di_fd < 0`; with an open descriptor, it returns `-1` if `DI_ReadDVDAsyncptr` is still null (`../../C-sources/libogc/libdi/di.c:554-565`). The mount callback installs that function pointer only after mount succeeds (`:430-438`). Rust's new `async_read_precheck` follows the same observable precedence for descriptor-not-open and not-ready states: `ENXIO`, then `-1`, then argument checks (`crates/services/ogc-di/src/lib.rs:326-339`). It runs before a callback slot is reserved or deferred work is queued (`:1136-1140`). Thus, the previously recorded valid-argument pre-init and open-but-unmounted requests now fail synchronously without a later user callback, as C does.

The added test covers no descriptor with valid and invalid arguments, and an open but not-ready descriptor with valid and invalid arguments. Its callback counter remains zero (`crates/services/ogc-di/src/lib.rs:1439-1485`). Per the handoff note, `cargo check -p ogc-di --target powerpc-unknown-linux-gnu --tests` passed, which compiled the target test configuration; tests did not run, and no hardware verification was performed. This review did not run tests.

### Fixed, needs independent re-review: misaligned async buffer errno

After descriptor and readiness checks pass, Rust now matches C's argument errors: a null buffer or missing callback returns `EINVAL`, while a non-null buffer whose address is not 32-byte aligned returns `EFAULT`. The check still runs before reserving a callback slot or queuing deferred work. Target-gated regression coverage exercises null, misaligned, missing-callback, and valid aligned cases through the production precheck helper.

### Existing async read differences remain

The fix only changes submission prechecks and does not change the deferred worker. C submits the D0/A8 ioctl asynchronously through `_DI_ReadDVD_Check_Async` (`../../C-sources/libogc/libdi/di.c:353-373,554-565`). Rust's worker calls synchronous `read_dvd`, which enters `read_from_cache_or_drive` (`crates/services/ogc-di/src/lib.rs:1096-1125`); with cache enabled it can return cached bytes or fill `CACHEBLOCKS` sectors and copy the requested range (`:483-505`). It also probes D0/A8 synchronously while `ReadMode::Unknown` (`:458-478`), whereas C probes using async ioctl submission (`di.c:353-373`). Thus the callback timing, number and size of IOS reads, and cache-hit behavior still differ after successful mount. These are pre-existing recorded findings and prevent marking the DI async path or crate complete.

The fix's state check does not install or model C's async function pointer separately; its `DVD_READY` check corresponds to the pointer being installed at mount completion in the ordinary state flow, but async transport/cache parity remains unresolved. The legacy `DI_*` C ABI is also still absent as recorded above. Do not mark the crate reviewed until these remaining findings are resolved or deliberately documented.

### Correction to earlier lock note

The earlier note incorrectly described a double-lock/single-unlock defect in C `DI_ReadDVDAsync`. The current source has one `LWP_MutexLock` and one `LWP_MutexUnlock` in the wrapper (`../../C-sources/libogc/libdi/di.c:554–565`); the D0/A8 async helpers do not lock that mutex (`:70–112, 353–374`). The async transport and callback differences above remain confirmed independently of that incorrect claim.

## Independent confirmation of the open async mismatch (2026-10-03)

Reviewer: `reviewer_a`. This audit confirms the async mismatch above and adds two concrete details. C's active async hook submits D0 through `IOS_IoctlAsync` for each request and tries A8 only if D0 submission fails synchronously (`../../C-sources/libogc/libdi/di.c:353-373,430-435`). Rust can retain a mode selected by an earlier synchronous read and use only that mode for later deferred requests; its initial D0/A8 probe is also synchronous (`crates/services/ogc-di/src/lib.rs:442-480,1096-1111`). For an open, mounted drive, C returns `-EFAULT` for a non-null misaligned buffer while Rust returns `EINVAL` (`di.c:70-110`; Rust `lib.rs:335-337`). C and Rust both return synchronously before queuing when the descriptor is closed or the drive is not ready. No builds or tests were run; no hardware verification was performed.

## Implementer handoff: legacy DI C ABI exports

Implementer: `/root/worker_b`. Added unmangled C exports for each function declared in `gc/di/di.h`, plus the `di_fd` and `__io_wiidvd` data symbols. The exported functions forward to the existing Rust operations; `di_fd` mirrors the descriptor opened or closed through Rust's `init` and `close`. The interface table has the C metadata and callback layout. A target test checks the exported interface metadata and selected C function signatures. `nm` confirmed all header-declared `DI_*` function names and both data symbols in the PowerPC rlib.

Validation: `cargo check -p ogc-di --target powerpc-unknown-linux-gnu --tests` and `cargo build -p ogc-di --target powerpc-unknown-linux-gnu` passed. The target test code compiled but tests were not run. `git diff --check` passed. No hardware verification was performed. Independent static re-review is needed; the review thread manages it and the ledger.

### Independent re-review of legacy C exports (`7f6418a`)

**Result: PASS for the added linker ABI surface.** Reviewer: `/root/reviewer_e`; implementer: `/root/worker_b`.

Commit `7f6418a` exports all functions declared in `C-sources/libogc/gc/di/di.h:90-126`, plus `di_fd` and `__io_wiidvd`. The Rust wrapper signatures use matching scalar, pointer, callback, and `extern "C"` types (`crates/services/ogc-di/src/lib.rs:1431-1639`). `DI_Close` has C's void return; the Rust internal result is discarded. `DI_DriveID` and `__io_wiidvd` use the matching C layouts (`di.h:80-84,91`; `gc/ogc/disc_io.h:56-67`; Rust `lib.rs:80-86,1438-1447`). `di_fd` mirrors the internal descriptor after init and close (`lib.rs:867-886,973-990,1449-1451`). The reviewer inspected the built PowerPC rlib with `nm` and found each expected export.

A documented callback-value difference remained at the time of this re-review: C's DVD `writeSectors` callback exists and returns true without writing; Rust exported `write_sectors: None` (`C-sources/libogc/libdi/di.c:961-964,977-985`; Rust `lib.rs:1438-1447`). The callback parity fix is recorded below. Async DI still uses deferred synchronous reads rather than C's direct IOS async submission.

Validation: PowerPC `cargo check` passed and compiled tests; tests did not run. `nm` confirmed the symbol set. No hardware verification was performed.

The exports provide the C linker surface but do not close the separate async transport/submission or error-precedence findings above. Rust's no-AHBPROT init behavior remains a documented difference.

## Implementer handoff — DVD writeSectors callback parity

Implementer: `/root/worker_b`. Independent re-review: needed; review thread owns it.

C's `diio_WriteSectors` callback returns true without writing data (`../../C-sources/libogc/libdi/di.c:961-964`), and the `__io_wiidvd` descriptor installs that callback (`:977-985`). Rust previously used `None` for `write_sectors` in both `WII_DVD_INTERFACE` and `__io_wiidvd`. Rust now installs a true-returning no-op callback in both descriptors. Async IOS submission and error-order behavior are separate findings and are unchanged.

Regression coverage invokes the callback through both descriptors and checks that each returns true. `git diff --check` and `cargo check -p ogc-di --target powerpc-unknown-linux-gnu --tests` passed; the target test code compiled but tests did not run. `rustfmt --edition 2021 --check` reports existing formatting differences elsewhere in `lib.rs`. No hardware verification was performed.

## Independent re-review: DVD `writeSectors` callback (`cf5e812`)

**Result: PASS for this bounded callback parity fix.** Reviewer: `worker_a`. Implementer: `worker_b`.

C's `diio_WriteSectors` returns true and is installed in `__io_wiidvd` (`../../C-sources/libogc/libdi/di.c:961–964,977–985`). Rust now installs the same true-returning callback in both `WII_DVD_INTERFACE` and the exported `__io_wiidvd` descriptor (`crates/services/ogc-di/src/lib.rs:1403–1456`). The test invokes the callback through both descriptors and checks the result. This reproduces the C interface callback's declared behavior; it does not add a storage write operation. The async IOS submission and error-order findings remain separate and open.

I inspected the complete source and test diff. No checks or tests were run during this re-review; the implementer's note records successful PowerPC target test compilation, with tests not executed. No hardware verification was performed.

## Coordinator review — DVD write callback needs changes under the real-implementation policy

The static C comparison above is correct: C `diio_WriteSectors` returns `true` without writing data (`../../C-sources/libogc/libdi/di.c:961-964`), and Rust now copies that behavior. However, Rust's callback is an unconditional success path with no write operation (`crates/services/ogc-di/src/lib.rs:1407-1413`). This violates the repository's real-implementation policy, which prohibits reporting success when the required operation did not complete. The Rust port may safely differ from this C bug: the DVD interface does not advertise `FEATURE_MEDIUM_CANWRITE` (`:1433-1440`), so a direct write callback should report unsupported/failure instead of success without writing.

The existing regression currently asserts `true` through both descriptors (`:1764-1768`), so it locks in the false-success behavior. This bounded fix is **not accepted** until the callback reports failure (or the interface omits it) and regression coverage checks that unsupported writes cannot return success. No tests were run in this coordinator review. This finding does not alter the separate async IOS submission and error-order findings.

## 2026-10-05 follow-up — async errno and unsupported DVD writes

The mounted async-read argument check now matches the C D0/A8 helpers for buffer alignment: null remains `EINVAL`, while a non-null 32-byte-misaligned buffer returns `EFAULT`. This removes the recorded errno parity blocker.

The DVD `writeSectors` callback deliberately no longer copies C's unconditional-success stub. The interface does not advertise `FEATURE_MEDIUM_CANWRITE`, and no write operation exists. Returning `true` would claim completion of an operation that never happened, which violates the repository real-implementation policy. Rust therefore returns `false` from both the Rust and legacy exported interface descriptors. This is an accepted safety/real-implementation deviation from C, not a parity blocker.

The successful async read path is now fixed at source level. Rust submits D0/A8 directly through `FileDescriptor::ioctl_async`, bypasses the synchronous sector cache, forwards the caller callback directly, and uses the original buffer as IOS user data. Unknown mode selects D0 or A8 from asynchronous submission success in the same order as C. This removes the last recorded non-safety DI blocker.

The crate remains `needs_re_review` until the direct PowerPC build and runtime/callback behavior are independently validated. The unsupported DVD write result remains an accepted safety/real-implementation deviation.

## 2026-10-05 completion pass — direct async IOS transport

The production `read_dvd_async` path no longer reserves a deferred-worker slot or calls synchronous `read_dvd`. After C-compatible descriptor/readiness/argument checks, it builds the D0 or A8 command in aligned static command storage and submits it directly with the Rust-owned `FileDescriptor::ioctl_async` API. The caller callback is supplied directly to IOS and the original read buffer is supplied as callback user data, matching libogc's `IOS_IoctlAsync(..., ipc_cb, buf)` behavior.

When the read mode is unknown, Rust submits D0 first. A successful submission selects D0 immediately; only a failed D0 submission tries A8, and a successful A8 submission selects A8. This matches `_DI_ReadDVD_Check_Async`. The asynchronous path does not consult or populate the synchronous sector cache.

This completion pass removes the last recorded non-safety DI parity blocker. It still requires independent PowerPC compilation and runtime validation of IOS completion/callback behavior before `ogc-di` can move from `needs_re_review` to `reviewed`.

## 2026-10-05 independent re-review — direct async IOS transport

**Result: PASS for the recorded DI parity fixes, with one accepted callback-safety deviation. Reviewer: `worker_d`.** I compared `read_dvd_async` and its direct submit helpers in `crates/services/ogc-di/src/lib.rs` with `_DI_ReadDVD_D0_Async`, `_DI_ReadDVD_A8_Async`, `_DI_ReadDVD_Check_Async`, and `DI_ReadDVDAsync` in `../../C-sources/libogc/libdi/di.c`. Rust now passes the caller buffer, 32-byte command, byte length, callback, and original buffer user data to `FileDescriptor::ioctl_async` (`lib.rs:1070-1098`), matching C's `IOS_IoctlAsync` calls and buffer/user-data arguments (`di.c:70-112`). Unknown mode submits D0 first and submits A8 only when D0 submission fails; it records the selected mode only after a successful submission (`lib.rs:1101-1127`; `di.c:353-373`). Descriptor/readiness precedence and null/misaligned buffer errors match (`lib.rs:292-307`; `di.c:70-112,554-565`). Async reads bypass the synchronous cache in both paths.

The legacy exports, callback table fields, and error-order fixes are documented in earlier bounded re-reviews in this note. The DVD `write_sectors` callback intentionally returns `false` (`lib.rs:1347-1355`), while C returns `true` without writing (`di.c:961-964`). This is an accepted real-implementation safety deviation: the interface does not advertise write support, and Rust must not report success for a write it does not perform.

Rust also rejects an absent async callback with `EINVAL` before submission (`lib.rs:292-307`). C passes a null callback through to `IOS_IoctlAsync` (`di.c:70-112`); libogc's IPC completion path simply skips callback invocation when the pointer is null (`../../C-sources/libogc/libogc/ipc.c:385-387`). This is an accepted real-implementation deviation: Rust requires an observable completion path so callers do not receive submission success for a read whose final result they cannot observe.

Validation limits: I did not run builds or tests in this re-review. The handoff reports PowerPC test-target compilation, but those tests were not run. IOS completion timing, callback delivery, and disc behavior were not verified on hardware. The recorded no-AHBPROT initialization difference remains documented and was not changed by this pass.
