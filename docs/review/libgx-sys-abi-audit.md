# GX sys ABI surface audit

Reviewer: `/root/reviewer_4`  
Date: 2026-10-01  
Scope: static comparison of `ogc-gx-sys` public ABI declarations with the C GX headers, excluding TPL behavior. No implementation changes, builds, tests, or hardware checks were performed. The shared `libgx.md` note and review ledger were not edited.

## Layouts present in Rust

The C declarations in `C-sources/libogc/gc/ogc/gx.h:1169-1323` and `gc/ogc/gx_struct.h:58-72` were compared with `crates/hardware/ogc-gx-sys/src/lib.rs:105-208`. For the types Rust declares, the field sequence and primitive widths match the corresponding C layout: `GXColor`/`GxColor`, `GXColorS10`/`GxColorS10`, `GXVtxDesc`/`GxVtxDesc` (C field `type`, Rust field `ty`), `GXVtxAttrFmt`, `GXFogAdjTbl`, `GXFifoObj`, `GXTexObj`, `GXTlutObj`, `GXTexRegion`, `GXTlutRegion`, `GXLightObj`, `Vtx`, and `WGPipe`. The different Rust field spelling for the vertex input type does not change the C layout. Callback signatures also correspond to C at `gx.h:1335-1353`.

The test at `ogc-gx-sys/src/lib.rs:218-226` checks four object sizes only. This is not a layout discrepancy by itself, but the source has no size or offset assertions for the other declared types.

## C ABI declarations absent from ogc-gx-sys

The Rust sys crate has no `extern` function declarations and no exported function definitions (`rg '^pub (unsafe )?fn' crates/hardware/ogc-gx-sys/src/lib.rs` returns none). The C public header declares the GX API, starting with `GX_Init` and FIFO functions at `gx.h:1378-1565`, followed by vertex, matrix, texture, TEV, lighting, copy, and other APIs throughout the header. The separate high-level Rust crate `ogc-gx` defines Rust functions, but they are not raw C ABI declarations in `ogc-gx-sys`; its 105 public functions also do not constitute complete coverage of the C header. For example, C declares `GX_Init` and `GX_InitFifoBase`, while neither name is declared in the sys crate and the Rust facade has no corresponding public function (`rg` against `ogc-gx/src/lib.rs`).

This is a clear difference in the public raw-binding surface if `ogc-gx-sys` is intended to represent the C GX header. If the crate is intentionally only a selected type/constants support crate for the independently implemented safe facade, that intent should be documented; the current crate has no module-level scope statement that identifies this as a partial ABI surface.

## Additional omitted declarations and constants

C `gx.h` declares `GXTexReg` with a four-byte `dummy` field at lines 1314-1316, but `ogc-gx-sys` has no corresponding type. C also declares the `wgPipe` global at line 1353, while the Rust sys crate has no corresponding static. `GXRModeObj` is declared in `gx_struct.h:58-72` and used by GX declarations such as `GX_AdjustForOverscan` (`gx.h:2359`); it is absent from `ogc-gx-sys`.

The sys crate exports a selected subset of GX constants. C header examples with no matching Rust `GX_*` constant include channel identifiers `GX_COLOR0`/`GX_ALPHA0` (`gx.h:44-52`), vertex formats `GX_VTXFMT1` and `GX_MAXVTXFMT` (`:67-74`), vertex input modes `GX_NONE`, `GX_DIRECT`, and `GX_INDEX8` (`:82-85`), vertex attributes including `GX_VA_NULL` (`:130-150`), and `GX_MAX_Z24` (`:1163`). These examples show omission rather than a value mismatch; no wrong value was found among the overlapping constants inspected.

## Result

No field-order or primitive-width mismatch was found among the C structs/unions that `ogc-gx-sys` does declare. The main parity issue is surface coverage: the crate omits many public C GX declarations, including all C function bindings, several types/globals, and many constants. The evidence establishes incompleteness relative to `gx.h`; whether each omission is a defect depends on the intended contract of this support crate. No tests were run or compiled, and no hardware verification was performed.
