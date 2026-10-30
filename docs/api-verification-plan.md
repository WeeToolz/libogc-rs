# API verification plan

## Scope and baseline

The initial 2026-10-29 inventory had 2,400 `unverified` entries in 67 packages. The
[generated inventory](generated/implementation-inventory.csv) is the item-level
work list: use crate, source path, symbol, and line together to identify an
entry. An entry is an evidence gap, not a confirmed defect. Counts are a
snapshot; regenerate and check the inventory after each work batch.

The declared classes are 917 compatibility forwarders, 651 hardware, 539 IOS,
133 protocol, 100 pure, and 60 scheduler entries. Classification is a starting
point for selecting a test, not proof that the operation is complete or safe.
Review the API contract, C reference where applicable, implementation, and
existing test or probe before selecting cases.

## Work order

1. **Establish the test environment.** Fix or diagnose the runtime probe's
   timebase and STM failures recorded in [development results](../DEVELOPMENT_RESULTS.md).
   Do not weaken these checks. Preserve the failed run and compare any new run
   with its recorded DOL and configuration. Review the open libmad Dolphin
   failure in [evidence summary](evidence-summary.md) before relying on it.
2. **Pure and protocol paths.** Start with `ogc-runtime`, `ogc-ui`, `mad-sys`,
   `ogc-controller`, `tinysmb-sys`, `ogc-fat`, and `iso9660`. Identify host
   harnesses that compile production logic. Check fixed input and output bytes,
   limits, malformed inputs, state changes, and errors. Use independent expected
   values. For target-only code use an existing target probe when host execution
   cannot exercise the production path. Review existing ISO and SHA probe results
   before adding repeated checks.
3. **Wrappers and shared backends.** Group forwarders by production operation.
   Exercise each public API with distinct arguments and observable results,
   including backend failures and output buffers. Share test fixtures, but do
   not count an engine test as evidence for a wrapper it never calls. Prioritize
   the largest groups: `ogc-gx`, `ogc-system`, `ogc-network`, and `ogc-usb`.
4. **IOS, scheduling, and device paths.** Expand the registered focused probes
   by operation: initialization and teardown, valid results, rejected input,
   transport errors, callbacks, and final completion. Cover `ogc-lwp-driver`,
   system/IOS services, input, storage, network, audio, and GX in bounded batches.
   Use Dolphin for target execution where it supports the operation; record
   unsupported or failed cases as open. Use the real Wii for hardware behavior.
   For HBC integration, use its `HBCI` network input on the Wiiload listener:
   `WeeToolz/hbc-rs/tools/hbc-input.py` sends button presses, holds, and
   releases. See [HBC network input](../../hbc-rs/docs/network-controller-input.md).
   Check the observed HBC action as well as the input reply; the reply confirms
   frame consumption, not action completion. This input ends when HBC transfers
   control to another DOL and does not exercise a physical controller. Use a
   suitable app-side input path and physical controller when those are required.
5. **Persistence and risk.** Test memory-card, FAT, NAND, and other persistent
   paths with controlled media and failure injection first. Check bytes after a
   new mount or restart and ensure cleanup errors are visible. Plan hardware
   probes around reversible operations and scratch data. Before physical work,
   follow the workspace `DEVELOPER.md`; persistent firmware writes require its
   applicable safety requirements and the workspace firmware policy.
6. **Close each batch.** Run its focused host tests and target probes, retain
   logs and run identity outside `target`, and document the limits. Add evidence
   rows only for checks that actually call the exact public API and assert its
   behavior. Refresh the inventory, run source and completeness audits, and
   investigate new findings rather than suppress them. Commit each reviewed batch.

## Evidence rules

The current [evidence registry](../config/implementation-evidence.csv) requires
a matching classification and a uniquely named evidence function that directly
calls the listed symbol. `pure`, `protocol`, and `compatibility-forwarder`
entries accept a host test or a probe source; `hardware`, `ios`, and `scheduler`
entries require a probe source. The registry checks source structure, not that
the check ran or proves its claim. Record actual results separately. A skipped
check, a target compile, or a DOL launch is not a pass. Hardware claims need a
real-Wii probe with production APIs and observed results; a Dolphin pass does
not replace it. Track blocked fixtures, unsupported operations, hardware
revisions, recovery requirements, and failures as open work.

For each batch record: exact entries and contract; existing evidence; cases and
independent expectations; test or probe source; commands and results; target,
DOL hash and run ID where relevant; failure and cleanup outcomes; and remaining
hardware limits. Use the existing [hardware register](hardware-api-validation.csv)
and [probe coverage map](../config/probe-coverage.csv). Keep durable run
findings in `DEVELOPMENT_RESULTS.md` and evidence outside `target`.

## Package work list

Counts below are the number of unverified inventory entries at the baseline.
Each package includes all of its listed entries, including repeated symbol names
in different files. Assign individual entries to a batch from the inventory;
do not infer API coverage from a package or probe name.

| Group | Entries | Packages and entry counts |
| --- | ---: | --- |
| core | 468 | `ogc` 1, `ogc-cache` 28, `ogc-conf` 32, `ogc-es-sys` 72, `ogc-ios-sys` 10, `ogc-ipc` 35, `ogc-irq` 13, `ogc-irq-sys` 10, `ogc-lwp` 33, `ogc-lwp-driver` 58, `ogc-lwp-sys` 61, `ogc-runtime` 22, `ogc-std-runtime` 10, `ogc-stm` 14, `ogc-sysbase` 19, `ogc-wiilaunch` 16, `tuxedo-sys` 34 |
| storage | 356 | `iso9660` 3, `ogc-card` 28, `ogc-card-sys` 70, `ogc-di` 55, `ogc-dvd` 20, `ogc-dvd-sys` 21, `ogc-fat` 17, `ogc-filesystem` 29, `ogc-gcsd` 43, `ogc-usbstorage` 19, `ogc-usbstorage-sys` 47, `ogc-wiisd` 4 |
| input | 518 | `lwbt-sys` 40, `ogc-bt-sync` 132, `ogc-controller` 67, `ogc-input` 20, `ogc-input-sys` 28, `ogc-si` 22, `ogc-si-sys` 28, `ogc-usb` 59, `ogc-usb-events` 1, `ogc-usb-sys` 43, `wiikeyboard-sys` 4, `wiiuse-rs` 1, `wpad-sys` 73 |
| network | 233 | `lwip-sys` 32, `ogc-network` 49, `ogc-network-sys` 92, `ogc-wd` 21, `tinysmb-sys` 39 |
| audio | 243 | `aesnd` 31, `aesnd-sys` 21, `asnd-sys` 20, `mad-sys` 46, `modplay-capi` 24, `mp3player-sys` 19, `ogc-aram` 26, `ogc-audio-sys` 22, `ogc-dsp` 15, `ogc-dsp-sys` 19 |
| debug | 254 | `libdb-sys` 10, `ogc-system` 76, `ogc-system-sys` 105, `ogc-usbgecko` 7, `ogc-video` 22, `ogc-video-sys` 34 |
| hardware | 301 | `ogc-exi` 27, `ogc-exi-sys` 14, `ogc-gx` 260 |
| ui | 27 | `ogc-ui` 27 |

## First batch

The first controller model batch added host evidence for 18 inventory entries.
A second batch recorded 14 controller model, transition, and HID APIs.
A third batch added Xbox report boundary and Bluetooth stage text checks.
The current inventory reports 2,335 unverified entries. See the
[development results](../DEVELOPMENT_RESULTS.md) for the test scope and limits.


Start with the `ogc-controller` button and snapshot APIs in `model.rs`. Review
the existing tests, then cover bit insertion and removal, player assignment,
capacity, and merged button transitions with independent expected values.
Inspect `ogc-runtime` separately: many inventory entries are trait method
declarations, so identify their production implementors and determine how the
evidence registry should represent contract versus implementation coverage.
Do not use a test double as proof of a production implementation. Diagnose the
two failed runtime probe checks alongside the next implementation batch.
Do not mark the initial SHA or ISO probe as an overall pass: the recorded probe
reported two failures even though the relevant checks passed.
