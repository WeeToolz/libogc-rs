# Wii runtime probes

Runnable target diagnostics live outside production crates under `probes/`.

Probe packages and focused DOLs:

- `abi-probe` — target object layout and ABI extraction;
- `ogc-system-probe` — aggregate focused probes for startup, buses, storage, input, networking, audio, and callbacks;
- `lwp-probe` — Milestone 1 scheduler, synchronization, timer, deferred-callback, Tuxedo, and IOS interrupt-wakeup stress.
- `libmad_probe` — deterministic full-rate and half-rate `libmad-rs` synthesis checks with on-screen pass/fail output.
- `libmad_rs_decode_probe` — pure Rust Layer III decoding of an embedded MP3 and finite ASND playback of the decoded stereo PCM.
- `aesnd_native_probe` — native DSP output, stream callback replacement, ONCE/LOOP playback, same-slot voice replacement, stop, and reset.
- `usbstorage_transport_probe` — physical USB reads, reset, suspend/automatic resume, and close/reopen without raw sector writes.

Probes call production implementations only. They must not contain replacement hardware models.

Build only the Milestone 1 probe with:

```sh
cargo xtask build-lwp-probe
```

Build the standard target probe set with:

```sh
cargo wii-test
# or
cargo xtask build-probes
```

The LWP artifact is:

```text
target/xtask/probes/lwp_probe.dol
```

Build the same LWP probe with the actual Wii std synchronization types with:

```sh
cargo xtask build-wii-std-probe
```

This command prepares the pinned Rust source and writes
`target/xtask/probes/wii_std_probe.dol`. It requires the matching nightly and
rust-src. The normal probe also checks the production futex provider. See
[synchronization status and limits](../docs/wii-std-sync.md). A build or link
does not mean that either probe has passed on Wii.

Build the standard probe DOLs, including the libmad smoke probe, with:

```sh
cargo xtask build-probes
```

The libmad artifact is `target/xtask/probes/libmad_probe.dol`. It synthesizes
fixed synthetic subband samples through the pure Rust `libmad-rs` polyphase
filter at full and half rate, then checks PCM checksums and synthesis state on
the Wii console. Building the DOL does not validate its output on hardware.

The MP3 artifact is `target/xtask/probes/libmad_rs_decode_probe.dol`. It decodes
the embedded 44.1 kHz stereo MP3 with the pure Rust `libmad-rs`, checks the
frame metadata and fixed-point checksums against upstream `libmad-c`
`FPM_DEFAULT` results, then plays the first second of decoded PCM
through ASND. The screen reports decoder and audio backend results. A passing audio backend check shows
that ASND accepted and completed the voice; a person must confirm that sound
was audible on the Wii setup.
The Wii `FPM_DEFAULT` path uses 32-bit `long` arithmetic for its rounding
additions. The host C reference uses 64-bit `long`. The Rust probe selects the
matching rules on each target. The installed Wii archive reports `FPM_PPC`,
which is another arithmetic profile and produces different hashes. The older
`C-sources/libogc/libmad` fork also has a different long IMDCT. It is not the
source reference for this port. The R3 MP3 probe names the upstream reference
on screen. The standalone C `PROFILE=fpm-default` helper now compiles that
same upstream source.

The video probe checks both freshly allocated XFBs after an uncached clear
and a flush through the cached alias. This catches dirty allocation cache
lines that would overwrite pixels drawn through the uncached alias.

The LWP probe initializes the standard visible console and persistent logger. It attempts `/tmp/ogc-system-probe.log` and the existing mounted-log backend. It also installs the scheduler debug hook, drains the fixed trace ring, and reports counters.

The debug hook can execute with interrupts masked. Probe hooks must use only bounded atomic writes; normal logging occurs later in application context.

Real-Wii execution is required for acceptance. Record the full log, hardware/loader details, and final counters when reporting results.

## WPAD lifecycle probe

Build the report-enabled probe with:

```sh
cargo xtask build-agent-probes
```

The DOL is `target/xtask/probes-agent/wpad_probe.dol`. It checks WPAD
initialization, enabled state, empty-channel errors, and shutdown. It does not
start Bluetooth inquiry, pair with a controller, or change saved controller
configuration. Shutdown sends an HCI reset before it closes the transport,
matching the controller reset in C `WPAD_Shutdown`. A Wii Remote is not
required for this lifecycle check; the Wii must have IOS Bluetooth available
and a reachable host report listener.
This probe does not validate live button or extension data.

## Dolphin regression validation

Dolphin runs provide emulated regression evidence. They do not replace the
real-Wii acceptance requirement above.

Build the automation variants and run all focused probes with the installed
Dolphin application:

```sh
cargo xtask build-dolphin-probes
cargo xtask dolphin-validate --no-build
```

Run one probe with:

```sh
cargo xtask dolphin-validate --no-build --probe arena_probe
```

The runner uses `/Applications/Dolphin.app/Contents/MacOS/Dolphin` by default.
Use `--dolphin PATH` or `DOLPHIN_BIN` to select a different executable. Each
probe gets a separate Dolphin user directory with a copy of the Wii NAND from
`~/Library/Application Support/Dolphin/Wii`. The source must contain the Wii
System Menu. Use `--nand-dir PATH` or `DOLPHIN_WII_NAND` to select another Wii
NAND directory. Dolphin writes only to each run's copy. The runner removes that
copy after it saves the probe log. Results, probe logs, Dolphin logs, and all
PNG frame dumps are under `target/dolphin-validation/`.
Each result lists `probe.log`, the readable transcript, and
`probe-transport.bin`, the exact bytes received from the emulated USB Gecko.
Keep both files when an automated run fails. The binary file preserves the
transport data for later parsing without losing bytes to text decoding.

The automation build mirrors the guest transcript through Dolphin's emulated
USB Gecko in memory-card slot B. The runner reads its local TCP endpoint and
writes the exact transcript to `probe.log`. This path does not depend on ISFS,
an SD image, OCR, or Dolphin's host log categories. At every console page
boundary the guest holds the page for 60 emulated frames and sends its
framebuffer address. The runner pauses Dolphin through the built-in GDB stub,
reads the stable XFB, converts Wii YCbCr 4:2:2 pixels to RGB, and writes
`pages/page-NNN.png`. Therefore `page_capture_count`, `page_frames`, and
`pages_valid` prove that every acknowledged page has a readable, non-uniform
image. A uniform image is a setup error. `final_frame` points to the last page
PNG. The transcript summary is the probe pass/fail authority.

The runner uses normal emulation speed (`Dolphin.Core.EmulationSpeed=1.0`),
enables the Dolphin Wii SD card and write support, disables host audio output,
and uses a private GDB port for each probe. It does not use Dolphin's movie
frame dumper because movie mode enables deterministic emulation and Dolphin
then rejects Wii network sockets. The numbered GDB/XFB page PNGs provide the
required visual evidence without that restriction.

The SD settings make Dolphin's raw Wii SD interface available. The automated
storage probe skips GameCube EXI SD adapters and the operator-driven hotplug
soak, but it still tests Wii SD sector access and FAT file operations. Dolphin
can pass through allow-listed host USB devices, but this runner does not attach
host hardware. It does not opt a real mass-storage device into a test that can
write to media.

Dolphin's current `/dev/net/ip/top` model accepts protocol zero for stream and
datagram sockets, but rejects explicit TCP and UDP protocol values. The runtime
uses protocol zero where only the default protocol is required. The network
probe keeps its explicit-protocol tests so this Dolphin limitation stays
visible. Other completed failures, such as SRAM snapshot parity, audio timing,
font ROM decoding, debugger listen setup, FAT file operations, or missing
media, remain probe or emulator coverage evidence. They are not reported as
host setup failures.

The `input_probe`, `hid_probe`, and `ipc_async_probe` DOLs use a bounded
600-frame result mode only when the `dolphin-automation` feature is active.
Normal probe builds keep their live dashboards. The input automation uses a
Dolphin pipe controller and does not use a host controller. Dolphin validation
does not enable host USB passthrough and never enables destructive card format.

Input JSON Lines are formatted by the same bounded, allocation-free formatter
used by `input_probe`. A record that exceeds its buffer or cannot reach the
configured logger is reported as a probe failure. Source and trace errors are
logged and count as failures. Raw reports are written before decoded events
from the same scan. Run the formatter and log failure tests with
`cargo test --manifest-path probes/input-event-line-tests/Cargo.toml` and
`cargo test --manifest-path probes/probe-log-state-tests/Cargo.toml` from
`RustPorts/libogc-rs`.
Run identity validation tests with `cargo test --manifest-path
probes/input-identity-tests/Cargo.toml`. With valid agent-report `--run-id` and
`--build-id` arguments, the run event uses those values and the build ID as
`dol_sha256`. The run ID must be 1 to 64 ASCII bytes, and the build ID must
be 64 hexadecimal characters. Without valid arguments, the run event uses a
time-based local run ID and records unknown DOL hash, device model, and
firmware as `null`.

The suite fails for a build error, setup error, crash, timeout, missing or
malformed summary, invalid frame output, or a nonzero probe failure count. The
default suite uses `arena_probe` as its startup smoke gate. If that probe cannot
complete IOS initialization, the runner keeps its final frame and stops before
it starts the remaining probes.

A complete run can fail when Dolphin does not emulate a peripheral or behavior
that a probe requires. Treat those failures as emulator regression evidence,
not as real-Wii acceptance results. Use `--probe NAME` to rerun one result and
inspect its `result.json`, `probe.log`, `dolphin.log`, and final PNG together.

The normal storage probe does not format memory cards. To run its destructive slot-A format check, use a sacrificial card and pass the exact DOL argument `--destructive-card-format=ERASE-SLOT-A`. This erases all data on that card.

The USB storage probe writes and synchronizes a dedicated close-persistence file, unmounts and deinitializes the transport, remounts it, and verifies the file before cleanup. A failed unmount retains retryable transport state and stops that probe path.

## PAD recalibration coverage

Build the complete logged PAD probe set with:

```sh
cargo xtask build-pad-probes
```

The output under `target/xtask/probes-logged/` contains these grouped probe
families:

- `pad_state_probe` for masks, queuing, deterministic SI failures, lifecycle,
  non-controller types, and cleanup;
- `pad_command_spec{0,1,2,4,5}` for command selection, legacy behavior,
  origin transformation, the X patch, and the low-memory disable bit;
- `pad_wired_probe` for real ports, analog controls, polling and rumble
  isolation, empty sockets, unplug behavior, and multi-controller sequencing;
- `pad_wavebird_probe` for connected, receiver-only, and reconnect paths;
- `pad_stress_probe` for initialization overlap, repeated operations, and four
  concurrent LWP callers;
- `pad_reset_probe` for the registered reset queue and the optional terminal
  reset smoke test.

Run hardware DOLs from SD or USB. Each DOL replaces its own
`libogc-pad-*.log`, opens it before the first scenario, and synchronizes it at
each checkpoint. Copy logs from each required hardware setup before a repeated
run replaces them. Dolphin automation writes the same probe transcript to the
host runner log through USB Gecko because a directly loaded DOL has no mounted
guest launch device.

For complete Wii evidence, use four wired controllers first. Follow the
on-screen prompts to test analog origins, two-controller isolation, rumble,
unplug, empty-socket, and reconnect behavior. Then run the WaveBird DOL with a
linked controller and receiver-only state. Run the stress DOL for its full
30-minute and 10,000-operation limit. The reset DOL checkpoints its log before
it offers the terminal reset.

After you collect the logs, validate the aggregate directory with:

```sh
cargo xtask validate-pad-logs PATH
```

The validator writes `pad-coverage.json`. It rejects missing scenarios,
explicit failures, missing summaries, missing hook evidence, unmatched
scenario boundaries, missing checkpoints, or dropped PAD trace events.

## Physical AESND stream and USB transport coverage

Build the normal DOLs for on-screen use:

```sh
cargo xtask build-probes
```

Build the report-enabled DOLs for the existing Wiiload/report workflow:

```sh
cargo xtask build-agent-probes
```

Use these files from `target/xtask/probes/`, or their report-enabled copies from
`target/xtask/probes-agent/`:

- `aesnd_native_probe.dol` runs the existing native DSP output, metrics,
  pause, reset, and reinitialization checks. It then runs four stream cases:
  buffer replacement that retains ONCE; replacement that retains LOOP;
  restart as LOOP from the callback; and free/reallocate of the same voice
  slot from the callback. Each case must first produce the initial source's
  fixed PCM values. Loop cases must then produce the replacement source's
  different fixed PCM values. ONCE must stop without playing the replacement.
  Explicit stop must prevent new voice callbacks and produce silence while
  AI DMA still runs. Reset must disable DMA and stop callbacks.
- `usbstorage_transport_probe.dol` requires a supported USB mass-storage
  device. Keep it attached for the complete run. The probe uses an exclusive
  raw handle and requests no sector writes or filesystem mounts. It reads
  capacity and sector zero twice, then performs three reset/suspend/resume
  cycles. Each cycle must preserve capacity and the full sector contents.
  Close and reopen must release resources and return the same data.
  Production close also sends the required cache-flush command; a flush
  failure is a failed cleanup check.

Run the USB DOL over Wiiload, or use the default build from SD. A logged build
launched from USB can mount that device for its logger. The transport suite
rejects this condition before reset or suspend. It does not unmount someone
else's filesystem to acquire the device.

The AESND input and replacement buffers have permanent 32-byte-aligned storage.
Callbacks use atomic counters and production setters; they do not log, allocate
sample storage, edit private voice fields, or supply artificial memory addresses.
The PCM checks inspect the real DSP output supplied to the audio callback.
Audible sound remains a separate human observation. Loop-output checks do not
replace the unit test for the internal cursor-reset edge case.

The report-enabled AESND DOL uses the standard system/IOS startup and the
Wiiload report arguments. It sends a checkpoint screenshot before each console
page clears. Each named test step sends a small progress record. Wiiload saves
the images and cumulative logs in
`target/wii-runs/<run-id>/checkpoints/`; `checkpoints.html` shows the
received pages. A checkpoint is progress evidence. The final report contains
the total passed and failed checks and the descriptions of failed checks.
DSP sample and callback failures also include their observed counts.
After IOS startup, the probe initializes and configures the report network
before its first checkpoint, reuses that connection state for later pages, and
closes it after the final report.

Each `STEP` record includes the real VI frame count, its increase since the
previous step, and the current pass/fail totals. The screen corner shows the
live VI frame count (`F`), the activity spinner, and seconds since the last
logged progress. Screenshot transfer failures count as probe failures. The
host must receive the final summary before it can rate the full test run.
Progress, page screenshots, and the final report share one TCP socket with
`TCP_NODELAY`. The socket stays open until the final report or a transfer error.
Page transfers use
nonblocking socket writes and one sixty-second deadline per packet. The sender
checks partial writes, transport failures, and socket close.
On Wii, page rollover prepares the next page in the other XFB and switches VI
after its page label is ready. The VI callback and live frame counter remain
active across the switch. This keeps the previous text visible during the
page change.
XFB snapshots use aligned, volatile 32-bit copies. Byte-based copies corrupt
uncached Wii pixel pairs. Page rollover occurs before the next text write, so
a frame wait does not erase the last page.
Run `cargo test -p xtask probe_` for the report and XFB regression checks.
Pass `--return-to-system-menu` as a DOL argument to request the Wii Channels
menu after the final report is sent. A request record does not prove that the
menu opened. The screen shows report cleanup and reset callback stages to
locate a blocked exit. Confirm the menu on the real Wii. This option takes
priority over returning
to HBC. Set `WII_PROBE_RESTART_AFTER=0` for `run-wii-probe.command` in this mode,
so the host does not wait for HBC's upload port to reopen. A failed report
keeps the probe on its results screen.

The USB suite logs IOS version/revision, Hollywood revision, device ID, VID/PID,
capacity, operation results, and reset duration. Record the console model and
hardware revision with the saved transcript and the exact DOL SHA-256. Every
`STEP` is printed before its operation so a stalled request can be identified.
The production USB transport owns request timeouts; this probe cannot cancel a
synchronous IOS operation from outside that transport. Audio observation uses a
fixed frame count and the existing bounded VI wait.

A missing USB device, invalid capacity, failed operation, or failed cleanup does
not count as a pass. The prerequisite or operation is recorded as FAIL; dependent
cases are UNTESTED. Controlled transport failure injection remains host-test
coverage. These probes do not simulate USB failure or intentionally disconnect
a device during a request. All new hardware acceptance rows remain `not-tested`
in `docs/hardware-api-validation.csv` until a real run is recorded.
