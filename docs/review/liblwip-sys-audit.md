# `lwip-sys` independent findings audit

## 2026-10-11 Accept output address initialization

C `net_accept` writes the eight-byte Wii address length and IPv4 family before
issuing `IOCTL_SO_ACCEPT` (`C-sources/libogc/libogc/network_wii.c:781`). The Rust
backend supplied a zero-filled `WiiSockAddrIn`. It now constructs the IOS output
address with length 8 and family 2 before the production ioctl. The byte regression
uses the exact production constructor and independently expects
`[8, 2, 0, 0, 0, 0, 0, 0]`; it passed in an extracted host harness. Full target
test execution was not performed.

This is a confirmed C/Rust input-contract difference. Correcting it alone did not
resolve the reproduced Dolphin host stop. The separate network polling descriptor
fix resolved that stop; see `docs/review/libnetwork.md` for the real TCP regression
and remaining Wii verification. No hardware parity completion is claimed.

- Reviewer: `/root/reviewer_5`
- Original reviewer: `/root/reviewer_4`
- Implementer: none; this is an independent audit of the existing first-pass findings
- Sources: `../../C-sources/libogc/libogc/network_wii.c`, `../../C-sources/libogc/gc/network.h`, `../../C-sources/libogc/gc/lwip/arch.h`, `../../C-sources/libogc/external/bsd/include/sys/socket.h`, and `crates/services/lwip-sys/src/lib.rs`
- Validation: static source inspection only. No builds, tests, or hardware verification were performed.

I independently checked the existing findings in `docs/review/liblwip.md` against C and Rust implementations. All five finding groups below are confirmed.

## Confirmed findings

### Socket type flags and default receive buffer

C defines `SOCK_CLOEXEC=0x10000000` and `SOCK_NONBLOCK=0x20000000` (`external/bsd/include/sys/socket.h:80-86`). `net_socket` masks these bits before its IOS socket ioctl, applies nonblocking mode with `F_GETFL`/`F_SETFL` when requested, and configures `SO_RCVBUF` to 32768 on every successful socket (`network_wii.c:699-723`).

Rust's Wii backend masks `FLAG_MASK=0x800 | 0x80000` instead (`lwip-sys/src/lib.rs:590-594,962-964`). It does not apply the `SOCK_NONBLOCK` request after creating the IOS socket; its socket allocation marks nonblocking false and only initializes local option tracking (`:703-721`). `DEFAULT_RCVBUF=32768` initializes the Rust tracking value (`:160-169,213-215`), but `socket` does not send a default `SO_RCVBUF` option to IOS (`:962-972`). The tracking default does not reproduce C's underlying socket receive-buffer configuration.

### Error values and validation behavior

When the network top descriptor is absent, C `net_socket` returns `-ENXIO` (`network_wii.c:701-712`; `ENXIO` is 6 in `gc/lwip/arch.h:66`). Rust's missing-descriptor path returns `-1` from `top_fd`/`ioctl_u32s`, which `socket` returns (`lwip-sys/src/lib.rs:764-793,962-972`).

For a valid address pointer with a non-IPv4 family, C `net_bind` and `net_connect` return `-EAFNOSUPPORT` (`network_wii.c:743-750,808-815`; `EAFNOSUPPORT` is 97 in `gc/lwip/arch.h:159`). Rust's `sockaddr_in_to_wii` rejects a non-IPv4 family with `None`, and `bind`/`connect` return `-1` (`lwip-sys/src/lib.rs:921-935,990-993,1127-1130`). C `net_connect` returns `-EINVAL` for a short length; Rust returns `-1` for the same condition.

There is also a length-validation difference in `bind`: C accepts `namelen` but does not inspect it before copying `sizeof(struct wii_sockaddr_in)` bytes (`network_wii.c:743-758`). Rust rejects a length shorter than that structure in `sockaddr_in_to_wii` (`lwip-sys/src/lib.rs:921-923`). This is a stricter Rust validation boundary.

### `SockLenT` signedness

C defines `socklen_t` as `u32` (`gc/network.h:107-109`) and uses it in the public socket prototypes (`:150-168`). `lwip-sys` declares `SockLenT=i32` and uses it in socket length parameters and pointers (`lwip-sys/src/lib.rs:22-23,921-957,990-1120`). Although both occupy 32 bits on PowerPC, direct Rust callers cannot represent the full unsigned range and negative values are treated as too short or invalid. The higher-level `ogc-network-sys` now declares `SockLenT=u32` and converts scalar lengths at its boundary (`ogc-network-sys/src/lib.rs:89,962-997`); that wrapper does not remove the signedness mismatch in the `lwip-sys` crate API itself and rejects values above `i32::MAX` during conversion.

### `FIONREAD`

C `net_ioctl` handles only `FIONBIO`; its default case, which includes `FIONREAD`, returns `-EINVAL` (`network_wii.c:1093-1109`). Rust previously stored the constant 4 in `SOCKET_RX_AVAILABLE` after successful connect and reported it for `FIONREAD`; receive calls later subtracted returned bytes from this synthetic value, without obtaining a pending-byte count from IOS. Therefore Rust reported four bytes available immediately after connect, even before data had arrived.

### BUG FIX: match C's unsupported `FIONREAD` result

Removed the synthetic receive-availability counter and its connect/receive bookkeeping. The Wii ioctl backend now returns `-EINVAL` for `FIONREAD`, matching C's default unsupported-command result. Regression coverage checks that `FIONREAD` returns `-EINVAL` after connect and leaves the caller's output value unchanged. Both `cargo check -p lwip-sys --target powerpc-unknown-linux-gnu` and `cargo check -p lwip-sys --tests --target powerpc-unknown-linux-gnu` passed; the second command compiled the test but did not run it. No hardware verification was performed. Implementer: `/root/worker_c`. Independent re-review required.

## Result

The `FIONREAD` finding has an implementation handoff and needs independent re-review. The other listed findings remain open. `lwip-sys` remains pending. This addendum is independent evidence and does not edit `/root/reviewer_4`'s grouped `docs/review/liblwip.md` note.

## Fix handoff: socket creation and error parity

Implementer: current parity pass. Independent re-review: pending.

The Wii socket backend now uses C's public socket creation flag values (`SOCK_CLOEXEC = 0x10000000`, `SOCK_NONBLOCK = 0x20000000`) and removes those bits before the IOS socket ioctl. After successful creation it applies `IOS_O_NONBLOCK` through the same F_GETFL/F_SETFL sequence when requested and sends the default 32768-byte `SO_RCVBUF` setting. As in C, failures in these two post-create setup operations do not replace the successfully created socket return value.

A missing network-top descriptor now propagates `-ENXIO` through the Wii ioctl helpers instead of collapsing to `-1`. `bind` and `connect` also reproduce the inspected C family errors: unsupported address families return `-EAFNOSUPPORT`, and `connect` returns `-EINVAL` for a short IPv4 address length with the C family-before-length precedence. `bind` intentionally does not reject a short `namelen`, because current C does not inspect that parameter before copying the fixed Wii IPv4 address structure.

The separate `SockLenT` signedness finding remains open. The earlier `FIONREAD` fix remains unchanged. No hardware validation is claimed.

## 2026-10-02 `socklen_t` follow-up

The remaining public length-type mismatch is fixed. `lwip-sys::SockLenT` is now `u32`, as declared by current `gc/network.h`, and `ogc-network-sys` no longer converts these values through signed `i32`. The nearby socket address and option structs were rechecked for field width/layout and did not require changes. Independent re-review and target runtime validation remain pending.

## Independent re-review — 2026-10-03

- Reviewer: `/root/reviewer_c`
- Implementer: `notOrrytrout` (commit author)
- Commits inspected: `aeb7649` (`Match lwIP FIONREAD unsupported result`), `c0b6c00` (`Align lwIP socket flags and error handling`), and the lwip-sys portion of `4aa6399` (`Apply runtime parity batch`).
- Scope: FIONREAD, socket flags/setup/errors, and `SockLenT`. Other socket API findings remain open.
- C evidence inspected: `../../C-sources/libogc/libogc/network_wii.c:701-723,743-758,808-830,1093-1109`; `../../C-sources/libogc/gc/network.h:107-109,150-168`; `../../C-sources/libogc/external/bsd/include/sys/socket.h:80-86`.
- Rust evidence inspected: `crates/services/lwip-sys/src/lib.rs:23,969-1000,1018-1045,1167-1192,1499-1524`, plus the Wii-backend regression tests.
- Validation: `cargo check -p lwip-sys --target powerpc-unknown-linux-gnu` passed. `cargo check -p lwip-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled the tests; tests were not run. No hardware verification was performed.

### PASSED

- `SockLenT` now has the C `u32` width and signedness (`gc/network.h:107-109`; Rust `lib.rs:23`). The follow-up also removes the wrapper's signed intermediate conversion. This fixes the earlier API range mismatch.
- For a valid non-null output pointer, unsupported `FIONREAD` returns `-EINVAL` and does not modify the output, matching C's `net_ioctl` default branch (`network_wii.c:1093-1109`; Rust `lib.rs:1499-1523`). The regression test checks both the return and unchanged output (`lib.rs:1935-1937`).
- Socket creation masks the C flag bits, applies nonblocking setup when requested, and attempts the default 32768-byte receive-buffer setting (`network_wii.c:699-723`; Rust `lib.rs:969-998`). Missing top descriptor and IPv4 family/short-connect-length results are also explicitly mapped to C's `-ENXIO`, `-EAFNOSUPPORT`, and `-EINVAL` values on the inspected paths (`network_wii.c:706,748-749,813-815`; Rust `lib.rs:1019-1027,1168-1179`).

### NEEDS_CHANGES

1. **Connect reads the full address before validating its length.** C reads only `sa_family` before checking `addrlen`, then returns `-EINVAL` for a short address before copying the Wii address (`network_wii.c:813-824`). Rust performs a full `SockAddrIn::read()` at `lib.rs:1174`, and checks `namelen` only at `1178`. A caller that supplies the family field but a buffer shorter than `SockAddrIn` can therefore trigger an out-of-bounds read/undefined behavior instead of C's early `-EINVAL`. Read/check the family and length before reading fields that the short input does not provide.
2. **Bind/connect no longer write the normalized `sa_len` into the caller's address.** C sets `name->sa_len`/`addr->sa_len` to the Wii sockaddr size before issuing the ioctl (`network_wii.c:751,817-819`). Rust builds a local `WiiSockAddrIn` at `lib.rs:1029-1034,1181-1186` and never updates the caller's `SockAddrIn.sin_len`. This is observable to callers after either operation and differs from C.
3. **The failed `F_GETFL` path for `SOCK_NONBLOCK` differs.** C always ORs `IOS_O_NONBLOCK` into the `net_fcntl(F_GETFL)` return and calls `F_SETFL` (`network_wii.c:715-720`). Rust only performs `F_SETFL` when `F_GETFL` returns nonnegative (`lib.rs:979-985`). The successful `socket` return is preserved in both, but the sequence and potential side effect differ when `F_GETFL` fails.
4. **`ioctl` null-argument error precedence differs when the network descriptor is absent.** C checks `net_ip_top_fd` before checking `argp` (`network_wii.c:1098-1100`) and returns `-ENXIO` when both conditions apply. Rust returns `-EINVAL` on the null pointer before reaching the helper that reports `-ENXIO` (`lib.rs:1499-1502`). The regression test covers null `FIONREAD` only with the network descriptor available (`lib.rs:2157`), so it does not catch this ordering difference.

Result: **NEEDS_CHANGES** for the three-commit re-review. The FIONREAD normal-argument result, `SockLenT`, and listed successful setup/error paths pass; the four mismatches above remain. The implementation handoff should return these findings to the implementer. The overall crate and the unrelated socket API findings remain open.

## Implementer handoff: short connect address validation

Implementer: `/root/worker_c`. Status: fixed; needs independent static re-review. `connect` now reads only the `sa_family` byte from the caller's address first, checks the family and `namelen`, and reads the full `SockAddrIn` only after the length check passes. A target regression test passes a two-byte buffer containing only `sa_len` and `sa_family` with a short length and expects `-EINVAL`. The other three lwIP findings remain open.

Validation: `cargo check -p lwip-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled the target tests; tests were not run. `git diff --check` passed for the changed files. `rustfmt --check --edition 2021 crates/services/lwip-sys/src/lib.rs` reports existing formatting differences at unrelated lines 1 and 1956; the added test is formatted. No hardware verification was performed.

### Independent re-review of short-address validation (`e0d7fd5`)

**Result: PASS for the bounded fix.** Reviewer: `/root/reviewer_d`; implementer: `/root/worker_c`.

C checks the network descriptor and address family, then validates `addrlen` before copying the complete address (`C-sources/libogc/libogc/network_wii.c:808-830`). Rust follows the same order: it reads only `SockAddr.sa_family`, checks family and length, then reads `SockAddrIn` (`crates/services/lwip-sys/src/lib.rs:1167-1199`). The two-byte regression buffer contains only `sa_len` and `sa_family`; the test checks IPv4 with a short length returns `-EINVAL` (`lib.rs:1887-1903`). No adjacent regression was found. Other findings recorded in this note remain open.

Validation: `cargo check -p lwip-sys --target powerpc-unknown-linux-gnu --tests` passed and compiled test code; tests did not run. No hardware verification was performed.

## Implementer handoff: `SOCK_NONBLOCK` failed `F_GETFL` sequence

Implementer: `worker_c`. Status: fixed; needs independent static re-review. Rust now always issues `F_SETFL` after the `F_GETFL` call when `SOCK_NONBLOCK` is requested. It ORs `IOS_O_NONBLOCK` into the `F_GETFL` return value, including negative values, and ignores both setup-call results so a successful socket result stays unchanged. Added regression coverage for the failed-get case and the `F_SETFL` arguments. Other recorded lwIP findings remain open.

Validation: `cargo check -p lwip-sys --target powerpc-unknown-linux-gnu --tests` passed and compiled the target test; tests were not run. Scoped `git diff --check` passed. No hardware verification was performed.

## BUG FIX handoff: read only the 8-byte Wii sockaddr

Implementer: `thread-current/root`. Independent static re-review required.

C's `wii_sockaddr_in` is 8 bytes. The Wii bind/connect paths now read that 8-byte layout instead of reading the 16-byte public `SockAddrIn`. They use unaligned 8-byte reads after validating the family and minimum address length. `connect` returns `-EINVAL` for fewer than 8 bytes after its family check. `bind` also rejects buffers shorter than the 8-byte minimum with `-EINVAL`; this is a deliberate memory-safety guard because C ignores `namelen` and may read beyond a short buffer. `sendto` uses the same 8-byte conversion helper.

The network DOL probe now binds an 8-byte Wii sockaddr and tries a port-zero connect with an 8-byte sockaddr; it checks result codes and canonical `sin_len` write-back. `cargo check -p lwip-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled tests, which did not run. The DOL source compiled but linking is blocked by unresolved native devoptab symbols in `ogc-sysbase`; no DOL was produced or executed. No hardware verification was performed.

### Independent re-review of the 8-byte Wii sockaddr fix (`ef6b5e6`)

**Result: PASS for the bind/connect buffer-boundary fix.** Reviewer: `reviewer_alpha`. Implementer: `thread-current/root`.

C defines `wii_sockaddr_in` as 8 bytes and copies that size in both `net_bind` and `net_connect` (`../../C-sources/libogc/libogc/network_wii.c:125-131,743-758,808-829`). Rust now reads `WiiSockAddrIn` with `read_unaligned`, so it reads exactly the same 8-byte fields and does not require the 16-byte public BSD `SockAddrIn`. `connect` checks the family and minimum 8-byte length before the read. `bind` uses the same minimum length check. Both canonicalize the local Wii address and write `sa_len=8` to the caller before issuing the IOS call, matching C's successful/valid-address write-back behavior. The bind minimum-length guard is a deliberate safety difference: C ignores `namelen` and can read beyond a buffer that is shorter than the 8 bytes it copies.

The probe source defines an 8-byte, 4-byte-aligned buffer, initializes `sa_len` to zero, and passes it to both `bind` and `connect`. The probe then checks that each call writes `sa_len=8`; its connect case also expects the zero-port `-EINVAL` result after parsing the address. The PowerPC `cargo check --tests` compiled test code but tests did not run. The DOL build could not link and therefore did not run because `ogc-sysbase` has unresolved native devoptab symbols. No hardware verification was performed.

This review passes the 8-byte bind/connect fix. Other socket API differences remain open; `lwip-sys` is not complete.

### Independent re-review of DOL `sa_len` assertions

**Result: PASS for the probe assertion update.** Reviewer: `reviewer_alpha`. Implementer: `thread-current/root`.

The follow-up changes only the two 8-byte DOL inputs from `sa_len=8` to `sa_len=0`; both existing assertions require the output byte to equal 8 after the call. This now tests observable canonical-length write-back for exact 8-byte bind and connect buffers. The DOL still could not link or run because `ogc-sysbase` has unresolved native devoptab symbols. No DOL execution or hardware verification is claimed.

## Independent re-review of socket parity handoffs

Reviewer: `reviewer_alpha`. Implementer: `worker_c` for the socket fixes and `thread-current/root` for the final caller `sa_len` handoff.

**Result: NEEDS_CHANGES.** The recorded bounded fixes match C on the inspected normal and error paths: socket flag masking and post-create setup, missing-descriptor `-ENXIO`, IPv4 family and short-connect errors, `SockLenT = u32`, unsupported `FIONREAD`, `F_GETFL`/`F_SETFL` call behavior, ioctl descriptor-before-null precedence, and caller `sa_len` write-back. The latest bind/connect fix writes the canonical length before the IOS call and preserves the input length on the tested short-connect error.

I found one remaining boundary mismatch in both address paths. C's private `wii_sockaddr_in` is 8 bytes (`network_wii.c:125-131`), and C `connect` accepts `addrlen == 8`, then copies exactly those 8 bytes (`network_wii.c:808-829`). C `bind` also copies exactly 8 bytes and does not use `namelen` (`network_wii.c:743-758`). Rust's public `SockAddrIn` is 16 bytes (`crates/services/lwip-sys/src/lib.rs:110-117`). `connect` accepts a length of 8, but then reads an entire `SockAddrIn` at `lib.rs:1202-1206`; `bind` likewise reads the entire 16-byte value at `lib.rs:1048`. A caller that supplies a valid 8-byte Wii sockaddr can therefore be read past its buffer, while C only reads those 8 bytes. The current regressions cover a 2-byte short-connect input and full-size Rust `SockAddrIn` values; they do not cover an 8-byte address. This issue remains open.

Validation evidence: prior handoffs record PowerPC `cargo check --tests` success, which compiled target tests but did not execute them. The available `network_probe.dol` build attempt is documented in `docs/review/libogc.md` and failed at link time due unresolved native devoptab symbols; no DOL was produced or run. No hardware verification was performed.

The other socket behavior differences noted in the linked first-pass review remain outside these bounded fixes and keep `lwip-sys` open.

## Independent re-review: three lwIP call-parity fixes (`801cb4b`, `aaed545`, `e2d798f`)

**Result: PASS for these bounded fixes.** Reviewer: `worker_a`. Implementer: `worker_c`.

The nonblocking socket path now always issues `F_SETFL` with the result of `F_GETFL` ORed with `IOS_O_NONBLOCK`, even when the first call returns a negative value, and it preserves the created socket result. This matches C's call sequence in `../../C-sources/libogc/libogc/network_wii.c:717–720`. The Wii ioctl path checks the network descriptor before it checks the argument pointer, matching the C error ordering (`network_wii.c:1093–1112`; Rust `crates/services/lwip-sys/src/lib.rs:1513–1520`). `bind` and `connect` now write canonical `sin_len` back to the caller after the existing validations and before the IOS ioctl; C updates that caller field on these paths (`network_wii.c`, bind/connect request paths; Rust `lib.rs:953–960,1040–1065,1190–1215`). Regression tests cover failed `F_GETFL` followed by `F_SETFL`, missing descriptor plus null ioctl argument, and successful `bind`/`connect` write-back plus short-connect non-write-back.

I inspected all three source diffs and tests. No checks or tests were run during this re-review; the implementer records successful PowerPC target test compilation, with tests not executed. Other socket behavior findings remain open. No hardware verification was performed.

## Implementer handoff: `ioctl` missing-descriptor precedence

Implementer: `worker_c`. Status: fixed; needs independent static re-review. The Wii `ioctl` path now checks for a network descriptor before it checks the argument pointer, matching C. Added a regression test with both the network descriptor absent and a null `FIONBIO` argument; it expects `-ENXIO`. Other lwIP findings remain open.

Validation: `cargo check -p lwip-sys --target powerpc-unknown-linux-gnu --tests` passed and compiled the target test; tests were not run. Scoped `git diff --check` passed. No hardware verification was performed.

## Implementer handoff: bind/connect caller `sa_len` write-back

Implementer: `worker_c`. Status: fixed; needs independent static re-review. Rust now writes the canonical Wii sockaddr length back to the caller after the existing descriptor/family checks in `bind`, and after the existing family/length checks in `connect`, before issuing the IOS ioctl. Regression coverage checks both successful operations update `sin_len` to 8 and that a short `connect` failure leaves its input unchanged. Other lwIP findings remain open.

Validation: `cargo check -p lwip-sys --target powerpc-unknown-linux-gnu --tests` passed and compiled the target test; tests were not run. Scoped `git diff --check` passed. No hardware verification was performed.

## 2026-10-05 completion review

The former incomplete socket review is now classified `parity_blocked`. Current C `net_getsockopt` always returns `-EINVAL`, while Rust returns locally tracked option values. C `net_setsockopt` permits `optval == NULL` when `optlen == 0`; Rust rejects a null option pointer. These are named non-safety public-result differences. Earlier FIONREAD, socket setup, ABI, sockaddr, ioctl-order, and write-back fixes remain accepted. See `review-incomplete-17-2026-10-05.md`.


## 2026-10-05 socket-option parity fix

The Wii backend now matches `network_wii.c` for the two socket-option differences named by the completion review. `get_socket_option` returns `-EINVAL` unconditionally, matching C's intentionally unimplemented `net_getsockopt`; it no longer exposes Rust-only tracked option values. `set_socket_option` checks the network-top descriptor first, rejects only option payloads longer than the C 20-byte parameter field, zero-fills that field, copies caller bytes only when the pointer is non-null and the length is nonzero, submits the IOS ioctl, and returns the IOS result directly. This also permits a null caller option pointer in the same cases C does instead of rejecting it in the Rust wrapper.

Regression coverage checks the unconditional `get_socket_option` result and the nullable/zero-filled `setsockopt` payload preparation. Independent PowerPC compile/re-review is still required. The crate remains `parity_blocked` because the separately recorded receive chunking/error-precedence scope is not changed by this fix.

## 2026-10-05 receive completion pass

The remaining receive chunking/error-precedence finding is fixed at source level. `recv`/`recvfrom` now check the network-top descriptor before the length error, return `-EINVAL` for non-positive lengths when the descriptor exists, allocate a 32-byte-aligned temporary receive block of at most 32 KiB, and issue repeated receive ioctls only after full blocks. This matches `network_wii.c`'s `maxblocksize = NET_HEAP_SIZE / 2` with the 64 KiB network heap.

The return policy also matches C's unusual precedence. A partial positive block returns the accumulated byte count. A later zero, negative result, or oversized result replaces any count accumulated from earlier full blocks; oversized results return `-EOVERFLOW`. The temporary buffer is zeroed before each IOS receive and copied into the caller buffer only for valid positive results. Allocation failure returns `IPC_ENOMEM`.

Rust deliberately keeps one stricter safety boundary: a null destination pointer with a positive length is rejected before the receive/copy operation instead of following C into an invalid `memcpy` destination. Descriptor and length checks still retain C's ordering before that safety validation.

Regression coverage checks full-block continuation, partial-block completion, later EOF/error precedence, overflow precedence, and descriptor-before-length ordering. This removes the last recorded non-safety `lwip-sys` blocker. Independent PowerPC compilation and runtime socket validation are required before changing the crate from `needs_re_review` to `reviewed`.
# 2026-10-11 IOS nonblocking message flags

### Subsequent receive-vector correction and hardware result

The Wii host report reached `ACK RECEIVE`, while the Dolphin client completed its
hello handling and timed out in MATCH. The user confirmed the Wii pinwheel stopped.
C `net_recv` omits the source-address output (`network_wii.c:901-954`); the Rust
backend always requested an 8-byte address output. The backend now uses the
explicit `IoctlvEntry::OutNull` for omitted source storage, giving IOS exactly a
null pointer and zero length. Requested address output stays on the real receive
path. The exact production enum conversion test passed, and the DOL built.

The user then completed games with Dolphin hosting and with the Wii hosting on
SHA-256 `24483a622613c99fc17db4a886852d9ea186bd23d47433a77dacb4a4d984f3ef`.
The real TCP Dolphin checks also verify the omitted IOS address vector from the
emulator socket log. These later results resolve the frozen receive in these
runs; they do not retroactively turn the earlier failed runs into passes.

After the corrected host sent its hello, the user confirmed a stopped pinwheel
on the real Wii. The Mac client received the hello and sent the acknowledgement,
but no sustained match input exchange was recorded. The hardware stop remains
open until another real run verifies the receive path.

The public lwIP `MSG_DONTWAIT` value is 0x40. Wii IOS uses `SO_MSG_NONBLOCK` 0x04,
as recorded in Dolphin's production IOS ABI definitions:
https://github.com/dolphin-emu/dolphin/blob/master/Source/Core/Core/IOS/Network/Socket.h.
The Rust Wii backend forwarded 0x40 without conversion. The new shared
`ios_socket_flags::to_ios` converts only that bit for both send and receive and
preserves other bits. Five fixed input/output assertions passed in the exact
production helper compiled with rustc. GameCube flags are unchanged.

The Rust-only `recv_slice` helper now retains backend errors rather than reducing
every negative code to -1. application retries IOS SO_EAGAIN (-6) within its existing
frame timeout and continues rendering while it waits. Other errors remain errors.
The full DOL built. The real TCP Dolphin regression passed with 218 match packets
and rejection of the second client:
DOL SHA-256: `5aad6e7fe88c2230923293329d4335ed913cc3d95aabe9442f10147f9aea0b85`.

The hardware retry (`1791765559-923021000`) reached handshake entry. Dolphin
received the hello, sent the acknowledgement, and displayed the match screen,
but again received no match input. The flag correction did not resolve this stop.
The next build separates `ACK POLL`, `ACK RECEIVE`, and `ACK RECEIVED` checkpoints
around the real host operations. No successful Wii match is claimed.

## 2026-10-14 hostent ABI correction for std DNS

The new std DNS adapter exposed a Rust layout error in `ogc-network-sys::HostEnt`:
`h_addrtype` and `h_length` were declared as 32-bit integers. DevkitPro's
`netdb.h` and C `struct hostent` declare both fields as 16-bit `short` values.
The Rust fields are now `i16`; `tinysmb-sys` casts the shared IPv4 family value
to the matching field type. This restores the 16-byte PowerPC Wii hostent header
layout and makes the IOS reply decoder use offsets 8 and 10 for those fields.

The shared DNS decoder checks address family and length, pointer relocation
bounds, list terminators, and copied IPv4 values. Its host test verifies that
returned addresses stay owned after the IOS reply buffer is cleared, and rejects
out-of-range or unterminated pointer lists. The Wii std probe DOL links with the
network provider. No Wii DNS execution has been performed.
