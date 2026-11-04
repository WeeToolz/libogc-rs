# Wii SD API by API Dolphin audit

## Scope and evidence

The generated inventory has five `ogc-wiisd` APIs. This audit maps each API
to a named probe check and its saved Dolphin evidence. The probe uses Dolphin's
virtual Wii SD card. It does not use the GameCube SD Gecko interface.

The latest saved virtual-card run passed 15 checks with no failures. Its DOL
SHA-256 is
`10346fd10cbb909eb91166eb905fe4f0adf0c15b48d57b4f45e09e82e5b40ea9`. The
[run report](2026-11-03-wiisd-multisector/README.md) records one-sector
read/write/restore and an unaligned nine-sector read/write/restore across the
eight-sector transfer limit. The
[probe log](2026-11-03-wiisd-multisector/api-path-audit-rerun/probe.log)
contains each named result.

A separate run disabled virtual SD. It passed 10 checks with no failures. The
[no-card report](2026-11-03-wiisd-no-card/README.md) records read and write
rejection, output-buffer preservation, shutdown, and restart without media.
That report is a summary; it does not include a per-check raw probe log.

## API evidence

| API | Dolphin check | Evidence and limit |
| --- | --- | --- |
| `startup` | `Wii SD startup and inserted state agree`; `Wii SD restart restores and then releases interface state` | Virtual-card run reports startup success, then successful restart and shutdown. The no-card run reports startup unavailable. |
| `is_inserted` | `Wii SD startup and inserted state agree`; `Wii SD shutdown releases the interface and is one-shot`; `Wii SD restart restores and then releases interface state` | Presence agrees with startup. It reports no card after shutdown and the saved state after restart. |
| `read_sectors` | `Wii SD read result matches card presence and updates its output`; `Wii SD nine-sector read returns the written bytes`; no-card read check | Reads sector zero and nine sectors from the private virtual card. The no-card run checks that the output remains unchanged. |
| `write_sectors` | One-sector and nine-sector write, readback, and restore checks; zero-sector check; no-card write check | The virtual-card run checks the deterministic bytes and restores the original bytes. The nine-sector request crosses the production eight-sector transfer buffer. The no-card run checks rejection. |
| `shutdown` | `Wii SD shutdown releases the interface and is one-shot`; `Wii SD restart restores and then releases interface state` | Checks the closed state, repeated shutdown rejection, restart, and final shutdown. |

All five rows also have source mappings in
[`implementation-evidence.csv`](../../config/implementation-evidence.csv)
and the [generated inventory](../generated/implementation-inventory.csv).

## Limits

These runs verify the production `ogc-wiisd` path against Dolphin's virtual
Wii SD card. They do not verify physical Wii SD hardware. They do not add
evidence for `ogc-gcsd`, which uses the GameCube EXI SD Gecko path. Dolphin
does not emulate an SD Gecko adapter, so its successful media operations remain
open.
