# GX active FIFO descriptor evidence

Dolphin 2503 on Linux ran `video_probe` with **64 passes and zero failures**
and six valid captured pages. The [transcript](checks.txt) records the check
results, and the [FIFO page](fifo-checks.png) shows the two new checks.

The DOL SHA-256 was
`5344e5c72c48e55cc4a73dcee6c7f62a42c52a74694b6838faa7bf6a1caea643`.
The DOL was built from `f0ab53ae155a` with the probe changes in this commit.

The probe initializes the linked CPU and GP FIFOs with the default 256 KiB
MEM2 buffer. It calls `ogc_gx::get_cpu_fifo` and `ogc_gx::get_gp_fifo` with
separate sentinel-filled output descriptors. Each check reads the base and
size at the C ABI byte offsets and compares them with the independently aligned
pre-allocation MEM2 address and the fixed default size.

The checks cover base and size for linked FIFOs in Dolphin. Separate CPU and
GP buffers, live pointer accounting, wrap and overflow behavior, and physical
console behavior remain unverified by this run.

Build with `cargo xtask build-dolphin-probes` and run
`cargo xtask dolphin-validate --no-build --probe video_probe --timeout-seconds 300`
with Dolphin, a display, and a configured NAND. This run used a private copy
of an imported NAND and software OpenGL.
