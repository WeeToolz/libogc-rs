# USB storage flush checks in Dolphin

## Run details

- Dolphin: 2609a
- Probe: `usbstorage_capacity_probe`
- Repository revision: `944beaca18dc1e91c0e021399702c54bbe9fd15b`
- The suite report marks the worktree dirty because the tested probe source
  contained the flush, remount, and READ/WRITE policy checks added in this run.
- Probe DOL SHA-256: `3804b516cead31a15daaf7853fbf482ee22a70855ced1f73ed0cbabac0ff7b83`
- MP3 fixture SHA-256: `c1f5d2789060c3e3e1527ebd08c4a7777fb0ca55f4f46da720c919e518b9ae73`
- Result: pass, 31 checks, 0 failures
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
READ/WRITE CSW status handling. Dolphin had no USB mass-storage device. This
run does not verify successful cache synchronization on a mounted LUN or USB
transport behavior. No USB data was written.

The complete report and probe log are in this directory. The private Dolphin
profile is ignored by Git.
