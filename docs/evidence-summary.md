# Evidence and open work

This page is a starting point for contributors. Detailed records remain in the
linked ledgers. Results apply to the recorded build and test scope; rerun a
check before making a claim about a later change.

| Evidence | What it establishes | Current limit |
| --- | --- | --- |
| [API verification plan](api-verification-plan.md) | Work order and package scope for open API evidence | Counts are a dated baseline; check the inventory for current status. |
| [API inventory](generated/implementation-status.md) | Tracked public APIs and their evidence classifications | An evidence reference does not prove the API behaves correctly. The completeness audit has open entries. |
| [Source review ledger](../config/static-review-scope.csv) | Review status and outstanding parity work by component | A reviewed scope does not establish hardware behavior. Run `cargo xtask review-status` for current counts. |
| [Hardware register](hardware-api-validation.csv) | Probe scope, run identity, and recorded Wii results | Many rows are not tested. Use `cargo xtask audit-hardware-register` to validate records, not hardware behavior. |
| [Dolphin probes](../probes/README.md) | Target execution for the checks in each probe | Dolphin does not replace a physical Wii run. Check the probe source and its report for exact coverage. |
| [Wii std checklist](wii-std-checklist.md) | Work and evidence for the standard library backend | Service and hardware acceptance remain incomplete. |

The [implementation status](implementation-status.md) records open API and
parity work, including USB FAT persistence evidence and Wii std entropy
support. These are collaboration tasks, not public-sharing gates.

An isolated clean source archive passed `cargo xtask test-host` with the
external MP3 fixtures and `cargo xtask check-all-runtime` for 83 Wii runtime
packages on 2026-10-28. The host suite includes 109 vendored decoder tests.
These results cover host tests and target compilation only.

The PowerPC `libmad_probe` DOL ran in Dolphin but failed. It reported 6 passes
and 4 failures: full-rate and half-rate PCM checksums, VI retrace timing, and
probe logging. The run used DOL SHA-256
`c349deb295e254acf181b38df185712e661aed33c43fbe1211b66c33269f8187`; its
local, untracked report is `target/dolphin-validation/1793219409-ca7e49690f3c/suite.json`.
The run used the code at revision `ca7e496`; only documentation changed after
that run. Earlier runs reported the same checks. The failure causes remain
open, and this result does not establish real-Wii behavior.

The completeness gate currently reports 2,298 APIs without accepted exact
test or probe evidence. The hardware register has 14 passed and 36 not-tested
rows. These counts describe evidence gaps; they do not mean that every API is
broken or tested. No real-Wii run was made for this repository preparation.

The [review directory](review/) contains component-level findings. The
[review workflow](review/REVIEW-WORKFLOW.md) explains how to compare a component
against its C reference. The C source is a separate checkout and its path in
older records describes the original development environment. The
[hardware evidence directory](hardware-evidence/) holds selected saved run
records; some historical notes refer to local artifacts that are not included.

The public Git history was consolidated for this repository. Older commit IDs
in historical records identify the original development history and may not
resolve in this checkout. Use the recorded DOL hashes, run IDs, dates, and
source references to interpret those records. The original history archive is
held outside this repository.
