# Tracked runtime audit debt

This file is generated from `config/audit-allowlist.csv`. Entries are temporary reviewed debt and are not completion claims.

## Independent USB event-path audit

| Path | Marker | Owner | Reason |
| --- | --- | --- | --- |
| `crates/compatibility/wiikeyboard-sys/src/lib.rs` | `EVENT_QUEUE` | `thread-current/root` | This FIFO carries events delivered by the production USB callback to get_event and stdin; it is not a simulated keyboard backend. |

