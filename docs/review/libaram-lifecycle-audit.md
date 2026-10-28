# Independent ARAM Lifecycle and Transfer Audit

Date: 2026-10-01

Reviewer: `/root/reviewer_5`  
Scope: independent cross-check of adjacent ARAM initialization/reset, callback, allocation, DMA, and ARQ request lifecycle behavior against C, beyond the already-recorded `AR_Reset` then `AR_Init` mismatch. This addendum is separate from `docs/review/libaram.md`. No implementation, shared note, or ledger files were changed. No builds, tests, or hardware verification were performed.

## Sources inspected

- Rust: `crates/hardware/ogc-aram/src/lib.rs`, `crates/hardware/ogc-aram/src/arq.rs`, `crates/hardware/ogc-aram/src/arq/manager.rs`.
- C: `../../C-sources/libogc/libogc/aram.c`, `../../C-sources/libogc/libogc/arqueue.c`, `../../C-sources/libogc/libogc/arqmgr.c`, and headers `../../C-sources/libogc/gc/ogc/aram.h`, `arqueue.h`, `arqmgr.h`.
- Existing grouped evidence: `docs/review/libaram.md`.

## Lifecycle re-check

The recorded `AR_Reset` followed by `AR_Init` mismatch remains: both implementations clear only the ARAM initialized flag on reset, but Rust's next `AR_Init` refuses the ARAM IRQ handler that its earlier initialization installed (`aram.c:207-210,95-110`; `crates/hardware/ogc-aram/src/lib.rs:470-495,596-602`).

The adjacent `ARQ_Reset` and `ARQ_Init` path matches the C flag/queue/callback lifecycle in the successful case. Both reset only the initialized flag (`arqueue.c:151-157`; `arq.rs:312-319`); both initialize the pending pointers, callback slots, default chunk size, queues, and ARAM completion callback (`arqueue.c:123-149`; `arq.rs:283-309`). Rust's queue-initialization error return is the already documented defensive difference.

`AR_Init` field initialization, ARAM base `0x4000`, stack pointer/free-block setup, size-check sequence, and DSP refresh-register write match by inspection (`aram.c:95-129`; `lib.rs:470-512`). Rust additionally guards state access and refuses an occupied IRQ slot; those documented safety policies are not C parity. `AR_RegisterCallback` returns the previous callback and replaces it in both implementations (`aram.c:74-84`; Rust `lib.rs:453-459`). The ARAM IRQ handler clears the interrupt before invoking the registered callback in both (`aram.c:227-235` and handler below; Rust `lib.rs:440-450`).

## DMA, allocation, and clear re-check

`AR_StartDMA` register field updates for main-memory address, ARAM address, direction, and length match the Rust register programming sequence (`aram.c:131-150`; `lib.rs:244-279,514-528`). C `AR_Alloc`/`AR_Free` update the stack pointer and block counts without bounds checks; Rust applies the same wrapping arithmetic and block-length stack operations (`aram.c:153-179`; `lib.rs:530-563`). C and Rust `AR_Clear` select the same three address ranges and issue equivalent zero-fill DMA chunks (`aram.c:182-200`; `lib.rs:299-324,565-585`). No additional public transfer or allocation mismatch was confirmed in these paths.

## ARQ ordering and callback re-check

Both implementations queue high- and low-priority requests FIFO and select high-priority work before low-priority work when the engine becomes idle (`arqueue.c:54-68,79-104,185-218`; `arq.rs:88-115,187-237,339-374`). Both split low-priority work into the configured chunk size, update request length and addresses after each non-final DMA interrupt, and invoke the request callback after marking the completed request finished (`arqueue.c:79-104,106-121`; `arq.rs:187-219,239-280`). Their callback-to-next-request order is equivalent: high-priority completion is dispatched first, then another high request is started, and low-priority service runs only when no high request is pending.

Queue flush and request removal behavior also match in the inspected cases: both clear queued high/low requests while retaining an active low request whose callback is already registered, and both clear an active low pending request only when no completion callback is set (`arqueue.c:172-183,233-241`; `arq.rs:410-439`). Both remove-owner functions scan queued requests only and return the number removed (`arqueue.c:243-270`; `arq.rs:441-459`).

The manager paths checked preserve the same 16-entry stack, 256-byte zero buffer, 32-byte rounding, alignment and positive-length checks, capacity check, high-priority ARQ push, callback polling, and LIFO pop/free-space behavior (`arqmgr.c:76-128,130-154`; `arq/manager.rs:11-12,71-159,161-179`). Rust's atomic poll flag and explicit unsafe lifetime requirements are Rust implementation choices; no separate output or ordering mismatch was confirmed in the compared paths.

## Result

This audit found no additional confirmed parity gap in the adjacent successful init/reset, callback, allocation, DMA, clear, ARQ queue, or ARQM paths. The known `AR_Reset`/`AR_Init` failure remains open, and the existing documented defensive IRQ ownership and ARQ queue-init error handling differences remain. This is source inspection only; it does not validate register timing, cache behavior, or ARAM hardware operation.

## Follow-up against current source

The reset/re-init finding above is stale for the current implementation. `ogc-aram::init` now distinguishes its own installed handler from an unrelated IRQ owner through `is_aram_handler`, and accepts its own handler both before and after `irq::request`. A prior successful `init`, followed by `reset`, can therefore enter the real initialization path again as C documents. No new ARAM implementation change was required in this pass; this is a documentation correction pending independent re-review.
