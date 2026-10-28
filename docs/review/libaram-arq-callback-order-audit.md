# ARQ interrupt callback ordering audit

Reviewer: reviewer-6. Implementer: N/A (independent static audit).

## Scope and result

This audit checked ARQ completion callback ordering and queue continuation against C, including priority handling and request enqueue behavior during callbacks. It excluded the already recorded ARQ reset/reinitialization issue. No additional mismatch was found in these paths.

## Evidence

- The C ARAM IRQ handler clears the ARAM interrupt before invoking the registered DMA callback (`C-sources/libogc/libogc/aram.c:395-404`). Rust's ARAM handler also clears the interrupt and then invokes the registered callback (`crates/hardware/ogc-aram/src/lib.rs:440-450`).
- C's ARQ service marks the completed request finished, calls its callback, then clears the corresponding pending request and callback state (`C-sources/libogc/libogc/arqueue.c:106-121`). Rust follows the same order: marks finished, releases its critical section to call the callback, then clears pending/callback state (`crates/hardware/ogc-aram/src/arq.rs:239-280`). The IRQ handler restores the prior interrupt state, so the critical-section wrapper preserves ISR interrupt state while matching C's callback-before-clear ordering.
- After a callback, both implementations start the next high-priority request first, then service low priority only when no high-priority DMA is pending (`arqueue.c:119-120`; `arq.rs:275-278`). High requests are started as full transfers; low requests are split by the configured chunk size and continue on subsequent completions (`arqueue.c:79-103`; `arq.rs:187-219`).
- Both enqueue a request to low priority only when priority equals `ARQ_PRIO_LO`, otherwise to high priority, and start work immediately only if neither priority has an active pending request (`arqueue.c:185-217`; `arq.rs:339-373`). Because completion callbacks run before pending-state cleanup in both versions, requests posted by those callbacks remain queued until the handler clears the completed request and selects the next task.

## Validation

Static source comparison only. No builds or tests were run. No hardware verification was performed. The shared `libaram.md` note and ledger were not changed.
