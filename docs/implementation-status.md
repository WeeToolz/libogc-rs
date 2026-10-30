# Runtime implementation status

The checked-in status is generated from public source APIs and explicit reviewed overrides.

- Machine-readable API inventory: `docs/generated/implementation-inventory.csv`
- Generated summary: `docs/generated/implementation-status.md`
- Classification overrides: `config/implementation-overrides.csv`
- Exact API evidence: `config/implementation-evidence.csv`
- Tracked source-audit debt: `docs/generated/runtime-audit-debt.md`

Generate and verify it with:

```sh
cargo xtask inventory
cargo xtask check-inventory
cargo xtask audit-source
cargo xtask audit-stubs
```

## Meaning of classifications

Allowed production classifications:

- `hardware`
- `ios`
- `scheduler`
- `protocol`
- `pure`
- `compatibility-forwarder`

Disallowed completion classifications:

- `stub`
- `synthetic`
- `host-model`
- `fake-success`
- `state-only`
- `unimplemented`
- `unverified`

`cargo xtask audit-stubs` fails while any public API has a disallowed classification.
Allowed classifications also require an exact evidence record. The record must
name a test or hardware probe function that contains a call to the API. Hardware,
IOS, and scheduler classifications require hardware-probe evidence. This check
confirms a syntactic call; it does not prove runtime behavior or hardware
acceptance. An API without acceptable evidence is classified as `unverified`.

## Milestone 1 implementation

The following source implementations now exist:

- PowerPC context switch and LWP thread lifecycle in `ogc-lwp-driver`;
- priority-ordered cooperative scheduling and stack guards;
- scheduler-backed queues, mutexes, condition variables, semaphores, and message queues;
- PowerPC-timebase sleep, timed waits, and callback timers;
- fixed deferred-callback worker and interrupt-safe wake surface;
- blocking synchronous IOS IPC with reply-interrupt LWP wakeups;
- Tuxedo forwarding over LWP rather than a second scheduler;
- focused `lwp-probe` logging and scheduler trace hooks;
- shared low-level timebase/Hollywood primitives, removing the previous dependency exception.

These entries have allowed declared classes, but they remain `unverified` until
their exact test or real-Wii probe evidence is recorded. A declaration is not
proof of implementation or hardware behavior.

## Source parity review status

The existing parity ledger is `config/static-review-scope.csv`. Run
`cargo xtask review-status` for current counts. `reviewed` means parity is
established for the recorded scope, with documented safety differences.
`work_status` records current activity separately.

The local C reference is now at
`ff53c4a7b7ba5587d82305e710ca65bce1092e1c`. The SDIO broadcast range and
alignment fix passed independent source review; real-Wii IOS validation remains
open. The LWP null-priority fix passed independent source review for that
bounded contract; target runtime and Wii scheduling validation remain open.
Other LWP findings and broader runtime parity work remain open. See
[SD status](review/libwiisd.md) and [LWP findings](review/liblwp.md) for
remaining comparisons.
The ledger also now includes `ogc-std-runtime`, which the previous ledger
omitted; its full parity review remains incomplete. Older review totals do
not describe the current source reference.

## Current status

The generated inventory is the source for current counts. See the
[generated summary](generated/implementation-status.md) for current API totals
and unverified counts. A test or probe reference records available evidence;
it does not by itself prove correct runtime behavior or hardware acceptance.

## Release decision — 2026-10-28

Do not release the repository as complete. The inventory has 3,385
public APIs, of which 2,400 are `unverified`; `cargo xtask audit-stubs` fails.
The inventory now includes 46 APIs from the local `mad-sys` ABI files. This
count records missing exact evidence. It does not mean that all 2,400 APIs
have defective implementations. The gate remains open until each API has the
required evidence and the audit passes.

The [source review ledger](../config/static-review-scope.csv) has 75 of 90
entries reviewed, four parity blocked, four needing re-review, and seven
incomplete. The [hardware register](hardware-api-validation.csv) has 14 passed
rows and 36 `not-tested` rows. Register validation checks evidence records; it
does not convert untested hardware into a pass.

Current USB FAT persistence and recovery have no accepted hardware result.
The [USB FAT investigation](review/usb-fat-eio-investigation-2026-10-28.md)
records four failures from an older run, but the retained evidence does not
show the first failed operation or identify the current source build. A new
run needs an identified, authorized storage medium and the hardware procedure.
The Wii std backend also has no accepted entropy source. Nonempty random
requests and default `HashMap` seeding fail under its documented policy; any
preview that includes this backend must state this supported-feature limit.

The release checks now include the existing DI retry and video state host
suites. A selection regression rejects an omitted standalone suite under
`tests/`. The full host command passes for 25 packages. The Wii runtime compile
check passes for 83 packages. The probe and example DOL builds pass after
separating Cargo invocations by package, so the LWP probe's std startup feature
cannot change the startup contract of other probe binaries. The target ABI
inspection and hardware-register evidence validator also pass.

Independent review accepted the bounded USB suspend/reset failure fixes and
AESND stream callback branch fix. The full USB transport, native DSP, and MAD
decoder comparisons remain incomplete. These results do not establish hardware
acceptance. The runtime completeness gate still fails on the unverified APIs,
and the source review ledger still has open findings and reviews. This tree is
not ready for release. See [USB review](review/libusbstorage.md),
[AESND review](review/libaesnd.md), and `cargo xtask review-status`.

The GU update adds runnable host coverage for all 45 public GU math APIs. The
host package compiles the same source modules used by the PowerPC crates.
See [the GU review](review/libgu.md) for scope and test commands.

| Area | Target compile | Runtime implementation | Hardware acceptance |
| --- | --- | --- | --- |
| Bluetooth/WPAD/keyboard | pass | declared complete; exact evidence pending | pending |
| Audio/DSP | pass | ASND and native AESND probes exist; full DSP review remains open | pending |
| USB Gecko/debugger | pass | declared complete; exact evidence pending | pending |
| Fonts | pass | declared complete; exact evidence pending | pending |
| Network/storage/lifecycle | pass | declared complete; exact evidence pending | pending |

`cargo xtask audit-stubs` reports unverified APIs until exact test and hardware
probe evidence is recorded.
The remaining non-hardware cleanup is to separate the reviewed host-only
compatibility branches in lwBT, TinySMB, WPAD, and keyboard code. Hardware
acceptance remains a separate gate.
