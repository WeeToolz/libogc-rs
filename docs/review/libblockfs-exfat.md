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

## 2026-11-27: MBR extent selection and both SD routes

BlockFS is a Rust-specific routing layer, not an API in the C reference.
The original extent scanner used one aligned sector buffer for the MBR and
candidate boot records. A read of a non-exFAT type-07 partition overwrote
unvisited MBR entries. As a result the scanner could fail to find a later
exFAT partition, or interpret bytes in the first candidate as another MBR
entry and read an unrelated sector.

The production scanner now preserves the 64 bytes of partition entries
before probing candidates. It uses one aligned sector buffer for I/O and
retains the previous error mapping, geometry limits and FAT fallback.
The separate extent module permits host testing of the exact production
algorithm, not a duplicate implementation. The exFAT probe independently
recognizes a later partition in its test image. This is a correction to an
added Rust feature, not a deviation from the C libogc interface.

Evidence: [blockfs-mbr-20261127](evidence/blockfs-mbr-20261127/summary.json).

| Test | Original | Corrected |
| --- | --- | --- |
| Host extent regression | 2 passed, 2 failed | 4 passed, 0 failed |
| Dolphin bare PowerPC scanner | 3 passed, 2 failed | 5 passed, 0 failed |
| Host exFAT tests | — | 23 passed |
| xtask host tests | — | 169 passed |
| PowerPC storage crate compilation | — | passed |
| Mixed-MBR exFAT SD runtime | — | 62 passed, 0 failed |
| FAT SD runtime with BlockFS routing | — | 53 passed, 0 failed |

The full probes use stock Linux Dolphin 2609b and a private imported
BootMii NAND containing the System Menu and IOS 58. The original backup
was not changed. No NAND files, keys, or raw emulator profile logs are
included in the evidence. All five captured pages for each probe were
valid. The mixed-MBR source image stayed unchanged; the test runner
wrote only to its private copy. The synthetic fixture has a non-exFAT
partition at LBA 32 and an exFAT volume at LBA 2048, and exercises the
complete SD driver, BlockFS selection, exFAT read/write and Newlib paths.
The compressed up-case table is an ASCII-focused diagnostic table, not a
replacement for a production formatter. The FAT run checks selection
from unmounted media, repeat mount generation, file I/O, and unmount.

Reproduce the mixed-MBR image using:

```sh
python3 scripts/create-exfat-mixed-mbr-fixture.py /path/to/new-test.raw
cargo xtask dolphin-validate --probe exfat_sd_probe \
  --sd-image /path/to/new-test.raw --allow-sd-image-writes
cargo xtask dolphin-validate --probe fat_sd_probe
```

Set `DOLPHIN_WII_NAND` to an imported private Wii NAND and `DOLPHIN_BIN`
to the Dolphin executable. The actual Linux probes in this review used
nightly `28221559263a3976766cf305940e80e30cf9ba8a` and a direct offline
`-Z build-std=core,alloc -Z build-std-features=compiler-builtins-mem`
release build with `--features ogc-system-probe/dolphin-automation` and
the repository's `targets/powerpc-nintendo-none.json` and linker script.
The restored compiler is older than the pinned Wii std source lock and
`rustup` is unavailable in this environment. The standard runner was
used with `--no-build` after `xtask elf-to-dol` converted the actual
probe ELFs. The repository source lock was not changed. Bare scanner
reproduction uses `scripts/blockfs-extent-diagnostic.py`. The negative
control uses the same extracted extent module and diagnostic, with only
its MBR-entry snapshot and loop replaced by the original indexed loop
from `b4b2b9ca:crates/services/ogc-blockfs/src/lib.rs`. The before and
corrected module/DOL digests and exact pass masks are in the saved
PowerPC reports.

Both routing paths now have direct runtime evidence. Move BlockFS to
`needs_re_review`; independent review of the correction and broader
media/error boundaries remains open. The exact libfat source identity
and its detailed comparison remain separately open under `ogc-fat`.
Physical Wii SD evidence remains pending and is not a completion gate.
