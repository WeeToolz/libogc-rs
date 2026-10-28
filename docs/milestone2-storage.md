# Milestone 2: Wii storage runtime

Milestone 2 replaces the remaining disabled or inline-completion storage facades with Wii-backed implementations. The code is target-only and must be validated on real Wii hardware before the milestone is hardware-accepted.

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
