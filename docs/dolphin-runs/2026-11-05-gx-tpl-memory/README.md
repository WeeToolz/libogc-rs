# GX memory-backed TPL evidence

Dolphin 2503 on Linux ran `video_probe` with **57 passes, zero failures** and
six valid screen captures. The [TPL screenshot](tpl-checks.png) shows all five
new checks; the [transcript](checks.txt) lists the probe's checks. The DOL
SHA-256 was `fab04c90caf1fdf4e8e6d9e3537c149e85ad64b5a3a0ceed8a04c39ba67ca87d`.
The source was based on `700afd4979dd` with the probe change in the commit
containing this record.

The fixed writable TPL fixture contains one aligned 8×8 I4 texture. The probe
checks that `tpl_open_tpl_from_memory` rejects a null input and opens the image;
`tpl_get_texture_info` returns the fixed format and dimensions and rejects an
invalid index; `tpl_get_texture` creates a matching texture descriptor and
rejects a negative index; and `tpl_close_tpl_file` clears the descriptor.
The descriptor is inspected without submitting texture data to GX. These
checks cover only that memory-backed path, not file-backed or indexed palettes.

Reproduce with `cargo xtask build-dolphin-probes` followed by `cargo xtask
dolphin-validate --no-build --probe video_probe --timeout-seconds 600` with a
configured NAND, Dolphin, and display. The run used software OpenGL and a
private NAND copy. The screenshot is probe output rather than an independent
rendered-pixel comparison. The evidence records Dolphin behavior for the
named calls and does not establish untested hardware behavior.
