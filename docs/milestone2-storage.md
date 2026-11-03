# Milestone 2: Wii storage runtime

Milestone 2 replaces the remaining disabled or inline-completion storage facades with Wii-backed implementations. The code is target-only and must be validated on real Wii hardware before the milestone is hardware-accepted.

## API verification groups

The generated inventory has 604 storage API entries in 14 packages. Its
`unverified` count tracks missing implementation-source mappings. A zero in
that field does not mean that every API passed a test or a Dolphin run. This
source-mapping result does not prove runtime behavior on Dolphin or Wii hardware.
The inventory audit checks that each entry maps to an evidence source. It does
not check runtime results.
Most evidence rows currently check only a function or method name; they do not
resolve its crate or receiver type. Five `ogc-wiisd` rows record the
`WII_SD_INTERFACE` callback field and require the matching function-pointer
call in the named probe. Twenty `ogc-usbstorage-lifecycle` rows record the
exact free-function or method receiver used by their named host test. Forty
`ogc-filesystem` rows record exact API paths in the runtime probe. Sixty-four
`ogc-fat` rows record exact function or method call paths from Dolphin probes
and host tests. Thirty `ogc-usbstorage` rows record the wrapper path called by
a named probe. Sixty-five `ogc-usbstorage-sys` rows record exact wrapper, C
ABI, callback, or helper paths. Nine `iso9660` rows record exact C wrapper, C
export, and Rust call paths. Fifty-nine `ogc-di` rows record exact wrapper
paths from the no-disc probe. Twenty-seven `ogc-dvd` rows record exact facade
calls from the no-disc probe. Thirty-two `ogc-dvd-sys` rows record exact calls
through the sys module. In total, 351 of 604 storage entries record exact API
paths. These checks confirm source identity only. A path match does not prove
that an API assertion passed or that a device-dependent branch ran. Review
each mapping and runtime report before using it as a result.

| Package | API entries | Probe group |
| --- | ---: | --- |
| `iso9660` | 9 | DVD |
| `ogc-card` | 63 | CARD |
| `ogc-card-metadata` | 56 | CARD |
| `ogc-card-sys` | 90 | CARD |
| `ogc-di` | 59 | DVD |
| `ogc-dvd` | 27 | DVD |
| `ogc-dvd-sys` | 32 | DVD |
| `ogc-fat` | 64 | FAT |
| `ogc-filesystem` | 40 | FAT |
| `ogc-gcsd` | 44 | Storage |
| `ogc-usbstorage` | 30 | Storage |
| `ogc-usbstorage-lifecycle` | 20 | Storage |
| `ogc-usbstorage-sys` | 65 | Storage |
| `ogc-wiisd` | 5 | Storage |

The workspace checks 15 runtime crates as one build group with
`cargo xtask check-group storage`. The probe group names map to source files in
`config/probe-coverage.csv`. They collect related packages, but they do not
define a result for every API. Use smaller operation batches inside each probe
group: initialization and teardown, status, file or block I/O, directory and
metadata operations, asynchronous completion, error handling, and cleanup.
Each API needs a direct observable check or an exact host test of production
logic. A broad probe run does not prove every API in its group.

Current runtime evidence is incomplete. An earlier media-aware `storage_probe`
passed in Dolphin 2609a with 111 checks and 0 failures. It checked no-device
results for USB and CARD, skipped operations that need those devices, and
exercised the virtual Wii SD path. That run did not cover successful USB
transfers, CARD saves, disc reads, GameCube SD Gecko I/O, or physical devices.
See the
[Dolphin report](dolphin-runs/2026-11-03-storage-probe-media-aware/README.md).

The latest rerun completed `card_probe`, `storage_probe`, `fat_sd_probe`, and
`dvd_probe` in Dolphin 2609a. They reported 93, 153, 40, and 120 passing
checks, with no reported failures. The CARD probe ran in GameCube mode with a
copied, verified virtual card image. The other probes ran in Wii mode. The
storage probe used Dolphin's virtual SD card and skipped its media hotplug
soak. The DVD probe ran without a disc. This run adds no physical-device
evidence and does not prove every inventory entry. See the
[audit rerun report](dolphin-runs/2026-11-03-storage-api-audit-rerun/README.md)
for DOL hashes, captures, and limits.
An earlier no-disc DVD probe passed 119 checks and compared C and Rust DI
wrapper results for the calls listed in its
[run report](dolphin-runs/2026-11-03-di-wrapper-parity-final/README.md).
The previous `runtime_probe` passed 85 checks. A later async-format diagnostic
run recorded 86 checks and 1 failure: `format_async` returned submission status
0 and erased the marker file and directory, but its callback did not complete
within the bounded wait. Treat this as a failure, not as a passing operation.
The run used Dolphin's isolated NAND copy. A later run with a longer bounded
wait received callback status 0 and verified marker removal. See the
[earlier diagnostic](dolphin-runs/2026-11-03-isfs-format-async-dolphin/README.md)
and [later run](dolphin-runs/2026-11-03-isfs-format-async-bounded/README.md).
The controlled image verified
ISO9660 file operations through both C and Rust mounts. The ISFS checks cover
closed-service errors, 20 sync and async path rejections, and successful sync
and async NAND file cycles with metadata, directory listing, and verified
cleanup. Async directory creation, usage, listing, and deletion also pass with
output checks after deletion. Sync and async rename returned `ISFS_EINVAL`; the
async result preserved the source and left the destination absent. Successful
rename remains unverified. Sync and async format returned success and removed
marker state from Dolphin's private NAND copy. Physical Wii NAND behavior
remains open. See the [sync format report](dolphin-runs/2026-11-03-isfs-format-sync-dolphin/README.md),
the [async rename report](dolphin-runs/2026-11-03-isfs-async-rename-cycle-dolphin/README.md),
the [async directory report](dolphin-runs/2026-11-03-isfs-async-directory-cycle-dolphin/README.md),
the [async file report](dolphin-runs/2026-11-03-isfs-successful-async-cycle-dolphin/README.md),
the [sync cycle report](dolphin-runs/2026-11-03-isfs-successful-file-cycle-cleanup/README.md),
the [path validation report](dolphin-runs/2026-11-03-isfs-path-length-dolphin/README.md),
and the [earlier runtime report](dolphin-runs/2026-11-03-isfs-get-stats-async-pass/README.md).
The separate `wii_std_fs_probe` passed 99 checks on Dolphin's private virtual
Wii SD card. It covered FAT directory, path, file, metadata, attribute,
timestamp, remount, and cleanup behavior. This is not physical Wii evidence
and does not prove every FAT and filesystem entry. See the
[FAT run report](dolphin-runs/2026-11-03-wii-std-fs-storage/README.md).
The Wii SD interface probe passed 15 checks. It also tested an unaligned
nine-sector write through the eight-sector bounce-buffer boundary, checked all
read-back bytes, restored the saved sectors, and verified restoration on
Dolphin's private virtual SD card. This does not prove physical Wii SD writes.
See the [multi-sector report](dolphin-runs/2026-11-03-wiisd-multisector/README.md)
and the earlier [one-sector report](dolphin-runs/2026-11-03-wiisd-write-roundtrip/README.md).
With Dolphin's virtual SD card disabled, the same probe passed 10 no-card
checks for startup, insertion, reads, writes, zero-sector requests, shutdown,
and restart. See the [no-card report](dolphin-runs/2026-11-03-wiisd-no-card/README.md).
Earlier failed launch attempts remain in their own run directories. The
hardware register still records physical CARD and DI behavior as not tested.
Do not treat the source count as completed storage verification.

### CARD typed query diagnostics

The later [CARD save roundtrip](dolphin-runs/2026-11-03-card-save-roundtrip/README.md)
passed 151 checks in Wii mode and 91 checks in GameCube mode. Both used copies
of the same virtual card. They checked save creation, data and status after
remount, deletion after remount, and free block recovery. Physical card
behavior, card formatting, and removal remain unverified.

The [raw CARD roundtrip](dolphin-runs/2026-11-03-card-raw-roundtrip/README.md)
also calls the raw read and write APIs and checks returned bytes in both modes.

Before the `CARD_Probe` parity correction, an early run expected typed queries
to return the raw slot-probe error. Its
four assertions failed because the APIs return different errors. The corrected
probe passed 104 checks in Dolphin 2609a. Without a memory-card image,
the old `probe_slot` and `probe_info` implementations returned `CARD_ERROR_BUSY` (`-1`), while
`sector_size`, `block_count`, `free_blocks`, and `file_status` returned
`CARD_ERROR_NOCARD` (`-3`). It also checked both `CardSlot` conversions and
that `read_slice` and `write_slice` reject empty, short, misaligned, and
overflowing requests before device I/O. Rejected reads preserved their output
bytes. See the [run report](dolphin-runs/2026-11-03-card-slice-preconditions-dolphin/README.md)
and the [earlier failed assertions](dolphin-runs/2026-11-03-card-query-status/README.md).

The probe skipped mounted-card queries, save-file operations, and format. This
run does not prove successful card geometry or file-status queries, persistence,
detach behavior, or Wii hardware behavior. The sandboxed launch failed at the
Dolphin `--version` check; the successful run used host permission. Target
clippy passed with warnings denied, and `cargo xtask audit-source` accepted the
direct API calls.

The Dolphin runner accepts `LIBOGC_DOLPHIN_CARD_IMAGE` for a raw GameCube card
image. It copies the image into the isolated probe profile before launch, so
the probe cannot change the source image. A run with the existing Dolphin card
image copied this way still returned `CARD_ERROR_NOCARD` and skipped save
operations. It passed 111 other checks. The card template therefore does not
close successful CARD runtime coverage. See the
[card-image attachment diagnostic](dolphin-runs/2026-11-03-card-image-template-dolphin/README.md).

The follow-up `card-owned-transfer-errors-dolphin` run passed 108 checks. It
checked that `read_async_owned` and `write_async_owned` reject invalid
channels; reject empty, short, misaligned, and overflowing transfers; and
return the original file state and write bytes on rejection. These checks do
not cover successful async transfers, callback delivery, completion polling,
abandonment, cancellation, mounted media, or Wii hardware. See the
[run report](dolphin-runs/2026-11-03-card-owned-transfer-errors-dolphin/README.md).

The `card-owned-metadata-errors-dolphin` run passed 111 checks. It also
checked rejected owned create/delete filenames, rejected unterminated
directory-entry names with payload retention, and rejected out-of-range status
file numbers while retaining `CardStat`. These are pre-device validation
errors; successful operations and callbacks remain open. See the
[run report](dolphin-runs/2026-11-03-card-owned-metadata-errors-dolphin/README.md).

CARD game and company identifiers now use tested types in `ogc-card-metadata`.
The host test checks valid bytes, round trips, and the exact index and byte for
NUL and `0xff` errors. `ogc-card-sys` re-exports these types, so existing public
type paths remain available. This closes six API entries; it does not verify
CARD transport behavior.

Owned CARD completion values now share definitions with the host-testable
`ogc-card-metadata` crate. Direct host tests check the status, file, entry,
stat, byte, and ownership accessors, including failed completion payloads.
These tests do not verify CARD hardware behavior.

`cargo test -p ogc-card-metadata --lib` currently passes all 17 tests. The
evidence registry maps all 56 inventory entries in `ogc-card-metadata` to
these host test functions. This closes its host-testable metadata and owned
value batch. It does not close the 153 entries in `ogc-card` and `ogc-card-sys`
or prove transport behavior.

The earlier planning count of 348 open storage entries is stale. Use
`cargo xtask check-inventory` and the generated inventory as the current
source of truth.

Milestone 1's scheduler source and target build are prerequisites. Its real-Wii stress acceptance remains open because the current probe display produced a solid green frame rather than a readable result. Storage work therefore records scheduler hardware validation as pending rather than assuming it passed.

## DVD compatibility over `/dev/di`

`ogc-dvd-sys` is a compatibility facade over `ogc-di`. It owns one LWP worker and a bounded serialized request queue. The public asynchronous operations enqueue work and return before the worker invokes the callback.

Implemented operations:

- initialization and shutdown of `/dev/di`;
- mount, cover/media status, drive status, and last-error reporting;
- reset and motor control;
- drive inquiry and disc ID reads;
- aligned synchronous and asynchronous reads;
- seek validation through a real low-level read at the requested position;
- GCM offset handling;
- queued cancellation with canceled completion delivery;
- cache invalidation for read destinations;
- current disc-ID and drive-information compatibility storage.

The queue is serialized because IOS DI command ownership is exclusive. Cancellation cannot abort an IOS ioctl that is already executing. It invalidates the active request generation. The worker then delivers canceled completions for the active and queued operations in queue order, followed by the cancellation callback; no declared asynchronous callback is invoked by the submitting stack frame.

### DVD limitations

- The compatibility priority argument is accepted but the current worker is serialized FIFO. Priority queueing is not yet hardware-validated.
- IOS does not expose a separately verified seek primitive through this implementation. Seek performs a small aligned read so the drive and IOS validate the requested position.
- `DVD_StopStreamAtEnd` uses the verified DI motor-stop path because no stable audio-stream-stop ioctl has been validated across Wii IOS revisions. Applications that require continuous drive audio streaming must treat this as a documented behavioral difference.
- Read buffers must be 32-byte aligned. Byte offsets must be four-byte aligned because IOS DI encodes them as word offsets. Invalid buffers and offsets return errors rather than success.

## CARD asynchronous worker and detach handling

`ogc-card-sys` now contains only ABI definitions and forwarding code in its public module. The previous host-style card state is not compiled into the PowerPC runtime. The Wii backend owns EXI access, card metadata, and one serialized LWP worker.

The following APIs perform queued work:

- mount;
- read and write;
- create by name or directory entry;
- delete by name or directory entry;
- format;
- status updates;
- attribute updates.

The worker polls mounted slots for removal. On detach it:

1. clears the mounted and work-area state;
2. increments the slot generation;
3. cancels queued work for the slot;
4. invalidates previously opened `CardFile` handles;
5. invokes the registered detach callback from worker context.

Partial writes use a per-slot aligned sector scratch buffer. They read the complete card sector, update the requested 512-byte-aligned range, erase the physical sector, and write the complete sector back.

CARD metadata uses the redundant directory and FAT sectors as alternating commits. Mount validates both copies, including the FAT free-block count, and selects the newest valid copy with a wrapping update counter. One valid copy is sufficient for recovery. Each change is applied to the inactive copy, checksummed, erased, written, read back, and compared before it becomes active. Create commits its FAT allocation before its directory entry. Delete removes its directory entry before it frees the FAT chain. This ordering can leak blocks after an interrupted two-block operation, but it cannot publish an entry that points to unallocated blocks. The API returns the actual metadata, removal, or transport error and reloads on-card state after a failed commit.

Format rebuilds complete 8 KiB header, directory, and FAT blocks. It writes and verifies all five system sectors, then reloads and validates them before it reports success.

The compatibility `CardFile.offset` field is used as an opaque mount-generation token. The public read and write functions already receive their byte offset explicitly, so the field is not treated as a moving cursor by this implementation. This preserves the structure layout but is a documented runtime-semantic difference from callers that inspect `offset` directly.

`shutdown()` drains queued callbacks with `CARD_ERROR_CANCELED`, joins the worker, destroys its LWP synchronization objects, unregisters detach callbacks, increments generations, and unmounts both slots.

### CARD callback context

Attach, operation-completion, and detach callbacks run on the CARD worker thread, not in EXI interrupt context. Callback code may use normal LWP-safe operations but must not recursively wait for another queued CARD request from the same worker.

## FAT mounts for SD and USB

`ogc-fat` registers FAT32 volumes through devoptab as `sd:` and `usb:`. It supports partition discovery, FAT32 boot-sector validation, normal file and directory operations, `fsync`, unmount, and global shutdown.
The current FAT service requires 512-byte logical sectors; devices exposing larger logical sectors return `UnsupportedVariant` rather than being mounted with incorrect geometry.

Each mount has an online flag and media generation. A background LWP monitor polls insertion state. When media disappears, the mount is unregistered immediately and marked offline. Existing file or directory allocations remain long enough to close safely, but every subsequent I/O operation returns a media-removed error. The old allocation is never made online again. A remount is allowed only after stale handles close.

USB `fsync`, unmount, and shutdown issue SCSI `SYNCHRONIZE CACHE` before closing the device. SD writes are synchronous through the Wii SD transport. `shutdown_all()` refuses to remove any mount while handles remain open.

An intentional USB close is retryable. It flushes every mounted LUN, retains the first flush error, and does not close the USB transport when any flush fails. A USB-close failure also retains the handle and backend slot. Only a successful close releases state. Confirmed physical removal uses the separate forced-invalidation path because the missing device cannot be flushed or retried.

USB deinitialization, disc shutdown, FAT unmount, and FAT global shutdown propagate close failures. A failed FAT teardown keeps an unregistered mount record so a later unmount or shutdown call can retry the retained transport. `ogc_usbstorage::deinitialize()` returns the raw status, and `try_deinitialize()` converts it to the typed result.

## USB storage lifecycle

The USB mass-storage backend now:

- verifies the mounted device by device ID, VID, and PID on every insertion check;
- detects removal even after a device was successfully mounted;
- skips SCSI cache commands after physical removal and invalidates the stale IOS handle directly;
- invalidates the global disc handle when the device vanishes;
- exposes explicit per-LUN and global-interface flush operations;
- flushes mounted LUNs before close;
- serializes flush and close against data transfers;
- closes the USB device, releases backend slots, and clears cached capacity and identity state only after successful deinitialization.

## Build verification

Run:

```sh
cargo fmt --all
cargo fmt --all --check
cargo xtask inventory
cargo xtask milestone2-verify
```

`milestone2-verify` checks repository roles, dependency direction, source audit, inventory freshness, all host-testable packages, every package in the storage group with `-D warnings`, and builds the focused storage probe.

The DOL is written to:

```text
target/xtask/probes/storage_probe.dol
```

The normal probe never formats a card. Destructive format verification requires this exact DOL argument:

```text
--destructive-card-format=ERASE-SLOT-A
```

This argument is the operator confirmation. It erases all data on the memory card in slot A. The probe then unmounts, remounts with a fresh work area, and checks the formatted free-block count.

## Probe logging and debugger hooks

The storage probe writes ordinary probe records and exports these symbols for a debugger or crash monitor:

- `OGC_STORAGE_PROBE_STAGE`
- `OGC_STORAGE_PROBE_LAST_RESULT`
- `OGC_STORAGE_PROBE_LWP_EVENTS`
- `OGC_STORAGE_PROBE_USB_V5_REMOVALS`
- `OGC_STORAGE_PROBE_USB_V5_LAST_RESULT`
- `OGC_STORAGE_PROBE_USB_V5_LAST_USERDATA`

Stage values:

| Stage | Operation |
| --- | --- |
| 0 | not started |
| 1 | DVD |
| 2 | USB storage and FAT |
| 3 | Wii SD and FAT |
| 4 | CARD persistence, async operations, optional format, and detach |
| 5 | removable-media soak and direct USB v5 removal callbacks |
| 6 | completed |
| `0xffffffff` | panic |

The probe attempts persistent logging through the existing probe logger. Do not place the log on a volume that will be removed during the hotplug soak; the probe skips the soak when its mounted log is on removable media.

For USB v5 acceptance, remove the USB mass-storage device, reinsert it, and
remove it a second time during the soak. The probe requires two callbacks with
the exact user-data tokens and `USB_OK`. It also requires an operation through
each removed device ID to fail. Cleanup callbacks do not increment
`OGC_STORAGE_PROBE_USB_V5_REMOVALS` and cannot satisfy this check.
