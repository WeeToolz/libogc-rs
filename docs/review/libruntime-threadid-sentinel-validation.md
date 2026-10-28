# ThreadId null-sentinel validation

Reviewer: `/root/reviewer_6`  
Scope: independently verify whether `ogc-runtime::ThreadId` can represent C's null thread handle, and distinguish manually constructed values from scheduler-issued handles. The shared runtime note and ledger were not changed.

## Result

The public `ThreadId` type can represent `LWP_THREAD_NULL`, but the current Rust LWP driver does not issue or return that value for a live thread.

C defines `LWP_THREAD_NULL` as `0xffffffff` and `lwp_t` as `u32` (`../../C-sources/libogc/gc/ogc/lwp.h:52,60-63`). Its thread adapter rejects both zero and this sentinel before converting a handle into a `KThread*` (`../../C-sources/libogc/libogc/lwpcompat/lwpc_thread.c:5-12`). On successful C creation, it stores the bitwise complement of the allocated `KThread*`; on allocation failure it stores the sentinel (`lwpc_thread.c:29-44`). A valid allocated thread pointer is non-null, so its complemented handle is not `0xffffffff`.

Rust `ThreadId::new` accepts any `NonZeroU32` without excluding `u32::MAX`, and `get` returns that raw value (`crates/core/ogc-runtime/src/scheduler.rs:7-18`). Therefore `ThreadId::new(NonZeroU32::new(u32::MAX).unwrap())` is a valid Rust `ThreadId` value even though its raw value is the C invalid-handle sentinel. The constructor makes this a public type-domain/invariant mismatch.

The current Rust LWP driver does not produce that value: it encodes scheduler slot `index` as `index + 1` (`crates/core/ogc-lwp-driver/src/runtime.rs:359-369`), returns `handle(CURRENT)` from `current_thread` (`:1035-1038`), and writes the sentinel only on thread-creation errors (`:946-965,1009-1013`). Those returned scheduler IDs are within the fixed table's small positive range. The `ogc-runtime` crate itself has no construction call sites beyond the public constructor/type export (`crates/core/ogc-runtime/src/lib.rs:24`; repository search found no other `ThreadId` use).

## Conclusion and limits

The earlier finding is confirmed as an invalid public `ThreadId` value accepted by the constructor; it is not evidence that the current driver returns the sentinel as a live thread handle. This was source-only review. No implementation changes, builds, tests, or hardware verification were performed.
