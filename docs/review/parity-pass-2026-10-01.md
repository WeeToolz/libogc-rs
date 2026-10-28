# Additional C parity pass — 2026-10-01

This pass compared selected open Rust review findings against the uploaded current libogc C source. It is a static source comparison. No hardware validation is claimed, and this environment did not provide a Rust toolchain.

## `ogc-conf`

Fixed the malformed `setting.txt` embedded-NUL difference. C uses `strchr`, so searches for `=` and CR/LF stop at the first NUL in the current line. Rust now applies the same boundary while retaining C's raw newline skip behavior.

## `ogc-lwp-driver`

Fixed the documented semaphore `max` and counter differences. The C compatibility implementation ignores `max`, stores a signed count, decrements before blocking, increments before deciding whether to wake, and reports zero from `LWP_SemGetValue` for negative counts. Rust now follows those rules.

Waiter selection also now preserves FIFO arrival order among equal-priority waiters instead of using thread-slot index as the tie break, matching C `KThrQueueInsert` ordering.

## `lwip-sys`

Fixed several confirmed Wii socket differences:

- C socket flag values are used for `SOCK_CLOEXEC` and `SOCK_NONBLOCK`.
- Creation masks those flags before the IOS socket ioctl.
- Requested nonblocking mode is applied with F_GETFL/F_SETFL after creation.
- The default 32768-byte receive buffer is sent with `SO_RCVBUF` after creation.
- Missing `/dev/net/ip/top` state propagates `-ENXIO`.
- IPv4 family validation returns `-EAFNOSUPPORT` for `bind` and `connect`.
- `connect` returns `-EINVAL` for a short IPv4 address length with C's family-before-length precedence.
- `bind` does not reject short `namelen`, matching current C's unused length argument on that path.

The separate `SockLenT` signedness finding remains open.

## `ogc-aram`

No implementation patch was required. The current source already distinguishes its own installed ARAM IRQ handler from another driver's handler, so `reset()` followed by `init()` can reinitialize as C documents. Older review text that still called this mismatch open was updated with a follow-up note.

## Validation boundary

`git diff --check` passed. Static assertions confirmed the expected changed source patterns. Cargo/rustc were not available in this environment, so compilation and tests are left to the build-capable agent.
