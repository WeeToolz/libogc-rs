# TinySMB static review

- Reviewer: `/root/reviewer_5`
- Implementer: none; first-pass review
- C implementation: `../../C-sources/libogc/libtinysmb/smb_devoptab.c`, `../../C-sources/libogc/libtinysmb/smb.c`, public declarations in `../../C-sources/libogc/gc/smb.h`
- Rust implementation: `crates/compatibility/tinysmb-sys/src/lib.rs` and `crates/compatibility/tinysmb-sys/src/powerpc_backend.rs`
- Reviewed history: `8865664` (UTF-16 SMB directory name decoding), `565302c` (response framing and transfer bounds)
- Validation: static source comparison only. No build or tests were run, as required for this review pass. No hardware verification was performed.

## Confirmed mismatches

### `smbInit` does not establish a persistent default device in the PowerPC backend

C `smbInit` delegates to `smbInitDevice("smb", ...)` (`smb_devoptab.c:1506-1509`). `smbInitDevice` connects, reserves one of ten `SMBEnv` slots, and calls `MountDevice` (`smb_devoptab.c:1487-1503`). `MountDevice` registers a devoptab with `AddDevice`, stores the connection and environment, and enables read-ahead (`smb_devoptab.c:1403-1452`).

Rust `smb_init` only connects and immediately calls `close_connection`, then returns `true` (`powerpc_backend.rs:1102-1116`). It does not retain a connection, register a device, or establish the default `smb` mount. This means a successful Rust initialization does not leave the persistent default device that C initialization provides.

### Device-name validation and duplicate handling differ

C rejects a null device name or a name longer than eight bytes, allows shorter names (including the default `smb`), and rejects an already registered device (`smb_devoptab.c:1455-1465`). Rust takes `CStr::to_bytes().get(..MAX_DEVICE_NAME - 1)` where `MAX_DEVICE_NAME` is 9 (`powerpc_backend.rs:18,1068-1099`). This fails for every name shorter than eight bytes, while names longer than eight bytes are silently truncated to eight bytes. Rust also picks the first free mount slot without checking whether another used mount has the same name (`powerpc_backend.rs:1087-1096`). The accepted names and duplicate behavior therefore differ from C.

### Close and connection-check name normalization differs

C `FindSMBEnv` removes one trailing colon before comparing a device name (`smb_devoptab.c:118-135`); `smbCheckConnection` also stops its input at the first colon (`smb_devoptab.c:1527-1542`). Rust `smb_close` and `smb_check_connection` compare the full `CStr` bytes directly against the stored mount name (`powerpc_backend.rs:1118-1146`). A caller passing `"smb:"` is accepted by C but does not match Rust's stored `"smb"` name.

### Outgoing UTF-8 to UTF-16 conversion is byte-wise

C `utf8_to_utf16` decodes a multibyte character with `mbrtowc` and writes one UTF-16 unit for the decoded character (`smb.c:265-303`). Rust `utf8_to_utf16le` writes every source byte as a separate low byte followed by zero (`powerpc_backend.rs:553-569`). The backend uses it for account/domain/tree names and file paths (for example `powerpc_backend.rs:799-836,906,1259,1596`). ASCII values match, but non-ASCII UTF-8 names become multiple unrelated UTF-16 units and do not encode as C does. Commit `8865664` corrected the opposite direction (UTF-16 to UTF-8) but did not correct this outgoing conversion; the new tests in `lib.rs:1067-1087` exercise only decoding.

## Observable difference that appears to correct a C bug

C `SMB_ReadFile` breaks when a response has zero data, but returns the requested `size` after the loop (`smb.c:1819-1877`). Rust returns the number of bytes actually received (`powerpc_backend.rs:1672-1749`). This is an observable return-value difference, but Rust reports the actual transfer count and appears to correct a C bug. It is recorded separately from the confirmed parity mismatches.

## Scope limits

The C `SMB_DiskInformation` API receives `struct statvfs *` (`gc/smb.h:144`) and the C implementation fills that structure (`smb.c:1731-1796`). Rust writes a local `HostStatVfs` representation (`powerpc_backend.rs:2093-2106`). The checkout does not contain the target `struct statvfs` definition, so the exact PowerPC field layout cannot be verified from the available C sources; no ABI mismatch is asserted here.

## Independent re-review: outgoing UTF-8 to UTF-16

**Result: NEEDS_CHANGES.** Reviewer: `reviewer_e`. Implementer: `worker_b`. Implementation commit reviewed: `1bd044a` (`Fix TinySMB UTF-16 outgoing conversion`). This result applies to the outgoing-conversion fix only; the other findings above remain open.

Compared C `C-sources/libogc/libtinysmb/smb.c:utf8_to_utf16` (lines 265–303) with Rust `crates/compatibility/tinysmb-sys/src/powerpc_backend.rs:decode_utf8_scalar` and `utf8_to_utf16le` (lines 561–603). On valid input, the conversion matches C for ASCII and BMP characters: C writes bytes `[tempWChar[3], tempWChar[2]]` (little-endian low 16 bits), and Rust writes `codepoint as u16` in little-endian order. For supplementary scalars, C's four-byte `wchar_t` is truncated to its low 16 bits and Rust performs the same cast. Both stop at an input NUL, write a two-byte NUL terminator, and use the `SMB_MAXPATH - 2` conversion limit: C's `count < len - 1` with callers passing `SMB_MAXPATH - 2` permits at most `SMB_MAXPATH - 2` data bytes before its separate terminator; Rust expresses the same bound as `max_data_bytes` and returns the terminator as part of the written count. C returns data bytes only, and its callers add two bytes; Rust callers use the returned count including the terminator, so the packet offsets agree.

Checked all C `utf8_to_utf16` call sites (`smb.c:677, 690, 703, 716, 784, 1380, 1487, 1550, 1615, 1682, 1702, 2011, 2111`) against Rust `utf8_to_utf16le_or_error!` callers in `powerpc_backend.rs` (`setup_andx`, `tree_andx`, `path_info`, `find_first`, `open_file`, `create_directory`, `delete_directory`, `delete_file`, and both `rename` names). Account, domain, tree, and file path strings all route through the corrected helper. For invalid or incomplete UTF-8, both converters write a zero unit at the failure position. Rust then returns an error to its callers instead of reproducing C's `size_t(-1)` offset wrap; this is a deliberate defensive correction to C's error path.

The added regression test does not compile: `powerpc_backend.rs:653` compares `&output` with an array, and `cargo check -p tinysmb-sys --target powerpc-unknown-linux-gnu --tests` reports `E0277` for that assertion. The command also reports type errors in older tests in `tinysmb-sys/src/lib.rs` (first at line 1246, introduced in commit `118db0c`), which are separate from `1bd044a`; the new converter test's own error is sufficient to prevent compiling the claimed conversion coverage. `cargo check -p tinysmb-sys --target powerpc-unknown-linux-gnu` passed for the library. Tests were not run. No hardware verification was performed.

## Implementer handoff: test build and persistent default mount

Implementer: `/root/worker_b`. Status: fixed; needs independent review. The UTF-8 conversion regression now compares matching byte types. The PowerPC compatibility tests now use `u8` buffers and cast them to the target `c_char` pointers at the ABI calls, matching the target's unsigned `c_char` type. This also repairs the earlier test compile errors that blocked the full TinySMB target test build.

Rust `smb_init` now delegates to `smb_init_device` with the C default name `smb`. On successful connection, `smb_init_device` retains the connection handle in a persistent `MOUNTS` slot instead of closing it immediately. The mount-record helper accepts names up to C's eight-byte limit, so it can store the short default name; it rejects names above that limit. The regression test checks the stored default name and connection handle without network or hardware calls.

Validation: `cargo check -p tinysmb-sys --target powerpc-unknown-linux-gnu --tests` passed; it compiled the target test code, but tests were not run. No hardware verification was performed. The backend retains connection and mount state, but it does not currently build and register C's full newlib `devoptab_t`; confirm the scope of that remaining integration gap in independent review. Duplicate-name checks and the other findings above remain open.

## Implementer handoff: device-name validation and duplicate handling

Implementer: `/root/worker_c`. Status: fixed; needs independent static review. Rust now accepts nonempty device names from one through eight bytes, rejects empty and overlong names without truncating them, and rejects names that already exist in a used TinySMB mount before it opens a new connection. Regression coverage checks one-byte and eight-byte names, empty and nine-byte names, and duplicate lookup behavior. This only checks TinySMB's mount table; confirm in review whether any additional devoptab registry interaction is needed for the current Rust mount implementation.

Validation: `cargo check -p tinysmb-sys --target powerpc-unknown-linux-gnu --tests` passed and compiled the target test code; tests were not run. `rustfmt --check --edition 2021 crates/compatibility/tinysmb-sys/src/powerpc_backend.rs` passed. Workspace `cargo fmt --check` is blocked by pre-existing formatting differences in unrelated files. No hardware verification was performed.

### Independent static re-review of `1f9eaba`

**Result: NEEDS CHANGES for full C duplicate-name parity.** Reviewer: `/root/reviewer_e`; implementer: `/root/worker_c`.

The commit's one-to-eight-byte name validation matches C's limit; rejecting empty names also avoids C's later `aux[l-1]` access. The new duplicate check rejects names already present in Rust's `MOUNTS` table before connecting (`crates/compatibility/tinysmb-sys/src/powerpc_backend.rs:1195-1200`). However, C appends `:` and calls `FindDevice`, which checks the global newlib device registry (`C-sources/libogc/libtinysmb/smb_devoptab.c:1455-1465`). Rust checks only its private mount table and does not register a corresponding newlib devoptab, so it can accept a name already used by another registered device. Other findings in this note, including the missing full devoptab registration, remain open.

Validation: `cargo check -p tinysmb-sys --target powerpc-unknown-linux-gnu --tests` and `rustfmt --check --edition 2021 crates/compatibility/tinysmb-sys/src/powerpc_backend.rs` passed. Tests compiled but did not run. No hardware verification was performed.

## Implementer handoff: newlib duplicate-name check

Implementer: `worker_b`. Status: fixed; needs independent static review. `smb_init_device` now formats a valid one-to-eight-byte mount name as the colon-terminated name expected by C `FindDevice` and checks the global newlib device registry before opening an SMB connection. This complements the existing private `MOUNTS` duplicate check. Regression coverage checks `smb:`, the eight-byte limit, and rejection of a nine-byte name.

The larger dispatch mismatch remains open: Rust still does not build and pass a TinySMB `devoptab_t` to C `AddDevice`, so C/newlib path APIs do not dispatch through this Rust backend. The Rust backend also continues to retain its private connection/mount state. This handoff closes only the global duplicate lookup behavior.

Validation: `cargo check -p tinysmb-sys --target powerpc-unknown-linux-gnu --tests` passed and compiled the target regression test; tests did not run. `git diff --check` passed. No hardware verification was performed.


## Independent re-review — newlib duplicate-name lookup

Reviewer: `worker_c`. Implementer: `worker_b` (commit `07674d3`). I inspected the complete name-formatting and `smb_init_device` diff. Rust builds the colon-terminated name within the C ten-byte buffer limit, calls `FindDevice` before opening a session, and rejects a name when the call returns a nonnegative device index. This matches `smbInitDevice` in `../../C-sources/libogc/libtinysmb/smb_devoptab.c:1458-1465`. Name bounds are checked before formatting. I found no mismatch in the duplicate registry check. The missing `AddDevice`/devoptab dispatch and other TinySMB findings remain open. I ran no checks for this static re-review; implementation validation is recorded in the handoff. No hardware verification was performed.

## 2026-10-05 Rust-owned device-registry lookup

Implementer: `/root`. `tinysmb-sys` no longer imports the external `FindDevice` symbol. Its duplicate-name check now calls the Rust-owned `ogc-sysbase::find_device` implementation, so TinySMB does not rely on a libogc/newlib C implementation at final link. This changes only the registry lookup backend; the separate TinySMB `AddDevice`/devoptab callback-dispatch parity blocker remains open. Independent target compilation and final-link validation are still required.

## Implementer handoff: Rust-owned devoptab dispatch and close-name parity

Implementer: OpenAI. Status: fixed; needs independent static and runtime review. The PowerPC backend now creates one stable Rust-owned `Devoptab` per mounted SMB environment and registers it through the shared `ogc-sysbase` device registry. The table supplies the file, directory, metadata, mutation, chdir, and statvfs callbacks that C `MountDevice` registers. Callback state is kept in newlib-allocated per-file/per-directory storage, while the mount slot remains the authoritative connection and current-directory state. `smbClose` removes the registered device before releasing the connection.

Name handling now follows the two C call sites: `smbClose` accepts the stored bare name and one trailing colon, while `smbCheckConnection` stops at the first colon so `smb`, `smb:`, and `smb:/path` resolve to the same environment. The callbacks also resolve the device prefix through the same normalization helper. No C implementation is linked or called.

The implementation uses Rust null/bounds checks instead of reproducing C undefined behavior for invalid pointers or overlong paths. The existing low-level SMB transport remains unchanged. Independent target compilation, devoptab dispatch tests, and Dolphin/hardware runtime validation are still required.

## 2026-10-28 test-review corrections

The PowerPC test module no longer assumes that the simulated host store exists
at `1.2.3.4`. Its target integration test calls the production SMB backend and
requires an explicitly configured, isolated writable SMB1 share. Set
`WII_SMB_TEST_USER`, `WII_SMB_TEST_PASSWORD`, `WII_SMB_TEST_SHARE`,
`WII_SMB_TEST_SERVER`, and `WII_SMB_TEST_DIRECTORY` in the target test
process. The directory must be a new, unique `libogc-test-*` name. A failed
creation stops the test before it writes or removes files. The test checks
real connection, directory and file operations, offset I/O, independent
handles, search, rename, metadata, disk information, reconnect, and checked
normal-path cleanup. Missing configuration is a test failure, not a skip.
A test failure after creation can leave the unique directory on the test share;
inspect that directory before manual cleanup.

The PowerPC Linux `cargo check --tests` checks compilation only. Running the
integration requires a Wii-capable test harness, the real network backend, and
the configured SMB1 test server; it was not run as part of this change. The
fixed NTLMv1 example is exercised separately on the host using the production
NTLM source. No host simulation is evidence of target SMB behavior.
