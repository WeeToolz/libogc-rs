# Reproduce MAD PowerPC differential

Use the corrected checkout, original C libogc, and the two original MP3
fixtures. This standalone diagnostic does not change the source trees or
need a Wii NAND. It uses C only as an independent test reference. Supply the
restored offline Rust nightly with rust-src, Clang, ld.lld, Python 3, readelf,
Xvfb, and Dolphin. Set the following variables to absolute paths:

```sh
export LIBOGC_C_SOURCE=/path/to/libogc
export MAD_FIXTURE_DIR=/path/to/libogc-rs-fixtures
export CARGO_VENDOR_DIR=/path/to/offline-cargo/vendor
export DOLPHIN_BIN=/path/to/dolphin/AppRun
bash scripts/mad-ppc-differential/run.sh codec
bash scripts/mad-ppc-differential/run.sh abi
```

The script verifies recorded codec and C source hashes, compiles the reference
and Rust diagnostic, runs it in Dolphin, and checks the guest result. The ABI
variant uses the checkout's mad-sys wrapper. Each run prints its temporary
result directory. See `docs/dolphin-runs/2026-11-27-mad-ppc/README.md` for scope
and limitations. The source manifest retains the baseline hashes and corrected
hashes for the two changed codec files. The script checks the corrected hashes.
