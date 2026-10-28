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
