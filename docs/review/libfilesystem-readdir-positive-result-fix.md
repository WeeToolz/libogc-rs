# Async `read_dir` positive-result fix handoff

Implementer: `/root/worker_b`.

## BUG FIX

The async `read_dir` completion now copies its entry count only when the result is exactly `ISFS_OK` (zero). This matches C `__isfsReadDirCB`, which leaves the caller's count unchanged for positive nonzero results. The user callback still receives the original result.

Regression coverage checks that a positive result preserves the caller's prior count and a zero result copies the new count.

## Validation

The PowerPC target `cargo check -p ogc-filesystem` passed with `RUSTFLAGS="-D warnings"`. The target test build reached `ogc-filesystem`, but this toolchain does not provide the `test` crate for the custom `powerpc-nintendo-none` target, so the tests could not compile or run. `cargo fmt --check` reports pre-existing formatting differences across the workspace, including in this crate outside this change; I did not apply workspace-wide formatting. No hardware verification was performed.
