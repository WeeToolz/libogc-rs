# ISFS async query copy-back audit

Reviewer: reviewer-6. Implementer: N/A (independent static validation).

## Result

No copy-back or completion-result mismatch was found for async GetStats, GetAttr, or GetUsage in the requested scope.

## Evidence

- C's shared `__isfsFunctionCB` dispatches these query helpers only for nonnegative results (`C-sources/libogc/libogc/isfs.c:154-163`). Each helper copies to caller outputs only when `result == 0`: GetStats copies the complete stats record (`isfs.c:116-121`), GetAttr assigns each output field (`:123-135`), and GetUsage assigns both values (`:137-145`). The shared callback then calls the user callback with the original result and returns that result (`:154-167`).
- Rust uses `is_isfs_success(result)`, defined as `result == ISFS_OK` (`crates/services/ogc-filesystem/src/lib.rs:1283-1285`), before copying in the corresponding callbacks: stats (`:1165-1184`), attributes (`:1187-1207`), and usage (`:1210-1226`). Each then invokes the user callback with the original result, ignores its return value, frees its context, and returns the original result.
- Rust's stats query uses an intermediate output buffer and copies it only on success, preserving the caller's prior bytes on failure (`ogc-filesystem/src/lib.rs:1229-1280`); C's `__isfsGetStatsCB` likewise does not touch the destination unless the result is zero. GetAttr and GetUsage also keep IOS output in their contexts and copy to caller storage only on zero result, matching the C helpers.

Positive nonzero completions do not copy results in either implementation for these three operations. This is separate from the read_dir callback, which is covered by the distinct positive-result audit.

## Validation

Static source comparison only. No builds or tests were run. No hardware verification was performed. The shared `libfilesystem.md` note and ledger were not changed.
