# GX texture fields and file-backed TPL target evidence

Date: 2026-11-26. Local base: `bf50639e`. C reference:
`7d4b8b3e7b845516f0cc2a85274879ddd8cbf142`.
Implementer: ChatGPT (current parity work); independent reviewer pending.
Stock Dolphin 2609b, Linux x86_64, OpenGL, private Wii test profile.
No emulator changes. The local offline toolchain lock is not part of this fix.

## Source finding and correction

**BUG FIX:** C `GX_InitTexObj` (`libogc/gx.c:3226` onward) uses
`_SHIFTL(fmt,20,4)` for `tex_size`. Rust `init_tex_obj_from_init` formerly
shifted the entire format byte into that word. Mask the input to four bits;
keep the full format byte in the separate format field. The same target
regression fails before the change and passes afterward. The test covers
normal, depth and copy-format encodings and field truncation, not support
for arbitrary unsupported texture formats.

The six former host-only GX mappings now cite target texture-size and
field-vector functions. `tpl_open_tpl_from_file` now also has successful
file-backed runtime evidence. A controlled read-only devoptab device
provides TPL bytes in partial reads. The target checks header-only open,
metadata access without payload reads, deferred aligned image and CI
palette reads, retained buffers on repeated access, close and unregister.
This is a production DeviceFile path test, not a physical filesystem test.
Rust intentionally retains the loaded payload rather than rereading it.
C `TPL_OpenTPLFromFile`, `TPL_GetTexture` and `TPL_GetTextureCI` are in
`libogc/tpl.c:86-140,210-305`. The earlier malformed-input tests remain.

## Runtime results

| Stage | Probe | Pass | Fail | Valid capture pages |
| --- | --- | ---: | ---: | ---: |
| Before format-mask fix | gx_api_probe | 117 | 1 | 12 |
| After format-mask fix | gx_api_probe | 118 | 0 | 11 |
| Broad texture vectors | video_probe | 422 | 0 | 32 |
| Final with file fixture | gx_api_probe | 125 | 0 | 11 |
| Final broad with file fixture | video_probe | 429 | 0 | 33 |

See `summary.json` for DOL hashes and runner results. The filtered check
logs preserve the texture/TPL assertions and summaries. Earlier interrupted
capture runs are retained privately with the complete raw reports. No
private NAND, profile or console identifiers are in this evidence commit.

## Coverage boundary

All 278 GX inventory entries now have a named, reachable target probe
function (276 `video_probe`, two `gx_api_probe`). This maps the functions
and demonstrates the recorded cases, not every branch or argument domain.
The GameCube abort result remains in the preceding ownership report and
was not rerun here. Physical CP clearing, PE memory-counter timing and
other physical Wii evidence remain pending. Full runtime was not repeated.
Independent approval of the complete current GX implementation remains open.
