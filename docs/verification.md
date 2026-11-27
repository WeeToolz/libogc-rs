# Verification guide

Use these commands to check the repository. Each command proves only the work
that it runs. A successful compile does not prove runtime behavior. A Dolphin
run does not prove behavior on a real Wii or GameCube.

## Host checks

Run the main host checks with:

```sh
cargo check -p xtask
cargo xtask check-roles
cargo xtask check-dependencies
cargo xtask test-host
cargo xtask check-inventory
cargo xtask audit-source
cargo fmt --all --check
```

`test-host` runs the selected Cargo tests, standalone Rust tests, and Python
tests registered in `xtask/src/host_tests.rs`. External MP3 fixtures are
required for the full suite. See [dependency notes](dependencies.md).

Do not use `cargo check --workspace` as runtime validation. Most runtime
crates only build for PowerPC targets. See [crate roles](crate-roles.md) for
package groups and ownership.

## Wii target checks

Compile one runtime group with:

```sh
cargo xtask check-group core
```

Available groups include `storage`, `input`, `network`, `audio`, `debug`, and
`hardware`. Compile all packages marked as runtime with:

```sh
cargo xtask check-all-runtime
# alias
cargo wii-check
```

These commands use nightly Rust, the checked-in PowerPC target, `build-std`,
and `RUSTFLAGS=-D warnings`. They check compilation only. For a direct
single-package command, see [crate roles](crate-roles.md).

## Probe builds and Dolphin runs

Build the standard probe set and run a registered probe in Dolphin with:

```sh
cargo xtask build-probes
cargo xtask dolphin-validate --probe lwp_probe
```

The runner saves its report under `target/dolphin-validation/`. Read the
report and check the named probe results. A DOL build or emulator launch alone
does not count as a probe pass.

For IPC formatter validation, the API coverage ID is `ipc-format-probe` and
the registered Dolphin runner is `runtime_probe`. The explicit mapping is in
`config/probe-coverage.csv`; use the runner ID with `dolphin-validate`:

```sh
cargo xtask dolphin-validate --probe runtime_probe
```

This command builds only the selected probe before it runs. The full
`cargo xtask build-dolphin-probes` command also builds MP3 probes and therefore
needs `LIBOGC_MAD_PROBE_MP3`. The selected formatter build passed without that
fixture. A Dolphin run also needs an installed executable and a Wii NAND with
the Wii System Menu. The [2026-11-22 formatter run](dolphin-runs/2026-11-22-ipc-format-aes-known-answer/batch-report.md)
used Dolphin 2609a with source revision
`324af80557f2b2115000219cbf2e096117ba989b`; it passed 844 checks with no
failures. The AES checks use a fixed NIST AES-128-CBC vector for typed, sync,
and async results. The async test also changes the staged input tail and checks
the callback result. This is emulator evidence. Physical Wii cache and DMA
validation remains open.

The fresh `runtime_probe` run at base revision
`5609ceae078986c8da4bbd00df000159cdc0a9b5` passed 851 checks with no failures
in Dolphin 2609a. It covers real Argv and Scalar heap exhaustion, async
callback re-entry, maximum 31-parameter staged `d` ownership, and real
exhaustion of both formatter `FormatState` and staged-buffer workspace
allocations. The staged-buffer case confirms `FormatState` allocates, then its
64-byte staging allocation fails and all formatter, heap, and workspace
ownership records balance. See the
[fresh workspace exhaustion evidence report](dolphin-runs/2026-11-22-ipc-format-workspace-oom-v1/README.md).

The earlier SIGABRT occurred before the probe started. Qt's arm64 initializer
aborted in `qDetectCpuFeatures` because it did not detect NEON. The sandbox
restricted the CPU-feature query. Native arm64 Dolphin 2609a started and passed
when the same command ran outside that restriction. No probe or formatter code
change was needed to fix this host launch failure.

The fresh run promotes the directly tested formatter heap exhaustion,
workspace exhaustion, callback re-entry, and maximum ownership workspace
contracts. The eight physical Wii contracts remain blocked on Broadway cache
behavior and IOS DMA coherency. Hardware
testing must include aligned and misaligned 31, 32, 33, 63, 64, and 65 byte
boundaries, dirty neighboring cache-line data, and both sync and async paths.
Dolphin evidence does not promote these hardware contracts.

The [869-check formatter run](review/evidence/ipc-format-dolphin-2026-11-22-869/README.md)
adds live AES output boundaries, real async allocator exhaustion, ES error
completion, callback re-entry failure, three linked requests, mixed output
completion, and mixed async ABI coverage. It records the source patch and raw
logs. The hardware contracts remain blocked.

The [formatter recovery follow-up](review/evidence/ipc-format-dolphin-2026-11-22-recovery/README.md)
checks a live formatted ES request immediately after each async Argv, Scalar,
FormatState, and first staged-buffer exhaustion case. It releases the workspace
reservations before the workspace recovery requests. The controlled output
boundary fixture reports the length and offset for each failed case.

The two DMA output copyback rows previously recorded only the formatter
interception checks. The fresh Linux run described below verifies both live
ES copyback contracts in Dolphin; physical cache coherency remains blocked. The sync check verifies software copyback and canary bytes. The async
controlled completion also checks the callback after copyback. The earlier run
also submits real ES requests through the emulated IOS paths. Both paths test
31, 32, 33, 63, 64, and 65 byte output buffers at offsets zero and one from a
32-byte boundary. They compare the returned title directory and check the
surrounding bytes. The async check also verifies one callback, its result and
userdata, and the output visible in that callback. See the
[live ES results](dolphin-runs/2026-11-22-ipc-format-workspace-oom-v1/runtime_probe/probe.log#L492).
Physical Wii testing must establish cache and DMA coherency for these paths.

The [2026-11-26 IPC reply and relaunch review](dolphin-runs/2026-11-26-ipc-reply-relaunch-linux/README.md)
records the reply decoder red/green test, a runtime baseline and repeat with
the same 13 unrelated failures, and a passing IOS reload probe. A private
System Menu fixture restored from the original boot content and TMD reaches
the destination after both relaunch operations. The runner still reports
timeout for a successful title handoff because the original probe cannot
emit a completion summary after replacement. See that report for the
destination evidence and remaining limits.
The [IPC follow-up](dolphin-runs/2026-11-26-ipc-followup-linux/README.md)
adds automatic foreground/background handoff results, nonzero loader-bound
cases, and delivery of a posted IOS reply across PPC client reinitialization.
The [IOS replacement follow-up](dolphin-runs/2026-11-26-ipc-ios-replacement-linux/README.md)
adds 42 passing checks for a real outstanding STM event hook across IOS 58
replacement. The old callback is not delivered and fresh IPC works, but one
request record and one deferred reservation remain allocated. The subsequent
[replacement-specific cancellation review](dolphin-runs/2026-11-26-ipc-ios-cancellation-linux/README.md)
passes 103/103 checks across three IOS replacements and restores all request
and callback capacity. It also passes the 29-check ordinary PPC reset test.
New cancellation paths require independent re-review; physical Wii behavior
remains pending.
The follow-up runtime fixture corrects eleven video checks; both known NAND
variants complete 887 checks successfully with two stock Linux Dolphin socket
option failures. The existing Linux Dolphin network patch still needs a
rebuilt executable and runtime verification.


The [probe guide](../probes/README.md) lists focused build commands, probe
coverage, Dolphin limits, and physical USB and AESND procedures. The
[hardware validation guide](wii-hardware-validation.md) explains how to record
real-console evidence.

For a group of related APIs, use the [Dolphin batch helper](api-probe-batch.md).
It records the selected checks and refreshes the evidence registry and
inventory after the run. Review the saved checks and their limits before
you accept the evidence.

## Milestone commands

Milestone commands combine checks for one area of work:

| Command | Scope |
| --- | --- |
| `cargo xtask milestone0-verify` | Role and dependency checks, source structure, inventory, phase 0 readiness, host tests, and all runtime target packages |
| `cargo xtask milestone1-verify` | Milestone 0 checks except the full runtime compile, plus LWP readiness, core target group, and LWP probe build |
| `cargo xtask milestone2-verify` | Milestone 1 readiness, storage readiness, storage target group, and storage probe build |
| `cargo xtask verify` | Milestone 0, hardware register, ABI checks, and strict runtime completeness audit |
| `cargo xtask verify-complete` | Hardware register, readiness, role and dependency, host, ABI, inventory, source, strict audit, runtime, and probe/example build gates |

Milestone 1 and 2 commands build probes. They do not run those probes on a
console. Milestone 2 checks that the LWP readiness files exist, but it does not
compile the core group or build the LWP probe. The strict completeness audit
currently reports public APIs without accepted evidence or with disallowed
classifications. Earlier checks may also fail first. See [implementation
status](implementation-status.md) for the current result.

## Evidence levels

Keep these results separate when you report work:

- Host tests check host-capable logic and selected behavior.
- Target compilation checks code for the selected PowerPC target.
- Dolphin checks the named probe under the emulator.
- Real-console tests check the recorded hardware, software, and accessory setup.

Use [evidence and open work](evidence-summary.md) to find current reports and
their limits. Use the [hardware API register](hardware-api-validation.csv) to
check which physical tests still need results.

### 2026-11-26 USB StartStop and IPC replacement scope (Linux)

The [scoped evidence](dolphin-runs/2026-11-26-usbstorage-startstop-ipc-scope-linux/README.md)
records the USB StartStop invalid-LUN error-order baseline (59 passed, 3
failed) and corrected run (62 passed, 0 failed), plus the extended IOS reload
scope run (105 passed, 0 failed). All completed capture pages are valid.
Host xtask tests passed 163/163, USB lifecycle tests passed 21/21, and the
PowerPC storage group passed. No new full runtime suite result or live USB
transport result is claimed. Physical Wii evidence remains pending.

### USB BOT buffer and CSW follow-up (2026-11-26)

[Scoped Dolphin results](dolphin-runs/2026-11-26-usbstorage-bot-buffer-linux/README.md):
64/65 before the pending-buffer correction, 65/65 after, 68/68 with CSW validation
checks, six valid final captures. Host lifecycle tests: 22/22. Controlled
fixtures do not establish live USB device transfer or physical DMA behavior.

### IPC aborted replacement scope (2026-11-26)

[Scoped reload evidence](dolphin-runs/2026-11-26-ipc-aborted-scope-linux/README.md):
108/108 in stock Dolphin, including three aborted scopes with a real pending
STM request, followed by actual successful replacements. Actual failed IOS
launch and unsent request handling remain unverified.

### GX FIFO read enable (2026-11-26)

[Scoped GX probe](dolphin-runs/2026-11-26-gx-fifo-read-linux/README.md):
27/28 before correction, 28/28 after. CP control is checked during a drained
FIFO detach, CPU-only assignment and GP restoration. Watermark interrupt
behavior remains pending.

### Linux Dolphin network options (2026-11-26)

[Scoped network results](dolphin-runs/2026-11-26-network-headless-linux/README.md):
84/84 with the patched NoGUI Null backend and independently measured native
Linux socket expectations, versus 79/84 on stock Dolphin using the same DOL.
Seven and eight valid guest framebuffer captures respectively. The default
patched run had 82/84, with host buffer-size and timeout differences.
This does not establish graphics behavior, physical Wii readback semantics,
or a clean full runtime suite. Host xtask tests passed 164/164.

### GX FIFO watermark interrupt masks (2026-11-26)

[Scoped GX evidence](dolphin-runs/2026-11-26-gx-watermark-linux/README.md):
30/32 before correction and 32/32 afterward, on stock graphical Dolphin.
Five and four valid capture pages respectively. The probe reads CP control
bits for detached, CPU-only, linked GP and linked CPU assignments. Actual
watermark interrupt delivery and physical Wii behavior remain pending.
