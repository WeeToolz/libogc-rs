# USB FAT EIO investigation — 2026-10-28

## Question

The combined Wii std run recorded four failed USB FAT checks. Determine which
operation first failed, whether the failure still applies to the current code,
and where the error enters the production path.

## Evidence found

The retained structured report is
[`hardware-evidence/wii-std-readiness-rvl001-20261015.json`](../hardware-evidence/wii-std-readiness-rvl001-20261015.json).
It records run `1792070611-686276000`, DOL SHA-256
`9638fbde1d3e9efb7a18786fd5b0f61c4ea1bbb731024f681e621b9c336d67db`, Wii IP
`10.0.0.242`, and an overall result of 405 passed and 8 failed. Four failures
belong to networking. The four USB FAT aggregate failures are:

- `directory checks complete`
- `path service checks complete`
- `probe obtains a new file`
- `owned medium unmount succeeds`

The checklist records 13 USB FAT checks passed and 4 failed. It identifies the
console as RVL-001 (USA), IOS58 revision 6176, and Hollywood revision `0x11`;
the printed motherboard revision was not recorded. The JSON has no per-check
error details. The raw `probe.log`, `report.wir`, original DOL, and DOL build
manifest were not found in the retained run directories. The source revision
and dirty-file state used to build this DOL are therefore unknown.

The run ID timestamp is 2026-10-15 09:23:31 EDT. The FAT sector-cache and
USB transfer-diagnostic commits `90a256a` and `c46e7b6` were recorded at
12:11 EDT on 2026-10-15. The aligned FAT sector-buffer change `ac10c1c` was
recorded on 2026-10-24. The old report does not prove which uncommitted source
was used, but it predates these commits by time and cannot serve as evidence
for the current source. Do not treat it as a current reproduction.

## Operation order in the retained probe source

`probes/lwp-probe/src/std_fs_probe.rs::run_medium` mounts USB, then runs
directory checks, path-service checks, a root-level `create_new` file check,
and owned-medium unmount, in that order.

The directory checks first create
`usb:/wii-std-fs-probe-dir`. They then inspect and enumerate the directory,
create `entry.bin`, write and sync bytes, inspect entries, exercise copy and
error cases, and remove only paths created by the probe. The path-service
checks create `usb:/wii-std-path-probe` and a nested directory, then create,
write, sync, rename, and inspect files. The last file check creates
`usb:/wii-std-fs-probe.tmp` with `create_new`. The probe unmounts only a mount
that it started.

The report records only the failed aggregate checks. It does not identify the
first failing operation inside either group. Later group failures may follow
the first error, so they do not identify separate causes.

## Current error path

Rust `std::fs` calls use the production FAT file-system service. In
`crates/services/ogc-fat/src/lib.rs`, `FatMount::read_sector` and
`FatMount::write_sector` call the mounted `DiscInterface` callbacks. For USB,
those callbacks enter `ogc-usbstorage-sys` `drive_read_sectors` or
`drive_write_sectors`, then `transfer_disc_sectors`, `transfer_blocks`, and the
BOT cycle.

If a sector callback returns false while the medium check still succeeds,
`ogc-fat` returns `FatError::Io`; `fat_errno` maps that to Newlib `EIO`. USB
flush failure can also map to `FatError::Io` during file sync or unmount.
The later USB diagnostics record transfer-read, transfer-write, flush result,
and SCSI sense data. The old run report has none of those values. The retained
evidence cannot locate the failure in FAT metadata handling, USB block
transport, flush, or mount cleanup.

The C reference has the USB `DISC_INTERFACE` and block transport, but no FAT
implementation. It cannot provide a matching C FAT operation for parity.

## Current status and next safe test

No root cause or production fix is established. No current DOL reproduction
occurred. On 2026-10-28, the fresh composite capture showed HBC 1.1.3 with
“Network not initialized.” The signed-in Terminal readiness check timed out at
Wii `192.168.2.3:4299` with exit status 3. No DOL was sent. The attached USB
volume's identity, contents, backup, and permission for probe-created files are
also unconfirmed.

The next evidence run requires all of these conditions:

1. HBC shows its current network state and a fresh Wiiload readiness check
   passes from Terminal.
2. The exact report-enabled DOL is rebuilt from the current source and its
   hash, size, features, source revision, and dirty-file state are recorded.
3. The exact recovery implementation for that DOL has passed the required
   focused recovery validation.
4. The USB device and filesystem are identified. The user confirms that the
   probe's uniquely named directories and files are safe to create and remove,
   and protects any data that must remain.
5. Composite capture stays active. The run saves `result.json`, `probe.log`,
   `report.wir`, USB probe status, sense values, and the matching screen and
   manifest. The first failed operation and its returned error are recorded.

Until these conditions pass, do not upload the storage probe or claim a
hardware result. Host tests and a DOL build cannot establish physical USB FAT
behavior.
