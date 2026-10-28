# libnetwork Review

## 2026-10-21 signed IPv4 host-ID result

The physical Wii startup display recorded error `-1062731261` in run
`1792604768-480134000`. Its bits are `0xC0A80203`: the assigned address
192.168.2.3. The C reference in `network_wii.c` returns host IDs as `u32`;
its startup state 10 tests for zero to continue DHCP. Rust's shared startup
and active-attach helpers instead rejected every negative signed host ID.
This rejected valid unicast addresses with first octets 128 through 223.

The helpers now share `src/startup_ip.rs`. Address validation accepts those
unicast bits and preserves negative IOS errors in the reserved upper range.
SO_STARTUP status remains a status value and still rejects negative errors.
Four native tests compile the exact production helper through xtask and
check active attachment, delayed DHCP assignment, error preservation, and
the existing 101-query/100-wait limit. All four passed. The diagnostic DOL
built. Hardware run `1792605142-670850000` with DOL SHA-256
`af114e68ee08c10b430e800bf0cef3ecc851ad5500c2c8ed70ced751629a26ea`
established the report channel and sent 501 verified progress records and
seven checkpoints. A later progress send failed with -1. This confirms the
address startup correction; it does not establish complete application startup.


## 2026-10-11 completed Wii matches and online latency

The user confirmed two completed real Wii/Dolphin games with DOL SHA-256
`24483a622613c99fc17db4a886852d9ea186bd23d47433a77dacb4a4d984f3ef`: first
Dolphin hosted, then the Wii hosted. The frozen acknowledgement receive was
resolved in this run after omitting the unused IOS recvfrom address vector.
This is user-observed hardware evidence; it is not an automatic complete-match report.

The online loop sent the current input and waited for the matching remote input
on every simulation frame. It also waited for VI twice: `draw_online_game` already
waits when it swaps framebuffers, followed by another wait in `run_match`.
The optimized loop waits once and sends sampled input three frames ahead.
The same shared `InputDelay` queue applies each local sample at its transmitted
sequence. Three neutral input frames prime the stream; remote input is still
validated and consumed in sequence before simulation. The IN wire format and
handshake remain version 1. Both players must use the current build for the full
performance benefit. The tradeoff is three frames of local input delay.

Two tests of the exact production queue passed: ordered input delivery and a
single button event delivered once. The DOL built. With a controlled 40 ms delay
on each real TCP response, the previous build exchanged 98 packets in 8.928 s,
while the optimized build exchanged 478 in 8.940 s (about 4.9 times the exchange
the handshake, match sequence, rejection of a second client, and null address
output on the production receive calls. This is an emulator transport comparison,
not a measured Wii frame-rate improvement. Optimized Wii gameplay remains pending.

The first optimized capture reported a TCP reset during deliberate emulator
termination. The capture now records its cleanup start time. The checker retains
the reset/error details and accepts that disconnect only after a completed capture
has entered cleanup. Earlier disconnects and protocol errors remain failures.
Normal matches also report WIP progress after each 120 confirmed input exchanges.

The real TCP disconnect check also passed. The peer closed after 30 input
responses; production application reported a MATCH error and returned to its menu.
The optimized DOL uploaded through AppleScript and the command launcher in run
`1791768465-008894000`. The Wii reported MENU, completed listener setup, and
reported WAITING FOR CLIENT. A completed optimized hardware match remains pending.

Optimized DOL SHA-256:
`17b3fe1f1e76bd64456c8e17d6e3e8aa143e69e320ff9a7b10cdc5359509eb9e`.
The duplicate identical network evidence row was removed, and moved references
in the affected IPC and network files were checked against their definitions and
refreshed. The completeness audit now stops at the existing stale
`mp3player-sys::init` reference (`src/lib.rs:711`); it is not marked passed.

## 2026-10-11 manual Dolphin host loaded a stale build

The user supplied a Wii client run (`1791766647-685001000`) and a manual Dolphin
file was built on October 9 and has SHA-256
`5aeb773a8c19976d0497695435f5b6187dad64bf3b819f4ebbab26f0b9a95873`.
`61b7362e110d7202a574de6784981d2a8167c8b078f1d9c6e371cb57383089aa`.
The manual host reached acceptance without recording handshake completion in
the supplied excerpt. This run does not verify the corrected Dolphin host.

The shared capture script now defaults application to the current custom DOL and its
matching custom-build ELF. Explicit path arguments still select the requested
artifacts. A new hardware run with the current build on both ends is required;
the stale artifact does not explain or resolve the earlier current-build Wii host stop.

## 2026-10-11 application host acceptance and persistent polling descriptor

### Follow-up hardware run and two-client check

The first corrected DOL uploaded to the Wii and reported `ACCEPT CLIENT`,
`CLOSE LISTENER`, and `STREAM OPTIONS`. It later returned to `MENU` and opened
another listener; this run did not remain frozen in acceptance. The Dolphin
client returned `-1` from the mandatory `TCP_NODELAY` setup. A match did not start.

application stream setup now reads the real descriptor flags and clears nonblocking
mode, with both fcntl results checked. The existing transfers use poll before
blocking reads. The game no longer requires TCP_NODELAY, which is an optional
latency optimization. Its rejection is not converted into a successful option call.

The regression now attempts a second real TCP connection during acceptance. It
requires that the second client receives no match data and is refused or closed,
while the first client completes the handshake and exchanges sequential packets.
The verified Dolphin run passed with 226 packets and second-client rejection:
DOL SHA-256: `deec9ffefdee7c94baaaecac6b93a6da184098f1363cfc03f7a5dae7823d9ec3`.
The production host accepts one stream and closes its listener before handshake;
it never accepts a second stream into that match. Hardware match verification of
this latest build remains pending. Clients retain the existing network error and
retry screen; a refusal does not prove that the host is busy rather than offline.

The latest build uploaded in run `1791765291-781470000`. Wii progress confirms
listener setup, acceptance, listener closure, stream setup, and handshake entry.
Dolphin received the Wii hello, sent its acknowledgement, and entered the client
Its log records only the first match input send and no match input receive before
capture cleanup. The Wii progress remains at handshake. This proves a successful
hello exchange, not a completed or continuously running hardware match. The
remaining hardware stop is open work; the two-client result is Dolphin evidence.

The build with separate acknowledgement checkpoints passed the same real TCP
two-client regression with 227 input packets and no match data sent to the
second client. Evidence:
DOL SHA-256: `61b7362e110d7202a574de6784981d2a8167c8b078f1d9c6e371cb57383089aa`.
The build uploaded through AppleScript and the command launcher in run
`1791765827-577046000`. The real Wii reported through `HANDSHAKE`, but did not
report `ACK POLL`. Dolphin received the hello, sent its acknowledgement, and
entered its match screen, without receiving match inputs before capture cleanup:
The remaining stop is before the acknowledgement poll checkpoint completes;
the hello send completion or the following progress-report call is not yet
distinguished. A successful Wii match is not established by the client screen.

A real TCP peer reproduced the host stop in Dolphin: application reached `ACCEPT CLIENT`,
then its synchronous `IOCTL_SO_ACCEPT` remained pending and no handshake bytes arrived.
The capture includes the pending request in

C `net_poll` uses the initialized `net_ip_top_fd` without opening or closing the
device (`C-sources/libogc/libogc/network_wii.c:1143`). Rust instead opened and closed
a temporary `/dev/net/ip/top` descriptor on every poll. Dolphin's device close
sets its active state false, and its kernel only calls `Update` on opened devices:
[Device.cpp](https://github.com/dolphin-emu/dolphin/blob/master/Source/Core/Core/IOS/Device.cpp),
[IOS.cpp](https://github.com/dolphin-emu/dolphin/blob/master/Source/Core/Core/IOS/IOS.cpp).
The network device's update services pending socket operations. This explains the
reproduced pending accept after polling closed the temporary descriptor.

Rust polling now reuses `NET_IP_TOP_FD` and returns `-ENXIO` if network initialization
has not provided it. It leaves descriptor teardown to the existing network lifecycle.
The IOS ioctl, descriptor buffers, timeout, copy-back, and negative operation result
remain on the production path. application also reports network stages through WIP1 and
shows acceptance, listener closure, stream options, and handshake on the host screen.

The reusable `scripts/check-application.py` drives the existing capture script
and connects a real TCP peer to the production host. It validates handshake bytes,
sequential match input packets, host handshake/match logs, and capture completion.
The pre-fix run failed before receiving the hello. With persistent polling, the
verified run completed the handshake and exchanged 224 input packets without a
reported network failure:
DOL SHA-256: `ed94cb16d7097727570bbed01283bfa72ea766b7a383d737a56f04a28be8f803`.
This is Dolphin evidence; the real Wii acceptance and match remain pending.

The source audit and Wiiload host compile passed. The completeness/inventory gate
is blocked by an existing duplicate evidence entry for `ogc-network-sys::if_config`
at `config/implementation-evidence.csv:544`. This blocker was not suppressed and
the generated inventory was not marked complete. Independent review remains pending.

## 2026-10-09 application client acceptance

The Rust-only `ogc-network::accept_socket` helper passed null peer-address and length pointers. Both the C Wii implementation (`../../C-sources/libogc/libogc/network_wii.c`, `net_accept`) and the Rust IOS backend require address storage. Rust rejected the helper's request before issuing `IOCTL_SO_ACCEPT`, returning `-1`. This explains a host error at `WAITING FOR CLIENT` immediately after a TCP connection reaches the listening socket. A TCP port check alone does not prove application acceptance or a completed game handshake.

The helper now supplies a local IPv4 address and length through the existing production accept path and discards only the optional peer address. The raw backend's validation is unchanged. The regression uses the exact production buffer helper, checks non-null writable storage with an independently expected 16-byte public address layout, and checks accepted-descriptor and failure-result propagation. That regression passed in a temporary host harness. The PowerPC crate and full target test code compiled. This harness verifies the argument contract; it does not perform a hardware accept.

`connect_addr` now retains the negative backend result through a shared backend dispatch function. The raw `connect` API retains its C socket-wrapper convention of returning `-1` for errors. application uses the typed helper, displays signed errors, and records `ACCEPT CLIENT` and `STREAM OPTIONS` separately from waiting and handshake stages.

The corrected example DOLs built successfully. The source-structure audit passed. The completeness audit stopped on an existing stale `mp3player-sys::init` evidence reference at `src/lib.rs:711`; this was not suppressed. Wii acceptance, the game handshake, and a full Wii-to-Dolphin match remain unverified until the corrected DOL runs on the Wii.

### Subsequent Wii result: active attachment timeout


Source tracing found that active attachment queried `SO_GETHOSTID` five times with four 50 ms waits and never issued `SO_STARTUP` for a zero address. It assumed that an HBC launch guaranteed an immediately available IP on the new descriptor. The C initialization path instead repeats socket startup and host-ID queries until it obtains an address. HBC's TCP loader closes its network descriptor before launch (`../../C-sources/hbc/channel/channelapp/source/loader.c`). This source evidence establishes the missing startup operation; the exact IOS state after launch still requires hardware verification.

Active attachment now queries the existing address once. If the address is zero, it uses the same socket-interface startup and address retry engine as full initialization, without restarting NWC24. It commits the descriptor only after a nonzero address is returned, retains cancellation checks, and returns descriptor-close failures on the changed failure paths. Startup and query failures remain errors. The common engine permits 101 attempts with 100 50 ms waits.

Five tests using the exact production decision and retry helpers passed in a temporary host harness. They cover reuse without startup, address acquisition after six startup/query attempts, initial query failure, startup failure, zero-address timeout, operation order, and cancellation results. Both network crates and their PowerPC test code compiled. The rebuilt example DOLs and source-structure audit passed. Real Wii attachment and a Wii-to-Dolphin match remain pending the next upload.


Compared `ogc-network` and `ogc-network-sys` with `../../C-sources/libogc/gc/network.h`, `libogc/network_wii.c`, and `libogc/network_common.c` on 2026-10-01. Scope is the network APIs and their C-compatible types. The implementation of the underlying lwIP and IPC layers was not reviewed here.

## Confirmed mismatches

### `net_init_async` is synchronous in Rust

The C implementation of `net_init_async` in `libogc/network_wii.c` sets up an IOS callback chain, starts it, and returns. Its user callback runs when that chain completes. Rust's `wii_backend::net_init_async` calls `net_init()` directly and invokes the callback before it returns (`crates/services/ogc-network-sys/src/lib.rs`). `net_init()` itself waits for completion. The Rust API therefore blocks the caller and has different callback timing; callers that expect to continue while startup proceeds will block. This is a direct behavioral mismatch for the same public API, not only an implementation detail.

### Resolved finding: `if_configex` retry-count parity

C `if_configex` retries `net_init()` up to `MAX_INIT_RETRIES` (32) and does not read `max_retries` (`../../C-sources/libogc/libogc/network_wii.c:1180-1203`). The prior Rust backend used `max_retries + 1` attempts and rejected negative values, which changed behavior for negative values, zero, and values other than 31. Rust now uses the fixed 32-attempt limit, ignores `max_retries`, and waits after every retryable result, including the final attempt, as C does. Both versions still reject `use_dhcp == false`. The high-level `ogc-network::if_config` delegates to this corrected sys function.

### `SockLenT` is signed in Rust and unsigned in C

`gc/network.h` defines `socklen_t` as `u32`; the Rust public alias `SockLenT` is `i32`. Both have the same 32-bit representation on PowerPC, so pointer layout is unchanged, but the Rust source API rejects negative lengths and cannot express the C unsigned contract. This affects the public signatures for bind/connect/sendto and the length pointers used by accept/recvfrom/getsockname/getsockopt. The types should be considered source/API-incompatible even though the machine-level width matches.

## Intentional Rust additions or safety changes observed

- `ogc-network` adds a socket descriptor table around raw lwIP sockets. The C `soc.h` only declares `socInit` and `socExit`; Rust `soc_init`/`soc_exit` and the safe slice/address helpers provide a higher-level interface. These are additional Rust conveniences, not C parity claims.
- `attach_active` / `net_attach_active` is a Rust-only way to attach to an already-running IOS network service without starting NWC24 again.
- Rust validates host lookup response pointers and their bounds before relocating pointers in the fixed hostent buffer. The C code relocates pointers directly. The extra checks are a safety hardening; no incorrect result for valid C responses was established.
- Rust `select` accepts a null timeout and translates it to lwIP's null timeout. The C `net_select` dereferences `timeout` without a null check. This is a safer added case; it is not a mismatch for valid non-null C calls.
- The Rust `if_config` string wrapper formats bounded dotted IPv4 strings into caller-provided pointers. The C wrapper uses `strcpy` with `inet_ntoa`; this review did not verify caller buffer capacities, so no behavior mismatch is claimed for valid buffers.

## Uncertain differences

- `net_gethostip` in C returns zero unless network initialization has set `net_ip_top_fd`; Rust opens `/dev/net/ip/top` itself. This can change behavior before or after initialization. Whether this is an intended convenience or a parity bug depends on whether callers rely on the C initialization precondition.
- `net_init_async` and startup/deinit state do not use the same asynchronous lifecycle as C. The synchronous behavior above is confirmed; effects of overlapping initialization, abort, or teardown calls were not validated on IOS hardware.
- This source comparison did not execute network calls on Wii hardware. IOS ioctl edge cases and lwIP behavior outside the compared wrapper code remain unverified.

## Fix pass: `if_configex` retry-count parity

**BUG FIX.** Implementer: `worker_b`. Independent reviewer: `/root`. Original auditor: `thread-current/luna-network`. In `ogc-network-sys`, `if_configex` now calls a shared retry helper with C's fixed 32 attempts. The helper ignores the API's `max_retries` value, retries only `-EAGAIN` and `-ETIMEDOUT`, and waits 50 ms after each retryable result, including the last attempt, matching the C loop. The `use_dhcp == false` error remains unchanged. The `ogc-network` wrapper already delegates to the sys implementation, so it needs no code change.

Added target-gated regression tests for negative, zero, 31, and `i32::MAX` retry arguments, checking 32 init calls and 32 delays on repeated `-EAGAIN`, plus early exit for a non-retryable result. Existing sys-crate test callback ABI and unsafe call annotations were corrected so the test module compiles.

Validation: `cargo check -p ogc-network-sys --tests --target powerpc-unknown-linux-gnu` and `cargo check -p ogc-network --target powerpc-unknown-linux-gnu` passed. `cargo test -p ogc-network-sys --lib --target powerpc-unknown-linux-gnu --no-run` could not link because the host linker rejects PowerPC options (`ld: unknown options: --as-needed -Bstatic ...`); the regression tests therefore compiled but did not run. `rustfmt --edition 2021 --check crates/services/ogc-network-sys/src/lib.rs` reports the pre-existing crate-level `compile_error!` formatting difference. No hardware verification was performed. `/root/reviewer_4` owns the current package re-review. The confirmed `net_init_async` difference remains open; the `SockLenT` parity fix is documented below.

## Independent re-review: `if_configex` retry-count parity

Reviewer: `/root`. Implementer: `worker_b`.

The helper matches C's fixed 32 attempts, ignores `max_retries`, retries only `-EAGAIN` and `-ETIMEDOUT`, and waits 50 ms after every retryable result, including the last attempt (`network_wii.c:1180–1203`). The tests cover retry-argument values, retryable exhaustion, and early exit on a non-retryable result. The fix is accepted for this finding. The `net_init_async` mismatch and async-init lifecycle questions remain open, so the crate stays in `finding`.

PowerPC test compilation passed, but the tests did not run because the host linker cannot link the PowerPC test binary. No hardware verification was performed.


## Fix pass: `SockLenT` unsigned C parity

**BUG FIX.** Implementer: `worker_b`. Independent reviewer: `/root/reviewer_4` (pending). The public `SockLenT` alias is now `u32`, matching `gc/network.h`. This changes the socket length arguments and pointers in both `ogc-network-sys` and the re-exported `ogc-network` API. The Wii backend converts scalar lengths to its internal signed lwIP interface with checked `i32::try_from`; values above `i32::MAX` return `-EINVAL` instead of wrapping. For output length pointers, the backend casts between same-width `u32` and `i32` pointers inside the existing unsafe boundary; both types accept every 32-bit bit pattern.

Added a regression test that requires `SockLenT` to represent `u32::MAX`. PowerPC test compilation passed for `ogc-network-sys`; `cargo check -p ogc-network --target powerpc-unknown-linux-gnu` passed. The high-level crate's existing test build currently fails on unrelated pre-existing test mismatches (callback return signatures, `FdSet` array lengths, and C string pointer types), so that test suite was not validated. Tests were compiled but not run. No hardware verification was performed. This fix is included in `/root/reviewer_4`'s package re-review.

## Fix handoff: asynchronous `net_init_async`

Implementer: `worker_c`. Independent reviewer: `/root/reviewer_4` (review pending).

`net_init_async` now reserves the initialization lock and schedules the existing synchronous network initialization sequence on a dedicated LWP worker. The public call returns after the worker is created instead of waiting for `net_init()` to finish. On completion, the worker stores the final network status, calls the supplied callback once with that result and the original userdata, ignores the callback's return value as C does, then releases the initialization lock. If the stack is already initialized, the call returns zero without invoking the callback. If another initialization is in progress, it returns `-EBUSY` without invoking the callback. Completed worker threads are joined through LWP's deferred callback queue; a pending join is retained for cleanup if that queue cannot accept the reaper.

This preserves the public asynchronous return and completion-callback contract while reusing the existing Rust initialization flow. It uses a Rust LWP worker rather than C's chain of IOS callbacks. Worker scheduling, overlapping `net_deinit`, and IOS timing need independent static review and hardware investigation where relevant; this change does not claim hardware validation.

Regression coverage verifies that completion forwards the result and userdata and ignores the callback return value. `cargo fmt -p ogc-network-sys`, `cargo check -p ogc-network-sys --tests --target powerpc-unknown-linux-gnu`, and `cargo check -p ogc-network --target powerpc-unknown-linux-gnu` passed. `cargo test -p ogc-network-sys --lib --target powerpc-unknown-linux-gnu --no-run` could not link because the host linker rejects PowerPC options. The test compiled but did not run. No hardware verification was performed.

## Independent re-review: `SockLenT` and asynchronous initialization

Reviewer: `/root/reviewer_d`. Implementers: `worker_b` (`182dea9`, `Match network socket length type to C`) and `worker_c` (`3d70dab`, `Make network initialization callback asynchronous`). Reviewed the current source after subsequent changes, including `4aa6399`. Scope: public socket length ABI and async initialization start/completion, status, and deinitialization interaction. Other network findings remain open.

### Result: NEEDS_CHANGES

#### `SockLenT`: PASSED

C defines `socklen_t` as `u32` and declares each socket length argument and output pointer with that type (`../../C-sources/libogc/gc/network.h:107-109,152-166`). Rust exports `SockLenT = u32` (`crates/services/ogc-network-sys/src/lib.rs:91`) and the high-level `ogc-network` crate re-exports it (`crates/services/ogc-network/src/lib.rs:11`). The public bind, accept, connect, sendto, recvfrom, getsockopt, setsockopt, and getsockname signatures use the alias consistently (`ogc-network-sys/src/lib.rs:1788-1950`; high-level counterparts at `ogc-network/src/lib.rs:481-971`). The current Wii backend passes the same-width unsigned type to `lwip_sys`, whose own `SockLenT` is `u32` (`crates/services/lwip-sys/src/lib.rs:23`); scalar arguments and in/out pointers therefore preserve C's width and signedness without truncation. The target tests assert the public type can represent `u32::MAX` and check representative function signatures (`ogc-network-sys/src/lib.rs:1623-1632`; `ogc-network/src/lib.rs:1228-1232`). The C implementation in `network_common.c` contains no socket-length or async-init implementation; those contracts are in `network.h` and `network_wii.c`.

#### `net_init_async`: callback path passes; deinitialization race remains

C's `net_init_async` returns zero if the top descriptor is already open, returns `-EBUSY` if `_init_busy` is set, and otherwise marks initialization active and starts its callback chain (`../../C-sources/libogc/libogc/network_wii.c:511-555`). At completion the C chain stores `_last_init_result`, calls the callback with `data->result` and the original `usrdata`, ignores its return, frees context resources, then clears `_init_busy` (`network_wii.c:495-508`). Rust follows the same key observable results: it returns zero without callback for an already initialized top fd, returns `-EBUSY` when its lock is held, sets `NET_STATUS=-EBUSY` before starting, and returns zero after successful worker creation (`crates/services/ogc-network-sys/src/lib.rs:809-848`). Its worker obtains the final result through `net_init_locked`, delivers that result and original userdata exactly once when a callback is present, ignores the callback return, then releases the init lock (`:779-799`). Thus the callback arguments and completion ordering against the initialization lock match C's completion contract for the implemented path. The target regression only tests the callback helper's argument forwarding/ignored return (`:731-762`); it does not exercise the actual worker lifecycle.

The nearby deinit path does not match and can leave Rust initialized after `net_deinit` returns. C sets `_init_abort` when initialization is active and waits until `_init_busy` becomes false before closing the top descriptor and setting the status to `-ENETDOWN` (`../../C-sources/libogc/libogc/network_wii.c:273-282,588-600`). The C callback chain checks `_init_abort` before each state transition and routes the in-progress sequence through cleanup (`network_wii.c:279-282,477-486`). Rust's worker calls the complete blocking `net_init_locked` sequence without checking a cancellation flag (`crates/services/ogc-network-sys/src/lib.rs:779-782`; init retries can loop in `:619-624,694-710`). Rust `net_deinit` does not acquire or wait on `NET_INIT_LOCK`, request abort, or join the worker: it runs cleanup, swaps the current top fd, clears lwIP's fd and status, and returns (`:917-934`). If deinit runs while init is still retrying before the top fd is stored, init can continue afterward and store a live fd at `:712-715`, leaving the process initialized after deinit returned. If deinit races after the fd is stored but before the worker's callback, the callback can report success after deinit has closed the fd and reset status. C explicitly coordinates both cases with its abort/wait path. This is a concrete async-init lifecycle mismatch; the crates remain open.

Validation: `cargo check -p ogc-network-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled its target-gated regression tests; those tests did not run. `cargo check -p ogc-network --target powerpc-unknown-linux-gnu` passed. `cargo test -p ogc-network-sys` was attempted on the host but cannot run because the crate intentionally emits a compile error on non-PowerPC targets. No hardware verification was performed. `git diff --check` passed for the source changes reviewed. Ledger not edited; coordinator owns the ledger update.

## BUG FIX handoff: abort and wait during network initialization teardown

Implementer: `worker_c`. Independent re-review is needed; the review thread handles reviewer assignment and review. Scope: cancellation coordination between `net_init`, `net_attach_active`, `net_init_async`, and `net_deinit` in `ogc-network-sys`, plus the high-level `ogc-network` delegation. The public callback contract and socket API were not changed.

Network init now claims its active state under an entry gate shared with deinit. `net_deinit` closes that gate, sets an abort request when initialization is active, and waits until the initialization lock is released before cleanup and closing the top descriptor. New init calls cannot start after deinit claims the gate. The init sequence checks the abort flag before and after blocking opens and ioctls, stops retry loops at the next cancellation checkpoint, and closes any temporary descriptors on cancellation. It checks once more before publishing the top descriptor. If deinit races with that final check and publication, it still waits for the init lock, then removes and closes the published descriptor before returning. The worker completes its status update and callback before it releases the lock, so it cannot publish a live top fd or alter status after deinit cleanup completes.

Added deterministic tests for cancellation during an injected init step, cleanup of that step's returned resource, skipping the next step, and the abort-and-wait sequence used by deinit. `cargo fmt -p ogc-network-sys -- --check`, `cargo check -p ogc-network-sys --target powerpc-unknown-linux-gnu --lib`, and `cargo check -p ogc-network-sys --target powerpc-unknown-linux-gnu --tests` passed. The target test code compiled. `cargo test -p ogc-network-sys --lib --target powerpc-unknown-linux-gnu --no-run` reached linking but could not link because the host linker rejects PowerPC options (`ld: unknown options: --as-needed -Bstatic ...`); tests were not run. `git diff --check` passed. No hardware verification was performed.

The worker still uses a Rust LWP thread around blocking IOS operations rather than C's asynchronous IOS callback chain. This fix covers cancellation and teardown ordering; it does not establish identical IOS scheduling or timing. Other recorded network API findings and differences remain open.

## Independent re-review — initialization retry and status audit

Reviewer: `/root/reviewer_e`. Scope: current `ogc-network` and `ogc-network-sys` behavior beyond the active-attach descriptor leak and deinit status finding already recorded above. Static source comparison only.

Rust's host-ID initialization loop has one fewer attempt and one fewer delay than C. C starts with 100 retries and checks for exhaustion before decrementing, so it permits 101 `IOCTL_SO_GETHOSTID` attempts and 100 waits of 100 ms (`C-sources/libogc/libogc/network_wii.c:424–475`). Rust loops over 100 attempts and waits only when another attempt remains, giving 100 attempts and 99 waits (`crates/services/ogc-network-sys/src/lib.rs:372, 861–888`). It can time out one attempt and 100 ms earlier.

The initial status value also differs. C initializes `_last_init_result` to `-ENETDOWN` and returns it from `net_get_status` (`network_wii.c:197, 584–586`). Rust initializes `NET_STATUS` to `SOCKET_ERROR` (`-1`) and returns it before initialization (`ogc-network-sys/src/lib.rs:17, 342, 1081–1082`). The previously recorded deinit status difference remains: C stores `-ENETDOWN`, Rust stores `-1` (`network_wii.c:597–599`; Rust `lib.rs:1166`).

The `net_gethostip` difference remains as an uncertain convenience change: C returns zero when this process has no `net_ip_top_fd`, then polls its existing descriptor (`network_wii.c:247–263`); Rust opens `/dev/net/ip/top` itself and may return an address before this process calls init/attach (`lib.rs:1178–1203`). Rust `net_init_async` no longer calls `net_init` synchronously. It forwards the result and userdata once and runs the callback before releasing the init lock (`lib.rs:1013–1033, 1043–1078`), matching C's public return and callback ordering (`network_wii.c:495–559`). The worker still uses synchronous IOS operations and busy waits where C chains async IOS calls and alarms; source-level scheduling differs, but target behavior was not measured.

No additional temporary descriptor leak was found in ordinary `net_init_sequence` error or cancellation branches in this audit. The active-attach host-ID negative-return leak remains the separate confirmed finding. `if_configex` retry count and the `SockLenT` fix remain aligned. This audit ran no checks or tests; earlier target tests only compiled and did not run. No hardware verification was performed.

## Independent re-review of cancellation/teardown fix `b3a868b`

**Result: PASS for the bounded fix.** Reviewer: `/root/reviewer_d`; implementer: worker_c.

C sets the abort flag, waits for initialization to finish, then closes the top descriptor and stores `-ENETDOWN` (`C-sources/libogc/libogc/network_wii.c:588-600`). Rust serializes init/deinit, requests cancellation, waits for the init lock, then cleans up and closes (`crates/services/ogc-network-sys/src/lib.rs:442-509,1148-1168`). Cancellation checks close temporary descriptors returned after abort and prevent publication after cancellation (`lib.rs:673-719,729-898`). The fix preserves status ordering and avoids post-teardown publication. The helper tests do not exercise the complete worker/deinit race or descriptor lifecycle.

Two separate mismatches remain: active attach returns on a negative host ID without closing `ip_top` (`lib.rs:691-701`), and deinit stores `SOCKET_ERROR` (`-1`) where C stores `-ENETDOWN` (`network_wii.c:597-599`; Rust `lib.rs:1166`). Validation: PowerPC checks for both crates passed; tests compiled but did not run. No hardware verification was performed.

## Fix pass: active-attach temporary descriptor leak

**BUG FIX.** Implementer: `worker_c`. Independent re-review: needed. `net_attach_active_sequence` now closes its temporary `/dev/net/ip/top` descriptor when `IOCTL_SO_GETHOSTID` returns a negative result, then returns that same error. This matches the cleanup behavior used by the regular network initialization path. Added a regression test for negative-error recognition and preservation. The separate deinit status mismatch remains open.

Validation: pending. Tests have not run. No hardware verification was performed.

### Independent re-review of active-attach cleanup (`b1f7fcb`)

**Result: NEEDS CHANGES for adjacent timeout cleanup.** Reviewer: `/root/reviewer_e`; implementer: `/root/worker_c`.

The targeted negative-host-ID path passes: Rust closes the temporary descriptor if the host-ID ioctl returns a negative result or the cancellable step errors (`crates/services/ogc-network-sys/src/lib.rs:701-727`), matching C's error cleanup (`C-sources/libogc/libogc/network_wii.c:477-486`). The helper test verifies the negative result closes and preserves the error (`lib.rs:2347-2361`). However, when all five host-ID attempts return zero, Rust returns `-ETIMEDOUT` without closing the descriptor (`lib.rs:701-736`). `FileDescriptor` has no `Drop` cleanup (`crates/core/ogc-ipc/src/lib.rs:221-245`); C timeout cleanup closes the top descriptor (`network_wii.c:451-485`). This adjacent path still leaks the descriptor.

The separate deinit status mismatch also remains: C stores `-ENETDOWN`, while Rust stores `SOCKET_ERROR` (`-1`; `network_wii.c:597-600`; Rust `lib.rs:1181-1195`). Validation: `cargo check -p ogc-network-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled tests; tests did not run. The combined check with `ogc-network` failed on pre-existing test compilation errors. No hardware verification was performed.

## Fix pass: active-attach timeout descriptor cleanup

**BUG FIX.** Implementer: `worker_c`. Independent re-review: needed. If all five `IOCTL_SO_GETHOSTID` attempts return zero, Rust now closes the temporary `/dev/net/ip/top` descriptor before returning `-ETIMEDOUT`, matching C's timeout error cleanup (`C-sources/libogc/libogc/network_wii.c:451-485`). Added a focused regression test that checks timeout error preservation and descriptor cleanup; a non-timeout host ID leaves the descriptor open. The separate deinit status mismatch remains open.

Validation: pending. Tests have not run. No hardware verification was performed.

### Independent re-review of active-attach timeout cleanup (`2ae29c3`)

**Result: PASS for the timeout cleanup fix.** Reviewer: `/root/worker_b`. Implementer: `/root/worker_c`.

C's network initialization error path closes `net_ip_top_fd`, clears the global descriptor, and then completes with the stored timeout error (`../../C-sources/libogc/libogc/network_wii.c:451–485`). Rust's active-attach loop now closes its temporary `/dev/net/ip/top` descriptor when the fifth host-ID query still returns zero, then returns `-ETIMEDOUT`; a nonzero host ID leaves the descriptor available for the rest of the attach sequence (`crates/services/ogc-network-sys/src/lib.rs:732–759`). The regression verifies both timeout cleanup and preservation of the non-timeout descriptor (`:2394–2412`). This resolves the reported active-attach leak on host-ID timeout. The independent `net_init` flow still differs in broader lifecycle behavior; the existing async-init race, deinit status, and other network findings remain open.

No checks or tests were run during this independent pass. The fix handoff's validation is still marked pending, and the regression has not been run per the note. No hardware verification was performed.


## Implementer handoff: deinit status parity

Implementer: `thread-current/root`. Independent reviewer: pending assignment. `net_deinit` now stores `-ENETDOWN` after cleanup, matching C's `_last_init_result` after network teardown (`../../C-sources/libogc/libogc/network_wii.c:597-600`). The PowerPC target regression checks the public status after deinit. Other network findings remain open, including API error parity. Independent static re-review is required.

Validation: `cargo check -p ogc-network-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled test code; tests did not run. No hardware verification was performed.

### Independent re-review: deinit status parity (`b69ca98`)

**Result: PASS for deinit status.** Reviewer: `reviewer_alpha`. Implementer: `thread-current/root`.

C stores `-ENETDOWN` as the final network status after `net_deinit`; Rust now stores the same result after cleanup and descriptor teardown. The added PowerPC-only assertion checks the public getter after deinit. The test code compiled but was not run. Other network API and lifecycle differences remain open.

## Implementer handoff: `net_gethostip` descriptor ownership parity

Implementer: `thread-current/root`. Independent reviewer: pending. `net_gethostip` now returns zero when this process has no initialized `/dev/net/ip/top` descriptor, polls the existing descriptor up to five times, waits after zero results, and returns the first nonzero ioctl result using C's `u32` conversion. It no longer opens and closes a separate descriptor, matching C's process-owned descriptor behavior (`../../C-sources/libogc/libogc/network_wii.c:247-263`). Regression coverage checks no-descriptor behavior, zero-result retry, and preservation of a negative nonzero ioctl result. Other network API and lifecycle findings remain open.

Validation: `cargo check -p ogc-network-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled the regression; tests did not run. No hardware verification was performed.

### Independent re-review of host-IP query timing (`a51f310`)

**Result: NEEDS_CHANGES.** Reviewer: `reviewer_alpha`. Implementer: `thread-current/root`.

The descriptor ownership, retry count, result conversion, and first-nonzero result match C. The wait after zero results initially busy-spun for 100 ms, while C calls `usleep(100000)` and yields the thread. The wait now uses the LWP timed-sleep path for the same 100 ms duration. The regression also checks all five zero-result attempts and waits.

## Follow-up handoff: yield during host-IP retry delay

Implementer: `thread-current/root`. Independent reviewer: pending. `wait_network_startup_retry` now calls `lwp::sleep_ticks` instead of spinning on the timebase. This yields the LWP thread during the 100 ms delay, matching C `usleep(100000)`. The helper test checks five queries and five waits when every result is zero. Other network API and lifecycle findings remain open.

Validation: `cargo check -p ogc-network-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled target test code; tests did not run. No hardware verification was performed.

### Independent re-review: yielding host-IP retries (`46d56f6`)

**Result: PASS for host-IP retry timing.** Reviewer: `reviewer_alpha`. Implementer: `thread-current/root`.

C calls `usleep(100000)` after every zero host-ID result, including the fifth. Rust now waits 100 ms with `lwp::sleep_ticks`, which yields the LWP thread. The regression verifies five queries and five waits on five zero results. The descriptor, return-value, and retry changes from `a51f310` also pass this bounded comparison. Other network findings remain open. The target test code compiled but was not executed.

## Implementer handoff: initial network status

Implementer: `thread-current/root`. Independent reviewer: pending. `NET_STATUS` now initializes to `-ENETDOWN` (`-100`), matching C's `_last_init_result` before initialization (`../../C-sources/libogc/libogc/network_wii.c:197,584-586`). A target regression checks the documented initial value. `cargo check -p ogc-network-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled the target test. `cargo test -p ogc-network-sys --lib --target powerpc-unknown-linux-gnu --no-run` reached linking but failed because the host linker rejects PowerPC options; tests did not run. No hardware verification was performed. Other network findings remain open.

## Implementer handoff: host-ID retry boundary

Implementer: `thread-current/root`. Independent reviewer: pending. The regular initialization path now allows 101 host-ID queries and waits after each of the first 100 zero results, matching C's initial retry count of 100 (`../../C-sources/libogc/libogc/network_wii.c:424-475`). Production and regression coverage use the same retry loop. Its tests check 101 attempts and 100 waits on timeout, plus early completion after a nonzero result. PowerPC test compilation passed; tests did not run because the host linker rejects PowerPC options as recorded above. No hardware verification was performed. Other network findings remain open.

### Independent re-review: initial status and host-ID retry boundary (`af2437f`, `6e6fc31`)

**Result: PASS for the initial status and retry boundary.** Reviewer: `reviewer_alpha`. Implementer: `thread-current/root`.

C initializes `_last_init_result` to `-ENETDOWN` and returns that value before initialization. Rust initializes `NET_STATUS` to the matching `INITIAL_NET_STATUS` (`-100`). C sets `retries` to `MAX_IP_RETRIES` (100), issues the first host-ID query, and retries a zero result while decrementing the counter; this produces 101 queries and 100 waits before timeout. Rust's shared `retry_network_ip` loop likewise performs 101 queries and 100 waits, returns `-ETIMEDOUT` after all-zero results, and returns early on a nonzero result. The bounded retry and initial status changes match C.

Validation evidence: the review note records that `cargo check -p ogc-network-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled the tests. A `cargo test ... --no-run` attempt reached linking but failed because the host linker rejects PowerPC options. Tests did not run. No hardware verification was performed. Other network findings remain open.

### Independent re-review: bind error preservation (`1c91265`, `4cda67c`)

**Result: PASS for the bounded `ogc-network::bind` behavior; the network crates remain open.** Reviewer: `reviewer_gamma`. Implementer: `thread-current/root`.

Rust now returns a negative `ogc-network-sys::net_bind` result unchanged and maps nonnegative results to zero (`crates/services/ogc-network/src/lib.rs:483-495`). The C Wii `net_bind` returns its converted IOS bind result directly (`../../C-sources/libogc/libogc/network_wii.c:743-760`), so preserving a specific negative result such as `-EADDRINUSE` is consistent with that API. The DOL probe opens two TCP sockets, tries candidate ports until the first bind succeeds, checks that binding the second socket to the same wildcard address and port returns `-98`, records whether it found a conflict, and records both close results (`probes/ogc-system-probe/src/network_probe.rs:176-230`).

`cargo check -p ogc-network --tests --target powerpc-unknown-linux-gnu` and `cargo check -p libdb-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled target unit-test code; tests did not run. The probe source compiled, but the DOL could not link: `ogc-sysbase` references unresolved `AddDevice`, `FindDevice`, and `RemoveDevice` symbols. Linking stock `libsysbase.a` resolves those symbols but causes duplicate definitions of `devoptab_list` and `__getreent`. The DOL was not produced or executed. No hardware verification was performed. Other network API and lifecycle findings remain open.

## Implementer handoff: preserve bind backend errors

Implementer: `thread-current/root`. Independent static re-review required.

The public `ogc-network::bind` wrapper now returns negative results from `ogc-network-sys::net_bind` unchanged, while mapping nonnegative results to zero. C returns the underlying bind result, so this preserves specific errors for callers such as `libdb-sys::tcpip_bind`. Regression coverage runs the same result helper used by the production wrapper with `UIP_ERR_USE` (`-10`) and success. This change is paired with the `libdb-sys` bind-error handoff in `docs/review/libogc.md`.

Primary regression: the network DOL probe attempts two TCP binds to the same candidate port and checks for the raw C-compatible address-in-use error. Its source compiled, but linking is blocked by unresolved newlib `AddDevice`/`FindDevice`/`RemoveDevice` symbols in `ogc-sysbase`; linking stock `libsysbase.a` instead duplicates `devoptab_list` and `__getreent`. The DOL was not produced or executed. PowerPC target checks for the two test suites passed and compiled their unit tests; tests did not run. Formatting passed. No hardware verification was performed.

## 2026-10-06 source-status reconciliation

The concrete network blockers recorded in this note are closed in the current source: asynchronous callback delivery, init/deinit abort-and-wait ordering, active-attach descriptor cleanup, deinit and initial status, process-owned host-IP lookup and retry timing, the 101-attempt initialization boundary, unsigned `SockLenT`, fixed `if_configex` retry behavior, and bind error propagation all have implemented fixes and bounded review results. The worker still uses a Rust LWP thread around blocking IOS operations instead of C's IOS callback chain, but no remaining externally observable mismatch has been demonstrated from that structural difference. `ogc-network` and `ogc-network-sys` move to `needs_re_review` for runtime and independent validation.

## 2026-10-07 `net_deinit` cleanup parity

`ogc-network-sys::net_deinit` previously called `net_wc24cleanup` and returned that operation's error before the IP-top descriptor close result. C `net_deinit` only aborts pending initialization, closes the IP-top descriptor, and resets network status (`../../../../C-sources/libogc/libogc/network_wii.c:588-600`); NWC24 cleanup is a separate API (`net_wc24cleanup`, lines 602-611). The extra Rust request could therefore make Wiiload shutdown report an unrelated cleanup error such as `IPC_ENOMEM` (`-22`). Rust now follows C's deinit sequence and preserves the IP-top close error. Callers that require NWC24 cleanup can still call `net_wc24cleanup` explicitly. The release HBC DOL completed its Dolphin startup scan after this change and displayed the expected no-storage notice. The Wiiload worker's cleanup-error value was not directly inspected in that run, and Wii hardware verification remains pending.

## 2026-10-08 socket startup during IP retries

Source comparison found that C repeats `IOCTL_SO_STARTUP` before each `IOCTL_SO_GETHOSTID` query when the host ID is zero (`../../C-sources/libogc/libogc/network_wii.c`, states 9 and 10). Rust previously performed socket startup once, then queried only the host ID during its 101 attempts. The production initialization sequence now uses `startup_network_ip`, which repeats startup before each query and reuses `retry_network_ip` for the existing attempt and wait limits. Both IOS operations retain cancellation checks. Startup or query failures stop the sequence and enter the existing descriptor cleanup path. Active attachment and standalone host-IP lookup are unchanged.

Two regressions check the exact startup/query/wait order until address acquisition, IOS startup failure, cancellation failure, and query failure. These exact source helpers and tests passed in a temporary host harness (two tests); the harness does not exercise IOS or replace the production backend. `cargo check -p ogc-network-sys -p ogc-network --tests --target powerpc-unknown-linux-gnu` passed, compiling the full target test code. A pre-existing missing unsafe block in the sys test callback was corrected to permit this check. Source-structure audit passed. Hardware verification remains pending; this difference is not yet proven to cause HBC's reconnect failure. No DOL was uploaded and no Wii state was changed.

The broader `cargo xtask audit-stubs` and inventory refresh are blocked by an existing stale evidence reference for `ogc-network::bind_addr` at `src/lib.rs:674`. This reference concerns the already-modified high-level crate, not the new startup retry helper. The checks were not suppressed or marked as passing. `git diff --check` passed.

## 2026-10-14 std TCP, UDP, DNS, and deadline adapters

The pinned std connection selector now selects `sys/net/connection/wii.rs` for
Wii. Its application provider in `ogc-std-runtime::net` uses the production IOS
socket service in `ogc-network-sys`. The provider initializes the network stack
on first use and retains a network lease while sockets exist; `net_deinit`
returns `-EBUSY` when a socket lease is live. Applications can explicitly attach
to an active IOS network stack before first use.

The adapter supports IPv4 TCP and UDP, actual byte counts, partial TCP I/O, EOF,
local and peer addresses, shutdown, and owned DNS answer lists. Numeric IPv4
addresses bypass DNS. DNS replies are decoded from the IOS hostent buffer into
owned addresses, with pointer bounds and terminators checked. The C/Newlib
hostent ABI uses 16-bit `h_addrtype` and `h_length`; the Rust declaration now
matches it. IOS replies and IPC submission failures have separate error domains,
so only real IOS `EAGAIN` and `EINTR` results are retryable.

The adapter sets each socket to IOS nonblocking mode and checks both fcntl
results before publishing the requested std state. Read and write timeouts keep
one absolute deadline while polling in slices no longer than 25 ms. The clock
service, poll result, partial I/O result, timeout, and close errors all stay
observable. Clones share one owned socket and close the IOS descriptor on the
final drop. Because IOS `SO_ERROR`/connection completion queries are not
implemented, `connect_timeout`, `take_error`, and option getters that IOS cannot
serve return Unsupported. IPv6 and multicast APIs return Unsupported.

The first target build caught and corrected a Wii-specific socket option value:
`IP_TTL` is 4 in devkitPro's `netinet/in.h`. Listener, stream, and UDP setters now
share one checked mapping. A final socket close failure is recorded and stops
the infallible Drop path with the real error.

The pinned std source check passes. Host tests pass for the production packet,
DNS parser, error-domain, nonblocking retry, and absolute-deadline engine. The
std network probe DOL builds and links; it exercises concurrent DNS, TCP, clone
ownership, and a timed UDP receive on hardware. No Wii execution is claimed.
