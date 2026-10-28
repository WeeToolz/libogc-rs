# Wii std service probes for application

Updated: 2026-10-14. All results below are from Dolphin. These DOLs still have
no real-Wii results.
application remains no_std until its required service and application runs pass.
The older 54-check thread probe result covers the tested thread/runtime-entry
path on RVL-101. The user also reports an old DOL pass on RVL-CPU-20, Rev 20,
6-layer board, likely in an RVL-001 console. It does not verify these new probes.

## Manual run order

No loader arguments or SD app folder are required for these three DOLs.
Build them with `cargo xtask build-wii-std-probe`.

1. `target/xtask/probes/wii_std_services_probe.dol` checks std System and normal
   Vec allocation on reserved MEM2, alignment, zeroing, failed resize, release,
   concurrent allocation, full framebuffer/heap/workspace separation, stdout and
   stderr cursor changes, missing-sink errors, futex waits/wakes, Mutex, Condvar,
   RwLock, Once, and OnceLock. Read the final PASS/FAIL summary. The probe holds
   that screen after normal Rust main return and runtime cleanup.
2. `target/xtask/probes/wii_std_usb_probe.dol` checks eight real asynchronous IOS
   USB HID root opens and closes, a real missing-path error, final callback
   results, deferred worker identity, callback counts, USB host initialization,
   a bounded real HID device list, and USB cleanup. Read the final summary.
   This probe does not write USB storage or firmware. Its root/device-list run
   does not verify controller report traffic, disconnect handling, or application
   input. Those require the full application/device run.
3. `target/xtask/probes/wii_std_panic_probe.dol` deliberately calls `panic!` with
   the default std hook. The expected final screen contains the default panic
   diagnostic, source file/line, and `WII_STD_PANIC_MARKER_51A7`. It then stops
   at panic abort. This terminal probe has no PASS summary and no return to the
   loader. Capture that screen. Do not treat an earlier "Expect" line as proof
   that the panic hook printed the marker. The binary exports
   `WII_STD_PANIC_DROPS`; a debugger can check that it remains zero after abort.

## Dolphin results and DOL hashes

All three DOLs were rebuilt with `cargo xtask build-wii-std-probe` and run in
Dolphin on 2026-10-14. The services DOL passed 113 checks with zero failures.
The USB DOL passed 76 checks with zero failures. The panic DOL displayed the
default hook diagnostic, source location, expected marker, and zero drops before
panic abort. The panic probe has no pass/fail count by design. These emulator
runs do not replace the required Wii runs.

| Probe DOL | SHA-256 |
| --- | --- |
| `wii_std_services_probe.dol` | `0899286366c45c80d144975a6dc4378939605eb6d82cd8ca5ee512061160c20d` |
| `wii_std_usb_probe.dol` | `737e800a86f1352df60a513c9a044e94afad7edfc9e8936dbfee797ddceef1f1` |
| `wii_std_panic_probe.dol` | `986d1ce4df57d4a7ad0a2585780fd851ca98e863b9e77a8cad1c636b428b6754` |

Screenshots: [services Dolphin result](hardware-evidence/wii-std-services-dolphin-20261014.png),
[USB Dolphin result](hardware-evidence/wii-std-usb-dolphin-20261014.png), and
[panic Dolphin result](hardware-evidence/wii-std-panic-dolphin-20261014.png).

For each console run, record the DOL hash, console model and board revision,
final screen, and result. A hang before the final result is incomplete.
Do not treat a missing result as a pass.

## Final failure display

The shared probe logger retains failed check names across earlier pages. On
failure, it clears the final screen, prints up to 16 retained failed names, and
prints the pass/fail totals. Names longer than 64 UTF-8 bytes have an explicit
ellipsis. If more than 16 checks fail, the final screen also gives the number
that did not fit. The total includes every failed check. Log-output failures
also appear as failures. Failure detail is not cleared by main return.

The final-screen path was tested in Dolphin with the clock probe and no UTC
argument. The final screen retained the real missing-offset check and showed
22 passes and 1 failure. This is a negative test of result display; it is not
clock acceptance.

## Production changes and scope

The std output adapter now calls `__wii_std_output`. The runtime writes through
the initialized production video console. It serializes the shared cursor and
escape parser with interrupt masking on the single Wii CPU. It does not allocate
or wait while that state is held. Missing console state returns ENODEV, malformed
input returns EINVAL, and console transport failure returns EIO. The runtime
retains the latest diagnostic error even when upstream panic abort ignores a
write result. Stdin still returns Unsupported for a nonempty read.

The `wii-std-sync` probe feature selects `std-system-allocator` in the probe
library. The custom probe global allocator is then disabled, so standard Vec,
thread closure storage, and other global allocations use std System. The no_std
probe build keeps its existing allocator. Explicit System calls also verify the
same production provider. Read-only reserved-range accessors expose actual
allocator state under its lock; no expected memory addresses are fabricated.

The first allocator run showed a 16-byte increase after std initialized its
main runtime state for scoped workers. The probe now warms both std worker paths,
takes the live-allocation baseline, and checks that a repeated run releases its
allocations. Dolphin snapshots showed 1,104 bytes before the measured run, 1,104
after direct allocation checks, 1,104 after the first join, 1,184 while the
second join handle vector remained live, and 1,104 after scope cleanup. The
final release check passes. This verifies that scoped worker allocations are
released while keeping required lazy std state in the measured baseline.

The old futex mismatch check measured scheduler waits across logger output.
Logger output can wait for a real VI retrace. The check now measures immediately
before and after the futex call, then logs the results. Its no-wait assertion
remains unchanged.

The clock, file, and network DOLs remain available for future applications that
need those services. application's current feature selection does not require std
file, wall-clock, or network acceptance. application still needs real-Wii service
results, conversion and dependency review, a full build, and rendering/input/
reset/error acceptance on Wii. The full repository API backlog is separate.
