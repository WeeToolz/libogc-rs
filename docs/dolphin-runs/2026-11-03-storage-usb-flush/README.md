# USB storage flush checks in Dolphin

## Run details

- Dolphin: 2609a
- Probe: `usbstorage_capacity_probe`
- Repository revision: `054525e289f8e4010dc14182f9870f72ce17986d`
- The suite report marks the worktree dirty because the tested probe source
  contained the USB C ABI checks added in this run.
- Probe DOL SHA-256: `a5b007d7663bccee99e765346dff87169eeb0b89c9b274473f6f0dec6e3fffb9`
- MP3 fixture SHA-256: `c1f5d2789060c3e3e1527ebd08c4a7777fb0ca55f4f46da720c919e518b9ae73`
- Result: pass, 43 checks, 0 failures
- Command:

  ```sh
  LIBOGC_MAD_PROBE_MP3=[libmad-rs source]/test-data/layer3/libmad-stereo-mpeg1.mp3 \
  LIBOGC_MAD_REFERENCE_MP3=[libmad-rs source]/test-data/layer3/libmad-stereo-mpeg1.mp3 \
  cargo xtask dolphin-validate --probe usbstorage_capacity_probe \
    --artifact-dir docs/dolphin-runs/2026-11-03-storage-usb-flush
  ```

The probe confirms that raw `flush` and safe `try_flush` return the interface
error for a null handle. It also checks the 60-second remount boundary, LUN
count conversion, short-read residue, capacity decoding, BOT retry rules, and
READ/WRITE CSW status handling. The C ABI checks cover null-handle errors,
capacity output preservation, unmounted ioctl, the exported ioctl bridge, and
the deinitialization status. Dolphin had no USB mass-storage device. This run
does not verify successful cache synchronization on a mounted LUN or USB
transport behavior. No USB data was written.

The complete report and probe log are in this directory. The private Dolphin
profile is ignored by Git.
