# lwip-sys Static Review

Date: 2026-10-01  
Reviewer: `/root/reviewer_4`  
Status: first-pass review found confirmed socket behavior and type mismatches; crate remains open.

## Scope and sources

Compared `crates/services/lwip-sys/src/lib.rs` and its manifest with the C Wii socket layer in `../../C-sources/libogc/libogc/network_wii.c`, declarations and constants in `../../C-sources/libogc/gc/network.h`, and BSD socket flag definitions in `../../C-sources/libogc/external/bsd/include/sys/socket.h`. Also inspected nearby `docs/review/libnetwork.md` and the Rust caller in `crates/services/ogc-network-sys/src/lib.rs`. The prior review note records that the underlying lwIP/IPC layer was not reviewed there; this pass checks the lwip-sys socket backend paths below.

## Confirmed findings

### Socket creation does not match C flags or default receive buffer

- C defines `SOCK_CLOEXEC` as `0x10000000` and `SOCK_NONBLOCK` as `0x20000000` (`external/bsd/include/sys/socket.h:80-86`). C `net_socket` removes those exact bits before sending the socket type to IOS, then applies `SOCK_NONBLOCK` through `F_GETFL`/`F_SETFL` and sets `SO_RCVBUF` to 32768 after socket creation (`libogc/network_wii.c:701-723`).
- Rust instead sets `FLAG_MASK` to `0x800 | 0x80000` (`crates/services/lwip-sys/src/lib.rs:590-607`) and passes `(type_ as u32) & !FLAG_MASK` to IOS (`lib.rs:962-964`). Those bits do not match the C creation flags. Rust's socket path then only allocates local tracking state (`lib.rs:965-972`); it does not apply `F_GETFL`/`F_SETFL` for a nonblocking request or call `setsockopt(SO_RCVBUF, 32768)`. The local `DEFAULT_RCVBUF` value only initializes tracking state (`lib.rs:160-169,212-214`); it does not configure IOS. Thus C nonblocking socket requests are not preserved, and C's explicit receive-window configuration is missing.

### Error handling returns different values

- C `net_socket` returns `-ENXIO` when its network top descriptor is unavailable (`network_wii.c:701-712`). Rust `top_fd()` represents a missing descriptor as `Err(-1)` and `ioctl_u32s()` turns that into `-1`; `socket()` returns that result (`crates/services/lwip-sys/src/lib.rs:764-768,777-793,962-972`).
- For address validation, C `net_bind` and `net_connect` return `-EAFNOSUPPORT` for a non-IPv4 family, and `net_connect` returns `-EINVAL` for a short address (`network_wii.c:743-759,808-826`). Rust `sockaddr_in_to_wii()` returns `None` for those cases (`lib.rs:921-929`), and Rust `bind`/`connect` convert that to `-1` (`lib.rs:1000-1012,1127-1130`). The returned error codes therefore differ even though both reject the invalid input.

### `SockLenT` signedness differs from the public C declaration

- C `gc/network.h` defines `socklen_t` as `u32` and uses it in `net_bind`, `net_connect`, and related declarations (`network.h:107-109,150-170`). Rust declares `SockLenT = i32` (`crates/services/lwip-sys/src/lib.rs:22-23`). The width is the same, but the source/API signedness differs. Rust's `namelen < sizeof(...)` checks reject negative values (`lib.rs:921-923`), whereas the C `u32` parameter receives the same bit pattern as a large unsigned length; this changes observable validation behavior for callers that pass a negative length.

### `FIONREAD` reports fabricated availability

- C `net_ioctl` implements only `FIONBIO`; its default branch returns `-EINVAL` for `FIONREAD` (`network_wii.c:1093-1109`).
- Rust's `connect` stores a constant value of 4 in `SOCKET_RX_AVAILABLE` after a successful connection (`crates/services/lwip-sys/src/lib.rs:1143-1148`), and `ioctl(FIONREAD)` reports that value as available bytes (`lib.rs:1475-1484`). No IOS readiness or receive-queue query updates this value. This reports four readable bytes immediately after connect, even before data arrives, instead of C's unsupported-operation result.

## Validation and handoff

No implementation code changed. No builds or tests were run. `lwip-sys` remains in `finding` with `/root/reviewer_4` as owner. The shared ledger already had staged edits from other work, so only this review note is staged; the `lwip-sys` ledger row remains unstaged and no commit was created.

## 2026-10-02 ABI parity follow-up

Compared again with current `gc/network.h` and `libogc/network_wii.c`.

- `lwip-sys::SockLenT` is now `u32`, matching C `socklen_t` exactly.
- The `ogc-network-sys` forwarding layer now passes scalar and pointer socket lengths as `u32` without an intermediate signed conversion or `i32::MAX` rejection.
- Adjacent socket ABI layouts used at this boundary (`InAddr`, `SockAddr`, `SockAddrIn`, `Linger`, `FdSet`, and `TimeVal`) were rechecked against the current libogc/lwIP headers; no additional width or signedness change was required.
- The earlier socket-flag, post-create receive-buffer/nonblocking, error-code, bind-length, and `FIONREAD` fixes remain present.

This closes the documented `socklen_t` source/API signedness mismatch. IOS and hardware behavior still needs target validation.

## Independent re-review of lwip-sys socket fixes

Reviewer: `reviewer_alpha`. Implementers: `worker_c` and `thread-current/root`.

**Result: NEEDS_CHANGES.** The bounded socket/error fixes recorded in `liblwip-sys-audit.md` match the inspected C paths, including the final `sin_len` write-back. One address-boundary issue remains: C's Wii sockaddr is 8 bytes, and C accepts and copies an 8-byte connect address; Rust checks for that 8-byte length but then reads the 16-byte public `SockAddrIn`. Rust `bind` also reads a 16-byte `SockAddrIn`, while C copies only the first 8 bytes. This can read beyond a valid 8-byte caller buffer. The existing tests use a 2-byte connect buffer for the too-short case and full-size address structs for success, so they do not cover this boundary. See the linked sys audit for source references and test evidence.

PowerPC target test code compiled according to the implementation handoffs, but tests did not run. The available `network_probe.dol` build did not link, so there is no DOL regression result for these fixes. No hardware verification was performed. Other socket API differences remain open.

## BUG FIX handoff: 8-byte Wii sockaddr boundary

Implementer: `thread-current/root`. Independent static re-review required. The Wii backend now reads bind/connect inputs as the 8-byte `WiiSockAddrIn`, matching C's copy size and avoiding a 16-byte read from an 8-byte caller buffer. `bind` rejects a `namelen` shorter than the eight-byte structure with `-EINVAL` as a memory-safety guard; C ignores this length and can read past a short input. The DOL source adds 8-byte bind and connect cases. The DOL could not be linked or run because `ogc-sysbase` has unresolved native devoptab symbols, as described in `docs/review/libogc.md`. Other socket differences remain open.

### Independent re-review of 8-byte bind/connect buffers (`ef6b5e6`)

**Result: PASS for the buffer-boundary fix.** Reviewer: `reviewer_alpha`. Implementer: `thread-current/root`.

Rust now reads exactly the 8-byte Wii sockaddr for bind and connect, matching C's `wii_sockaddr_in` and avoiding the previous 16-byte read. The source also checks the minimum length before reading, canonicalizes the local family/length fields, and writes caller `sa_len=8` before the IOS operation. The stricter short-bind rejection is an intentional safety difference because C ignores `namelen` before its 8-byte copy. The DOL source passes exact 8-byte buffers to both APIs. PowerPC test code compiled but tests did not run. The DOL could not link or run because `ogc-sysbase` has unresolved native devoptab symbols. Other socket findings remain open.
