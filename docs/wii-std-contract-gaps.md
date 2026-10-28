# Wii standard library contract gaps

Updated: 2026-10-15. The tables below preserve the initial contract inventory
and are not a current implementation status report. Since that baseline, the
repository added and connected allocator, thread/TLS, synchronization, clock,
network, filesystem, path, environment, output, and runtime providers. Use the
[current checklist](wii-std-checklist.md), [handoff](wii-std-handoff.md), and
the topic evidence linked below for current results. Unsupported operations
must still fail explicitly. DOL builds and Wii runs are recorded separately;
they do not prove every Rust standard-library contract.

## Runtime provider source review — 2026-10-15

The `ogc-std-runtime` boundary is Rust-owned and has no matching C API. Its
exported allocation, errno, TLS, futex, thread, clock, output, filesystem, and
network operations are consumed by the pinned std overlay. The boundary
validates pointers, lengths, flags, operation codes, and resource ownership
before it dispatches to the production allocator, LWP, system, FAT, or IOS
network service. Unsupported std operations return an unsupported error where
their API can return one; APIs without an error result use an explicit target
policy documented in the topic notes.

The provider DOL set built on 2026-10-15 with `cargo xtask
build-wii-std-probe`; the command produced ten probe DOLs. These are build/link
results only. `cargo test -p ogc-std-runtime` cannot run on the host because
its PowerPC-only dependencies reject host targets. The real-Wii evidence and
the remaining API support limits are in the current checklist and handoff.
This closes the static provider-boundary comparison only; it does not claim
that every `std` API is supported or hardware-accepted.

Date: 2026-10-14. Status: contract assessment with foundation and synchronization adapters.

Use the [implementation checklist](wii-std-checklist.md) for current progress
and the [thread handoff](wii-std-handoff.md) for repository and toolchain details.
The contract rows preserve the original assessment. The first mounted FAT file
adapter is described in [file design](wii-std-files.md); directory/path services
and hardware acceptance remain open. Source implementations now
exist for TLS, detach, and threaded synchronization. Full std integration and
hardware acceptance remain open.

## Foundation implementation status

The initial implementation adds checked heap startup, zeroed allocation,
resize with failure preservation, per-thread errno access, TLS keys and
destructors, and join/detach ownership with stack cleanup. The standard library
adapters call the application's `ogc-std-runtime` provider. The provider uses
the existing allocator, system services, and LWP scheduler. The error classifier
is shared with `ogc-sysbase`; std does not create another scheduler or heap state.

The matching Rust source is pinned in `rust-std/source-lock.json`. Run
`python3 scripts/check-wii-std.py --offline` to prepare the source, install the
Wii sysroot when its inputs change, and compile a std consumer against that
sysroot. The producer checks compiler and source hashes. Consumer checks do
not pass `-Z build-std`.

The std compile check now passes. Mutex, condition variable, once, and
reader/writer lock selectors use the pinned Rust futex algorithms over LWP.
Random requests explicitly fail as unsupported; no entropy source is accepted.
The actual std synchronization probe builds and links as
`target/xtask/probes/wii_std_probe.dol`, with no undefined symbols or dynamic
dependencies. It has not run on Wii. See [the synchronization design and entropy
decision](wii-std-sync.md). Further platform contracts remain open.

Host allocator and shared TLS/lifecycle engine tests pass. The production Wii
LWP probe builds and links as `target/xtask/probes/lwp_probe.dol`. It checks TLS
and errno isolation, destructors, 64 detached dynamic stacks, allocation bytes,
alignment, failed resize preservation, and provider error results. It has not
run on hardware. Source and link checks do not close hardware evidence gaps.

The build result below records the original unpatched source baseline.
See [foundation verification evidence](wii-std-foundation-evidence.txt) for the
earlier baseline and [synchronization evidence](wii-std-sync-evidence.txt) for
the current checks, failed checks, and target build limits.

## Scope and evidence

This document maps the Rust standard library platform contracts to the current
Wii production APIs. It records missing behavior before backend implementation.
An API name or a passing compile check does not prove the required behavior on
hardware. Every gap below is open until the stated evidence is available.

The repository baseline is `79fc8f88beaecf0f7fdd290238cfc806ff5960a8`.
The target is [powerpc-nintendo-none.json](../targets/powerpc-nintendo-none.json):
PowerPC 750, big endian, 32-bit pointers, `os=none`, `env=eabi`, maximum atomic
width 32, static linking, and panic abort. The separate GameCube target is outside
this assessment. Backend selection must distinguish Wii from GameCube.

The Rust source inspected is the installed nightly `rust-src`:

- Compiler: `1.101.0-nightly`, commit
  `ea137335b78829b4514bf1b4c16302f74fab8581`, dated 2026-10-05.
- Source root: the nightly sysroot's `lib/rustlib/src/rust/library`.
- Rust references below start at `std/src/` in that source root.
- Public contracts come from the source documentation for `alloc`, `thread`,
  `sync`, `time`, `net`, `fs`, `io`, `env`, and `process`.
- Platform entry points come from `sys/mod.rs` and the selectors in each `sys`
  component. These internal interfaces can change between Rust releases.

Pin the compiler and its matching source before implementing the backend.
The active stable compiler is 1.97.0; it is not the source baseline for this map.

## Original unpatched build result

Two isolated Cargo compile checks ran outside this repository with the checked-in
Wii target and the installed nightly compiler. Neither check linked or ran a DOL.

| Library components | Result | Evidence limit |
| --- | --- | --- |
| `core`, `alloc`, `compiler_builtins` with memory builtins | Passed in a small `no_std` library that exposes an `alloc::vec::Vec<u8>` result | Confirms compilation, not heap initialization or allocation on a Wii |
| `std`, requested with `panic_abort` | Failed with 14 compiler errors in `std` | Confirms missing platform selection; does not identify every later link or runtime failure |

The `std` diagnostics identify five immediate blockers:

1. `sys/alloc/mod.rs` has no selected allocator implementation.
2. `sys/io/error/mod.rs` has no selected error implementation.
3. Mutex, condition variable, once, and reader/writer lock selection reaches
   `no_threads`; those implementations reject this target with compile errors.
4. OS thread-local storage selects missing `Key`, `LazyKey`, `get`, and `set`
   implementations. The thread-exit guard also lacks its key implementation.
5. Random support has no selected `fill_bytes` implementation.

`std/build.rs` also marks this target with `restricted_std`. Enabling
`#![feature(restricted_std)]` in the temporary application did not supply any of
the missing implementations. No target flags were changed to bypass these errors.
The compile check used the default `std` feature set; its log also shows compilation
of unwind packages. A production build still needs an explicit panic feature and
link audit.

See [compile evidence and reproduction](wii-std-build-evidence.txt).
Since 2026-10-15, DOL application and probe builds use the installed Wii
sysroot and do not pass `-Z build-std`. Run
`cargo xtask build-wii-std-sysroot` to build or verify the pinned libraries.
That producer still uses `-Z build-std=std,panic_abort`. Consumer builds keep
`-Z json-target-spec`. A DOL build does not establish application acceptance.

## Status terms

- **Adapter needed:** a production primitive exists, but the Rust contract needs
  translation, ownership, validation, or error handling.
- **Service gap:** the current primitive cannot provide all required behavior.
- **Unsupported candidate:** the operation is outside the initial Wii service
  model. An API with a result must return a clear unsupported error until a real
  implementation exists. This is a proposed support boundary, not a claim that
  the hardware can never provide it.
- **Hardware evidence needed:** source behavior exists, but real-device acceptance
  for the Rust contract is open.

Rust has APIs with no error result. `Instant::now`, `SystemTime::now`, thread
sleep, and several synchronization operations cannot return `io::ErrorKind::Unsupported`.
They need a real implementation and a defined fatal failure policy where necessary.
Do not substitute a zero clock, a no-op sleep, or an unlocked mutex.

## Allocation, startup, and errors

Wii source: [ogc-alloc](../crates/core/ogc-alloc/src/lib.rs),
[ogc-lwp-driver](../crates/core/ogc-lwp-driver/src/runtime.rs),
[ogc-sys startup](../crates/abi/ogc-sys/src/lib.rs),
[ogc-sysbase](../crates/core/ogc-sysbase/src/lib.rs), and
[target linker script](../targets/powerpc-nintendo-none.ld).
Rust source: `sys/alloc`, `sys/io/error`, `rt.rs`, and `sys/pal`.

| ID | Rust contract | Current Wii path | Gap and required behavior |
| --- | --- | --- | --- |
| A01 | `std::alloc::System` allocates, frees, zeroes, and resizes with the requested alignment | `ArenaAllocator::allocate`, `deallocate`, `GlobalAlloc` implementation | Adapter needed. Implement the platform `System` path as well as the application's optional global allocator. Preserve the old allocation if resize fails; check bounds and alignment; return null for allocation failure. |
| A02 | Heap use is valid during runtime initialization, thread creation, and error reporting | `initialize_wii` reserves MEM2 and advances arena2 low; LWP has separately configured workspace allocation callbacks | Define one startup order and explicit ownership of both heaps. Prevent overlapping ranges and allocator/TLS initialization cycles. Default heap limits are configurable limits, not a measured free-memory guarantee. |
| A03 | Rust runtime startup and termination execute in the correct order | `_start_rust` shelters loader arguments, clears BSS, installs the IRQ vector, calls the exported C ABI `main`, then exits | Adapter needed for Rust `lang_start`/runtime initialization and cleanup. Define how `std` programs enter this existing path; verify initial thread registration and initialization before any allocation or TLS use. |
| E01 | `io::Error` keeps useful raw error codes and classifies failures correctly | Network APIs use negative results; devoptab callbacks use `-1` plus per-thread `Reent` errno; `FatError` is typed | Service gap. Add source-specific translation for IOS/socket codes, LWP results, devoptab errno, and FAT errors. Never treat these code spaces as one POSIX errno table. Distinguish timeout, would-block, interruption, permission, missing file, exhausted resources, and unsupported operations. |
| E02 | `last_os_error` reads the calling thread's error state | `current_reent` delegates to the LWP thread's newlib-compatible storage | Adapter needed. Read errno immediately after a failed devoptab call. Existing `DeviceFile::open_with_flags` returns only `Option`; some network wrappers collapse detailed failures to `SOCKET_ERROR`. These interfaces lose details needed by `std`. |

The allocator disables interrupts while it owns its internal lock on PowerPC.
The backend must preserve this rule. Allocator failure paths must not allocate
again to format an error. Thread-local access must also avoid allocator recursion.

## Threads, thread-local storage, and synchronization

Wii source: [LWP driver](../crates/core/ogc-lwp-driver/src/runtime.rs),
[LWP ABI](../crates/core/ogc-lwp-sys/src/lib.rs), and
[LWP wrappers](../crates/core/ogc-lwp/src/lib.rs).
Rust source: `thread/mod.rs`, `sys/thread`, `sys/thread_local`, and
`sys/sync/{mutex,condvar,rwlock,once,thread_parking}`. Standard channels and barriers
also depend on these shared primitives; they do not require separate Wii transports.

| ID | Rust contract | Current Wii path | Gap and required behavior |
| --- | --- | --- | --- |
| T01 | `Builder::spawn` returns creation failures and runs the supplied closure on a valid stack | `create_thread`, `thread_exit`, and `join_thread` | Adapter needed. Own closure storage until transfer succeeds; release it on creation failure. Validate `usize` stack requests before conversion to `u32`. Configure priority and stack alignment explicitly. Translate `LWP_CLOSED`, which currently covers several distinct failures. |
| T02 | Joining waits for completion and collects the result with synchronization | `join_thread` blocks, collects the exit pointer, clears the slot, and releases an owned dynamic stack | Adapter needed and hardware evidence needed. Preserve Rust result publication and memory ordering; ensure closure data and stack storage are each released once. Panic abort means a panic cannot produce an unwind result for a joiner. |
| T03 | Dropping `JoinHandle` detaches the thread without stopping it | No exported detach operation was found; `thread_exit` leaves a finished slot for `join_thread` | Service gap. Add detached lifecycle and safe reclamation after the thread switches off its stack, or an equivalent production reaper. Repeated detached threads must not retain stacks and slots indefinitely. |
| T04 | `thread_local!` isolates values and runs required destruction at thread exit | Per-thread newlib `Reent` exists; a general Rust TLS key/value and destructor service was not found | Service gap. Provide lazy key initialization, per-thread values, destructor registration/order, destruction-state checks, and initial-thread handling. Newlib reentrancy storage is not itself a Rust TLS implementation. Do not advertise native TLS without compiler and ABI support. |
| T05 | `sleep` waits at least the requested duration; `yield_now` reaches a scheduling point | `sleep_ticks` and `yield_thread` | Adapter needed. Use checked duration conversion with rounding up; split very long waits if needed. `sleep_ticks` currently uses wrapping deadline addition. Check cancellation and early wake behavior against the infallible Rust sleep contract. |
| T06 | Thread identity remains valid; `available_parallelism` describes usable CPU parallelism | `current_thread`; one application PowerPC CPU | Adapter needed. Keep Rust `ThreadId` independent of reusable raw slot handles. Report one usable application CPU only after target identity is established; thread capacity is not parallelism. Rust names can remain in Rust thread metadata. |
| S01 | `Mutex` blocks, gives exclusive access, and supplies the Rust poison policy | LWP non-recursive mutexes with priority inheritance | Adapter needed. Implement constant construction or safe lazy initialization; handle the fixed pool of 64 mutexes without reporting an acquired lock on exhaustion. Pool failure cannot become `PoisonError`, which has a different meaning. Preserve acquire/release ordering. |
| S02 | `Condvar` releases the mutex and waits atomically, then reacquires it, including on timeout | `cond_wait` queues while interrupts are masked, releases and reacquires the mutex; timed results use `-116` | Adapter needed and hardware evidence needed. Map timeout separately from cancellation/errors; implement Rust mutex association rules and checked duration conversion. The fixed pool contains 64 condition variables. |
| S03 | `RwLock` permits concurrent readers or one exclusive writer | No reader/writer lock service was found | Service gap. Build it from the production blocking primitives with explicit ownership, wake policy, and memory ordering, or implement a production LWP reader/writer lock. The selected Rust `no_threads` implementation is invalid for Wii threads. |
| S04 | `Once`/`OnceLock` synchronize initialization and wake all waiters; poison behavior is preserved where relevant | LWP locks and wait queues can supply building blocks | Service gap in platform integration. Select or implement a real threaded once engine. Verify publication, contended initialization, recursion/failure handling, and resource exhaustion. |
| S05 | `park`/`unpark` retain one token, including unpark-before-park; timed waits can return spuriously | LWP queue permits and external wait/wake APIs | Adapter needed. LWP's bounded permit queue is not automatically Rust's one-token parker. Implement token coalescing, wake races, timeout races, and object lifetime. Do not use spin waiting as the ordinary blocking path. |
| S06 | Atomic operations used by the backend match the target | Target maximum atomic width is 32 | Adapter needed. Use target-supported atomics or locking for wider shared data. Do not assume `AtomicU64` exists on Wii. Verify synchronization across context switches and interrupt completion. |

Current source allocates additional thread pages and owned stacks through workspace
callbacks. The older [LWP runtime note](lwp-runtime.md) describes a fixed 32-thread
limit and a 32 KiB stack ceiling. Those statements do not describe all current
creation paths. Use `reserve_thread_slot` and `allocate_owned_stack` as the source
for this assessment. Hardware acceptance of the scheduler remains open; consult
existing probe reports, but do not count a source audit as a device pass.

## Time

Wii source: [PowerPC timebase](../crates/core/ogc-core/src/lib.rs),
[LWP deadlines](../crates/core/ogc-lwp-driver/src/runtime.rs), and
[system clock](../crates/core/ogc-system-sys/src/lib.rs).
Rust source: `time.rs`, `sys/time`, and `sys/thread/mod.rs` (`sleep_until`).

| ID | Rust contract | Current Wii path | Gap and required behavior |
| --- | --- | --- | --- |
| C01 | `Instant` is monotonic and supports checked arithmetic | Stable upper/lower/upper timebase read; Wii conversion uses 60,750,000 ticks/second | Adapter needed. Use quotient/remainder conversion to avoid overflow. `set_system_time` can rewrite the hardware timebase during initialization; establish monotonic state after that step and define behavior if another component changes it later. |
| C02 | `SystemTime` represents wall time relative to `UNIX_EPOCH` | `read_rtc`, counter bias, `set_boot_time`, and `read_system_time` | Service gap. Verify RTC epoch, bias interpretation, and boot-time adjustment before conversion. `sys_time` is a tick value and cannot be used directly as Unix seconds. Failed RTC reads currently cause `set_boot_time` to return without exposing a validity result. |
| C03 | Public clock reads without a result do not invent a valid timestamp on hardware failure | `SystemTime::now` is infallible; RTC initialization can fail | Define an observable clock-validity state and a documented fatal failure policy if no valid wall clock exists. A fallible internal clock API is needed; zero or a fixed timestamp is not acceptance evidence. |
| C04 | Timed operations use the same duration/deadline interpretation | LWP tick deadlines and socket millisecond/timeval forms | Adapter needed. Check overflow, fractional rounding, zero duration, infinite wait, early wake, and retries against an absolute deadline. Hardware probes must measure elapsed time independently. |

## Network and DNS

Wii source: [socket wrapper](../crates/services/ogc-network/src/lib.rs),
[IOS network service](../crates/services/ogc-network-sys/src/lib.rs), and
[socket transport](../crates/services/lwip-sys/src/lib.rs).
Rust source: `net/{tcp,udp,addr}.rs`, `sys/net/connection`, and `sys/net` lookup code.

| ID | Rust contract | Current Wii path | Gap and required behavior |
| --- | --- | --- | --- |
| N01 | TCP and UDP use real connections, return byte counts, and release resources | `init`, `soc_init`, socket table, IOS socket operations | Adapter needed and hardware evidence needed. `soc_init` initializes the wrapper table; it does not initialize IOS networking. Define network initialization ownership and a policy that cannot deinitialize networking while live sockets use it. Preserve partial I/O and distinguish EOF from failures. |
| N02 | `set_nonblocking` succeeds only after the mode changes; I/O returns `WouldBlock` when appropriate | `fcntl`/`ioctl`; the low-level socket constructor also has a `SOCK_NONBLOCK` path | Service gap. Socket construction currently discards the nonblocking setup result and records nonblocking state. The backend must check the final transport result and keep state consistent on failure. |
| N03 | `connect_timeout` rejects zero duration and verifies connection completion before success | Nonblocking connect and IOS poll exist | Service gap. `get_socket_option` always returns `-EINVAL`, including `SO_ERROR`. Poll readiness alone is not proof that connect succeeded. Supply a verified completion/error query or return `Unsupported` for timed connect until one exists. Preserve the deadline across retries and restore the required mode. |
| N04 | Socket option getters return actual state; `take_error` reads and clears a pending error | `setsockopt` submits an IOS request; `getsockopt` is unimplemented | Service gap. Timeout, nodelay, TTL, multicast, broadcast, and error queries need per-option support decisions. A successful setter does not prove a getter exists. Return `Unsupported` for unavailable queries; do not fabricate `None` from `take_error`. |
| N05 | Read/write timeouts obey their duration, with zero rejected and `None` meaning no timeout | Timeval option constants and poll are present | Adapter needed plus transport verification. Check each IOS option response. A poll-before-I/O substitute needs concurrency control so another caller cannot consume readiness and cause an unbounded blocking operation. |
| N06 | `try_clone` creates another owned handle to the same underlying socket and option state | No socket duplication API was found | Service gap. A shared Rust owner with reference counting may satisfy this contract without an IOS duplication call, but close must occur once and operation lifetimes must prevent close/reuse races. Do not allocate an independent connection. |
| N07 | Local and peer addresses describe the real socket | IOS `getsockname`; low-level `get_peer_name` uses peer state cached after successful connect/accept | Adapter needed. The high-level wrapper lacks a peer query. Verify cached peer state on reconnect, shutdown, UDP association, failed connect, and socket reuse; do not report attempted connection data as an established peer. |
| N08 | DNS produces an owned address iterator and retains useful lookup errors | `net_gethostbyname` uses one `static mut HOSTENT_BUFFER` and returns a raw pointer or null | Service gap. Serialize the complete request and result copy, validate all output extents and list termination, and copy results into owned storage before unlocking. Null currently loses transport/validation failure details. A lock around only the final copy is insufficient. |
| N09 | Numeric addresses do not require DNS; socket-address encoding matches the transport | IPv4 types; Wii sockaddr length byte and network-order fields | Adapter needed. Preserve port/address byte order and validate lengths. IPv6 socket operations are an unsupported candidate in the current IPv4 transport; parsing an IPv6 address remains pure Rust behavior. |
| N10 | UDP send/receive, peek, broadcast, and multicast preserve datagram semantics | Datagram calls, flags, and option submission exist | Adapter needed and hardware evidence needed. Verify truncation behavior, empty datagrams, peek without consumption, source addresses, membership changes, and error handling. Unix-domain sockets and ancillary Unix APIs are outside this initial support model. |

Do not expose virtual socket handles as IOS descriptors. The wrapper and low-level
transport keep separate socket tables. Several wrapper operations, including
`close`, `select`, and option operations, convert detailed failures to `-1`.
The low-level socket constructor also ignores its receive-buffer setup result.
These results require review before this path can support the production policy.

## Files, paths, and standard streams

Wii source: [FAT service](../crates/services/ogc-fat/src/lib.rs),
[devoptab and DeviceFile](../crates/core/ogc-sysbase/src/lib.rs),
[ISFS service](../crates/services/ogc-filesystem/src/lib.rs), and
[system reporting](../crates/core/ogc-system/src/lib.rs).
Rust source: `fs.rs`, `sys/fs`, `sys/fd`, `sys/path`, `sys/paths`, `sys/os_str`,
`sys/stdio`, and `io/stdio.rs`.

| ID | Rust contract | Current Wii path | Gap and required behavior |
| --- | --- | --- | --- |
| F01 | `OpenOptions` uses every requested access/create/truncate/append flag, including atomic create-new | FAT `open_file_from_path` and devoptab open; public `FatFile` constructors expose a smaller set | Adapter needed. Expose exact flags without fallback to weaker flags. `DeviceFile::open_append` can retry without `O_APPEND`, so it cannot be used directly for the Rust append contract. Return the actual open error. |
| F02 | Read, write, seek, and set-length validate widths and preserve cursor behavior | FAT uses a `u32` size/position; devoptab seek accepts `i64`; ISFS uses narrower offsets | Adapter needed. Validate public `u64` offsets and signed seeks before conversion. Return a clear range error for unrepresentable files; do not truncate offsets. Handle partial I/O and zero-length requests. |
| F03 | `File::try_clone` shares the same file position and underlying file | No file duplication wrapper was found; FAT stores position in `FatFileState` | Service gap. Share one synchronized open-file state and resource lifetime. Reopening the path gives a different cursor and can fail because FAT permits one writer with no other open handles. |
| F04 | Shared file access remains safe across threads | FAT mount locks and open-file accounting exist; `DeviceFile` contains raw pointers | Adapter needed. Prove the required `Send`/`Sync` behavior through synchronized state and mount lifetime. Do not add unsafe marker implementations without these guarantees. |
| F05 | `sync_all` and `sync_data` report persistence and media failures | FAT `flush`, devoptab `fat_fsync_r`, and `flush_transport`; USB issues a transport flush | Service gap. Current file flush paths can return immediately when metadata is not dirty. Define whether data is still buffered and ensure the required transport synchronization occurs. Non-USB transports need documented write-completion guarantees. `sync_data` may use the stronger `sync_all` path. Verify by reopening/remounting persisted data. |
| F06 | File destruction releases resources; explicit sync exposes persistence errors | `FatFile::drop` discards flush errors; `DeviceFile::drop` discards close errors | Adapter needed and policy gap. Rust `Drop` cannot return errors. Keep explicit sync fallible and add an observable failure record where cleanup failures affect validity. Do not describe drop as verified persistence. Review ownership after failed close. |
| F07 | Metadata reports actual size, type, permissions, and supported times | FAT basic metadata and `build_stat`; ISFS attributes and file stats | Service gap. FAT `build_stat` leaves timestamp fields at their default; do not convert those fields to real epoch timestamps. Return `Unsupported` from unavailable timestamp methods. Implement timestamp reads/setters only after real on-disk decoding and persistence support exists. |
| F08 | Directories, rename, remove, copy, and canonicalization preserve their documented results | FAT directory iterator, create/remove/rename, current-directory cluster, and devoptab hooks | Partial: typed FAT traversal is connected with owned names/paths, dot filtering, per-entry errors, and mount lifetime. Source tests and probe builds do not establish hardware acceptance. Implement cross-volume behavior, copy cleanup, and rename failure behavior. Resolve paths against actual mount state. Recursive removal must validate every operation. Existing NAND APIs must not be selected implicitly for ordinary filesystem paths. |
| F09 | `Path` and `OsStr` represent Wii paths and filename bytes correctly | Paths such as `sd:/apps/x/boot.dol`; FAT names can be non-UTF-8 | Service gap in path integration. Default Rust path selection is Unix syntax: `sd:/` is not an absolute Unix path. Define mount-prefix parsing, roots, joins, parents, `..`, relative paths, and `is_absolute`; preserve filename bytes and reject embedded NUL before transport calls. Define `env::join_paths`/`split_paths` so a list separator does not corrupt mount prefixes. A device lookup alone is insufficient. |
| F10 | File locks, hard links, symbolic links, and timestamp changes perform real operations | No file-lock service; FAT `fat_link_r` returns `ENOTSUP`; symlink/readlink/utimes hooks are absent | Unsupported candidates. Return clear unsupported errors for these operations until implemented. Read-only permission changes have FAT support but need mapping; `fchmod` and arbitrary Unix permissions are separate gaps. |
| F11 | Standard output/error writes reach a sink or report a failure; input has defined EOF/error behavior | Devoptab stream slots start as `stdnull`; UI/devices can install sinks; `try_report` is a formatting interface | Service gap. `try_report` returns success on PowerPC when no callback exists. The Rust unsupported stdio backend also reports output success without a sink. Select a real byte transport and return `Unsupported` or a device error when absent; define partial writes, flush, input, stream locking, and panic output. |

ISFS is a distinct NAND service with permissions and alignment/IPC requirements.
It is not the default replacement for a mounted FAT filesystem. Filesystem
acceptance must use a controlled test location, preserve existing data, and follow
the firmware policy if any test would modify firmware or unique device state.

## Remaining platform contracts

Rust source: `sys/{args,env,env_consts,paths,process,pipe,random,backtrace,personality}`,
`sys/cmath.rs`, `process.rs`, and `rt.rs`.

| ID | Rust contract | Current Wii path | Gap and support boundary |
| --- | --- | --- | --- |
| P01 | `args_os` owns valid loader arguments for iteration | `ogc-sys` shelters the HBC argument block before BSS clear and passes argc/argv to main | Adapter needed. Initialize Rust's argument storage from this validated block; preserve byte strings and lifetime. Zero arguments are valid only when the loader actually supplies none. |
| P02 | Environment lookup and changes describe real process state | No general environment-variable store was found | Service gap. Either provide a documented application environment store with thread-safe lookup/mutation or define absent-variable behavior and an explicit failure policy for mutation. `set_var`/`remove_var` have no error result, so silently ignoring them is not an implementation. |
| P03 | Current directory, executable path, temporary directory, and home queries describe usable paths | FAT tracks current-directory clusters; loader arguments can contain an executable path | Adapter/service gap. Need an owned canonical current path and validated executable identity. `current_exe` and current-directory functions can return errors. `temp_dir` has no error result: define a real mounted location or explicit failure policy. No home directory service was established. |
| P04 | `process::exit` never returns and carries out defined runtime cleanup | `sys_exit_to_loader` chooses HBC reload, reset, or power-off; ignores status for C compatibility | Adapter/service gap. Document the lack of a parent status channel; do not claim status delivery. Define concurrent exit arbitration and cleanup. Do not use this reset/power-off path as recovery after a critical firmware-write failure. |
| P05 | `process::abort` terminates without Rust stack destructors; panic behavior matches panic abort | Hardware trap/backend abort path needs selection; the current target selects abort panics | Adapter needed. Define a diagnostic terminal path and verify it on hardware. Do not use fake success or an automatic destructive recovery action. `catch_unwind` cannot recover from abort panics. Unwinding/backtraces are not claimed. |
| P06 | `Command`/child waiting and pipes refer to real child processes and transports | No process creation/wait or pipe service was found | Unsupported candidates. Return `Unsupported` for process creation and pipe operations until real services exist. Launching a Wii title or DOL does not by itself provide a Rust child-process contract. |
| P07 | `HashMap` random seeding and the selected Rust random API use valid random data | No accepted entropy service was found in the inspected runtime/service crates | Service gap and compile blocker. Define and validate an entropy source before implementing `fill_bytes`. Timebase values, newlib PRNG state, fixed seeds, and unique console data alone are not evidence of a suitable entropy source. An infallible random entry point needs an explicit failure policy. |
| P08 | Floating-point library operations and memory builtins resolve to real implementations | Memory builtins compile; `sys/cmath.rs` declares math symbols | Link evidence needed. Check the selected compiler-builtins/libm features and all required symbols; verify representative numeric results independently. Do not rely on unresolved libogc C symbols. |
| P09 | Backtrace, personality, platform version, and environment constants reflect the target | No accepted Wii unwind/backtrace service; CPU/ABI are known | Unsupported candidates for unwind/backtrace where the API permits it. Keep panic abort and expose a truthful unavailable backtrace status. Set Wii architecture/OS constants and platform selection explicitly; dynamic platform version lookup is not currently required by an inspected Wii service. |
| P10 | Dependencies select code that matches Wii services | Most current runtime crates are `no_std`; `xtask` is host tooling | Integration gap. Audit dependency cfgs and native links after selecting the backend. A Linux PowerPC build or the presence of `std` does not establish Wii compatibility. Pure collections, formatting, parsing, and iterator code can reuse shared Rust implementations after platform integration. |

## Backend implementation order

1. Pin Rust and matching library source. Add Wii-specific platform selectors,
   error translation, allocator selection, and a reproducible sysroot build.
   Preserve Wii/GameCube separation and the target's actual ABI.
2. Define initialization and memory ownership. Connect the existing startup to
   Rust runtime initialization; make arguments, heap, initial thread, error state,
   and failure reporting valid before dependent services use them.
3. Implement TLS and thread exit, including detach/reclamation. Then implement
   mutexes, condition variables, once, reader/writer locks, and parking through
   the shared scheduler. Resolve pool exhaustion and duration overflow explicitly.
4. Establish valid monotonic and wall clocks and an accepted entropy service.
   Implement fatal failure behavior for the required infallible entry points.
5. Add sockets/DNS and mounted-filesystem adapters. Preserve errors and ownership;
   resolve option queries, timed-connect completion, path prefixes, shared file
   cursors, and persistence. Mark each unsupported operation at its API boundary.
6. Build and link a complete `std` Wii DOL. Inspect unresolved symbols, startup,
   panic features, native dependencies, and every selected fallback implementation.
7. Run contract probes on hardware before enabling `std` in an application port.
   Then inspect each application dependency against the same support boundary.

The Rust backend must reuse the production scheduler, IPC, allocator, networking,
and filesystem engines. Choose a sysroot dependency arrangement that uses the
same `core`/`alloc` instances and has no dependency back from those engines to
`std`. Do not introduce a second scheduler or replacement transport to make a
compile check pass.

## Required acceptance evidence

| Area | Evidence needed to close gaps |
| --- | --- |
| Allocation | Alignment, zeroing, resize contents, exhausted heap, preserved old allocation after failed resize, concurrent allocation, and released storage |
| Threads/TLS | Actual execution, join result publication, TLS isolation/destruction, repeated detached threads with bounded retained resources, stack/creation failure cleanup, and initial-thread access |
| Synchronization | Contention, shared-reader/exclusive-writer behavior, once publication, wake-before-wait, timeout races, mutex reacquisition, and resource exhaustion |
| Time | Independent elapsed-time measurements, no early sleep completion, checked large durations, RTC read failure, and verified epoch/bias conversion |
| Network | Traffic with an external peer, partial I/O, refusal/reset/EOF, nonblocking failure, timeout bounds, failed timed connect, clone lifetimes, concurrent DNS, and close/deinit failures |
| Files | Exact open flags, shared clone cursor, offset limits, directory errors, persisted bytes after reopen/remount, media removal, transport flush failure, and cleanup diagnostics |
| Runtime | Real stdout/stderr sink, missing-sink errors, loader argument lifetime, controlled exit/abort, and link audit for math/runtime symbols |

## Original assessment verification record

The following record applies to the initial assessment commit `c52e22a`.
Use [foundation verification evidence](wii-std-foundation-evidence.txt) for
the later implementation checks.

Use independent expected bytes/results and failure injection at production
boundaries. Hardware tests must call the production APIs and the eventual `std`
adapter. An emulator run or a skipped test is not a real-Wii pass. No hardware
probe or unit test was run for this document. The only execution evidence added
here is the two isolated compile checks described above.

Repository checks also passed: `cargo run --offline -p xtask -- audit-source`
and `cargo run --offline -p xtask -- check-inventory` (3,127 inventory entries).
The source audit still reports three existing tracked keyboard event-queue debt
entries. Cargo warns that the application source belongs to two example targets.
These checks do not establish standard library completeness. No allowances,
inventory entries, target definitions, or production functions were changed.
The documentation references and whitespace checks passed.
