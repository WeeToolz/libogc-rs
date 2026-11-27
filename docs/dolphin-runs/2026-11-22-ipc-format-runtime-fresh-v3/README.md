# IPC formatter runtime evidence

## Run result

- Source revision: `5609ceae078986c8da4bbd00df000159cdc0a9b5`
- Dolphin: `2609a` (native arm64)
- Probe: `runtime_probe`
- Result: **849 passed, 0 failed**
- Runtime DOL SHA-256: `ba177cb5a3804dca39853469fc408c3476cabe2f81d4012c01e2aba541bc5ea8`
- Run duration: 96.443 seconds

The targeted PowerPC probe build created this DOL from the source revision
above. The final native arm64 run used that fresh DOL. The runner recorded the
revision, executable, version, arguments, DOL hash, and result in
[`suite.json`](suite.json) and
[`result.json`](runtime_probe/result.json).

The command used for the successful run was:

```sh
cargo xtask dolphin-validate \
  --no-build \
  --probe runtime_probe \
  --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin \
  --artifact-dir docs/dolphin-runs/2026-11-22-ipc-format-runtime-fresh-v3 \
  --timeout-seconds 600
```

To build the selected PowerPC probe and run it in one command, remove
`--no-build`.

## New formatter evidence

The raw [`probe.log`](runtime_probe/probe.log) records:

- Maximum 31-parameter staged `d` ownership with all workspace allocation and
  free events balanced: [line 474](runtime_probe/probe.log#L474).
- Async callback re-entry submits a distinct second request exactly once:
  [line 538](runtime_probe/probe.log#L538). The check covers separate userdata
  and outputs, expected values, and balanced request, formatter heap, and
  workspace ledgers.
- Real formatter heap exhaustion rejects Argv allocation and frees Argv plus
  the first Scalar exactly once: [lines 556–557](runtime_probe/probe.log#L556).
- Formatter use recovers on the same heap after exhaustion:
  [line 559](runtime_probe/probe.log#L559).
- The final complete probe summary is at
  [line 1248](runtime_probe/probe.log#L1248).

The run also includes the prior formatter ABI, parser, scalar and vector,
known-answer AES, sync and async request, concurrency, copyback, and cleanup
checks. Dolphin proves functional behavior in the emulator. It does not prove
Broadway cache behavior or IOS DMA coherency.

## Host launch diagnosis

The earlier `SIGABRT` occurred before the probe started. The bundled arm64 Qt
initializer aborted in `qDetectCpuFeatures` because the restricted host process
did not detect NEON. The Dolphin crash report and the message naming `neon`
identify this startup failure. Native arm64 Dolphin 2609a reported its version
and completed the probe when launched outside the restricted sandbox. No
probe or formatter code change was needed.

## Remaining hardware evidence

These eight contracts remain blocked until physical Wii evidence is available:

- `ipc-format.sync.aes-live-input`
- `ipc-format.async.aes-live-input`
- `ipc-format.sync.ios-output-boundaries`
- `ipc-format.async.ios-output-boundaries`
- `ipc-format.sync.live-input-buffer`
- `ipc-format.async.live-input-buffer`
- `ipc-format.async.callback-less`
- `ipc-format.sync.large-live-input`

Physical testing must cover aligned and misaligned input and output boundaries
at 31, 32, 33, 63, 64, and 65 bytes, dirty neighboring cache-line data, and both
sync and async paths.

## Raw artifacts

- [Runner suite report](suite.json)
- [Probe result](runtime_probe/result.json)
- [Probe log](runtime_probe/probe.log)
- [Raw probe transport](runtime_probe/probe-transport.bin)
- [Dolphin stderr and emulator log](runtime_probe/stderr.log)
- [Dolphin stdout](runtime_probe/stdout.log)

Raw artifacts are preserved as captured.
