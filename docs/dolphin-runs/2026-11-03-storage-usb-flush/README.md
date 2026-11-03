# USB storage flush checks in Dolphin

## Run details

- Dolphin: 2609a
- Probe: `usbstorage_capacity_probe`
- Repository revision: `a8bfed3`
- The suite report marks the worktree dirty because the tested probe source
  contained the two new flush checks.
- Probe DOL SHA-256: `5ef1193246a55133951acd6dbe4ac6ab8ea5ab1f8199a2071dd5d3d3f76f2cba`
- MP3 fixture SHA-256: `c1f5d2789060c3e3e1527ebd08c4a7777fb0ca55f4f46da720c919e518b9ae73`
- Result: pass, 29 checks, 0 failures
- Command:

  ```sh
  LIBOGC_MAD_PROBE_MP3=[libmad-rs source]/test-data/layer3/libmad-stereo-mpeg1.mp3 \
  LIBOGC_MAD_REFERENCE_MP3=[libmad-rs source]/test-data/layer3/libmad-stereo-mpeg1.mp3 \
  cargo xtask dolphin-validate --probe usbstorage_capacity_probe \
    --artifact-dir docs/dolphin-runs/2026-11-03-storage-usb-flush
  ```

The probe confirms that raw `flush` and safe `try_flush` return the interface
error for a null handle. Dolphin had no USB mass-storage device. This run does
not verify successful cache synchronization on a mounted LUN or USB transport
behavior. No USB data was written.

The complete report and probe log are in this directory. The private Dolphin
profile is ignored by Git.
