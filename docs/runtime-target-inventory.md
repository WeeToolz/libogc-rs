# Runtime target inventory

The previous hand-maintained table has been replaced because it described target-gated crates as implemented without examining individual APIs.

The current inventory is generated from public source functions and methods:

- `docs/generated/implementation-inventory.csv`
- `docs/generated/implementation-status.md`

Each API row records:

- crate
- symbol
- interface kind
- target implementation file and line
- implementation classification
- probe coverage
- known limitations

Runtime APIs must name a focused probe coverage entry, and the completeness gate rejects runtime rows that still rely on a placeholder probe name.

`config/implementation-overrides.csv` records reviewed exceptions to the crate-level default classification. `cargo xtask check-inventory` fails when generated output is stale.

Target-only compilation is not proof of real hardware behavior. Hardware validation state remains part of each API limitation and subsystem documentation.

IPC formatter contract evidence has its own status model in `docs/generated/ipc-format-status.md`. `verified` means all evidence for the stated contract is complete. `partial` means some evidence exists and more work is possible in the current environment. `blocked` means the remaining evidence needs an unavailable prerequisite. `unverified` means verification is actionable but sufficient evidence is missing. The actionable metric is the `unverified` count; do not combine it with `blocked` work.

Formatter contracts that pass in Dolphin but still need physical Wii validation remain blocked until hardware is available. The hardware run must check Broadway cache behavior and IOS DMA coherency for aligned and misaligned 31, 32, 33, 63, 64, and 65 byte boundaries, dirty neighboring cache-line data, and sync and async paths.
