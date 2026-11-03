# GameCube SD API checks in Dolphin

## Run details

- Dolphin: 2609a
- Probe: `fat_sd_probe`
- Repository revision: `6872b91265f4e7ebe4332f47e51fabade6643cac`
- The suite report marks the worktree dirty because the tested probe source
  contained the SD Gecko backend checks added in this run.
- Probe DOL SHA-256: `b7c0d0135d79c8b6dfe8eea5a64f05ebc1e37c340d695a80e38d7d6b407fb38f`
- MP3 fixture SHA-256: `c1f5d2789060c3e3e1527ebd08c4a7777fb0ca55f4f46da720c919e518b9ae73`
- Result: pass, 36 checks, 0 failures
- Command:

  ```sh
  LIBOGC_MAD_PROBE_MP3=[libmad-rs source]/test-data/layer3/libmad-stereo-mpeg1.mp3 \
  LIBOGC_MAD_REFERENCE_MP3=[libmad-rs source]/test-data/layer3/libmad-stereo-mpeg1.mp3 \
  cargo xtask dolphin-validate --probe fat_sd_probe \
    --artifact-dir docs/dolphin-runs/2026-11-03-gcsd-invalid-drive
  ```

The checks cover invalid-drive results for C and Rust entry points, no-card
backend results, slot A interface callbacks, speed and page-size state
restoration, and allocation, exhaustion, freeing, and reuse of the three-buffer
pool. Dolphin has no EXI SD Gecko adapter. FAT mounts for slots A and B return
`DeviceUnavailable`. This run does not test successful EXI reads or writes, a
physical SD Gecko, or behavior on GameCube hardware.

The report, logs, and page captures are in this directory. The private Dolphin
profile is ignored by Git.
