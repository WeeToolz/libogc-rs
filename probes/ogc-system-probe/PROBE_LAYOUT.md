# Probe directory layout

Each named probe binary has its own directory at `src/<binary-name>/main.rs`.
The crate `Cargo.toml` declares each binary with an explicit `[[bin]]` path.
Shared implementations and test utilities remain in `src/` and are linked through the `ogc_system_probe` library.
The other probe crates remain independent under `probes/`.

Run `cargo check -p ogc-system-probe --bins` from the workspace root with a compatible toolchain to verify compilation.
