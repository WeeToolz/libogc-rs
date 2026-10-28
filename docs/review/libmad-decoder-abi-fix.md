# libmad decoder ABI and initialization fix handoff

Implementer: `/root/worker_b`.

## BUG FIX

C declares `MadDecoderAsync.pid`, `in`, and `out` as `s32`. Rust declared `pid` as `i64`, which changed the size and offsets of the embedded async state and the later `MadDecoder` fields on 32-bit PowerPC. Rust also initialized decoder mode, async input, and async output to zero, while C initializes them to `-1`.

Rust now uses `i32` for all three async fields, gives the async default values `0, -1, -1`, and initializes decoder mode to `-1`. Target-side regression coverage checks field offsets, complete decoder layout, and initialized values against the C declarations.

## Validation

The PowerPC target `cargo check -p mad-sys` passed with `RUSTFLAGS="-D warnings"`. The target test build could not compile because the custom `powerpc-nintendo-none` target does not provide `std` or the `test` crate, so the regression tests did not run. No hardware verification was performed. Other `mad-sys` findings remain open.
