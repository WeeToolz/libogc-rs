# ISFS async `read_dir` positive-result audit

Reviewer: reviewer-6. Implementer: N/A (independent static validation).

## Result

Confirmed a result-handling mismatch for positive nonzero IOS results. C does not update the caller's `num` in that case; Rust does.

## Evidence

- C dispatches the `ISFS_FUNCREADDIR` helper for every nonnegative completion result: `C-sources/libogc/libogc/isfs.c:154-163`. The helper writes `param->fsreaddir.no_entries` to the caller's `num` only when `result == 0`, then returns the original result (`isfs.c:147-152`). Thus positive results reach the helper but leave caller count unchanged.
- Rust's `read_dir_async_callback` writes `context.count` to `*context.num` for every `result >= ISFS_OK` (`crates/services/ogc-filesystem/src/lib.rs:469-477`). With `ISFS_OK` equal to zero, positive nonzero results update the caller count, unlike C.
- The user callback is invoked afterward with the original result in both implementations (C `isfs.c:154-167`; Rust `ogc-filesystem/src/lib.rs:478-480`).

This audit is limited to whether positive nonzero results produce a difference. No other `read_dir` behavior was assessed.

## Validation

Static source comparison only. No builds or tests were run. No hardware verification was performed. The shared `libfilesystem.md` note and ledger were not changed.
