# Wii Launch-State Persistence

`ogc-wiilaunch` treats launch metadata as a required transaction. It does not call ES when the metadata for the next title is not verified.

## Transaction

The launch path validates the target ticket views before it changes NAND state. It then prepares local metadata copies and uses this order:

1. Open the state file for write.
2. Write the exact structure size.
3. Close the write descriptor successfully.
4. Reopen the file for read.
5. Read the exact structure size.
6. Close the read descriptor successfully.
7. Verify the checksum and exact content.

A plain title launch commits `NANDBOOTINFO`. Argument launches and return operations commit `NANDBOOTINFO` first and `state.dat` last. Network cleanup, video blanking, and `ES_LaunchTitle` occur only after all required files pass verification.

All persistence failures use the existing `LaunchError::Internal` and `WII_EINTERNAL` contract. A short read or write is a failure. The cached copy of each file changes only after that file passes verification.

## Host Tests

`ogc-wiilaunch-state` is the host-compatible transaction engine that production uses. Its tests inject failures at every open, read, write, and close boundary. They also test short transfers, invalid checksums, mismatched read-back data, argument serialization, structure layout, commit order, and the rule that a failed commit cannot launch.

Run the focused tests and Wii target check:

```sh
cargo xtask test-host
cargo xtask check-group core
```

## Hardware Acceptance

The automated probe does not corrupt or inject failures into Wii NAND state files. For a manual smoke test, use a known safe installed title, record the title ID and arguments, and call the typed launch API. A successful call transfers control and does not return. Repeat with `try_return_to_menu` and confirm that the System Menu starts at the requested destination.

If a typed launch call returns, record its `LaunchError`. Confirm that the current application remains active and that IOS subsystem teardown did not occur on a failed reset-to-menu request. Do not mark the path as hardware-accepted until both the title-argument and return-destination cases pass on a real Wii.
