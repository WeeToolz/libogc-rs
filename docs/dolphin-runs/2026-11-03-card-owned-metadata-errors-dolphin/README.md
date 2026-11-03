# CARD owned metadata rejection probe

## Result

`storage_probe` passed in Dolphin 2609a with 111 checks and no failures. The
DOL SHA-256 is
`3a43a18f7fbf677b6b412011fa0ae50818eeb1262c96fcbd6656908cd44fa04f`. See
[`suite.json`](suite.json) and
[`storage_probe/probe.log`](storage_probe/probe.log).

The private profile had no GameCube memory card. The probe checked that owned
create and delete reject embedded NUL bytes and filenames longer than
`CARD_FILENAMELEN`. It checked that owned create-entry and delete-entry reject
unterminated fixed-size names and return the original directory entry. It
checked that `set_status_async_owned` rejects negative and out-of-range file
numbers and returns the original status structure. These are input-validation
errors before device I/O.

This run does not prove successful create, delete, entry, or status operations;
it does not prove callbacks, completion polling, request cleanup, mounted card
behavior, or physical Wii behavior.

## Reproduction

```sh
LIBOGC_MAD_PROBE_MP3=[libogc-rs fixtures]/layer3-stereo.mp3 \
LIBOGC_MAD_REFERENCE_MP3=[libogc-rs fixtures]/libmad-stereo-mpeg1.mp3 \
cargo xtask dolphin-validate --probe storage_probe \
  --artifact-dir docs/dolphin-runs/2026-11-03-card-owned-metadata-errors-dolphin
```
