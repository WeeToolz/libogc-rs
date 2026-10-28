# Milestone 2 real-Wii hardware report

Milestone 2 is not hardware-accepted until this report is completed from a real Wii run of `target/xtask/probes/storage_probe.dol`.

## Build information

- Repository/archive revision:
- Rust nightly:
- Wii model and region:
- IOS used by loader:
- Loader and AHBPROT state:
- SD card model/capacity/filesystem:
- USB device model/capacity/filesystem:
- GameCube memory card size/vendor/slot:
- Disc type and title:

## Build gates

```text
cargo fmt --all --check: PASS / FAIL
cargo xtask inventory: PASS / FAIL
cargo xtask milestone2-verify: PASS / FAIL
storage_probe.dol produced: YES / NO
```

## DVD results

- `/dev/di` initialization:
- cover open/closed and disc-present transitions:
- mount:
- inquiry:
- disc ID:
- synchronous sector 0 read:
- asynchronous sector 0 read:
- asynchronous callback observed after API return:
- cancellation result:
- motor stop and shutdown:
- last DI error word on failure:

## CARD results

- slot and card type:
- asynchronous mount callback after return:
- file create:
- unmount/remount after create:
- file and status metadata persisted after remount:
- 512-byte partial write:
- readback comparison:
- file close and delete:
- unmount/remount after delete:
- deleted entry absent and free-block count restored:
- destructive format argument supplied: YES / NO
- format completion and remount validation:
- detach callback:
- stale `CardFile` rejected after removal:
- remount after reinsertion:
- repeated cycles completed:
- corruption or checksum errors observed:

## FAT and removable-media results

### SD

- mount `sd:`:
- create/write/fsync/read/delete:
- removal detected:
- old handle rejected:
- remount after closing stale handle:
- cycle count:

### USB

- mount `usb:`:
- create/write/fsync/read/delete:
- `SYNCHRONIZE CACHE` result:
- unmount and USB deinitialize results:
- remount after close:
- close-probe file persisted after remount:
- removal detected:
- old handle rejected:
- remount after closing stale handle:
- cycle count:

## Shutdown results

- `fat::shutdown_all`:
- `card::shutdown`:
- `dvd::shutdown`:
- repeated initialize/shutdown cycle:
- stale IOS, EXI, or USB handles observed:

## Debug state

- Last visible probe record:
- `OGC_STORAGE_PROBE_STAGE`:
- `OGC_STORAGE_PROBE_LAST_RESULT`:
- `OGC_STORAGE_PROBE_LWP_EVENTS`:
- `OGC_STORAGE_PROBE_USB_V5_REMOVALS`:
- `OGC_STORAGE_PROBE_USB_V5_LAST_RESULT`:
- `OGC_STORAGE_PROBE_USB_V5_LAST_USERDATA`:
- Persistent log path:
- Persistent log attached: YES / NO

## Outcome

- PASS / FAIL / BLOCKED
- First failing operation:
- Exact return code/error word:
- Reproduction steps:
- Notes:
