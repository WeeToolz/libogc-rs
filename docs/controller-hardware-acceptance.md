# Controller hardware acceptance

## Status and evidence

Hardware acceptance is pending. Host tests and DOL builds do not establish that
a device works on a Wii. Use the production input probe and the target app for
each device. Keep Wii runs separate from Dolphin runs.

Use [the device inventory](../config/controller-devices.csv) to identify the
test targets. Use [the behavior ledger](review/libcontroller.md) for open
implementation work and [the action matrices](controller-action-matrices.md)
for expected app actions. These documents do not certify an unidentified unit.

For each physical unit, record:

- Manufacturer, product name, rear-label model, and hardware revision.
- Firmware version, or `unknown` with the reason it could not be read.
- Connection method, cable, receiver, and adapter model and revision.
- Wii model, IOS, loader, and relevant configuration.
- For Dolphin, its version, input configuration, and actual passthrough setup.
- Run ID, test name, DOL SHA-256, build manifest, and source commit.

Archive the transcript, JSON events, transport logs, and device identification
with the run. Preserve failed runs. An absent log, truncated capture, source
error, or failed cleanup prevents a complete pass.

## Run preparation

Build the input probe with the project's `cargo xtask build-dol` command. Use
the `mounted-log` feature when a writable mounted log is available. Use the
`agent-report` feature when the run uses the agent capture transport. Archive
the generated DOL manifest and verify the hash of the DOL that is loaded.

Supply `--run-id=<unique-run-id>` and `--build-id=<64-hex-DOL-SHA256>` through
the loader's argument mechanism. The input probe records a timebase run ID and
a null DOL hash when these arguments are absent. A null identity is incomplete
acceptance evidence; it is not a guessed value.

The app action capture has a 128 KiB limit. Raw report traffic can fill it during
a long run. Use separate short runs for individual tests and archive each run,
or use a probe with a checked durable log. A full or busy capture is a failure;
do not accept the retained prefix as a complete event sequence.

Check that the probe uses the production `ogc-controller` manager. A decoder
fixture or a host input emulator cannot replace this run. Record raw USB and
Bluetooth reports where the production transport exposes them. PAD and WPAD
records contain the actual copied scan data and button masks, rather than USB
or Bluetooth packet captures.

## Common test sequence

Run the same sequence in the probe and in each target app:

1. Start with all controls released. Record detection, type, slot, generation,
   and capabilities.
2. Press and release each button once. Check press, hold, release, and action.
3. Test a quick tap, a long hold, and repeated directions. Record timing.
4. Test likely combinations, including confirm with cancel. Check that one
   input cannot activate both a pointer target and the focused widget.
5. Move each stick to neutral, each direction, and each corner. Record raw and
   normalized values. Check off-center neutral behavior and dead zones.
6. Move each analog trigger from released to fully pressed. Test its digital
   click separately when the device has one.
7. Where a pointer exists, test center, each edge, off-screen, and invalid
   pointer states. Record coordinates, resolution, and target.
8. Open and close menus, dialogs, pause screens, and error screens. Check that
   the opening button does not activate the next screen while it remains held.
9. Disconnect while a button and a direction are held. Check the release/reset
   record and the app result. Reconnect while still held; check that no new
   press action occurs until release and a fresh press.
10. Check all errors and cleanup results before assigning a verdict.

Record detection, raw input, normalized events, mapped actions, app behavior,
and hardware testing as separate results. A failure in one layer must remain
visible even if another layer passes.

## Source-specific checks

| Source | Required additional evidence |
| --- | --- |
| Native GameCube PAD | Official wired controller, WaveBird receiver, and each chosen third-party model tested separately; scan errors, sampling, reset, stick and trigger calibration recorded. |
| Wii Remote | Upright and sideways behavior, pointer validity and edges, reconnect, and exact Remote revision recorded. |
| Nunchuk and Classic Controller | Each extension identified separately; extension removal and replacement during the run, calibration, all sticks and triggers tested. |
| Generic USB HID | Exact VID/PID and interface, saved descriptor, endpoint details, raw reports, and explicit physical-button profile. Descriptor parsing alone does not prove physical bindings. |
| Nintendo GameCube USB adapter | Exact `057e:0337` device; initialization completion, all four ports, port removal, adapter removal, reconnect, and simultaneous input. Fixed-center normalization does not prove per-unit calibration. Clones need separate evidence. |
| Switch Pro USB | Exact `057e:2009` device; completed USB setup, matching acknowledgements, read-only calibration, report mode, timeout/error behavior, and reconnect. Enumeration alone is not readiness. |
| Switch Pro Classic Bluetooth | Discovery name and actual unit identity, control/interrupt traffic, matching setup responses, calibration reads, disconnect and reconnect. |
| Joy-Con 1 Classic Bluetooth | Left alone, right alone, and both together; rail buttons, per-half identity, calibration, pair assignment, removal of either half, and reconnection. Record whether the app sees separate halves or a logical pair. |
| Other Classic Bluetooth HID | Exact model, revision, firmware, supported transport, report format, and physical mapping. A familiar name is only a discovery hint. |

### Wii Remote connection hold probe

Build `wii_remote_stability_probe` with `cargo xtask build-dol ogc-system-probe
--bin wii_remote_stability_probe`. It waits up to 90 seconds for a Remote, then
checks it for three minutes through the same `ControllerManager` WPAD scan path
used by the examples. During the hold test, press and release a button every
15 seconds. The report records missing scans, disconnects, reconnects, button
edges, and Bluetooth host state. Any missing scan fails the continuous-hold
check.

Run this DOL on a physical Wii for Wii acceptance. The probe reports the
Hollywood revision, but that value does not identify Dolphin. A Dolphin run
can help diagnose software behavior; it cannot pass the physical-Wii
acceptance row. The probe checks the shared Remote input path. It does not
replace an input-mapping smoke test in each example.

### Reconnect and in-game pairing status

The shared example manager now enables Wii Remote support by default, begins
with guest pairing, and repeats discovery during gameplay. It accepts a new
incoming Remote during guest search and keeps an outgoing connection attempt
for the same address intact. Host tests cover the guest PIN and competing-link
cases. All nine application/example DOL targets built with Wii Remote support.

These changes have not passed a new real-Wii test. The prior Wii run did not
reconnect its Remote. A DOL build, host protocol test, or Dolphin run is not
hardware acceptance. Run both of these checks on each supported Wii revision:

1. Start an app with an already paired Remote. Record time from DOL launch to
   the first valid input report. Disconnect it during play, then reconnect it
   after restoring power or replacing its batteries. Confirm input returns and
   that disconnect releases held buttons.
2. While another Remote remains connected and the game is active, press SYNC
   or 1+2 on an unpaired Remote. Confirm it connects, receives a distinct player
   slot, and sends input without dropping the first Remote.

Hold both connections for at least three minutes and send a button press every
15 seconds. Save the final screen, run ID, DOL hash, Wii model, board revision,
and raw report. Until this run passes, quick reconnect and hot pairing remain
unverified.

Report `0x3F` has no current support claim. Do not treat a deliberate unsupported
result as a passing input test. External USB HCI, BLE/HOGP, Xbox BLE, Joy-Con 2,
and Switch 2 wireless are excluded future work.

## App and C comparison

For HBC, test its original PAD/WPAD bindings and the documented normalized
controller mappings. Include screenshot, notice dismissal, page movement,
options, layout, and Home/Back behavior. Record whether an action was dispatched
and whether its backend operation completed; these are different results.

For application, autoplay_2048, application, application, sudoku, snake-gc, and application,
preserve the documented Rust bindings. Matching original C game sources are
absent from the current C reference checkout. Record Rust behavior and leave
the C comparison pending until those sources and runnable C builds are supplied.

For a UI probe, record the actual focused widget ID, pointer target, and widget
action from the production adapter and tree. Record game movement and other
non-UI actions inside the game. An input probe alone cannot prove app mapping.

## Dolphin boundary

An emulated GameCube port checks PAD behavior. An emulated Wii Remote checks
the emulated WPAD path. Neither proves a physical Wii USB or Bluetooth path.
A USB adapter exposed as an emulated GameCube port does not test the USB driver.
Document actual USB or Bluetooth passthrough if it is used. Repeat claims about
Wii hardware on a real Wii.
