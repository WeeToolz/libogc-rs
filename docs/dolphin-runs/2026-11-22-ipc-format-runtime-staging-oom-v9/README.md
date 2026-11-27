# IPC formatter allocator exhaustion evidence

## Run result

- Base source revision: `5609ceae078986c8da4bbd00df000159cdc0a9b5`
- Working tree: dirty; the probe source contains the workspace OOM checks
- Dolphin: `2609a` (native arm64)
- Probe: `runtime_probe`
- Result: **851 passed, 0 failed**
- Runtime DOL SHA-256: `d2fdaf702a8c0f820ad97e52077436cf4e4928d3c9b91029e1098cee61cdb504`
- Run duration: 96.505 seconds

The runner records the source revision, dirty state, Dolphin executable,
version, arguments, DOL hash, and result in [`suite.json`](suite.json) and
[`result.json`](runtime_probe/result.json).

The run command was:

```sh
cargo xtask dolphin-validate \
  --probe runtime_probe \
  --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin \
  --artifact-dir docs/dolphin-runs/2026-11-22-ipc-format-runtime-staging-oom-v9 \
  --timeout-seconds 600
```

## Real allocator checks

The probe uses the production 512 KiB LWP workspace allocator. It does not set
the formatter's role-failure hook for either workspace OOM check.

- [`FormatState` exhaustion](runtime_probe/probe.log#L560) fills the workspace
  until a real state allocation fails. The formatter returns `IPC_ENOMEM`,
  leaves the independent formatter heap ledger balanced, and frees each held
  workspace allocation once.
- [Staged-buffer exhaustion](runtime_probe/probe.log#L561) leaves an isolated
  block for `FormatState`, consumes the remaining workspace, and sends a
  misaligned 33-byte `d` input. The state allocation succeeds; its 64-byte
  staged-buffer allocation fails. The probe verifies `IPC_ENOMEM`, exact
  workspace allocation/free pairs, Argv cleanup, and a balanced formatter
  heap ledger.
- The same complete run also passes the real Argv and Scalar IOS heap
  exhaustion checks, async callback re-entry, and maximum 31-parameter staged
  ownership stress.
- The full probe summary is at
  [line 1250](runtime_probe/probe.log#L1250).

These results complete Dolphin evidence for the sync allocation cleanup rows.
They do not prove Broadway cache behavior or IOS DMA coherency.

## Raw artifacts

- [Runner suite report](suite.json)
- [Probe result](runtime_probe/result.json)
- [Probe log](runtime_probe/probe.log)
- [Raw probe transport](runtime_probe/probe-transport.bin)
- [Dolphin stderr and emulator log](runtime_probe/stderr.log)
- [Dolphin stdout](runtime_probe/stdout.log)

Raw logs are preserved as captured.
