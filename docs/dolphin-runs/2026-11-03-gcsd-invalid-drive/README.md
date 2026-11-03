# GameCube SD API checks in Dolphin

## Run details

- Dolphin: 2609a
- Probe: `fat_sd_probe`
- Repository revision: `7cfe7ebb805fccc2985f4e1915e3e34af96f3de1`
- The suite report marks the worktree dirty because the tested probe source
  contained the SD Gecko API checks added in this run.
- Probe DOL SHA-256: `105341d9c498d04791e12362d0d62d0c68df760d2a7e2af5bd9591b30845a57e`
- MP3 fixture SHA-256: `c1f5d2789060c3e3e1527ebd08c4a7777fb0ca55f4f46da720c919e518b9ae73`
- Result: pass, 30 checks, 0 failures
- Command:

  ```sh
  LIBOGC_MAD_PROBE_MP3=[libmad-rs source]/test-data/layer3/libmad-stereo-mpeg1.mp3 \
  LIBOGC_MAD_REFERENCE_MP3=[libmad-rs source]/test-data/layer3/libmad-stereo-mpeg1.mp3 \
  cargo xtask dolphin-validate --probe fat_sd_probe \
    --artifact-dir docs/dolphin-runs/2026-11-03-gcsd-invalid-drive
  ```

The checks cover invalid-drive results for C and Rust entry points, speed
state changes and restoration, and allocation, exhaustion, freeing, and reuse
of the three-buffer pool. Dolphin has no EXI SD Gecko adapter. FAT mounts for
slots A and B return `DeviceUnavailable`. This run does not test successful
EXI reads or writes, a physical SD Gecko, or behavior on GameCube hardware.

The report, logs, and page captures are in this directory. The private Dolphin
profile is ignored by Git.
