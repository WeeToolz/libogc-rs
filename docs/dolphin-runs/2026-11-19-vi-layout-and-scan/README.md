# VI framebuffer layout and scan mode evidence

Dolphin 2503 on Linux ran `video_probe` with **69 passes and zero failures**
and six valid captured pages. The [transcript](checks.txt) records the check
results, and the [layout page](layout-checks.png) shows the sizing and clear checks.

The DOL SHA-256 was
`705307a3d6dafa732a6ab25257c55d58dad8f97360f4fd2811ed492ddbced8eb`.
The DOL was built from `bfd76dc3e978` with the probe changes in this commit.

The probe calls both framebuffer size APIs with a 17-pixel-wide,
one-row mode. It expects 64 bytes with anti-aliasing disabled, 320 bytes with
anti-aliasing enabled, and zero for a null mode. It then calls the sys clear
function on an 80-word buffer with a guard word at each end. The check
compares every word against the requested color and verifies both guards.

After setting up VI, the probe compares the wrapper and sys scan mode results
separately to the scan selector in the configured render mode. The run covers
one configured mode. Transitions between scan modes and physical console
behavior remain open.

Build with `cargo xtask build-dolphin-probes` and run
`cargo xtask dolphin-validate --no-build --probe video_probe --timeout-seconds 300`
with Dolphin, a display, and a configured NAND. This run used a private copy
of an imported NAND and software OpenGL.
