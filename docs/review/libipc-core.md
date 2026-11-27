# IPC and system core parity review

Status: static source review complete for the recorded scope. No build, test, or hardware run was done.

## Scope and source map

Reviewed all Rust production source files in `ogc-ipc`, `ogc-sys`, and `ogc-sysbase`:

- `crates/core/ogc-ipc/src/lib.rs` (lines 1–1978; tests at 1980–2053), `dma_buffer.rs` (1–40), and `poll_guard.rs` (1–68).
- `crates/abi/ogc-sys/src/lib.rs` (1–1029).
- `crates/core/ogc-sysbase/src/lib.rs` (1–633).

The direct IPC C interface is `../../C-sources/libogc/gc/ogc/ipc.h` (lines 36–104) and its implementation is `../../C-sources/libogc/libogc/ipc.c` (lines 55–1327). The IOS version/reload interface is `../../C-sources/libogc/gc/ogc/ios.h` (lines 39–57), implemented by `../../C-sources/libogc/libogc/ios.c` (lines 44–406). For interrupt and startup primitives in `ogc-sys`, relevant public C declarations include `gc/ogc/irq.h` (lines 57–112); exception and runtime call sites are in `libogc/system.c` and the C startup sources. `ogc-sysbase` models newlib `_reent` and `devoptab_t` APIs, but the matching newlib implementation/header is not present in the supplied `C-sources/libogc` tree. The only in-tree evidence found for this interface is its use in `libogc/sys_report.c` (lines 61–112), `console.c` (lines 20, 485–486), and `ogc_sockets/soc_init.c` (lines 65–104).

## Historical mismatch — heap model and capacity

An earlier Rust version used one fixed heap and did not preserve C's 16 heap
slots or requested sizes. The current implementation tracks 16 heaps, allocates
each from the IPC buffer using its requested size, and advances the current
buffer low bound (`../../C-sources/libogc/libogc/ipc.c:721–774`,
`crates/core/ogc-ipc/src/lib.rs:1804–1855`). The original model/capacity
finding is superseded by the 2026-10-02 implementation note below. A custom
allocator remains an implementation difference; the current note does not
claim identical fragmentation behavior.

### Historical blocker: IPC buffer bounds were not available

This finding is superseded by the 2026-11-26 current-source follow-up below.
The active linker script now defines `__ipcbufferLo` and `__ipcbufferHi`,
`ogc-ipc` reads those bounds, and its Wii path applies nonzero loader overrides
from `EnvRvlLowMem`. The remaining evidence gap is direct execution with
nonzero override values on Wii; see the current follow-up for verification
limits.

## Differences and additions

- Rust uses a Rust-owned request queue and static request storage, while C allocates fixed-size requests through LWP workspace. Both use 16 queue entries and the 64-byte request ABI; Rust asserts the request size and defines the request fields at `crates/core/ogc-ipc/src/lib.rs:42–59, 600–720, 1980–1996`. C defines the request at `ipc.c:72–115` and queues/sends it at `ipc.c:254–325`. The source review found matching request command values, offsets, queue capacity, and mailbox register masks. Rust's queue locking and ownership are different implementation details.
- C invokes the completion callback from its IPC reply interrupt handler (`ipc.c:327–407`). Rust queues callbacks to the LWP deferred worker (`crates/core/ogc-ipc/src/lib.rs:1386–1403, 1723–1753`). This changes callback execution context and timing. Rust documents this as a deliberate interrupt-safety choice; this review did not classify it as a defect.
- Rust validates C strings against `IPC_MAXPATH_LEN` and copies open paths into aligned storage (`crates/core/ogc-ipc/src/lib.rs:47–55, 1756–1777, 1904–1910`). C checks null only for synchronous open and flushes at most `IPC_MAXPATH_LEN + 1` bytes (`ipc.c:844–867`); asynchronous C open does not check null (`ipc.c:869–887`). Rust's bounded path contract is a safety/API addition and is stricter for oversized inputs.
- Rust stages partial or unaligned DMA buffers to protect neighboring cache data (`crates/core/ogc-ipc/src/dma_buffer.rs:1–7`, `lib.rs:444–504, 1780–1902`). C passes supplied buffers directly and flushes/invalidates their cache ranges (`ipc.c:327–383, 1051–1186, 1242–1327`). This is a Rust-only safety behavior.
- Rust exposes typed wrappers (`IoctlBuffers`, `IoctlvEntry`, `HeapId`, `FileDescriptor`) and the `open_file`/`open_file_async` free functions (`crates/core/ogc-ipc/src/lib.rs:84–240, 1904–1978`). C exposes the raw `IOS_*` API, buffer accessors, and variadic `IOS_IoctlvFormat[Async]` (`ipc.h:69–104`). The Rust crate does not implement a variadic format parser; it now provides C ABI counterparts for the four IPC buffer get/set functions described in the handoff below.
- `ogc-sysbase` adds a no-std device-file helper, mounted-device registry, and stdin replacement helpers (`crates/core/ogc-sysbase/src/lib.rs:217–437, 440–563`). These helpers are not part of the IPC C API. The source set does not include the newlib `devoptab_t` definition or `AddDevice`/`FindDevice`/`RemoveDevice` implementation needed to verify structure offsets and registration semantics.
- `ogc-sys` includes startup assembly, HBC argument capture, memory routines, error wrappers, and exception-vector installation (`crates/abi/ogc-sys/src/lib.rs:98–366, 389–475, 490–589, 593–911`). This crate has no direct single C header/implementation pair. Some behavior is Rust-runtime-specific. `Bool`, float aliases, status conversion helpers, HBC signature helpers, debug/test helpers, and configurable dispatch callbacks are Rust additions, not C ABI declarations.

## Areas checked

- IPC public constants and `Ioctlv` layout: compared `ipc.h:36–67` to `ogc-ipc/src/lib.rs:21–40, 82–87`; values and pointer/length field order agree for 32-bit PowerPC.
- IPC request layout, queueing, request submission, acknowledgement/reply handling: compared `ipc.c:72–115, 254–447, 686–719` to `ogc-ipc/src/lib.rs:600–720, 1200–1442, 1673–1753`.
- Open, close, read, write, seek, ioctl, ioctlv, and reboot requests: compared `ipc.c:844–1187, 1242–1327` and declarations `ipc.h:82–104` to Rust `ogc-ipc/src/lib.rs:230–1100, 1756–1902`.
- Heap behavior and buffer bounds: compared `ipc.c:721–803` to Rust `ogc-ipc/src/lib.rs:164–225, 1513–1671`.
- Poll and interrupt guards: reviewed Rust `poll_guard.rs:1–24`, `ogc-ipc/src/lib.rs:1115–1138`, and the C interrupt sequence at `ipc.c:327–447`.
- IOS version and reload routines: source files were inventoried (`ios.h:39–57`, `ios.c:44–406`); no corresponding public functions are implemented by these three Rust crates.
- `ogc-sysbase` public types, callbacks, and methods: inventoried all exported declarations at `crates/core/ogc-sysbase/src/lib.rs:16–159, 217–563`. Full ABI comparison is limited by the missing newlib header and implementation noted above.
- `ogc-sys` public helpers and assembly: inventoried all public functions/types and exported symbols at `crates/abi/ogc-sys/src/lib.rs:17–48, 368–478, 592–911`. Direct C parity could not be established for the custom Rust startup and exception-vector implementation from this source set.

## Uncertain or unchecked

- No hardware behavior was checked. In particular, IOS mailbox response ordering, DMA cache effects, callback timing under load, relaunch interactions, and exception-vector behavior remain unverified at runtime.
- The exact newlib ABI for `_reent`, `devoptab_t`, `DIR_ITER`, and filesystem callbacks is uncertain because the corresponding headers and implementation are external to this C source tree. Rust field/callback layouts at `ogc-sysbase/src/lib.rs:26–160` were inventoried, not proven against that external ABI.
- No equivalent C implementation was found for the Rust custom IRQ/decrementer exception vector stubs, standalone `_start_rust` entry, or HBC command-line sheltering. These areas were inventoried at `ogc-sys/src/lib.rs:98–366, 490–589, 757–911`, but are not claimed as parity-checked.
- The C variadic ioctlv format parser (`ipc.c:448–684`) and completion cleanup path (`ipc.c:214–251`) have no Rust equivalent and were not independently reimplemented or behavior-compared.
- Rust `ogc-sys` helpers outside IPC/system ABI glue, including byte-copy routines and generic status conversion, were source-inventoried but not exhaustively compared with external compiler-runtime/newlib implementations. No claim is made about their full cross-platform parity.
- Rust test modules were inventoried but not executed (`ogc-ipc/src/lib.rs:1980–2053`, `dma_buffer.rs:9–40`, `poll_guard.rs:26–68`, `ogc-sys/src/lib.rs:912–1029`). No compile, test, or static-analysis command was run, as requested.

## 2026-10-02 IPC heap-model implementation

The heap-model finding and its linker-bound blocker are addressed in the current source.

- The runtime now uses the Wii IPC arena symbols `__ipcbufferLo = 0x933e0000` and `__ipcbufferHi = 0x93400000`, matching the devkitPPC Wii linker layout used by current libogc. `IPC_GetBufferLo/Hi`-style state is tracked centrally and heap creation advances the current low bound with 32-byte alignment.
- The default request heap is 4096 bytes, matching C `IPC_HEAP_SIZE`.
- Sixteen public heap slots are tracked. `HeapId::create` allocates a distinct heap and returns `IPC_ENOHEAP` after all public slots are occupied; `alloc`/`free` operate on the selected heap instead of accepting only heap zero.
- Rust's DMA-staging allocator remains an internal extra heap carved from the high end of the IPC arena so unaligned DMA safety does not consume a public C heap slot.
- Temporary ISFS async callback contexts now allocate from the shared default IPC heap instead of creating a new heap per operation. USB likewise caches one descriptor heap, matching C's single `hId` creation, so the new multi-heap semantics do not leak public heap slots through existing callers.

The custom Rust heap allocator is not libogc's `__lwp_heap_*` implementation, so fragmentation behavior is not claimed bit-for-bit identical. Heap-count, requested-size, default-size, buffer-bound, allocation/free-by-id, and public slot-exhaustion semantics now follow the C model. Hardware/runtime validation remains pending.

## IPC buffer accessor API handoff

Implementer: `worker_a`. Status: fixed; needs independent static review.

Rust now exports `IPC_GetBufferLo`, `IPC_GetBufferHi`, `IPC_SetBufferLo`, and `IPC_SetBufferHi` with C-compatible names and pointer signatures. The exported accessors use the existing centrally tracked current bounds. Setter validation matches C: the low bound cannot move below the original IPC buffer low address, and the high bound cannot move above its original high address. Regression coverage checks all four C ABI signatures and the accepted/rejected low/high boundary cases. The variadic ioctlv format APIs remain outside this change.

Validation: `cargo check -p ogc-ipc --tests --target powerpc-unknown-linux-gnu` passed; target test code was compiled but not run. `nm -g` confirmed all four accessor symbols are global text exports. `git diff --check` passed. No hardware verification was performed.

## GameCube build fix handoff — ACR IRQ cfg

Implementer: `worker_c`. Status: fixed; needs independent review.

The GameCube IRQ API omits `IRQ_PI_ACR` and `IM_PI_ACR`, matching C's `interrupt.h` where these declarations exist only under `__wii__` (`../../C-sources/libogc/gc/tuxedo/interrupt.h:47-52,84-88`). `ogc-ipc` previously imported and used those names for every PowerPC build. Rust now compiles its ACR import, handler, state, and registration only when `cfg(gamecube)` is not set. The GameCube IPC path therefore does not install a Wii-only ACR interrupt handler.

Validation: nightly `cargo check -p ogc-ipc` passed for both `targets/powerpc-nintendo-gamecube.json` and `targets/powerpc-nintendo-none.json` with `-Z build-std=core -Z json-target-spec`. The GameCube build no longer reports unused IPC ACR imports/state/handler; both target builds still report existing `unexpected_cfgs` warnings because target rustflags do not register `cfg(gamecube)`. `git diff --check` passed. `cargo fmt -p ogc-ipc -- --check` reports pre-existing formatting differences elsewhere in `lib.rs`, outside this change. No tests were run and no hardware verification was performed.

## Independent re-review — exported IPC accessors and Wii ACR gate

**Result: PASSED for the bounded fixes; `ogc-ipc` remains open.** Reviewer: `/root/reviewer_f`. Implementation group: `8781602` (exported buffer accessors and Wii-only ACR handling).

The accessor signatures and bounds match C, and the ACR import, handler, state, and registration are gated out of the GameCube profile, matching C's Wii-only source guard. The bounded implementation fixes pass static comparison. Other IPC behavior remains open.

### Remaining heap and variadic API differences

The previous heap-count/default-size concern is stale: Rust now supports 16 heap slots and the corresponding default request size (`ogc-ipc/src/lib.rs:60, 182–206, 1639–1672`), matching C's public heap operations (`C-sources/libogc/libogc/ipc.c:55–57, 721–774`). Allocator fragmentation behavior still differs. C scans its free list and allocates from the high end of a split block (`lwp_heap.c:62–95`); Rust scans blocks by ascending offset and allocates from the low end (`ogc-ipc/src/lib.rs:1688–1735`). For a 25-byte request, C's page-size formula computes 72 bytes while Rust's prior aligned-header calculation consumed 64 bytes (`lwp_heap.c:51–60`; Rust `lib.rs:1694–1697`). Rust's 32-byte block metadata then requires a 96-byte block to represent the request. Returned addresses and allocation success under fragmentation can therefore differ. Rust also reserves a 65,600-byte staging heap (`lib.rs:1801–1831`), reducing space available to public heaps; this is an intentional Rust-only addition with an observable capacity effect.

C declares and defines variadic `IOS_IoctlvFormat` and `IOS_IoctlvFormatAsync` (`gc/ogc/ipc.h:100–101`; `libogc/ipc.c:1189–1240`). Rust exposes a typed `FileDescriptor::ioctlv` but no equivalent C-named variadic symbols (`ogc-ipc/src/lib.rs:152–167, 556`). C callers cannot link against the legacy functions.

`cargo check -p ogc-ipc --tests --target powerpc-unknown-linux-gnu` passed; it compiled target tests, but tests were not run. No hardware verification was performed.

## Independent re-review — sysbase devoptab and target ABI

Reviewer: `/root/reviewer_f`. The installed devkitPPC headers confirm that several Rust callback and record declarations do not match the target ABI. The target `sys/iosupport.h` declares `seek_r` and `ftruncate_r` offsets as 64-bit `off_t` (`:43, 59`; target `machine/_types.h:6–8`), while Rust `SeekFn` and `FtruncateFn` use 32-bit `i32` (`ogc-sysbase/src/lib.rs:148–149, 180–181`). The target `stat` uses 16-bit `ino_t`, while Rust `Stat.st_ino` is `u32` (`sys/stat.h:27–36`, `_types.h:70–76`; Rust `lib.rs:85–100`). Target `statvfs` uses 32-bit `unsigned long` for `f_bsize` and `f_frsize`; Rust uses `u64`, shifting later fields (`sys/statvfs.h:14–25`; Rust `lib.rs:103–118`). Target `timeval.tv_sec` uses 64-bit `time_t`, while Rust `TimeVal` uses two `i32` fields (`sys/_types.h:184–189`; Rust `lib.rs:66–71`). The target declares `devoptab_list` with `STD_MAX = 35`; Rust exports a three-entry array (`sys/iosupport.h:16–20, 78`; Rust `lib.rs:637–639`).

Rust's `add_device`, `find_device`, and `remove_device` operate only on its private 16-slot registry, which `DeviceFile` resolves directly (`ogc-sysbase/src/lib.rs:237–290, 538–629`). They do not call C/newlib `AddDevice`. C ISO9660 registration calls native `AddDevice` (`C-sources/libogc/libiso9660/iso9660.c:981–993`), while Rust ISO9660 calls only `sysbase::add_device` (`crates/media/iso9660/src/lib.rs:792–829`). ES separately adds a native registration path; that does not change the shared sysbase helper. The C reference tree does not contain newlib's `AddDevice` implementation, so internal registry behavior beyond installed target declarations is not asserted.

`DirIter` layout and `_reent` errno offset match the target headers. Rust-only `DeviceFile` routing and unqualified-path search are API additions; they do not replace native path visibility. `cargo check -p ogc-sysbase --tests --target powerpc-unknown-linux-gnu` passed and compiled tests; no tests were present or run. No hardware verification was performed.
## Fix pass: newlib `ftruncate_r` offset width

**BUG FIX.** Implementer: `worker_c`. Independent re-review: needed. The shared Rust `FtruncateFn` now takes a 64-bit `i64` length, matching devkitPPC `off_t`. `fat_ftruncate_r` uses checked conversion to the FAT implementation's 32-bit `u32` length and rejects negative or out-of-range values rather than truncating them. The `SeekFn` width corrected in the prior ES task also required adapting FAT's `fat_seek_r`; it now accepts full `off_t` input and checks against FAT's `u32` position range. Added callback signature coverage and boundary tests, including values above `i32::MAX`.

The installed `${DEVKITPRO}/devkitPPC/powerpc-eabi/include/sys/iosupport.h` declares the callback argument as `off_t`; the target `machine/_types.h` defines `off_t` through a 64-bit `_off_t`. The target C compile-time assertion, `cargo check -p ogc-sysbase -p ogc-fat --target powerpc-unknown-linux-gnu --tests`, and scoped `git diff --check` passed. Test code compiled but tests did not run. `cargo fmt -p ogc-sysbase -p ogc-fat -- --check` reports pre-existing crate-level `compile_error!` formatting differences; added test formatting matches rustfmt. No hardware verification was performed. Other stat, statvfs, timeval, and registry findings remain separate.

## Fix pass: target `statvfs` layout

**BUG FIX.** Implementer: `worker_c`. Independent re-review: needed. Rust `StatVfs` now follows the installed devkitPPC field widths: `f_bsize` and `f_frsize` are `u32`; `f_blocks`, `f_bfree`, and `f_bavail` are `u64`; and `f_files`, `f_ffree`, `f_favail`, `f_fsid`, `f_flag`, and `f_namemax` are `u32`. The FAT statvfs producer now writes those narrowed fields with matching types.

Target layout assertions cover all 11 offsets and the 56-byte structure size. `powerpc-eabi-gcc -std=gnu11 -fsyntax-only` passed matching static assertions against `sys/statvfs.h`; `cargo check -p ogc-sysbase -p ogc-fat --target powerpc-unknown-linux-gnu --tests` passed and compiled test code; scoped diff check passed. Tests did not run. No hardware verification was performed. The timeval, inode, devoptab capacity, and private registry findings remain separate.

## Fix pass: ISO9660 `statvfs` producer widths

**BUG FIX.** Implementer: `worker_c`. Independent re-review: needed. ISO9660 now assigns its 0x800-byte block sizes as `u32` and stores the `statvfs` flags as `u32`, matching the corrected target `StatVfs` layout and C's values. Added a focused target test for the block size, flags, and maximum name length.

`cargo check -p iso9660 --target powerpc-unknown-linux-gnu --tests` and scoped `git diff --check` passed. Test code compiled but tests did not run. No hardware verification was performed.

## Fix pass: target `timeval` layout

**BUG FIX.** Implementer: `worker_c`. Independent re-review: needed. `TimeVal.tv_sec` is now `i64` to match the installed devkitPPC 64-bit `time_t`; `tv_usec` remains `i32`, matching the target 32-bit `suseconds_t`. Added target assertions for field offsets and total size.

The installed `_timeval.h` declares `time_t` and `suseconds_t`; `_types.h` defines `time_t` as 64-bit and `suseconds_t` as `long`. `powerpc-eabi-gcc` accepted C assertions for widths and offsets. `cargo check -p ogc-sysbase --target powerpc-unknown-linux-gnu --tests` passed and compiled the target test; tests did not run. Scoped `git diff --check` passed. No hardware verification was performed.

## Fix pass: IPC heap split direction and request accounting

**BUG FIX.** Implementer: `worker_b`. Independent re-review: needed. Rust now walks its free list in list order and allocates from the high end of a split free block. Whole-block allocations remove that node from the list; frees merge physically adjacent free blocks while keeping the surviving node in its existing list position, or insert an isolated block at the list head, matching the C free-list behavior. Regression coverage checks high-end returned addresses, the consumed Rust block size, and first-fit selection after fragmentation.

The earlier `96 bytes` note is stale. With 32-byte pages and the target's 8-byte used-block overhead, C consumes 72 bytes for a 25-byte request (`lwp_heap.c:51–60`). The current Rust `c_heap_block_size()` computes that same 72-byte footprint. The separate staging-heap and variadic ioctlv API review items are independent of this fixed allocation-size issue.

`cargo check -p ogc-ipc --tests --target powerpc-unknown-linux-gnu` passed and compiled the target regression tests; tests did not run. No hardware verification was performed.

## Raw IOS IOCTLV address handoff

Implementer: `/root`. Independent static re-review is needed; the review thread handles it. `FileDescriptor::ioctlv_raw` submits aligned IOS vectors while preserving a zero-length vector's address as protocol data. It validates vector counts, descriptor storage, and non-empty DMA buffers, translates non-empty buffers to the IOS request address, and restores caller pointers after completion. Regression tests cover count bounds, descriptor alignment, zero-length address preservation, and DMA address translation.

The change does not implement the C variadic `IOS_IoctlvFormat` functions and does not change the typed slice-based `ioctlv` path. `cargo check -p ogc-ipc --tests --target powerpc-unknown-linux-gnu` compiled the target tests. The repository's Wii DOL build also passed. Tests were not executed, and no hardware verification was performed.

## Independent re-reviews — target devoptab layouts and IPC heap split

The bounded target ABI fixes pass static review. Reviewer: `/root`. Implementer: `worker_c`.

The `FtruncateFn` fix in `d41685e` uses the 64-bit target `off_t` declared by devkitPPC `sys/iosupport.h`; the FAT callback checks conversion to its `u32` backend range and rejects negative or out-of-range lengths. The adapted seek callback also accepts full `off_t` and checks its backend range. The `StatVfs` fix in `a90846e` and `TimeVal` fix in `6d253b7` match the installed target headers and layout assertions. Handoff checks compiled the PowerPC test code and C layout assertions; tests did not run. The target `Stat` inode width, devoptab capacity, and private registry findings remain open.

The IPC heap split fix in `c84bd6a` passes for list-order traversal, high-end split allocation, whole-block unlink, and coalescing/list order. Reviewer: `worker_a`; implementer: `worker_b`. C computes a 72-byte block for a 25-byte request, and current Rust now computes the same 72-byte footprint. The variadic `IOS_IoctlvFormat` symbols and the separate staging-heap capacity review items remain independent. `worker_a` did not review the raw IOCTLV changes implemented by `/root`.

Neither reviewer ran tests or package checks. The implementer handoff checks and limits are recorded above. No hardware verification was performed.

## 2026-10-05 fix handoff: native devoptab registry integration

Implementer: `/root`. `ogc-sysbase` now implements and exports `AddDevice`, `FindDevice`, `RemoveDevice`, `GetDeviceOpTab`, `setDefaultDevice`, and the 35-slot `devoptab_list` in Rust; it no longer imports the newlib device-registry functions. The Rust table uses the reference `stdnull` sentinel, reuses the first `stdnull` or same-name slot, restores `stdnull` on removal, applies the same device-name and optional single-digit suffix lookup rule, and uses the selected default device for unqualified paths. Rust rejects `STD_MAX` as a default-device index instead of preserving the reference out-of-range bug; this is an intentional memory-safety deviation. Earlier target checks applied to the preceding native-integration handoff; this Rust-owned replacement requires fresh target compilation and final-link validation. Independent static re-review is also needed. The target `Stat` inode-width finding remains open. No hardware verification was performed.

## 2026-10-05 fix handoff: IPC allocation block size

Implementer: `/root`. Rust's IPC heap now allocates the C-sized block (`c_heap_block_size`) instead of rounding that size to 32 bytes to fit an oversized Rust header. The header now uses the C used-block overhead, the allocator returns the next 32-byte boundary, and it stores the alignment delta immediately before the user pointer so free can recover the block start. The existing regression now asserts that a 25-byte request consumes a 72-byte block, returns an aligned pointer, splits from the high end, and can be freed and reused by the surrounding allocator tests. `cargo check -p ogc-ipc --tests --target powerpc-unknown-linux-gnu` passed; tests were compiled but not run. Independent static re-review is needed. Fragmentation edge cases, staging-heap capacity, and variadic IOCTLV remain open. No hardware verification was performed.

## 2026-10-05 fix handoff: devoptab slot capacity

Implementer: `/root`. The Rust-owned exported `devoptab_list` has 35 pointer slots, matching devkitPPC `STD_MAX = 35`, so registration no longer depends on a native libsysbase device table or the earlier private 16-device cap. The current Rust-owned replacement requires a fresh target compile because the earlier check predates the removal of the native registry calls. Independent static re-review is needed. No hardware verification was performed.


## 2026-10-05 Rust-owned devoptab port

Implementer: `/root`. The `ogc-sysbase` production path no longer imports `AddDevice`, `FindDevice`, or `RemoveDevice`. Rust now owns the exported 35-slot `devoptab_list` and implements the newlib-compatible registration/lookup/removal entry points plus `GetDeviceOpTab` and `setDefaultDevice`. `DeviceFile` resolves the same table, so Rust filesystem registration and C-ABI/newlib-style lookup share one source of truth. A target regression registers a device through exported `AddDevice`, resolves it through `FindDevice`/`GetDeviceOpTab`, removes it, and verifies it is no longer found. Independent target compilation and final-link validation are still required.

## Independent re-review — Rust-owned devoptab port (2026-10-05)

Reviewer: `worker_e`. The replacement implementation has one registry for both Rust and C-ABI lookup: `devoptab_list` has 35 slots and starts with the `stdnull` descriptor (`crates/core/ogc-sysbase/src/lib.rs:274-303,666-668`). `AddDevice` reuses the first `stdnull` slot or replaces a same-name slot; `FindDevice` returns the selected default for unqualified paths; `RemoveDevice` restores the `stdnull` entry; and `GetDeviceOpTab` returns the selected table entry (`:571-649`). The installed devkitPPC `sys/iosupport.h` confirms `STD_MAX = 35` and the exposed `devoptab_t` layout (`:14-20, 32-77`). Checked-in libogc call sites register and remove devices through the C API (ISO9660 `../../C-sources/libogc/libogc/libiso9660/iso9660.c:963-1010`; ES `../../C-sources/libogc/libogc/es.c:1288-1292`; stdio table updates `../../C-sources/libogc/libogc/console.c:485-486`). The target regression exercises registration, lookup, default-device resolution, removal, and absence (`ogc-sysbase/src/lib.rs:730-758`).

**Result: NEEDS CHANGES for the documented C registry behavior.** The 35-slot capacity and bounded exported entry points are present, but the path-name and removal differences below are observable mismatches.

The checked-in C tree does not include the libsysbase implementation. I compared the registry functions with installed `libsysbase.a` member `iosupport.o`. C `FindDevice` compares the exact path prefix before `:` against each registered name; it does not infer an optional numeric suffix. Rust `name_matches_path` accepts a one-digit suffix (`crates/core/ogc-sysbase/src/lib.rs:571-584`), so e.g. Rust can resolve `usb1:/...` through a device named `usb`, while the C function cannot. C `RemoveDevice` clears the selected slot to NULL and returns 0 (`libsysbase.a` disassembly); Rust restores a nonnull `stdnull` pointer (`:608-618`). This also means the Rust-owned table's free-slot sentinel behavior differs from the C table after removal. C `setDefaultDevice` accepts index 35 due its inclusive range check; Rust rejects it (`:650-662`), which is an accepted memory-safety deviation documented in the handoff. Do not generalize beyond the installed target library version.

The final application link has not been checked, so this source review does not prove that the Rust definitions satisfy all final C/newlib references without collision.

The previous target `Stat.st_ino` width finding is stale for the currently installed devkitPPC headers: `machine/_types.h:13-14` defines `__ino_t` as `__uint32_t`, and `sys/stat.h:25-26` uses `ino_t`; Rust `Stat.st_ino` is `u32` (`crates/core/ogc-sysbase/src/lib.rs:99-103`). This removes that specific width mismatch. The target `setDefaultDevice(35)` behavior is an accepted safety deviation. Exact table initialization and other undocumented cases need further comparison. Other existing scope notes, including full target ABI and final-link checks, remain applicable.

Validation: `cargo check -p ogc-sysbase --target powerpc-unknown-linux-gnu --lib` passed. The target test build failed before compiling tests because `assert_eq!` at `crates/core/ogc-sysbase/src/lib.rs:742,746` requires `Devoptab: Debug`; no tests ran. This is a test-compilation blocker, not a production registry finding. No hardware verification was performed.

## Independent re-review — exact devoptab lookup and removal (2026-10-05)

**Result: PASS for commit `5cbfd54`.** Reviewer: `worker_f`. Implementer: `worker_e`. The revised `name_matches_path` requires the complete registered name followed immediately by `:` (`crates/core/ogc-sysbase/src/lib.rs:571-579`); it no longer accepts an extra numeric suffix. In `5cbfd54`, the target registry regression registers a device, resolves `parity:/file`, removes it, confirms lookup fails, then verifies the freed slot can be reused (`:692-758`); a separate matcher test rejects `usb1:/apps` for the name `usb` and accepts it for `usb1`. `RemoveDevice` clears the slot to null (`:608-615`) and `AddDevice` stores into the first free slot (`:619-640`). A later ES regression uses the actual bare `es` name and its normal content path.

This matches installed devkitPPC `libsysbase.a` member `libsysbase_libsysbase_a-iosupport.o`: `FindDevice` compares the path prefix before `:` with each registered name; `RemoveDevice` writes NULL to the selected table entry; and `AddDevice` selects the first NULL, exact-name, or `stdnull` entry. The checked-in libogc C tree does not contain this newlib implementation, so these internal semantics are based on the installed target library. Checked-in callers use the same colon-terminated lookup/removal form (ES `../../C-sources/libogc/libogc/es.c:1288-1292`; ISO9660 `../../C-sources/libogc/libiso9660/iso9660.c:963-1010`).

`cargo check -p ogc-sysbase --tests --target powerpc-unknown-linux-gnu` passed and type-checked target test code; tests did not run. This pass covers only exact path matching, null-on-removal, and first-free-slot reuse. Other `ogc-sysbase` ABI/layout and final-link findings remain open. No hardware verification was performed.

## 2026-10-05 `ogc-sys` / `ogc-sysbase` completion review

`ogc-sys` and `ogc-sysbase` are now `reviewed` for their own recorded scope. The remaining staging-heap and variadic IOS API blockers belong to `ogc-ipc`, not `ogc-sys`. `ogc-sysbase` now owns the devoptab registration/lookup/removal/default symbols and 35-slot table in Rust, so no libogc C implementation is required at link time. Current target type/layout checks match the inspected devkitPPC ABI. Rejecting an out-of-range default-device index instead of permitting an out-of-bounds table access is an accepted safety deviation.

## 2026-10-06 fix handoff: staging capacity and variadic IOCTLV ABI

Implementer: `/root`. The two named `ogc-ipc` source blockers are closed in production code. DMA safety staging no longer carves a private 65,600-byte heap from the high end of the public IPC arena. Temporary partial or unaligned DMA buffers now allocate from the same 32-byte-aligned dynamic LWP workspace used by the runtime, through a small shared allocator facade in `ogc-lwp-driver`/`ogc-lwp-sys`. This preserves all 16 public IPC heap slots and the complete IPC buffer capacity. Returning `IPC_ENOMEM` when safety staging cannot be allocated remains a narrow Rust safety divergence from C's direct use of potentially cache-unsafe caller buffers.

The legacy `IOS_IoctlvFormat` and `IOS_IoctlvFormatAsync` symbols are now Rust-owned PowerPC exports. Small assembly entry shims preserve the 32-bit EABI variadic register/overflow arguments and pass them to a Rust parser. The parser implements the C `b`, `h`, `i`, `q`, `d`, `s`, and `:` format grammar, uses the requested IOS heap for C-style scalar/string/vector-array allocations, stages only partial or unaligned direct data vectors, copies output scalar/staged data back on completed requests, and routes synchronous/asynchronous operations through the existing Rust-owned IPC request engine. Invalid null scalar/string pointers return `IPC_EINVAL` rather than reproducing C undefined behavior. Async parse/submission failures release Rust allocations without invoking the user callback; accepted requests invoke it only after final IOS completion.

Independent target compilation, static re-review, final-link symbol verification, and Dolphin/hardware runtime checks remain required before `ogc-ipc` can move to `reviewed`.

## 2026-10-07 stale variadic-API note correction

The earlier statement that `IOS_IoctlvFormat` and `IOS_IoctlvFormatAsync` have no Rust equivalent is stale for the current tree. `ogc-ipc` now exports both PowerPC C ABI symbols through assembly entry shims. The shims capture the PowerPC EABI register/stack variadic state and route it through the Rust-owned format parser and IOS request path. Current static review therefore treats the missing-symbol/API finding as closed. PowerPC EABI execution, asynchronous copy-back, and callback timing still require target/runtime validation.


## 2026-10-08 BUG FIX: heap lock preemption during USB completion

The user tested HBC DOL `a962d958c1acee48635b5e1fb3eb31ab5ce48460f27dcf6fc51d9842901fee51` on Wii. The loading circle stopped and HOME did not respond. This proves a full execution stall, but does not identify its exact program counter.

Source comparison found a reachable priority deadlock: `heap_alloc` and `heap_free` held an atomic spin lock with interrupts enabled. The LWP completion worker has priority 126; the HBC storage worker has priority 62. If an IPC interrupt schedules the completion worker while storage holds the heap lock, the completion worker can request that lock. Yielding the higher-priority waiter does not let the lower-priority owner run. C `lwp_heap.c::__lwp_heap_allocate` and `__lwp_heap_free` disable interrupts around these operations.

The bounded fix masks interrupts before acquiring the heap lock, releases the lock before restoring the previous interrupt state, and restores interrupts before yielding if an acquisition fails. Heap sections do not sleep or submit IOS operations. Other IPC synchronization behavior is unchanged. Shared production ordering tests cover acquisition, failed acquisition, release, and preservation of the previous mask. Both tests ran on the host. PowerPC test code compiled. The HBC release build passed without warnings. Source-structure audit passed with the existing keyboard event queue debt.

This is a confirmed source defect and a candidate cause of the reported Wii stall. The new DOL still needs a Wii check for startup completion, working HOME input, catalog reads, and USB removal. Independent source re-review remains pending; the implementer has not approved the fix.

The rebuilt DOL was copied to `[device volume]/apps/hbc-rs/boot.dol` and compared byte for byte with the local output. Both SHA-256 hashes are `1a11ad1ace6fae2c0e130b0037f811088da43a6d502fe112086b4aacc94ad1e9`. The completeness audit remains blocked by the existing stale evidence entry for `aesnd-sys::get_dsp_process_time` at line 914; no completeness pass is claimed.


### 2026-10-08 request queue lock follow-up

The user reports a selected DOL launch that leaves the HBC image frozen. The app name and exact last program counter remain unknown. Source tracing found that `submit_request` and `submit_async_request` also acquired `IPC_LOCK` while preemption was enabled. As with the earlier heap-lock defect, a deferred completion can preempt the owner and wait above it. Both sections only initialize the IPC client or submit a queued request; they do not wait for an IOS reply while holding this lock. They now use the same shared interrupt-masked lock as heap operations. The synchronous reply wait keeps its separate LWP mutex. The duplicate unmasked spin acquisition and optional mask state were removed.

The two production-shared mask/ownership ordering tests passed. PowerPC IPC test code compiled. The release HBC build completed without warnings. Wii launch validation remains pending. This is a confirmed reachable locking defect, but the specific user's launch freeze has not been located on hardware. Independent re-review remains pending.

Final combined DOL: release build passed without warnings; copied to `[device volume]/apps/hbc-rs/boot.dol`, flushed, and verified byte for byte. Local and CARROT SHA-256: `f7047b3d75b52ef1e661891a2d3e94f4d2338c36960b3e461c0808b6e4dccca3`. The source audit passed with existing keyboard debt; the SDK completeness audit remains blocked by stale AESND evidence at line 914. Network save, wider keyboard output, and selected-app launch require Wii verification.

## Independent IPC re-review — 2026-10-15

Reviewer: `/root/review_ipc`. The bounded heap lock-order fix passed source
review, but the full IPC request path has a confirmed asynchronous result-loss
case. If `deferred_call_from_interrupt` rejects a reply callback because the
shared LWP deferred queue is full, IPC decrements the pending count and frees
the request without calling the user callback
(`crates/core/ogc-ipc/src/lib.rs:1550-1569`). The queue has capacity 32 and is
shared by other producers. C calls the callback from the IOS reply handler
(`C-sources/libogc/libogc/ipc.c:385-387`). Accepted requests must always report
their final result; reserve completion capacity at submission or add a safe
fallback that preserves callback contract and IRQ rules.

The reviewer found the heap split, interrupt mask, synchronous IOCTLV, and
variadic ABI parsing consistent in the inspected paths. No tests or hardware
checks were run. Keep `ogc-ipc` blocked until completion delivery is fixed and
independently reviewed.

## 2026-10-28 Async IPC completion reservation fix

The 2026-10-15 result-loss finding now has a scoped production fix. A callback
request reserves one tokenized slot in the LWP interrupt-deferred queue before
IPC accepts the request. Unreserved producers cannot consume reserved slots.
If capacity is unavailable, submission returns `IPC_EQUEUEFULL`; if IPC request
queue submission fails, it cancels the exact token. The IOS reply path uses
that token to enqueue the completed request. The deferred worker runs the user
callback and then releases the request. The reservation token uses existing
request padding at byte offset 52; `IpcRequest` remains 64 bytes.

The focused host regression runs the production-shared
`enqueue_reply`/`deliver_reply` helpers with the production `DeferredSlots`
source. It reserves one of 32 slots, fills the other 31 with ordinary
callbacks, confirms the ordinary queue is full, delivers an IOS error result
through the reserved path, and checks that the user callback sees that result
while the request remains live and that release occurs after the callback.
`rustc --edition=2021 --test crates/core/ogc-ipc/src/async_completion.rs`
passed all three tests, including the slot accounting tests.

Independent reviewer `/root/bt_event_queue_rereview` passed this scoped fix on
2026-10-28. The review checked the reservation accounting, interrupt-masked
queue access, failure paths, request ABI, C reply behavior, and regression
coverage. The reviewer did not run tests. `cargo xtask check-group core`
passed for the Wii runtime packages after the fix. The registered Dolphin run
`cargo xtask dolphin-validate --probe ipc_async_probe` also passed for the
current dirty worktree: `PASS summary passes=8 fails=0`, DOL SHA-256
`0c560937030e7567476f5d67e59b5c31a96f787e22727091020129d81dd60fbd`.
The Dolphin result contains a separate startup line saying Wiiload report
arguments were missing because the registered runner launched the DOL with no
arguments; the eight IPC probe checks passed, but this run does not validate
the Wiiload report argument path. The inspected report and screenshot are
preserved under `[external tools]/reports/evidence/libogc-rs-ipc-async-dolphin-2026-10-28/`.

This closes only the async reply-loss finding. The broader `ogc-ipc` parity
scope and physical Wii validation remain open. The fix does not claim that
callbacks run on the same stack or in the same context as C; Rust continues to
run user callbacks on the deferred worker to keep user code out of the
interrupt handler.

## 2026-11-26 current-source follow-up — IPC buffer bounds

The earlier blocker that said the Rust runtime lacks IPC buffer bounds is
stale. `targets/powerpc-nintendo-none.ld` now defines `__ipcbufferLo` and
`__ipcbufferHi`, and `ogc-ipc::initialize_ipc_buffer_locked` uses them. The
current heap implementation also scans 16 heap slots and advances the low
bound after heap creation.

A separate default-bound mismatch existed. Wii libogc selects `ios_ipc_lo` and
`ios_ipc_hi` from `EnvRvlLowMem` when those values are nonzero
(`../../C-sources/libogc/gc/tuxedo/mm_env.h:16`,
`../../C-sources/libogc/gc/tuxedo/env.h:145-159`,
`../../C-sources/libogc/libogc/system.c:496-503,536-537`). Rust now reads both
loader values with volatile reads during first IPC buffer initialization and
uses the linker bound for each zero loader value. This matches C's independent
fallback for the low and high bounds. GameCube continues to use linker bounds.

The shared bounds-selection regression first failed because the production
helper was absent, then passed after the helper was added. It checks low-only,
high-only, and zero-value fallbacks. `cargo check -p ogc-ipc --tests --target
powerpc-unknown-linux-gnu` passed. Dolphin 2609a ran `ipc_async_probe`: 11
checks passed, 0 failed, DOL SHA-256
`c35c859fe18144b118a21dc713cd1887c5b3248a99fc81da0ebae6cd5d4d5fed`. The
broader `runtime_probe` first timed out at its 90-second limit after 790 checks.
At 240 seconds it passed 880 checks, including the Rust and C ABI buffer
setter/getter checks; DOL SHA-256
`af672d9cd218ee14d17bd016d78f424aa4e79e95a5cd97797aa3cb959af43761`. Both
Dolphin suites use the linker-default path and do not establish behavior with
nonzero loader overrides. Evidence is under
`target/dolphin-validation/1795711175-448b559fcfa6/` and
`target/dolphin-validation/1795711611-448b559fcfa6/`.

`cargo xtask audit-source`, `cargo fmt --all --check`, and `cargo xtask
check-inventory` passed. The source structure portion of `cargo xtask
audit-stubs` passed; its final completeness gate reports 148 unverified API
entries across the workspace. The loader-bound mismatch is fixed in source.
Direct verification with nonzero override values on Wii, full IPC parity, and
physical Wii validation remain open.

## 2026-11-26 fix — zero-sized heap error result

C `iosCreateHeap` checks for a free heap slot first. For a zero-sized heap with
a free slot, `__lwp_heap_init` returns zero because the requested size is below
`HEAP_MIN_SIZE`, and `iosCreateHeap` returns `IPC_ENOMEM`
(`../../C-sources/libogc/libogc/ipc.c:721-755`,
`../../C-sources/libogc/libogc/lwp_heap.c:9-14`). When all slots are occupied,
C returns `IPC_ENOHEAP` before it checks the size. Rust previously returned
`IPC_EINVAL` for zero before checking either condition. `HeapId::create(0)` now
uses the heap creation path, which returns the C result in the same order.
Negative sizes still return `IPC_EINVAL` as a safety check because C's signed
to unsigned size conversion can reach invalid memory bounds.

The production-shared preflight regression checks zero size with an available
slot, zero size with no free slot, and a size at the minimum. Dolphin 2609a's
`ipc_async_probe` calls the public API and checks `IPC_ENOMEM` for zero and
`IPC_EINVAL` for a negative size. It passed 13 checks, 0 failed; DOL SHA-256
`b5d3d2b31381853f4df77fb6aed5baf3f1f258daec96253febd8ac60209e2c95`.
PowerPC test code compiled with `cargo check -p ogc-ipc --tests --target
powerpc-unknown-linux-gnu`. Physical Wii heap error behavior remains pending.


## 2026-11-26 fix — C IPC heap entry points

The C header declares `iosCreateHeap`, `iosAlloc`, and `iosFree`
(`../../C-sources/libogc/gc/ogc/ipc.h:73-75`). Rust exposed only the typed
`HeapId` methods, so C callers could not link these functions. The crate now
exports all three C ABI symbols as thin wrappers around `HeapId`. A null
`iosFree` pointer is ignored, and the unsafe Rust declaration documents the
pointer ownership requirement.

The PowerPC test build passed. The `ipc_async_probe` ran in Dolphin with 16
checks and 0 failures. It tested zero-size creation, allocation and byte
round-trip, freeing, and an invalid heap id. The linked target archive
contains all three symbols. Physical Wii behavior remains pending.

## 2026-11-26 fix — IOS file-operation C ABI

The C header also declares `IOS_Open`, `IOS_OpenAsync`, `IOS_Close`,
`IOS_CloseAsync`, `IOS_Seek`, `IOS_SeekAsync`, `IOS_Read`, `IOS_ReadAsync`,
`IOS_Write`, and `IOS_WriteAsync` (`../../C-sources/libogc/gc/ogc/ipc.h:82-93`).
Rust had the request behavior in `FileDescriptor` and `open_file*`, but did not
export those C symbols. `ogc-ipc` now exports all ten as thin entry points that
reuse the existing request allocation, submission, callback, cache, and cleanup
paths. The C path reader rejects a null pointer or a missing terminator in the
64-byte bound before it builds a `CStr`. Read and write reject negative lengths
and null pointers for nonzero lengths. These checks prevent invalid slices and
out-of-bounds path reads; the C implementations did not make these checks.

The C `*Async` entry points always queue requests, including when the user
callback is null. The Rust convenience `open_file_async` and `close_async`
methods use synchronous behavior when their callback is null, so the C wrappers
call shared async submit helpers directly. The async open trampoline still frees
its path message when the user callback is null.

The existing `runtime_probe` now calls the exported sync open/read/seek/close
symbols on the settings file, tests invalid path and buffer inputs, and calls
all four read/write/seek/close async symbols on the private `/tmp` ISFS probe
file. It also keeps direct Rust async-method checks. The existing
`ipc_async_probe` now calls the exported `IOS_OpenAsync` symbol. Dolphin 2609a
passed `runtime_probe` (883 checks) and `ipc_async_probe` (16 checks), with zero
failures. DOL SHA-256 values are `85bfc0c2d640b842d5c554c6f4128ea91fc816e5c0faf3952a4f3938fbbf6bf7`
and `5dd9fa6803395ba954a67c5b760a0a792131f09dd8c30492e6bf2f6c9488081d`.
The report and transport evidence are in
`../dolphin-runs/2026-11-26-ipc-c-abi-file-apis/`. `nm -g` on the target IPC
archive found all ten exported symbols. `cargo check -p ogc-ipc --tests
--target powerpc-unknown-linux-gnu`, `cargo xtask check-inventory`, and
`cargo xtask audit-source` passed. Cross-target test linking was not available
on the host, so the test suite was compile-checked, not run. No physical Wii
check was done.

At the time of this file-operation fix, six C declarations still lacked Rust exports: `IOS_Ioctl`, `IOS_IoctlAsync`,
`IOS_Ioctlv`, `IOS_IoctlvAsync`, `IOS_IoctlvReboot`, and
`IOS_IoctlvRebootBackground`. The following section records their later implementation and verification limits.

## 2026-11-26 in progress — C ioctl and vector entry points

Commit `f4f140b1` adds C ABI entry points for `IOS_Ioctl`, `IOS_IoctlAsync`,
`IOS_Ioctlv`, `IOS_IoctlvAsync`, `IOS_IoctlvReboot`, and
`IOS_IoctlvRebootBackground`. The existing runtime probe called the first four
against file statistics and the network MAC service in Dolphin 2609a. It passed
888 checks, with zero failures. Its DOL SHA-256 is
`099aae5bc0841e1b0aed9f095f2fc897e1a60c1e4582a670007ed9a8843fdeab`;
see `../dolphin-runs/2026-11-26-ipc-ioctl-c-abi-final/suite.json` and the
probe transport. The reboot entry points were called only with negative vector
counts. A forced async submission failure restored both caller vector addresses in
Dolphin; the 889-check run is recorded in
`../dolphin-runs/2026-11-26-ipc-ioctl-failure/suite.json`. A real relaunch
remains unverified. Physical Wii evidence remains pending. These changes are not yet
accepted as complete parity work.

### Vector reply address review

C converts each non-null vector address to its cached alias in the reply handler
before it calls the user callback (`ipc.c:363-379`). The Rust reply handler does
the same before it schedules the callback (`ogc-ipc/src/lib.rs`, reply handler).
The synchronous raw wrapper then restores the exact original caller addresses,
including zero-length protocol values. This is a deliberate Rust difference:
callers retain their supplied aliases instead of receiving a cached alias. The
async wrapper still exposes the cached alias on completion, as C does. The
Dolphin MAC checks exercise aligned nonempty vectors; they do not verify alias
preservation for all address spaces or a real IOS relaunch.

### Open relaunch mailbox ordering gap

C's send path decrements the mailbox acknowledgement count twice for a
foreground relaunch and once for a background relaunch. Its acknowledgement
handler signals the relaunch waiter only when that count becomes positive
(`ipc.c:300-319,409-429`). Rust stores a zero acknowledgement state for both
forms and signals the waiter on the first acknowledgement (`ogc-ipc/src/lib.rs`,
`write_request` and `ackhandler`). This is a confirmed ordering difference:
the foreground path can return after the first acknowledgement in Rust while C
waits for the count to become positive. The current invalid-count Dolphin
checks do not reach either path. Implement and test the acknowledgement state
machine for both relaunch modes before treating those APIs as verified.

The current mailbox change uses a signed count and applies C's extra foreground
relaunch decrement. The acknowledgement handler wakes the relaunch waiter only
when the count is positive. The reply handler clears the relaunch marker and
returns a credit before waking the waiter, as C does. Host tests cover the count
transitions. Dolphin 2609a passed the existing `ipc_async_probe` with 16 checks
and zero failures; evidence is in
`../dolphin-runs/2026-11-26-ipc-mailbox-order/suite.json`. That probe checks
ordinary async IPC and does not exercise an IOS relaunch. A real relaunch and
physical Wii behavior remain unverified.

The existing `runtime_probe` has only invalid-argument ES launch checks. The
Dolphin validation runner expects the probe's completion summary; a successful
title launch replaces the probe and therefore needs a separate observation of
the launched title. The HBC recovery runner observes an installed title entry
through its debugger, but its recovery-specific result does not establish the
IPC C exports. A relaunch probe must retain a private NAND fixture, identify the
installed title and ticket, and record the destination after the handoff. No
such end-to-end IPC result is claimed here.

The local Dolphin NAND has system menu title `00000001-00000002`. The existing
runtime probe reported one ticket view and verified that the view identifies
that title in the 889-check run. At inspection, its content `title.tmd` SHA-256
was `1a5df7b0e72721c9ffae34664b00144eda2aeee0e327332deda84d3cfddb317b`
and the content `ticket` SHA-256 was
`71b4d356f4e46f3c4f18226a06b5cb758f4dc2dd6eb6ae1b96a52957aa5d7d86`.
These identify the inspected files only; they do not prove the copied NAND or
launched title in a future run. The existing probe can obtain the ticket view,
but a new handoff observation is still needed.

The first focused foreground probe ran in Dolphin 2609a with a private NAND.
The probe retrieved the system menu ticket view and logged that handoff began.
Dolphin logged `Re-launching title 0000000100000002 after IOS reload` and then
read and wrote the system menu's `state.dat`. The runner classified the session
as a crash because the probe did not emit its completion summary. The saved
result and logs are in `../dolphin-runs/2026-11-26-ipc-relaunch-initial/`.
This is evidence that Dolphin entered the target title path, not evidence that
the system menu reached a usable state or that background relaunch works.
The DOL SHA-256 was
`13317a4a659692f7ed6fefeab5a26e8c050699c508fc40ab949d3eeedeb5c622`.

A subsequent foreground run captured Dolphin's process status as SIGABRT. A
background build returned zero to the probe, and Dolphin logged the system menu
relaunch before it also exited with SIGABRT. Both runs logged system menu
`state.dat` access after the title reload. The reports are at
`../dolphin-runs/2026-11-26-ipc-relaunch-signal/suite.json` and
`../dolphin-runs/2026-11-26-ipc-relaunch-background/suite.json`. The foreground
probe did not record its return value before handoff. Neither run establishes a
usable menu or explains the emulator abort. The background result establishes
that the call returned zero and Dolphin entered the title reload path.

A foreground diagnostic run used the same DOL with
`Dolphin.Interface.AbortOnPanicAlert=False`. Dolphin logged the system menu
reload and remained running until the runner's 45-second timeout. The debugger
then failed to answer the interrupt and memory requests, so the saved diagnostic
contains no program counter. The setting change and outcome are recorded in
`../dolphin-runs/2026-11-26-ipc-relaunch-panic-diagnostic/suite.json`. No alert
text was captured. This comparison does not establish the cause of SIGABRT or
prove a usable menu.

A controlled foreground comparison used the same DOL with Dolphin's Metal
backend instead of OpenGL. Dolphin again logged system menu `state.dat` access
and exited with SIGABRT before a probe summary. The recorded arguments and
result are in `../dolphin-runs/2026-11-26-ipc-relaunch-metal/suite.json`.
This rules out the choice between these two backends as a sufficient explanation
for the observed abort; it does not identify the cause or prove menu usability.

Native LLDB is not currently a usable way to inspect this failure. A direct
control run of `/usr/bin/true` under `/usr/bin/lldb` exited with status 1:
macOS denied the debugger attach. The raw control output is in
`../dolphin-runs/2026-11-26-ipc-lldb-permission/lldb-true.log`. This does not
identify the cause of the Dolphin abort. A working diagnostic method is still
needed before the relaunch result can be resolved.

### Reinitialization interrupt exclusion

Source comparison found an ordering gap. C's
`__IPC_Reinitialize` disables CPU interrupts before it writes IPC control and
resets the mailbox credit, relaunch marker, and request queue; it restores the
previous interrupt state after those operations. Rust's `reinitialize` reset
the same shared state without an equivalent critical section. The Rust IOS
reload path restores and unmasks the IPC interrupt before calling it, so the
IPC interrupt handler could potentially observe an incomplete reset. Rust now
uses the existing interrupt guard around reinitialization and preserves the
request magic, as C does.

The instrumented Dolphin baseline recorded MSR 41010 before and during the
reset and failed the interrupt exclusion check. The fixed run recorded 41010
before, 50 during, and 41010 after; it passed both the enabled and nested
disabled interrupt checks and 16 other IPC checks. Results are in
`../dolphin-runs/2026-11-26-ipc-reinitialize-msr-red/` and
`../dolphin-runs/2026-11-26-ipc-reinitialize-final/`. The probe calls
reinitialization directly with no request pending. It does not test an actual
IOS reload, outstanding replies, or the separate relaunch abort.

A separate `ios_reload_probe` run in Dolphin 2609a reloaded IOS 58. The probe
reported return code zero, the same IOS version after the call, and a
successful `/dev/fs` open and close. Dolphin's IOS log records IOS 58 starting
twice, once at boot and again during the probe. The result and log are in
`../dolphin-runs/2026-11-26-ios-reload-run/`. This covers one IOS version and
does not exercise pending IPC replies or the menu relaunch abort. Physical Wii
verification remains pending.

An instrumented repeat in `../dolphin-runs/2026-11-26-ios-reload-masked/`
observed IPC reinitialization during the IOS 58 reload. Its saved MSR was 50,
with external interrupts disabled, and all 12 probe checks passed. The probe
build enabled the `format-probe-hooks` feature. This run still
does not cover pending IPC requests during reload.

### 2026-11-26 Linux reply decoder and relaunch follow-up

The C request declaration overlays the descriptor field with `req_cmd` at
byte offset 8. In `__ipc_replyhandler`, C selects the read, ioctl and ioctlv
cache operations from this reply field. IOS replaces command word 0 with
`IPC_REPLY`. Rust previously selected the operation from word 0, so a
successful raw async ioctlv reply could leave the caller's vector data
address physical instead of cached (K0). Rust now selects the operation from
word 2 (`fd` during submission), before cache maintenance and callback
scheduling. The focused live ES title count test checks the vector address,
result, callback count, output and neighboring canary bytes in the callback.
The baseline failed only the address check (22 passed, one failed); the fixed
probe passed 23 checks. The broader runtime run retained exactly the same 13
video/network failures as its baseline (876 passed, 13 failed); it is not a
full suite pass. All IPC formatter checks passed, including the live sync and
async output boundary cases. The formatter ledger now has 80 verified and
eight hardware-blocked rows, with no partial or unverified rows. IOS reload
passed 21 checks. Physical cache behavior is not established by Dolphin.

The relaunch abort reproduced on a direct System Menu boot without the Rust
probe. A diagnostic with automatic panic abort disabled exposed an invalid
read of address zero at PC `0x8134d700`. The imported NAND contains a
Priiloader-modified System Menu boot file and TMD, and original copies of
both. Restoring the original pair in a private fixture (after matching the
original boot file against its original TMD size and SHA-1) allowed direct
boot to the Health and Safety screen with automatic panic abort enabled.
The identical foreground and background DOLs also reached this screen.
The sequential background run returned zero before the handoff. The raw
runner reports remain timeouts, not passes, because the launched menu
replaces the probe. The destination screenshots and IOS title/boot logs
establish title handoff for both operations, not subsequent menu interaction
or every possible mailbox sequence. This isolates the abort to the modified
fixture; the exact failing guest instruction's cause within that fixture
has not been established. No production IPC change was needed for this
fixture correction.

See [IPC reply and relaunch evidence](../dolphin-runs/2026-11-26-ipc-reply-relaunch-linux/README.md).
Outstanding replies across reinitialization still need targeted evidence.
The physical Wii cache and loader override cases remain pending.


## 2026-11-26 follow-up — loader overrides, held reply and relaunch runner

The [follow-up evidence](../dolphin-runs/2026-11-26-ipc-followup-linux/README.md)
verifies low-only, high-only and both nonzero loader bounds in Dolphin by
narrowing the reserved arena. A real posted async IOS reply survives the PPC
client reset and completes once; a new request succeeds afterward. The
combined test passes 29 checks. This does not establish survival of in-flight
requests across replacement of the old IOS instance. Both foreground and
background relaunch now report automatic handoff success with a matching
mode, IOS boot log and live destination code. The previous timeout records
remain unchanged. Physical cache coherency and physical loader behavior
remain pending. Keep the package parity-blocked for the combined actual IOS
reload/outstanding request coverage gap. No IPC production changes in this
follow-up were needed.

## 2026-11-26 follow-up — outstanding request during IOS replacement

The [replacement probe](../dolphin-runs/2026-11-26-ipc-ios-replacement-linux/README.md)
passes 42 checks. A real pending STM event hook is discarded by the replaced
IOS 58; the old callback does not run and a fresh request completes once.
The old request record and deferred reservation remain allocated: available
records decrease from 39 to 38 and available slots from 32 to 31. C
`__IPC_Reinitialize` does not reclaim old requests; the bounded deferred
reservation is Rust-specific. Generic PPC reset must still preserve posted
replies. A distinct cancellation design is required before claiming cleanup;
keep the package parity-blocked. No production IPC change was made. Physical
Wii evidence remains pending.

## 2026-11-26 follow-up — replacement-specific cancellation

The [cancellation review](../dolphin-runs/2026-11-26-ipc-ios-cancellation-linux/README.md)
records 103 passing IOS reload checks across three replacements, including
exact-once raw and formatted cancellation, full capacity recovery and no
staged-output copyback. A new IPC request completes after each replacement.
The separate ordinary PPC reset probe passes 29/29; the broad runtime probe
retains exactly the two existing stock Linux socket-option failures (887
passed). The resource-lifetime gap measured in the 42-check baseline is
resolved in these Dolphin cases. The new Rust-only cancellation design
requires independent re-review of concurrent and failed replacement paths;
physical Wii cache/DMA and loader evidence remain pending. Move the package
to needs_re_review, not reviewed.

## 2026-11-26 cancellation re-review — competing thread and scope release

The submission gate is checked under the interrupt-protected queue path.
The async tracker claims a slot before queue acceptance and removes it on
rejection; the reply handler removes it before transferring ownership to
the deferred worker. Cancellation scans only outstanding tracker entries
with interrupts disabled, not already deferred completions. Formatter
cancellation uses the separate no-copyback completion callback. `finish`
requires a successful fresh synchronous ES open/close before retirement;
ordinary PPC reset does not scan or cancel the tracker. A failed round trip
intentionally leaves old records allocated because freeing memory without
retirement confirmation is unsafe. This preserves safety at the cost of
capacity until a later confirmed replacement; recovery on that failure path
is not tested. The existing reset clears the software send queue, so a test
of an already posted reply does not establish behavior of unsent queued
requests. The replacement scope is limited to the IOS launch path, not the
entire surrounding subsystem shutdown/initialization sequence.

A new competing-LWP test checks actual synchronous/asynchronous rejection
and nested replacement rejection, followed by owner release and fresh IPC.
The repeat reload run passes 105/105, with ten valid captures, and preserves
all three existing cancellation iterations. The first extended run was
interrupted at capture page 9 and has no complete summary. See
[scoped evidence](../dolphin-runs/2026-11-26-usbstorage-startstop-ipc-scope-linux/README.md).
No IPC production change is made in this review. Retain `needs_re_review`
for the remaining failure, queued-request and concurrency cases; physical
Wii cache/DMA evidence remains pending.

### Aborted replacement scope and queued-request review (2026-11-26)

A real pending STM event-hook request survives a failed owner IPC request and
release of the replacement scope without `finish`. The extended Dolphin reload
probe passes 108/108 over three iterations (see
[scoped evidence](../dolphin-runs/2026-11-26-ipc-aborted-scope-linux/README.md)).
A failed actual IOS launch and failed fresh ES handshake remain untested.
`reset_request_queue` clears software queue slots and counters on PPC-only
reinitialization, as C does. Accepted requests still in the software queue
have not reached IOS; clearing the slots can leave asynchronous ownership
tracked without a corresponding reply, or strand a synchronous waiter.
This is a source-level concern, not a reproduced runtime defect. Distinguish
unsent requests from transmitted requests before changing cancellation or
mailbox-credit behavior. Keep independent concurrency review open.

## External IRQ dispatch context — 2026-11-26

The external IRQ dispatcher now exposes a scoped runtime context flag for
LWP self-suspension. This avoids a cooperative context switch from the
shared exception stack and uses the existing exception-return scheduler,
matching C's deferred switch in exception context. The `ogc-sys` host test
checks the flag inside the callback and its restoration on return. The
[GX live interrupt evidence](../dolphin-runs/2026-11-26-gx-live-cp-irq-linux/README.md)
verifies real suspension and resumption; independent review is required.
This is not a blanket change to exception handling or other blocking APIs.

### Dolphin reboot API handoffs

Dolphin 2609a launched the installed OHBC homebrew test title
(`000100014f484243`) from a fresh private NAND copy for each variant.
The boot content `00000001.app` was 2,454,528 bytes.
Its SHA-1 `8636c3acb85f20080a0baa18c324ac0feb7ea717` matched the TMD.
Its SHA-256 was `5d3f61d8e7e4304461d361b269b93777de4fb08c03683413cc85b8310fd4f610`.

| API | Mode | Dolphin run | Result |
| --- | --- | --- | --- |
| `IOS_IoctlvReboot` | Foreground | `1795784008-a2355468ad9e` | Pass |
| `IOS_IoctlvRebootBackground` | Background | `1795784020-a2355468ad9e` | Pass |
| `FileDescriptor::ioctlv_reboot` | Foreground | `1795784032-a2355468ad9e` | Pass |
| `FileDescriptor::ioctlv_reboot_background` | Background | `1795784044-a2355468ad9e` | Pass |

In each run, the source reported its API family, mode, and title ID.
Dolphin then logged the IOS title launch and the expected boot content path.
The destination emitted `HBC: startup complete; entering render loop` after
the boot log. The runner used these events in order to confirm handoff.

Select OHBC with `LIBOGC_DOLPHIN_RELAUNCH_TITLE=000100014f484243` and use
`cargo xtask dolphin-validate --probe ipc_relaunch_probe --dolphin PATH`.
Set `LIBOGC_DOLPHIN_RELAUNCH_API=rust` for the Rust API and
`LIBOGC_DOLPHIN_RELAUNCH_BACKGROUND=1` for background mode.
The run IDs identify the local Dolphin reports and logs.

At this stage, the System Menu handoff remained open because the default
fixture paired a 837,400-byte modified boot image with a TMD that declared
3,867,904 bytes. The later
[2026-11-27 run](../dolphin-runs/2026-11-27-stock-menu-relaunch/README.md)
uses the matching original boot image and TMD and passes all four API/mode
handoffs. Physical Wii cache and DMA checks remain open separately.

### Dolphin formatter contracts

All 90 contract rows match named passing checks in the 890 check report
(`1795770694-ac3843e4fb24`) and its 892 check repeat
(`1795783878-a2355468ad9e`). The heap exhaustion contract lists its three
separate assertions for Argv failure, Scalar failure, and recovery.
Eight rows still require a physical Wii for cache and DMA verification.

The expanded runtime run (`1795784435-a2355468ad9e`) passed 898 checks.
The probe DOL SHA-256 was `88eac13127b1206d1831765ea63a52b9bf87f11ee78de4fad8ef3d1ffe1949af`.
All 90 formatter rows matched again. The four raw ioctl APIs passed their
original file stats or MAC checks, mixed input and output, IOS errors, and
local argument rejection. The async checks confirmed callback count and
userdata. Both vector APIs restored cached buffer pointers on reply.
