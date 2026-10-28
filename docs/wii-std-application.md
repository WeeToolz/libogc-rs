# Wii std application acceptance

Application acceptance depends on the services that an application uses.
A successful build does not establish acceptance on a real Wii. The tracked
examples do not select a first std conversion candidate at this time.

The allocator, scheduler, drivers, and `ogc-std-runtime` remain `no_std`.
Applications can use the prepared Wii std compiler when their required
contracts and dependencies have passed the applicable checks.

## Build an independent application

Use an explicit manifest to build a crate outside the library workspace:

```sh
cargo xtask build-dol my-app --bin my-app --manifest-path /path/to/my-app/Cargo.toml
cargo xtask build-wii-std-app my-app --bin my-app --manifest-path /path/to/my-app/Cargo.toml
```

Both commands use the shared compiler preparation and DOL conversion path.
The manifest selects the application's dependencies and features. The std
command uses the pinned, prepared Wii standard library. Each command writes
a DOL and a manifest with the artifact hash and build selection.

## Required review

Review the application's actual source and resolved dependency graph. Record
its startup and return path, allocator ownership, panic diagnostics, TLS,
synchronization, input, storage, clocks, network, and entropy requirements.
Test the services that it uses through their production APIs on a real Wii.
Record the console model, DOL hash, test results, and open failures.

Default `HashMap` seeding still requires a verified entropy source. A game
pseudorandom sequence is not a secure entropy source. Do not enable a service
from a successful link or an emulator result alone.

See the [readiness checklist](wii-std-checklist.md),
[service probes](wii-std-service-probes.md), and
[combined hardware probe](wii-std-readiness-probe.md) for the current evidence
and remaining acceptance work. The full API verification backlog is separate
from an application's required service gates.
