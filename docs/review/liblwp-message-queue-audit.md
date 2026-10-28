# Independent LWP Message Queue Audit

Date: 2026-10-01

Reviewer: `/root/reviewer_5`  
Scope: message-queue initialization, send/receive/jam flags, exception behavior, blocked callers, and close behavior compared with the C LWP compatibility layer and Tuxedo mailbox. This is separate from `docs/review/liblwp.md`. No implementation, shared note, or ledger files were changed. No builds, tests, or hardware verification were performed.

## Sources inspected

- Rust: `crates/core/ogc-lwp-driver/src/runtime.rs`, `crates/core/ogc-lwp-sys/src/lib.rs`, `crates/core/ogc-lwp/src/lib.rs`.
- C: `../../C-sources/libogc/libogc/lwpcompat/lwpc_mq.c`, `../../C-sources/libogc/tuxedo/mailbox.c`, `../../C-sources/libogc/gc/tuxedo/mailbox.h`, `../../C-sources/libogc/gc/ogc/message.h`.

## Confirmed differences

### Queue capacity

C `MQ_Init` accepts any nonzero `u32` count if allocation succeeds, allocates that many pointer slots, and passes the count to `KMailboxPrepare` (`lwpc_mq.c:16-31`). `KMailbox.num_slots` is `u16` (`mailbox.h:8-15`), so capacities above 65535 are truncated by C. Rust instead uses fixed storage for 16 messages and rejects capacities above `MAX_MESSAGES == 16` (`runtime.rs:42-43,249-269,1686-1705`). Capacities 17 through 65535 can initialize in C but fail in Rust. The limit is confirmed; C counts above `u16::MAX` have truncation behavior.

### `MQ_Send` blocking behavior and exception handling

The C compatibility implementation contains `if (true) { flags |= MQ_MSG_NOBLOCK; }` in `MQ_Send` (`lwpc_mq.c:42-58`). As written, it always forces no-block behavior, not only in exception context. It then calls `KMailboxTrySend`, returning false when the mailbox is full (`tuxedo/mailbox.c:8-29`). Rust `message_send` blocks when the queue is full unless flags equal `MQ_MSG_NOBLOCK` (`runtime.rs:1731-1773,1757-1766`). Therefore `MQ_MSG_BLOCK` full-queue sends block in Rust but return false in this C implementation. A Rust send from exception context can also block; C's forced no-block path cannot.

### No-block flag combinations

C receive checks `flags & MQ_MSG_NOBLOCK` and therefore treats any flag word containing the bit as no-block (`lwpc_mq.c:64-88`). Rust checks exact equality `flags == MQ_MSG_NOBLOCK` for both send and receive (`runtime.rs:1757-1759,1807-1809`). Thus combined/unknown bits, such as `MQ_MSG_NOBLOCK | 2`, are no-block in C receive but block in Rust if the mailbox is empty. C send is already always no-block due to the unconditional `true`; Rust combined flags make a full-queue send block. The flag interpretation differs.

### Null receive output

C `MQ_Receive` calls the mailbox receive operation regardless of whether `msg` is null. It writes the received slot only when `msg != NULL` (`lwpc_mq.c:75-88`), so a null output pointer discards a queued message and returns success. Rust rejects a null output pointer before it checks queue state and leaves the message queued (`runtime.rs:1782-1805`). This is an observable API behavior difference.

### Jam operation

The public C header declares `MQ_Jam`, but the compatibility implementation marks it unimplemented and provides no function body (`message.h:99-107`; `lwpc_mq.c:61-62`). Rust implements `message_jam` by inserting at the front and can block when full unless flags equal `MQ_MSG_NOBLOCK` (`runtime.rs:1731-1776`). Rust therefore adds behavior for an API declaration that C does not implement in this source tree; it should be treated as a Rust addition rather than parity.

### Closing with blocked senders or receivers

C `MQ_Close` converts the handle to a `KMailbox*` and frees it without waking or unlinking blocked senders/receivers (`lwpc_mq.c:34-40`). Tuxedo stores blocked receivers in a global queue keyed by the mailbox address, and blocking receive sleeps there until a send wakes it (`tuxedo/mailbox.c:51-68`). C send does not block as noted above, so the relevant supported blocked-close case is a receiver. Closing such a mailbox leaves the waiter referring to freed storage; the exact subsequent result is unsafe rather than a defined return code. Rust close wakes both sender and receiver waiter sets with `Cancelled` before clearing queue state (`runtime.rs:1708-1729`); blocked operations return false (`:1761-1766,1811-1815`). This is a material safety and behavior difference, with Rust defining cancellation where C frees the mailbox without waiter cleanup.

### Receive from exception context

C `MQ_Receive` detects `PPCIsInExcpt()` and sets `MQ_MSG_NOBLOCK` before choosing `KMailboxTryRecv` versus blocking `KMailboxRecv` (`lwpc_mq.c:64-82`). Rust has no exception-context check in `message_receive`; with an empty queue and a flag value other than exactly `MQ_MSG_NOBLOCK`, it blocks through the scheduler (`runtime.rs:1782-1818`). This can suspend in exception context where C returns false immediately. This differs independently of the combined-flag issue.

## Matching behavior checked

For a valid non-full queue, both send operations append at the tail and wake one receiver; receive removes from the head and wakes one blocked sender (`tuxedo/mailbox.c:8-48`; `runtime.rs:1739-1755,1798-1805`). Queue contents are pointer-sized message values in both implementations. No message ordering mismatch was found for ordinary send/receive operations.

## Result

The maximum supported capacity, full-queue send behavior, flag-bit handling, null-output receive behavior, exception behavior, and close-with-blocked-receiver behavior differ. C `MQ_Jam` is declared but unimplemented, while Rust provides an implementation. Ordinary FIFO send/receive ordering matches for valid operations. This is source-only; no runtime or hardware claims are made.
