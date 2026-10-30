# libogc-rs

Rust ABI bindings, Wii runtime implementations, and safe wrappers for Wii PowerPC.

This is an active development project. Contributions and independent reviews
are welcome. Some APIs and hardware paths still need validation; see the
[implementation status](docs/implementation-status.md) before relying on them.
See [contribution guidance](CONTRIBUTING.md) to get started.
The project includes material from multiple sources; see
[license and source notices](docs/source-licenses.md).

Start with [the documentation guide](docs/README.md) for current status,
validation instructions, and detailed evidence.
For a short overview of test and review results, see
[evidence and open work](docs/evidence-summary.md).

The workspace includes a PowerPC scheduler, IOS and hardware interfaces,
storage and input services, media support, and compatibility APIs. The
implementation is incomplete and real-Wii acceptance remains open for many
paths. See [the evidence summary](docs/evidence-summary.md) for the current
limits and [Wii launch state](docs/wii-launch-state.md) for NAND transaction
behavior and its hardware acceptance limits.

## Target policy

The Wii standard library backend has build and probe support for allocation,
errors, TLS, threads, synchronization, and selected services. Complete service
and hardware acceptance remains open. Use the
[std implementation checklist](docs/wii-std-checklist.md) for current work and
acceptance gates. The [contract gap map](docs/wii-std-contract-gaps.md) records
the Rust-to-Wii API trace, unsupported operations, and required hardware evidence.

Runtime packages use the checked-in PowerPC targets. `targets/powerpc-nintendo-none.json` remains the Wii/current target. `targets/powerpc-nintendo-gamecube.json` uses the same CPU/linker model and `.cargo/config.toml` enables one registered `cfg(gamecube)` switch for that target. APIs that C hides under `__wii__`/`HW_RVL` use that shared switch, while both console targets keep the valid PowerPC EABI target environment. Ordinary host builds are limited to:

- `xtask` repository tooling;
- ABI and layout inspection;
- source inventory and dependency checks;
- code generation and documentation;
- pure tests that do not pretend to operate Wii hardware.

Most runtime crates reject non-PowerPC compilation. Packages marked `abi-only` in `config/crate-roles.csv` retain only the host path required by tooling.

## Repository structure

```text
crates/
  abi/          ABI definitions and ABI tooling
  core/         runtime, synchronization, IOS, and scheduler foundations
  hardware/     EXI, SI, DSP, audio, video, GX, GU, and ARAM
  services/     storage, USB, networking, input, and system services
  media/        audio, decoder, player, and media crates
  compatibility/ historical libogc ABI facades
examples/      Wii example programs
probes/        target-only diagnostic packages
targets/       checked-in Wii target and linker definitions
config/        roles, inventory overrides, dependency policy, and audit debt
xtask/         host-side repository commands
docs/          contracts, generated status, architecture, and review notes
```

## Milestone 1 runtime ownership

```text
ogc-core
  shared PowerPC timebase and low-memory hardware primitives
        ↓
ogc-runtime
  scheduler, deadline, deferred-executor, and interrupt-wake contracts
        ↓
ogc-lwp-driver
  PowerPC contexts, scheduler, waits, timers, deferred worker, debug ring
        ↓
ogc-lwp-sys ── ogc-lwp
  compatibility ABI    safe handles
        ↓
tuxedo-sys
  compatibility facade; no independent scheduler or timer queue
```

`ogc-ipc` blocks synchronous callers on an LWP queue. The IOS reply/ack interrupt records an IPC wake event and signals that queue. Callback execution is not performed in the interrupt handler.

The scheduler selects threads by effective priority. Threads switch when they yield, block, sleep, exit, or reach another scheduling point. External IRQ and decrementer return paths also preempt the interrupted thread when a strictly higher-priority thread is ready, as in C. Equal-priority threads rotate at explicit scheduling points; there is no periodic time slice. A non-yielding thread can delay equal- or lower-priority work. Interrupt-return behavior still requires target runtime verification.

See `docs/lwp-runtime.md` for context layout, capacities, callback context, synchronization semantics, and hardware-test instructions. Use `docs/milestone1-hardware-report.md` when reporting the real-Wii result.

## Prerequisites

- Rust and Cargo
- `rustfmt` and `clippy`
- nightly Rust with `rust-src`
- `llvm-tools-preview` for object and DOL conversion
- External MP3 fixtures for the full host suite and MP3 probe builds; see
  [dependency notes](docs/dependencies.md).

```sh
rustup toolchain install nightly
rustup component add rust-src --toolchain nightly
rustup component add llvm-tools-preview --toolchain nightly
```

The Wii standard library build checks the compiler commit and the Rust library
source hash in [`rust-std/source-lock.json`](rust-std/source-lock.json). A newer
nightly may fail that check even when `rust-src` is installed. Use a nightly
that matches the lock file for Wii std work; do not change the lock to bypass
the check. The runtime checks that do not build Wii std have separate scope.

## Host-side structural checks

```sh
cargo check -p xtask
cargo xtask check-roles
cargo xtask check-dependencies
cargo xtask test-host
cargo xtask check-inventory
cargo xtask audit-source
cargo fmt --all --check
```

`test-host` runs selected Cargo suites, standalone Rust test files, and the
Python suites registered in `xtask/src/host_tests.rs`. It fails if any suite fails.

Do not use host `cargo check --workspace` as runtime validation. Runtime packages are target-only.

## Wii target checks

Check the Milestone 1 and 2 groups:

```sh
cargo xtask check-group core
cargo xtask check-group storage
```

Check another ownership group:

```sh
cargo xtask check-group input
cargo xtask check-group network
cargo xtask check-group audio
cargo xtask check-group debug
```

Check every package marked as runtime:

```sh
cargo xtask check-all-runtime
# alias
cargo wii-check
```

The target checks use nightly `build-std=core,alloc`, the checked-in target specification, and `RUSTFLAGS=-D warnings`.

An individual package can be checked directly:

```sh
RUSTFLAGS="--check-cfg=cfg(gamecube) -D warnings" \
RUSTC="$(rustup which --toolchain nightly rustc)" \
rustup run nightly cargo check \
  -Z build-std=core,alloc \
  -Z build-std-features=compiler-builtins-mem \
  -Z json-target-spec \
  --target ./targets/powerpc-nintendo-none.json \
  -p <crate-name>
```

## Build and run the Milestone 1 and 2 probes

Use Dolphin to run registered target probes:

```sh
cargo xtask dolphin-validate
# Or run one registered probe:
cargo xtask dolphin-validate --probe lwp_probe
```

The runner writes results under `target/dolphin-validation/`. Check the probe
summary and report; a successful build or emulator launch is not a probe pass.
See [the probe guide](probes/README.md) and
[hardware validation](docs/wii-hardware-validation.md) for scope and limits.

```sh
cargo xtask build-lwp-probe
cargo xtask build-storage-probe

# Build the standard probe set instead:
cargo xtask build-probes
# alias
cargo wii-test
```

The focused probes are written to:

```text
target/xtask/probes/lwp_probe.dol
target/xtask/probes/storage_probe.dol
```

The full probe build also includes focused physical-device tests:

- `target/xtask/probes/aesnd_native_probe.dol` checks native DSP output,
  stream callback buffer replacement, ONCE and LOOP playback, same-slot voice
  replacement, stop, and callback cleanup.
- `target/xtask/probes/usbstorage_transport_probe.dol` checks physical capacity
  and reads, three reset/suspend/resume cycles, and close/reopen. It requests
  no raw sector writes. Keep a supported USB storage device attached. A USB
  filesystem already mounted by the probe logger prevents this test from running.

Build report-enabled copies under `target/xtask/probes-agent/` with
`cargo xtask build-agent-probes`. See [the probe instructions](probes/README.md#physical-aesnd-stream-and-usb-transport-coverage)
for setup, expected results, and limits. The new physical cases are built but
remain `not-tested` until real-Wii results are recorded in
[the hardware register](docs/hardware-api-validation.csv).

Build one Rust target as a DOL with an explicit feature set:

```sh
cargo xtask build-dol ogc-system-probe --bin aesnd_native_probe \
  --features agent-report \
  --output target/xtask/probes-agent/aesnd_native_probe.dol
```

The command sorts and removes duplicate feature names before the build. It writes
the DOL and a stable `.manifest.json` beside it. The manifest records the target,
feature list, DOL size, and SHA-256. Use Cargo features in the target package to
select code that the target supports. This build path applies source-level
features before linking and DOL conversion; a binary-only DOL needs a separate,
version-specific patch recipe because its code layout is unknown.

Run the LWP probe on a real Wii. It covers:

- at least 40,000 cooperative worker iterations and repeated context switches;
- thread return values and joins;
- mutex/condition-variable coordination;
- semaphore blocking and wakeup;
- timed condition waits using the Wii timebase;
- message queues;
- deferred callback execution;
- Tuxedo queue/thread forwarding and tick tasks over LWP;
- a real IOS `/dev/stm/immediate` open/close reply interrupt;
- two differently prioritized synchronous IOS callers exercising scheduler-aware serialization and priority donation;
- an asynchronous IOS completion verified to run on the deferred worker;
- scheduler counters, stack guards, and the debug-event ring.

The logger writes to the visible console and attempts persistent output at:

```text
/tmp/ogc-system-probe.log
```

When `mounted-log` succeeds, it also uses the existing mounted probe-log policy. Preserve the log and report the first failed record plus the final scheduler counters.

## Debug hooks

`ogc-lwp-sys` exposes:

- `set_debug_hook`
- `debug_counters`
- `debug_drain`
- `note_ipc_wake`

The hook can run while interrupts are masked, including from an IOS completion path. A hook must therefore be bounded, nonblocking, allocation-free, and normally limited to atomic stores or a preallocated trace sink. The built-in ring contains 256 events and tracks dropped records.

## Implementation inventory

Generate the machine-readable public API inventory and summaries:

```sh
cargo xtask inventory
```

Outputs:

- `docs/generated/implementation-inventory.csv`
- `docs/generated/implementation-status.md`
- `docs/generated/runtime-audit-debt.md`

Allowed completion classes are `hardware`, `ios`, `scheduler`, `protocol`, `pure`, and `compatibility-forwarder`.

The full completeness gate fails on `stub`, `synthetic`, `host-model`,
`fake-success`, `state-only`, `unimplemented`, or `unverified`:

```sh
cargo xtask audit-stubs
```

The gate still fails on APIs without accepted exact test or probe references.
A probe reference identifies available coverage; it does not establish a
hardware pass. Read [the implementation status](docs/implementation-status.md)
and [the source review ledger](config/static-review-scope.csv) for current work.

## Verification levels

Structural verification:

```sh
cargo xtask milestone0-verify
```

Milestone 1 compile-and-probe-build verification:

```sh
cargo xtask milestone1-verify
```

This checks repository structure, inventory freshness, all host-testable packages, the complete core target group, and builds the focused `lwp_probe.dol`. It does not replace running that probe on real hardware.

Milestone 2 compile-and-probe-build verification:

```sh
cargo xtask milestone2-verify
```

This additionally checks the storage target group and builds `storage_probe.dol`. It does not replace running that probe on real hardware.

Full project verification:

```sh
cargo xtask verify
```

Full verification includes the strict runtime-completeness gate and will not pass until later milestones remove every disallowed implementation class.

Compile acceptance gate:

```sh
cargo xtask verify-complete
```

This runs the structural, inventory, audit, target-build, and probe-build checks, including the workspace `examples` package.

## Remaining release work

The repository is shared for development and review. Public collaboration does
not mean the project has passed release acceptance. Release acceptance still
requires:

- resolution of the findings and reviews in the source review ledger;
- complete USB transport, native AESND DSP, and MAD decoder comparisons;
- exact test or probe references for the remaining unverified APIs;
- real-hardware results for the required probes and supported hardware revisions.

Source review, test compilation, DOL linking, and hardware execution are separate
checks. `cargo check --tests` checks target test code; it does not build or run
a Wii DOL test harness. See [the implementation status](docs/implementation-status.md),
[the probe README](probes/README.md), and
[the hardware validation guide](docs/wii-hardware-validation.md).
