# Wii std implementation checklist

Updated: 2026-10-15. Scope: Wii PowerPC in libogc-rs.

A checked item means the stated source or build work is complete. It does not
mean hardware acceptance unless the item states a Wii run. The 54-check std
thread/runtime-entry probe passed on the RVL-101. The user also reports an old
thread DOL pass on RVL-CPU-20 (likely RVL-001). The combined RVL-001 hardware run now supplies the service evidence listed below. Unverified service gates remain open.
Use the contract IDs in [the gap map](wii-std-contract-gaps.md) for details.
See the [Wii hardware test ledger](wii-std-hardware-test-ledger.md) for every
archived combined/network probe attempt, including incomplete Wiiload runs.

## 1. Existing foundation work

- [x] Record the std contracts, current Wii services, and unsupported candidates.
- [x] Use Rust edition 2024 with shared workspace settings.
- [x] Pin the Rust compiler commit and matching library source hash.
- [x] Prepare an isolated source overlay without changing installed rust-src.
- [x] Preserve Cargo's actual failure status in the std build check.
- [x] Select Wii allocator, Newlib error, and OS TLS adapters in Rust std.
- [x] Add `ogc-std-runtime` providers over the production system and LWP services.
- [x] Add checked heap startup, zeroed allocation, and resize failure preservation.
- [x] Share the Newlib error classifier and read the calling thread's errno.
- [x] Add generational TLS keys, per-thread values, and exit destructors.
- [x] Add join/detach ownership and release detached stacks from another stack.
- [x] Pass host allocator and shared TLS, lifecycle, error, and startup tests.
- [x] Build and link the production LWP foundation probe to a Wii DOL.

## 2. Synchronization and random data

The four threaded synchronization selectors now use the pinned Rust futex
algorithms over production LWP waits. The std compile check passes. Random
requests have an explicit unsupported failure policy; no entropy source is
accepted. See [synchronization and entropy status](wii-std-sync.md).

- [x] Implement/select a threaded std mutex over production LWP primitives (S01).
- [x] Implement/select a condition variable with atomic release/wait/reacquire,
      timeout reporting, and mutex association rules (S02).
- [x] Implement/select a blocking reader/writer lock with exclusive writers,
      concurrent readers, ownership checks, and a documented wake policy (S03).
- [x] Implement/select threaded once initialization, publication, waiter wakeup,
      and the applicable Rust poison behavior (S04).
- [x] Avoid per-lock handle pools: use constant atomic state and the existing
      LWP thread wait record, with no lazy allocation (S01–S04).
- [x] Select acquire/release algorithms that use only target-supported 32-bit atomics.
- [ ] Verify acquire/release ordering and all synchronization behavior on Wii (S06).
- [ ] Find and validate a real entropy service; define failure behavior for the
      infallible random entry point; then implement `fill_bytes` (P07).
- [x] Define an explicit unsupported random failure policy. Nonempty requests
      and default HashMap seeding fail; there is no substitute seed.
- [x] Run the actual std build again. Record errors and selected fallbacks.
- [x] Build and link the actual std synchronization probe with LWP workers.

## 3. Runtime, allocation, threads, and TLS

- [x] Verify heap/workspace ownership, non-overlap, and startup order on Wii (A01–A02): 29 allocator checks passed in run `1792070611-686276000`.
- [x] Connect Rust runtime entry and cleanup to `_start_rust`; check the initial
      thread and sheltered arguments before Rust main (A03, P01). Source and
      normal-main probe linking are complete. The tested normal-entry and return
      path passed on RVL-101; see the thread evidence for the exact scope.
- [x] Translate IOS, socket, LWP, FAT, and devoptab errors at their std boundaries.
      Preserve useful raw errors; keep their code spaces separate (E01–E02).
      The source changes and controlled failure probe pass in Dolphin. Real
      transport failure acceptance remains open; see
      [error and cleanup evidence](wii-std-error-cleanup.md).
- [x] Add std thread spawn/closure transfer with checked stack sizing and complete
      cleanup after creation failure (T01).
- [x] Add std join/result publication and JoinHandle drop/detach adapters (T02–T03).
- [x] Connect std TLS destruction state, lazy keys, destructor passes, and initial
      thread cleanup through the actual std adapter; build its acceptance probe (T04).
- [x] Implement sleep/yield duration conversion, rounding, long waits, and early
      wake handling (T05).
- [x] Provide stable Rust thread IDs, thread metadata, and truthful usable CPU
      parallelism (T06).
- [x] Implement park/unpark with one retained token, token coalescing, wake and
      timeout races, and safe object lifetime (S05).

See [runtime and thread design](wii-std-thread-runtime.md) and
[verification evidence](wii-std-thread-evidence.txt). These checked source items
have hardware evidence for the 54 checks in the thread probe. That evidence
does not close untested timing or other service gates in section 8.

## 4. Clocks and deadlines

- [x] Implement monotonic Instant conversion and checked arithmetic; keep the
      Wii hardware timebase stable across pre-main startup (C01).
- [x] Review the RTC epoch, counter bias, and boot adjustment; implement the UTC
      SystemTime adapter with an explicit application UTC offset (C02).
- [x] Expose fallible clock reads and validity; define panic-abort for an unavailable
      infallible clock read without returning a substitute timestamp (C03).
- [x] Use the shared tick budget for sleep and futex/parking waits; build real
      Instant and condition-variable absolute-deadline acceptance checks (C04).
- [x] Implement std socket read/write deadlines over bounded IOS poll calls,
      with one absolute end time across retries and real transport errors (C04, N05).
- [x] Run the combined Wii probe with an explicit UTC-4 offset, real RTC samples,
      monotonic time, sleep, and deadlines: 31 clock checks passed. This does not
      compare wall time with an independent time source.
- [ ] Verify clock accuracy against a trusted time source and test RTC failures.

See [clock design and policy](wii-std-clocks.md) and
[clock verification evidence](wii-std-clock-evidence.txt). Source and build checks
do not close hardware acceptance. Wall time requires initialized console
configuration, a real RTC/bias sample, and an explicit UTC offset.

## 5. Network and DNS

- [x] Define IOS network initialization ownership and live-socket lifetimes (N01).
- [x] Add TCP/UDP std adapters with real byte counts, partial I/O, EOF, and cleanup.
- [x] Check nonblocking transport results before updating local state (N02).
- [x] Verify connection completion/error queries for connect_timeout, or return
      Unsupported until that capability exists (N03).
- [x] Define supported socket options and queries, including take_error. Return
      Unsupported for unavailable queries (N04).
- [x] Implement read/write timeouts with checked durations and bounded retries (N05).
- [x] Implement socket clones with shared ownership and exactly one final close (N06).
- [x] Implement local/peer address queries, shutdown, and explicit IPv4-only behavior.
- [x] Implement owned DNS results with correct lifetime and concurrent lookup behavior.
- [x] Record unsupported address families and network operations at each API boundary.
- [x] Run the real-Wii network-only suite: 41 checks passed in a 130-pass,
      0-failure run, including peer TCP/UDP traffic, DNS concurrency, HTTP bytes,
      deadlines, options, and socket cleanup. See the run record below.
- [ ] Test additional router, DNS, and failure-injection cases beyond this run.

## 6. Files, paths, and standard streams

The adapters support mounted FAT files, directory operations, relative paths,
and application environment state. Source checks and host engines pass;
hardware acceptance remains open. Independent writer opens remain limited by
the FAT service. See [file adapter design](wii-std-files.md) and
[file evidence](wii-std-files-evidence.txt).

- [x] Implement exact OpenOptions flags, including append and create_new (F01).
- [x] Check file offset/size widths; preserve partial I/O and cursor behavior (F02).
- [x] Implement shared clone cursors and synchronized open-file lifetime (F03–F04).
- [x] Make sync_all/sync_data reach the file metadata and transport and report failures (F05).
- [x] Record file cleanup failures and define std Drop failure behavior (F06).
- [x] Return real metadata and stored FAT timestamps. Set access/modification
      times and read-only permissions through path and file handles (F07).
      Absent FAT dates remain Unsupported.
- [x] Connect directory traversal to typed FAT handles, exclude dot entries,
      preserve name bytes, and return entry errors (F08). The SD suite passed on Wii; USB acceptance remains open.
- [x] Connect copy to typed FAT transfers, sync, checked closes, and read-only
      attributes (F08). The SD suite passed on Wii; USB and media-failure acceptance remain open.
- [x] Connect rename with replacement, recursive removal, and canonicalization
      to real FAT operations (F08). Wii persistence and failure tests remain open.
- [x] Define mount-prefix paths such as `sd:/`, OsStr bytes, NUL validation,
      roots, relative paths, current directory, and semicolon path lists (F09).
      See [paths and environment](wii-std-paths-environment.md).
- [x] Implement or explicitly reject file locks, links, timestamp changes, and
      unsupported permission operations (F10).
- [x] Connect stdout/stderr and panic output to the checked console sink.
      Missing sinks report errors; stdin explicitly reports Unsupported (F11).
      Real-Wii panic/diagnostic acceptance remains open.

## 7. Other platform contracts

- [x] Preserve sheltered loader argument bytes and lifetime in args_os; test
      non-UTF-8 bytes and rejected buffers on the host (P01). Wii acceptance is open.
- [x] Implement application environment lookup, mutation, validation, and owned
      iterator snapshots under a mutex (P02). The combined SD suite exercised these operations on Wii.
- [x] Provide current/executable/temp/home path behavior with explicit loader,
      mount, HOME, and TMPDIR requirements and errors (P03). Wii checks remain open.
- [ ] Define exit cleanup and concurrent exit arbitration. Document the absent
      parent exit-status channel (P04).
- [ ] Define and verify abort/panic diagnostics with panic abort (P05).
- [x] Verify actual Unsupported errors for child process creation and captured
      child output in the combined probe (P06). No process service is claimed.
- [ ] Resolve math and runtime symbols to real Rust implementations (P08).
- [x] Set truthful Wii platform constants and verify unsupported backtrace
      capture in the combined probe (P09).

## 8. Build, hardware acceptance, and application adoption

- [x] Build std with the pinned compiler/source and intended panic features.
- [x] Link a real std synchronization probe DOL and confirm no undefined symbols
      or dynamic dependencies. Entropy remains explicitly unsupported.
- [x] Build a second std DOL with normal Rust `main`, thread creation, TLS cleanup,
      park/unpark, and the real main return callback.
- [x] Build a third std DOL for real clocks, UTC configuration, sleep, and
      condition-variable absolute deadlines. Missing UTC input is not a pass.
- [x] Audit pinned std platform selectors and selected default backends; record
      unsupported contracts and path semantics in [the selector audit](wii-std-selector-audit.md).
- [x] Add a normal application DOL build command with prepared Wii std.
- [ ] Resolve the contracts and native dependencies that the selected application
      uses before its adoption. Keep unused unsupported contracts in the backlog.
- [x] Build a fifth std DOL for actual file flags, shared cursors, std workers,
      sync, resize, fixed persisted bytes, reopen, and SD remount checks.
- [x] Run allocation failure, alignment, resize, concurrency, and cleanup probes on Wii: 29 allocator checks passed in run `1792070611-686276000`.
- [x] Run the 54-check std thread, TLS, detach, stack failure, parking, and
      initial-thread probe on RVL-101. Record the additional user-reported old
      DOL pass on RVL-CPU-20; the console model is likely RVL-001.
- [x] Run the implemented lock contention, reader/writer, once, parking, timeout, and exhaustion probes: 33 futex and 43 synchronization checks passed in run `1792070611-686276000`. This closes the exercised scenarios only.
- [ ] Compare wall-clock accuracy with a trusted source and inject RTC failure.
- [ ] Test additional socket reset, failed-connect, and nonblocking failure cases.
- [x] Verify SD file bytes and timestamps after reopen and owned unmount/remount:
      the combined Wii run passed the SD suite, 78 checks with no failures.
- [ ] Verify USB FAT persistence, media removal, flush failures, interrupted
      writes, and recovery. The 2026-10-15 combined run failed four USB FAT
      checks. Its DOL identity is old and the retained report lacks the first
      failing operation. See the [current investigation](review/usb-fat-eio-investigation-2026-10-28.md).
- [ ] Verify real streams, loader arguments, and controlled exit/abort behavior.
- [ ] Record hardware model, toolchain, probe commit, results, and failures.
- [ ] Enable std in an application only after the required contracts pass (P10).
- [ ] Audit each application dependency for Wii cfg assumptions and native links.

Use production APIs and independent expected results. A successful compile,
source evidence entry, emulator run, or skipped test is not a real-Wii pass.

## First application gate

Select an application and record its actual service requirements before
conversion. See [application acceptance](wii-std-application.md). Convert it
when its required services, dependency link checks, and Wii tests pass.
The repository API backlog
is a separate task. Keep the allocator, scheduler, drivers, and
`ogc-std-runtime` on `no_std`.

## application service probe handoff — 2026-10-14

The service, USB, and panic DOLs are built. See
[the service probe instructions](wii-std-service-probes.md). The service probe
selects std System as its global allocator and checks normal Vec storage, direct
System operations, full framebuffer/heap/workspace ranges, and concurrent
allocation. It also runs std synchronization and the production futex tests.
The checked console transport supplies stdout, stderr, and the default panic
hook after console initialization. Stdin remains explicitly unsupported.
The later combined run supplies real-Wii evidence below. application service readiness still depends on its remaining required contracts.

## Combined hardware run

The [combined readiness DOL](wii-std-readiness-probe.md) reuses the standalone
suites. It runs SD then USB, records real prerequisites and failures, and checks
normal main return and TLS cleanup before the final summary. Failed check names,
error details, model/board labels, and measured chipset revision remain on
paged final screens. Panic and entropy are terminal modes of the same DOL and
require separate launches. Full real-Wii acceptance remains open.

## Verified service checks on RVL-001 — 2026-10-15

Wiiload run `1792070611-686276000`, DOL SHA-256
`9638fbde1d3e9efb7a18786fd5b0f61c4ea1bbb731024f681e621b9c336d67db`,
returned its complete log and framebuffer. Native settings report RVL-001(USA),
IOS58 revision 6176, Hollywood 0x11. The printed motherboard revision was not
supplied or detected. The result was **405 passed, 8 failed**, not an overall pass.

| Suite | Passed | Failed | Recorded scope |
|---|---:|---:|---|
| Diagnostics | 6 | 0 | Checked console writes and flush/errors |
| Allocator | 29 | 0 | Memory ranges, alignment, resize, concurrent workers, release |
| Normal entry, threads and TLS | 46 | 0 | Shared thread suite before final main return |
| Futex transport | 33 | 0 | Production wait/wake checks |
| Synchronization | 43 | 0 | Implemented std synchronization scenarios |
| Clocks | 31 | 0 | Explicit UTC-4 offset, RTC, monotonic clocks and measured deadlines |
| SD files and environment | 78 | 0 | Directory traversal/copy, rename, recursive removal, paths, timestamps, permissions, file bytes and remount |
| USB IOS and callbacks | 75 | 0 | 17 real requests, worker completion, HID enumeration and cleanup |
| Platform limits | 5 | 0 | Actual unsupported contracts and target constants |
| Controlled error/cleanup | 22 | 0 | Production fault endpoint and diagnostics |
| Networking | 12 | 4 | DNS and outgoing connection checks passed; binds and HTTP bytes failed |
| USB FAT | 13 | 4 | Mount/path setup passed; directory/file writes and unmount failed |

Final main return and TLS cleanup also completed. Evidence:
[structured returned checks](hardware-evidence/wii-std-readiness-rvl001-20261015.json)
and [Wii framebuffer](hardware-evidence/wii-std-readiness-rvl001-20261015.png).
The complete raw run artifacts remain in `target/wii-runs/1792070611-686276000-10.0.0.242/`.

Keep real media removal, interrupted writes, power loss, RTC failure, terminal
panic/entropy and all unexercised cases open. These passes apply to the tested
RVL-001 run; they do not establish acceptance on every motherboard revision.

The later network-only run `1792071595-115194000` returned 109 passed and
3 failed. SD and USB were removed. This run did not execute either storage
suite. It confirmed that a native 32-byte HTTP receive returns `HTTP/`, while
the five-byte std receive returns `HTTP\0`. The local TCP acceptance and UDP
broadcast option checks also failed. The actual report and framebuffer are in
`target/wii-runs/1792071595-115194000-10.0.0.242/`.

The next build uses one shared receive buffer and reader mutex per socket.
Short stream reads keep extra bytes for later reads through any clone. Datagram
reads discard only the remainder of that datagram. Six host regression tests
pass, and the Wii DOL builds. Real-Wii acceptance of this change is still open.

Run `1792071743-658606000` tested the shared receive-buffer build
`d2fb5755744cc07359bb3e0f75bfd727ee062de06084f06d9aad4dfc9c4c3d6a`
on the same RVL-001 without SD or USB. The complete returned report records
110 passed and 3 failed. The actual HTTP prefix and the following version bytes
read through a clone both passed. Final socket cleanup also passed. Local TCP
acceptance, the UDP broadcast option, and UDP datagram acceptance remain open.
The latter reached a real IOS send error `-63` (newlib `95`). The full report and
framebuffer are in `target/wii-runs/1792071743-658606000-10.0.0.242/`.

The network-only run `1792074739-165610000` tested DOL SHA-256
`3c7392d3d14a6dc446d1ff1fa2c8da71010cc7150c1055f0d97b01c9b797af55` on the
same RVL-001. It returned **130 passed, 0 failed**. All 41 network checks
passed: TCP listener callbacks, TCP_NODELAY state, UDP sends and echoes for
non-empty, empty, and connected datagrams, DNS, HTTP bytes, deadlines, and
socket cleanup. The Mac peer log confirms receipt of the three Wii datagrams.
The peer now handles the socket timeout exception used by the firewall-approved
Python 3.9 runtime; its regression test waits through the poll interval before
sending. The probe shows its final report for five seconds, then requests a
return to the System Menu. Evidence:
[structured result](hardware-evidence/wii-std-network-rvl001-20261015.json),
[Wii framebuffer](hardware-evidence/wii-std-network-rvl001-20261015.png), and
raw artifacts in `target/wii-runs/1792074739-165610000-10.0.0.242/`.
