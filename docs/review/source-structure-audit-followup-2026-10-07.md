# Source-structure audit follow-up — 2026-10-07

The source and stub audits initially reported 15 untracked source patterns. This note records their disposition.

## Fixed findings

- `libdb-sys::private_net::send_tcp_frame` no longer suppresses Clippy's argument-count lint. Its frame inputs are grouped in a `TcpSegment` value.
- Five `non_upper_case_globals` suppressions in `ogc-gx-sys` are removed. The exact C spellings remain available through `pub use` aliases to the canonical Rust constants, including `wgPipe`.
- Six `non_snake_case` suppressions in `ogc-system-sys` are removed. Rust implementation identifiers now use snake case and `export_name` preserves each `SYS_*` C symbol. Public Rust aliases keep the prior C-header spellings available to Rust callers.

## Tracked finding

The three `EVENT_QUEUE` matches in `wiikeyboard-sys` are one production event FIFO: the USB callback pushes events, and `get_event` and stdin consume them. They do not indicate a simulated keyboard backend. The marker is listed in `config/audit-allowlist.csv`, with its reason and review owner also shown in `docs/generated/runtime-audit-debt.md`.

## Validation

`cargo xtask audit-source` passes with the keyboard FIFO recorded as tracked audit debt. `cargo xtask check-inventory` passes after updating evidence line references affected by formatting and adding exact probe evidence for `tcpip_socket`.

`cargo xtask audit-stubs` passes its source-structure and inventory checks, then reports 2,565 public API entries without exact test or probe evidence. These are findings from the separate runtime completeness audit; this bounded source-pattern follow-up does not classify or suppress them.
