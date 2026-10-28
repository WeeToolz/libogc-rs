# Independent `ogc-runtime` ThreadId audit

Reviewer: `/root/reviewer_6`  
Original reviewer: `/root/reviewer_3`; finding in `docs/review/libruntime.md`.  
Result: finding confirmed by direct C and Rust source inspection.

C declares `LWP_THREAD_NULL` as `0xffffffff` and defines `lwp_t` as `u32` (`../../C-sources/libogc/gc/ogc/lwp.h:52,60-63`). The C LWP compatibility implementation treats both zero and `LWP_THREAD_NULL` as invalid thread handles (`../../C-sources/libogc/libogc/lwpcompat/lwpc_thread.c:5-12`). Rust `ThreadId` wraps `NonZeroU32`, and `ThreadId::new` accepts every nonzero value without excluding `u32::MAX` (`crates/core/ogc-runtime/src/scheduler.rs:7-18`). Therefore `ThreadId::new(NonZeroU32::new(u32::MAX).unwrap())` constructs a Rust thread ID whose raw value is the C null sentinel. The earlier note is correct about the accepted value domain. The earlier note also states that this type currently has no in-workspace call sites, so this audit confirms a type invariant mismatch rather than a currently observed bad call into C.

No implementation changes, builds, tests, or hardware verification were performed.
