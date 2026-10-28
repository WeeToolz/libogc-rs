# Requested parity-list closure — 2026-10-02

The user reported the preceding DVD/keyboard/LWP/IPC/API batch and the follow-on Bluetooth/LWP/GCSD/USB batch compile cleanly. This re-audit checks the requested keyboard, LWP context, IPC heap, and umbrella/API groups against the current C source-of-truth.

## Wii keyboard

- `KS_F1..KS_F20` are public, and HID F1-F12 codes map to the C symbols used by the current keymap.
- The old 31-event public queue is already a dynamically growing FIFO.
- This follow-up also removes the remaining fixed 256-byte stdin loss point by reusing the same generic dynamic queue abstraction for stdin bytes.
- Automatic 10 ms polling/300-poll rediscovery and Caps/Scroll/Num-lock modifier plus LED behavior remain present and were source-rechecked against C.

## LWP context

- All 32 paired-single images are saved/restored at the C `PPCContext.ps` offsets.
- Fresh threads use the C eight-byte initial frame and `UINT32_MAX` back-chain sentinel.
- PC/MSR/CR/LR/CTR/XER/GPR/FPSCR/FPR/GQR offsets and restore coverage were rechecked; no new adjacent context-field omission was found.

## IPC heap model

- Default heap size is 4096 bytes.
- Sixteen public heap slots are centrally tracked and allocate/free by heap id.
- Runtime bounds use the Wii IPC linker arena (`0x933e0000..0x93400000`).
- Shared/default heap users and the cached USB descriptor heap avoid per-call heap-slot consumption.
- The internal Rust DMA-staging heap remains separate from the 16 C-visible public slots.

## Umbrella/API surface

- memory-card support is re-exported as `ogc::card`;
- cast/GQR constants, setters, and conversion helpers are present;
- all 20 packed C color constants are present;
- Wii-only modules are hidden on GameCube;
- this follow-up adds the direct `ogc::isfs` alias for the existing filesystem implementation, matching the C umbrella name.

The remaining header-name differences map to existing Rust subsystem modules rather than missing implementations. No hardware-validation claim is made by this closure note.
