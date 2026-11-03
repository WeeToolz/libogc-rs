# CARD owned async transfer rejection probe

## Result

`storage_probe` passed in Dolphin 2609a with 108 checks and no failures. The
DOL SHA-256 is
`6606cc99ccaa494359bd9d36f2effb654fa368ddb448ebe323d96ff99801b124`. See
[`suite.json`](suite.json) and
[`storage_probe/probe.log`](storage_probe/probe.log).

The private profile had no GameCube memory card. The probe checked that
`read_async_owned` and `write_async_owned` reject an invalid channel and return
the original file state. It also passed empty, short, misaligned, and
overflowing transfer shapes to both APIs. It checked `CARD_ERROR_FATAL_ERROR`
and confirmed that rejected writes return their original bytes. These
validation failures occur before the APIs submit device I/O.

This run does not prove a successful async transfer, callback delivery,
completion polling, request abandonment, cancellation, or card behavior with
mounted media. It does not prove Wii hardware behavior.

## Reproduction

```sh
LIBOGC_MAD_PROBE_MP3=[libogc-rs fixtures]/layer3-stereo.mp3 \
LIBOGC_MAD_REFERENCE_MP3=[libogc-rs fixtures]/libmad-stereo-mpeg1.mp3 \
cargo xtask dolphin-validate --probe storage_probe \
  --artifact-dir docs/dolphin-runs/2026-11-03-card-owned-transfer-errors-dolphin
```
