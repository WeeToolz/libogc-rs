# Network `SockLenT` fix re-review

- Implementer: `worker_b`, commit `182dea9`
- Independent reviewer: `/root`
- C declaration: `../../C-sources/libogc/gc/network.h:107-108,152-166`
- Rust declarations: `crates/services/ogc-network-sys/src/lib.rs` and `crates/services/ogc-network/src/lib.rs`

The C header defines `socklen_t` as `u32`. Rust now exposes `SockLenT = u32` in both network crates, and the affected public signatures use that alias for scalar lengths and length pointers. This fixes the public type mismatch. The regression assertions cover the unsigned range and function signatures.

In the Wii Rust backend, scalar values that do not fit its signed lwIP interface return `-EINVAL`; output length pointers are cast between equal-width signed and unsigned integers. This is a checked Rust boundary. The C-facing API type and 32-bit pointer layout match; the behavior for oversized lengths remains documented as a defensive Rust behavior change.

I found no remaining mismatch in the public type/layout fix. The independent review does not accept the network crate as complete: `net_init_async` remains synchronous in Rust, and other lifecycle questions remain in `docs/review/libnetwork.md`.

The PowerPC `ogc-network-sys` test code compiled, but the test binary did not run because the host linker cannot link PowerPC output. The high-level test build also has unrelated pre-existing errors. No hardware verification was performed.
