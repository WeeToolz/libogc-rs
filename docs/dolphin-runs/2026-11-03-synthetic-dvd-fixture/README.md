# Synthetic DVD fixture run

The existing Dolphin runner attached a generated synthetic Wii-format image
to `dvd_probe` on Dolphin 2609a. The image contained a `TST1TS` disc ID, Wii
magic, and a 2,048-byte test pattern at offset `0x2000`. Its SHA-256 was
`eeabeac80855703cd5909fdad7ccaebcc97663bdede38a7afe8d241788c37335`.
The DOL SHA-256 was
`f2e9d6838a54021cd2fefc61639d01fdb5e6a99859e0acfbcd5e2c98953b1eee`.

The [probe log](probe.log) records **35 passes and 4 failures**. Disc ID,
sector-zero reads, the full offset-sector asynchronous read, and synchronous
and asynchronous two-sector reads across the payload boundary passed. DVD
mount failed, as did three direct DI inserted-disc, ID, and read checks. Direct
DI asynchronous reads were skipped because the fixture did not verify that
path. This image has no signed Wii partition or filesystem and does not prove
successful mount or partition handling. The runner's [suite report](suite.json)
records the result and page captures.

The logged `di media C sector read matches fixture` pass is invalid: the old
check only compared the C and Rust results, and both returned an error and
identical buffers. The probe now requires a successful C read and the expected
header bytes. The old total remains here as the original runner output; it
must not be used as evidence of a successful C read.

The run used revision `27fe84f771bda42fe348085d894dc6724d7be710` with an
untracked fixture generator in the worktree. Reproduce with new paths:

```sh
python3 scripts/make-dolphin-dvd-fixture.py /path/to/new-fixture.iso
cargo xtask dolphin-validate --no-build --probe dvd_probe \
  --disc-image /path/to/new-fixture.iso --dolphin /path/to/Dolphin \
  --artifact-dir /path/to/new-report
```

The image has a logical size of 4,699,979,776 bytes. Some filesystems allocate
all of that space when the image is created.
