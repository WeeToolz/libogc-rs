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

The [probe guide](../probes/README.md) lists focused build commands, probe
coverage, Dolphin limits, and physical USB and AESND procedures. The
[hardware validation guide](wii-hardware-validation.md) explains how to record
real-console evidence.

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
