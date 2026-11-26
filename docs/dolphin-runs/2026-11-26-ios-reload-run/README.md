# IOS reload probe

Run from the repository root:

```sh
cargo xtask dolphin-validate --probe ios_reload_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --timeout-seconds 60 --artifact-dir docs/dolphin-runs/UNIQUE-RUN
```

The runner builds the selected probe and copies the configured Dolphin NAND
to a private profile. The probe reloads the IOS version used at boot, checks
the return code and version, then opens and closes `/dev/fs`. A run passes only
when the probe reports all checks as passed. Check the Dolphin IOS log for a
second IOS start to confirm that the reload was attempted. The profile can be
removed after its logs are saved. Do not use this Dolphin result as physical
Wii evidence.

This run used Dolphin 2609a and IOS 58. It reported 11 passing checks and no
failures. `ios_reload_probe/dolphin.log.gz` records the two IOS 58 starts. The
profile was removed after the log was copied. This run does not cover pending
IPC requests, another IOS version, or the system menu handoff.
