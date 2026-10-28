# Runtime contract review

Reviewer: `thread-current/reviewer_3`  
Scope: all source files in `crates/core/ogc-runtime`  
Status: the `ThreadId` sentinel fix in commit `99c2d1a` passed independent static re-review by `/root/reviewer_f`. The original finding below describes the behavior before that fix. No hardware verification is recorded.

## Original confirmed mismatch

### Original finding — addressed in the fix handoff

- `ThreadId` does not exclude libogc's null-thread sentinel. C defines `LWP_THREAD_NULL` as `0xffffffff` in `../../C-sources/libogc/gc/ogc/lwp.h:52`, and defines `lwp_t` as `u32` at lines 60–63. Rust stores a `NonZeroU32` and `ThreadId::new` accepts every nonzero value (`crates/core/ogc-runtime/src/scheduler.rs:7–18`), including `0xffffffff`. Therefore a caller can construct a `ThreadId` whose `get()` value is the C null handle, even though the type is documented as a scheduler-assigned thread identifier. `NonZeroU32` excludes zero, while C's invalid sentinel is all ones. This is a public API invariant mismatch. `ThreadId` currently has no in-workspace call sites, so the invalid value does not currently flow into scheduler operations through this type.

Commit `99c2d1a` changes `ThreadId::new` to return `None` for `u32::MAX` and `Some(ThreadId)` for other nonzero values. The matching handoff is `libruntime-threadid-sentinel-fix.md`; independent re-review is recorded below.

## Compared contract boundaries

`ogc-runtime` has no direct C module equivalent; it is a Rust-only shared interface crate with no device implementation or C ABI translation (`crates/core/ogc-runtime/src/lib.rs:3–7, 14–25`). The C interfaces it abstracts are subsystem-specific. For example, C LWP exposes `u32` thread and queue handles plus queue initialization, sleep, signal, and broadcast functions (`../../C-sources/libogc/gc/ogc/lwp.h:60–69, 157–199`; implementations in `../../C-sources/libogc/libogc/lwpcompat/lwpc_queue.c:7–48`). C USB asynchronous transfers use callback-plus-user-data functions and signed status returns (`../../C-sources/libogc/gc/ogc/usb.h:206–225`), while C storage drivers use `DISC_INTERFACE` callback tables (`../../C-sources/libogc/gc/ogc/disc_io.h:56–67`). The Rust `Lifecycle`, `BlockDevice`, `AsyncOperation`, `DebugTransport`, and `HidTransport` traits, and its detailed `WiiError`, are shared abstraction choices rather than direct ABI replicas (`crates/core/ogc-runtime/src/driver.rs:7–53`, `error.rs:1–54`). No additional static mismatch was confirmed in those declarations; the downstream adapters own their operation and compatibility semantics.

`WakeToken` also has a public raw `u32` field (`scheduler.rs:33–35`) despite being described as opaque. This permits callers to construct arbitrary tokens, but the type does not claim to encode a C ABI handle, and downstream LWP adapters validate their queue sentinels. I did not record this as a C parity finding.

## History and validation

The scheduler contract was introduced in commit `497ffc7` and moved under `crates/core` by `fce62d7`; no later implementation fix is under review. `cargo check -p ogc-runtime --target powerpc-unknown-linux-gnu` passed. No tests were run, and no hardware verification was performed.

## Independent re-review of ThreadId sentinel fix

**Result: PASSED** for commit `99c2d1a` (`Reject null sentinel from runtime ThreadId`). Reviewer: `/root/reviewer_f`. Implementer: `/root/worker_b` (as identified in `libruntime-threadid-sentinel-fix.md`). This passes the recorded public-value-domain mismatch; no other mismatch was found in the affected runtime path.

- C defines `LWP_THREAD_NULL` as `0xffffffff` and `lwp_t` as `u32` (`../../C-sources/libogc/gc/ogc/lwp.h:52,60-63`). The C LWP implementation rejects both zero and `LWP_THREAD_NULL` before inverting a thread handle into a `KThread *` (`../../C-sources/libogc/libogc/lwpcompat/lwpc_thread.c:5-12`). C thread creation writes `LWP_THREAD_NULL` when allocation fails, while `LWP_GetSelf` returns the encoded current thread (`:14-32,75-78`).
- Rust `ThreadId` wraps `NonZeroU32`; its revised constructor rejects exactly `u32::MAX` and returns an identifier for other nonzero values (`crates/core/ogc-runtime/src/scheduler.rs:7-24`). This excludes both C invalid representations: zero is unrepresentable by the input type and the all-ones sentinel is explicitly rejected. `get` returns the underlying ID unchanged (`:24-26`).
- The adjacent Rust LWP driver has a different internal scheduler ID scheme: it issues `index + 1` for live threads, rejects zero and `LWP_THREAD_NULL` during handle decoding, and writes the null sentinel on thread-creation failures (`crates/core/ogc-lwp-driver/src/runtime.rs:359-370,945-978,1015-1045`). Its `current_thread` returns `handle(CURRENT)` (`:1048-1050`). The LWP facade's `ThreadHandle` deliberately preserves the raw sentinel and exposes `is_null` (`crates/core/ogc-lwp/src/lib.rs:14-17,44-59`); that is separate from the runtime `ThreadId` constructor and is unchanged by this fix. The runtime crate has no in-workspace `ThreadId::new` call sites beyond the declaration/export, so the constructor's new `Option<Self>` return requires no repository call-site changes.
- Regression coverage checks that `u32::MAX` is rejected, a representative valid nonzero value is accepted without modification, and zero cannot be represented by `NonZeroU32` (`scheduler.rs:82-92`). It covers constructor semantics but does not exercise a scheduler lifecycle.

Validation: `cargo check -p ogc-runtime --target powerpc-unknown-linux-gnu` passed. `cargo test -p ogc-runtime --no-run --target powerpc-unknown-linux-gnu` compiled the test crate but failed at link because the configured `cc`/`ld` rejected cross-target options such as `--as-needed` and `-Bstatic`. No tests ran. No hardware verification was performed.
