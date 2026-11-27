# IPC formatter workspace exhaustion evidence

## Run result

- Base source revision: `5609ceae078986c8da4bbd00df000159cdc0a9b5`
- Probe source diff SHA-256: `91fa49bbb498d8b60f51ee11a81645f9665d73cd7fa0e6d69762f0fbcb27720b`
- Dolphin: `2609a` (native arm64)
- Probe: `runtime_probe`
- Result: **850 passed, 0 failed**
- Runtime DOL SHA-256: `687905cdfbaa9d24a05c3e3e9177067dab7483c5f0096da68619a002aad22a14`
- Run duration: 96.485 seconds

The repository was dirty because the probe source included the OOM check. The
suite report records the base revision, dirty state, Dolphin executable,
version, arguments, DOL hash, and result in [`suite.json`](suite.json). The
probe source diff hash above identifies the added source change used for the
build.

The successful command was:

```sh
cargo xtask dolphin-validate \
  --probe runtime_probe \
  --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin \
  --artifact-dir docs/dolphin-runs/2026-11-22-ipc-format-runtime-workspace-oom-v1 \
  --timeout-seconds 600
```

## Evidence

The probe used the production LWP workspace allocator. It held tracked
allocations until the fixed workspace arena rejected further allocation, then
called `IOS_IoctlvFormat` so its `FormatState` allocation failed naturally.
The check did not use the formatter allocation role hook. It verified the
formatter returned `IPC_ENOMEM`, its independent heap ledger stayed balanced,
and each reserved `(pointer, size)` had exactly one allocation and one free.
The event count stayed within the 64-event capacity.

Evidence is at:

- [Real workspace exhaustion check, probe log line 560](runtime_probe/probe.log#L560)
- [Full suite summary, probe log line 1249](runtime_probe/probe.log#L1249)
- [Probe result](runtime_probe/result.json)
- [Runner suite report](suite.json)
- [Raw probe transport](runtime_probe/probe-transport.bin)
- [Dolphin stderr and emulator log](runtime_probe/stderr.log)
- [Dolphin stdout](runtime_probe/stdout.log)

The run promotes the existing `ipc-format.sync.heap-failure-paths` State
allocation row because the contract now has direct real-OOM evidence for
`FormatState`. It does not prove staged-buffer OOM; that remains actionable.

## Remaining limits

This Dolphin run does not prove Broadway cache behavior or IOS DMA coherency.
The eight formatter contracts that require those physical Wii behaviors
remain blocked. Hardware coverage must include aligned and misaligned 31, 32,
33, 63, 64, and 65 byte boundaries, dirty neighboring cache-line data, and
sync and async paths.
