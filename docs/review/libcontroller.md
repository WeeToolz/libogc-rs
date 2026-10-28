# Controller production implementation

## Static parity scope result — 2026-10-15

`ogc-controller` is a Rust-owned manager. `C-sources/libogc` has no
`ControllerManager` API or matching high-level state engine. Its low-level PAD,
WPAD, Bluetooth, USB, and Wiiuse behavior belongs to their separate crate
reviews. The manager's Rust-only state, edge gating, descriptor decoder,
ownership rules, and event ordering have fixed production-engine tests; this
crate's static C parity review is complete within that scope.

Game-specific action maps whose C sources are absent, exact device captures,
and real-device acceptance remain open application and hardware work. They are
not evidence of a C API mismatch in this crate. On 2026-10-15,
`cargo test -p ogc-controller` passed 76 unit tests and 12 HID integration
tests. It did not run on Wii.

## Contract and scope

This is the behavior ledger for the controller project. The crate-level static
review ledger links here. Implemented behavior, independent review, host tests,
Dolphin evidence, and Wii evidence are separate. No entry below has Wii hardware
acceptance from this change.

Target apps: application, autoplay_2048, application, application, sudoku, snake-gc, application,
and HBC-RS. Keep each app's C controls where a C reference exists. Preserve its
intentional Rust changes and existing presets/save behavior. Do not add a binding
editor. Other Rust ports are outside this change.

Current sources: GameCube PAD, Wii Remote and supported extensions,
descriptor-backed USB HID with explicit device profiles, identified USB vendor
drivers, and the Wii's built-in Classic Bluetooth HID path. The Switch protocol
path covers Joy-Con 1 and Switch Pro reports handled by current code. Generic
HID Button usages remain unbound until an exact device profile gives them
physical meaning.

## Behavior ledger

| ID | Contract | Implementation and software verification | Remaining evidence/work |
| --- | --- | --- | --- |
| CTRL-001 | Apps/probes share production controller code | Manager moved to `ogc-controller`; examples and HBC use it; probe path is a re-export; optional `ogc-ui` adapter consumes normalized snapshots | Real-device UI and app acceptance |
| CTRL-002 | Press/hold/release use one transition engine | Shared transition engine and ordered production BT queue; paired samples accumulate after each report; portable queue tests and PowerPC manager regressions | Execute manager regressions and matched real-device sequences |
| CTRL-003 | Disconnect/error releases controls and reconnect changes identity | Shared resets, generation IDs, neutral state, held-at-connect gate; host tests pass | Driver integration hardware tests and event ordering review |
| CTRL-004 | PAD state comes from the successful current scan | Manager scan returns the successful PAD status array; `scan_with_raw` returns it beside normalized state | Sampling, calibration, clamps, removal/reset and WaveBird hardware tests |
| CTRL-005 | WPAD errors and extension changes are explicit | Poll errors recorded; failed samples excluded; source button edges preserved; `scan_with_raw` returns the copied per-channel WPAD sample used for normalization; snapshot metadata reports observed extension, pointer, and motion capabilities | Extension identity changes, orientation, pointer edge behavior and hardware tests |
| CTRL-006 | USB removal preserves buffer ownership and reports failure | Exclusive manager ownership; serialized retirement; pending reads drained before close/reuse; read and close failures retained; portable ownership regressions | Tests at actual driver operation boundaries; Wii removal/reconnect |
| CTRL-007 | USB reports are decoded from descriptors or identified device drivers | Live USB open fetches and validates the selected interface report descriptor; shared production decoder merges partial report IDs; controller application is required; Pokken explicit profile preserved; exact Nintendo `057e:0337` adapter driver exposes four ports; descriptor capabilities remain stable across report IDs | Wii driver boundary and exact device captures; identified generic button bindings; acceptance for all four adapter ports |
| CTRL-008 | Switch output packets have one implementation | Shared `ogc-bt-sync::switch` builders; fixed neutral rumble; fixed byte-vector host tests | Device packet capture |
| CTRL-009 | Switch readiness requires matching acknowledgements | Shared setup builder; matching ACKs, bounded retry, SPI echo validation and calibration required before ready; host state-machine tests | Wii initialization capture |
| CTRL-010 | Known and unknown reports cannot share a guessed layout | Bluetooth unknown families reject guessed layouts; generic USB runtime uses its parsed report map and ignores non-controller IDs | Dedicated 0x3F decoder with device evidence; USB hardware captures |
| CTRL-011 | Switch axes use valid controller calibration | Read-only SPI user/factory setup; applicable-stick validation; production Bluetooth and exact Nintendo USB Switch Pro drivers use the shared setup engine and calibrated mapper with no fixed-center fallback | Real-device initialization, raw reports, and calibration evidence |
| CTRL-012 | Device controls have explicit profile and transport evidence | USB runtime selects controller interfaces from parsed report descriptors; generic physical button mappings remain unset without an identified profile | Add exact VID/PID profiles only with device captures and fixed report fixtures |
| CTRL-013 | External USB Bluetooth LE, HOGP, and Xbox BLE support | Excluded from current work; open for a future project; no support claim | Separate implementation and real-device acceptance project |
| CTRL-014 | Joy-Con 2 and Switch 2 wireless support | Excluded from current work; open for a future project; no support claim | Separate protocol research, implementation, and real-device acceptance project |
| CTRL-015 | UI and game mappings are explicit per app/controller | Source action matrices recorded; HBC normalized mappings, logical labels, and pointer/action arbitration tested; optional UI adapter implemented | Matching C sources for seven games; real-device UI and app acceptance |
| CTRL-016 | Probes expose raw-to-action evidence and failures | Input probe writes JSON event lines from production raw scans and trace callbacks for USB/BT reports, decoded state changes, resets, and errors; probe log write, capacity, open, console, and sync failures are sticky; supplied run ID and DOL hash are validated; partial JSON records are rejected; example action traces and a production UI monitor record app actions and focus targets | Exact device/firmware identity, matched C/Rust logs, and real hardware acceptance |

Transition review correction (2026-10-13): a pending press was previously lost
when its source disconnected before the next snapshot. Reset events now preserve
both accumulated press and release edges, and merged edge accessors include
those reset events. The production transition engine and snapshot behavior have
independent regression coverage. This does not change the held-at-connect gate.

USB integration correction (2026-10-13): generic USB runtime previously used
fixed report guesses after interface enumeration. It now fetches the selected
interface's HID report descriptor, rejects malformed or non-controller layouts,
checks the largest input report against the endpoint and static buffer limits,
and uses the same stateful descriptor decoder tested on the host. Partial
report IDs update only fields they contain. Keyboard and mouse-only report IDs
do not alter controller state. Malformed reports clear decoder state and flow
into the existing invalid-report reset path. HID Button usages have no standard
physical face-button meaning, so the generic profile leaves them unbound; an
identified device profile must provide explicit bindings. No real USB device
test has run.

USB decoder correction (2026-10-13): HID axes X/Y map to the left stick and
Rx/Ry map to the right stick. Z/Rz and slider, accelerator, brake, and steering
values remain in `HidInput` for an explicit device profile. The profile API can
bind button usages by exact VID/PID; it tracks usages even before binding and
recomputes held logical buttons when a profile changes. Hat, D-pad, and button
sources are merged independently so a release from one source does not clear
another held source. A valid non-controller report ID does not establish or
reset a controller. Descriptor and input parser errors remain in the controller
error record. `HidInput.axes` keeps the parser's full signed `i16` values;
canonical `ControllerState` axes clamp to -32767..32767, including PAD and USB
normalization.

Raw scan API correction (2026-10-13): `ControllerManager::scan_with_raw()` now
returns `RawInputFrame` with the normalized snapshot, the PAD status array, and
per-channel PAD and WPAD down/held/up masks plus copied per-channel WPAD data.
The raw mask values pass through the same connection gate as normalized input,
so a held control at connection has no down edge and a disconnect produces
release masks. The raw frame also retains the last observed WPAD extension type
for each channel so disconnect release masks keep the mapping context. The
manager copies each WPAD sample once during its normal scan loop and uses that
same value for both the raw frame and normalized mapping. It does not perform a
second PAD scan or WPAD poll. Hardware runtime behavior is not validated on Wii.
Classic full L/R now contribute SELECT/START normalized edges, and Guitar Hero
3 extension controls map to their HBC action equivalents so those raw actions
can use the manager's held-at-connect edge gate.

Metadata and trace correction (2026-10-13): each normalized snapshot now
provides capability metadata for its physical controller source. PAD and known
Bluetooth controller kinds use explicit capabilities; generic USB capabilities
come from the union of controller fields in the parsed descriptor, and a stick
is reported only when both axes are present. Partial input report IDs do not
change the declared capabilities. WPAD pointer data is present only for
a valid IR sample with finite coordinates and nonzero reported resolution.
Nunchuk, Classic Controller, Guitar Hero 3 and motion capabilities come from
the same WPAD sample that produced normalized state. Guitar Hero 3 stick axes
now use its reported calibration. The manager also offers a synchronous
borrowed trace observer for USB/BT raw reports and normalized states, resets,
and source errors. Observer errors remain visible in the snapshot and do not
reset valid controller state. Callbacks must not re-enter the manager or
perform blocking I/O. The input probe now copies raw USB/BT reports into a
bounded single-producer queue and writes them as JSON lines after the scan.
It writes normalized state changes with PAD/WPAD raw fields, capability and
pointer metadata, resets, and source errors. The probe validates supplied
`--run-id` and `--build-id` arguments. Without
those arguments it assigns a run ID from the Wii timebase and records a null
DOL hash. Unknown device model and firmware remain JSON `null`; those values
are not inferred from a controller name or build.
The probe records controller-layer results. It does not record actions inside
each target app or focused UI widget IDs.

USB composite selection correction (2026-10-13): enumeration now checks every
eligible HID interrupt-IN interface and alternate setting. It sets the listed
alternate setting before reading that interface's report descriptor, skips
valid keyboard/mouse/vendor applications, and selects a parsed controller
application only when its largest input report fits the endpoint packet and
runtime buffer. Descriptor transport failures remain errors. Host tests cover
selection after a non-controller interface, unsupported sets, malformed
descriptors, and selected alternate-setting metadata. Real USB hardware remains
untested.

Generic USB report decoding uses the live descriptor parser, but device-specific
physical button bindings and real USB captures remain open. USB Switch
calibration remains open. The Classic Bluetooth Switch path now requires
validated calibration from the active connection. External Bluetooth LE,
HOGP, Xbox BLE, Joy-Con 2, and Switch 2 wireless are excluded from this work and
have no support claim here.
Report 0x3F currently returns an unsupported result instead of a guessed action.
A valid ACK/status reply does not clear input. A malformed known report resets
controls and appears as an error in the next input snapshot.

Bluetooth family selection correction (2026-10-13): the host and public
controller detector now use the same known-name selector. Product names that
contain `Pro Controller`, `Joy-Con`, or `Xbox` as part of a third-party name
cannot select those vendor protocols. Unknown names remain generic. A known
name is a discovery hint, not proof of manufacturer, model, revision, or
firmware. Exact device identity and protocol captures are still required for
hardware acceptance. Host regressions cover complete names, NUL padding, and
third-party names. The controller and Bluetooth host unit tests and strict
PowerPC library Clippy checks pass.

Bluetooth queue audit correction (2026-10-13): report bytes now use a fixed
field outside the event tag. Queue capacity, connection generations, report
order, and backpressure remain unchanged. The queue still uses no allocation.
This removes the enum-size warning suppression. All 88 Bluetooth host tests
and strict PowerPC Clippy checks pass.

## Part 1 code paths

- `crates/services/ogc-controller/src/model.rs`: controller states, identities,
  snapshot reset/error evidence, and preserved player/snapshot API.
- `crates/services/ogc-controller/src/transitions.rs`: the shared edge/reset engine.
- `crates/services/ogc-controller/src/hardware.rs`: real PAD/WPAD/USB polling and
  Bluetooth sample integration. Hardware APIs are available only on PowerPC.
- `crates/services/ogc-controller/src/reports.rs`: portable report decoding.
- `crates/services/ogc-bt-sync/src/switch.rs`: shared Switch bytes and parsers.
- `crates/services/ogc-bt-sync/src/hid_host.rs`: live HIDP initialization and queue.

The host compiles the actual shared model/protocol engines. It has no substitute
PAD, WPAD or IOS backend. Pending USB buffers retain static lifetime. A retired
slot is not reused while its old request or failed descriptor close remains.

## C reference and deliberate changes

C `PAD_ScanPads` at `../../C-sources/libogc/libogc/pad.c:707` retains old key state
for transient errors. Low-level Rust PAD keeps that behavior. The manager now
requires the successful scan mask and resets its normalized state on errors.
This is a deliberate safety difference at the production manager boundary.

The previous manager accepted a held button as a new press on connection.
The new shared engine exposes held state but suppresses its press action until
release. The previous generic Bluetooth test expected a fixed-layout guess;
its replacement checks explicit rejection because unknown devices need an
identified driver or report descriptor. This does not claim the descriptor
implementation is finished.

Switch formats use the existing project protocol references and the dekuNukem
Bluetooth HID notes. Manufacturer specifications and reverse-engineering
sources remain distinct. Record source revisions and real unit captures before
hardware acceptance. Reading calibration is in scope; firmware/calibration writes
are not in scope.

## Delivery sequence

1. Production extraction, state/reset fixes and shared Switch packet setup.
2. Descriptor-based USB decoding, identified USB drivers and driver-boundary tests.
3. Switch calibration, 0x3F, ordered BT reports and model-specific initialization.
4. Pointer/capability model, HBC/UI integration, and C/Rust action matrices.
5. Add exact USB VID/PID profiles from captures, app action logs, matched C/Rust
   sequences, and real Wii acceptance for current paths.

External USB HCI, LE/HOGP, Xbox BLE, Joy-Con 2, and Switch 2 wireless are
excluded future work. They are not steps in the current delivery sequence.

New UI defaults, where no existing C/app rule applies: 25% direction threshold,
20% release threshold, 350 ms repeat delay, 80 ms repeat interval, no spatial
edge wrap, and cancel wins over simultaneous confirm/cancel. Existing app rules
must remain explicit. Screen-opening inputs are consumed until release; one
activation must reach only one target.

## Verification record

This commit checkpoint records partial implementation and the checks below.
The full controller plan remains open. No controller hardware run has been
performed for this part.

### Earlier checkpoint software checks (2026-10-13)

- `cargo test -p ogc-controller --lib --test hid_descriptor`: the current run
  passes 46 unit tests and 12 descriptor integration tests. The descriptor
  tests import the public production module.
- `cargo test --manifest-path probes/probe-log-state-tests/Cargo.toml`: 5 tests
  passed for the production bounded log state used by the probe.
- `cargo clippy -p ogc-controller --all-targets --target
  powerpc-unknown-linux-gnu --no-deps -- -D warnings`: passed, including
  PowerPC-only manager regressions.
- `cargo clippy -p ogc-system-probe --lib --target
  powerpc-unknown-linux-gnu --features mounted-log --no-deps -- -D warnings`:
  passed. A full probe-binary target check needs the project's panic-abort
  target configuration; the generic target command fails because the prebuilt
  core library uses unwind.
- `cargo test -p ogc-ui --lib`: 12 tests passed, including pointer activation
  ownership and stationary-pointer focus tests.
- `cargo check -p ogc-controller --tests --target powerpc-unknown-linux-gnu`:
  passed. This check compiles the hardware tests; it does not execute them.
- Focused independent source review found USB release ordering, concurrent
  retirement, lost read errors, duplicate device opens and lost paired taps.
  Those findings were corrected. Readiness-time calibration capture also
  prevents a later disconnect from changing an earlier connection event.
  Broader source review and hardware acceptance remain open.

All implementation tasks must use `C-sources` for available C references and
keep packet construction, decoding, transitions, and cleanup in shared
production code. A probe must call that code. The C app sources for the seven
game examples have not been found in this checkout; their C mappings remain
open until the matching sources are available.

### Dolphin test boundary

The current `xtask dolphin-validate` input profile uses an input pipe mapped to
an emulated GameCube controller. `scripts/capture-example-dolphin.py` also has
an emulated Wii Remote smoke-test profile. These profiles exercise the guest
PAD/WPAD path. They do not expose a host controller's USB or Bluetooth reports
to the guest. Neither profile configures USB or Bluetooth passthrough.

USB HID tests need a real device passed through to Dolphin. Native GameCube
adapter input exposed as a GameCube port tests PAD; direct USB passthrough
tests the USB driver. Raw Joy-Con/Switch Pro Bluetooth tests need a compatible
adapter passed through and the real production Bluetooth driver. These tests
remain experiments until a run records the device and transport results.
USB and built-in Classic Bluetooth hardware acceptance remain open. External
BLE/HOGP and Switch 2 wireless are excluded future work, not current delivery.

The fresh input probe was built and run with Dolphin `2603-415`:

- Build: `cargo xtask build-dol ogc-system-probe --bin input_probe --features
  ogc-system-probe/dolphin-automation --output target/xtask/probes-dolphin/input_probe.dol`.
- Run: `cargo xtask dolphin-validate --no-build --probe input_probe
  --artifact-dir target/controller-dolphin-input-20261013`.
- Result: `setup_error` after the 90-second limit, with zero captured frames,
  zero acknowledged pages and no probe transcript. This is not an input pass.
  Startup/runner diagnosis is open. The missing transcript does not identify
  a controller driver defect by itself.
- Evidence: `target/controller-dolphin-input-20261013/suite.json`, the DOL
  manifest and the retained Dolphin logs.

After correcting the runner classification, an isolated 25-second diagnostic
run reported `timeout` and boot stage 3: IOS pre-main completed. The evidence is
in `target/controller-dolphin-input-diag2-20261013/input_probe/`. It still has
zero frames, zero pages and no transcript. The queried PC resolves to scheduler
timeout code; the stop packet gives a different PC in scheduler selection code.
Both raw values are retained. Neither value establishes the cause.

Earlier host validation checkpoint: 86 Bluetooth unit tests, 28 controller unit tests,
12 HID descriptor integration tests, 12 UI tests and 28 xtask tests passed.
The ordered queue is now used by the production WPAD producer and controller
consumer. Queue tests run on the host; hardware-manager tests compile for
PowerPC but have not executed. The current application Wii DOL also builds at
tests, two shared retrace wait tests and the socket-flag conversion test passed.

### Earlier repository audit checkpoint

- Inventory generation passed and records 3,067 APIs. Source anchors were
  refreshed without changing their evidence classifications. The WPAD init
  mapping now names the WPAD probe that actually calls and checks that API.
- The source-structure audit passed with existing tracked keyboard queue debt.
- The runtime completeness audit failed with 2,392 disallowed or unverified
  API entries. This includes missing exact evidence; it does not establish
  2,392 runtime bugs. The failure log is `/tmp/controller-final-audit-stubs.log`.
  No audit rule or pass requirement was removed.
- The role check still reports two existing policy/guard mismatches, in
  `ogc-core` and `wiiuse-rs`. The dependency check still reports the three
  existing upward dependencies from GX, IOS sys and video sys.
- Changed platform libraries compile for PowerPC. The broader PowerPC test
  check fails on existing host-only assumptions in `ogc-system-sys` tests.
  Native Cargo tests for video sys are blocked by PowerPC-only dependencies;
  its shared retrace engine was tested directly instead.

These results permit a source checkpoint. They do not establish emulator
acceptance, real Wii acceptance or completion of the launch device matrix.

### Controller commit checks (2026-10-13)

The example apps now record actions after their production input branches apply
them. The optional `action-trace` capture records raw and decoded controller
events and app actions. The production UI monitor records focus and pointer
targets. A full or busy capture is a failure. These changes do not establish
real-device acceptance.

- Four shared host harnesses pass: 11 input/profile tests, 5 JSON event tests,
  15 application tests, and 2 Sudoku tests. Strict host Clippy passes for each.
- The seven example apps pass 14 strict PowerPC Clippy checks and 14 DOL
  builds: `action-trace`, and `wii-controllers,action-trace`.
- HBC host verification passes 313 tests and strict Clippy. Its PowerPC check
  is blocked by four closure lifetime errors in unchanged
  `src/platform/network_config.rs` at lines 368, 377, 400, and 417.
- Inventory validation passes with 3,125 entries. The source audit passes
  with three existing tracked `wiikeyboard-sys` queue findings.
- The final runtime completeness audit (`cargo xtask audit-stubs`) fails
  with 2,383 disallowed or unverified API entries. This is a repository-wide
  evidence and implementation limit; it is not a count of controller bugs.
  The log is `/tmp/controller-commit-audit-stubs.log`. No audit rule was changed.

The left Joy-Con reset-release regression calls the same mapping helper as
production. It does not construct a full reset snapshot in the host harness.
Real transport removal and release ordering still require the hardware probe.
No real Wii acceptance run was made. Matching C sources for the seven games
remain absent from `C-sources`. The Dolphin startup result above remains open.
