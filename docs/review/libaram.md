# libaram Review

Date: 2026-10-01

Status: commit `20ec977` passed independent static re-review for the `AR_Reset` then `AR_Init` mismatch. The original two defensive failure-path differences remain documented.

## Scope

- `crates/hardware/ogc-aram`, including its ARQ modules
- C references: `C-sources/libogc/libogc/aram.c`, `arqueue.c`, and `arqmgr.c`; headers `gc/ogc/aram.h`, `arqueue.h`, and `arqmgr.h`

## Reviewed behavior

- ARAM size detection and register programming, init/reset, callback handling, DMA setup, allocation/free stack behavior, clear ranges, and size queries.
- ARQ request setup, queue processing, and manager operations.

The Rust initialization also refuses to replace an IRQ handler already owned by another driver. This defensive behavior is an addition and is not represented as C parity.

Rust ARQ initialization also returns without setting the initialized flag when `lwp::init_queue` fails. C ignores the queue-init result and continues. This is another defensive failure-path difference; successful initialization behavior matches in the reviewed paths.

## Validation boundary

`cargo xtask check-group hardware` passed, and `git diff --check` passed. This is a static source review, not a hardware verification record.

## Independent static re-review

Reviewer: `/root/reviewer_6`; re-review of the recorded ARAM/ARQ scope against current Rust and C. No builds or tests were run; no hardware verification was performed.

### Documented defensive boundaries verified

- `AR_Init` in C unconditionally calls `IRQ_Request(IRQ_DSP_ARAM, __ARHandler, NULL)` and unmasks the IRQ (`../../C-sources/libogc/libogc/aram.c:95-110`). Rust first rejects any existing IRQ handler, then verifies the request had no previous handler (`crates/hardware/ogc-aram/src/lib.rs:470-495`). Rust therefore deliberately refuses to replace an IRQ owner as described above.
- C `ARQ_Init` calls `LWP_InitQueue(&__ARQSyncQueue)` and continues to initialize the callback and flag without checking its result (`../../C-sources/libogc/libogc/arqueue.c:123-148`). Rust returns without setting `initialized` if `lwp::init_queue` fails (`crates/hardware/ogc-aram/src/arq.rs:283-309`). This is the documented defensive failure-path difference.

### Confirmed reinitialization mismatch

C documents that after `AR_Reset`, a later `AR_Init` performs a real initialization (`../../C-sources/libogc/gc/ogc/aram.h:207-223`). `AR_Reset` only clears the init flag (`aram.c:207-210`), and `AR_Init` does not reject an already registered ARAM handler: it calls `IRQ_Request` again (`:103-110`), whose C wrapper replaces the handler and returns the previous one (`../../C-sources/libogc/gc/ogc/irq.h:57-68`).

Rust `reset` likewise only clears `init_flag` (`crates/hardware/ogc-aram/src/lib.rs:596-602`), but `init` returns zero whenever `irq::get_handler(IRQ_DSP_ARAM)` is nonempty (`:475-485`). After a successful Rust `init`, its own `handler` remains installed; therefore `reset(); init(...)` takes this rejection path and cannot reinitialize ARAM. This violates the documented reset/reinitialize contract. The ownership guard needs to distinguish the crate's existing handler from a handler owned by another driver, or the reset path must otherwise allow reinitialization. This confirmed mismatch keeps the crate open.

### Remaining recorded scope

I compared the current ARAM size detection, DSP register programming for read/write DMA, clear ranges, allocation/free stack, callback, ARQ high/low queue service and chunking, reset/flush/remove behavior, and ARQM stack allocation against `aram.c`, `arqueue.c`, `arqmgr.c` and their public headers. I found no additional confirmed mismatch in these checked paths. The register addresses/masks, size-detection probes, 32-byte rounding, request structure layout, and callback calling conventions match in the inspected code. The existing target-side tests were not run.

## Handoff

The original `AR_Reset` followed by `AR_Init` finding is addressed by commit `20ec977` and is awaiting independent re-review by `/root/reviewer_4`. The two failure-path differences above remain documented defensive behavior. No hardware verification is claimed.

## Follow-up: reset/re-init finding is resolved in current source

Current source inspection shows that the reset/re-init mismatch described earlier in this note is no longer present. `init` checks an installed IRQ handler with `is_aram_handler`; it rejects a handler owned by another driver but accepts the ARAM crate's own previously installed handler. It also accepts the previous handler returned by `irq::request` when that handler is the same ARAM handler. Therefore `reset()`, which clears only `init_flag`, can be followed by another `init(...)` without failing solely because the first initialization left the ARAM IRQ handler installed. This matches the C reset/reinitialize lifecycle while preserving the documented defensive policy against taking an IRQ from another owner.

This addendum updates stale review text; it does not claim independent review or hardware validation. Commit `20ec977` is assigned to `/root/reviewer_4` for re-review. The separate defensive behavior differences remain documented.

## Independent re-review of reset/reinitialization fix

**Result: PASSED** for commit `20ec977` (`Allow ARAM reset and reinitialization`). Reviewer: `/root/reviewer_f`. Implementer: `/root/worker_b`.

- C `AR_Reset` clears only `__ARInit_Flag` (`../../C-sources/libogc/libogc/aram.c:207-210`), and the public header says a later `AR_Init` performs a real initialization (`../../C-sources/libogc/gc/ogc/aram.h:215-223`). Rust `reset` similarly clears only `init_flag` (`crates/hardware/ogc-aram/src/lib.rs:601-606`).
- C `AR_Init` returns `0x4000` only when already initialized; after reset it clears the DMA callback, registers `__ARHandler`, unmasks the IRQ, resets stack state, checks ARAM size, sets the init flag, and returns the stack pointer (`../../C-sources/libogc/libogc/aram.c:95-125`). Rust follows that successful path and returns `ARAM_BASE` (`crates/hardware/ogc-aram/src/lib.rs:474-515`).
- C `IRQ_Request` wraps `KIrqSet`, which returns the prior handler (`../../C-sources/libogc/gc/ogc/irq.h:57-68`). Rust's `irq::request` returns the prior handler and `get_handler` reads the registered function (`crates/core/ogc-irq/src/lib.rs:25-35`; `crates/core/ogc-irq-sys/src/lib.rs:733-750`). The new Rust helper compares the candidate function pointer with the ARAM crate's `handler` (`crates/hardware/ogc-aram/src/lib.rs:453-455`). After reset, `init` therefore permits its own still-installed handler both in the preflight lookup and as `irq::request`'s returned previous handler (`:486-499`), then completes initialization. A different nonempty handler fails the preflight without being replaced (`:486-490`). The second check handles an unexpected backend change during registration and restores the previous function before returning failure (`:491-499`).
- The regression test checks the helper's `None`, ARAM handler, and other-handler cases (`crates/hardware/ogc-aram/src/lib.rs:654-661`). It does not execute the hardware-writing `reset`/`init` lifecycle itself, so this approval is based on source tracing plus the helper test's compilation, not a runtime reset test.

Validation: `cargo check -p ogc-aram --target powerpc-unknown-linux-gnu` passed. `cargo test -p ogc-aram --no-run --target powerpc-unknown-linux-gnu` compiled the test crate but failed at link because the configured `cc`/`ld` rejected the cross-target linker options (`--as-needed`, `-Bstatic`, and related flags). No tests ran. No hardware verification was performed.

This result approves only the reset/reinitialization fix. The documented Rust-vs-C defensive failure-path differences above remain as recorded; this re-review does not close unrelated ARAM/ARQ scope.
