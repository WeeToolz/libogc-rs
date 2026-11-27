# HBC return-stub recovery check

Status: partial Dolphin evidence. The test script exited 1. It does not qualify
physical Wii recovery.

## Build and fixture

- Dolphin: 2609a.
- Probe: `dev_wrapper_probe`, built from libogc-rs revision `0d7fcbb9`.
- DOL SHA-256: `63dab41e6eae7dd8646d90c81e9e14fda05778d22b09115d26ec7716f5566cbf`.
- ELF SHA-256: `81a55004c06290870373cd2616f7bb8d866c2ec5c954ec0157bff0f172acdc59`.
- The runner copied the local Dolphin NAND to a private profile. It did not
  install or change a title in the source NAND.
- Return title: installed OHBC, `000100014f484243`. Its TMD SHA-256 is
  `f84e20c6e30d6dfe77002a66260d4fce42888d3e567cbb81b1990cee6e2effb4`; its
  boot content SHA-256 is
  `5d3f61d8e7e4304461d361b269b93777de4fb08c03683413cc85b8310fd4f610`.
  The existing runner checked the boot content size and SHA-1 against the TMD.

## Observed behavior

- The development wrapper armed recovery before `main` and saved the caller
  stub. The 10-second deadline expired with exit status `-124`.
- The runner observed reset stages 13, 14, 15, 26, 7, 12, 27, 28, 6, 10, 11,
  16 through 25 in the expected order. It observed the caller stub restored and
  its real, virtual, and `_main` entry points execute.
- Dolphin stopped at the installed OHBC boot entry. The runner's read-only
  Wiiload readiness check passed.
- The Dolphin log reports that OHBC completed startup, rendered its first
  scene to XFB, enabled the display, and reached the Wiiload `Listening` state.

## Verification limit

The test did not finish successfully. Its screen capture step tried to read VI
registers at `0xCC002000`; Dolphin's debugger returned zero bytes. The Computer
Use runtime could not load, so no separate screenshot was captured. The saved
`result.json` therefore records `success: false`, and the returned HBC screen
has not been visually verified. Keep recovery qualification and all physical
Wii cache and DMA checks open.

The exact runner output is in `result.json` and `dolphin-console.log`. The
private NAND copies and temporary DOL/ELF files were removed after the run.
