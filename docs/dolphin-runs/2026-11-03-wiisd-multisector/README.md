# Wii SD multi-sector round trip

## Result

`wiisd_probe` passed in Dolphin 2609a with 15 checks and no failures. The DOL
SHA-256 hash is
`ed35abbb8ccd954d72c80cdd71f6a1802eb30b08ffe1ed6c6274750e4cb9c189`.

The probe tested the production Wii SD interface on Dolphin's private virtual
SD card. It saved sectors 1 through 9, wrote a test pattern through an
unaligned buffer, read back and compared all bytes, then restored and verified
the saved bytes. The unaligned buffer uses the production bounce-buffer path,
which transfers at most eight sectors in one chunk. The test therefore checks
the transition from the first chunk to the next chunk. The earlier one-sector
round trip, zero-sector request, shutdown, and restart checks also passed.

This run does not prove physical Wii SD behavior, or every storage API in the
inventory. The card image was private to this Dolphin run.

## API path inventory rerun

The `wiisd_probe` DOL also passed a fresh Dolphin 2609a run with 15 checks and
0 failures. Its SHA-256 is
`10346fd10cbb909eb91166eb905fe4f0adf0c15b48d57b4f45e09e82e5b40ea9`. The
runner captured 3 readable pages. The [complete probe log](api-path-audit-rerun/probe.log)
and [summary capture](api-path-audit-rerun/summary-page.png) are retained.

This DOL binds and calls the five `WII_SD_INTERFACE` callback fields listed in
the implementation evidence registry. The probe tested Dolphin's virtual SD
card, including one-sector and unaligned nine-sector write/read/restore
cycles, shutdown, and restart. This result does not prove physical Wii SD
behavior.

## Reproduction

Build the probes after you set the MP3 fixture variables described in
[`dependencies.md`](../../dependencies.md):

```sh
LIBOGC_MAD_PROBE_MP3=<path-to-probe-mp3> \
LIBOGC_MAD_REFERENCE_MP3=<path-to-reference-mp3> \
cargo xtask build-dolphin-probes

cargo xtask dolphin-validate --no-build --probe wiisd_probe \
  --dolphin <path-to-dolphin-executable> \
  --artifact-dir target/dolphin-wiisd-multisector
```
