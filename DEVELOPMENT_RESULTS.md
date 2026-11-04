# Development results

## 2026-11-03: Wii FAT file and path probe

`wii_std_fs_probe` passed 99 checks in Dolphin 2609a on the private virtual
Wii SD card. It checked directory and path operations, file I/O, metadata,
attributes, timestamps, read-only access, remount persistence, and cleanup.
The DOL SHA-256 is
`51d1dc129173a4c427340fad52540bcb36f4fdd4fc05efddaa3c394e82ed226f`. This
does not validate physical Wii SD, USB, or GameCube card behavior. See
[`docs/dolphin-runs/2026-11-03-wii-std-fs-storage/README.md`](docs/dolphin-runs/2026-11-03-wii-std-fs-storage/README.md).

## 2026-11-03: storage runtime probe follow-up

The `runtime_probe` passed 63 checks in Dolphin 2609a with no failures. Its
new `get_stats_async` check confirms that the closed ISFS service returns
`ISFS_EINVAL`, preserves all seven output fields, and does not call the
callback. The controlled ISO image also passed both C and Rust file-operation
checks. This does not prove successful NAND operations or physical disc
behavior. See
[`docs/dolphin-runs/2026-11-03-isfs-get-stats-async-pass/README.md`](docs/dolphin-runs/2026-11-03-isfs-get-stats-async-pass/README.md).

## 2026-11-03: ISFS rename result and cleanup checks

The updated `runtime_probe` checks that sync and async rename results match
the file state. Dolphin 2609a returned `ISFS_EINVAL` for both calls. The source
files remained, destination paths stayed absent, and cleanup checks passed.
The full probe reported 87 passes and 1 failure. `format_async` removed its
marker file and directory, but its callback did not complete, so the final IOS
result remains unknown. The DOL SHA-256 is
`e6cd69a135e2fd730cb2bacda1de473e3ee54b71cc389c1a091e7fb1de6c06dd`. This
does not prove successful rename or physical Wii NAND behavior. See the
[run report](docs/dolphin-runs/2026-11-03-isfs-rename-state/README.md).

## 2026-11-03: ISO9660 file runtime probe

The earlier runtime probe passed 62 checks in Dolphin 2609a with no failures. Its
controlled in-memory image verified file lookup, open, read, seek, close,
stat, and root-directory change through both the C ISO mount and Rust
filesystem mount. The probe found and verified fixes for the devoptab device
name and valid negative pointer-shaped open results. This does not test a
physical disc or drive. See
[`docs/dolphin-runs/2026-11-03-iso9660-file-runtime-pass/README.md`](docs/dolphin-runs/2026-11-03-iso9660-file-runtime-pass/README.md).

## 2026-11-03: storage inventory and host-test audit

The media-aware `storage_probe` run passed in Dolphin 2609a with 97 checks and
0 failures. It captured 10 pages and no video frames. The private Dolphin
profile had no GameCube card image and no USB storage device. The probe checked
the expected no-card async mount result, verified the USB no-device result,
skipped operations that require those devices, and exercised the virtual Wii
SD path. This run does not verify card saves, USB transfers, disc reads, or
physical hardware. See
[`docs/dolphin-runs/2026-11-03-storage-probe-media-aware/README.md`](docs/dolphin-runs/2026-11-03-storage-probe-media-aware/README.md).

`cargo xtask check-inventory` passed and confirmed 3,387 API entries. The
current crate-role data places 15 runtime crates in the storage build group.
The generated API inventory has 604 entries in 14 of those packages, with no
entry classified `unverified` for implementation evidence. This count proves
source-evidence mapping only. It does not prove runtime behavior. The CARD and
DI runtime checks remain open; see the storage verification groups in
`docs/milestone2-storage.md`.

`cargo test -p ogc-card-metadata` passed 17 tests. `cargo test
-p ogc-usbstorage-lifecycle` passed 21 tests. These host tests check shared
CARD metadata and USB lifecycle logic. They do not exercise the target-only
storage backends or physical devices.

The `xtask` test harness now imports the exact `ogc-fat` shared source modules
for file transfer, media presence, rename transactions, sector caching, and
timestamps. This runs their existing tests on the host without copying the
logic or compiling the Wii-only backends. `cargo test --manifest-path
xtask/Cargo.toml --bin xtask` passed 107 tests, including 28 FAT shared-logic
tests. These results do not verify FAT device I/O or media behavior.

`LIBOGC_MAD_REFERENCE_MP3=[libogc-rs fixtures]/libmad-stereo-mpeg1.mp3
cargo xtask test-host` passed all 28 host-capable packages, the standalone
Rust suites, and the Python suites. The fixture SHA-256 is
`c1f5d2789060c3e3e1527ebd08c4a7777fb0ca55f4f46da720c919e518b9ae73`. The
first run stopped because this fixture variable was unset. The next run
reached the Python network tests but sandbox rules denied a loopback bind. The
final run used the approved local socket access and passed. This host suite
does not verify Wii IOS or physical storage devices.

The DI no-disc `dvd_probe` now compares the C ABI wrappers with the paired
Rust APIs for drive identification, support, disc ID, cover status,
configuration, copyright, disc key, physical data, BCA, and report-key calls.
The final Dolphin 2609a run passed 119 checks with 0 failures. It had no disc,
so it does not validate data reads from physical media. Two consecutive
`DI_GetError` calls returned different non-sentinel values, while both
returned success; the probe checks each call and does not require the values
to match. See
[`docs/dolphin-runs/2026-11-03-di-wrapper-parity-final/README.md`](docs/dolphin-runs/2026-11-03-di-wrapper-parity-final/README.md).

`cargo xtask check-group storage` passed for all 15 Wii runtime crates.
Clippy passed with warnings denied for the two host-testable packages:
`cargo clippy -p ogc-card-metadata -p ogc-usbstorage-lifecycle --all-targets
-- -D warnings`.

The combined command `cargo test -p ogc-card-metadata -p ogc-fat -p
ogc-filesystem -p iso9660 -p ogc-usbstorage-lifecycle` failed before tests ran.
Cargo attempted to build target-only Wii dependencies for the host, which
stopped at their PowerPC target guards. This result does not invalidate the
two separately passing host-test suites.

The latest storage probe DOL build did not run in Dolphin. Dolphin 2609a
aborted on `--version` before launch; the crash report records `SIGABRT` in
Qt's `qDetectCpuFeatures` initializer. The two CARD launch attempts and their
DOL identities are recorded in `docs/dolphin-runs/2026-11-03-storage-api-audit/`
and `docs/dolphin-runs/2026-11-03-storage-card-inventory-zero/`. No new
Dolphin or hardware result was collected in this audit. Diagnostic retries
with the offscreen Qt platform did not start: the bundle has only the Cocoa
platform plugin. The ARM64 retry still reported the missing `neon` feature.
The x86_64 retry still failed to connect to the macOS pasteboard. Launching
the app through LaunchServices also returned `kLSNoExecutableErr` even though
the signed app bundle and its executable are present. These checks do not
provide runtime evidence.

## 2026-11-03: FAT SD API probe and devoptab fix

The focused `fat_sd_probe` passed all 21 checks in Dolphin 2609a. It mounted
the private virtual FAT32 SD image, checked the SD insertion and mount state,
compared the C and Rust volume labels, created and removed a temporary file,
checked file seek and truncate results, set and restored a DOS attribute, and
checked the C FAT init, mount, and unmount exports. Both GameCube SD Gecko FAT
mount calls returned `DeviceUnavailable` because this Dolphin setup has no EXI
SD adapter. That result checks the no-device path. It does not verify a
physical SD Gecko, non-virtual Wii SD transport, or disk persistence outside
the private profile.

The probe found that the FAT devoptab stored the colon-terminated lookup key
(`sd:`) as its registered device name. `FindDevice("sd:")` expects the bare
name (`sd`), so both `fatInit` and `fatInitDefault` returned false even though
the mount remained online. The mount now keeps a bare devoptab name and a
separate colon-terminated lookup key. The probe checked direct path lookup and
both init exports after this change.

The repository base was `caeaa31`; the run used a dirty worktree containing
the FAT fix and probe. The DOL SHA-256 was
`2a93ae09e5b67fdfbc1735456edf9d152cc9a9b354277c059c89597ad628ce43`.
The command was `cargo xtask dolphin-validate --no-build --probe fat_sd_probe
--artifact-dir docs/dolphin-runs/2026-11-03-fat-sd-final --dolphin
[Dolphin executable]`. See the
[report](docs/dolphin-runs/2026-11-03-fat-sd-final/suite.json) and
[probe log](docs/dolphin-runs/2026-11-03-fat-sd-final/fat_sd_probe/probe.log).
The two physical GameCube SD Gecko mount checks remain open in the
[hardware register](docs/hardware-api-validation.csv).

## 2026-11-03: Wii SD interface probe

The focused `wiisd_probe` calls the registered Wii SD interface callbacks for
startup, insertion state, sector-zero reads, zero-sector writes, and shutdown.
Dolphin 2609a passed all 9 checks with no failures. The runner used a private
profile with its virtual Wii SD card enabled. The zero-sector write uses an
unaligned buffer so the callback skips the data-command loop; it sends no data
command and leaves the buffer unchanged. This does not test a non-zero sector
write or physical Wii SD hardware. The repository base was `609c0ae`; the DOL
was built with the probe changes in a dirty worktree. Its SHA-256 was
`7d95e5e9446de29913a52d140570ad59a1af543072fe46dbca72374f31297439`.
The command was `cargo xtask dolphin-validate --no-build --probe wiisd_probe
--artifact-dir docs/dolphin-runs/2026-11-03-wiisd --dolphin
[Dolphin executable]`. See the
[report](docs/dolphin-runs/2026-11-03-wiisd/suite.json) and
[probe log](docs/dolphin-runs/2026-11-03-wiisd/wiisd_probe/probe.log).
Physical Wii SD startup and read behavior remain open in the
[hardware register](docs/hardware-api-validation.csv).

## 2026-11-03: Wii SD virtual-card write round trip

The Dolphin-only section in `wiisd_probe` now writes a known pattern to sector
zero of the runner's private virtual SD card, reads it back, restores the
original sector, and verifies the restored bytes. Dolphin 2609a passed 12
checks with no failures. The DOL SHA-256 is
`dd4be0b0ec2ec7b88d2637685b5e7e7bc4cd5a6b3a4f413a38afb2c07c6959a8`.
This run verifies the emulated write path and restoration only. It does not
prove physical SD transport or physical-card data integrity. See the
[Dolphin report](docs/dolphin-runs/2026-11-03-wiisd-write-roundtrip/README.md).

## 2026-11-03: Wii SD transfer and no-card probes

The focused `wiisd_probe` passed 15 checks in Dolphin 2609a with the virtual
Wii SD card enabled. It wrote and read back nine sectors through the
unaligned-buffer bounce path, then restored and verified the original data.
A separate run with the virtual card disabled passed 10 checks: reads and
writes failed, read data stayed unchanged, and shutdown/restart kept the
interface closed. These runs verify Dolphin behavior only. See the
[multi-sector report](docs/dolphin-runs/2026-11-03-wiisd-multisector/README.md)
and [no-card report](docs/dolphin-runs/2026-11-03-wiisd-no-card/README.md).
The [Wii SD API audit](docs/dolphin-runs/2026-11-03-ogc-wiisd-api-audit.md)
maps all five public APIs to the saved runs and records that the no-card
summary has no raw per-check log.

## 2026-11-03: ISO9660 C ABI probe update

The runtime probe now calls the exported `ISO9660_Mount`,
`ISO9660_GetVolumeLabel`, and `ISO9660_Unmount` functions directly. It checks
null arguments, rejected media reads, the fixed volume-label bytes, and
unmount cleanup. Dolphin 2609a passed all 41 runtime-probe checks, including
five ISO9660 checks, with no failures. The runner used a private profile and a
copy of the Wii NAND. The run command was `cargo xtask dolphin-validate
--no-build --probe runtime_probe --artifact-dir
docs/dolphin-runs/2026-11-03-iso9660-c-abi --dolphin
[Dolphin executable]`. The DOL SHA-256 was
`141586a156931ad96fe445c125dd1dd7f68011c5a6b8e25f2b3acbae2dac8b02`; the
source revision was `982b3f4`. The run used the Intel Dolphin slice because
the native slice stopped at Qt's NEON check. The temporary launcher used for
the run is archived as [dolphin-intel-wrapper.sh](docs/dolphin-runs/2026-11-03-iso9660-c-abi/dolphin-intel-wrapper.sh).
See the
[report](docs/dolphin-runs/2026-11-03-iso9660-c-abi/suite.json) and
[probe log](docs/dolphin-runs/2026-11-03-iso9660-c-abi/runtime_probe/probe.log).
The disc interface reads a controlled in-memory ISO image. This checks the
production mount code and C ABI, but it does not test a physical optical drive.
The build used `cargo xtask build-dol ogc-system-probe --bin runtime_probe
--probe-features dolphin-automation --no-dev --output
target/xtask/probes-dolphin/runtime_probe.dol` with the external fixture paths
recorded in `docs/dependencies.md`. The fixture was
`[libmad-rs source]/test-data/layer3/libmad-stereo-mpeg1.mp3`
(SHA-256 `c1f5d2789060c3e3e1527ebd08c4a7777fb0ca55f4f46da720c919e518b9ae73`).

## 2026-11-03: USB storage facade no-device checks

The focused USB storage DOL passed 28 checks in Dolphin 2609a. Seventeen
`ogc-usbstorage` APIs now have direct evidence: no-device initialization and
cleanup, raw-to-safe error mapping, null-handle rejection, default flush, and
the DVD query. Failed open returned `-6` and left the handle closed with no
buffers. The probe made no sector writes. The DOL SHA-256 was
`fbe8aabb289e0de13037dced3d3f702644bbefde651beea3a8fc86eafdc601dd`. The run
used source revision `4ecca92` plus the probe changes in this batch, a private
Dolphin profile, and a copy of the Wii NAND. See the
[report](docs/dolphin-runs/2026-11-03-usbstorage-facade-final/suite.json) and
[probe log](docs/dolphin-runs/2026-11-03-usbstorage-facade-final/usbstorage_capacity_probe/probe.log).
This run did not have a physical USB storage device. It does not test a live
USB transport. `flush` and `try_flush` remain open because their contract needs
a valid mounted handle. Build with `cargo xtask build-dol ogc-system-probe
--bin usbstorage_capacity_probe --probe-features dolphin-automation --no-dev
--output target/xtask/probes-dolphin/usbstorage_capacity_probe.dol` and the
fixture variables recorded in `docs/dependencies.md`. Run with `cargo xtask
dolphin-validate --no-build --probe usbstorage_capacity_probe --artifact-dir
docs/dolphin-runs/2026-11-03-usbstorage-facade-final --dolphin
docs/dolphin-runs/2026-11-03-iso9660-c-abi/dolphin-intel-wrapper.sh`.

## 2026-11-02: AESND voice controls and cycle checks

The focused AESND DOL ran in Dolphin 2609a with a private profile and copied
NAND. It reported 100 passes and no failures. The DOL SHA-256 was
`f1a880317ff563bb70dba052105ec862e4d104402ebe67a1ecd72a6e7d16992c`.
The build used revision `479981a` and the probe changes in this batch. See the
[report](docs/dolphin-runs/2026-11-02-aesnd-delay-drain/suite.json). HBC was
not under test.

The probe checks buffer replacement, mute, format, frequency, delay, loop, and
stream behavior through wrapper and backend calls where applicable. It checks
PCM output, silence, callbacks, and playback duration. The private DSP cycle
methods are checked using the same source compiled into the DOL; that check
does not observe the live callback's internal state. All 23 remaining AESND
inventory entries now have accepted evidence. A separate application build has
played audible sound effects on Wii hardware; the individual API checks in this
audio batch use Dolphin or host tests and have not run on Wii hardware.

Two earlier runs failed both delay checks. The diagnostic run recorded one
matching output buffer during each early window. The final run drains four
frames before measuring; its following twelve-frame window had zero matches,
and matching output arrived later. The failed reports remain in
`docs/dolphin-runs/2026-11-02-aesnd-timing` and
`docs/dolphin-runs/2026-11-02-aesnd-delay-diagnostic`.

Build with `cargo xtask build-dol ogc-system-probe --bin aesnd_native_probe
--probe-features dolphin-automation --no-dev --output
target/xtask/probes-dolphin/aesnd_native_probe.dol`. Run with `cargo xtask
dolphin-validate --no-build --probe aesnd_native_probe --timeout-seconds 180
--artifact-dir docs/dolphin-runs/2026-11-02-aesnd-delay-drain`.

## 2026-11-02: AESND wrapper and callback checks

The AESND native DOL ran in Dolphin 2609a with a private profile and copied
NAND. It reported 85 passes and no failures. The DOL SHA-256 was
`20dfd70155a9b52bfcbb00ce90af817fa3b685970f57f4d3c229871bf0920339`.
The build used revision `677d38f` and the AESND probe changes in this batch.
The [report](docs/dolphin-runs/2026-11-02-aesnd-callbacks/suite.json) and
probe log record the run. HBC was not under test.

The checks cover callback registration and delivery, known DSP output, wrapper
volume, stop, voice slot reuse, DSP usage, and owned buffer replacement. They
add evidence for 22 inventory entries. Seven `aesnd` and 16 `aesnd-sys`
entries remain unverified. The run does not prove physical Wii audio or every
setter and error path. The full probe build required an unavailable libmad
fixture; the focused AESND DOL was built separately and run with `--no-build`.

Build with `cargo xtask build-dol ogc-system-probe --bin aesnd_native_probe
--probe-features dolphin-automation --no-dev --output
target/xtask/probes-dolphin/aesnd_native_probe.dol`. Run with `cargo xtask
dolphin-validate --no-build --probe aesnd_native_probe --artifact-dir
docs/dolphin-runs/2026-11-02-aesnd-callbacks`.

## 2026-11-02: UI canvas clipping checks

Host tests exercise the production clipped canvas for fill and text calls,
including partial overlap, no overlap, and backend errors. The UI inventory
now has evidence for all 66 entries. The GX rendering backend still needs
pixel evidence. An experimental video probe in Dolphin 2609a reported 52
passes and one failure: both rendered pixels and a control EFB poke read back
as black. The DOL SHA-256 was
`e01937c69728e569e1602d7f9e251f002c4b3a117a8334dafdb08cc8ac1f03a6`.
The [report](docs/dolphin-runs/2026-11-02-ui-canvas-poke/suite.json) records
the run. The exploratory probe and runner changes were not retained. This
result does not establish whether GX drawing or Dolphin EFB readback failed.
Physical Wii drawing remains untested.

## 2026-11-02: UI canvas evidence review

The existing [Dolphin video run](docs/dolphin-runs/2026-11-02-gx-texconv/video_probe/probe.log)
passed the GX canvas dimension and validation check. It calls `from_video_mode`,
`new`, and `size`; the evidence registry now records those calls. At that time the two Canvas
drawing trait methods remained unverified. A new Dolphin attempt stopped before
the probe: Dolphin aborted during its version check and reported that its Qt
build requires NEON. The drawing behavior and physical Wii behavior remain open.

## 2026-11-02: GX RGB565 conversion in Dolphin

The video probe checks two RGB565 tiles using fixed expected bytes, the
1024-byte source stride, partial dimensions, a negative width, and output
sentinels. The DOL ran in Dolphin 2609a with 52 passes and no failures. Its
SHA-256 was `f26d092530bf92f317f1444fdef380f4d377bdae8120de2e307e533279053374`.
The build used revision `fde546b` and the probe change in this batch. The
runner used a private profile and copied Wii NAND. HBC was not under test.
The [report](docs/dolphin-runs/2026-11-02-gx-texconv/suite.json) records the
run. Texture rendering and physical Wii behavior remain untested.

Build with `cargo xtask build-dol ogc-system-probe --bin video_probe
--probe-features dolphin-automation --no-dev --output
target/xtask/probes-dolphin/video_probe.dol`. Run with `cargo xtask
dolphin-validate --no-build --probe video_probe --artifact-dir
docs/dolphin-runs/2026-11-02-gx-texconv`.

## 2026-11-02: GX texture buffer sizes in Dolphin

The video probe checks fixed texture sizes for I4, I8, RGB565, and RGBA8
across tile boundaries and mip levels. The DOL ran in Dolphin 2609a with 51
passes and no failures. Its SHA-256 was
`55c45b38c6cc9137babb5333a4d537af414605b1564f64d63eede345a9c00924`.
The build used revision `8860d0e` and the probe change in this batch. The
runner used a private profile and copied Wii NAND. HBC was not under test.
The [report](docs/dolphin-runs/2026-11-02-gx-texture-sizes/suite.json)
records the run. These checks do not establish texture rendering or physical
Wii behavior.

Build with `cargo xtask build-dol ogc-system-probe --bin video_probe
--probe-features dolphin-automation --no-dev --output
target/xtask/probes-dolphin/video_probe.dol`. Run with `cargo xtask
dolphin-validate --no-build --probe video_probe --artifact-dir
docs/dolphin-runs/2026-11-02-gx-texture-sizes`.

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

## 2026-11-02: Low-level audio register probe

`audio_sys_probe` called all 22 previously unverified `ogc-audio-sys` APIs
directly. Dolphin 2609a passed 11 checks with zero failures. The checks covered
AI initialization and callback stack storage, AI volume, sample rates, stream
trigger and play state, stream counter reset, DMA address and length
readback, DMA enable and stop, and callback registration and removal. The DMA
address check uses the hardware-visible 29-bit bus address, which omits the
cached-memory prefix from the CPU pointer. The final DOL SHA-256 is
`95c6874a3d5484780822dc953a2765efdec34dc53b0ff769b35254fba653a4aa`.
See [the passing suite](docs/dolphin-runs/2026-11-02-audio-sys-final/suite.json)
and [probe log](docs/dolphin-runs/2026-11-02-audio-sys-final/audio_sys_probe/probe.log).

Dolphin did not advance the DMA byte count or dispatch the AI callbacks, so
callback delivery remains open for a real-Wii probe. The failed exploratory
runs are retained at
[the invalid-address/full sound run](docs/dolphin-runs/2026-11-02-audio-sys/suite.json),
[the focused callback run](docs/dolphin-runs/2026-11-02-audio-sys-callbacks/suite.json),
and [the aligned-buffer run](docs/dolphin-runs/2026-11-02-audio-sys-valid-dma/suite.json).
The [bus-address diagnostic run](docs/dolphin-runs/2026-11-02-audio-sys-focused/suite.json)
also records the first address mismatch.
The full sound run also stalled later during ASND startup, outside this focused
probe. These results do not prove physical Wii behavior. The wider audio group
remains open. `cargo xtask check-group audio`, PowerPC Clippy with warnings
denied for `ogc-audio-sys` and the probe library, host Clippy with warnings
denied for the audio libmad host harness, `cargo fmt --all -- --check`,
`cargo xtask audit-source`, and the generated inventory check passed. The
general host Clippy attempt was not applicable because runtime dependencies
reject non-PowerPC builds.

## 2026-11-02: ASND raw voice and timing probe

`asnd_sys_probe` called the 20 open `asnd-sys` APIs directly. Dolphin 2609a
passed 16 checks and failed none. The checks covered fixed note-frequency and
time values, owned buffer reads, finite and infinite voice setup, busy queue
rejection, pointer retention, pitch and volume requests, voice timers, owned
voice accessors, and voice cleanup. The DOL SHA-256 is
`4720835054ef41480b696a5559c1f57a2eae89419a7ec7c02891353259523266`.
See [the suite report](docs/dolphin-runs/2026-11-02-asnd-sys-initial/suite.json)
and [probe log](docs/dolphin-runs/2026-11-02-asnd-sys-initial/asnd_sys_probe/probe.log).
The probe did not establish callback cadence or audible output on a physical
Wii. The inventory now has 104 open entries across the eight wider audio
packages. The Wii audio group remains open.

## 2026-11-02: libmad unsupported message result

The `decoder_message` entry now points to its existing production-source host
test. The test passes for a null decoder and a valid decoder: both return the
documented unsupported result, and the message bytes and length stay unchanged.
`cargo test --manifest-path tests/mad-sys-host-tests/Cargo.toml` passed all 35
tests. This verifies the sync-only unsupported result; it does not verify
asynchronous message exchange. The older `libmad_probe` Dolphin checksum and
timing failures remain open and need their own diagnosis. The inventory now
has 103 open entries across the eight wider audio packages.

## 2026-11-02: ModPlay state API probe

The Dolphin `modplay_probe` now calls all 24 open `modplay-capi` state APIs
directly. It uses a fixed 2,112-byte caller-owned MOD image and checks ABI
layout, parsed pattern values, SFX channel setup, note routing, volume bounds,
supported and rejected rates, mono/stereo state, silent paused output, mixer
timing, worker start, callback progress, pause/resume, stop, and unload.
Dolphin 2609a passed 27 checks with zero failures on the current source tree.
The DOL SHA-256 is
`4eb58b203e15485c65e34aacfe68a0a64a4ceb72af0353da20853ac9af3938eb`.
See [the current suite](docs/dolphin-runs/2026-11-02-modplay-state-current/suite.json)
and [probe log](docs/dolphin-runs/2026-11-02-modplay-state-current/modplay_probe/probe.log).
The earlier final run remains at
[its suite](docs/dolphin-runs/2026-11-02-modplay-state-final/suite.json).
Two earlier failures exposed errors in the new probe's expected channel index
and callback setup. Their reports remain at
[the first run](docs/dolphin-runs/2026-11-02-modplay-state/suite.json) and
[the diagnostic run](docs/dolphin-runs/2026-11-02-modplay-state-diagnostic/suite.json).
The inventory now has 79 open entries across the eight wider audio packages.

## 2026-11-02: libmad Dolphin checksum diagnosis

The earlier focused `libmad_probe` used host `FPM_DEFAULT` checksums in a
PowerPC build. Two Dolphin reports returned the same PowerPC hashes:
`0x5a848a33` at full rate and `0xc89b5e9a` at half rate. The C reference selects
`FPM_PPC` in `C-sources/libogc/libmad/global.h`; its multiply and scale path
matches the Rust PowerPC `i64` product shifted by 28 bits in
`vendor/libmad-rs/src/fixed.rs`. The probe now checks the PowerPC hashes. This
diagnosis uses source comparison and repeatable target output; it does not
claim a separate C DOL comparison. Earlier failures remain in the
[diagnostic suite](docs/dolphin-runs/2026-11-02-libmad-init-diagnostic/suite.json)
and [probe log](docs/dolphin-runs/2026-11-02-libmad-init-diagnostic/libmad_probe/probe.log).
After a Clippy cleanup, the final probe passed 11 checks and zero failures in
Dolphin 2609a. Its DOL SHA-256 is
`6353a1b9cb993e1d1547cbea98b8037358a4d6bd4fb4015e51a8fed294201c41`. See the
[passing suite](docs/dolphin-runs/2026-11-02-mad-ppc-profile-final/suite.json)
and [probe log](docs/dolphin-runs/2026-11-02-mad-ppc-profile-final/libmad_probe/probe.log).

## 2026-11-02: MP3 volume and speed output probe

The MP3 probe chains the ASND DMA callback and measures actual DMA samples.
Direct full volume and alias volume clamp produced nonzero PCM energy; alias
mute produced zero energy. Normal speed and 1.4x speed produced different
nonzero PCM hashes. Dolphin 2609a passed 19 checks with zero failures. The DOL
SHA-256 is `a812a8c8c30bf6361031d85b7d7e8c3ca127610f8d65324625b8ce612586eb3c`.
See the [suite](docs/dolphin-runs/2026-11-02-mp3-speed-capture-final/suite.json)
and [probe log](docs/dolphin-runs/2026-11-02-mp3-speed-capture-final/mp3player_probe/probe.log).
No physical Wii run was made.

## 2026-11-02: DSP task scheduler probe

The bus probe now uses the AESND DSP mixer fixture and its required mailbox
handoff to exercise `add_task`, `cancel_task`, and `assert_task` through the
direct APIs and wrappers. It verifies task boot, init and done callbacks,
cancellation flags, and task removal. `assert_task` checks the empty-queue
result; active-task preemption remains untested. The first run timed out because
the earlier DSP register checks had consumed the boot mailbox. The final probe
resets the DSP and waits until the boot message is ready without reading it.
Dolphin 2609a passed 32 checks with zero failures. The task microcode and DRAM
buffers are 32-byte aligned. The final DOL SHA-256 is
`2cd036f50c2b265c8e16c80d79e537bb0e134c62ccce63a72e25d241fc536a55`. See the
[suite](docs/dolphin-runs/2026-11-02-dsp-task-scheduler-final/suite.json) and
[probe log](docs/dolphin-runs/2026-11-02-dsp-task-scheduler-final/bus_probe/probe.log).
The initial timeout is retained in
`docs/dolphin-runs/2026-11-02-dsp-task-scheduler/`. User callback delivery
after `assert_int` and physical Wii DSP shutdown remain unverified.

The regenerated inventory contains zero unverified entries in the ten audio
packages. `cargo xtask check-inventory`, `cargo xtask check-group audio`,
`cargo xtask audit-source`, and `cargo fmt --all -- --check` passed. The full
`cargo xtask test-host` suite passed with the required local socket permission.
PowerPC Clippy with warnings denied passed for the `bus_probe` binary. Dolphin
does not verify DSP callback delivery after `assert_int`; physical Wii shutdown
and callback delivery remain outside this evidence.

## 2026-11-03: CARD probe return and EXI address

The C `CARD_Probe` calls `EXI_Probe` and returns its device-presence value.
The Rust facade now forwards that value. CARD register access uses the same
console detection as the EXI driver. A Wii-mode Dolphin run passed the new
return-value check but failed four asynchronous mount checks with a busy
submission. See the [diagnostic](docs/dolphin-runs/2026-11-03-card-probe-parity/README.md).
GameCube mode also reached the probe but did not complete its summary. CARD
mounting and persistence remain open on both paths.

## 2026-11-03: CARD probe status and mount diagnostic

The CARD probe no longer changes the recorded operation status when it returns
a device result, matching the C `CARD_ProbeEx` control flow. Dolphin accepted
the later asynchronous mount in Wii and GameCube modes; both completed with a
no-card result. The Wii probe passed 112 checks. The GameCube run did not reach
its final summary. Card save and persistence checks remain open. See the
[diagnostic report](docs/dolphin-runs/2026-11-03-card-probe-status/README.md).

## 2026-11-03: CARD EXI presence diagnostic

Both Dolphin modes reported the EXI external-device presence bit before the
CARD probe, while the extended probe returned busy and mount completed with no
card. The GameCube run also reported an external interrupt bit. The next
investigation is the EXI probe and device ID transaction. Save persistence and
physical hardware remain unverified. See the
[EXI diagnostic](docs/dolphin-runs/2026-11-03-card-exi-diagnostic/README.md).

## 2026-11-03: CARD save roundtrip in Dolphin

CARD immediate transfers now wait for completion and send longer commands in
pieces. The probe waits for card detection and uses a full sector for writes.
The Wii-mode probe passed 151 checks, including save data and deletion after
remount. The final GameCube probe passed 91 checks, including the save and
remount checks. Physical hardware, card formatting, and removal remain unverified.
See the [run report](docs/dolphin-runs/2026-11-03-card-save-roundtrip/README.md).

## 2026-11-03: CARD raw read and write

The storage probe now calls the raw CARD read and write APIs with an aligned
buffer and compares the bytes read after clearing that buffer. Dolphin passed
152 Wii-mode checks and 92 GameCube-mode checks, including save cleanup and
remount. Physical card behavior remains open. See the
[raw roundtrip report](docs/dolphin-runs/2026-11-03-card-raw-roundtrip/README.md).

## 2026-11-03: DI raw read error checks

The direct DI probe checks that a misaligned read preserves its output and
that a read after close reports the missing descriptor. Dolphin passed 120
checks. Successful reads with a disc and physical Wii behavior remain open.
See the [DI report](docs/dolphin-runs/2026-11-03-di-raw-errors/README.md).

## 2026-11-03: FAT file and mount checks

The focused FAT SD probe passed 39 checks in Dolphin. It checked direct file
create, open, read, write, and length operations, including read-only rejection,
temporary file removal, and a configured mount and unmount. See the
[FAT report](docs/dolphin-runs/2026-11-03-fat-direct-file/README.md). Physical
SD and USB devices remain unverified by this run.

## 2026-11-03: USB storage direct errors

The USB storage probe directly checks facade and system open, close, read, and
write errors without a device. Dolphin passed 49 checks, including unchanged
read-buffer bytes and failed-open cleanup. See the
[USB report](docs/dolphin-runs/2026-11-03-usb-direct-errors/README.md).
Successful USB media transfers and physical device behavior remain open.

## 2026-11-03: USB inquiry error checks

The USB storage probe calls the Rust inquiry API directly and checks null and
unavailable LUN errors without changing the failed-open handle. Dolphin passed
50 checks. Successful inquiry and physical USB behavior remain open. See the
[inquiry report](docs/dolphin-runs/2026-11-03-usb-inquiry-direct/README.md).

## 2026-11-03: CARD close and persistence checks

The CARD probe records the created entry's close result and compares all 512
bytes read after remount. Dolphin passed 93 GameCube-mode checks and 153
Wii-mode checks, including deletion and free-block recovery. Physical card
behavior remains open. See the [run report](docs/dolphin-runs/2026-11-03-card-close-persistence/README.md).

## 2026-11-03: Dolphin blank page audit

An audit found 137 identical green and black screenshots with no readable text.
They do not support visual claims. The Dolphin runner now rejects that blank
page pattern. A diagnostic run detected the pattern on its first page. See the
[audit and file list](docs/review/evidence/screenshot-audit-2026-11-03/README.md).

## 2026-11-03: Console scroll capture

Console scrolling now copies uncached framebuffer rows with aligned word
accesses. The USB storage probe passed 50 checks in Dolphin with five captured
pages and the blank-page check enabled. Its first page contains readable
startup text. See the [run report](docs/dolphin-runs/2026-11-03-console-scroll/README.md).
Physical Wii video behavior remains unverified.

## 2026-11-03: Storage probes with readable page captures

After the console scroll fix, five Dolphin storage probes passed with the
blank-page check enabled. Their first and summary captures were inspected and
contain readable text. See the [run report](docs/dolphin-runs/2026-11-03-storage-readable-pages/README.md)
for counts, logs, captures, and limits. Physical storage remains unverified.

## 2026-11-03: FAT read-only mount

The FAT SD probe now checks that a read-only mount rejects file creation and
leaves the path absent. Dolphin passed 40 checks and unmounted the volume. See
the [run report](docs/dolphin-runs/2026-11-03-fat-readonly-mount/README.md).
Physical media behavior remains unverified.

## 2026-11-03: ISFS async format callback

With a longer bounded wait, Dolphin received the async format callback with
status 0 and verified marker removal after restarting the ISFS service. The
runtime probe passed 88 checks. The prior timeout and physical Wii limit remain
documented in the [run report](docs/dolphin-runs/2026-11-03-isfs-format-async-bounded/README.md).

## 2026-11-03: SD Gecko setter API audit

The SD Gecko probe now calls the C speed and page-size setters on valid slot A,
checks the saved settings through the getters, and restores the original
settings. The final Dolphin run passed 45 checks with no failures. The first
run had one failed assertion because the page-size setter returns a negative
CMD16 result for the absent adapter, not the invalid-drive sentinel. The probe
now checks the negative backend result and passes. The audit maps all 44
`ogc-gcsd` inventory APIs to probe checks. Dolphin has no EXI SD Gecko adapter,
so card metadata, successful CMD16, sector transfers, and physical callback
delivery remain unverified. A fresh post-build rerun stopped at Dolphin's
version preflight with a Qt `neon` processor error, before the DOL ran; the
cause is unknown. See the [per-API audit](docs/dolphin-runs/2026-11-03-ogc-gcsd-api-audit.md),
[final run report](docs/dolphin-runs/2026-11-03-gcsd-setter-api-final/suite.json),
[initial assertion failure](docs/dolphin-runs/2026-11-03-gcsd-setter-api/suite.json),
and [preflight failure record](docs/dolphin-runs/2026-11-03-gcsd-setter-api-reviewed/README.md).

## 2026-11-03: DI legacy descriptor checks

The DI no-disc probe now checks the exported `di_fd` value after Rust and C
initialization and after close. The updated Wii `dvd_probe` DOL built with
SHA-256 `e8ad73da79f591faec307d6afcff4b2a415ef68f45fc9e889f8a733a1e6f6ea3`.
It has no new Dolphin result: the installed Dolphin executable exits with
`SIGABRT` on its `--version` preflight before any probe starts. See the
[DI API audit](docs/dolphin-runs/2026-11-03-ogc-di-api-audit.md), which records
the direct checks and the current emulator limit.

## 2026-11-03: DI asynchronous media reads

The direct-media probe now submits Rust `read_dvd_async` and C
`DI_ReadDVDAsync` requests when it verifies the `TST1TS` ID and sector payload.
It checks callback completion, callback user-data identity, and returned
bytes. The saved synthetic-disc run did not expose that image through `/dev/di`,
so it does not prove this path. The updated Wii `dvd_probe` DOL built from the
prepared PowerPC sysroot with SHA-256
`a90c41cdcc835cf9564dd4b278d6a23b3a36a6dfbb4f65ddc34217a936118d25`. It has
not run: Dolphin still fails during its version preflight, and a supported
direct-DI media setup is needed. See the [DI API audit](docs/dolphin-runs/2026-11-03-ogc-di-api-audit.md)
and [synthetic-media report](docs/dolphin-runs/2026-11-03-di-synthetic-media/README.md).

## 2026-11-03: CARD raw async transfer checks

The GameCube CARD probe submits raw sys `write_async` and `read_async` calls
on the test save. It checks callback completion, status, readback bytes, and
restoration before cleanup. It also updates and restores file status and
attributes through raw sys async APIs. The probe creates and deletes separate
temporary files through raw sys file and directory-entry async calls. It
reopens the directory entry through raw `open_entry` and checks that it
resolves to the created file. The probe checks callback, handle, close, and
removal results. The updated `card_probe` DOL built with SHA-256
`b2dfc0a68d69651c7c6dba4323d2ddb70cf17a1afe8661e3fa7f95fec8aa8e72`. These
checks await Dolphin because the installed executable aborts on its version
preflight. The [CARD API audit](docs/dolphin-runs/2026-11-03-ogc-card-api-audit.md)
maps the direct checks to the Rust sys wrappers and Wii backend entries.

## 2026-11-03: CARD async format probe

The private-card format probe now runs a second cycle through raw sys
`format_async`. It creates and closes a marker file, submits the format call,
checks callback completion and status, then unmounts and remounts the card to
check marker removal and free space. Failure paths try to remove the marker
when the asynchronous operation did not complete. The changed source has not
produced a new DOL: the aggregate build needs the external reference MP3
fixture, and the direct target build needs an uncached Rust source dependency
while network access is unavailable. The installed Dolphin executable also
aborts during its version preflight, before a DOL can run. The
[CARD API audit](docs/dolphin-runs/2026-11-03-ogc-card-api-audit.md) records
these limits and the previous synchronous format runs.

## 2026-11-03: CARD mounted query checks

The GameCube CARD probe now checks the raw sys and facade geometry queries,
free-block count, and serial number against each other on mounted media. It
also checks raw `get_status` and `get_status_ex` results for the created `SAVE`
file and compares the file name, IDs, and length. The GameCube `card_probe` DOL
built with the prepared PowerPC sysroot and `gamecube` cfg. Its SHA-256 is
`adf14800b143535d14cbc99452060111e2482e7f060f8560e91e737a31e6de9b`. The
target build and DOL conversion passed. The DOL has not run: Dolphin still
aborts during its version preflight. The current saved CARD run predates these
checks, so the new assertions have no runtime result. See the [CARD API
audit](docs/dolphin-runs/2026-11-03-ogc-card-api-audit.md).

## 2026-11-03: CARD API evidence audit

The CARD audit maps all 209 inventory rows for `ogc-card`, `ogc-card-sys`,
and `ogc-card-metadata` to their probe or test source. Saved Dolphin runs cover
invalid-channel checks, save persistence and transfers, mount and unmount,
format, and metadata bit helpers. The audit marks raw sys success paths and
media hotplug paths that remain open. The probe adds a valid directory scan
with `find_first` and `find_next`. It also reads, changes, verifies, and restores
the save attributes. The updated GameCube DOL built with SHA-256
`5e63d14e37a430221bbe84ba741afa0462322a061c1e00562ed7320361915ad7`. Both new
checks await a Dolphin run. `cargo test -p ogc-card-metadata` passed 18 tests.
`cargo test -p ogc-card --lib` could not build because its dependencies require
the PowerPC target. A fresh Dolphin preflight failed in both arm64 and x86_64
modes before any DOL ran. See the [CARD API audit](docs/dolphin-runs/2026-11-03-ogc-card-api-audit.md)
and [Dolphin preflight record](docs/dolphin-runs/2026-11-03-card-api-audit-preflight.md).

## 2026-11-03: Storage probe extensions

New checks now cover storage edge paths. DVD reads a full known sector
asynchronously from offset `0x2000`; it also reads the payload sector and the
following zero-filled sector with both synchronous and asynchronous two-sector
requests. The probe checks callbacks, transfer lengths, and every byte. USB
checks live closed handles in the safe facade and C ABI after absent-device
opens. It checks LUN rejection, output preservation, close, flush, reset,
suspend, and start-stop behavior. A null-handle C ABI read also checks that the
caller buffer stays unchanged. SD Gecko checks that a documented null buffer
free does not damage the buffer pool, and that Rust and C APIs reject drive
numbers below and above the valid range while preserving invalid-transfer
buffers. CARD checks positive raw and facade media probes and compares both
`probe_ex` outputs to detected geometry. It also requests one block beyond
reported free space, checks
`CARD_ERROR_INSSPACE`, confirms output and free-space preservation, and checks
that no directory entry remains. It also checks synchronous `create_entry`
and `delete_entry`, plus facade `create` and `delete`. The checks cover handle
close, reopen, status, entry absence, and free-space restoration. CARD checks
use private names and skip a lifecycle if its name already exists.
The inventory checker now accepts an exact public facade call as evidence for
its internal backend entry. This supports SD Gecko mappings where the backend
function name differs from the public API name.

`cargo fmt --all --check`, `git diff --check`, and the full
`cargo xtask build-dolphin-probes` passed. The build used the configured MP3
probe and reference fixtures. A separate build with the prepared PowerPC
sysroot and `gamecube` cfg built and converted the GameCube `card_probe` DOL.
The fresh DOL SHA-256 values are:

- Wii `dvd_probe`: `f2e9d6838a54021cd2fefc61639d01fdb5e6a99859e0acfbcd5e2c98953b1eee`
- `usbstorage_capacity_probe`: `62187ea1bb7f87c243e51db3fa9ef97cafb1af2ff6be1e6dd7e9b60bfdd87817`
- `fat_sd_probe`: `74d5b711abac355a06a8be396ef53fb0bf4fa758b44576553ed9c89dcd4ccb4c`
- GameCube `card_probe`: `d851aa51879ed77ab7efcd36eec89eb0cd77e17914abf265ae6c4c9f30a3a526`

These changed probe checks have source and build evidence only. They do not
have new Dolphin results because the installed Dolphin executable fails its
version preflight before a DOL starts. Existing saved Dolphin logs still prove
the earlier checks described in the individual [DVD](docs/dolphin-runs/2026-11-03-ogc-dvd-api-audit.md),
[USB](docs/dolphin-runs/2026-11-03-ogc-usbstorage-api-audit.md),
[SD Gecko](docs/dolphin-runs/2026-11-03-ogc-gcsd-api-audit.md), and
[CARD](docs/dolphin-runs/2026-11-03-ogc-card-api-audit.md) API audits. Real
USB media, SD Gecko protocol and media I/O, physical CARD behavior, valid Wii
partition operations, and direct DI media reads remain open.

## 2026-11-03: Storage probes run in Dolphin

The existing `cargo xtask dolphin-validate` runner completed the GameCube
CARD, Wii storage, FAT/SD, no-disc DVD, and no-device USB probes with 811
passes and no failures. The CARD, SD Gecko invalid-drive, and USB closed-handle
checks added after the earlier preflight failure now have Dolphin results.
The previous preflight failure occurred in a restricted environment; the
installed Dolphin 2609a executable launched when that restriction was removed.
The run used private profile, NAND, and card copies. Disc reads, USB media,
SD Gecko media, card format, and hotplug were not tested in this run. See the
[run report](docs/dolphin-runs/2026-11-03-storage-unrestricted/README.md) and
its individual probe logs.

## 2026-11-03: Confirm the virtual Wii SD setup

The existing Dolphin runner passed `wiisd_probe` with 15 checks and no
failures. The probe detected the virtual card and verified one-sector and
nine-sector write, readback, and restoration. The prior FAT SD run also
mounted the virtual card and passed file operations. The HBC setup uses Wii SD
folder sync; the probe runner uses its own private virtual SD profile. See the
[setup confirmation](docs/dolphin-runs/2026-11-03-wiisd-setup-confirm/README.md).
USB storage, EXI SD Gecko, and optical media each need their own device or
fixture for positive I/O checks.
