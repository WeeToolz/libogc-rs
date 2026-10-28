# Filesystem and FAT Review

## Scope and result

This note records a static API and source comparison of the Rust crates `ogc-filesystem`, `ogc-filesystem-sys`, and `ogc-fat` against the C sources available in this workspace. Reviewer `/root` updated the `ogc-filesystem` result after independent callback audits by `/root/reviewer_4` and `/root/reviewer_6`.

| Crate | Result | Basis |
| --- | --- | --- |
| `ogc-filesystem` | Async `read_dir` count-copy fix `2df4a63` passed independent static re-review. Current audit also found close-error and invalid raw descriptor differences. Positive nonzero IOS `read_dir` behavior remains unverified. | Rust `crates/services/ogc-filesystem/src/lib.rs`; C `C-sources/libogc/gc/ogc/isfs.h` and `C-sources/libogc/libogc/isfs.c`. |
| `ogc-filesystem-sys` | ABI declaration review completed for the exposed filesystem types/constants; no mismatch confirmed. | Rust `crates/services/ogc-filesystem-sys/src/lib.rs`; C `gc/ogc/disc_io.h` and `gc/iso9660.h`. |
| `ogc-fat` | **Parity blocked.** The matching devkitPro/libfat implementation is available. Mount idempotence, primary/extended partition discovery, FAT12/FAT16 BPB and FAT-entry handling, and fixed FAT12/FAT16 root-directory traversal are ported. Variable logical-sector size and remaining libfat API/behavior coverage are confirmed open work. | Rust `crates/services/ogc-fat/src/lib.rs`; devkitPro/libfat `source/*.c`, `source/*.h`, and `include/fat.h`. |

The review is static. It does not establish runtime IOS behavior or hardware behavior. No implementation code was changed during the review.

## Fix handoff: async `read_dir` count copy

Implementer: `/root/worker_b`. Independent reviewer: `/root/reviewer_f` (re-review passed; see evidence below).

Commit `2df4a63` copies the entry count only when the completion result is `ISFS_OK` (zero), matching C `__isfsReadDirCB`. The callback still receives the original result. The fix note records target compilation limits; the tests could not run for the custom PowerPC target. No hardware verification was performed.

## Coverage map

### `ogc-filesystem`

Rust has one module: `crates/services/ogc-filesystem/src/lib.rs` (1,456 lines). The C API is declared in `C-sources/libogc/gc/ogc/isfs.h:30-64` and implemented in `C-sources/libogc/libogc/isfs.c`.

| Rust surface | Rust location | C surface checked |
| --- | --- | --- |
| ISFS constants; `Fstats`, `IsfsStats` | `lib.rs:32-61` | `isfs.h:8-26`; `isfs.c:87-96` |
| `IsfsFile`: `from_raw`, `raw`, read/write/write-all/seek, async read/write/seek, close/async close, async file stats | `lib.rs:177-322` | `isfs.h:35-38,45-49,55-56`; `isfs.c:424-485,578-620` |
| Initialize, deinitialize, format and async format | `lib.rs:330-386` | `isfs.h:30-34`; `isfs.c:170-194,369-389` |
| Directory read and async directory read | `lib.rs:392-585` | `isfs.h:41-42`; `isfs.c:196-275` |
| Stats and async stats | `lib.rs:588-630` | `isfs.h:53-54`; `isfs.c:391-422` |
| Create directory/file, sync and async | `lib.rs:633-710` | `isfs.h:43-44,51-52`; `isfs.c:277-326,486-535` |
| File stats | `lib.rs:713-728` | `isfs.h:55-56`; `isfs.c:600-620` |
| Get/set attributes, sync and async | `lib.rs:731-884` | `isfs.h:57-62`; `isfs.c:622-779` |
| Get usage, sync and async | `lib.rs:887-991` | `isfs.h:63-64`; `isfs.c:781-839` |
| Delete/rename, sync and async | `lib.rs:994-1060` | `isfs.h:39-40,59-60`; `isfs.c:537-576,679-726` |
| Open and async open | `lib.rs:1063-1128` | `isfs.h:35-36`; `isfs.c:328-367` |
| ISO9660 mount, unmount, volume label wrappers | `lib.rs:1361-1369` | `C-sources/libogc/gc/iso9660.h:13-21`; implementation in `C-sources/libogc/libiso9660/iso9660.c` |

The mapping covers every operation declared in the ISFS header. Rust groups fd operations on `IsfsFile`, returns `Result` for open, and adds `write_all`. These are Rust API shape additions, not additional C ISFS operations. Rust also validates Rust slice lengths and documents async pointer lifetimes through unsafe contracts.

### `ogc-filesystem-sys`

Rust has one module: `crates/services/ogc-filesystem-sys/src/lib.rs` (34 lines). It exposes `Bool` (`lib.rs:6`), `ISO_MAXPATHLEN` (`lib.rs:8`), and `DiscInterface` (`lib.rs:11-32`). The C `DISC_INTERFACE` declaration is `C-sources/libogc/gc/ogc/disc_io.h:56-67`; the struct has the same field order: IO type, features, startup, insertion check, sector read, sector write, status clear, shutdown. The C ISO path constant is `C-sources/libogc/gc/iso9660.h:13` and has the same value, 128.

### `ogc-fat`

Rust has one module: `crates/services/ogc-fat/src/lib.rs` (3,152 lines). Its public surface is:

| Rust API | Rust location |
| --- | --- |
| `MountSource`, `MountOptions` (including public `mount_name` and `read_only` fields), `FatError` | `lib.rs:169-199` |
| `FatFile`: open/create/create-truncate, length/empty checks, read/write, seek, truncate, flush | `lib.rs:623-749` |
| `FatDirEntry` and its public `name`, `is_directory`, `size` fields; `FatMetadata` and its public fields; `stat`, create/remove file/directory | `lib.rs:751-791` |
| `FatDirectory`: open and `next_entry` | `lib.rs:793-932` |
| SD, USB, card A/B, and configurable mount APIs | `lib.rs:935-1097` |
| Unmount, media polling, global shutdown, named media-generation query, mounted-state query | `lib.rs:1099-1235` |

This is an inventory only. No behavior or parity claim is made for `ogc-fat`.

## Findings

### Confirmed mismatches

No missing ISFS operation is confirmed in the inspected public surface. The `ogc-filesystem-sys` declarations have no confirmed mismatch.

### Confirmed conditional difference: async `read_dir` count update

For synchronous `read_dir`, Rust and C both update `*num` only when the ioctl result equals zero (`crates/services/ogc-filesystem/src/lib.rs:412,448`; `C-sources/libogc/libogc/isfs.c:232-233`). The prior note incorrectly stated that Rust's helper accepts nonnegative results; current `is_isfs_success` returns true only for `ISFS_OK` (`lib.rs:1283-1285`).

For asynchronous `read_dir`, C `__isfsFunctionCB` invokes `__isfsReadDirCB` for any result `>= 0`, but `__isfsReadDirCB` writes `*num` only when `result == 0` (`C-sources/libogc/libogc/isfs.c:147-163`). Rust's `read_dir_async_callback` writes the count for any `result >= ISFS_OK` (`crates/services/ogc-filesystem/src/lib.rs:469-477`). Thus a positive nonzero completion changes `*num` in Rust but leaves it unchanged in C. The user callback receives the original result in both. Whether IOS returns such a result is not established by the checked sources. Independent evidence is in `libfilesystem-readdir-positive-result-audit.md`. The adjacent async GetStats/GetAttr/GetUsage callbacks were also checked and match C for copy-back and result handling; see `libfilesystem-query-copyback-audit.md`.

### Rust additions and checked implementation details

- The Rust API uses `CStr`, typed results, and bounded slices where C takes raw pointers and lengths.
- `IsfsFile::write_all` is an additional Rust convenience method.
- `ogc-ipc` stages unaligned DMA buffers in `FileDescriptor::ioctl` (`crates/core/ogc-ipc/src/lib.rs:444` onward). This supports the Rust sync stats buffer path; Rust also applies the C 32-byte alignment requirement for file stats (`ogc-filesystem/src/lib.rs:713-716`, matching C `isfs.c:600-604`). The inspected stats, attribute, and usage marshalling patterns match the corresponding C IOS ioctl/ioctlv shapes.
- `ogc-filesystem` delegates ISO9660 operations to the Rust `iso9660` dependency. The public mount function names and path-length constant match the C header, but this review did not perform a line-by-line algorithm comparison of the ISO9660 implementations.

## Explicitly unchecked

- Whether IOS returns a positive nonzero result for asynchronous `ISFS_ReadDir`.
- Whether IOS uses the return value from user completion callbacks in a way that makes callback return-value differences observable.
- Full algorithm-level equivalence between the Rust ISO9660 dependency and `C-sources/libogc/libiso9660/iso9660.c`.
- All `ogc-fat` behavior and parity. A source-availability audit found no authoritative libfat implementation in the C reference tree `C-sources/libogc`: it has no libfat directory or FAT implementation `.c` files. `C-sources/libogc/gc/sdcard/wiisd_io.h:34-43` only declares the Wii SD `DISC_INTERFACE`; `C-sources/libogc/libogc/usbstorage.c:103-106,808-811` only identifies libfat as a consumer of USB storage interfaces. The broader `C-sources` checkout has `C-sources/usbloadergx/source/libs/libfat/` with `fat.h`, `libfatversion.h`, `fatfile_frag.h`, and `libcustomfat.a`, but no implementation sources there; Nintendont includes FatFs (`fatfs/ff.c`), which is a different implementation. Headers, a prebuilt archive, and other applications' FAT implementations do not establish source-level parity with the `ogc-fat` implementation, so no behavior is inferred and `ogc-fat` remains unverified.
- Hardware behavior, media failure cases, and concurrency behavior.

## 2026-10-02 async read-dir result recheck

The uploaded Rust baseline already matches C's completion rule: the async callback copies the returned directory count into `*num` only when the IOS result is exactly `ISFS_OK` (`0`). Positive nonzero results no longer update the count.

## Independent re-review of async `read_dir` count-copy fix

**Result: PASSED** for commit `2df4a63` (`Fix async filesystem read-dir copyback parity`). Reviewer: `/root/reviewer_f`. Implementer: `/root/worker_b`. This approves the count-copy fix; it does not resolve the stated uncertainty about positive IOS results or callback-return handling.

- C `ISFS_ReadDirAsync` sets the input capacity, names output vector, and count output vector when `name_list` is non-null; without names it sends only path input and count output (`../../C-sources/libogc/libogc/isfs.c:237-275`). It passes the same original caller count pointer through `funcargv[0]` (`:252-255`). The async callback dispatches the read-dir copyback for nonnegative results, while `__isfsReadDirCB` writes the count only for exact result zero; the user callback receives the original result afterward (`:147-166`). C also passes the names buffer directly to IOS as a `cnt * 13` output vector (`:263-272`), so it does not stage or restore names on error.
- Rust builds the equivalent input/output vector counts and direct names buffer (`crates/services/ogc-filesystem/src/lib.rs:510-570`), passes its async callback through `FileDescriptor::ioctlv_async_raw` (`:573-582`), and retains the caller count pointer plus initial capacity in the context (`:454-467,535-545`). The fix helper now copies `context.count` only for `result == ISFS_OK` (`:469-487`), matching C for zero, positive nonzero, and negative completion results. Both implementations leave the caller count unchanged for positive and negative nonzero results. Both expose names as the IOS output vector, so errors may leave whatever IOS wrote into that buffer; neither implementation adds a wrapper copyback or rollback for names.
- For a null names buffer, C sends the uninitialized `no_entries` field as the count output (`:258-262,271-272`); Rust initializes its context count to zero and sends it as the count output (`:519-520,552-560`). The caller's `num` is only copied on exact-zero success in both paths. Thus the output count on success matches, while the temporary's pre-IOS value differs; the service is expected to write the output field, and the caller cannot observe that temporary value through the public API.
- On synchronous calls, C assigns `*num` only when `IOS_Ioctlv` returns zero (`:230-234`); Rust also assigns only on exact-zero result (`crates/services/ogc-filesystem/src/lib.rs:392-451`). The Rust helper `is_isfs_success` is exact-zero (`:1285-1286`).
- Both paths pass the original result to the user's async callback (`isfs.c:164`; Rust `lib.rs:473-476`). The C internal completion callback returns the IOS result (`isfs.c:147-151,166-167`), while Rust's internal callback returns `ISFS_OK` after freeing its context (`lib.rs:469-480`). The C sources inspected do not establish whether IOS observes this callback return value; this remains an explicit uncertainty, not a confirmed mismatch. The public C callback signature is `s32 (*)(s32,void *)` (`../../C-sources/libogc/gc/ogc/isfs.h:41-42`).
- The added regression test checks that a positive result preserves the caller's count and an exact-zero result copies it (`crates/services/ogc-filesystem/src/lib.rs:1409-1417`). It tests the helper, not the full asynchronous ioctl/callback lifecycle.

Validation: `cargo check -p ogc-filesystem --target powerpc-unknown-linux-gnu` passed. `cargo test -p ogc-filesystem --no-run --target powerpc-unknown-linux-gnu` compiled the test crate but failed at link because the configured `cc`/`ld` rejected cross-target options such as `--as-needed` and `-Bstatic`. No tests ran. No hardware verification was performed. The conditional question of whether IOS can return a positive nonzero result for this operation remains unresolved by the C implementation/header.

## Independent `ogc-fat` source-availability review

**Result: UNVERIFIED; crate remains open.** Reviewer: `reviewer_e`. I independently searched the required reference tree `../../C-sources/libogc`; no authoritative libfat/FAT filesystem implementation source is present there. I did not infer behavioral parity or mark `ogc-fat` complete.

### C tree search and available contracts

I searched filenames under `../../C-sources/libogc` for `libfat`, FAT filesystem sources, and FatFs names, then searched its C/header files for libfat/FAT mount, open, read, stat, and directory APIs. There is no libfat directory, FAT filesystem `.c` implementation, or corresponding public libfat header in this tree. The Rust implementation in `crates/services/ogc-fat/src/lib.rs` therefore has no available C filesystem algorithm to compare for boot-sector parsing, allocation, FAT-chain traversal, directory entries, file operations, or mount lifecycle.

The C tree does contain the lower-level block-device contracts which libfat consumes:

- `../../C-sources/libogc/gc/ogc/disc_io.h:47–67` defines `DISC_INTERFACE`, with `uint32_t sec_t`, boolean startup/insertion/read/write/status/shutdown callbacks, and the callback field order.
- `../../C-sources/libogc/gc/sdcard/wiisd_io.h:34–43` declares `__io_wiisd`; its implementation is the Wii SD transport in `../../C-sources/libogc/libogc/wiisd.c:666–675`.
- `../../C-sources/libogc/gc/sdcard/gcsd.h:42–44` declares the GameCube SD transports; `../../C-sources/libogc/libogc/gcsd.c:198–225` initializes their `DISC_INTERFACE` values.
- `../../C-sources/libogc/gc/ogc/usbstorage.h:65–86` declares USB storage operations and `__io_usbstorage`; `../../C-sources/libogc/libogc/usbstorage.c:103–106,808–811,1000–1009` says it implements a `DISC_INTERFACE` used by libfat and defines that transport.

These sources define or implement transport operations, not FAT volume/filesystem behavior. They cannot serve as a reference implementation for `ogc-fat`. The wider `../../C-sources` tree contains a prebuilt `libcustomfat.a` and headers under USB Loader GX/WiiFlow, but no corresponding libfat `.c` source there; Nintendont's `fatfs/ff.c` is a separate FatFs implementation and was not treated as the Rust crate's authoritative reference.

### Rust scope checked and limits

The current Rust module implements its own mounted FAT volume (`crates/services/ogc-fat/src/lib.rs:154–170`), BPB parsing (`:1265–1413`), file and directory operations (`:575–932`), and mounting over Wii SD, USB storage, and GameCube SD interfaces (`:933–1097`). It consumes the shared `DiscInterface` (`:18`); its `repr(C)` declaration in `crates/services/ogc-filesystem-sys/src/lib.rs:10–34` has the same field order and, on 32-bit PowerPC, corresponding 32-bit `unsigned long`/`uint32_t` fields and matching callback parameter/pointer shapes as C `DISC_INTERFACE`. This checks only the transport interface shape. No C FAT API declaration or implementation exists to compare with Rust's public `MountSource`, `MountOptions`, `FatFile`, `FatDirectory`, `stat`, or mutation APIs (`ogc-fat/src/lib.rs:124–152, 579–791, 933–1189`); their behavioral compatibility remains unknown.

Validation: `cargo check -p ogc-fat --target powerpc-unknown-linux-gnu --tests` passed, which compiled the target crate in test mode. The Rust source contains no `#[test]` functions, so no `ogc-fat` regression tests were present or executed. This compile does not establish filesystem correctness or C parity. No hardware/media verification was performed. The existing source-availability result and open state are preserved; no ledger or implementation changes were made.

## Independent `ogc-fat` and `DiscInterface` audit (2026-10-03)

**Result: UNVERIFIED; `ogc-fat` remains open.** Reviewer: `/root/reviewer_d`. This independently confirms the source-availability boundary; no implementer or parity fix applies to this bounded audit. I did not treat another FAT implementation or a binary archive as the C reference.

I searched all 435 files in `../../C-sources/libogc` for FAT/libfat/FatFs source names and searched its C/header files for FAT mount, open, read, write, directory, and stat APIs. The tree contains no libfat directory, FAT filesystem `.c` implementation, or corresponding public FAT filesystem header. It does contain the block-device interface `DISC_INTERFACE` (`gc/ogc/disc_io.h:47-67`), and transports that implement it: Wii SD (`libogc/wiisd.c:666-675`, declaration `gc/sdcard/wiisd_io.h:34-43`), GameCube SD (`libogc/gcsd.c:198-227`, declarations `gc/sdcard/gcsd.h:42-44`), USB storage (`libogc/usbstorage.c:1000-1009`, declaration `gc/ogc/usbstorage.h:85`; comments at `:103-106,808-811` identify this as an interface consumed by libfat), and Wii DVD (`libdi/di.c:977-985`, declaration `gc/di/di.h:91`). They supply sector I/O, not FAT volume parsing or file operations. The C tree also includes ISO9660 (`libiso9660/iso9660.c` and `gc/iso9660.h`), which is not a FAT implementation.

Rust `ogc-fat` implements BPB parsing, FAT mount state, file/directory operations, and mount lifecycle (`crates/services/ogc-fat/src/lib.rs:125-199,579-932,933-1235,1265-1413`). Its transport dependency is `ogc-filesystem-sys::DiscInterface` (`:18`). The Rust `#[repr(C)]` declaration has `u32` `io_type` and `features` followed by the six callbacks in the same order as C `unsigned long` fields and callback fields (`crates/services/ogc-filesystem-sys/src/lib.rs:10-34`; C `disc_io.h:56-67`). On 32-bit PowerPC, the integer fields and function pointers have the corresponding C widths. Callback argument and return shapes also correspond: zero-argument `bool` startup/insertion/status/shutdown, `bool` sector read with `(u32, u32, void*)`, and `bool` sector write with `(u32, u32, const void*)`. The transport declaration comparison passes; it does not establish parity for Rust's FAT algorithms or APIs. No C FAT API exists in the inspected reference tree to compare with `MountSource`, `MountOptions`, `FatFile`, `FatDirectory`, `stat`, or mutation methods.

Validation: `cargo check -p ogc-fat --target powerpc-unknown-linux-gnu --tests` passed and compiled the target test configuration. `ogc-fat` has no `#[test]` functions, so no tests ran. No hardware or media verification was performed. No implementation or ledger files were changed. The earlier independent source-availability review is confirmed, and the crate remains unverified rather than complete.

## Independent audit — ISFS close error paths

Reviewer: `/root/reviewer_d`. The async `read_dir` count fix remains correct. The earlier note's statement that Rust copies the count for any nonnegative synchronous result is stale: Rust's `is_isfs_success` requires `ISFS_OK` (zero) and guards copyback (`crates/services/ogc-filesystem/src/lib.rs:1285–1286, 448–450`), matching C's `ret == 0` check (`C-sources/libogc/libogc/isfs.c:230–234`). The async count fix was independently accepted by reviewer_f. C and Rust both pass the original result to the user callback and ignore its return, so callback-return values do not create an observable difference in the inspected IPC paths (`isfs.c:154–167`; Rust `lib.rs:469–480`; C IPC `ipc.c:385–388`; Rust IPC `ogc-ipc/src/lib.rs:861–873`). Rust frees its callback context on synchronous submit failure; C leaves the allocated context, so Rust avoids a C-side leak (`isfs.c:249–250, 274`; Rust `lib.rs:573–586`). Callback timing differs: C invokes in the IPC reply handler, while Rust queues an LWP deferred worker; this is an intentional scheduling choice noted in `libipc-core.md`.

Two current close-result differences remain. C `ISFS_Deinitialize` ignores `IOS_Close`, clears `_fs_fd`, and returns zero (`isfs.c:186–194`). Rust clears initialized state and descriptor, then returns the `FileDescriptor::close` error (`lib.rs:348–360`). Also, C `ISFS_Close` returns zero without IPC when `fd < 0` (`isfs.c:578–583`), but Rust's public `IsfsFile::from_raw` accepts any descriptor and `close` always submits IPC (`lib.rs:181–190, 266–268`). The async close has the same negative-descriptor difference (`isfs.c:585–598`; Rust `lib.rs:270–276`). The latter is limited to callers that create an `IsfsFile` from an invalid raw descriptor.

Other inspected behavior matched for sync read-directory count copyback, zero-only async count copyback, list buffer alignment/size checks, read-buffer validation, and maximum path length. `cargo check -p ogc-filesystem --tests --target powerpc-unknown-linux-gnu` passed and compiled target tests; tests did not run. `git diff 2df4a63^ 2df4a63 --check` passed. No hardware verification was performed.

### Independent confirmation

Reviewer: `/root/reviewer_f`. The two close-path findings above were independently checked against current C and Rust source and confirmed: `ISFS_Deinitialize` clears state but Rust returns the IOS close error while C ignores it and returns zero; Rust also closes a negative raw descriptor through IPC while C sync and async close return zero before IPC. Reviewer_f ran no checks; the PowerPC test compilation recorded above did not execute tests. No hardware verification was performed.

## BUG FIX handoff: skip IPC close for negative raw descriptors

Implementer: `/root/worker_a`. Independent reviewer: pending. Synchronous and asynchronous `IsfsFile` close now return `ISFS_OK` for a negative raw descriptor without calling IPC, matching C's early return in `ISFS_Close` and `ISFS_CloseAsync` (`C-sources/libogc/libogc/isfs.c:578-598`). Nonnegative descriptors retain their existing close paths and results. The separate `ISFS_Deinitialize` close-result finding remains open.

Added focused coverage for the shared close guard: negative descriptors return success without invoking the close operation, while nonnegative descriptors invoke it and preserve its result. `cargo check -p ogc-filesystem --tests --target powerpc-unknown-linux-gnu` passed and `git diff --check` passed. `cargo test -p ogc-filesystem --target powerpc-unknown-linux-gnu --no-run` reached linking but failed because the configured host linker rejected the PowerPC link options. Tests did not run. No hardware verification was performed.

### Independent re-review of negative descriptor close (`17aa0e5`)

**Result: PASS for this fix.** Reviewer: `/root/worker_b`. Implementer: `/root/worker_a`.

C `ISFS_Close` and `ISFS_CloseAsync` return zero before making an IOS call when `fd < 0` (`../../C-sources/libogc/libogc/isfs.c:578–598`). Rust now checks the raw descriptor before entering either the synchronous or asynchronous close operation and returns `ISFS_OK` for a negative descriptor (`crates/services/ogc-filesystem/src/lib.rs:42–50,272–291`). Nonnegative descriptors still use the existing IPC operation and preserve its result. The shared-helper regressions cover the negative skip and nonnegative call paths (`:1417–1434`). The reported mismatch is fixed. The separate `ISFS_Deinitialize` close-result mismatch remains open.

No checks or tests were run during this independent pass. The handoff records PowerPC test-target compilation passed; the test binary failed at linking with host linker errors, so tests did not run. No hardware verification was performed.

## 2026-10-05 `ogc-fat` reference-source requirement

`ogc-fat` remains `review_incomplete` for one specific reason: the available libogc tree does not contain the authoritative libfat filesystem implementation. To complete this review, obtain the matching **devkitPro/libfat source tree/version** used by this libogc/toolchain. The review needs the implementation of mount/partition parsing, FAT file and directory operations, metadata/stat/free-space behavior, devoptab dispatch, cache behavior, media lifecycle, and public error mapping. Existing `DISC_INTERFACE`, SD, and USB-storage sources cover only the block-device transport and are already available. A prebuilt archive or an unrelated FatFs implementation is not an acceptable source reference.


## 2026-10-05 final `ogc-filesystem` safety classification

The last recorded `ogc-filesystem` blocker was `ISFS_Deinitialize` close-result handling. C calls `IOS_Close`, clears `_fs_fd`, and returns zero even when the close operation fails. Rust clears its initialized state and descriptor but returns the real `FileDescriptor::close` failure. Under this repository's safety policy, preserving a cleanup failure is intentional error-preservation behavior and must not be weakened merely to reproduce C's unconditional success result. This is therefore an **accepted safety deviation**, not a non-safety parity blocker.

The previously confirmed negative raw-descriptor close mismatch is already fixed and independently re-reviewed. The recorded ISFS public-operation review found no other confirmed non-safety mismatch in the required scope. `ogc-filesystem` is therefore `reviewed`; no production change was made in this classification pass.


## 2026-10-05 libfat source parity pass — source identity not verified

The earlier review recorded a matching devkitPro/libfat source tree as supplied.
The current workspace does not contain that implementation or a pinned version.
The previous claim that this closed `review_incomplete` is therefore withdrawn.
The implementation notes below record historical Rust work and are not a
current parity result until the authoritative source is restored and identified.

Confirmed non-safety differences from that source are now explicit:

- **Fixed, needs independent re-review:** Rust now determines FAT12/FAT16/FAT32 from the cluster count like libfat, accepts the FAT12/FAT16 BPB layout, implements packed FAT12 and FAT16 FAT-entry reads/writes, ignores the undefined high cluster word on FAT12/FAT16 directory entries, and traverses the fixed root-directory sector span without treating cluster 0 as a data cluster. Fixed roots cannot be extended, matching libfat. Malformed fixed-root layouts whose directory-entry bytes do not form an integral 512-byte sector span are rejected explicitly as a safety/validation deviation instead of being rounded silently.
- libfat accepts logical sector sizes from 512 through 4096 bytes. Current Rust fixes filesystem sectors to 512 bytes. Variable-sector support remains a parity blocker.
- libfat exposes volume-label and DOS-attribute behavior (`fatGetVolumeLabel`, `FAT_getAttr`, `FAT_setAttr`) and its complete devoptab file/directory semantics. Those operations and edge cases still need a complete Rust-side comparison/port before the crate can be reviewed.
- C `fatUnmount` synchronizes open files while destroying the partition. Rust refuses unmount/shutdown while handles are live. That stricter lifetime rule prevents live Rust handles from becoming dangling references and is therefore retained as an accepted safety deviation.

This pass fixes two confirmed mount-path differences. C `fatMount` succeeds immediately when the requested devoptab name is already registered; Rust now treats an already-online Rust mount or any existing devoptab registration as successful instead of returning `AlreadyMounted`. C `FindFirstValidPartition` validates every non-empty primary partition and follows an extended-partition chain for up to eight logical partitions rather than trusting only FAT32 MBR type bytes. Rust now performs the same primary/extended search and validates candidate boot sectors through its filesystem parser. FAT12/FAT16 candidate boot sectors are now accepted when their BPB geometry and FAT capacity are valid.


## 2026-10-05 FAT12/FAT16 port

The prior implementation note used devkitPro/libfat `partition.c`, `file_allocation_table.c`, and `directory.c` as its reference, but the source revision is not present or pinned now. It records that Rust shares one BPB/cluster-count classifier for FAT12, FAT16, and FAT32. FAT12 uses packed 12-bit entries, FAT16 uses 16-bit entries, and FAT32 uses 28-bit entries and active-FAT handling. These implementation facts do not establish current C parity.

FAT12/FAT16 root directories now use the fixed root sector range between the FATs and data region, represented by the same directory-walk helper used for cluster-chain directories. The fixed root is never extended. Directory entry cluster decoding ignores the high 16-bit word outside FAT32, matching libfat. The existing create/read/write/rename/remove directory operations therefore reuse the same path logic for all three FAT formats.

The implementation deliberately fails on malformed FAT12/FAT16 BPBs whose fixed-root entry area is not an integral 512-byte sector span. This is stricter than libfat's integer division and is retained as a safety/error-detection deviation. Logical sectors larger than 512 bytes remain a separate confirmed parity blocker; this commit does not claim variable-sector support. Public volume-label, DOS-attribute, and remaining detailed libfat API/devoptab behavior also remain in the parity backlog.

## 2026-10-05 libfat completion pass — sector sizes, metadata, devoptab, and safety deviations

The previous note claimed coverage of logical sectors from 512 through 4096 bytes, but the source identity supporting that claim is not available in this workspace. Treat the recorded sector behavior as an implementation claim only, not verified parity.

The remaining public libfat surfaces are now represented by Rust-owned entry points and devoptab callbacks: `fatMount`, `fatMountSimple`, `fatUnmount`, `fatInit`, `fatInitDefault`, `fatGetVolumeLabel`, `FAT_getAttr`, and `FAT_setAttr`. Mounts can be created from caller-provided `DISC_INTERFACE` pointers and start sectors without taking ownership of transport startup/shutdown, matching libfat's public mount path. The default-init path mounts SD, USB, and GameCube SD interfaces and can publish the default device/cwd from the runtime argv descriptor. Volume-label lookup follows the root directory label entry first and then the BPB label fallback. DOS attribute get/set updates the short directory entry and rejects unsupported directory/volume operations with explicit errors.

The devoptab behavior has been expanded to cover libfat file and directory semantics: open/create/exclusive/truncate/append behavior, read-only protection, link as unsupported, chdir, unlink, rename, mkdir, rmdir, directory open/reset/next/close, stat/statvfs, ftruncate, fsync, fpathconf, and pathconf. New errno mappings are published through `ogc-sysbase` so callbacks fail loudly with the expected newlib-style errors instead of returning an unclassified `-1`. `statvfs` now reports clusters as POSIX blocks, sets the host `io_type` as the filesystem ID, and includes the read-only and no-suid flags like libfat.

Cache/media lifecycle parity is represented by explicit transport flushing, FAT32 FSInfo read/write support, media insertion checks, and stale-mount replacement. Rust still fails loudly when FSInfo/flush/sync operations fail instead of silently discarding those errors. The implementation also still refuses unmount while Rust file or directory handles are live, preventing dangling handles after partition teardown. Those behaviors are accepted safety/lifetime deviations. The previous malformed fixed-root rejection remains an accepted validation deviation.

The prior conclusion that no non-safety blocker remains is withdrawn. The crate
is `review_incomplete` until the exact libfat source revision is available. The
public devkitPro/libfat `master` page checked in a later bounded pass is not an
authoritative substitute for the project version.


## 2026-10-14 mounted std file increment

The FAT source has changed since the 2026-10-07 review. The new std adapter
uses typed FAT handles and exact flags. The shared sector engine preserves
partial byte counts and dirty state on flush failure. A newly retained start
cluster remains pending in metadata after a later operation failure. Allocation
rollback errors are returned; a failed FAT link update keeps the allocated link
target valid because a mirror may already contain the link. Cleanup failures
are recorded, and file metadata reflects the mounted read-only policy. Live
lengths are published by the open-file registry, so path metadata observes
writes and partial resize results before explicit flush.

See [file design](../wii-std-files.md) and
[file evidence](../wii-std-files-evidence.txt). A new independent source review
is required for these changes. The prior review does not establish their C
parity or hardware behavior. SD/USB failure recovery and interrupted-write
acceptance remain open. The std adapter has no raw devoptab registration
pointer and does not add a C link dependency.


## 2026-10-14 shared FAT flush and close review

`fat_close_r` no longer has a separate directory-entry update. Typed close,
typed Drop, and devoptab close use `close_file_state`, which calls
`flush_file_state` and releases the open-file reference after success or failure.
Release clears the magic value and mount pointer. Invalid or repeated close
returns `BadDescriptor` without transport access or a second release.

Flush checks the descriptor and online state first. It updates the stored size,
start cluster, and archive attribute only when metadata is dirty. Clean
read-only files do not rewrite their directory entry. The USB cache command
still runs for clean files; its error does not create dirty metadata. Metadata
or transport failure preserves existing dirty state for an explicit retry.

The public [devkitPro libfat source](https://github.com/devkitPro/libfat/blob/master/source/fatfile.c)
was checked at `_FAT_syncToDisc` and `_FAT_close_r`. C updates metadata for
modified writable files and releases the file after sync failure. Rust keeps
those rules. Rust also checks clean-file transport errors. The C reference
updates dates; this Rust change preserves the existing fields and does not
implement date updates. The public source is supplementary evidence and does
not replace a complete review against the project's supplied version.

Six new host regression tests use the same flush/close engine as production.
They check clean read-only metadata, online failure, USB disconnection, stored
bytes after reload, retry, and release after flush failure. The SD file probe
now checks read-only metadata across flush and close. On its owned file, it
sets read-only, clears archive, checks exact attribute preservation, rejects
writable open, and restores the original attributes with checked cleanup. The USB storage probe
uses a read-only reopen, and the hotplug probe checks stale fsync and close
results. Probes require Wii hardware; they have not run. Independent source
review and hardware acceptance remain open.

## Independent bounded FAT re-review — 2026-10-15

Reviewer: `/root/review_fat`. The exact matching libfat checkout was not found
in this workspace or the wider Wii project tree. The review compared several
behaviors with public devkitPro/libfat `master`, which is not the pinned project
reference. Its observations are therefore **unverified parity candidates**, not
confirmed libfat mismatches. They include partial-I/O errno, raw devoptab root
directory handling, stat timestamps and fields, automatic date updates, open
handle concurrency, and cleanup error propagation. Cleanup findings remain
repository-policy concerns even without a C comparison. The earlier claim that
matching source was supplied is stale. Do not mark FAT parity complete until
the correct C source version is restored. No tests or hardware acceptance were
claimed by the reviewer.

## Source availability check — 2026-10-15

The installed devkitPro package is `libfat-ogc 2.1.0-4`, described as
`libdvm`. Its package archive contains headers and static libraries, but no
implementation source. Its package recipe builds `libdvm 2.1.0` with
`fatfs-mod 0.15.3`; this is not the source tree used for the legacy libfat
parity review. The archived [devkitPro/libfat repository](https://github.com/devkitPro/libfat/releases)
has a latest tagged release of 1.2.0, but the project does not identify that
tag as its build input. The [devkitPro package recipe](https://github.com/devkitPro/pacman-packages/blob/master/libfat/PKGBUILD)
confirms the installed replacement source inputs. Keep this review incomplete
until the legacy libfat source revision used by the project is identified.

## Typed create API probe coverage — 2026-10-28

The registered `wii_std_fs_probe` now calls `FatFile::create_new`,
`FatFile::create_truncate`, and `FatFile::is_empty` directly. It checks the
initial empty state, exact write counts, persisted bytes after close and
reopen, exclusive-create failure without loss of the original bytes,
truncate-and-replace behavior, and cleanup of its uniquely named file. The
three API evidence records point to `typed_create_checks` in
`probes/lwp-probe/src/std_fs_probe.rs`.

`cargo xtask dolphin-validate --probe wii_std_fs_probe` passed 91 checks with
zero failures under Dolphin 2609a, including seven valid page captures. DOL
SHA-256: `dc450ccf23dc2ecd6e912f1be1e6fe7e32f3ff6c96da0db36addfd3d6019abd3`.
The full report is in
`/Volumes/SSD/WiiDev/Devtools/reports/evidence/libogc-rs-wii-std-fs-dolphin-fat-create-v2-2026-10-28/suite.json`.
This verifies those calls against Dolphin's isolated writable SD image. It does
not complete source parity against the absent project libfat revision or prove
real Wii media behavior.

## Typed FAT path operations — 2026-10-28

The same registered probe now exercises `create_directory`, `rename`,
`remove_file`, and `remove_directory` on a uniquely tagged directory. It checks
that rename preserves bytes after reopen, a failed rename leaves destination
bytes intact, removal of a non-empty directory fails without removing its
file, and the file and empty directory can then be removed. Cleanup is checked
and reported. The four exact inventory records point to
`typed_path_operations_checks` in `probes/lwp-probe/src/std_fs_probe.rs`.

`cargo xtask dolphin-validate --probe wii_std_fs_probe --artifact-dir
/Volumes/SSD/WiiDev/Devtools/reports/evidence/libogc-rs-wii-std-fs-dolphin-fat-path-ops-2026-10-28`
passed on Dolphin 2609a with 99 checks, zero failures, and seven valid page
captures. The probe summary passed. DOL SHA-256:
`5c1ceb883ed2d859b334136ad077d413ee28e149ceb723a6894215ee36c53e86`.
The suite report is in the artifact directory. This is evidence from Dolphin's
isolated writable SD image; physical Wii and USB media behavior remain open.

The same 99-check Dolphin run includes the existing owned-file attribute
probe: `get_attributes` and `set_attributes` preserve DOS attributes while
changing the read-only bit, verify read-only metadata, reject a writable open,
and restore the original attributes with checked cleanup. Exact inventory
evidence now points to `readonly_attribute_checks`. This records probe coverage;
it does not establish parity with the unavailable project libfat source.
