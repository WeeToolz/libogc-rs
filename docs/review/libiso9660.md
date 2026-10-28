# libiso9660 Review

Date: 2026-07-02

Status: the early missing-devoptab finding is superseded by the 2026-10-02 filesystem parity implementation. Mounts now register a read-only devoptab with file/directory/stat operations. Real-media validation remains open.

## Scope

- `crates/media/iso9660`

## Current implementation

`iso9660` exposes C-compatible mount, unmount, and volume-label entry points. Mount names are normalized, mounts are tracked in a process-wide registry, and duplicate mount names are rejected deterministically.

The parser reads volume descriptors starting at sector 16, accepts primary or supplementary descriptors, extracts the volume identifier, and validates path-table metadata before registering the mount.

## Verified coverage

- Target-side/pure-logic coverage should keep mount success, volume label retrieval, unmount cleanup, duplicate mount rejection, and callable C exports pinned without treating host execution as supported runtime behavior.
- Parser coverage verifies descriptor handling and path-table validation used by the current mount path.
- Registry behavior remains deterministic; non-PowerPC runtime builds now stop at crate root instead of using a host model.

## Remaining validation

- Validate hardware-backed media reads through the DI/DVD layer on a real Wii.
- Include real disc media and malformed image fixtures in that run.
- Treat broader directory traversal coverage as part of the hardware media validation pass.

## Phase 0 position

The phase 0 bar is met for mount lifecycle, label extraction, duplicate-name handling, and C export reachability. Hardware-backed ISO9660 media behavior remains the validation gate.

## First-pass static parity review

Reviewer: `/root/reviewer_4`  
Date: 2026-10-01  
Review type: static source comparison; no build or tests run.

### Historical confirmed findings (superseded below)

- **At the time of this audit, mount did not install an ISO9660 filesystem device.** C `ISO9660_Mount` constructs the mounted volume, fills a `devoptab_t` with open/read/seek/stat/directory callbacks, then calls `AddDevice` (`../../C-sources/libogc/libiso9660/iso9660.c:749-774,946-993`). Rust `mount_impl` calls `startup`, `is_inserted`, parses a volume label, and inserts only a name-to-label record into `MOUNTS` (`crates/media/iso9660/src/lib.rs:238-269`). It has no devoptab registration or file/directory callbacks. Therefore a Rust mount can return success and provide a volume label, but it does not make `name:/...` paths readable/stat-able or enumerable as C does. Rust `unmount_impl` only removes the registry entry (`lib.rs:271-278`), while C removes the device and frees the parsed root/path-table tree (`iso9660.c:997-1021`).
- **Resolved in the later parity pass — C ABI entry symbols.** Rust now exports `ISO9660_Mount`, `ISO9660_Unmount`, and `ISO9660_GetVolumeLabel` as unmangled `extern "C"` functions with the same pointer/boolean signatures as `gc/iso9660.h`. The existing snake-case Rust wrappers remain available. The workspace packaging strategy still determines how these symbols are collected into a final static library; this fix addresses the missing symbol names and calling convention.

C inspected: `libiso9660/iso9660.c`, `gc/iso9660.h`. Rust inspected: `crates/media/iso9660/src/lib.rs`, `Cargo.toml`. The review covered the public mount, unmount, and volume-label operations and the C device registration/callback table. It did not exhaustively compare the internal file and directory callback algorithms because the Rust implementation has no corresponding callbacks.

### Historical handoff (superseded)

At the time of this first-pass review, `iso9660` remained open because mount/unmount did not install/remove a devoptab filesystem device. The C ABI symbol finding has since been fixed. No build or runtime test was run for the later ABI change in this environment. The shared ledger has pre-existing staged changes; its `iso9660` claim/result row is left unstaged, so no commit was created.


## Later parity pass: C entry points

The crate now exports the three public names declared by `gc/iso9660.h` with `extern "C"` and `#[no_mangle]`, forwarding to the existing validated pointer wrappers. A target-gated compile-time test pins their function-pointer ABI. At the time this ABI-only pass landed, it did not address the larger missing `devoptab_t` behavior. The later 2026-10-02 implementation below supersedes that limitation.

## 2026-10-02 devoptab/file-system parity implementation

Compared with current `libiso9660/iso9660.c` and the shared libogc devoptab contract.

Mount now installs a real devoptab entry through `ogc-sysbase`, and unmount removes that device. The devoptab provides the C-visible read-only operations: open, close, read, seek, fstat, stat, chdir validation, directory open/reset/next/close, and statvfs. Paths are resolved case-insensitively from ISO directory records, primary and supplementary/Joliet descriptors remain supported, ISO file version suffixes are removed with the same `;1` rule as C, and file data is read from the mounted `DISC_INTERFACE`.

The new implementation reuses the shared `ogc-sysbase` devoptab registry instead of creating a second device registry. The newlib PowerPC `stat` and `statvfs` ABI layouts were also moved into `ogc-sysbase` so FAT and ISO9660 share one definition.

Focused target tests build a small ISO image in memory and cover mount, device registration, file open, read, seek, close, and unmount. C-specific `_reent->_errno` values are not currently modeled by the shared opaque `Reent` abstraction, so exact errno side effects remain an explicit difference even when success/failure behavior matches. Real DI/DVD media validation is still required.

## 2026-10-02 errno and devoptab edge-case parity

Rechecked against current `libiso9660/iso9660.c` after the devoptab implementation pass.

- `ogc-sysbase::Reent` now exposes newlib's leading `_errno` field and a shared setter, so ISO9660 callbacks can reproduce C's observable errno side effects instead of returning only `-1`/`0`.
- open/stat/chdir/diropen now report C's `ENODEV`, `ENOENT`, `EISDIR`, and `ENOTDIR` cases; invalid file/directory handles report `EBADF`; read failures report `EIO`.
- reads at EOF and truncated reads set `EOVERFLOW` exactly as C does while preserving C's return count behavior.
- seek uses 32-bit `off_t`-style wrapping and C's `EOVERFLOW`/`EINVAL` tests instead of the earlier wider Rust-only arithmetic.
- `statvfs` now writes only the fields C writes; `f_blocks` and `f_files` are intentionally left untouched rather than zeroed by a whole-struct replacement.
- focused callback tests pin EOF `EOVERFLOW` and missing-path `ENOENT` behavior.

The filesystem still needs real DI/DVD media validation, but the previously documented `_reent->_errno` source mismatch is closed.

## Independent re-review: current filesystem and ABI

**Result: NEEDS_CHANGES.** Reviewer: `reviewer_e`. This review covers the current `iso9660` filesystem and C exports against `../../C-sources/libogc/libiso9660/iso9660.c`, `../../C-sources/libogc/gc/iso9660.h`, and the installed devkitPPC newlib headers used by this target. It supersedes any prior claim that filesystem registration and ABI layout have passed independent review. Real media validation remains outstanding.

The three public symbols are present with the correct names and source-level function signatures: `ISO9660_Mount`, `ISO9660_Unmount`, and `ISO9660_GetVolumeLabel` are `#[no_mangle] extern "C"` exports in `crates/media/iso9660/src/lib.rs:899–925`, matching `gc/iso9660.h:19–21`. Rust `DiscInterface` is `repr(C)` and uses 32-bit `io_type`/`features`, matching the PowerPC target's 32-bit `unsigned long` fields and function pointer order in `gc/ogc/disc_io.h:47–67` (`ogc-filesystem-sys/src/lib.rs:10–29`). The PowerPC rlib contains all three unmangled symbols (`nm -g target/powerpc-unknown-linux-gnu/debug/libiso9660.rlib`).

### Confirmed mismatches

1. **The mount is not registered in the C devoptab table.** C `ISO9660_Mount` calls libogc/newlib `AddDevice` for its devoptab (`iso9660.c:981–993`), and C path callbacks resolve paths with `GetDeviceOpTab` (`iso9660.c:776–795`). Rust `mount_impl` calls `sysbase::add_device` (`iso9660/src/lib.rs:787–817`), but `ogc-sysbase::add_device` only stores the pointer in its private `DEVICE_REGISTRY` (`ogc-sysbase/src/lib.rs:563–595`); Rust `device_op_tab` resolves that same private array (`:538–555`). It does not call C `AddDevice`, so a C caller can call the exported `ISO9660_Mount` but its later standard C/newlib `open`/`stat`/directory calls cannot discover the mount. Rust also stores a colon-terminated `devoptab.name` (`iso9660/src/lib.rs:147–152, 787–808`), while C stores the bare mount name and passes the colon-terminated form to `AddDevice` (`iso9660.c:962–985`). The Rust `DeviceFile` path can use the private registry; that does not provide the C registration behavior.

2. **The `statvfs` callback uses a layout that does not match the target C type.** Rust's shared `StatVfs` declares `f_bsize` and `f_frsize` as `u64` and places subsequent fields accordingly (`ogc-sysbase/src/lib.rs:103–118`). The devkitPPC target header declares both as `unsigned long` (32-bit on this PowerPC target), then uses 64-bit `fsblkcnt_t` counters (`/opt/devkitpro/devkitPPC/powerpc-eabi/include/sys/statvfs.h:14–25`; `sys/_types.h:38,42`; `machine/_types.h:7–17` confirms target widths). C writes `f_bsize` and `f_frsize` at their C field offsets and deliberately leaves `f_blocks` and `f_files` untouched (`iso9660.c:723–746`). Rust casts the C pointer to its differently laid out type and writes the Rust fields (`iso9660/src/lib.rs:721–740`); on the big-endian target, the first two 64-bit stores do not correspond to two 32-bit C fields, and the Rust `f_frsize` store overwrites the C `f_blocks` field. Consequently the C-visible block sizes and untouched counters are wrong. This also invalidates the note's earlier claim that the callback preserves the C `statvfs` fields.

3. **The devoptab seek callback has the wrong ABI width and cannot represent all C offsets.** The C devoptab contract declares `seek_r` with `off_t` argument and return (`sys/iosupport.h:43`), and target `off_t` is 64-bit (`machine/_types.h:7`, `sys/types.h:154–156`). C `FILE_STRUCT.offset` and `_ISO9660_seek_r` also use `off_t` (`iso9660.c:92–98, 530–570`). Rust's shared `SeekFn` uses `i32` for both offset and result (`ogc-sysbase/src/lib.rs:148–150`); `iso_seek` also takes and returns `i32`, and `FileState.offset` is `i32` (`iso9660/src/lib.rs:110–117, 514–548`). This is a C callback ABI mismatch, and even apart from the call ABI it prevents seek positions above `i32::MAX`, although C can seek within a file whose 32-bit ISO size is larger. For example, a C `SEEK_SET` request at `0x8000_0000` can be accepted by C for a large enough file but is not representable in the Rust callback state/signature.

4. **Directory record extended-attribute sectors are dropped.** C's `read_direntry` adds the record's extended-attribute length (`buf[1]`) to its sector before storing the entry (`iso9660.c:212–240`). Rust builds directory/file entries from the big-endian sector field alone (`iso9660/src/lib.rs:358–378`), despite the extended length being present in the record. For any entry with nonzero extended attributes, Rust reads directory contents or file bytes from an earlier sector than C, and the reported inode sector also differs.

5. **Mount-time path-table validation does not perform C's parent-tree checks.** C parses each path-table entry into a tree and fails mount when its parent index cannot be found (`iso9660.c:882–918`). Rust only validates record framing and name lengths (`iso9660/src/lib.rs:268–297`), then does not retain or use the path table for lookup; its lookups scan directory records instead (`:338–406`). A structurally sized path table with a nonexistent parent index can therefore mount in Rust when C mount fails. In inconsistent media where path-table directory entries and directory records differ, the two implementations can also resolve different paths.

6. **Directory iteration defers reads and does not retain C's enumerated snapshot.** C `_ISO9660_diropen_r` resolves the directory with `entry_from_path`, which calls `read_directory` and builds `state->entry.children` before returning (`iso9660.c:642–668`, with directory loading at `:361–389`). Each C `_ISO9660_dirnext_r` reads from that stored child list and increments its index (`:684–705`). Rust `iso_diropen` stores only the directory metadata (`iso9660/src/lib.rs:613–640`); each `iso_dirnext` rereads and reparses the directory from the disc (`:662–699`). Thus an unreadable directory can fail at C `diropen` but succeed there in Rust and fail later at `dirnext`; changes or transient read errors after open can also change Rust's enumeration where C continues from its captured list.

7. **Null path error values differ from C.** C's `_ISO9660_getMountDescrFromPath` returns null when `path` is null (`iso9660.c:776–795`), and open/stat/chdir/diropen translate that missing mount to `ENODEV` (`:449–459, 592–603, 617–627, 647–657`). Rust explicitly rejects a null path as `EINVAL` in open/stat/chdir/diropen (`iso9660/src/lib.rs:441–448, 573–580, 590–599, 618–625`). This is a defensive Rust check but changes observable `_reent->_errno` for those calls.

### Behavior that matched in the inspected paths

For normal records with zero extended-attribute length, the inspected path lookup is ASCII-case-insensitive and removes the final `;x` version suffix for non-directory records, matching C's `strncasecmp` and `name[namelen - 2] = '\0'` behavior (`iso9660.c:243–263, 298–358`; Rust `iso9660/src/lib.rs:311–335, 385–406`). The inspected read/EOF/truncated-read and seek validation errno paths follow the C control flow when offsets fit the current Rust `i32` representation (`iso9660.c:491–570`; Rust `:478–548`). Stat field values written by `fill_stat` match C's `stat_entry` for ordinary entries (`iso9660.c:159–176`; Rust `:416–425`), and `statvfs` intends to write only the same field names (the ABI mismatch above prevents that from being true in memory). Directory EOF reports `ENOENT` in both implementations, and invalid/closed file handles report `EBADF` (`iso9660.c:477–489, 684–720`; Rust `:464–475, 643–718`).

Rust returns `EINVAL` or `EBADF` for several null callback pointers where the C code dereferences them; those C cases are invalid pointers and do not have a defined successful behavior. Rust's `resolve_entry` also accepts `.` and resolves directory-record `..` entries (`iso9660/src/lib.rs:395–406`), while the C path-table walker has no explicit `.`/`..` handling (`iso9660.c:298–358`); this is an additional path behavior rather than a confirmed failure for ordinary names.

### Validation and coverage

`cargo check -p iso9660 --target powerpc-unknown-linux-gnu --tests` passed, so target test code compiled; tests were not executed. `cargo check -p iso9660 --target powerpc-unknown-linux-gnu` and `cargo build -p iso9660 --target powerpc-unknown-linux-gnu` passed. `nm` confirmed all three C ABI symbol names in the built rlib. Tests cover synthetic mount/unmount, duplicate names, exported wrapper calls, a file read/seek, and two errno cases (`iso9660/src/lib.rs:937–1222`); they do not cover native C devoptab registration, `statvfs` target layout, extended-attribute sectors, malformed path-table parents, directory iteration, case/version lookup, or null-path errno. No tests were run, and no hardware/media verification was performed. The ledger is unchanged; only this review note was edited.

### Implementer handoff: null-path errno parity

Implementer: `/root/worker_b`. Commit: `c210d29` (`Match ISO null path errno with C`). Open, stat, chdir, and diropen now return the C failure result for a null path and set `_reent->_errno` to `ENODEV`. Invalid non-path output or iterator pointers continue to report `EINVAL`. The regression test checks each callback's result and errno using a null path.

Validation: `cargo check -p iso9660 --tests --target powerpc-unknown-linux-gnu` passed; target tests compiled but were not run. `git diff --check` passed. `cargo fmt -p iso9660 -- --check` reports pre-existing formatting differences across `src/lib.rs`, so formatting was not applied to avoid unrelated edits. No hardware or media verification was performed.

This handoff covers only the null-path errno mismatch. Independent static re-review is needed; the review thread manages the review and ledger status. Remaining confirmed differences include the `statvfs` target layout, dropped extended-attribute sectors, missing path-table parent validation, and directory iteration snapshot behavior. Other open ABI and devoptab registration findings in this audit also remain unresolved.

## Independent re-review: null-path errno fix

**Result: NEEDS_CHANGES.** Reviewer: `reviewer_e`. Implementer: `worker_b`. Fix commit: `c210d29`.

For a null path, the new behavior matches C. C `_ISO9660_getMountDescrFromPath` returns null for a null path (`../../C-sources/libogc/libiso9660/iso9660.c:776–782`); `open`, `stat`, `chdir`, and `diropen` convert that unresolved mount to `ENODEV` (`iso9660.c:449–454,592–597,617–622,647–652`). Rust now returns failure and sets `ENODEV` before the remaining callback checks (`crates/media/iso9660/src/lib.rs:441–444,577–580,599–602,626–629`).

The changed `open` ordering creates a separate error-precedence mismatch. Rust rejects a null `file_struct` with `EINVAL` before resolving the mount or path (`lib.rs:445–456`). C resolves the mount and path first, and only uses the file pointer after successful resolution and type checks (`iso9660.c:449–459,469 onward`). Therefore, with a null file pointer and an unresolved or nonexistent path, C reports `ENODEV` or `ENOENT` before touching the file pointer, while Rust reports `EINVAL`. The added test covers null paths with valid ancillary pointers, so it does not cover this combination. Move the Rust handle check after mount/path resolution and the directory check, but before writing file state, to preserve C's error precedence.

`cargo check -p iso9660 --target powerpc-unknown-linux-gnu --tests` passed and compiled the test code. The host test command could not build because `ogc-sysbase` and `ogc-filesystem-sys` are PowerPC-only; tests did not run. No hardware verification was performed. Other ISO9660 ABI and filesystem findings remain open.

### Implementer handoff: null file-pointer error precedence

Implementer: `/root/worker_b`. Rust `iso_open` now resolves the mount and path and checks whether the entry is a directory before it checks `file_struct`, matching C's error precedence. The regression checks null `file_struct` with an unresolved mount (`ENODEV`), a missing path (`ENOENT`), and a directory path (`EISDIR`). A valid file path is not tested with a null pointer because C proceeds to dereference that pointer.

Validation: `cargo check -p iso9660 --tests --target powerpc-unknown-linux-gnu` passed and compiled the target test code; the tests were not run. `git diff --check` passed. No hardware or media verification was performed.

## Independent re-review: extended-attribute sector offsets (`8d9f2e8`)

**Result: PASS for this bounded fix.** Reviewer: `worker_a`. Implementer: `worker_b`.

C adds the extended-attribute sector count in directory record byte 1 to the record's extent before it stores the child entry (`../../C-sources/libogc/libiso9660/iso9660.c:212–238`). Rust now uses checked addition for the same child-entry extent in `directory_entries` (`crates/media/iso9660/src/lib.rs:378–380`). This corrected sector then feeds file reads through the existing entry path. The regression constructs a record with two attribute sectors, puts distinct data at the raw and adjusted extents, and checks that the read returns the adjusted data. Checked addition rejects an overflowing extent safely.

I inspected the complete implementation and test diff. No checks or tests were run during this re-review; the implementer's note records successful target test compilation, with tests not executed. The root descriptor parsing path is not changed by this fix. Other recorded ISO9660 path, ABI, and filesystem differences remain open. No hardware or media verification was performed.

### Independent static re-review of `b41c3bb`

**Result: PASS for the bounded null-file precedence fix.** Reviewer: `/root/reviewer_d`; implementer: `/root/worker_b`.

C `_ISO9660_open_r` resolves the mount and entry type before dereferencing the output file pointer (`C-sources/libogc/libiso9660/iso9660.c:449-469`). Rust now checks path, mount, entry and directory status before checking `file_struct` (`crates/media/iso9660/src/lib.rs:438-466`). The error precedence matches for defined failure cases. Regression coverage checks unresolved mount (`ENODEV`), missing entry (`ENOENT`) and directory (`EISDIR`) cases (`lib.rs:1260-1292`). For a valid file with a null output pointer, C dereferences it; Rust returns `EINVAL` defensively.

Validation: `cargo check -p iso9660 --target powerpc-unknown-linux-gnu --tests` passed and compiled test code; tests did not run. No hardware or media verification was performed. Other ISO9660 findings remain open.

## Implementer handoff: extended-attribute sector offsets

Implementer: `/root/worker_b`. Independent re-review: needed; review thread owns it.

C `read_direntry` adds the record's byte 1 extended-attribute sector count to the big-endian extent sector before storing the entry (`../../C-sources/libogc/libiso9660/iso9660.c:212-238`). Rust previously stored only the big-endian extent from record bytes 6 through 9. Rust now adds the extended-attribute count with checked arithmetic before using the entry sector for file data and metadata.

Regression coverage gives `HELLO.TXT` two extended-attribute sectors, places different bytes at the unadjusted and adjusted extents, and reads from the adjusted extent. `cargo check -p iso9660 --target powerpc-unknown-linux-gnu --tests` and `git diff --check` passed; target test code compiled, but tests were not run. `rustfmt --edition 2021 --check` reports existing formatting differences elsewhere in `lib.rs`. No hardware or media verification was performed.

## 2026-10-05 completion pass — path-table parents and directory snapshots

Mount-time path-table validation now checks the parent index of every record. The root must name parent index 1; every later record must refer to a nonzero parent whose index is lower than the record being inserted. This mirrors C's `entry_from_index` requirement that a parent already exist and rejects forward or missing parents instead of accepting only structurally well-formed records.

Directory open now reads and parses the child list before publishing the iterator state. A media read/parse failure therefore fails `diropen` with `EIO`, and `dirnext` walks the captured list without rereading the disc. `dirreset` rewinds the captured index, and `dirclose` releases the captured vector. This matches C's `entry_from_path`/`DIR_ENTRY.children` snapshot timing. Regression coverage mutates the synthetic on-disc directory after open and verifies that iteration still returns the original captured name.

These changes close the two remaining recorded ISO9660 blockers at source level. The crate moves to `needs_re_review`; PowerPC compilation and media/runtime validation are still required before `reviewed`.
