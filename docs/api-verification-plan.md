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

1. **Establish the test environment.** Review the runtime probe's
   timebase and STM results recorded in [development results](../DEVELOPMENT_RESULTS.md).
   Do not weaken these checks. Preserve the failed run and compare any new run
   with its recorded DOL and configuration. Review the resolved libmad Dolphin
   checksum profile and earlier failures in
   [evidence summary](evidence-summary.md) before relying on them.
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

The optional `api_path` field records an exact Rust call path or function-pointer
field for an evidence row. The inventory checker confirms that the named
function binds or calls that path and calls the listed symbol. Rows without
this field still use the older symbol-name check. Add `api_path` when a probe
uses callbacks, aliases, or generic operation helpers so that the evidence
points to the intended API.

For each batch record: exact entries and contract; existing evidence; cases and
independent expectations; test or probe source; commands and results; target,
DOL hash and run ID where relevant; failure and cleanup outcomes; and remaining
hardware limits. Use the existing [hardware register](hardware-api-validation.csv)
and [probe coverage map](../config/probe-coverage.csv). Keep durable run
findings in `DEVELOPMENT_RESULTS.md` and evidence outside `target`.

## Wider audio group status

The current audio batch covers `aesnd`, `aesnd-sys`, `asnd-sys`, `mad-sys`,
`modplay-capi`, `mp3player-sys`, `ogc-aram`, `ogc-audio-sys`, `ogc-dsp`, and
`ogc-dsp-sys`.
Use the generated inventory for current per-symbol counts; the package counts
in the initial audio work list are a baseline and can change as evidence is
accepted. `ogc-audio-sys` has a focused Dolphin probe. Its callback registration
and removal are checked, but callback delivery is not accepted from Dolphin:
the AI DMA byte count did not advance and neither AI callback ran during the
emulated wait. Track callback delivery for a real-Wii run. All current audio
inventory entries have accepted Dolphin or host evidence. The sync-only libmad
decoder message has host evidence.
These inventory results are Dolphin or host evidence; they do not imply that
the APIs passed in this batch were tested on Wii hardware. A separate
application build played audible sound effects on a physical Wii, as recorded
in [the Wii hardware log](wii-hardware-validation.md), but that observation
does not validate individual audio APIs.
The earlier Dolphin checksum mismatch came from using host `FPM_DEFAULT`
goldens in a PowerPC `FPM_PPC` probe; the probe now uses the repeatable
PowerPC hashes. No fixture-dependent decoder failure is counted as evidence.

The current generated inventory has zero open entries in the wider audio
group. The MP3 player probe passed 19 checks in Dolphin 2609a. It checked
direct and alias initialization, invalid arguments, buffer and reader playback
to completion, looped playback, pause and resume through ASND voice state,
error state, and cleanup. A chained AI DMA callback sampled actual DMA buffers.
Full volume and
the alias clamp produced nonzero PCM energy, while mute produced zero energy.
Normal speed and 1.4x speed produced different nonzero PCM hashes. The run used
DOL SHA-256
`a812a8c8c30bf6361031d85b7d7e8c3ca127610f8d65324625b8ce612586eb3c` and a
fixed 8,777-byte MPEG Layer III fixture. See
`docs/dolphin-runs/2026-11-02-mp3-speed-capture-final/` for the report and logs.

The ARAM and ARQ probe passed 21 checks in Dolphin 2609a. It checked DMA
write/read round trips, callback delivery, allocation and free, request
completion and queue removal, manager push/pop, initialization failure state,
and reset/reinitialization. The DOL SHA256 was
`3bd9d9f755e8abc408e861929cf6b11fe9c2291cc22ba7760e47e8ee236a64bd`. See
`docs/dolphin-runs/2026-11-02-aram-verified/`. Dolphin validates the emulated
hardware path; a physical Wii run has not been recorded.

The DSP control probe passed 26 checks in Dolphin 2609a. It checked mailbox
messages and status bits, DSP halt and unhalt control bits, interrupt control
bits, volatile task fields, callback replacement and restoration, DMA status,
and all four shutdown progress stages. Its DOL SHA-256 was
`3e2510b7597394e7bd1989be99873475e2250092d352b4a3163fb9cc39919ec6`. See
`docs/dolphin-runs/2026-11-02-dsp-complete/`.

A second bus probe now exercises the task scheduler with the repository's
AESND mixer code and mailbox handoff. It checks task boot, init and done
callbacks, cancellation flags, task removal, wrapper null rejection, and the
empty-queue result from `assert_task`. Dolphin 2609a passed 32 checks with zero
failures. Its final DOL SHA-256 is
`2cd036f50c2b265c8e16c80d79e537bb0e134c62ccce63a72e25d241fc536a55`. See
`dolphin-runs/2026-11-02-dsp-task-scheduler-final/`. The first task run
timed out because the preceding mailbox probe had consumed the DSP boot
message; the corrected probe resets the DSP and leaves its boot mailbox ready
for `add_task`. Active-task preemption through `assert_task` remains untested.
A diagnostic run also did not deliver the registered user callback after
`assert_int`, so callback delivery remains unverified. Dolphin shutdown passed;
physical Wii shutdown and callback delivery remain untested.

| Package | Open entries |
| --- | ---: |
| `aesnd` | 0 |
| `aesnd-sys` | 0 |
| `asnd-sys` | 0 |
| `mad-sys` | 0 |
| `modplay-capi` | 0 |
| `mp3player-sys` | 0 |
| `ogc-aram` | 0 |
| `ogc-audio-sys` | 0 |
| `ogc-dsp` | 0 |
| `ogc-dsp-sys` | 0 |
| **Total** | **0** |

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

## Current storage audit

The current inventory has 604 API entries in 14 storage packages. Its 0
`unverified` entries mean that every API has an implementation-source mapping;
they do not mean that every API has passed a test or a Dolphin run. These
counts come from `docs/generated/implementation-inventory.csv`. Exact call
paths are now recorded for 550 entries: 9 `iso9660`, 27 `ogc-dvd`, 32
`ogc-dvd-sys`, 59 `ogc-di`, 30 `ogc-gcsd`, 63 `ogc-card`, 50 `ogc-card-sys`,
56 `ogc-card-metadata`, 5 `ogc-wiisd`, 20
`ogc-usbstorage-lifecycle`, 30
`ogc-usbstorage`, 65 `ogc-usbstorage-sys`, 64 `ogc-fat`, and 40
`ogc-filesystem`. Exact source mapping does not prove runtime behavior. Keep
runtime evidence in the run reports and validation register. The storage build
group has 15 runtime crates; one has no inventory API rows. The 56
`ogc-card-metadata` entries have host test evidence. The 18 crate tests check
identifiers, completion values, metadata validation, commit order, failure
injection, readback, and state preservation. These tests do not verify CARD
IOS behavior on Dolphin or Wii.

| Package | Inventory API entries |
| --- | ---: |
| `iso9660` | 9 |
| `ogc-card` | 63 |
| `ogc-card-metadata` | 56 |
| `ogc-card-sys` | 90 |
| `ogc-di` | 59 |
| `ogc-dvd` | 27 |
| `ogc-dvd-sys` | 32 |
| `ogc-fat` | 64 |
| `ogc-filesystem` | 40 |
| `ogc-gcsd` | 44 |
| `ogc-usbstorage` | 30 |
| `ogc-usbstorage-lifecycle` | 20 |
| `ogc-usbstorage-sys` | 65 |
| `ogc-wiisd` | 5 |
| **Total** | **604** |

Use the package counts above to split storage work into these feature batches.
All entries have source mapping. Each batch still needs runtime evidence for
every API entry. A batch count does not mean that each API has a test or probe.

| Batch | Packages | API entries |
| --- | --- | ---: |
| Optical disc | `iso9660`, `ogc-di`, `ogc-dvd`, `ogc-dvd-sys` | 127 |
| GameCube memory card | `ogc-card`, `ogc-card-metadata`, `ogc-card-sys` | 209 |
| FAT and file-system services | `ogc-fat`, `ogc-filesystem` | 104 |
| SD card transports | `ogc-gcsd`, `ogc-wiisd` | 49 |
| USB mass storage | `ogc-usbstorage`, `ogc-usbstorage-lifecycle`, `ogc-usbstorage-sys` | 115 |
| **Total** | **14 packages** | **604** |

The previously open `ogc-di` APIs now have direct source checks in
`storage_probe`: the configuration setters preserve no-disc status, init
callbacks do not fire for no-disc mounts, and partition calls return the
closed-descriptor error through Rust and C entry points. The updated DOL builds
with SHA-256
`37547912e2242cde337056fcfa16312ff8d23d62d0fafcf7699cbf7004465350`. These
checks have source evidence. A later storage probe passed in Dolphin 2609a with
97 checks and 0 failures, including the available no-disc checks. Partition
success with media and init-callback delivery on a successful mount remain
untested. The hardware register remains `not-tested` for DI.

The original `ogc-di` split grouped the APIs as follows:

| Probe group | APIs | Count |
| --- | --- | ---: |
| DI configuration and callbacks | `load_dvdx`, `use_cache`, `set_init_callback`, and their C exports | 6 |
| DI partition lifecycle | `open_partition`, `close_partition`, and their C exports | 4 |

The CARD APIs have direct probe-source evidence. The probe checks
invalid-channel errors and output preservation through the wrapper and system
APIs, no-callback submission errors, owned request slots and completion state,
save persistence, and user-data-safe cleanup. An earlier Wii `storage_probe` ran
without an available card and skipped save and format operations. A separate
GameCube `card_probe` run used a copied, verified virtual card and passed 93
checks. It verified create, read, write, status, delete, persistence after
remount, and cleanup. A later CARD accessor run passed 98 checks with the
updated DOL. It checked direct sys and wrapper calls, owned async completion,
raw transfers, persistence after remount, delete, and cleanup. See the
[CARD accessor report](dolphin-runs/2026-11-03-card-accessor-update/README.md)
for the DOL hash and logs. See the
[CARD audit rerun](dolphin-runs/2026-11-03-storage-api-audit-rerun/README.md).
This proves these operations on Dolphin's virtual card, not on a physical
GameCube card. The CARD hardware register remains `not-tested` for physical
hardware. The metadata invalid-count change has host test evidence. Earlier
sandboxed Dolphin attempts stopped before launch. The CARD accessor run used
host access and passed. That result does not cover APIs in the other probe DOLs.

The Dolphin runner can copy a raw card image from `LIBOGC_DOLPHIN_CARD_IMAGE`
into its isolated profile. An earlier attachment diagnostic used this option,
but CARD returned `CARD_ERROR_NOCARD` and skipped save operations. That run
adds no successful CARD operation evidence. See the
[attachment diagnostic](dolphin-runs/2026-11-03-card-image-template-dolphin/README.md).

The storage probe uses exclusive creation for fixed FAT probe names and skips
the CARD save checks when `SAVE` already exists. It removes a probe file only
after this run created it. A previous `storage_probe` run passed in Dolphin
2609a with 111 checks and 0 failures. It skipped operations that require absent
USB or memory-card media. It does not prove successful USB transfers, memory-card
save operations, disc reads, or physical device behavior. Earlier failed runs
remain historical evidence in their run directories.

The current Dolphin suite passed `card_probe` with 98 checks, `storage_probe`
with 217, `fat_sd_probe` with 40, and `dvd_probe` with 178. Each reported zero
failures. The copied memory card and virtual Wii SD paths ran. USB transfers,
SD Gecko success paths, DVD reads, destructive format, and media hotplug were
not tested. See the
[current storage-suite report](dolphin-runs/2026-11-03-storage-api-full-current/README.md)
for DOL hashes, logs, and limits.

A repeat at revision `43b800a` mounted the verified card copy and completed
the save and remount checks in both GameCube and Wii modes. The probes reported
98 and 217 passes, respectively, with zero failures. See the
[CARD repeat report](dolphin-runs/2026-11-03-card-current-repeat/README.md)
for the logs, DOL hashes, and limits.
The later [full-buffer CARD run](dolphin-runs/2026-11-03-card-full-buffer/README.md)
checks all bytes returned by the 512-byte owned async read and the full safe
sync sector read. It passed in both Dolphin modes.

The protected DI read group has direct Rust and C calls. It returned `-5` for
DVD key, physical data, and report-key requests, and `0` for BCA in Dolphin.
The run had no disc, so the BCA result does not validate BCA data contents.

The direct DI no-disc Dolphin probe passed 119 checks. It verifies Rust and C
entry points for mount state and callback delivery, drive identification,
disc-ID errors, read argument errors, drive controls, and close/reopen cleanup.
The latest run also compares C and Rust outputs for drive identification,
support, disc ID, cover status, configuration, copyright, protected reads,
BCA, and report-key calls. A no-disc partition-open attempt did not return in
an early run, so partition-open behavior with a mounted descriptor remains
unverified. No readable disc was present, so successful sector and partition
reads and physical-drive behavior remain open. See the [final DI Dolphin
run](dolphin-runs/2026-11-03-di-wrapper-parity-final/README.md).

The ISO9660 probe calls its three C ABI exports and the Rust file-system
mount, label, and unmount facade directly. A controlled in-memory disc image
checks file lookup, open, read, seek, close, stat metadata, and root directory
changes through both the C mount and the Rust mount. The latest Dolphin
`runtime_probe` passed 97 checks with no failures. It includes the lower-level
Rust C wrapper mount, label, and cleanup checks. The probe exposed and fixed
the device name used for devoptab lookup and the handling of valid negative
pointer-shaped file handles. This does not verify a physical optical drive.
See the [ISFS and ISO9660 follow-up](dolphin-runs/2026-11-03-isfs-iso-followup/README.md).
The focused DVD probe checks `/dev/di` no-disc status, rejected mount,
asynchronous mount, inquiry, block-safe read, drive-control, GCM
offset, stream-stop, and seek operations, command-block state, pause and reset
state, invalidation state restore, shutdown, spin-up, and stop-motor calls in
Dolphin. Its synchronous control, seek, and stream-stop calls return `-1`; the
asynchronous forms report the cover-closed error. Dolphin inquiry succeeds
without a disc. These checks do not verify disc reads or physical drive motor
behavior. See the
[DVD Dolphin run](dolphin-runs/2026-11-03-dvd-no-media-final/README.md) and
[DVD async no-media run](dolphin-runs/2026-11-03-dvd-no-media-async/README.md).
The probe calls all 32 `ogc-dvd-sys` APIs directly and checks their no-disc
results, callbacks, output state, and cleanup. The current `dvd_probe` passed
178 checks in Dolphin 2609a, including these direct sys calls. The run had no
disc. Successful disc reads and physical drive behavior remain unverified.
See the
[current storage-suite report](dolphin-runs/2026-11-03-storage-api-full-current/README.md).
The ISFS probe checks file-system API errors while the `/dev/fs` service is
closed. It checks service close and reopen, async errors for invalid
descriptors, output preservation, and that format requests do not reach IOS
while the service is closed. Separate Dolphin runs check successful sync and
async file operations against a private NAND copy. Dolphin returns
`ISFS_EINVAL` for sync and async file rename when the basename changes; the
probe checks that the source stays and the destination stays absent, then
removes both paths. Sync and async rename of an empty directory passed in
Dolphin, with moved-path state, callback result, and cleanup checks. Successful
file rename on Wii remains unverified. See the
[directory rename report](dolphin-runs/2026-11-03-isfs-directory-rename/README.md).
The bounded async-format run reports callback status 0
and removal of its marker state; the earlier run without callback completion
was superseded by that result. The current probe now also checks synchronous
format after creating marker state, restarting the service, and verifying the
marker paths are absent. The latest Dolphin `runtime_probe` log reports 99
checks with no failures. It includes the format cycle, directory renames, and
new checks that sync and async file bytes persist after close and reopen. Its
summary capture was black; the log and readable check pages are in the
[ISFS and FAT reopen report](dolphin-runs/2026-11-03-isfs-fat-reopen/README.md).
An earlier run
exposed that the preceding async format had removed `/tmp` from the private
NAND copy; the probe now recreates that parent and records the setup and
format results. Earlier sandboxed launches stopped before startup. These runs
do not prove physical Wii NAND behavior. The probe also checks that
`get_stats_async` returns
`ISFS_EINVAL` with `/dev/fs` closed, preserves all seven output fields, and
does not call its callback. See
the [rename result run](dolphin-runs/2026-11-03-isfs-rename-state/README.md),
[async file cycle](dolphin-runs/2026-11-03-isfs-successful-async-cycle-dolphin/README.md),
[async format diagnostic](dolphin-runs/2026-11-03-isfs-format-async-dolphin/README.md),
[ISFS closed-service run](dolphin-runs/2026-11-03-isfs-get-stats-async-pass/README.md),
and [earlier ISFS run](dolphin-runs/2026-11-03-filesystem-isfs/README.md).
The USB storage safe facade probe covers no-device errors and
lifecycle cleanup. A focused Dolphin probe also checks that raw `flush` and
safe `try_flush` return the interface error for a null handle. The run has no
USB device, so successful cache synchronization on a mounted LUN remains
untested. The same probe checks the LUN count bounds, read residue, capacity
decoding, BOT retry behavior, and READ/WRITE status handling. The focused FAT
probe passed 41 checks in Dolphin and covers the mounted SD API, C exports,
FAT file seek/truncate, attribute updates, init functions, and mount cleanup.
It also verifies truncated file bytes after unmount and remount, then removes
the file. See the [ISFS and FAT reopen report](dolphin-runs/2026-11-03-isfs-fat-reopen/README.md).
The `wii_std_fs_probe` passed 99 Dolphin checks on the private virtual Wii SD
card. It checked directory and path operations, file I/O, metadata, attributes,
timestamps, read-only access, remount persistence, and cleanup. This verifies
many FAT operations on Dolphin; it does not verify each FAT inventory entry or
physical Wii media. See the
[Wii FAT run](dolphin-runs/2026-11-03-wii-std-fs-storage/README.md).
The `fat_sd_probe` also checks that GameCube SD Gecko mounts report the
expected no-adapter error. Its earlier 2026-11-03 Dolphin run passed 36
GameCube SD API checks. The storage audit rerun passed 40 `fat_sd_probe`
checks, including the SD Gecko checks. These checks cover invalid-drive
results for C and Rust APIs, no-card results, speed and page-size state
restoration, buffer-pool exhaustion and reuse, and interface callback cleanup.
They do not exercise an EXI SD Gecko. See the
[earlier SD Gecko run](dolphin-runs/2026-11-03-gcsd-invalid-drive/README.md)
and the [storage audit rerun](dolphin-runs/2026-11-03-storage-api-audit-rerun/README.md).
The Wii SD probe passed 15 checks in Dolphin with its private virtual SD card.
It checks startup, insertion, sector-zero read, a one-sector write/read-back
round trip, a nine-sector unaligned write across the bounce-buffer boundary,
read-back, restoration of all original sector bytes, the zero-sector no-data
path, shutdown, and restart. The non-zero write tests are enabled only for
Dolphin; they do not test writes or transport behavior on a physical Wii. See
[the multi-sector report](dolphin-runs/2026-11-03-wiisd-multisector/README.md),
[the earlier one-sector report](dolphin-runs/2026-11-03-wiisd-write-roundtrip/README.md),
and [development results](../DEVELOPMENT_RESULTS.md). A separate Dolphin run
disabled the virtual SD card. Ten checks passed for no-card startup, insertion,
read, write, zero-sector, shutdown, and restart behavior. See the
[no-card report](dolphin-runs/2026-11-03-wiisd-no-card/README.md). Both runs use
Dolphin's virtual SD interface; physical Wii behavior remains open.

The 2026-11-03 storage probe run passed 62 checks and failed 3 because it had
no Wiiload arguments and no USB storage device. The run configured a private
Dolphin memory card path, but Dolphin did not create the card image. The card
probe therefore skipped save operations and formatting. This run does not
reduce the 90 open memory-card entries or the open USB transport entries. See
the [private-card Dolphin run](dolphin-runs/2026-11-03-storage-card-abs/README.md)
for the DOL hash, command, report, and limits. The separate
[USB storage probe](dolphin-runs/2026-11-03-storage-usb-flush/README.md) passed
48 checks, including raw and safe null-handle errors, transfer-policy checks,
C ABI error paths, capacity output preservation, disc-interface status
cleanup, and deinitialization status. It does not test a mounted USB device.

## First batch

The first controller model batch added host evidence for 18 inventory entries.
A second batch recorded 14 controller model, transition, and HID APIs.
A third batch added Xbox report boundary and Bluetooth stage text checks.
The inventory had 2,000 unverified entries after those batches. It now has
1,965. See the
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
