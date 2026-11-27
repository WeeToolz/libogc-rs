# Report Protocol Review

Date: 2026-11-27

Status: needs re-review. The shared protocol source and its development runtime
consumer boundary have been reviewed. Live transport and independent review
remain open.

## Scope and reference

- `crates/core/ogc-report-protocol/src/lib.rs` (all production definitions).
- `crates/core/ogc-system-sys/src/dev.rs` (argument handling, log capture,
  snapshot and report writer call sites).
- `probes/ogc-system-probe/src/probe_support.rs` (separate WIP1/WIR2 writers,
  inspected as a second in-tree wire format reference).

No direct C counterpart was identified in the pinned libogc reference. This
is an additional Rust contract, not a C ABI parity claim. The independent
host receiver is not in this checkout. HBC startup diagnostics have their own
argument and address policies.

## Source coverage

| Definition | Contract and review |
| --- | --- |
| `Identity`, `parse` | Requires endpoint, run ID, 64 ASCII hex build bytes and positive recovery deadline; rejects duplicate known options. Run ID is 1–64 ASCII alphanumeric, hyphen, underscore or period bytes, with zero-filled remainder. Unknown options are ignored. Runtime publishes a parsed identity before worker creation. |
| `parse_report_endpoint` | Four IPv4 octets and nonzero `u16` port. Address policy is separate: `parse` rejects first octet zero or at least 224. The parser does not claim full network reachability or canonical address validation. |
| `parse_recovery_deadline` | Inclusive 2,000 to 2,147,483,647 millisecond interval. |
| `ProgressHeader::encode`, `progress_header` | 24 bytes, WIP1 version 1, big-endian sequence, stage, two identity lengths and text length. Bit 0 is truncation; development helper sets stage and flags to zero. The separate probe path supports identity-unavailable bit 1. The helper is used only with a parsed identity and a bounded log delta or heartbeat. |
| `final_header` | 156 bytes, WIR2 version 2, bit 0 for truncation, 32-bit text length, zero framebuffer length and dimensions, 36-byte performance section, run/build fields at offsets 28 and 92. Matches the separate probe layout for the supported no-framebuffer subset. |
| `write_final_report` | Writes exactly header, log, loss counters, status summary and zero performance bytes; propagates first callback error without further writes. Application status, log loss or contention produce a failure summary. Contention alone does not set truncation bit 0. |
| `append`, `append_decimal` | Private fixed-size formatting path. At most two 20-digit counters plus fixed text and newline fit in the 128-byte tail on host and PowerPC; the 20-byte digits array covers all supported `usize` values. |
| `LogBuffer`, `new`, `Default`, `fmt::Write::write_str` | Allocation-free byte buffer, capacity-bound copies, saturating lost-byte count; truncation may split UTF-8. Runtime locks before capture or snapshot and uses the resulting bounded length. |

Direct construction of public `Identity` and `LogBuffer` fields must preserve
parser and length invariants. Raw header helpers require lengths that fit
in the wire fields. These preconditions are now documented rather than
changing signatures or wire behavior. The inspected development call sites
use a validated identity and a 16 KiB bounded log. The generic report writer
requires its callback to write the full supplied slice or return an error;
`dev.rs` uses its production `write_all` loop. This inspection does not prove
socket delivery or worker scheduling and does not certify the whole runtime.

## Verification

- Host: 14/14 tests pass (10 prior tests plus four expanded boundary tests).
  New cases cover 64-byte run IDs, invalid ASCII/Unicode fields, missing
  fields, maximum deadline and port, all-ones wire lengths, nonoverlapping
  identity fields, maximum loss counters, full log length, contention-only
  flags, zero-capacity logging, UTF-8 byte truncation and saturated loss.
- PowerPC: production protocol crate compiles with the Wii target.
- PowerPC: `ogc-system-sys` compiles with `ogc_dev` selected, including the
  production report protocol consumer.
- See [host tests](evidence/report-protocol-20261127/host-tests.txt),
  [target check](evidence/report-protocol-20261127/powerpc-check.txt), and
  [development consumer check](evidence/report-protocol-20261127/dev-consumer-check.txt).

## Open work

- Independently review these contracts against the host receiver and inspect
  application-specific callers outside this checkout.
- Execute live development worker report delivery, partial writes, timeout,
  recovery and failure paths. The target checks are compilation evidence,
  not a Dolphin or physical network run. Physical Wii verification is pending.
- Preserve source and test scope separately from transport acceptance. No
  production behavior change was necessary in this review.
