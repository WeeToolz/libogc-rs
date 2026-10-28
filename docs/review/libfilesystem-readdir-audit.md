# ISFS read-directory return-path audit

Reviewer: `/root/reviewer_4`  
Date: 2026-10-01  
Scope: independent resolution of the sync `ogc-filesystem::read_dir` result/count uncertainty against C `ISFS_ReadDir` and its callback path. No implementation changes, builds, tests, or hardware checks were performed. The shared filesystem note and ledger were not edited.

## Synchronous `read_dir`: no current mismatch

C `ISFS_ReadDir` passes `IOS_Ioctlv`'s return value through unchanged and writes `*num` only when `ret == 0` (`C-sources/libogc/libogc/isfs.c:196-234`, especially `:230-234`). Rust `read_dir` passes the `run_fs_ioctlv` result through unchanged and writes `*num` only when `rc == ISFS_OK` (`crates/services/ogc-filesystem/src/lib.rs:392-415,418-451`). `ISFS_OK` is exactly zero (`lib.rs:36`), `run_fs_ioctlv` returns `fd.ioctlv`'s result without sign or value conversion (`:323-328`), and `is_isfs_success` is exact equality to `ISFS_OK` (`:1283-1285`). Therefore both implementations update the count only on a zero result, including the case of any hypothetical positive nonzero result. The low-confidence sync finding in `docs/review/libfilesystem.md` is not supported by the current Rust source; its note says `is_isfs_success` accepts nonnegative results, but the function currently accepts only zero.

The synchronous C implementation is a direct ISFS wrapper, not a devoptab `readdir` callback. A search of the checked-in `C-sources/libogc` tree finds no other `ISFS_ReadDir` caller or ISFS devoptab directory callback; the matching result/count handling is in `isfs.c` itself.

## Adjacent asynchronous callback path

There is a separate conditional difference in `ISFS_ReadDirAsync`. C's general `__isfsFunctionCB` dispatches the read-dir handler for nonnegative completion results (`isfs.c:154-163`), but `__isfsReadDirCB` stores the count only when `result == 0` (`:147-152`). Rust's `read_dir_async_callback` stores the count for `result >= ISFS_OK` (`crates/services/ogc-filesystem/src/lib.rs:469-485`). Thus, if IOS reports a positive nonzero completion, Rust updates `num` while C leaves it unchanged; both then invoke the user's callback with the original result. The C caller does not convert the result before this branch (`isfs.c:237-275`). This is a distinct async-only behavior difference, conditional on IOS returning positive nonzero. The source comparison does not establish whether that result occurs at runtime.

## Result

The synchronous `read_dir` uncertainty is resolved in Rust's favor: the current C and Rust paths use the same exact-zero success condition and return the same ioctl result. The nearby asynchronous callback retains a conditional positive-result count-update difference. No tests were run or compiled, and no hardware verification was performed.
