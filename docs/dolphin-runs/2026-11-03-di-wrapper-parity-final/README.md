# DI wrapper parity checks in Dolphin

## Result

The `dvd_probe` passed in Dolphin 2609a with 119 checks and 0 failures. It
captured 10 pages and no video frames. The private Dolphin profile had no
disc. The probe checked no-disc behavior and compared C ABI wrappers with the
paired Rust APIs for drive identification, DVD support, disc ID, cover status,
error status, configuration, copyright, disc key, physical data, BCA, and
report-key calls.

The probe checked that protected-read C and Rust calls returned the same
results and bytes. The BCA and report-key calls used non-zero report-key
arguments (`keytype=0x5a`, `lba=0x1234`). This run does not verify successful
disc reads, partition access with media, valid disc contents, or physical Wii
behavior.

Two consecutive `DI_GetError` calls both returned success and changed their
output from its sentinel. The outputs differed: Rust call `0x01023a00`, then C
wrapper call `0x01000000`. The probe checks each result and output update. It
does not assume that these consecutive drive error queries return identical
values.

## Reproduction

```sh
LIBOGC_MAD_PROBE_MP3=[external tools]/libogc-rs-fixtures/layer3-stereo.mp3 \
LIBOGC_MAD_REFERENCE_MP3=[external tools]/libogc-rs-fixtures/libmad-stereo-mpeg1.mp3 \
cargo xtask dolphin-validate --probe dvd_probe \
  --artifact-dir docs/dolphin-runs/2026-11-03-di-wrapper-parity-final
```

The fixture hashes are recorded in the
[media-aware storage run](../2026-11-03-storage-probe-media-aware/README.md).

## Build evidence

- Repository revision: `c5d231959720204f85df7f865c4ce2ed572245c3` with the probe changes in this batch
- Dolphin: `Dolphin 2609a`
- DOL SHA-256: `d286526ba5feb71a17b821b99267052c2ebff93c7fb1558425eeadd0f116ec19`
- Result: `passed=true`; 119 passed; 0 failed
- Final page: `dvd_probe/pages/page-010.png`
- Suite report: [`suite.json`](suite.json)
- Probe log: [`dvd_probe/probe.log`](dvd_probe/probe.log)

The earlier wrapper comparison attempts are retained in the
[initial failing run](../2026-11-03-di-wrapper-parity/README.md) and its
[diagnostic rerun](../2026-11-03-di-wrapper-parity-diagnostic/README.md).
