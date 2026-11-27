# USB StartStop error precedence and IPC replacement scope

2026-11-26 local date. Rust base `3b9a6cc2212b80f2cabf5998f6cdeb17fcb62a65`
plus the probe and scoped production changes, local source lock and unrelated
worktree edits. C reference `7d4b8b3e7b845516f0cc2a85274879ddd8cbf142`.
Stock Linux x86_64 Dolphin 2609b with a private restored System Menu NAND.

| Run | Passed | Failed | Valid pages |
| --- | ---: | ---: | ---: |
| USB capacity baseline, stricter StartStop test | 59 | 3 | 6 |
| USB capacity, corrected production code | 62 | 0 | 5 |
| IOS reload, competing thread and scope release | 105 | 0 | 10 |

The [summary](summary.json) records the exact DOL and raw report hashes.
The selected guest check logs are excerpts; raw reports, logs, transports and
captures (including an interrupted first IPC run) are preserved in a private
archive whose hash is in the summary. No NAND, keys or raw emulator logs are
in Git. Host xtask tests passed 163/163, USB lifecycle tests passed 21/21 and
the PowerPC storage group check passed.

## USB StartStop

C `libogc/usbstorage.c:691-720` rejects a LUN outside `max_lun` before taking
the mutex. Rust previously called `TransferGuard::acquire` first; the
missing mutex on the closed handle caused `USBSTORAGE_EINIT` (-10009) instead
of `IPC_EINVAL` (-4). The stricter probe observes the old result and fails
three checks (one direct and two grouped checks of the same error order).
The fixed code checks the LUN before acquiring the mutex, retaining the null
handle and Wii hardware checks and the existing mutex failure policy for
valid LUNs. Both the Rust facade and C ABI now return -4 for the closed
handle case. The run does not establish successful live USB transfers.

## IPC replacement scope

The existing 103-check reload probe now also creates and joins a real LWP
while the owner holds the replacement scope. The competing thread attempts
a synchronous open, an asynchronous open and a nested replacement. All
three return `IPC_EQUEUEFULL`. The owner can still use synchronous IPC.
After dropping the scope, a fresh open/close succeeds and the measured
request/deferred capacities are unchanged; no rejected callback runs.
The three actual IOS replacement iterations still pass, with full capacity
recovery and exact-once raw/formatted callbacks. The first extended run
was interrupted by the execution window at page 9; it is not a completed
pass. The repeat of the same built DOL passed all 105 checks and all ten
capture pages. No production IPC code changed in this pass.

This is a scoped concurrency check, not complete coverage of replacement
failure, new IOS round-trip failure or replies that race with replacement.
Physical Wii cache/DMA and USB timing evidence remain pending.

Reproduce with the configured offline toolchain and private test fixture:

```sh
cargo xtask dolphin-validate --probe usbstorage_capacity_probe --timeout-seconds 60
cargo xtask dolphin-validate --probe ios_reload_probe --timeout-seconds 100
```
