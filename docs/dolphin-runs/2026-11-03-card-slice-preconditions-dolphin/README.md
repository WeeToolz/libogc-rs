# CARD no-media and slice precondition probe

## Result

`storage_probe` passed in Dolphin 2609a with 104 checks and no failures. The
DOL SHA-256 is
`1384b2daed9458aca4fc448438633cb9410ef6164270887683e92d5d6a5538bb`. The
report has 11 page captures and no video frames. See [`suite.json`](suite.json)
and [`storage_probe/probe.log`](storage_probe/probe.log).

The private Dolphin profile had no GameCube memory-card image. The probe
checked that `probe_slot` and `probe_info` returned `CARD_ERROR_BUSY` (`-1`),
and that `sector_size`, `block_count`, `free_blocks`, and `file_status`
returned `CARD_ERROR_NOCARD` (`-3`). It also checked slot A/B conversions and
that `read_slice` and `write_slice` reject empty, short, misaligned, and
overflowing requests before device I/O. Rejected reads preserved the output
bytes. The mounted-card save operations remained skipped.

This run does not verify successful geometry or file-status queries, CARD
reads or writes with a card present, save-file persistence, format, detach
behavior, or physical Wii behavior. The previous sandboxed attempt stopped at
the Dolphin `--version` check. This run used the same validation script with
host permission, which let Dolphin start.

## Reproduction

```sh
LIBOGC_MAD_PROBE_MP3=[libogc-rs fixtures]/layer3-stereo.mp3 \
LIBOGC_MAD_REFERENCE_MP3=[libogc-rs fixtures]/libmad-stereo-mpeg1.mp3 \
cargo xtask dolphin-validate --probe storage_probe \
  --artifact-dir docs/dolphin-runs/2026-11-03-card-slice-preconditions-dolphin
```
