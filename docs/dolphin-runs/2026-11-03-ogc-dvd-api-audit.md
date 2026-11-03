# `ogc-dvd` API runtime audit

This audit covers the 27 Rust functions listed for `ogc-dvd` in the generated
implementation inventory. The current evidence is the
[fresh DVD probe run](ogc-dvd-api-audit-current/README.md). Dolphin 2609a
reported 178 passes and no failures. The probe ran with no disc. These results prove
the stated no-media behavior only. They do not prove successful reads,
callbacks for completed reads, or drive behavior with a disc.

| API functions | Probe call | Evidence in the saved log |
| --- | --- | --- |
| `init`, `get_drive_status`, `mount` | `probe_dvd` | `dvd init and drive status report no disc`; `dvd no fake success without media`. |
| `inquiry`, `read_disk_id`, `read_abs_async_prio`, `read_prio` | `probe_dvd` | `dvd no fake success without media`; the log does not retain a separate pass line for each call. |
| `mount_async`, `inquiry_async`, `read_abs_async_for_bs`, `control_drive_async`, `set_gcm_offset_async`, `set_gcm_offset`, `seek_abs_async_prio`, `seek_prio`, `stop_stream_at_end_async`, `stop_stream_at_end`, `spin_up_drive_async`, `spin_up_drive` | `probe_dvd_no_media_async` | The current log records submission, callback, callback-after-return, and final result for each async call. It records separate result checks for the synchronous calls. |
| `get_cmd_block_status`, `set_auto_invalidation`, `pause`, `cancel_all_async`, `reset`, `get_current_disk_id`, `get_drive_info` | `probe_dvd_no_media_helpers` | The inventory and source map these calls to the helper probe. The saved log records idle command-block status, invalidation update and restore, pause state, cancel with no pending requests, reset state, and stable information pointers. These are grouped checks, not one assertion per API. |

The fresh log now verifies the no-disc submission, callback, callback timing,
and result for `read_abs_async_for_bs`. It still does not record a successful
read or successful-media callback. The 27 inventory rows contain direct and
forwarded APIs; some checks remain grouped, as the table states. Successful
DVD reads and partition paths still need a suitable disc image.
