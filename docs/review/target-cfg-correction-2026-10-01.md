# Target cfg correction — 2026-10-01

A full `-D warnings` compile rejected the earlier `target_env = "gamecube"` predicates as unexpected cfg values. `gamecube` is not a supported Rust `target_env` value.

The target split now uses one repository-defined `cfg(gamecube)` switch. `.cargo/config.toml` registers that cfg for all supported build paths and enables it only for `powerpc-nintendo-gamecube`. Both Wii and GameCube target JSON files use the valid `eabi` environment.

The same switch is reused by `ogc-irq-sys`, `ogc-irq`, `ogc-stm`, and the compatibility `ogc` facade. This avoids separate platform tests in each crate.

Cargo does not use rustc `--cfg` flags to select `[target.'cfg(...)'.dependencies]`, so `ogc-stm` is now an unconditional facade dependency. On GameCube the `ogc-stm` crate is compiled empty with `#![cfg(not(gamecube))]`, and the facade does not re-export it. This preserves the C-facing Wii-only STM surface without duplicating target-selection logic in Cargo.toml.

Validation in this environment is static only because the Rust toolchain is unavailable. The compile-capable agent should run the full `-D warnings` runtime check for both console targets.
