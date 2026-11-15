# GX indexed TPL and frame configuration evidence

The Dolphin 2503 `video_probe` run on Linux completed with **60 passes and zero
failures**, and six valid captured pages. The [screenshot](checks.png) shows the
new checks; the [transcript](checks.txt) records the full set. DOL SHA-256:
`091d33c6d8663da8e64da7d4de16267f4a7e7a23bc3f4e8170d5b32802ee0128`.
The source was based on `4d0071f62211` with the probe change in the commit
containing this record.

A fixed memory-backed TPL fixture holds an aligned 8×4 CI8 tile and a 16-entry
RGB565 palette. The probe verifies `tpl_get_texture_ci` rejects an invalid
index and null palette output, then checks texture dimensions, format, palette
index and data pointer, plus palette format, count and data pointer. It closes
the backing descriptor afterward. The resulting objects are inspected, not
submitted to GX.

Separate checks exercise `FrameConfig::new` with a valid aligned pointer and
reject null, unaligned, and zero-width inputs. They verify that
`FrameConfig::with_copy_filter` and `FrameConfig::with_field_mode` preserve the
provided settings and framebuffer pointer. No framebuffer copy is submitted.
These checks cover the stated cases rather than every input or rendering path.

Build with `cargo xtask build-dolphin-probes` and run `cargo xtask
dolphin-validate --no-build --probe video_probe --timeout-seconds 600` with a
configured NAND, Dolphin, and display. The run used software OpenGL and a
private NAND copy. The screenshot captures the probe transcript, not an
independent rendered-pixel comparison. The result is Dolphin evidence for the
named calls and does not establish untested hardware behavior.
