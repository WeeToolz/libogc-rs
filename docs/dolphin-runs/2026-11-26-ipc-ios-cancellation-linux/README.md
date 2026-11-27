# IPC cancellation after actual IOS replacement

2026-11-26 local date. Stock Linux x86_64 Dolphin 2609b, private restored
System Menu fixture, offline nightly `28221559263a3976766cf305940e80e30cf9ba8a`.
Rust base `3258d37b3e4a4a6b9b401a1f55f42b6bd6a52369` plus the cancellation
changes, probe changes, local source lock and unrelated worktree edits.
C reference `7d4b8b3e7b845516f0cc2a85274879ddd8cbf142`. This is not a
clean-base build. The full runtime run preceded the final scope-only
probe additions; production IPC and IOS source did not change between them.

| Run | Passed | Failed | Valid capture pages |
| --- | ---: | ---: | ---: |
| IOS reload, replacement scope and cancellation | 103 | 0 | 10 |
| Ordinary PPC reset and async IPC | 29 | 0 | 4 |
| Runtime regression | 887 | 2 | 51 |

The [summary](summary.json) records raw result and DOL digests. The three
`*-checks.log` files are selected guest checks, not complete raw transcripts.
The private archive preserves the raw reports, logs, transport and captures,
excluding duplicated emulator user profiles. No private NAND or raw
console-identifying logs are committed.

## Behavior and verification

The earlier [42-check baseline](../2026-11-26-ipc-ios-replacement-linux/README.md)
showed that IOS replacement leaves one request record and one deferred slot
allocated. The C implementation does not explicitly reclaim abandoned
requests. Rust intentionally completes these requests with `IPC_ENOENT`
through the deferred worker, with no staged-output copyback. Requests already
handed to the worker retain their real result. Ordinary PPC mailbox reset
must not cancel outstanding replies.

The replacement scope rejects nested launch and asynchronous submission;
the owner can perform synchronous IPC. Rejection returns resources without
calling the rejected callback. The launch path clears and observes the
uncached IOS version word, checks the replacement version, resets the PPC
mailbox and requires a fresh `/dev/es` open/close round trip before retiring
old requests. If the round trip fails, old records are retained rather than
freed on uncertain ownership. The mailbox readiness polling is advisory, as
in C. The request tracker distinguishes requests still owned by IOS from
those already passed to the deferred worker.

Three consecutive same-version IOS 58 replacements each use a pending STM
hook and a formatted idle Bluetooth ACL receive. Each old callback receives
cancellation once, the formatted allocation ledger balances, the staged
caller output remains unchanged, and a fresh request works after reload.
Request capacity is 39 before submission, 37 while pending, and 39 after
replacement. Deferred capacity is 32, 30 and 32. The ordinary PPC reset
regression retains delivery of the real held reply.

The two runtime failures are the already documented stock Linux Dolphin
`SO_REUSEADDR` readback cases. No runtime checks were disabled. Host xtask
checks passed 163/163 and the PowerPC core group passed. This is scoped
Dolphin evidence, not independent re-review of all cancellation races,
failed replacement/round-trip behavior or physical Wii DMA/cache coherency.
The eight physical formatter contracts remain pending. `ogc-ipc` needs
independent re-review of this new production code; physical Wii evidence
is not a completion gate for Dolphin coverage.

Reproduce with the configured private stock NAND and offline toolchain:

```sh
cargo xtask dolphin-validate --probe ios_reload_probe --timeout-seconds 120
LIBOGC_DOLPHIN_IPC_BOUNDS=both cargo xtask dolphin-validate --probe ipc_async_probe --timeout-seconds 120
cargo xtask dolphin-validate --probe runtime_probe --timeout-seconds 240
```
