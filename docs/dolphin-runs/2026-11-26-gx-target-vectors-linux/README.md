# GX copy-scale and FIFO vectors on the PowerPC target

Stock graphical Dolphin 2609b, OpenGL, private stock-menu NAND. The
production GX crate requires PowerPC dependencies and is not a host-test
package. The standalone `ogc-gx-host-tests` crate tests isolated arithmetic
helpers; it is supplemental evidence, not a replacement for runtime tests.

The focused GX probe now exercises those five helper-test groups through
public production APIs: invalid dimensions and fallback, valid scale
ratios, exact scale BP commands and resulting XFB line counts, copy
control scale-enable bits, frame configuration rejection, and normal and
wrapped FIFO metadata with invalid-pointer rejection. The command test
captures actual GX writes in a display list without executing the copy.
The FIFO metadata check does not claim a physical hardware wrap.

The first focused run passed 77 of 78 checks. The new third copy command
case incorrectly assumed that a previous display-list capture retained
its shadow configuration. The final case configures its own fields before
capture, as the previous cases do. The production code did not change.
The repeated focused run passed all 78 checks and all eight framebuffer
captures validated. See `summary.json` and the filtered check logs.

The broader video repeat was interrupted at capture page 17 without a
final report. It is not fresh passing evidence. The private archive retains
both focused reports and the incomplete video artifacts, excluding user
profiles and NAND. Inventory check passed (3,419 entries).

This is scoped target coverage of the standalone helper vectors, not an
assertion that every target-only unit test has a one-to-one Dolphin case.
The GX review status is unchanged. Physical Wii timing and FIFO status
clearing remain pending.
