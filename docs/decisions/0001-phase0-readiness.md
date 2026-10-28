# 0001: Milestone 0 structural readiness

Date: 2026-07-11

## Status

Accepted for structural work only. This decision does not certify runtime subsystem completion.

## Decision

Milestone 0 requires:

- an explicit role layer owner group and host policy for every workspace package;
- mechanically checked dependency direction with reviewed temporary exceptions;
- target diagnostics outside production crate directories;
- a shared target-only runtime contract crate;
- a generated public-API implementation inventory;
- explicit tracking of known synthetic disabled state-only and unimplemented APIs;
- corrected documentation that does not equate target compilation with hardware implementation;
- group-based and all-runtime target check commands.

Required commands:

```sh
cargo xtask check-roles
cargo xtask check-dependencies
cargo xtask inventory
cargo xtask check-inventory
cargo xtask audit-source
cargo xtask milestone0-verify
```

`cargo xtask audit-stubs` and full `cargo xtask verify` remain stricter completion gates and are expected to fail until later milestones remove disallowed implementation classes.

## Rationale

Subsystem rewrites should not be mixed into the dependency and ABI restructuring. The repository first needs a reliable map of ownership APIs known debt and validation coverage.

## Consequences

- Adding a package without role metadata fails Milestone 0 verification.
- Adding an upward dependency without a reviewed exception fails verification.
- Public runtime APIs absent from the generated inventory make the inventory stale.
- Known implementation debt is visible and assigned to a milestone.
- Runtime completion claims must be derived from the inventory rather than prose.
