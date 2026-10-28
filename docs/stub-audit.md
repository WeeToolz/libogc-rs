# Stub and placeholder audit

Milestone 0 replaces the previous literal phrase scan and removes the inaccurate claim that no placeholder PowerPC path remains.

## Structural source audit

```sh
cargo xtask audit-source
```

This check scans runtime source for target-model gates synthetic device queues synthetic device storage shadow-state placeholders and empty public functions. Existing debt must be listed in `config/audit-allowlist.csv` with:

- exact path
- marker
- reason
- owner
- removal milestone

Untracked findings and stale allowances fail the check. The generated debt report is `docs/generated/runtime-audit-debt.md`.

## Public API completeness audit

```sh
cargo xtask audit-stubs
```

This runs the structural audit checks the generated public-API inventory and fails for every API classified as:

- stub
- synthetic
- host-model
- fake-success
- state-only
- unimplemented
- unverified

The command fails until all disallowed entries are resolved. An `unverified`
entry can lack a required test or probe reference even when implementation code
exists. Passing `audit-source` means the structural audit has no untracked
findings; it does not mean the runtime is complete.

An allowed implementation classification must have an exact entry in
`config/implementation-evidence.csv` with:

- crate-relative implementation path, public symbol, and implementation line
- the declared classification
- `host-test` or `hardware-probe` evidence kind
- an existing evidence source path and named test or probe function
- an API call inside that function, checked from the Rust syntax tree

The audit accepts host tests for `pure`, `protocol`, and
`compatibility-forwarder` entries. It requires a hardware probe for `hardware`,
`ios`, and `scheduler` entries. Missing evidence produces the disallowed
`unverified` classification. Host evidence must name a `#[test]` function.
Hardware evidence must name a function in `probes/`. Stale, duplicate,
mismatched, missing, or invalid evidence fails inventory generation. This check
confirms a syntactic call; it does not prove runtime behavior or hardware
acceptance.

`cargo check --tests` checks target test code without running it. A linked DOL
provides a runnable probe, but hardware acceptance requires that probe to run
on the stated device and validate real results. Use
[the probe README](../probes/README.md) for runnable suites and
[the hardware guide](wii-hardware-validation.md) for the evidence record.

Use `cargo xtask verify-complete` for the combined compile gate when you also want role, dependency, ABI, inventory, and probe-build checks.

## Reviewed no-op policy

A legitimate no-op must have a specific architectural reason. Milestone 0 keeps reviewed exceptions in the central allowlist because custom Rust attributes would require a proc-macro or compiler tool registration. Future work may replace this with a namespaced audit attribute. Generic success returns and silent unsupported operations are not legitimate no-ops.

### Discarded operation results

The source inventory treats `let _ = operation()` as a review signal, not as
proof of a `fake-success` implementation. The reviewer must trace the result
and confirm that required failures reach the caller or a supported diagnostic
channel. Matching C's ignored result is not sufficient to satisfy the Real
Implementation Policy. Such functions require the normal evidence path;
without exact evidence they remain `unverified`, and the generated inventory
records the discarded-result review note in `known_limitations`.
