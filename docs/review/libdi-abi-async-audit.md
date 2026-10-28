# DI ABI and async callback audit

Reviewer: `/root/reviewer_6`  
Scope: direct comparison of the legacy DI C declarations and `DI_ReadDVDAsync` path with the current Rust `ogc-di` crate. The shared `docs/review/libdi.md` and ledger were not changed.

## Confirmed legacy ABI gap

The C header declares the `DI_*` API, the global `di_fd`, and `__io_wiidvd` with C linkage (`../../C-sources/libogc/gc/di/di.h:90-126`). Rust provides similarly named service functions such as `init`, `mount`, and `read_dvd`, but these use Rust names and have no `#[no_mangle]` / `#[export_name]` declarations (`crates/services/ogc-di/src/lib.rs:830-1319`). `WII_DVD_INTERFACE` is a public Rust `const`, not an exported symbol named `__io_wiidvd` (`lib.rs:1368-1377`). The Rust crate therefore cannot satisfy existing C callers or link-time references to the legacy DI symbols. This limitation is also stated in the crate module docs (`lib.rs:3-8`).

Some individual ABI components are compatible: `DiDriveId` uses `#[repr(C)]` with the C field types (`di.h:80-84`; Rust `lib.rs:80-86`), and `DiCallback`, `ReadFunc`, `ReadFuncAsync`, and `Ipccallback` use `extern "C"` function pointers consistent with the C typedefs (`di.h:86-89`; Rust `lib.rs:88-91`; `../../C-sources/libogc/gc/ogc/ipc.h:71`). Those type matches do not provide the missing exported functions or globals.

## Confirmed async behavior differences

- **The Rust callback path is deferred synchronous work, not the C IOS async request.** C `_DI_ReadDVD_D0_Async` and `_DI_ReadDVD_A8_Async` submit `IOS_IoctlAsync` with the caller's callback and buffer as user data (`../../C-sources/libogc/libdi/di.c:70-112`). `DI_ReadDVDAsync` invokes the selected async function pointer directly (`di.c:554-565`). Rust `read_dvd_async` stores the request in a slot and queues an LWP deferred worker (`crates/services/ogc-di/src/lib.rs:1090-1146`); that worker runs synchronous `read_dvd` before invoking the callback (`lib.rs:1050-1079`). The read therefore occupies the worker until completion and does not preserve the C submission/completion behavior or execution context.
- **Callback result values differ for IOS result 2.** C translates immediate submission return `2` to `EIO`, but the callback receives the IOS completion result directly (`di.c:83-89,106-112`). Rust's synchronous `ioctl_words` normalizes result `2` to `-EIO` (`crates/services/ogc-di/src/lib.rs:270-289`), then `async_callback_status` maps only success `0` to `1` and forwards other values unchanged (`lib.rs:72-78,1075-1078`). Thus a completed IOS result of `2` is delivered to a C callback as `2`, but as `-5` through the Rust callback.
- **Async reads use the Rust read cache while the C async API bypasses it.** C synchronous `DI_ReadDVD` wraps the chosen read function in `ReadBlockFromCache` (`di.c:540-551`), but `DI_ReadDVDAsync` calls `DI_ReadDVDAsyncptr` directly (`di.c:554-565`). Rust's async worker calls `read_dvd`, which routes through `read_from_cache_or_drive` (`crates/services/ogc-di/src/lib.rs:1063-1065,1042-1044,465-500`). With `use_cache` enabled, the Rust async API can return cache data without issuing the asynchronous IOS read that C would submit.

## Validation limits

This was a source-only ABI and callback audit. No build, tests, or hardware verification were performed. No implementation changes were made.
