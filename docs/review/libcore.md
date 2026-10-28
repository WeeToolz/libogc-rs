# libogc-core Review

Date: 2026-10-01

Status: static review of the system constants, memory address helpers, RNC1 decoder, and RNC2 decoder is complete. No confirmed behavior mismatch was found in these paths. This is not hardware verification.

## Scope

- `crates/core/ogc-core`
- C references: `C-sources/libogc/libogc/depackrnc1.c`, `depackrnc.S`, `C-sources/libogc/gc/ogc/gccore.h`, and `gc/ogc/system.h`

## Reviewed behavior

- System constant values and cached/uncached address conversion helpers.
- RNC1 header values, CRC checks, Huffman decoding, output length handling, and error codes.
- RNC2 bit and carry operations, raw copies, match copies, and end marker handling against the PowerPC assembly implementation.

Rust checks buffer bounds and malformed RNC1 inputs that the C ABI does not check. The RNC2 ABI remains unsafe because C exposes no buffer lengths. Existing host tests cover RNC1 error cases and empty/raw RNC2 samples; they do not cover a broad compressed-file corpus.

## Validation boundary

`cargo test -p ogc-core --lib` passed all 11 tests. `cargo test -p ogc-core` without `--lib` fails because the crate intentionally rejects non-PowerPC production builds. No hardware verification is recorded.
