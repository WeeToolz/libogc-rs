# `ogc-dvd` API runtime audit

The current [Dolphin run](2026-11-04-dvd-mount-fix/README.md) passed 52
checks with a synthetic Wii disc and 183 checks without a disc. The disc run
covers drive mount, disc ID, synchronous and asynchronous sector reads, and
direct DI reads through the Rust API and Rust-owned C ABI. The no-disc run
covers error handling and callbacks. Both used Dolphin 2609a.

| API functions | Probe | Evidence |
| --- | --- | --- |
| `init`, `get_drive_status`, `mount` | `probe_dvd` | Drive mount passed with the synthetic disc; no-disc mount was rejected. |
| `inquiry`, `read_disk_id`, `read_abs_async_prio`, `read_prio` | `probe_dvd` | Inquiry, disc ID, and sector reads passed, including asynchronous completion and a two-sector boundary. |
| `mount_async`, `inquiry_async`, `read_abs_async_for_bs`, `control_drive_async`, `set_gcm_offset_async`, `set_gcm_offset`, `seek_abs_async_prio`, `seek_prio`, `stop_stream_at_end_async`, `stop_stream_at_end`, `spin_up_drive_async`, `spin_up_drive` | `probe_dvd_no_media_async` | The no-disc log records submissions, callbacks, and final results. |
| `get_cmd_block_status`, `set_auto_invalidation`, `pause`, `cancel_all_async`, `reset`, `get_current_disk_id`, `get_drive_info` | `probe_dvd_no_media_helpers` | The no-disc log records grouped state and helper checks. |

A game disc image is planned for a later partition and filesystem test. A Wii
NAND backup does not contain the game disc partition.

The synthetic disc has no signed Wii partition or filesystem. These runs do
not establish partition opening, filesystem access, or physical drive behavior.
The C ABI checks call Rust-owned exports; they are not an independent C control.

The earlier [synthetic run](2026-11-03-synthetic-dvd-fixture/README.md) and
[corrected baseline](2026-11-04-dvd-c-read-check/README.md) remain as failure
and diagnostic history. The former contains one invalid C-read pass, explained
in its report. The current report records the reset reply mismatch, probe
corrections, and verification.
