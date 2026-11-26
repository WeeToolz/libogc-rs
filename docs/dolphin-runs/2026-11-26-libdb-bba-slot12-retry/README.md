# libdb debugger probe with Dolphin built-in BBA

The existing `cargo xtask dolphin-validate` runner sets `Dolphin.Core.SlotB=7` for the standard profile. This wrapper adds `Dolphin.Core.SlotB=12` before the `--exec` DOL argument, so the built-in BBA setting takes precedence. A stub executable confirmed that the wrapper preserves each argument as a separate value.

Run command:

```sh
DOLPHIN_METAL_BIN=/path/to/DolphinQt cargo xtask dolphin-validate \
  --probe debugger_probe \
  --dolphin docs/dolphin-runs/2026-11-26-libdb-bba-slot12-retry/bba-slot-wrapper.sh \
  --artifact-dir docs/dolphin-runs/2026-11-26-libdb-bba-slot12-retry \
  --timeout-seconds 25
```

The probe reached its summary path, but it timed out while the summary tried to capture a page through USB Gecko. The debugger trace ends in `ProbeDisplay::capture_page` while it sends bytes over USB Gecko. The run did not produce a summary, so it does not prove the TCP checks passed. See `suite.json`, `debugger_probe/result.json`, and `debugger_probe/timeout-diagnostic.json`.
