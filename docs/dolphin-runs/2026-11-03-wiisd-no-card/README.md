# Wii SD no-card checks

## Result

`wiisd_probe` passed in Dolphin 2609a with 10 checks and no failures. The DOL
SHA-256 hash is
`a7ab3374a84c202bba8f6822aa73fe9e8273e89363b3eb2856ac42c0b6b21b5b`.

The runner disabled Dolphin's virtual Wii SD card. The production interface
reported no startup and no inserted card. A one-sector read failed and kept
the destination bytes unchanged. A one-sector write failed. A zero-sector
write failed without changing the probe buffer. Shutdown and restart kept the
interface closed. These checks cover absent-media behavior in Dolphin; they do
not verify physical Wii SD hardware.

## Reproduction

Build the probes after you set the MP3 fixture variables described in
[`dependencies.md`](../../dependencies.md):

```sh
LIBOGC_MAD_PROBE_MP3=<path-to-probe-mp3> \
LIBOGC_MAD_REFERENCE_MP3=<path-to-reference-mp3> \
cargo xtask build-dolphin-probes

cargo xtask dolphin-validate --no-build --probe wiisd_probe \
  --disable-wii-sd-card \
  --dolphin <path-to-dolphin-executable> \
  --artifact-dir target/dolphin-wiisd-no-card
```
