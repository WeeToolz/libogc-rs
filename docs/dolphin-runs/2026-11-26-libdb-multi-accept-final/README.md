# libdb multiple TCP connections in Dolphin

The existing `debugger_probe` ran in Dolphin df4bd44 and passed all 23 checks.
The final DOL SHA-256 was
`aa8587bc2a678097ea307bd1964852fd10240a6cff72e89cfa258354e8aa0713`.
The command was:

```sh
DOLPHIN_METAL_BIN="$PWD/target/dolphin-usb-build/Binaries/DolphinQt.app/Contents/MacOS/DolphinQt" cargo xtask dolphin-validate --probe debugger_probe --dolphin "$PWD/docs/dolphin-runs/2026-11-15-gx-efb-metal/dolphin-metal.sh" --artifact-dir docs/dolphin-runs/2026-11-26-libdb-multi-accept-final
```

The standard Dolphin profile has no EXI BBA. The probe checked invalid TCP
handles, socket allocation, and the missing-BBA initialization path. It did
not complete a TCP handshake or test multiple accepted sockets, so this is
not runtime evidence for the new queue or per-socket routing. The suite report,
probe result, transcript, transport log, and four captured pages provide the
available evidence. The private Dolphin profile was removed after the run.

Physical Wii behavior remains pending.
