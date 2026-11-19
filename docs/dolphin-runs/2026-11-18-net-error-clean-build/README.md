# IOS network error conversion

The C reference `libogc/ogc_sockets/soc_common.c` maps negative IOS socket
results to negative newlib errno values and offsets unknown results by 10000.
The Rust backend previously returned every input unchanged. The red Dolphin
run in `../2026-11-18-net-error-red` failed the mapping and unknown-value
checks. This run uses the shared IOS errno table and passes those checks.
Nonnegative results are unchanged.

The Rust lookup rejects an out-of-range table index. The C code uses `sizeof`
with a strict comparison and can index beyond the last element at the boundary.
Rust uses wrapping subtraction for the unknown offset so that extreme inputs do
not overflow in a checked build. Those boundary cases and physical Wii behavior
were not tested in this Dolphin run.

The first rebuild after changing the Rust standard-library shared source failed
to find target crates. Moving the previous generated probe build directory aside
and rebuilding it resolved that build failure. The failed attempt is retained in
`../2026-11-18-net-error-green`.
