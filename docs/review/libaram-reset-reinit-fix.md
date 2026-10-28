# ARAM reset and reinitialization fix handoff

Implementer: `/root/worker_b`.

## BUG FIX

C `AR_Reset` clears the initialization flag. A later `AR_Init` then registers `__ARHandler` again and continues initialization. Rust reset also clears only its initialization flag, but Rust init rejected every existing DSP-ARAM IRQ handler. That included the same `handler` installed by an earlier Rust init, so reset followed by init failed.

Rust init now accepts the handler when the existing DSP-ARAM IRQ owner is its own `handler`. It continues to reject a different handler and retains the existing check after registration. Regression coverage checks the helper's behavior for no handler, the ARAM handler, and a different handler.

## Validation

The PowerPC target `cargo check -p ogc-aram` passed with `RUSTFLAGS="-D warnings"`. The target test build could not compile because the custom `powerpc-nintendo-none` target does not provide the `test` crate, so the regression test did not run. No hardware verification was performed.
