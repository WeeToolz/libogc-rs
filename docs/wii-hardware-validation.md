# Wii hardware validation

The API and feature test register is
[`hardware-api-validation.csv`](hardware-api-validation.csv). It records
expected behavior, probe, hardware requirements, platforms, result, run and
build identity, evidence, and limits. Use `not-tested` until a result has
matching hardware evidence. A successful target build does not count as a
hardware pass.

Run `cargo xtask audit-hardware-register` to check the register schema,
status values, evidence paths, and run/build identities. `cargo xtask verify`
and `cargo xtask verify-complete` include this audit.

The register maps the available system, storage, input, runtime, and
controller probe suites to the APIs they exercise. The `probe` field names
the DOL or DOLs. The `build_or_test_command` field gives the command for the
right build profile. Some monitor and controller probes are not part of the
agent-report build because they need operator input or a mounted log. The
`requirements` and `platforms` fields state when a run can produce valid
evidence. A `passed` or `failed` hardware result must include the
Wiiload run ID, the SHA-256 build ID, and a saved result artifact. Use
`skipped` only when a stated hardware requirement is absent, and record the
reason in `notes`. A missing accessory, media device, or operator action
cannot count as a pass. Rows with no hardware run keep the run and build
identity fields empty and remain `not-tested`, even when their probe builds
or host-side tests pass.

Rows without an existing DOL name a missing probe explicitly and remain
`not-tested`. The bounded WPAD lifecycle DOL checks initialization and
no-controller behavior without starting inquiry or changing saved
controller settings. A separate row tracks live Wii Remote and extension
input; that check needs a Wii Remote.

The Rust WPAD shutdown path now sends an HCI reset before it closes the
Bluetooth transport. The C reference also resets the controller during
`WPAD_Shutdown`. The agent-report DOL checks this cleanup path, but it still
needs a Wii hardware run; a successful build is not a hardware pass.

The probe source paths in pending rows identify the checks that the DOL runs;
they do not prove that the hardware behavior passed. Committed compact reports
in `hardware-evidence/` preserve the screen transcript, checks, run identity,
build identity, and measurements. The latest successful report also has its
WIP1 progress record there. Full PNG, XFB, and WIR2 files stay in each local
`target/wii-runs/<run-id>-<wii-ip>/` folder. For a new run, review the full
report, copy its compact JSON into `hardware-evidence/`, and then update the
register. Do not reuse an older run ID after changing the DOL.

## Capture the physical Wii video output

EasyCap Studio is available at `/Volumes/SSD/easycap-studio`. Its Rust source
is in `src/`, and its CLI is `target/release/easycap-studio`. Use this CLI
to capture the physical Wii output during a hardware debug run. The device
uses direct USB access; it is not a generic macOS camera.

Check for the capture card before a run:

```sh
/Volumes/SSD/easycap-studio/target/release/easycap-studio list
```

The supported USB IDs are `1c88:003c` for capture and `1c88:0007` for the
bootloader. No listed device means capture is unavailable. The CLI capture
commands do not load firmware. If the device needs firmware, use the
separate EasyCap Studio firmware workflow. Do not add an automatic firmware
upload to a Wii debug run. Only one application can claim the USB interface;
stop GUI capture before using the CLI.

Create an evidence directory, then save a screen image or a raw frame:

```sh
mkdir -p target/wii-runs/capture
/Volumes/SSD/easycap-studio/target/release/easycap-studio snapshot \
  --output target/wii-runs/capture/screen.png --timeout 15
/Volumes/SSD/easycap-studio/target/release/easycap-studio raw-frame \
  --output target/wii-runs/capture/screen.uyvy --timeout 15
```

For a startup failure or a freeze, start a bounded recording in a separate
Terminal before the existing Wiiload launcher:

```sh
/Volumes/SSD/easycap-studio/target/release/easycap-studio record \
  --output target/wii-runs/capture/video.mkv --seconds 60 --no-audio
```

The examples use the default composite input. Use `--input s-video` only
when the physical connection uses S-Video; that input remains experimental.
Capture is native NTSC at 720x480. The recording time limit includes device
setup. Omit `--no-audio` when audio evidence is required, but treat audio
synchronization as unverified until a physical synchronization test passes.

Retain capture logs and review the saved files. Place the evidence in the
matching Wiiload run directory and record the run ID, DOL SHA-256, capture
settings, and observed result. A video can show a black screen, a frozen
display, or a failed return to the System Menu. It does not establish an
internal probe result. A moving frame counter alone does not establish a
successful reset. Keep the structured probe report and the capture together.

The operator confirmed that capture from the physical Wii worked during
EasyCap Studio debugging. The card was disconnected during the device check
for this guide update. The documented commands have not yet been run as part
of a combined Wiiload and capture session.

## AESND stream and USB transport DOLs

The focused hardware suites now include `aesnd_native_probe.dol` and
`usbstorage_transport_probe.dol`. The AESND suite checks real DSP output during
four stream callback cases, stop, and reset. The USB suite checks capacity and
sector bytes across reset, suspend/automatic resume, and close/reopen. It
requests no raw sector writes. Its close path still checks the required cache
flush result. A logger-owned USB mount is rejected before reset or suspend.

Build with `cargo xtask build-probes`, or use `cargo xtask build-agent-probes`
for report-enabled copies. Read
[the physical probe instructions](../probes/README.md#physical-aesnd-stream-and-usb-transport-coverage)
before a run. Record the exact DOL SHA-256, device identity, prerequisites,
full transcript, failures, and cleanup results. The new rows remain
`not-tested`; no real-hardware run of these new cases has been recorded.

These DOL cases exercise production APIs with real sample buffers and physical
USB reads. They do not turn controlled host failure tests into hardware passes.
Internal cursor edge cases and injected transport failures retain their
separate unit-test evidence.

## application controller checks

On 2026-09-29, the operator confirmed that application controls worked on the
physical Wii after the SI transfer length correction in commit `3412487`.
The tested DOL SHA-256 was
`6c39c9091b249709d63f21427d3afaca8acba6c8e703099f21c31ae9728414d5`.
The Terminal upload log was
This is an operator observation of GameCube input in application. It does not
verify every PAD API, every button, recalibration, or wireless controllers.

On 2026-10-08, the operator confirmed that application sound effects were audible
while playing on the physical Wii with a GameCube controller. The tested DOL
SHA-256 was
`a86a24f2aa910ac24cc5e955713af11930cade797a557d79d884eb8b54aac865`.
The Terminal upload log was
sound output from this application build. It does not complete the separate ASND
API probe, which checks sound API and DMA behavior and still requires its own
hardware report.

application now enables the existing USB HID controller backend. Menu, game,
and pause controls accept buttons from all connected gamepads, so a
GameCube controller does not prevent a USB controller from controlling
the human player. USB report mapping uses Switch and generic report
decoders plus a decoder for the HORI Pokken Tournament pad (`0f0d:0092`).
The Pokken decoder uses the documented button order and separate hat byte
from [Nintendont's controller report](https://github.com/FIX94/Nintendont/issues/240).
Arbitrary HID report descriptors are not parsed by this backend. USB
controller acceptance remains pending a hardware test.

The operator reported a black screen for the USB-enabled application build
with SHA-256 `82fd568fb97310bc709b7c419ba9ef124844e9191fa1f4784210b00a528e44eb`.
This is a failed USB startup result. The next build displays stages before
USB initialization, discovery, device open, descriptor reads, configuration,
interface selection, and interrupt reads. These stages are screen output;
they are not automatically saved by the upload-only Terminal launcher.
The C reference initializes `/dev/usb/hid` alongside `/dev/usb/ven`, while
the earlier Rust HID discovery path fell back to `/dev/usb/oh0`.

The subsequent operator test reached normal application startup with no USB
controller attached, then froze when the Pokken controller was inserted.
This narrows the failure to discovery, setup, or report polling after attach.
Controller report polling now submits asynchronous interrupt requests using
permanent aligned buffers. Pending requests return immediately to the game
loop; completed reports are copied before their buffers can be reused.
The next operator photo stopped at `USB DEVICE LIST BEGIN`, before device
setup or report reads. The USB layer now starts separate vendor and HID
hosts, keeps a device snapshot for each host, and reads modern device lists
from those snapshots. It uses the HID descriptor buffer layout and native
byte order from the C reference. HID control and interrupt requests use the
HID host; interrupt requests use its direction field. Discovery does not
fall back to the old OH0 host while a modern host is present.

Portable regression tests cover host selection, descriptor offsets, and
interrupt request fields. Device setup still uses synchronous operations.
These changes require another hardware test with the Pokken pad connected;
USB input is not yet verified.

## Reset to Homebrew Channel

On 2026-09-29, repeated runs of `video_probe.dol` on the Wii at `10.0.0.242`
ended with the Wii back at the Homebrew Channel. The probe run used
`SYS_RESTART` through `ogc_system::reset_system`; the host recorded
`recovery_boot_status: returned_to_hbc` for runs `1790704947-634616000` and
`1790705128-483762000`.

This is partial real-hardware evidence for the `reset_system(SYS_RESTART)`
path. Both runs timed out without receiving a probe report, so they do not
confirm report delivery, every reset mode, or all reset callbacks.

## Complete report smoke runs

On 2026-09-29, the Mac Terminal launcher received complete reports for runs
`1790713566-628716000` and `1790713982-112712000`. These runs supplied the
report endpoint, run ID, and DOL hash through HBC arguments. The reports
confirmed that `SYS_Init` and IOS pre-main completed. They also supplied the
screen transcript, one boot progress record, the final `WIR2` report, and a
640 × 480 framebuffer. The host saved the raw XFB and PNG without an SD card.

The upload endpoint went offline and then returned in both runs. This is
stronger evidence for `reset_system(SYS_RESTART)` returning to HBC than an
endpoint check made only after a timeout. It does not validate other reset
modes or every callback.

The first complete reports also exposed a video timing fault. After a long
startup delay, `wait_v_sync` used a deadline that was in the past. Repeated
waits returned immediately. Run `1790713982-112712000` measured an average
of 20 timebase ticks per wait, which rounded to zero microseconds.

## Startup order and injected operations

The early `SYS_Init` stage list now drives the target operation dispatch.
Host-injected tests check the Wii and GameCube branch order, IPL memory
protection branch, completion mask, and behavior when the SRAM read fails.
They also confirm that GameCube skips the Wii IPC-buffer step. An SRAM
failure is recorded at stage 6; later startup operations still run. These
tests exercise the stage runner that drives the target calls, while the
physical Wii report checks the real hardware operations.

Run `1790717999-867357000` tested this startup dispatch on the Wii. It
reported completion mask `0x7ff`, failed stage 0, completed all seven IOS
pre-main stages, passed all 16 video checks, saved all artifacts, and
returned to HBC. This confirms the Wii branch on hardware. The GameCube branch
has host-injected ordering tests but no target build or hardware run. These
targets create DOL applications, so the production `SYS_Init` path selects
the non-IPL sequence. The IPL branch is covered only by the injected tests.

## Frame wait and VI retrace

The timebase deadline fix prevented waits from returning immediately after a
long startup delay. A later change replaced timebase pacing with the VI IRQ:
the IRQ handler clears VI interrupt status, advances the retrace counter,
calls retrace callbacks, and applies a queued framebuffer. `wait_v_sync`
returns only after that counter changes. The video probe checks the counter,
callback order, active buffer pointer, and VI register readback.

Hardware run `1790714280-559707000` tested DOL SHA-256
`36e4c8784c68a94f417cea5e6837b2acfcc07c73d40548c7060952e22506494e`.
Its result is in
`target/wii-runs/1790714280-559707000-10.0.0.242/result.json`.

| Measurement | Result |
| --- | --- |
| Probe summary | 5 passes, 0 failures |
| Warmup / sample waits | 30 / 120 |
| Average / minimum / maximum wait | 16,666 / 16,666 / 16,666 microseconds |
| Probe elapsed time | 2,750,038 microseconds |
| Screen transcript | 1,737 bytes, not truncated |
| Framebuffer | 614,400 bytes, 640 × 480 |
| HBC endpoint went offline | 1,056 milliseconds after upload |
| HBC endpoint returned | 10,140 milliseconds after upload |
| Host run duration | 39,548 milliseconds |
| Missing artifacts | None |

Repeat run `1790714355-254102000` used the same DOL hash. It also passed
with 5 passes and 0 failures, saved every artifact, and measured the same
frame times. The HBC endpoint returned after 10,146 milliseconds. The host
completed the run after 40,969 milliseconds.

Run `1790716156-972790000` tested the VI IRQ and framebuffer changes with DOL
SHA-256 `a0aee52d791e18f54178683ece9d0de0b71d9088bc7b9a70527331837f6230a1`.
The probe passed 16 checks with no failures. It recorded 120 waits at an
average of 16,683 microseconds and passed VI counter, callback order,
framebuffer register, and current framebuffer pointer checks. The host saved
all artifacts, and the HBC endpoint returned after 10,265 milliseconds.

This is real VI IRQ and two-buffer swap evidence for this NTSC Wii. The VI
register setup now selects the matching C-reference timing table for NTSC,
PAL, MPAL, EURGB60, and progressive modes. Host tests cover mode-to-table
selection. PAL output still needs a physical PAL-mode test. GameCube timing
still needs a target configuration and hardware test.
Repeat run `1790716372-284239000` passed the same 16 checks with the same
16,683 microsecond average and returned to HBC with all artifacts saved.

The platform timing helper uses the C reference timer clocks: 60.75 MHz for
Wii and 40.5 MHz for GameCube. Its host tests check NTSC-family 59.94 Hz and
PAL 50 Hz retrace periods, tick-to-microsecond conversion, and mode-to-register
table selection. A source review found that Rust had assigned EURGB60 mode IDs
to MPAL and had not accepted EURGB60 IDs 20 through 22. The table now follows
the C switch exactly, including rejection of unsupported debug interlace IDs.
The video probe also measures eight VI waits and checks the average against
the selected mode's field rate with an 8 percent tolerance. That new runtime
assertion has not run on hardware yet. The agent probe selects its timer
conversion from the detected console. Wii run `1790716665-753631000` confirmed the detected Wii
clock still reports an average 16,683 microseconds per retrace. Run
`1790717111-879991000` and `1790717353-434934000` tested the mode-table
change on the NTSC Wii. Both runs passed all 16 video checks, measured an
average of 16,683 microseconds per wait, saved all artifacts, and returned
the HBC endpoint. The second run confirms a new upload after the first run
returned to HBC. These runs do not verify PAL output or GameCube timing on
hardware.

Comparison with the C video implementation found that progressive modes
write VI control value `0x0005` and enable VI register 54. Rust now uses those
values and reports progressive modes as non-interlaced. Host tests cover
NTSC, PAL, EURGB60, and progressive mode IDs. Run `1790718459-601434000`
was an NTSC regression run with 16 checks. Run `1790718839-655245000` also
checked the reported scan mode. It passed all 17 checks, measured an average
retrace wait of 16,683 microseconds, saved every artifact including one
identity-verified startup progress record, and returned to HBC. PAL and
progressive output have not been checked on a display.

## Progress report transport

Run `1790719069-587573000` tested an experimental TCP close-acknowledgment
step. The host saved the WIP1 startup record, but it did not receive the final
WIR2 report. The host timed out after accepting one Wii connection. A later
readiness check could not reach the Wii at `10.0.0.242:4299`.

The current code removes the blocking acknowledgment read and keeps the Wii
network stack active between the progress connection and final report. The
target build passed, but this transport change still needs a hardware run
because the Wii upload endpoint is not reachable. Do not count run
`1790719069-587573000` as a complete report pass.

These runs provide evidence for the tested startup, argument, report,
framebuffer, and HBC return paths. They do not validate all libogc-rs APIs.
Earlier runs of the same smoke DOL also timed out. The later successful
runs do not establish the cause of every earlier failure. Keep the timeout
connection diagnostics when checking further hardware runs.

## Return-to-HBC verification

The Rust runtime now checks the `STUBHAXX` signature at `0x80001804` after
`main` returns. `SYS_Init` registers an exit callback. On a valid signature,
the callback runs the Rust `SYS_SHUTDOWN` cleanup and branches to the loader
stub at `0x80001800`. If the signature is missing, it uses the existing
`SYS_RESTART` fallback. Probe runs can also request this return after sending
their report.

On 2026-09-29, direct-return run `1790715431-877839000` reported the stub
present, the host observed the upload endpoint go offline and return, and a
subsequent DOL upload passed as run `1790715560-220414000`. Injected-failure
run `1790715489-642815000` reported 5 passes and 1 deliberate failure, sent
all artifacts, and returned the endpoint after 10,248 milliseconds. A
passing run after that failure also completed as
`1790715560-220414000`.

Run `1790715637-617531000` tested the separate `main` return path. Its
transcript records `exit_path=hbc_reload_stub status=present
return=main_dispatch`; the final report arrived and the upload endpoint
returned after 10,160 milliseconds. This verifies both explicit probe exit
and dispatch after `main` returns on this Wii. It does not test the
stub-missing restart fallback on hardware.
