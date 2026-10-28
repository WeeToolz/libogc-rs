# Wii std platform selector audit

Updated: 2026-10-14. Target: `targets/powerpc-nintendo-none.json`, pinned std
source from `rust-std/source-lock.json`.

## Target cfg

The target is PowerPC, Nintendo, `os=none`, `env=eabi`, big endian, and 32-bit.
`rustc --print cfg` reports no `target_family` and no native
`target_thread_local`. The `gamecube` cfg is absent in the Wii build. Therefore
selectors that use a Unix family condition do not select Unix on this target;
fallback arms must be reviewed as actual platform behavior.

## Selected production backends

| std area | selected source | audit result |
| --- | --- | --- |
| allocation | `sys/alloc/wii.rs` | Wii allocator adapter; existing production allocator is used. |
| arguments | `sys/args/wii.rs` | Wii startup argument provider. |
| OS PAL | `sys/pal/wii/` | Wii LWP, TLS, timebase, process exit, and system interfaces. |
| thread | `sys/thread/wii.rs` | Wii thread adapter backed by LWP. |
| thread-local storage | `sys/thread_local/os.rs` | Generic key-based storage backed by the Wii PAL; not native compiler TLS. |
| synchronization | `sys/sync/{futex,mutex,condvar,once,rwlock}/wii.rs` | Wii selectors use the pinned futex logic and LWP wait/wake transport. |
| clock | `sys/time/wii.rs` | Checked Wii monotonic and RTC-backed wall clock adapters. |
| random | `sys/random/wii.rs` | Nonempty requests return Unsupported until a real entropy source is verified. |
| I/O errors | `sys/io/error/wii.rs` | Wii/Newlib error translation. |
| standard streams | `sys/stdio/wii.rs` | Nonempty stdin reads and stdout/stderr writes return Unsupported; empty writes succeed with zero bytes. Flush reports Unsupported. No bytes are discarded with success. |
| TCP/UDP and DNS | `sys/net/connection/wii.rs` | IPv4 IOS socket service, owned DNS results, shared clones, real poll-based I/O deadlines. `connect_timeout` and unavailable option queries return Unsupported. |

## Remaining selector behavior

These selections are deliberate limits or open contracts. They do not claim
that the corresponding std facilities are complete.

| std area | selected source | behavior and remaining work |
| --- | --- | --- |
| environment | `sys/env/unsupported.rs` | `getenv` reports no value; environment iteration is unavailable and panics; mutation returns Unsupported. A real environment adapter remains open. |
| filesystem | `sys/fs/wii.rs` | Production typed FAT files, shared clone cursors, synchronized I/O, checked flush, metadata, mkdir/unlink/rmdir. Directory traversal, rename/copy, recursive removal, canonicalization, and unsupported file services remain open. See [file design](wii-std-files.md). |
| paths | `sys/paths/unsupported.rs` | Current directory, executable, home, temp, and path-list services are unavailable. |
| path syntax | `sys/path/wii.rs` | Uses `/` separators and a Wii mount prefix. `sd:/` is absolute and rooted joins retain the mount. Only absolute mounted paths reach the file adapter; relative/current-directory and path-list services remain open. |
| child processes | `sys/process/unsupported.rs` | Child process creation and status operations are unsupported. Process exit uses the Wii PAL and has a separate cleanup/arbitration contract. |
| pipes | `sys/pipe/unsupported.rs` | Pipe creation returns Unsupported. |
| terminal detection | `sys/io/is_terminal/unsupported.rs` | Reports `false`; the bool API cannot return an unsupported error. No Wii terminal contract is established. |
| host name | `sys/net/hostname/unsupported.rs` | Host-name lookup returns Unsupported. DNS address lookup is implemented in the network connection backend. |
| raw OS descriptors | `sys/fd/mod.rs` empty fallback | Unix raw-fd APIs are not selected for this target. |
| OS string storage | `sys/os_str/bytes.rs` | Byte representation is selected. Its compatibility with all Wii path and environment encodings remains open. |
| backtrace | `sys/backtrace.rs` | Backtrace default is disabled; panic strategy is abort. No useful hardware backtrace output is claimed. |
| OS constants | `sys/env_consts.rs` fallback | Target identity and executable suffix constants use the fallback values; review remains open where applications consume them. |

The Rust std source contains additional selectors for Unix-only kernel copy,
personality, file-descriptor, and platform-specific math paths. With this
target's cfg, their Unix arms are not selected; the empty/no-unwind or generic
Rust implementation is selected. Their DOL link is covered by the current std
probe build, but platform behavior beyond the rows above is not accepted by
that link result.

## Audit result

The source selector review is complete for this pinned target. The audit found
and removed a false-success standard-stream fallback by selecting
`sys/stdio/wii.rs`. The file increment replaces the generic Unix path fallback with a Wii mount
parser and selects a typed FAT file adapter. Network and DNS source adapters
are also selected directly for Wii. Environment, remaining file/directory
services, path services, child processes, pipes, terminal detection, and hardware
behavior still block full std adoption.

Source preparation and std compilation passed with `python3
scripts/check-wii-std.py --offline`. The std probe DOL build and link passed with
`cargo xtask build-wii-std-probe`. Neither result is Wii hardware execution.
