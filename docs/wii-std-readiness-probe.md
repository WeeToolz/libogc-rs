# Combined Wii std readiness probe

The combined DOL runs the implemented std acceptance suites through the real
Rust providers. It runs SD files first and USB FAT files second. It then checks
USB IOS completion, networking, native platform limits, and controlled cleanup
errors. The final summary is produced after compiler-generated main return,
initial-thread TLS destruction, and real runtime cleanup.

## Build and HBC installation

```sh
python3 scripts/package-wii-std-readiness.py \
  --console-model 'RVL-101 (USA)' --motherboard RVK-CPU-01
```

The packager uses the existing prepared-std application builder. It creates:

```text
target/xtask/apps/wii-std-readiness-probe/
  boot.dol
  meta.xml
  build.manifest.json
  loader-settings.json
```

Copy that directory to `apps/wii-std-readiness-probe` on the HBC medium. Launch
it through HBC so its meta.xml arguments are supplied. Direct Wiiload of the
DOL without arguments cannot establish clock or declared-board acceptance.

The default UTC offset is the build computer's actual current local offset.
This assumes the Wii clock shows the same local time. The prepared package on
2026-10-15 uses -14400 seconds (UTC-4). Override it if the Wii clock differs:
`--utc-offset-seconds=0` means the Wii clock shows UTC. The probe checks the
configured RTC conversion, not an independently authenticated UTC source.

The default DNS/HTTP endpoint is `example.com:80`. Use
`--network-endpoint=host.example:80` to change it. A DNS name is needed for DNS
acceptance. Wiiload connectivity alone does not prove the std network APIs.
There is no required desktop peer process for the existing loopback tests.

## Included checks and code sharing

| Suite | Checks |
| --- | --- |
| Diagnostics | Missing console errors, actual stdout/stderr bytes and flush |
| Allocation | Heap/workspace/framebuffer separation, alignment, zeroing, resize, exhausted allocation, threads |
| Entry and threads | Sheltered arguments, real worker creation, identities, join/detach, TLS, cleanup errors |
| Synchronization | Production futex operations, mutexes, reader/writer locks, once, condition variables, parking |
| Clocks | Arithmetic, hardware timebase, waits, deadlines, RTC epoch/bias and supplied UTC offset |
| SD and USB FAT | File flags and bytes, partial I/O, clones, resize, attributes, timestamps, directories, copy, rename, paths, environment, reopen/remount |
| USB IOS | Actual asynchronous opens/closes and deferred callbacks, HID enumeration and cleanup |
| Network | TCP/UDP, DNS ownership/concurrency, clones, options, partial I/O, EOF, errors, nonblocking behavior, deadlines |
| Platform | Unsupported stdin, child processes and captured child output, unavailable backtraces, truthful target constants |
| Controlled failures | Production devoptab error propagation and cleanup diagnostics using an explicit fault endpoint |
| Final return | Main return status, initial/late TLS destructors, final cleanup state and rejection of repeated cleanup |

The standalone DOLs and the combined DOL call the same suite modules. No suite
was copied to create the combined entry point. SD and USB use the same FAT suite
with an explicit medium argument. Loader option parsing and failure pagination
also use shared code. The platform-constant check found an empty OS constant in
the upstream fallback; the Wii source patch now sets OS="none", no OS family,
no dynamic-library suffix, and executable suffix ".dol".

## Media and failure behavior

Provide both an SD card and a writable FAT USB drive for full storage coverage.
The tests mount and unmount their own media. An existing mount prevents remount
acceptance and is reported. An absent medium is a failure with its actual FAT
error. No absent or skipped hardware check is converted to a pass.

The suite obtains file/tree ownership with exclusive creation before destructive
writes. Existing probe files and directories are preserved and cause a failure.
Only paths created by this run are removed. Each medium uses these names:
`wii-std-fs-probe.tmp`, `wii-std-fs-probe-dir`, `wii-std-path-probe`.
Do not remove media while this normal suite runs. Real removal, flush-failure,
interrupted-write, USB power-loss and recovery tests require controlled runs.

The final screen lists failed check names, error details when available, totals,
console model, declared motherboard revision, and measured Hollywood revision.
Press the console RESET button to change failure pages. This reads the button;
it does not request a reset. Each failing check counts once. A suite summary
does not add a second failure for the same check. Retention is allocation-free
and bounded to 64 failures, with 96 bytes per label/detail. Any omitted count is
visible. Four failures per page leave space for wrapped text and hardware data.

MODEL is read through the existing initialized setting.txt parser. A supplied
`--console-model` overrides its displayed label and is marked as a label. The
native setting is still printed during startup. `--motherboard` is a declared
PCB label, not an inferred hardware result. The Hollywood value is a chipset
revision, as documented in [the hardware register reference](https://wiibrew.org/wiki/Hollywood/Registers).
This probe has no verified software method to establish the printed PCB label.
Without a supplied board label, it prints unknown. The prepared package uses
the user's RVL-101 (USA) / RVK-CPU-01 labels. Change them for another Wii.

## Terminal modes

The same DOL accepts `--mode=normal`, `--mode=panic`, and `--mode=entropy`.
Change the mode argument in meta.xml, or regenerate the package with `--mode`.

Panic and entropy checks require separate launches because panic=abort stops
execution. Panic mode uses the unchanged default hook and the shared terminal
panic suite. It must show `WII_STD_PANIC_MARKER_51A7` and stop without dropping
the stack guard. Entropy mode constructs a default HashMap. With the current
backend, it must show the explicit missing-entropy panic and stop. Neither
terminal mode produces a PASS summary. Normal mode cannot prove a terminal
panic path in the same run as its successful main return.

## Evidence and open work

Builds and emulator evidence do not close real-Wii gates. The combined result
symbol is `WII_STD_READINESS_PROBE_RESULT`: i32::MIN means incomplete, 0 means
all completed normal checks passed, -1 means failures, and -2 means a missing
final logger. A terminal panic does not publish a normal result.

Entropy remains unverified. Native FAT limits still include the u32 file-size
bound, independent writer-open restriction, and unsupported links/file locks.
Child processes and stdin have explicit unsupported errors. Full application
readiness still depends on the APIs that its dependencies use.

See the [Wii hardware test ledger](wii-std-hardware-test-ledger.md) for the
complete run history, including incomplete report attempts and the final
130-pass network result. Focused network runs do not exercise the storage
suites, even when their launch arguments include `--media=both`.

## Verification on 2026-10-15

The normal DOL is 946,940 bytes. Its SHA-256 is
`6a6acf34e436427f5d2694c505d25cba655c8d80ae9574cf62f3ad02bf8c8bfd`.
All std probe DOLs build after the suite extraction. The combined ELF has no
undefined symbols and no dynamic section. Packaging checks the DOL against the
exact `dol_sha256` field in its build manifest before it copies the artifact.

The normal combined DOL completed in Dolphin 2603-415 with 414 passed and two
failed checks. GDB observed result -1 after real main return and cleanup. The
last screen named both failures: missing UTC-offset loader input and USB FAT
`DeviceUnavailable`. The emulator profile supplied neither HBC arguments nor a
USB storage device. No missing prerequisite was suppressed. The remaining
suites, including TCP/UDP/DNS, SD persistence, process error contracts, platform
constants, and final runtime/TLS cleanup, completed without another failure.
The native settings reported RVL-001(USA); no PCB label was supplied to this
emulator run, so its board field was unknown. The HBC package separately carries
the user's explicitly marked RVL-101 (USA) / RVK-CPU-01 labels.
See the [final failure screen](hardware-evidence/wii-std-readiness-dolphin-20261015.png).
An independent fsck_msdos run after completion found zero files and all 32,731
data clusters free on the controlled FAT16 SD image.

Host checks passed: five failure-retention/pagination tests, one loader-option
test, one shared model-setting parser test, three packaging tests, two source
preparation tests, and 33 xtask tests. Source, roles, dependency direction, and
inventory freshness pass. The full repository completeness audit remains open
with 2,416 unverified APIs; no thresholds or allowances were relaxed.

The combined DOL also ran on a real Wii on 2026-10-15. It completed with
**405 passed and 8 failed**. The SD file and path suite passed 78 checks with no
failures, including persisted bytes and timestamps after unmount/remount. The
USB FAT suite passed 13 checks and failed 4: directory checks, path checks,
exclusive file creation, and owned unmount returned I/O error 5. The USB IOS
and deferred-callback suite passed 75 checks. Networking passed 12 checks and
failed 4 in this run; a later network-only run passed 130 checks with no
failures. This combined run is not an overall pass. The failed checks were:

- `local TCP acceptance completes`, `UDP edge acceptance completes`, and `UDP
  bind reaches the real IOS socket service` returned Wii network error -28
  (Newlib 22).
- `TCP exchanges real HTTP bytes` saw response prefix `[72, 84, 84, 80, 0]`
  (`HTTP\0`).
- `directory checks complete`, `path service checks complete`, and
  `probe obtains a new file` returned EIO (Newlib errno 5) on USB FAT.
- `owned medium unmount succeeds` failed on USB FAT.

The Wii probe read model `RVL-001(USA)` from settings and Hollywood revision
`0x11`. Its motherboard label was unknown. This is separate from the user's
RVL-101 / RVK-CPU-01 identification for the std thread probe; the combined
probe did not verify that board label. See the [combined Wii result](hardware-evidence/wii-std-readiness-rvl001-20261015.json),
[captured screen](hardware-evidence/wii-std-readiness-rvl001-20261015.png),
and raw run directory `target/wii-runs/1792070611-686276000-10.0.0.242/`.

NOT RUN: USB FAT persistence acceptance, media removal, interrupted writes,
power-loss/recovery, and panic/entropy terminal modes through HBC. The later
network-only run is recorded in the [readiness checklist](wii-std-checklist.md).
