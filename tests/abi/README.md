# ABI Checks

This directory is reserved for ABI and layout validation.

Current contents:

- `cargo xtask check-abi` prints the current Rust-side layout snapshot for `MOD_INSTR`, `MOD`, `MODSNDBUF`, and `MODPlay`
- `cargo xtask check-abi-rust-target` builds `crates/abi/abi-probe` for `targets/powerpc-nintendo-none.json` and extracts the canonical Rust-side target layout facts for the same ABI surface
- `tests/abi/modplay-rust-layout-powerpc-nintendo-none.txt` records the current Rust-side custom-target snapshot for the modplay ABI surface
- the canonical target remains 32-bit `powerpc-eabi`, so host snapshots are informative but not sufficient

To verify the actual exported ModPlay frequency table in a freshly built
PowerPC object, run `python3 tests/modplay-target-table/check.py`. The check
requires nightly Rust, rust-src, and LLVM tools. It checks the ELF target,
locates the exported `FREQTAB_TABLE` symbol and verifies its section bytes.

Verified target-build command:

```sh
rustup toolchain install nightly
rustup component add rust-src --toolchain nightly

RUSTC="$(rustup which --toolchain nightly rustc)" \
rustup run nightly cargo build \
  -Z build-std=core,alloc \
  -Z json-target-spec \
  --target ./targets/powerpc-nintendo-none.json \
  -p abi-probe
```

- `xtask` is host-only and should not be built for the Wii target.
- Runtime crates should not be checked with host `cargo check --workspace`; use the PowerPC target and explicit packages.
- Current `modplay` ABI snapshots match between the recorded Rust target outputs.
