# LWP PowerPC Context Sub-audit

Date: 2026-10-01

Reviewer: `/root/reviewer_5`  
Scope: `ogc-lwp-driver` thread context creation and PowerPC save/restore assembly against Tuxedo's `KThreadPrepare` and context/exception assembly. This is separate from the shared LWP review note. No implementation code, shared note, or ledger row was changed. No builds, tests, or hardware verification were performed.

## Files inspected

- Rust: `crates/core/ogc-lwp-driver/src/arch/powerpc.rs`, `crates/core/ogc-lwp-driver/src/runtime.rs`.
- C: `../../C-sources/libogc/tuxedo/thread.c`, `../../C-sources/libogc/tuxedo/interrupt_asm.S`, `../../C-sources/libogc/tuxedo/exception_asm.S`, `../../C-sources/libogc/gc/tuxedo/ppc/context.h`.

## Context layout and general registers

The Rust `Context` field order and offsets match C `PPCContext` (`arch/powerpc.rs:15-29,49-54`; `gc/tuxedo/ppc/context.h:5-16,23-36`). Its assembly saves and restores r0-r31, CR, LR, CTR, XER, and PC/MSR (`arch/powerpc.rs:62-85,138-160,201-224`). In particular, the initial store of r0 and the later save/restore of r12 account for the temporary register use in the switch routine. The corresponding C exception/switch path saves/restores these GPR and control-register values (`tuxedo/interrupt_asm.S:9-39,60-111,117-138`). No missing GPR or control-register save was established in the Rust path inspected.

## Confirmed paired-single state gap

C's lazy FPU exception handler saves all 32 scalar FPR values and each paired-single shadow value into distinct `PPCContext.fpr[]` and `PPCContext.ps[]` arrays, and restores both (`tuxedo/exception_asm.S:124-130,138-149`). Rust's switch saves/restores scalar FPR values and FPSCR at the corresponding offsets (`arch/powerpc.rs:87-120,161-194`) but has no `psq_st`/`psq_l` operations and does not read or write its `Context.ps` array. The Rust struct comment explicitly calls the paired-single area reserved (`arch/powerpc.rs:12-14`).

Therefore a thread that keeps paired-single values live across a scheduling switch can observe those values changed by another thread's paired-single use. This is a concrete register-state parity gap against the C FPU context handler. The Rust implementation's eager scalar-FPU save/restore does not cover the separate paired-single shadow state.

## FP enable and lazy ownership

C initializes a prepared thread's MSR to `MSR_EE|MSR_ME|MSR_IR|MSR_DR|MSR_RI`, without `MSR_FP`, and Tuxedo tracks the FPU owner through SPRG3 (`tuxedo/thread.c:43-51,54-65`). `exception_asm.S:48-163` enables FP lazily and saves/restores the previous FPU owner's context on first use. Rust instead initializes a new context MSR with `current_msr() | 0x0000_b032` (`runtime.rs:418-430`), whose mask includes FP, and its switch temporarily enables FP to save/restore scalar registers (`arch/powerpc.rs:83-89,138-163`). No Rust update of an FPU-owner pointer or SPRG3 was found in the inspected driver sources.

This is a different eager-versus-lazy FP policy. It is not by itself recorded as an additional corruption finding: the Rust switch directly captures scalar FPR/FPSCR on each context switch. Its correctness when combined with the platform's existing exception FPU-owner machinery was not established statically. The paired-single omission above remains regardless of the eager scalar save.

## Initial context and stack setup

C `KThreadPrepare` sets PC directly to the entrypoint, LR to `KThreadExit`, SP to the eight-byte-aligned top minus eight bytes, writes `UINT32_MAX` as the initial stack back-chain sentinel, places `arg` in r3, initializes r2/r13 to SDA bases, and starts the thread suspended (`tuxedo/thread.c:54-80`). Rust instead aligns SP to 32 bytes, subtracts 32, writes zero to the first two words, starts at `thread_bootstrap`, leaves LR and r3 zero, copies current r2/r13, and stores entry/argument in its scheduler's `Thread` record (`runtime.rs:418-430,847-857`). The trampoline invokes the saved entry and explicitly calls `thread_exit`, so the changed PC/LR/r3 setup is intentional for the Rust runtime design. However, the initial stack back-chain sentinel differs (`0` in Rust versus `UINT32_MAX` in C); code that inspects or unwinds a fresh thread's first stack frame will see a different marker. This is a smaller observable setup mismatch, though normal function entry does not use the marker as an argument.

The Rust context also copies the current thread's r2/r13 instead of assigning linker SDA bases as C does. This may be correct for Rust's static runtime and its generated code, but no ABI link configuration was inspected here to prove parity for arbitrary C callbacks run on Rust-created LWP threads. Record this as an ABI compatibility question, not a confirmed failure.

## Result

One confirmed context mismatch remains: paired-single register state is not preserved across Rust scheduler switches. Initial stack sentinel behavior also differs, with potential effects for stack unwinding/debug inspection. General-purpose/control-register save and restore coverage matched in the reviewed paths. No tests or builds were run; this sub-audit is source-only and does not claim hardware verification.

## 2026-10-02 paired-single and initial-stack follow-up

Compared with current `gc/tuxedo/ppc/context.h`, `tuxedo/exception_asm.S`, and `tuxedo/thread.c`.

The PowerPC context switch now saves/restores all 32 paired-single register images in the existing `Context::ps` area at C offset `0x1c0`, in addition to the FPR/FPSCR/GQR state already preserved. The public `Context` layout remains `0x2c0` bytes and the adjacent PC/MSR/CR/LR/CTR/XER/GPR/FPR/GQR offsets are unchanged.

Fresh-thread setup now aligns the stack to 8 bytes, reserves C's 8-byte initial frame, and writes `UINT32_MAX` into the back-chain word. The previous Rust frame used a 32-byte decrement and zero back-chain. Runtime target validation is still required.
