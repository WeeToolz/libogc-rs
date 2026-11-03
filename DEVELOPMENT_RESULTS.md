# Development results

## 2026-11-02: GX perspective fog table in Dolphin

The video probe checks ten fixed perspective fog adjustment values and the
null input handling. The DOL ran in Dolphin 2609a with 50 passes and no
failures. Its SHA-256 was
`fa5b0236ab1b8c3afae18ae01bd9f3c1f44ebc274ac854f924450eacca112518`.
The build used revision `1541644` and the probe change in this batch. The
runner used a private profile and copied Wii NAND. HBC was not under test.
The [report](docs/dolphin-runs/2026-11-02-gx-fog-perspective/suite.json)
records the run. Fog rendering and physical Wii behavior remain untested.

Build with `cargo xtask build-dol ogc-system-probe --bin video_probe
--probe-features dolphin-automation --no-dev --output
target/xtask/probes-dolphin/video_probe.dol`. Run with `cargo xtask
dolphin-validate --no-build --probe video_probe --artifact-dir
docs/dolphin-runs/2026-11-02-gx-fog-perspective`.

## 2026-11-02: GX fog adjustment table in Dolphin

The video probe checks ten fixed orthographic fog adjustment values and
verifies that zero width leaves the table unchanged. The DOL ran in Dolphin
2609a with 49 passes and no failures. Its SHA-256 was
`fbe17609fccbf7b711f56ceff9803f3e929168c0e3a964022b417e73f27c61be`.
The build used revision `ce2ef2c` and the probe change in this batch. The
runner used a private profile and copied Wii NAND. HBC was not under test.
The [report](docs/dolphin-runs/2026-11-02-gx-fog/suite.json) records the run.
The check examines the table. Fog rendering and physical Wii behavior remain
untested.

Build with `cargo xtask build-dol ogc-system-probe --bin video_probe
--probe-features dolphin-automation --no-dev --output
target/xtask/probes-dolphin/video_probe.dol`. Run with `cargo xtask
dolphin-validate --no-build --probe video_probe --artifact-dir
docs/dolphin-runs/2026-11-02-gx-fog`.

## 2026-11-02: Wii-target probe Clippy cleanup

The Wii-target `ogc-system-probe` library and its dependencies passed Clippy
with `-D warnings` and the `dolphin-automation` feature. The host default
packages also passed Clippy with `-D warnings`, formatting, and their tests.
The USB lifecycle host suite passed 21 tests. The source audit passed with
three existing tracked debt entries. The completeness audit reported 2,218
APIs without accepted exact evidence.

The cleanup changes two Rust API signatures. USB bulk `begin` now returns
`PendingCompletion` instead of `()` for a pending completion. Diagnostic
`SiFaultStep::new` now accepts the payload length and bytes as a tuple.
Callers must update these arguments and error types. Other changes preserve
the control flow and the arena probe's direct evidence calls.

The [arena probe](docs/dolphin-runs/2026-11-02-arena-lint/suite.json) ran in
Dolphin 2609a with 11 passes and no failures. Its DOL SHA-256 was
`ca62557b6ae49e513070be93d8b3608d28975a6171b921e412685ab17b3e24f4`.
The [PAD state probe](docs/dolphin-runs/2026-11-02-pad-clippy/suite.json) ran
with 61 passes and no failures. Its DOL SHA-256 was
`c2f87f0666893db690bb499ae4c590e64a8fa98887a6db7217a0b29c8c4a55ea`.
Both used private profiles, copied Wii NAND, revision `14400cd`, and the
uncommitted changes in this batch. HBC was not under test. These runs do not
establish physical Wii behavior.

## 2026-11-02: GX spotlight modes in Dolphin

The video probe checks the cosine, cosine-squared, sharp, and two ring
spotlight modes at 90 degrees against fixed coefficients. The DOL ran in
Dolphin 2609a with 48 passes and no failures. Its SHA-256 was
`47f48cf99d9423d530d3c5250f6a5a19624114a1777216312761c61e42ce832a`.
The build used revision `cdf9fc1` and the probe change in this batch. The
runner used a private profile and copied Wii NAND. HBC was not under test.
The [report](docs/dolphin-runs/2026-11-02-gx-spot-modes/suite.json) records
the run. The checks inspect an in-memory light object. Rendered lighting and
physical Wii behavior remain untested.

Build with `cargo xtask build-dol ogc-system-probe --bin video_probe
--probe-features dolphin-automation --no-dev --output
target/xtask/probes-dolphin/video_probe.dol`. Run with `cargo xtask
dolphin-validate --no-build --probe video_probe --artifact-dir
docs/dolphin-runs/2026-11-02-gx-spot-modes`.

## 2026-11-02: GX overscan and probe lint

The video probe checks interlaced and progressive overscan, in-place output,
preservation of sample data, and rejection of zero source height. The DOL ran
in Dolphin 2609a with 47 passes and no failures. The final DOL SHA-256 was
`1293f7dbff4bfb958d1c5ab3aefa13c97d877134d0334c887e8f1e6852a05a3c`.
The build used revision `9c7a3cd` and the probe changes in this batch. The
runner used a private profile and copied Wii NAND. HBC was not under test.
See the [report](docs/dolphin-runs/2026-11-02-gx-overscan-lint/suite.json).
The earlier [run](docs/dolphin-runs/2026-11-02-gx-overscan/suite.json) also
passed. Physical Wii behavior remains untested.

Build with `cargo xtask build-dol ogc-system-probe --bin video_probe
--probe-features dolphin-automation --no-dev --output
target/xtask/probes-dolphin/video_probe.dol`. Run with `cargo xtask
dolphin-validate --no-build --probe video_probe --artifact-dir
docs/dolphin-runs/2026-11-02-gx-overscan-lint`.

## 2026-11-02: GX specular light in Dolphin

The video probe checks explicit and computed specular directions and the
zero half-angle case. The DOL ran in Dolphin 2609a with 46 passes and no
failures. Its SHA-256 was
`62e88fbcda7bbb4319ae1a1f442e47ae99b7aa72276b7c1623d80c522fc993c5`.
The build used revision `1702da0` and the probe change in this batch. The
runner used a private profile and copied Wii NAND. HBC was not under test.
The [report](docs/dolphin-runs/2026-11-02-gx-specular/suite.json) records the
run. The checks inspect an in-memory object. Rendered lighting and physical
Wii behavior remain untested.

Build with `cargo xtask build-dol ogc-system-probe --bin video_probe
--probe-features dolphin-automation --no-dev --output
target/xtask/probes-dolphin/video_probe.dol`. Run with `cargo xtask
dolphin-validate --no-build --probe video_probe --artifact-dir
docs/dolphin-runs/2026-11-02-gx-specular`.

## 2026-11-02: GX light attenuation in Dolphin

The video probe checks gentle, medium, and steep distance attenuation,
invalid distance, and flat spotlight cutoff and rejection. It checks an
in-memory light object. Rendered lighting and physical Wii behavior remain
untested. The DOL ran in Dolphin 2609a with 45 passes and no failures. Its
SHA-256 was `3aec8348add7ed5dab7bf90e47d8265e0b284e03b965e956f74c402c25c4caab`.
The build used revision `d287dd8` and the probe change in this batch. The
runner used a private profile and a copy of the configured Wii NAND. HBC was
not under test. The [report](docs/dolphin-runs/2026-11-02-gx-falloff/suite.json)
records the configuration and result.

Build with `cargo xtask build-dol ogc-system-probe --bin video_probe
--probe-features dolphin-automation --no-dev --output
target/xtask/probes-dolphin/video_probe.dol`. Run with `cargo xtask
dolphin-validate --no-build --probe video_probe --artifact-dir
docs/dolphin-runs/2026-11-02-gx-falloff`.

## 2026-11-02: GX light object in Dolphin

The video probe checks light position, negated direction, color, and both
attenuation coefficient groups. It verifies that updating attenuation preserves
position and direction. The DOL ran in Dolphin 2609a with 43 passes and no
failures. Its SHA-256 was
`fdad8f4853efe8d69e0c28808b11da4cd72aa85d821f00a5a97c6423206a376d`.
The build used revision `0109bb9` and the uncommitted probe change in this
batch. The runner used a private profile and a copy of the configured Wii
NAND. HBC was not under test. The [report](docs/dolphin-runs/2026-11-02-gx-light/suite.json)
contains the run configuration and results. These checks inspect an in-memory
object. Rendered lighting and physical Wii behavior remain untested.

Build with `cargo xtask build-dol ogc-system-probe --bin video_probe
--probe-features dolphin-automation --no-dev --output
target/xtask/probes-dolphin/video_probe.dol`. Run with `cargo xtask
dolphin-validate --no-build --probe video_probe --artifact-dir
docs/dolphin-runs/2026-11-02-gx-light`.

## 2026-11-02: GX texture regions and palette in Dolphin

The video probe checks cache and preload region fields against the C packed
layout, preservation of reserved bytes, palette region fields, and palette
format masking. Before the fix, three checks failed: cache region layout,
preload region layout, and palette format masking. The [before report](docs/dolphin-runs/2026-11-02-gx-regions-before/suite.json)
records 39 passes and three failures; DOL SHA-256
`de5c6711341fdbb9f5739713e6c93f63dcd8d21f705c27d535a2c125bb8e16b3`.

The production GX code now writes the two cache flags at their C byte offsets,
preserves the adjacent size and reserved fields, and masks the palette format
to two bits. The [after report](docs/dolphin-runs/2026-11-02-gx-regions-after/suite.json)
records 42 passes and no failures; DOL SHA-256
`95d2e8dbcab4164bc21df982130e57ce408ae391cbdd263d2afca3564f68b844`.
The final capture was inspected. Both DOLs used Dolphin 2609a, a private
profile, a copy of the configured Wii NAND, and revision `75d7f06` with the
uncommitted changes in this batch. HBC was not under test. Transcripts,
transport data, logs, and captures are retained with the reports. The copied
profiles remain local. The checks inspect descriptors and do not establish
texture transfer or physical Wii behavior.

Build each DOL with `cargo xtask build-dol ogc-system-probe --bin video_probe
--probe-features dolphin-automation --no-dev --output
target/xtask/probes-dolphin/video_probe.dol`. Run with `cargo xtask
dolphin-validate --no-build --probe video_probe --artifact-dir <run-directory>`.
`cargo test --manifest-path tests/ogc-gx-host-tests/Cargo.toml` passed five
host tests after the fix. The evidence registry records the five directly
called region and palette APIs.
`cargo xtask check-group hardware` also passed target compilation for six
hardware packages; compilation is not target execution.
The source audit passed with three existing tracked debt entries. The
completeness audit reported 2,230 APIs without accepted exact evidence.

## 2026-11-02: GX indexed textures and LOD in Dolphin

The video probe checks both indexed texture initializers, palette selection,
grouped level-of-detail initialization, edge LOD, and LOD bias. It verifies
fixed register words from the C field layout, including minimum and maximum
LOD clamps. The six directly called APIs are in the evidence registry. The
probe inspects in-memory descriptors; it does not submit texture data to GX or
verify rendered output.

The DOL ran in Dolphin 2609a with 38 passes and no failures. Its SHA-256 was
`2901bee5ff5feb70cdaff1e6ed79ffaec5ec1433c19c7c15412fa33efcb84d4c`.
The build used revision `3c1946f` and the uncommitted video probe change in
this batch. The runner used a private profile and a copy of the configured Wii
NAND. HBC was not under test. The [report](docs/dolphin-runs/2026-11-02-gx-indexed/suite.json),
[transcript](docs/dolphin-runs/2026-11-02-gx-indexed/video_probe/probe.log),
transport data, logs, and captures record the run. The final capture was
inspected and shows the passing summary. The copied profile remains local.

Build: `cargo xtask build-dol ogc-system-probe --bin video_probe
--probe-features dolphin-automation --no-dev --output
target/xtask/probes-dolphin/video_probe.dol`. Run: `cargo xtask
dolphin-validate --no-build --probe video_probe --artifact-dir
docs/dolphin-runs/2026-11-02-gx-indexed`. The checks do not establish physical
Wii behavior or texture rendering.
The source audit passed with three existing tracked debt entries. The
completeness audit reported 2,235 APIs without accepted exact evidence.

## 2026-11-02: GX texture descriptor queries in Dolphin

The video probe checks image and user-data pointers, mipmap state, filters,
level of detail, and the individual and grouped descriptor queries. The
three new texture checks passed in each run. They use aligned in-memory image
buffers and do not submit the images to GX; they do not establish rendered
texture or physical Wii behavior. Twelve directly called APIs were added to
the evidence registry.

The overall video probe did not pass: each run reported 35 passes and one
failure, `video framebuffer register readback`. The [first run](docs/dolphin-runs/2026-11-02-gx-texture/suite.json)
and [repeat](docs/dolphin-runs/2026-11-02-gx-texture-repeat/suite.json) used
the same DOL, SHA-256 `32e2f8f21fe7830690d01a43184a4cea510c27202fa89a859de00c8b1756819e`.
The [diagnostic run](docs/dolphin-runs/2026-11-02-gx-texture-diagnostic/suite.json)
used DOL SHA-256 `c99f42af1d207b98b4c3b202ef253455f8ad9f8f43dade5438c34145a7c9b174`.
The diagnostic build added register logging only. All runs used Dolphin 2609a,
private profiles, a copy of the configured Wii NAND, and revision `151cc35`
with the uncommitted video probe changes in this batch. HBC was not under test.
The diagnostic final capture was inspected and shows the failed summary.

In the diagnostic run the expected framebuffer was `0x809712a0`; the top VI
registers were `0x0084:0x52a0`. The page-capture marker identifies its buffer
as `0x808452a0`, whose physical address matches those register values. The
page logger changed the displayed framebuffer at a page boundary between
video initialization and the readback. The capture code now restores the
framebuffer that was displayed before capture. The same video probe then
passed all 36 checks, including the framebuffer readback, with DOL SHA-256
`c722130242fd2ae15786bd636d3f6cfd310cb0af2e2b95d8f9bb03e4ebb7eb42`;
see the [passing report](docs/dolphin-runs/2026-11-02-gx-texture-restored/suite.json).
The shared capture path also passed the runtime probe with 40 checks; its DOL
SHA-256 was `9aa2607e86666a50d4cf2f98bc446b858be34e0b97757fcab6c037fb6e6c3b87`.
See its [report](docs/dolphin-runs/2026-11-02-page-capture-runtime/suite.json).
The passing video capture was inspected. These runs verify the reported paths
in Dolphin, not every page-capture state or physical Wii behavior.
Reports, transcripts, transport bytes, logs, and captures are retained in the
three run directories. The copied profiles remain local.

Build: `cargo xtask build-dol ogc-system-probe --bin video_probe
--probe-features dolphin-automation --no-dev --output
target/xtask/probes-dolphin/video_probe.dol`. Each run used `cargo xtask
dolphin-validate --no-build --probe video_probe --artifact-dir <run-directory>`.
The source audit passed with three existing tracked debt entries. The
completeness audit reported 2,241 APIs without accepted exact evidence.

## 2026-11-02: GX FIFO metadata in Dolphin

The video probe now verifies the FIFO base, size, initial count, high and low
watermarks, and rejection of a short FIFO initialization. It checks the
watermark bytes against the C `__gxfifo` layout and verifies that adjacent
fields remain unchanged. It also reads both values of the wrap flag from an
isolated FIFO object. The flag is set in the object for that check; this does
not establish that graphics hardware produced a FIFO wrap. The evidence
registry records the four directly called APIs.

The DOL ran in Dolphin 2609a with 33 passes and no failures. Its SHA-256 was
`f2a235b588e78a96633edc299925bc5e915fe323e5ad20000d50781baa16cb56`.
The build used revision `2e492ee` and the uncommitted video probe change in
this batch. The runner used a private profile and a copy of the configured Wii
NAND. HBC was not under test. The [report](docs/dolphin-runs/2026-11-02-gx-fifo/suite.json),
[transcript](docs/dolphin-runs/2026-11-02-gx-fifo/video_probe/probe.log),
transport data, logs, and captures record the run. The final capture was
inspected and shows the passing summary. The copied profile remains local.

Build: `cargo xtask build-dol ogc-system-probe --bin video_probe
--probe-features dolphin-automation --no-dev --output
target/xtask/probes-dolphin/video_probe.dol`. Run: `cargo xtask
dolphin-validate --no-build --probe video_probe --artifact-dir
docs/dolphin-runs/2026-11-02-gx-fifo`. The checks do not establish physical
Wii behavior or hardware FIFO wrap and watermark interrupt behavior.
The source audit passed with three existing tracked debt entries. The
completeness audit reported 2,253 APIs without accepted exact evidence.

## 2026-11-02: HCI event parsing in Dolphin

The runtime probe calls the HCI event parser with fixed command completion,
command status, connection, disconnection, unknown, and malformed event
packets. It verifies decoded fields, declared-length truncation, and the
handling of extra bytes. The evidence registry records this exact parser API.

The DOL ran in Dolphin 2609a with 40 passes and no failures. Its SHA-256 was
`aa5c564f7f472f8f24128e132ccdd7ac44440a859f5be2d17ae1c4d1e08660e0`.
The build used revision `a2da667` and the uncommitted runtime probe changes in
this batch. The runner used a private profile and a copy of the configured Wii
NAND. HBC was not under test. The [report](docs/dolphin-runs/2026-11-02-hci-events/suite.json),
[transcript](docs/dolphin-runs/2026-11-02-hci-events/runtime_probe/probe.log),
transport data, logs, and captures record the run. The final capture was
inspected and shows the passing summary. The copied profile remains local.

Build: `cargo xtask build-dol ogc-system-probe --bin runtime_probe
--probe-features dolphin-automation --no-dev --output
target/xtask/probes-dolphin/runtime_probe.dol`. Run: `cargo xtask
dolphin-validate --no-build --probe runtime_probe --artifact-dir
docs/dolphin-runs/2026-11-02-hci-events`. These checks do not exercise a
Bluetooth transport or establish physical Wii or controller behavior.
The source audit passed with three existing tracked debt entries. The
completeness audit reported 2,257 APIs without accepted exact evidence.

## 2026-11-02: Bluetooth packet parsing in Dolphin

The runtime probe now calls the ACL, L2CAP packet, L2CAP signal, signal
payload, signal iterator, and HIDP parsers with independent fixed bytes. It
checks decoded fields, every truncated header and payload prefix, an empty
HIDP packet, two consecutive signals, and preservation of the iterator input
after an error. The evidence registry records these six exact APIs. It also
records five response and reject builders that the existing probe directly
checks with fixed output bytes, short buffers, and invalid handles.

The DOL ran in Dolphin 2609a with 37 passes and no failures. Its SHA-256 was
`f7308607ad258fd8f07869970d3a9ad43cc5eb0dd8a8e19de5e4f5adb5bc98b0`.
The build used revision `f5de973` and the uncommitted runtime probe change in
this batch. The runner used a private profile and a copy of the configured Wii
NAND. HBC was not under test. The [report](docs/dolphin-runs/2026-11-02-bt-parser/suite.json),
[transcript](docs/dolphin-runs/2026-11-02-bt-parser/runtime_probe/probe.log),
transport bytes, logs, and captures record the run. The final capture was
inspected and shows the passing summary. The copied profile remains local.

Build: `cargo xtask build-dol ogc-system-probe --bin runtime_probe
--probe-features dolphin-automation --no-dev --output
target/xtask/probes-dolphin/runtime_probe.dol`. Run: `cargo xtask
dolphin-validate --no-build --probe runtime_probe --artifact-dir
docs/dolphin-runs/2026-11-02-bt-parser`. These packet checks do not exercise a
Bluetooth transport or establish physical Wii or controller behavior.
The source audit passed with three existing tracked debt entries. The
completeness audit reported 2,258 APIs without accepted exact evidence.

## 2026-11-02: Memory-card metadata helpers in Dolphin

The bus probe calls the eight `ogc-card-sys` banner, icon format, icon speed,
and icon and comment address helpers directly. It checks fixed bit patterns,
preservation of adjacent fields, the first and last icon, an out-of-range
index, null inputs, and distinct address values. It uses an in-memory
`CardStat` and does not write to a memory card. The evidence registry records
the exact calls; it does not establish card persistence or physical hardware
behavior.

The DOL ran in Dolphin 2609a with 16 passes and no failures. Its SHA-256 was
`e66ac5f9bee92668b05dcc6285e362250ccd95c4f8298047628d565a38964f36`.
The build used revision `97eadf3` and the uncommitted bus probe change in this
batch. The runner used a private profile and a copy of the configured Wii
NAND. HBC was not under test. The [report](docs/dolphin-runs/2026-11-02-card-metadata/suite.json),
[transcript](docs/dolphin-runs/2026-11-02-card-metadata/bus_probe/probe.log),
transport bytes, logs, and page captures record the run. The final capture was
inspected and shows the passing summary. The private profile remains local.

Build: `cargo xtask build-dol ogc-system-probe --bin bus_probe
--probe-features dolphin-automation --no-dev --output
target/xtask/probes-dolphin/bus_probe.dol`. Run: `cargo xtask dolphin-validate
--no-build --probe bus_probe --artifact-dir
docs/dolphin-runs/2026-11-02-card-metadata`.
The refreshed inventory reports 2,269 entries without accepted exact API
evidence. The source audit passed with three existing tracked debt entries;
the completeness audit failed on those 2,269 entries.

## 2026-11-02: Arena boundary and allocation probes in Dolphin

The arena probe now tests `get_arena1_lo`, `get_arena1_hi`,
`get_arena2_lo`, and `get_arena2_hi` after changing both bounds within the
original free arena. It checks the reported size and restores both bounds
before it checks them again. It also calls `alloc_arena1_mem_lo` and
`alloc_arena2_mem_lo` with two alignments, including a 0x31-byte allocation
whose end requires alignment. The expected addresses are calculated without
the production allocation helper. These six exact APIs now have entries in
the evidence registry. The existing host evidence for the setter and size
APIs remains in that registry.

The focused DOL ran in Dolphin 2609a with 11 passes and no failures. Its
SHA-256 was `17f56683c8b0cd05bc1d55ecc496257f51d535efcc25c79741b5c910aa565de0`.
The build used revision `5a19cba` and the uncommitted arena probe change in
this batch. It used a private profile and a copy of the configured Wii NAND;
HBC was not under test. The [suite report](docs/dolphin-runs/2026-11-02-arena-bounds/suite.json),
[transcript](docs/dolphin-runs/2026-11-02-arena-bounds/arena_probe/probe.log),
transport bytes, logs, and page captures record the result. The final captured
page was inspected and shows the passing summary. The copied profile is local.

Build: `cargo xtask build-dol ogc-system-probe --bin arena_probe
--probe-features dolphin-automation --no-dev --output
target/xtask/probes-dolphin/arena_probe.dol`. Run: `cargo xtask dolphin-validate
--no-build --probe arena_probe --artifact-dir
docs/dolphin-runs/2026-11-02-arena-bounds`. The Dolphin run does not establish
physical Wii behavior or allocation behavior outside the cases in this probe.
The refreshed inventory has 2,277 entries without accepted exact API evidence;
the six entries in this batch were previously unverified.

## 2026-11-02: Dolphin API probe baseline and arena checks

The registered Dolphin suite ran 30 probes from revision
`8de76a4cc4a340cf436feeb6a56a6db47ef5f4c7`, using Dolphin 2609a. It
reported 23 passes, five probe failures, one timeout, and one crash. The
[suite report](docs/dolphin-runs/2026-11-02-baseline/suite.json) records each
DOL SHA-256, command arguments, result, and evidence path. The runner used
private profiles, copies of the configured Wii NAND, USB Gecko logging, and
emulated SD media. HBC was not under test. The baseline DOLs were built before
the arena probe change in this batch.

The failed probes were `libmad_probe` (PCM checksums and probe logging/timing),
`callback_probe` (power callback dispatch), `network_probe` (socket opens),
`font_probe` (second initialization), and `storage_probe` (argument handoff,
USB media, and SD file operations). `sound_probe` timed out after audio
register and timing failures; `debugger_probe` crashed. See the individual
results, transcripts, transport data, Dolphin logs, and page captures under
the suite directory. These failures remain open; they do not identify a cause
on their own. The private profiles and writable SD images remain local and are
not tracked. The final storage and Wii std filesystem SD image SHA-256 values
were `e519dc22ae640559de7109bccedb9b5755c28304a4c8f6d756096741e8ebfc56`
and `b433d515edae3b200ffe51729a91a0faaa6e496e7e5cc63e3a753adc2f664ebb`,
respectively.

The arena probe now checks both reported arena sizes against the captured
bounds and calculates allocation expectations independently of the production
allocation helper. The updated DOL ran with nine passes and no failures; its
SHA-256 was `e93fe21ee54889af8a7f891f430011339337fb4bd23565627a7f877d3c5648c3`.
See its [suite report](docs/dolphin-runs/2026-11-02-arena/suite.json) and
[transcript](docs/dolphin-runs/2026-11-02-arena/arena_probe/probe.log).

Builds used the reference MP3 fixture at
`[libmad-rs source]/test-data/layer3/libmad-stereo-mpeg1.mp3`,
SHA-256 `c1f5d2789060c3e3e1527ebd08c4a7777fb0ca55f4f46da720c919e518b9ae73`,
for both fixture variables. The baseline command was `cargo xtask
dolphin-validate --no-build --artifact-dir docs/dolphin-runs/2026-11-02-baseline`
after `cargo xtask dolphin-validate` built the automation DOLs. The focused
command was `cargo xtask dolphin-validate --no-build --probe arena_probe
--artifact-dir docs/dolphin-runs/2026-11-02-arena` after a focused automation
build. The reports retain the absolute paths used in the runs.

`cargo xtask verify-complete` passed its host tests, inventory and source
checks, then failed the completeness gate with 2,283 unverified API entries.
The source audit also reports three existing tracked audit-debt entries. A
Dolphin result does not establish physical Wii behavior, and the suite does
not exercise every public API.

## 2026-10-29: ISO 9660 and SHA runtime probe in Dolphin

- Command: `LIBOGC_MAD_PROBE_MP3=<fixture> LIBOGC_MAD_REFERENCE_MP3=<fixture> cargo xtask dolphin-validate --probe runtime_probe --artifact-dir [workspace temp]/libogc-runtime-iso-20261029`.
- Fixture: `[libmad-rs source]/test-data/layer3/libmad-stereo-mpeg1.mp3`, SHA-256 `c1f5d2789060c3e3e1527ebd08c4a7777fb0ca55f4f46da720c919e518b9ae73`.
- Dolphin 2609a used a private profile and a copy of the configured Wii NAND. The runner used the OpenGL backend and USB Gecko TCP logging. HBC was not under test.
- The DOL ran. Its SHA-256 was `3a9dcf92e146dead0b5d61a9bcb12fe8e84a563ed6f7fb03bdf9461ef814fb00`. The build used revision `3c8ac8237c86f4c57c66e9623821558669862e74` and the uncommitted ISO probe and ELF test changes in this commit.
- The SHA context check and all four ISO 9660 checks passed. ISO used a controlled read-only medium and the production mount API. This does not validate a physical drive.
- The overall probe failed: 17 checks passed and two failed. The PowerPC timebase did not advance between consecutive reads; STM LED mode returned `-4`. These failures need separate investigation.
- The retained [probe log](docs/review/evidence/runtime-iso-20261029/probe.log) and [suite report](docs/review/evidence/runtime-iso-20261029/suite.json) identify the checks and configuration. The full temporary run directory contains screenshots and Dolphin logs.

The `cargo xtask audit-stubs` completeness audit failed with 2,400 API entries marked unverified. The inventory itself was current. These existing repository-wide findings remain open; this run did not suppress them.

## 2026-10-29: Controller model host tests

`cargo test -p ogc-controller` passed 78 unit tests and 12 integration tests.
The new checks exercise button masks, snapshot capacity, player assignment,
and held, pressed, and released button aggregation. The inventory records
18 directly exercised APIs as protocol evidence. These tests exercise the
production model on the host; they do not validate a physical controller or
Wii transport.

## 2026-10-29: Controller transitions and HID host tests

`cargo test -p ogc-controller` passed 80 unit tests and 12 integration tests.
New checks validate empty model values and transition identities across a
disconnect and reconnect. Existing tests exercise raw button transitions and
the production HID parser, decoding, report sizes, and application types.
The evidence registry records 14 directly exercised APIs. These host results
do not establish real controller, USB, Bluetooth, or Wii behavior.

## 2026-10-29: Xbox report and Bluetooth stage host tests

`cargo test -p ogc-controller` passed 82 unit tests and 12 integration tests.
New tests check invalid Xbox report lengths, fixed axis and trigger endpoints,
and the text for each Bluetooth lifecycle stage. The inventory records the
two directly exercised APIs. These host tests do not verify a physical Xbox
controller or Bluetooth transport.

## 2026-10-29: Switch HID and USB event host tests

`cargo test -p ogc-usb-events -p ogc-bt-sync` passed 9 USB event tests
and 103 Bluetooth tests. New fixed-byte checks cover Switch HIDP output, input
headers, and standard reports. The silent stop test checks that pending
callbacks do not survive a restart. Four API entries have host test evidence.
These results do not validate a physical controller or USB/Bluetooth transport.

## 2026-10-29: USB event failure and WPAD button packing

`cargo test -p wiiuse-rs -p ogc-usb-events` passed 53 Wiiuse tests and
10 USB event tests. The USB test checks that stale and oversized events
leave registrations intact until the current generation completes them.
The WPAD test checks fixed Classic and Wii U Pro button masks. The shared
production logic ran on the host; no physical controller or USB transport
was tested. The evidence registry records `wpad_buttons` for this host test.

## 2026-10-29: libmad ABI helper host tests

`cargo test --manifest-path tests/mad-sys-host-tests/Cargo.toml` passed nine
tests. The host package includes the production ABI, bit, fixed-point, timer,
and error source files directly. It checks fixed bit and checksum values,
fixed-point arithmetic, timer arithmetic, and error codes. The inventory
records host evidence for 26 directly called helpers. This does not validate
the PowerPC ABI layout, MP3 decoding, audio output, or physical hardware.

## 2026-10-29: Expanded libmad host checks

The host package now includes all production libmad ABI modules.
`cargo test --manifest-path tests/mad-sys-host-tests/Cargo.toml` passed 30
tests. New checks cover stream synchronization and synthesis initialization
and mute state. Existing tests cover a fixed Layer I frame, synchronous
decoder callbacks, and synthesis. The inventory records 12 additional APIs.
This result does not verify the PowerPC ABI or physical audio output.

## 2026-10-30: libmad header and lifecycle host checks

The libmad ABI host package passed 35 tests. New checks cover a fixed MPEG
header and truncated input, frame and stream allocation cleanup, timer text,
and the sync-only decoder message error. Seven API evidence entries were
added. Decoder message exchange remains unsupported and is not marked as
implemented. These tests do not establish PowerPC ABI or audio hardware behavior.

## 2026-10-30: Runtime probe and UI API checks

Dolphin 2609a ran the runtime probe in a private profile with a copied NAND.
The command was `cargo xtask dolphin-validate --probe runtime_probe --artifact-dir
[workspace temp]/libogc-runtime-final-20261030`, with both libmad fixture
variables set to the fixture recorded above. The DOL SHA-256 was
`8b5e5e4f7f7655ab6a1e60ec6b7a0d3bf41a096423c6a0271ebf519634f3cdc0`.
The source revision was `72a34b2d965cf43b8bcca25f5760162bcc9d51da`, with
uncommitted probe and UI changes. The probe passed 19 checks and failed zero.
The adjacent timebase reads were equal; bounded polling measured two ticks.
STM LED mode returned zero. The old probe repeated pre-main, which closes IOS
descriptors and can invalidate STM state. The revised probe uses initialized
services and requires a successful LED result. These results do not prove Wii
hardware behavior. HBC was not under test. See the [run evidence](docs/review/evidence/runtime-20261030/suite.json)
and [probe log](docs/review/evidence/runtime-20261030/probe.log).

The UI host suite passed 35 tests with controller input enabled. New tests
check layout bounds, rectangle edges, colors, text, visibility, enabled state,
and invalid widget IDs. Five deliberate faults caused the relevant tests to
fail before the production source was restored. The host suite, UI clippy,
and formatting checks passed. The completeness audit reports 2,298 unverified
API entries; physical hardware tests and the remaining API checks are open.

## 2026-10-30: UI target adapter checks

The runtime probe ran in Dolphin 2609a with a private profile and copied NAND.
The probe passed 21 checks, including PAD edge, reset, and reconnect checks
and WPAD button, pointer, invalid viewport, and error checks using controlled
samples. The DOL SHA-256 was
`15df0c5195aa8760868801cf5c1b0da7c9ae9d767bd8f783a78c69ed92b795ac`.
See [the probe log](docs/review/evidence/ui-adapters-20261030/probe.log) and
[run report](docs/review/evidence/ui-adapters-20261030/suite.json). This run
does not validate physical controller input. The GX canvas check was added
to the UI input probe and its DOL built, but it has not run in Dolphin or on
hardware. The UI input probe is not registered with the Dolphin runner.
The UI host suite passed 35 tests. Formatting and UI clippy passed. The source
audit passed; the completeness audit reports 2,294 unverified entries.

## 2026-10-30: AESND owned buffer and handle checks

The runtime probe ran in Dolphin 2609a with a private profile and copied NAND.
It passed 23 checks and failed none. The new checks use production AESND APIs
to check independent byte values, copies after source mutation, lengths and
32-byte alignment at cache-line boundaries. They also check null and non-null
voice handle pointer round trips without dereferencing the pointer. The DOL
SHA-256 was `37bca7057551179581b58f9c0a3912c417808e9a92c525232f4c93084d0805f4`.
See the [probe log](docs/review/evidence/audio-buffer-20261030/probe.log)
and [run report](docs/review/evidence/audio-buffer-20261030/suite.json).
These checks do not establish DSP cache visibility, audio playback, or physical
Wii behavior. HBC was not under test. The completeness inventory reports 2,287
unverified entries.

## 2026-10-30: L2CAP packet construction in Dolphin

The runtime probe passed 27 checks in Dolphin 2609a, including fixed-byte
checks for connection and configuration requests, MTU options, data framing,
invalid handles, and a short buffer. The DOL SHA-256 was
`65987c7045a554df3b0e6e9c5ab129072f220910f502834a75cc145c19595b01`.
See the [probe log](docs/review/evidence/l2cap-runtime-20261030/probe.log) and
[run report](docs/review/evidence/l2cap-runtime-20261030/suite.json). These
checks do not establish Bluetooth transport or physical controller behavior.
The Bluetooth host suite passed 103 tests; clippy and formatting passed.
The completeness audit remains open with 2,283 unverified entries.
