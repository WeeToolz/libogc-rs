# `ThreadId` C sentinel fix handoff

Implementer: `/root/worker_b`.

## BUG FIX

`ThreadId::new` now returns `None` for `u32::MAX`, the C `LWP_THREAD_NULL` sentinel. Its `NonZeroU32` input already excludes zero. This prevents the safe public constructor from creating a value that C treats as an invalid thread handle. Repository search found no call sites beyond the type definition and export, so the constructor's new `Option<Self>` result does not require caller updates in this workspace.

Regression coverage verifies rejection of `u32::MAX`, acceptance of a valid ID, and that zero cannot be represented by the constructor input.

## Validation

The PowerPC target `cargo check -p ogc-runtime` passed with `RUSTFLAGS="-D warnings"`. The target test build could not compile because the custom `powerpc-nintendo-none` target does not provide the `test` crate, so the regression test did not run. No hardware verification was performed.
