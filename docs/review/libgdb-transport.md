# GDB Transport Review

Date: 2026-11-27

Status: needs re-review. The Rust-owned transport and diagnostic contracts
have been inspected; independent lifecycle and live socket validation remain
open. This is not C ABI parity or full GDB remote serial protocol support.

## Scope and source

- `crates/compatibility/gdb-transport/src/lib.rs` (transport, decoder, control).
- `crates/compatibility/gdb-transport/src/diagnostics.rs` (listener diagnostics).
- `crates/compatibility/libdb-sys/src/wii_network.rs` (production Wii adapter).

No corresponding C implementation of this additional Rust control plane was
identified in the pinned libogc reference. The older libdb debugger is a
separate compatibility implementation. Keep the cooperative-only scope;
standard memory and execution commands do not perform those operations.

## Source contracts

| Area | Observed behavior |
| --- | --- |
| Decoder | Fixed 4096-byte payload capacity; incremental framing, hexadecimal checksum, oversized discard and reset. Only the limited command subset is supported. |
| Poll and reply queue | One bounded read and at most two writes; reserves reply space before beginning a frame. Preserves unwritten bytes and buffered receive bytes. Rejects invalid backend byte counts. Partial frames expire after 30000 ms. |
| Command dispatch | ACK valid frames, NACK malformed frames; unknown commands receive an empty response. Status is read-only. Cooperative requests enter the bounded control queue; acknowledgement of a queued request does not mean the main thread acted on it. |
| Cooperative state | Single producer and single consumer; explicit arm, pause eligibility, local resume, reset and worker-failure behavior. Requests carry the connection generation. |
| Socket lifecycle | Explicit close, retry after failed close, separate client and listener cleanup results. The Wii adapter makes listeners and accepted sockets nonblocking, maps IOS retry/interrupt/EOF results, rejects extra clients and retains a failed extra-client close for cleanup. |
| Diagnostic wire | Fixed HBCD version-one query and 32-byte response. Preserves original operation and raw IOS error when available, and distinguishes cleanup failure from the initial failure. |

## Confirmed corrections

The previous no-authorization value was also the largest valid packed
connection generation (`0x3fff_ffff`). At that generation a disarmed
connection could report authorization. Reserve `u32::MAX` outside the packed
range; do not mask the stored authorization sentinel on reads. The public
authorization check also requires connection and transport availability.

When a paused client disconnected and a replacement client arrived before
the next main-thread service, the generation mismatch reset the state but
did not report `Resumed`. Return that notification first and preserve queued
requests from the new connection for a subsequent safe point. Stale generation
requests cannot act on the new connection. This retains the original bounded
queue and does not add general execution control.

## Verification

- Both new deterministic regressions fail on the original implementation
  (37 passed, 2 failed), then pass after the production corrections.
- The final host suite passes 39/39, including decoder, queue, partial writes,
  timeout, disconnect, retryable cleanup and diagnostic wire tests.
- The PowerPC check includes `libdb-sys/wii-network`, not only the generic
  transport. It verifies compilation, not live socket behavior.
- See [baseline checks](evidence/gdb-transport-20261127/before-checks.txt),
  [host checks](evidence/gdb-transport-20261127/host-tests.txt), and
  [PowerPC check](evidence/gdb-transport-20261127/powerpc-check.txt).

## Remaining review

- Independent review of concurrent worker/main-thread transitions, connection
  generation snapshots, worker restart and queue ownership. The tests exercise
  deterministic sequences, not every atomic interleaving or generation wrap.
- Verify adapter startup/accept/close failure ownership and retry behavior,
  including repeated failures of the pending extra-client cleanup slot.
- Execute the production Wii socket adapter with a real Dolphin peer and retain
  target wire/cleanup evidence. The previously reported controlled-backend run
  was not recovered and is not counted as evidence for this revision.
- Inspect application-level consumer scheduling outside this checkout. Physical
  Wii evidence remains pending and is not a source-review completion gate.
