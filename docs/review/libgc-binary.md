# `gc-binary` independent source audit

- Reviewer: `/root/reviewer_5`
- Implementer: none; Rust helper crate added with GameCube tool extraction in commit `530d086`
- Scope: public Rust functions in `crates/tools/gc-binary/src/lib.rs`; search the checked-in `../../C-sources/libogc` and `../../C-sources/gamecube-tools` trees for corresponding APIs and primitive behavior.
- Validation: static source inspection only. No build, tests, or hardware verification were performed.

## C counterpart and public API scope

No `gc-binary` counterpart or shared C parsing API exists in the checked-in C trees. The crate exposes bounds-checked byte-slice range, big-endian 16/32-bit read/write, and alignment helpers (`gc-binary/src/lib.rs:26-80`); its Cargo manifest declares only the standalone `gc-binary` package (`crates/tools/gc-binary/Cargo.toml:1-7`). The repository references it from `gc-elf` (`crates/tools/gc-elf/Cargo.toml:8`), where it supports the Rust converter. It does not expose libogc C symbols or replace a corresponding C library API.

There are private/conceptual C equivalents in the ELF converter, not public APIs for the crate. `elf2dol.c` defines `swap16` and `swap32` for reading and writing big-endian ELF/DOL fields (`../../C-sources/gamecube-tools/elftool/elf2dol.c:68-88`), and a fixed 64-byte `DOL_ALIGN` macro (`:109-111`). In libogc, `gc/ogc/machine/processor.h` has scalar byte-swap helpers (`:101-114`) and byte-reversed loads/stores (`:58-75`); these do not provide slice range checking or a shared binary parser API.

## Static behavior comparison

For valid in-range input, Rust `read_u16_be`/`read_u32_be` return the same big-endian numeric fields as the C converter's byte-swapped ELF reads, and Rust writes the corresponding big-endian bytes (`gc-binary/src/lib.rs:46-71`; C `swap16`/`swap32` at `elf2dol.c:68-88`). Rust `align_up(value, 64)` yields the same result as C `DOL_ALIGN(value)` when the arithmetic does not overflow (`gc-binary/src/lib.rs:73-80`; `elf2dol.c:109-111`).

The checked range and checked arithmetic errors have no direct C API behavior to compare. Rust rejects out-of-bounds ranges, arithmetic overflow, invalid alignments, and alignment overflow. C's private fixed-alignment macro does not validate overflow. These are Rust safety properties, not mismatches in a corresponding public C contract.

## Result

No confirmed defect or public C/Rust API mismatch was found in the recorded `gc-binary` scope. Mark this Rust-only helper crate reviewed; this does not certify `gc-elf` converter parity, which remains separately open in `docs/review/libgc-elf.md`.
