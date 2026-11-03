# libogc-rs

`libogc-rs` is a Rust library for Wii and GameCube software. It provides ABI
bindings, runtime and hardware services, and safe wrappers for parts of the
libogc API. The project is in active development. Many APIs still need exact
test evidence, and real-console acceptance remains incomplete.

## Project status

Do not treat a successful build or Dolphin run as proof that an API works on a
real console. The implementation inventory has many APIs without accepted
test or probe evidence. See [implementation status](docs/implementation-status.md)
and [evidence and open work](docs/evidence-summary.md) before you depend on a
subsystem.

Release work includes source reviews, evidence for unverified APIs, and
real-console results. See [implementation status](docs/implementation-status.md)
for the current work list.

## Features

The workspace contains implementations for these areas:

- PowerPC runtime, threads, and synchronization;
- IOS and IPC interfaces;
- storage and USB services;
- input, graphics, and audio;
- networking and system services;
- historical libogc compatibility APIs.

Each area has a different level of implementation and validation. See the
[API inventory](docs/generated/implementation-status.md) and the linked
component reviews for details.

## Supported targets

The checked-in PowerPC target specifications support Wii and GameCube builds.
Wii support is broader. Some APIs apply only to Wii, and GameCube support is
limited to the implemented shared services and hardware paths. Most runtime
crates do not support host builds.

Host checks cover repository structure, ABI data, and selected pure logic. They
do not validate console hardware. See [crate roles](docs/crate-roles.md) and
[Wii hardware validation](docs/wii-hardware-validation.md).

## Prerequisites

- Rust and Cargo;
- `rustfmt` and `clippy`;
- nightly Rust with `rust-src`;
- `llvm-tools-preview` for object and DOL conversion;
- external MP3 fixtures for the full host suite and MP3 probe builds. See
  [dependency notes](docs/dependencies.md).

Install the nightly components with:

```sh
rustup toolchain install nightly
rustup component add rust-src --toolchain nightly
rustup component add llvm-tools-preview --toolchain nightly
```

The Wii standard library build checks the compiler commit and Rust library
source hash in [`rust-std/source-lock.json`](rust-std/source-lock.json). A
different nightly can fail this check. Use the version named by the lock file
for Wii standard library work.

## Quick start

Check the repository tools and host tests:

```sh
cargo check -p xtask
cargo xtask test-host
```

Check a Wii runtime group and build the standard probe set:

```sh
cargo xtask check-group core
cargo xtask build-probes
```

Run a registered probe in Dolphin:

```sh
cargo xtask dolphin-validate --probe lwp_probe
```

The runner saves a report under `target/dolphin-validation/`. Read the report
to see which checks passed. Dolphin does not replace real-Wii validation.
See the [probe guide](probes/README.md) and [verification guide](docs/verification.md)
for more commands and limits.

## Repository structure

```text
crates/       ABI, runtime, hardware, service, media, and compatibility crates
examples/     Wii example programs
probes/       target diagnostics and shared host test harnesses
targets/      checked-in Wii and GameCube target definitions
config/       crate roles, inventory evidence, and audit policy
xtask/        repository checks, builds, and probe runners
docs/         contracts, status, validation records, and review notes
```

## Testing and validation

Validation has separate levels: host checks, target compilation, Dolphin probe
runs, and real-console tests. A pass at one level does not prove a pass at the
next level. Real-Wii acceptance remains open for many APIs and hardware paths.

Use the [verification guide](docs/verification.md) for the full check commands.
Use the [probe guide](probes/README.md) to build and run probes. Use the
[hardware validation guide](docs/wii-hardware-validation.md) to record physical
console results.

## Documentation

- [Documentation guide](docs/README.md)
- [Implementation status](docs/implementation-status.md)
- [Evidence and open work](docs/evidence-summary.md)
- [LWP runtime design](docs/lwp-runtime.md)
- [Verification commands](docs/verification.md)
- [Probe instructions](probes/README.md)
- [Hardware validation](docs/wii-hardware-validation.md)

## Contributing, credits, and licensing

Contributions and independent reviews are welcome. Read
[`CONTRIBUTING.md`](CONTRIBUTING.md) and the repository
[`AGENTS.md`](AGENTS.md) before you make changes.

This project uses the C [libogc](https://github.com/devkitPro/libogc) project
as a reference. It is an altered Rust implementation and does not claim
authorship of the original library. See [source licenses and notices](docs/source-licenses.md)
for upstream credits, component origins, and license details.
