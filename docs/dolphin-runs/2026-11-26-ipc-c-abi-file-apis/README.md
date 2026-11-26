# IPC file-operation C ABI Dolphin run

The existing `runtime_probe` and `ipc_async_probe` called the new `IOS_*`
exports in Dolphin 2609a. The runner used a private NAND copy. The runtime probe
used the settings file for read-only checks and its existing `/tmp` ISFS fixture
for write and asynchronous checks.

Command:

```sh
cargo xtask dolphin-validate --no-build --probe runtime_probe --probe ipc_async_probe \
  --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin \
  --timeout-seconds 180 --artifact-dir /private/tmp/ipc-c-abi-dolphin-final
```

Both probes passed:

- `runtime_probe`: 883 checks, 0 failures; DOL SHA-256 `85bfc0c2d640b842d5c554c6f4128ea91fc816e5c0faf3952a4f3938fbbf6bf7`.
- `ipc_async_probe`: 16 checks, 0 failures; DOL SHA-256 `5dd9fa6803395ba954a67c5b760a0a792131f09dd8c30492e6bf2f6c9488081d`.

The checked-in `suite.json`, per-probe `result.json`, and probe transport logs
record the results. The temporary Dolphin screenshots and full stderr logs were
not retained. Dolphin evidence does not establish physical Wii behavior.
