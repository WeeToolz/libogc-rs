# FAT metadata and partial-transfer evidence — 2026-11-27

`before`: baseline production code with the new metadata assertions.
`after`: metadata correction; complete Dolphin 2609b run, 58 passes, zero failures, five valid pages.
`final`: metadata and partial-transfer corrections; PowerPC build and selected guest assertions pass, but capture times out before the complete report.
The host suite passes 170 tests with both corrections. `summary.json` records digests of the raw local artifacts. Only selected checks and report metadata are included; private NAND and emulator profiles are excluded.
