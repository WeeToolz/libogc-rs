# ABI probe static review

Reviewer: `/root/reviewer_5`  
Implementer: none (first-pass review)  
Scope: `abi-probe`, the PowerPC object-section probe for the `MOD_INSTR`, `MOD`, `MODSNDBUF`, and `MODPlay` Rust layouts.

## Result

No confirmed mismatch was found in the probe's declared layout surface. The C declarations in `../../C-sources/libogc/gc/modplay/modplay.h:40-126` and `../../C-sources/libogc/gc/gcmodplay.h:11-26` have the same field order and corresponding scalar, array, pointer, and callback types as Rust's `ModInstr`, `Mod`, `ModSndBuf`, and `ModPlay` (`crates/abi/modplay-capi/src/lib.rs:108-209`). The C `BOOL` typedef is `unsigned int` (`../../C-sources/libogc/gc/gcbool.h:3`); Rust aliases it as `u32` (`crates/abi/ogc-sys/src/lib.rs:368`). Rust `#[repr(C)]` declarations retain the C field ordering and target C layout rules.

The probe measures `size_of`, `align_of`, and every field offset for these four structures. Its 96 `LayoutEntry` values and section placement are in `crates/abi/abi-probe/src/lib.rs:11-158`. The target snapshot records 32-bit PowerPC values (`tests/abi/modplay-rust-layout-powerpc-nintendo-none.txt:4-99`): `MOD_INSTR` size 52/alignment 4, `MOD` 3112/4, `MODSNDBUF` 24/4, and `MODPlay` 3164/4, with field offsets following the C declarations. The values produced by `cargo xtask check-abi-rust-target` matched this snapshot. This command compiled the PowerPC-target dependencies and extracted the Rust object section; no tests ran.

## Evidence and limits

- C structure fields and order: `../../C-sources/libogc/gc/modplay/modplay.h:40-126` and `../../C-sources/libogc/gc/gcmodplay.h:11-26`.
- C scalar aliases: `../../C-sources/libogc/gc/gcbool.h:3`; standard libogc integer and float aliases come from `gctypes.h`, included by both C headers.
- Rust ABI declarations: `crates/abi/modplay-capi/src/lib.rs:108-209`; `Bool` alias: `crates/abi/ogc-sys/src/lib.rs:368`.
- Probe generation and extraction: `crates/abi/abi-probe/src/lib.rs:11-158`; decoding and the PowerPC-target build in `xtask/src/main.rs:192-246,260-307`.
- The repository contains no C-side compiled layout probe or checked-in C layout snapshot. The static field mapping and Rust target output support parity, but an exact comparison against the selected devkitPPC C compiler's `sizeof`/`offsetof` output is not established here. Hardware verification was not performed.

## Validation

`cargo xtask check-abi-rust-target` completed successfully. It compiled the PowerPC target dependencies and extracted the Rust `.layout` section. Tests were not run, and this result is not a C-compiler ABI comparison.
