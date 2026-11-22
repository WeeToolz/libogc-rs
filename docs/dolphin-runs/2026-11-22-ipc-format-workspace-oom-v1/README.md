# IPC formatter workspace exhaustion evidence

## Run result

- Base source revision: `5609ceae078986c8da4bbd00df000159cdc0a9b5`
- Worktree: dirty; the probe included the formatter workspace OOM checks
- Dolphin: `2609a` (native arm64)
- Probe: `runtime_probe`
- Result: **851 passed, 0 failed**
- Runtime DOL SHA-256: `d2fdaf702a8c0f820ad97e52077436cf4e4928d3c9b91029e1098cee61cdb504`
- Run duration: 96.408 seconds

The runner recorded the source revision, Dolphin path and version, DOL hash,
arguments, and pass result in [`suite.json`](suite.json) and
[`result.json`](runtime_probe/result.json). The targeted PowerPC build compiled
the current worktree before Dolphin started.

The run command was:

```sh
cargo xtask dolphin-validate \
  --probe runtime_probe \
  --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin \
  --artifact-dir docs/dolphin-runs/2026-11-22-ipc-format-workspace-oom-v1 \
  --timeout-seconds 600
```

## Workspace OOM checks

The raw [`probe.log`](runtime_probe/probe.log) records these successful checks:

- Real `FormatState` workspace allocation exhaustion, with all held workspace
  reservations freed once and the formatter heap ledger balanced:
  [line 560](runtime_probe/probe.log#L560).
- Real staged-buffer allocation exhaustion after `FormatState` allocation,
  with the 64-byte staged allocation failing and state, Argv, heap, and
  workspace ownership balanced:
  [line 561](runtime_probe/probe.log#L561).
- Real Argv and Scalar heap exhaustion and cleanup:
  [lines 556–557](runtime_probe/probe.log#L556).
- Async callback re-entry:
  [line 538](runtime_probe/probe.log#L538).
- Maximum 31-parameter staged `d` ownership:
  [line 474](runtime_probe/probe.log#L474).
- Full probe summary:
  [line 1250](runtime_probe/probe.log#L1250).

The allocation failures use the production workspace allocator. The test does
not use formatter role-failure hooks to cause either workspace failure. It
holds real workspace allocations, submits formatter calls, and checks the
independent workspace and formatter heap ledgers. The full `runtime_probe` suite
passed, so the later formatter checks also ran after the OOM cases.

## Limits

Dolphin proves these allocation and cleanup paths in the emulator. It does not
prove Broadway cache behavior or IOS DMA coherency. Eight formatter contracts
remain blocked until physical Wii testing covers aligned and misaligned 31, 32,
33, 63, 64, and 65 byte boundaries, dirty neighboring cache-line data, and
both sync and async paths.

## Raw artifacts

- [Runner suite report](suite.json)
- [Probe result](runtime_probe/result.json)
- [Probe log](runtime_probe/probe.log)
- [Raw probe transport](runtime_probe/probe-transport.bin)
- [Dolphin stderr and emulator log](runtime_probe/stderr.log)
- [Dolphin stdout](runtime_probe/stdout.log)

Raw logs and transport are preserved as captured.
