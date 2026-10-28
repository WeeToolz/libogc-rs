# TinySMB outgoing UTF-8 conversion fix handoff

Implementer: `/root/worker_b`. Independent reviewer: `/root/reviewer_5` (re-review pending).

## BUG FIX

C `utf8_to_utf16` uses `mbrtowc`, writes the low 16 bits of each decoded `wchar_t` as one little-endian unit, and appends a two-byte zero unit. It stops before a decoded unit when the `SMB_MAXPATH - 2` byte limit is reached. Rust previously emitted every source byte as a separate UTF-16 unit.

Rust now decodes complete UTF-8 scalars and writes one little-endian unit per scalar. It preserves C's low-16-bit output for supplementary code points, truncates only between complete units, and appends the terminator. Invalid and incomplete sequences write the same zero unit at the failure position; the Rust callers then return `SMB_ERROR` instead of adding C's `(size_t)-1` error result to the packet offset. This avoids the C error-path offset wrap. A destination slice reserves space for the terminator and bounds output safely.

Regression cases cover ASCII, BMP non-ASCII, supplementary scalars, invalid input, incomplete input, complete-unit truncation, and too-small output.

## Validation

The PowerPC target `cargo check -p tinysmb-sys` passed with `RUSTFLAGS="-D warnings"`; `rustfmt --check` passed for `powerpc_backend.rs`. The target test build could not compile because `powerpc-nintendo-none` does not provide `std` or the `test` crate, so tests did not run. No hardware verification was performed.
