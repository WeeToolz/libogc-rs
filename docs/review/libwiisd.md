# libwiisd Review

Date: 2026-10-01

Status: static review of Wii SD interface startup, status, sector transfers, and shutdown is complete. The review found and fixed one C parity bug. No hardware verification is recorded.

## Scope

- `crates/services/ogc-wiisd`
- C references: `C-sources/libogc/libogc/wiisd.c`, `C-sources/libogc/gc/sdcard/wiisd_io.h`, and `C-sources/libogc/gc/ogc/disc_io.h`

## Reviewed behavior

- SDIO request layout, IOCTL versus IOCTLV selection, response copy size, and command arguments.
- Card startup, initialization retries, RCA/SDHC state, bus width and clock setup.
- Sector addressing, aligned direct transfers, unaligned transfers through the eight-sector buffer, status, and shutdown.
- Exported DiscInterface device type, feature flags, and callback mapping.

## BUG FIX

Rust previously rejected every zero-sector read or write before card selection. C passes zero-sector requests through the selected-card path: an unaligned request performs no data loop and then uses the selection result, while an aligned request reaches the SDIO command. Rust now follows that path and lets the corresponding command flow determine the result. A regression test checks that zero sectors are accepted by the request precondition.

## Other BUG FIX parity checks

- C ignores the card-deselect result when it returns the sector transfer result. Rust now returns the command result, not a failure caused only by deselection.
- C's inserted-card test can treat a negative IOS error with bit 0 set as inserted. Rust now rejects negative status values before checking the inserted bit.
- C's deinitialize path returns success after its close attempt even if IOS close fails. Rust retains that return behavior.

## Validation boundary

`cargo xtask check-group storage` passed, and `git diff --check` passed. Full-file rustfmt reports pre-existing formatting differences in the crate-level `compile_error!` and the non-PowerPC callback stub; the changed code follows rustfmt output. PowerPC-only unit tests and Wii hardware behavior were not run.

## C safety fix — 2026-10-15

Reference commit: `ff53c4a7b7ba5587d82305e710ca65bce1092e1c`.
This supersedes the earlier no-open-gap status for the affected contract.

C `libogc/wiisd.c` now prevents `SDIOCMD_TYPE_BC` data from crossing a 4 KiB
boundary. IOS can hang on this command type when the buffer crosses that
boundary. C uses a 64-byte local function buffer with 64-byte alignment and a
compile-time check at command call sites.

Rust now gives its 64-byte switch-function buffer 64-byte alignment and labels
both commands as broadcast commands. `send_command` uses the production-shared
`sdio_rules` module to check the command range before constructing a data slice
or submitting IPC. It checks transfer-length multiplication, the buffer's
64-byte alignment, the one-page maximum, null non-empty buffers, address
addition, the 32-bit Wii address bound, and that the full range stays in one
page. Invalid ranges return `ogc_ipc::IPC_EINVAL`. Other command types retain
their existing range behavior.

The regression tests call that same validation code. They cover valid aligned
ranges, unaligned and oversized buffers, null buffers, page crossing,
32-bit address overflow, transfer-length multiplication overflow, empty
transfers, and non-broadcast commands. The compile-time assertion checks that
the production buffer alignment matches the validator's 64-byte requirement.

`cargo xtask check-group storage` passed. The five shared host tests pass.
The existing production storage probe calls Wii SD startup and reads sector 0
when a card is present; startup exercises the switch-function commands. This
probe has not run on real Wii hardware. Keep hardware acceptance open.

`cargo xtask build-storage-probe` passed. The Dolphin 2603-415 run timed out at
boot stage 3 in `compiler_builtins::u64_div_rem`, before the storage probe
wrote its log or reached SD startup. The run is recorded in
`target/dolphin-validation/1792083837-e882d364e0b9/storage_probe/result.json`.
This does not validate the SDIO range checks. Keep Wii hardware validation open.

## Independent re-review — broadcast range and alignment (2026-10-15)

Reviewer: `/root/review_sdio_parity`. Result: **PASS for source parity**.
C's two broadcast calls use a 64-byte buffer aligned to 64 bytes, and the C
comment documents the 4 KiB boundary hang risk
(`../../C-sources/libogc/libogc/wiisd.c:187-194,397,496-504`). Rust uses the
same buffer size and alignment, and validates the checked transfer length and
address before constructing a data slice or submitting IPC
(`crates/services/ogc-wiisd/src/lib.rs:109-117,153-155,282-289`; `sdio_rules.rs:19-35`).
The validator's stricter checks preserve the real switch-function operation
and return `IPC_EINVAL` for invalid ranges. Shared tests cover alignment, size,
null and wrapping addresses, overflow, empty transfers, and non-broadcast
commands (`sdio_rules.rs:45-89`).

The page-crossing examples also fail the alignment condition, but an accepted
64-byte-aligned range of at most 64 bytes cannot cross a 4 KiB page. Real-Wii
SD startup remains required because host checks and the Dolphin timeout do not
verify IOS behavior.
