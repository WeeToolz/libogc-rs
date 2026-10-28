# EasyCap pattern DOL

The source, tests, Homebrew Channel metadata, and this guide live in
`probes/easycap-patterns/`. Generated DOLs and reference images stay under
`target/`. This is a standalone Cargo workspace, like the host action test
packages, so it can export references on the host and build for Wii through
`xtask build-dol`. The app uses production Rust video and PAD APIs. It forces
NTSC 480i with a 640x480 source framebuffer, which the capture card samples
into 720x480. It does not change stored console settings.

## Run

Load `target/easycap-diagnostics/boot.dol` through the existing Wiiload upload
workflow or Homebrew Channel.
The eight pages cycle automatically every 600 displayed retraces (about ten
seconds at full rate). Rendering load can make the cycle slower.

GameCube controller controls:

- A or Right: next page; disable automatic cycling.
- B or Left: previous page; disable automatic cycling.
- Start: toggle automatic cycling.
- X: hold or release the animation counter and automatic page timer.

No controller is required for automatic cycling. Wii Remote input and audio
output are not implemented in this local diagnostic. The last digits on screen
show the software presentation counter, not a measured capture frame number.

## Patterns

| Page | Purpose |
| --- | --- |
| Geometry | Outer border, inset guides, 40-pixel grid, and center cross for crop, alignment, and scale. |
| Luma levels | Fixed values below black, at black, above black, and near white; limited and full-range ramps. |
| 75 percent bars | Explicit BT.601 limited-range YCbCr values for white, yellow, cyan, green, magenta, red, blue, and black. These are diagnostic bars, not a complete broadcast certification pattern. |
| Luma detail | Vertical and horizontal stripes at widths of 1, 2, 4, 8, and 16 source pixels. |
| Chroma edges | Constant Y=128, with U or V alternating between 64 and 192 at several widths. |
| Row parity and motion | Alternating source rows and a moving white bar to inspect weave, bob, combing, and temporal behavior. |
| Flat fields | Neutral Y=16, 64, 128, and 235 regions to inspect noise, clipping, and brightness variation. |
| Frame and field ID | Large binary presentation and VI retrace markers in separate even-row and odd-row bands. |

Do not use the labels as measurement regions. The image reference format file
lists their positions and the exact luma steps. Source row parity alone cannot
identify the order in which the capture card received the two fields.

## Frame and field identification

Page 7, `FRAME AND FIELD ID`, has two marker panels. The upper panel carries
data only on even source rows; the lower panel carries data only on odd source
rows. The other row parity has neutral Y=128. Each panel has three 32-bit words:

1. The software presentation ID shown in the footer.
2. The bitwise inverse of that ID. Reject a decoded panel if the two words
   do not XOR to `0xffffffff`.
3. The actual VI retrace counter sampled before rendering that source image.

Bits run from most significant to least significant, left to right. Each cell
is 16 source pixels wide. Zero has Y=16 and one has Y=235; chroma stays neutral.
White guard blocks and black separators bracket each band. The generated
`target/easycap-diagnostics/references/identification.json` records the exact
coordinates, row bands, byte order, levels, and format version for a decoder.
All eight reference images use presentation and VI snapshot values of zero.

For EasyCap analysis, use raw original-scan frames before scaling, filtering,
or deinterlacing. First align the source image to the 720x480 capture using
the geometry pattern. Decode each source row parity separately. Use guard
and separator samples to assess levels; the analog path can change the
source's exact luma values. Preserve the original samples when decoding fails.

Across captured images, compare IDs and VI snapshots to investigate sequence
gaps, repeated images, and field pairing. A captured pair can contain different
IDs because its two fields can come from different source presentations.
The application can also leave one source image on screen for several VI
retraces. A repeated ID alone therefore does not prove a USB duplicate, and
an ID jump alone does not locate the stage that lost data. This page labels
source row parity; it does not claim a measured top-field-first or
bottom-field-first capture order, nor a unique ID for every physical field.

X freezes both the presentation ID and VI snapshot. The hardware retrace
counter continues to run. The page deliberately retains the sampled value
while held so reference images remain stable. The title states NTSC 480i;
the app cannot detect whether the analog cable is composite or S-Video.

## Capture and image correction

Start with neutral EasyCap software corrections and original scan mode. Save
raw UYVY frames as well as images. Use a held static page for color, brightness,
crop, and detail measurements; release Hold on the motion page for interlace
checks. Keep the original capture before applying any correction.

1. Use geometry to identify the active picture and measure cropping and scaling.
2. Use luma steps to check black and white levels without hiding clipping.
3. Use bars and chroma edges to inspect hue, saturation, chroma displacement,
   and color fringes.
4. Use detail and flat fields to inspect softness, ringing, noise, and banding.
5. Use motion to compare original, bob, and motion processing separately.

These patterns supply evidence for a correction. They do not automatically
clean an image or establish a universal correction for every source. Wii analog
encoding, cable type, decoder settings, and capture sampling all affect results.
Avoid assuming that every source pixel maps to one capture pixel.

## References and rebuild

`target/easycap-diagnostics/references/` contains exact source YUYV bytes and
RGB PPM previews for all eight pages at counter zero. Source byte order is
Y0 U Y1 V. EasyCap raw frames use
U Y0 V Y1. Align the active picture and account for sampling before comparing.
PPM previews clip out-of-range RGB; use the raw bytes for level checks.

Run from the `libogc-rs` repository root:

```sh
cargo test --offline --manifest-path probes/easycap-patterns/Cargo.toml
cargo run --offline --manifest-path probes/easycap-patterns/Cargo.toml -- \
  --export target/easycap-diagnostics/references
cargo xtask build-dol easycap-patterns --bin easycap-patterns \
  --manifest-path probes/easycap-patterns/Cargo.toml \
  --output target/easycap-diagnostics/boot.dol
python3 scripts/capture-example-dolphin.py \
  --example easycap-patterns --dol target/easycap-diagnostics/boot.dol \
  --elf target/xtask/custom-dol-build/powerpc-nintendo-none/release/easycap-patterns \
  --warmup-seconds 3 --idle-seconds 0
```

The shared Dolphin capture script includes a diagnostic page sequence. It
checks source XFB rows against the exported references and checks that Hold
freezes the complete framebuffer. Results and PNG captures go
to `target/dolphin-example-easycap-patterns/`. This checks software rendering
and input in Dolphin; it does not check the physical analog capture path.
For the identification page, it also decodes both parity panels, checks the
inverted IDs, and confirms that presentation IDs and real VI snapshots advance.
Its decoded observations are saved in the run's `identification.json`.

No physical Wii/EasyCap run has been made with this new DOL yet.

For Homebrew Channel, copy the generated `boot.dol` and this directory's
`meta.xml` into the same `apps/easycap-patterns/` directory on the SD card.
