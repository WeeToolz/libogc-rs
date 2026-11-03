# USB storage flush checks in Dolphin

## Run details

- Dolphin: 2609a
- Probe: `usbstorage_capacity_probe`
- Repository revision: `ff95f9a36fbea40e11b021a8e309a96233d9bfc7`
- The suite report marks the worktree dirty because the tested probe source
  contained the raw USB backend checks added in this run.
- Probe DOL SHA-256: `521044a84790877c91e888c67d27f502618f2a9134529941ffcc3469103ad7a9`
- MP3 fixture SHA-256: `c1f5d2789060c3e3e1527ebd08c4a7777fb0ca55f4f46da720c919e518b9ae73`
- Result: pass, 48 checks, 0 failures
- Command:

  ```sh
  LIBOGC_MAD_PROBE_MP3=[external tools]/libmad-rs/test-data/layer3/libmad-stereo-mpeg1.mp3 \
  LIBOGC_MAD_REFERENCE_MP3=[external tools]/libmad-rs/test-data/layer3/libmad-stereo-mpeg1.mp3 \
  cargo xtask dolphin-validate --probe usbstorage_capacity_probe \
    --artifact-dir docs/dolphin-runs/2026-11-03-storage-usb-flush
  ```

The probe confirms that raw `flush` and safe `try_flush` return the interface
error for a null handle. It also checks the 60-second remount boundary, LUN
count conversion, short-read residue, capacity decoding, BOT retry rules, and
READ/WRITE CSW status handling. The C ABI checks cover null-handle errors,
capacity output preservation, unmounted ioctl, the exported ioctl bridge, and
the deinitialization status. Dolphin had no USB mass-storage device. This run
also checks raw backend error paths and the disc-interface status callback.
Dolphin had no USB mass-storage device. This run does not verify successful
cache synchronization on a mounted LUN or USB transport behavior. No USB data
was written.

The complete report and probe log are in this directory. The private Dolphin
profile is ignored by Git.
