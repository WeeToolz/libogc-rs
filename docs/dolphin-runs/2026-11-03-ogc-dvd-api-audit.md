# `ogc-dvd` API runtime audit

**No-disc run:** The [DVD log](2026-11-03-storage-unrestricted/dvd_probe/probe.log) passed 183 no-disc checks. It had no disc and did not exercise media reads.

**Synthetic media run:** The later [fixture log](2026-11-03-synthetic-dvd-fixture/probe.log) passed the offset and two-sector sync/async read checks. Its full probe result was 35 passes and four failures: mount and three direct DI media checks. The image is not a valid signed Wii partition.

This audit covers the 27 Rust functions listed for `ogc-dvd` in the generated
implementation inventory. The no-disc evidence is the
[fresh DVD probe run](ogc-dvd-api-audit-current/README.md). Dolphin 2609a
reported 178 passes and no failures. The probe ran with no disc. These results prove
the stated no-media behavior only. They do not prove successful reads,
callbacks for completed reads, or drive behavior with a disc.

| API functions | Probe call | Evidence in the saved log |
| --- | --- | --- |
| `init`, `get_drive_status` | `probe_dvd` | The no-disc run passed the status check. The Wii-format media run reported a disc. |
| `mount` | `probe_dvd` | Mount rejected the no-disc case. It returned `-1` with the synthetic Wii-format image. |
| `inquiry`, `read_disk_id`, `read_abs_async_prio`, `read_prio` | `probe_dvd` | The saved synthetic-media run passed `inquiry`, `read_disk_id`, sector-zero async read, and sync reads at sector zero and offset `0x2000`. The current source also checks a full-sector async read and matching sync/async two-sector reads at `0x2000`, including the payload-to-zero boundary; these checks are pending a new Dolphin run. See the [run report](dvd-wii-header-synthetic/README.md). |
| `mount_async`, `inquiry_async`, `read_abs_async_for_bs`, `control_drive_async`, `set_gcm_offset_async`, `set_gcm_offset`, `seek_abs_async_prio`, `seek_prio`, `stop_stream_at_end_async`, `stop_stream_at_end`, `spin_up_drive_async`, `spin_up_drive` | `probe_dvd_no_media_async` | The current log records submission, callback, callback-after-return, and final result for each async call. It records separate result checks for the synchronous calls. |
| `get_cmd_block_status`, `set_auto_invalidation`, `pause`, `cancel_all_async`, `reset`, `get_current_disk_id`, `get_drive_info` | `probe_dvd_no_media_helpers` | The inventory and source map these calls to the helper probe. The saved log records idle command-block status, invalidation update and restore, pause state, cancel with no pending requests, reset state, and stable information pointers. These are grouped checks, not one assertion per API. |

The fresh log now verifies the no-disc submission, callback, callback timing,
and result for `read_abs_async_for_bs`. It still does not record a successful
read or successful-media callback. The 27 inventory rows contain direct and
forwarded APIs; some checks remain grouped, as the table states. Successful
DVD reads and partition paths still need a suitable disc image.

The later [synthetic Wii-format run](dvd-wii-header-synthetic/README.md)
changes the media results for `inquiry`, `read_disk_id`, `read_abs_async_prio`,
and `read_prio`: those checks passed against a sparse fixture with a Wii disc
header and known sector bytes. `mount` returned `-1`. The image has no valid
partition, and direct DI still reports no disc, so no successful partition or
direct DI read is established. This is partial read evidence, not a valid game
disc result.
