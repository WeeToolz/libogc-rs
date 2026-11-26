# Block filesystem and exFAT parity

## `ogc-exfat`: reviewed as an added feature

The checked-in libogc source has no exFAT reader or matching exFAT public API.
`ogc-exfat` is a Rust-only feature. Its on-device boundary uses the same
`DISC_INTERFACE` contract that libogc declares in
`../../C-sources/libogc/gc/ogc/disc_io.h:31-66`. Rust's Wii SD interface
supplies the matching callback set and feature values in
`crates/services/ogc-wiisd/src/lib.rs:1099-1110`; the C implementation supplies
those callbacks in `../../C-sources/libogc/libogc/wiisd.c:548-674`.

The current production exFAT path reads sectors through that interface, uses
the Wii SD capacity when available, and bounds volume reads by validated boot
or partition geometry (`crates/services/ogc-blockfs/src/lib.rs:178-264`).
Read-only and writable volume operations use the same callbacks. The exFAT
reader has no direct C behavior to match, so its format behavior is recorded
as an added feature rather than as libogc parity.

The existing `exfat_sd_probe` now calls all public exFAT volume operations and
all public blockfs operations that apply to the exFAT mount. This includes
mount and unmount, stat, open modes, file and directory handles, reads, writes,
truncate, sync, error mapping, and Newlib access through the registered
Devoptab. Its writable fixture is a private copy managed by the existing xtask
Dolphin runner.

Verification on 2026-11-26:

- `cargo test -p ogc-exfat`: 23 passed, 0 failed.
- `cargo check -p ogc-blockfs -p ogc-exfat --tests --target powerpc-unknown-linux-gnu`: passed; test code compiled.
- Dolphin 2609a `exfat_sd_probe`: 62 checks passed, 0 failed. DOL SHA-256:
  `9f6dc1c3563ed2438bbba32b8b418d65acdf22fe272324973380936416da32fa`.
  Report: `target/dolphin-validation/1795714122-599d71c3c226/suite.json`.
- The fixture source hash stayed at
  `d4c9b2cb02be8c59700de3bb3abcf7b49202cc93e228e8c53cb2d8b82c1fb6f5`.
  Dolphin wrote only to its private copy. Physical SD behavior remains
  pending.

The first run had writes disabled and failed its nine write checks. The report
showed that the runner set `Dolphin.Core.WiiSDCardAllowWrites=false`; the
fixture source remained unchanged. The corrected run used the existing
`--allow-sd-image-writes` option. Its private copy allowed the write checks to
pass. The first report is retained at
`target/dolphin-validation/1795713605-599d71c3c226/suite.json`.

## `ogc-blockfs`: source review remains incomplete

`ogc-blockfs` is a Rust routing and Newlib integration layer. For exFAT it
selects the added Rust feature above. For FAT it delegates to `ogc-fat` through
`ogc_fat::mount_sd` and `ogc_fat::unmount` (`crates/services/ogc-blockfs/src/lib.rs:267-315,355-362`).
The C tree contains the Wii `DISC_INTERFACE` and its device driver, but it does
not contain the exact libfat source used by `ogc-fat`. The `ogc-fat` ledger
entry already records this source limitation. Therefore this review does not
claim parity for the FAT mount or Newlib registry internals.

The 62-check Dolphin run exercises the exFAT route in `ogc-blockfs`, including
its public file and directory operations and Newlib registration. It does not
cover the FAT route. Compare that route after restoring and pinning the libfat
revision used by this workspace. Physical Wii checks remain separate.
