# libgu Review

Date: 2026-10-01

Status: static review of the previously identified GU math paths complete; no confirmed mismatch found in that scope.

## Scope

- `crates/hardware/ogc-gu`
- `crates/hardware/ogc-gu-sys`
- C reference: `C-sources/libogc/libogc/gu.c` and `C-sources/libogc/gc/ogc/gu.h`

## Reviewed behavior

- Projection matrix formulas for frustum, perspective, and orthographic views.
- Vector arithmetic, normalization, multiplication, and dot/cross products.
- Quaternion arithmetic, normalization, inverse, and matrix conversion.
- Affine matrix inverse and the in-place alias cases that were fixed in prior commits.
- The seven scalar helpers in `ogc-gu-sys`: max, min, degree/radian conversion, fixed-point conversion, and fractional conversion.

The reviewed formulas, constants, output ordering, and supported alias behavior match the corresponding C implementations. I found no confirmed remaining bug in these paths. This note covers the listed paths, not every function in `ogc-gu`.

## Validation boundary

This was a static comparison with the checked-in C source and header. It does not establish hardware verification. No hardware verification is recorded by this review.

## Runnable coverage update — 2026-10-15

The completeness audit lacked exact evidence for all 45 public `ogc-gu` APIs.
Its previous unit tests were limited to PowerPC test builds. The two production
crates now keep the math in shared `src/math.rs` modules. Their public exports
and PowerPC-only crate guards remain in place. The standalone host test package
compiles those same modules with the real `libm` dependency. It has no hardware
model or replacement calculations. `cargo xtask test-host` selects this package.

The 21 passing tests include the 10 existing unit tests and 11 new tests. They
check fixed matrix and vector results, projection scale and offsets, camera
translation, reflection, quaternion conversion and zero inputs, axis case and
normalization, distinct and in-place outputs, and singular inverse return codes.
A dense 4-by-4 inverse uses fixed rational values from independent row reduction.
The axis selector now converts `c_char` to `u8` before matching ASCII values,
so the shared source also compiles where C `char` is signed.

Exact evidence records cover the 45 GU APIs and retain the existing seven
scalar records at their new source locations. No classification, threshold,
or audit allowance changed. The generated inventory has 3,271 APIs, with 2,364
unverified entries. The previous count was 2,409; the older std baseline was
2,416. The full completeness audit still fails for the remaining entries.

Validation:

- `RUSTFLAGS='--check-cfg=cfg(gamecube) -D warnings' cargo test --offline --manifest-path tests/ogc-gu-host-tests/Cargo.toml`: 21 passed.
- `cargo test --offline -p xtask`: 34 passed, including test package selection.
- PowerPC `cargo check` for `ogc-gu` and `ogc-gu-sys`: passed with nightly and the repository target, using `build-std=core,alloc`.
- `cargo xtask inventory` and `cargo xtask check-inventory`: passed.
- `cargo xtask audit-source`: passed with the three existing keyboard FIFO debt entries.
- `cargo xtask audit-stubs`: still fails for the 2,364 remaining unverified APIs.

This evidence covers the tested math contracts. It does not establish complete
input-space parity or real-Wii execution. No new hardware probe was run.

## C reference check — 2026-10-15

The local C reference is at commit
`ff53c4a7b7ba5587d82305e710ca65bce1092e1c`, with no local changes.
The last commit that changes `libogc/gu.c` or `gc/ogc/gu.h` is
`0ea72783a3792ed7d9fa2571f8f2f66676467099` (2024-05-03).
Thus the new C commits do not change the GU reference used by these tests.

SHA-256 of the inspected files:

- `libogc/gu.c`: `f2d282ac6744c8f5520f94e22554aeadfe669ea442c0596e0d424770380f1fcd`
- `gc/ogc/gu.h`: `9c331a32cb35953db383e5e5b6fe7830a6fbe0c2a39ac3f9a9118eb17a942d54`

The Rust completeness count does not measure changes to the C reference.
New C APIs and changed contracts need a separate source comparison through
the existing parity ledger. They can require port work without increasing the
Rust inventory count.
