# libgcsd Review

Date: 2026-10-01

Status: the EXI selection, CMD0 startup, read-transfer output, multiwrite stop, CMD0 write-protect retry, final operating-condition attempt, generic write-protect command, mode-specific data-read clock bytes, and CRC-delay fixes have passed static re-review. GCSD remains open for token-timeout draining, initialization defaults, and missing helper APIs. No hardware verification is recorded.

## Scope

- `crates/services/ogc-gcsd`
- C references: `C-sources/libogc/libogc/gcsd.c`, `C-sources/libogc/libogc/sdgecko_io.c`, `C-sources/libogc/libogc/sdgecko_buf.c`, `C-sources/libogc/gc/sdcard/{gcsd.h,card_io.h,card_buf.h,card_cmn.h}`, and `C-sources/libogc/gc/ogc/disc_io.h`

## Reviewed behavior

- Three exported interfaces map to slot A, slot B, and port 2.
- Device type, read/write and slot feature bits, and callback order match the C descriptors.
- Each Rust callback routes to the matching slot backend operation.
- SD command setup, response checks, CRC handling, 512-byte sector reads and writes, and stop-command flow were compared with `sdgecko_io.c`.

## Confirmed differences recorded by the review

1. **EXI selection differs.** Rust selects the SD device for every transaction (`wii_backend.rs:253`). C uses `EXI_Select` for normal transfers and `EXI_SelectSD` only for CMD0 startup clocks (`sdgecko_io.c`). These functions program different EXI register bits (`exi.c:334,386`).
2. **Lock contention differs.** Rust reports no card when `exi::lock` fails (`wii_backend.rs:257`). C waits until the lock is available (`sdgecko_io.c:259`).
3. **CMD0 startup clocks differ.** Rust's generic command path sends 10 dummy bytes (`wii_backend.rs:655,869`). C sends 20 blocks of 128 dummy bytes before CMD0 (`sdgecko_io.c:283`).
4. **Read transfer output differs.** Rust transmits the caller's existing buffer bytes during reads (`wii_backend.rs:542`). C fills the transfer buffer with the idle byte first (`sdgecko_io.c:675`).
5. **CRC timing differs.** C delays 1 microsecond between block data and CRC transfers for reads and writes (`sdgecko_io.c:721,778`). Rust transfers CRC immediately (`wii_backend.rs:542,591`).
6. **Multiwrite stop clocks differ.** C sends four clock bytes after the stop token before polling (`sdgecko_io.c:807`); Rust polls immediately (`wii_backend.rs:621`).
7. **Write-protect handling differed.** The original implementation retried reset without C's inverted CMD0 mode. The CMD0 branch now matches C and passed independent review below. Generic command encoding and multiwrite-stop encoding in write-protect mode remain open; see [the residual audit](libgcsd-residual-audit.md).
8. **Final operating-condition attempt differed.** The original implementation returned an I/O error after timeout without a final ACMD41/CMD1. Commit `8f0df8b` adds this attempt and passed independent static review below.
9. **C helper API coverage differs.** `card_io.h` declares CID/CSD arrays, I/O initialization and status helpers, speed/page-size/addressing accessors, and insertion/ejection callbacks; `card_buf.h` declares buffer-pool operations. Rust does not expose all of these counterparts. `sdgecko_writeSector` is declared but has no definition in the checked-in C source, so it cannot be compared.

Other observed differences: Rust `clear_status` clears error state and reads status, while the C wrapper currently returns `true` with a FIXME; C ignores the app-command result during operating-condition setup, while Rust propagates it; and some C timeout paths clock one extra byte before returning. The first is an existing Rust addition. Intent and impact of the others need consideration during the fix pass. C declares insertion/ejection callbacks, but the callback pointers are static and have no public setter.

## BUG FIX pass: bounded protocol corrections

- **BUG FIX — EXI selection.** Normal data and command transfers now use `EXI_Select`. Only CMD0 startup clocks use `EXI_SelectSD`. This matches `sdgecko_io.c`, where `__card_writecmd0` uses `EXI_SelectSD` for the initial clocking and `EXI_Select` for CMD0, while other transfer helpers use `EXI_Select`.
- **BUG FIX — CMD0 startup clocks.** Rust now sends 20 blocks of 128 idle bytes under `EXI_SelectSD`, deselects, then selects normally and sends CMD0 with its CRC. This replaces the generic ten-byte preamble for CMD0 and matches `__card_writecmd0` in `../../C-sources/libogc/libogc/sdgecko_io.c`.
- **BUG FIX — Read transfer idle output.** Rust now fills each data-read buffer with `0xff` before it polls for the data token and starts the read/write transfer. This matches C `__card_dataread`, which fills the transfer span with `_ioClrFlag` first.
- **BUG FIX — Multiwrite stop clocks.** Rust now sends four `0xff` read/write clock bytes after the stop token and before it polls the busy response, as C `__card_multiwritestop` does.

Regression assertions cover the read-buffer initialization and four-byte stop-clock sequence. The PowerPC-target test build passed with `cargo check -p ogc-gcsd --target powerpc-unknown-linux-gnu --tests`. The tests were not executed because the host cannot safely run the hardware-facing PowerPC backend. No hardware behavior was tested.

## Remaining confirmed findings

The following findings remain open: EXI lock contention does not wait; read and write CRC transfers omit C's 1 microsecond delay; generic command and multiwrite-stop encoding differ in write-protect mode; and the C helper API surface is not fully exposed. The final operating-condition attempt was fixed by commit `8f0df8b`. The note also records the operating-condition app-command handling difference, extra timeout clocks, and the existing Rust `clear_status` addition. See [the residual audit](libgcsd-residual-audit.md) for current evidence.

## Independent re-review: commit `b64e2bf`

Reviewer: `worker_c`. Implementer: `/root/di_review`.

The four fixes match the checked-in C behavior for the normal operating mode: ordinary transactions use `EXI_Select`; CMD0 uses `EXI_SelectSD` for 20 blocks of 128 idle bytes before switching to `EXI_Select`; reads initialize the transfer span with `0xff`; and multiwrite stop sends four idle read/write bytes before polling busy. These correspond to `sdgecko_io.c` functions `__card_writecmd0`, `__card_dataread`, and `__card_multiwritestop`.

One mismatch remains in the changed CMD0 path. C `__card_writecmd0` checks `_ioWPFlag`: it changes `_ioClrFlag` to `0x00`, inverts all five command bytes, and complements the CRC before sending. Rust `send_cmd0` always fills the startup blocks with `0xff` and sends the normal command and CRC. Therefore the new CMD0 path does not preserve C's write-protect mode. The broader write-protect fallback finding remains open; the new fix must either implement this CMD0 branch or document why this Rust crate cannot enter that mode.

This re-review does not close the crate. Existing open findings also remain, including lock contention, CRC delay, the last operating-condition attempt, and helper API coverage. Static comparison only; no tests were run by this reviewer and no hardware verification was performed.

## BUG FIX handoff: CMD0 write-protect mode

Implementer: `worker_c`. Independent reviewer: `/root`.

Rust now retries soft reset in write-protect mode after the normal CMD0 attempt fails, as C `sdgecko_initIO` does. `send_cmd0` uses idle byte `0x00`, inverts the five CMD0 bytes, and complements the CRC in that mode, matching C `__card_writecmd0`. The mode is reset to normal at the start of each card initialization. A regression test checks normal and write-protect wire formats.

`cargo check -p ogc-gcsd --target powerpc-unknown-linux-gnu --tests` passed. `cargo test -p ogc-gcsd` could not run on the host because its PowerPC-only dependency crates reject non-PowerPC targets. Tests compiled for the PowerPC target but were not executed. No hardware verification was performed. This fix was independently re-reviewed; other findings listed above remain open.

## Independent review: CMD0 write-protect mode

Reviewer: `/root/reviewer_1`. Implementer: `worker_c`.

Static comparison accepted this fix. C `__card_writecmd0` sets the idle byte to `0x00`, inverts all five CMD0 command bytes, then complements the CRC after setting its end bit (`C-sources/libogc/libogc/sdgecko_io.c:292–301,327–328`). Rust `cmd0_wire_format` and `send_cmd0` apply the same changes in write-protect mode (`crates/services/ogc-gcsd/src/wii_backend.rs:122–133,772–806`), and `initialize_detected_card` resets that mode before normal reset and enables it only for the retry (`wii_backend.rs:1016–1028`). This retry corresponds to C `sdgecko_initIO` (`sdgecko_io.c:1222–1230`). The regression assertions cover the normal and write-protect wire bytes (`wii_backend.rs:1291–1295`). No mismatch remains in this fix. The other recorded GCSD findings remain open, so the crate stays `finding`.

The implementer's PowerPC test compilation passed. I also ran `cargo check -p ogc-gcsd --target powerpc-unknown-linux-gnu --tests`; it passed. Tests were not run, and no hardware verification was performed.

## Independent re-review: final operating-condition attempt

Reviewer: `reviewer a` (primary reviewer). Implementer: `worker_b`. Implementation commit: `8f0df8b`.

C `__card_sendopcond` checks the idle response after each timed attempt, then sends one final ACMD41/CMD1 after timeout, reads and checks its response, and returns success only when IDLE clears (`../../C-sources/libogc/libogc/sdgecko_io.c:918-963`). Rust `send_opcond_with_attempts` follows that order and makes exactly one final attempt (`crates/services/ogc-gcsd/src/wii_backend.rs:833-855`). The SDHC closure ignores the APP_CMD result only on the final attempt, as C does, and checks the command, response read, and response value (`wii_backend.rs:857-884`). It returns I/O error when IDLE remains and success when it clears. I found no mismatch in this fix.

`cargo check -p ogc-gcsd --target powerpc-unknown-linux-gnu --tests` passed. This compiled the regression tests; they did not run. No hardware verification was performed. Other GCSD findings remain open.

## Coverage and limits

The review covered `lib.rs`, `timeout.rs`, and all of `wii_backend.rs` against the listed C implementation and headers, including interface descriptors, callback routing, initialization, status, sector reads/writes, CRC, stop flow, and the declared helper surface. It did not compare behavior on real devices. Rust-only error handling and unavailable external consumers of the C helper APIs remain context limits.

## BUG FIX

Rust rejected a zero-sector read or write before it initialized the card. C calls `sdgecko_preIO` before checking `num_sectors`, so a zero-sector request can still initialize the card before it returns failure. The Rust transfer path now performs initialization before rejecting the request. A regression test checks this order.

Rust stopped after one protocol initialization failure. C retries failed initialization up to six times, and it attaches an EXI removal handler before the protocol sequence. Rust now retries up to six times, attaches the handler, clears its card state on removal, and detaches after a failed initialization or shutdown.

Rust's startup callback called `init_io` even when the card was already initialized. C uses `sdgecko_preIO`, which skips initialization in that state. Rust now uses the same pre-I/O check for startup, status, and transfers.

Rust's `clear_status` returns the result of its status operation; the checked-in C wrapper currently returns `true` with a FIXME. This Rust behavior is an existing addition, not a C parity claim.

Other C declarations still lacking Rust counterparts include buffer-pool helpers, CID/CSD reads, card-state/status helpers, and insertion/ejection callbacks. The speed, page-size, and addressing accessors are now present; their initialized-state parity is reviewed below. No other checked-in C source calls the buffer allocator or card-state accessors, but their external API scope still needs review. `sdgecko_writeSector` is declared in `card_io.h` but has no definition in the checked-in C tree.

## Validation boundary

Static source comparison and target compile checks only; no hardware verification is recorded.

The test build compiles the regression tests but does not execute them. `cargo xtask review-status` fails because the shared worktree ledger lacks multiple workspace crate rows. This handoff updates only the `ogc-gcsd` row; the other missing rows need separate coordination.

## 2026-10-02 write-protect residual parity follow-up

The generic write-protect command path now matches C in addition to the earlier CMD0 fix. A shared formatter applies the normal command bit and CRC first, then in write-protect mode inverts all five command bytes, complements the CRC, and selects zero-valued idle bytes. Multiwrite stop now uses C's `0x02` write-protect token instead of `0xfd` and uses the mode-specific idle byte for its clock/busy transfers. The earlier EXI wait and one-microsecond CRC-delay findings were rechecked and are already present in the current source. The low-level helper/API-surface gap remains separate.

## Implementer handoff: card speed, page-size, and addressing accessors

Implementer: `/root/worker_b`. Commit: `06d630a` (`Expose GCSD card parameter helpers`). Rust now exposes `sdgecko_get_speed`, `sdgecko_set_speed`, `sdgecko_get_page_size`, `sdgecko_set_page_size`, and `sdgecko_get_addressing_type`, backed by the current per-slot state. Valid slots read and update that state as C does. `sdgecko_set_page_size` stores the requested page size before issuing the block-length command and retains the stored size if that command fails, matching C's update-then-command behavior. Rust handles invalid slots safely: getters and the page-size setter return `None`; the speed setter returns `false`.

Regression tests cover per-slot state access, invalid slots, the page-size update before the command callback, persistence on command failure, and invalid-slot command suppression. Validation: `cargo check -p ogc-gcsd --tests --target powerpc-unknown-linux-gnu` passed; target tests compiled but were not run. `git diff --check` passed. `cargo fmt -p ogc-gcsd -- --check` reports existing crate-wide formatting differences outside the changed code. No hardware verification was performed.

This handoff covers only the three accessor groups above. Independent static re-review is needed. CID/CSD accessors, status/init/unmount APIs, insertion/ejection callbacks, and buffer-pool APIs remain missing; they are not included in this fix.

## Independent re-review: card parameter accessors

Reviewer: `/root/reviewer_f`. Implementer: `/root/worker_b` (handoff identity); source commit: `06d630a`; grouped review handoff commit: `5983696`.

For valid drive numbers after `sdgecko_initIODefault`, the new accessors match the corresponding C state operations. C `sdgecko_getSpeed`/`sdgecko_setSpeed`, `sdgecko_getPageSize`, and `sdgecko_getAddressingType` directly read or write their per-drive arrays (`../../C-sources/libogc/libogc/sdgecko_io.c:1449-1474`). Rust bounds-checks the three slots, then accesses the corresponding indexed atomics (`crates/services/ogc-gcsd/src/lib.rs:24-52`; `wii_backend.rs:92-129`). The public addressing constants match `card_io.h`'s enum values (`CARD_IO_SECTOR_ADDRESSING=0`, `CARD_IO_BYTE_ADDRESSING=1`; `card_io.h:21-25`; Rust `lib.rs:21-22`). Invalid drive inputs return `None` or `false` instead of indexing outside C's arrays; this is a deliberate safe Rust boundary.

`sdgecko_setPageSize` stores a changed value before calling `__card_setblocklen` with the stored value (`sdgecko_io.c:1464-1469`). Rust stores the supplied value before calling `set_blocklen` and returns that command's result (`wii_backend.rs:112-125`). Both command helpers clamp only the CMD16 argument to 512 and then read/check the one-byte response (`sdgecko_io.c:1022-1045`; `wii_backend.rs:1051-1072`). The stored value remains the requested value when the command fails in both implementations. The changed code has no mismatch in these operations after defaults have been initialized.

One confirmed default-state difference remains. Before the first `sdgecko_initIODefault`, C's file-static `_ioCardFreq`, `_ioPageSize`, and `_ioAddressingType` arrays are zero-initialized (`sdgecko_io.c:68,74,85`), and the getters return those array values directly (`:1449-1474`). The initializer that assigns 16 MHz and byte addressing runs from the first C `__gcsd_startup` (`gcsd.c:52-60`; `sdgecko_io.c:1187-1204`), but it does not initialize `_ioPageSize`; a successful CSD read later sets that value (`sdgecko_io.c:1249-1254`). The Rust arrays start at 16 MHz, page size 1, and byte addressing before any startup (`wii_backend.rs:56-70`), and the public getters do not initialize the backend (`lib.rs:30-52`). Before defaults, valid-slot getters return `0, 0, CARD_IO_SECTOR_ADDRESSING` in C and `EXI_SPEED16MHZ, 1, CARD_IO_BYTE_ADDRESSING` in Rust. After defaults but before CSD is read, C returns `EXI_SPEED16MHZ, 0, CARD_IO_BYTE_ADDRESSING`, while Rust still returns `EXI_SPEED16MHZ, 1, CARD_IO_BYTE_ADDRESSING`. Also, each Rust initialization attempt calls `reset_slot`, which resets page size to 1 (`wii_backend.rs:308-317`); C does not clear `_ioPageSize` at `sdgecko_initIO` entry, so a failed later initialization can preserve its previous page size. The C header does not document a startup precondition for these accessors. Keep this as an open parity finding unless the API contract is explicitly limited to successful card initialization.

Regression coverage added by the implementation checks invalid public drive values (`lib.rs:269-276`), valid access to one indexed backend slot and invalid backend indices (`wii_backend.rs:1419-1437`), and page-size storage before the command callback plus retention on command failure (`:1439-1454`). It does not check isolation between two valid slots, pre-startup defaults, or the real `set_blocklen` wire operation; the latter is covered here by direct source comparison. `cargo check -p ogc-gcsd --target powerpc-unknown-linux-gnu --tests` passed during this review. This compiled the test code; tests were not run. `git diff --check -- docs/review/libgcsd.md` passed. No hardware verification was performed.

This re-review accepts the valid, initialized-slot behavior implemented by `worker_b`, and leaves the default-state difference above open. The pre-existing GCSD findings also remain open, so `ogc-gcsd` must stay in a finding state and must not be marked reviewed.

## Independent confirmation of initialization defaults finding (2026-10-03)

Reviewer: `reviewer_c`. C's card frequency, page-size, and addressing statics start at zero; `sdgecko_initIODefault` sets frequency to 16 MHz and addressing to byte mode but leaves page size zero until a successful CSD read (`../../C-sources/libogc/libogc/sdgecko_io.c:68,74,85,1187-1204,1249-1254`). Rust initializes these fields to 16 MHz, page size 1, and byte mode before startup, and its getters return them without initialization (`crates/services/ogc-gcsd/src/wii_backend.rs:56-70`; `crates/services/ogc-gcsd/src/lib.rs:30-52`). Rust also resets page size on every initialization attempt, while C preserves it until a successful CSD read (`sdgecko_io.c:1222-1227,1253-1254`; Rust `wii_backend.rs:308-317,1170-1174`). Both allow six protocol attempts. The existing tests do not cover pre-startup getters or page-size retention across failed reinitialization. No validation ran; no hardware verification was performed.

## Independent API-surface and initialization audit

Reviewer: `/root/reviewer_d`. The parameter getters still differ before initialization and after failed reinitialization: C statics start at zero, defaults set speed/addressing but leave page size zero until CSD succeeds, and failed init preserves the previous page size (`sdgecko_io.c:68, 74, 85, 1187–1204, 1249–1254, 1222–1254`). Rust exposes 16 MHz, page size 1, and byte addressing before startup and resets page size to 1 on each attempt (`crates/services/ogc-gcsd/src/wii_backend.rs:56–70, 308–316, 1170–1174`; getters at `lib.rs:31–52`).

The C public declarations in `gc/sdcard/card_io.h:30–58` include CID/CSD data and readers, low-level init/pre-I/O/unmount/status routines, and insertion/ejection notifications. Rust has private CID/CSD reads used to derive page size but discards both byte arrays; no caller-visible query data is exposed (`wii_backend.rs:1075–1117`). `isInserted` and shutdown map to status/unmount behavior through `DiscInterface` (`lib.rs:101–132`; `wii_backend.rs:1265–1279, 1398–1400`), and normal unmount/detach behavior matches C. The retry mechanism is represented internally by Rust's init loop; C's `_ioRetryCB` is private. C's inserted/ejected callback arrays have no setter in checked-in C and remain null/no-op, so their omission has no demonstrated callback effect.

`card_buf.h` publicly declares a three-node static buffer pool implemented in `sdgecko_buf.c:9–51`; Rust has no equivalent helpers. No other checked-in C source calls alloc/free, so this is a missing public helper surface without demonstrated in-tree runtime effect. `sdgecko_writeSector` is declared but has no C definition. There is no separate `ogc-gcsd-sys` crate. `cargo check -p ogc-gcsd --target powerpc-unknown-linux-gnu --tests` passed and compiled test code; tests did not run. No hardware verification was performed.

## Independent re-review of protocol fix `b64e2bf`

**Result: NEEDS CHANGES.** Reviewer: `/root/reviewer_d`. The generic write-protect command encoding and multiwrite-stop fixes pass against C. Two `data_read` differences remain. Rust clocks `0xff` for the initial buffer, token polling, payload and CRC bytes (`crates/services/ogc-gcsd/src/wii_backend.rs:673-712`), while C uses `_ioClrFlag` for those transfers (`C-sources/libogc/libogc/sdgecko_io.c:677-724`); C changes this flag to zero in write-protect mode (`:292-301`), which Rust continues using after its CMD0 fallback (`wii_backend.rs:1170-1182`). Also, on token timeout C clocks another byte, transfers payload and CRC, waits, then deselects and unlocks (`sdgecko_io.c:698-729`); Rust returns immediately (`wii_backend.rs:699-704`). The prior API/default findings remain open. Prior claims about EXI lock waiting, CRC delay, generic WP command encoding, and multiwrite stop are stale: those paths now match C.

Validation: `cargo check -p ogc-gcsd --target powerpc-unknown-linux-gnu --tests` passed and compiled test code; tests did not run. Existing tests do not cover EXI timeout draining or write-protect data-read bytes. No hardware verification was performed.

## Fix pass: write-protect data-read clock byte

**BUG FIX.** Implementer: `worker_c`. Independent re-review: needed. `data_read` now selects the C `_ioClrFlag` value from the slot's write-protect mode and uses it for the initial read buffer, each token poll, the payload transfer bytes, and the CRC response bytes. In write-protect mode these bytes are zero; in normal mode they remain `0xff`. Added regression coverage for both mode-specific values and initial buffer bytes.

The separate C timeout-drain behavior remains open: C performs additional clocks and drains payload/CRC after token timeout, while Rust returns immediately. Other recorded API and default-state differences also remain open. `cargo check -p ogc-gcsd --target powerpc-unknown-linux-gnu --tests` and `git diff --check` passed. Test code compiled but tests did not run. `cargo fmt -p ogc-gcsd -- --check` reports pre-existing formatting differences in `lib.rs` and other lines in `wii_backend.rs`; the added helper follows rustfmt formatting. No hardware verification was performed.

## Independent re-review of write-protect data-read clock-byte fix `d85e391`

**Result: PASS for the bounded fix.** Reviewer: `/root/reviewer_d`; implementer: `worker_c`.

C `__card_dataread` uses `_ioClrFlag` for the initial buffer, token polls, payload clock bytes, and CRC response bytes (`C-sources/libogc/libogc/sdgecko_io.c:658-729`). C sets this value to `0xff` normally and `0x00` in write-protect mode (`:292-301`). Rust now selects the same value from per-slot write-protect state and uses it for the buffer, each poll, payload, and CRC (`crates/services/ogc-gcsd/src/wii_backend.rs:160-168,688-730`). Both modes match C.

The separate token-timeout drain mismatch remains: C clocks one extra byte and transfers payload and CRC before deselect/unlock; Rust returns immediately on timeout (`sdgecko_io.c:698-729`; Rust `wii_backend.rs:699-704`). Validation: PowerPC `cargo check --tests` passed and compiled tests; tests did not run. `git diff --check` passed. No hardware verification was performed.

## Fix pass: `data_read` token-timeout drain

**BUG FIX.** Implementer: `worker_c`. Independent re-review: needed. On token timeout, Rust now clocks one final token byte, keeps `CARDIO_ERROR_IOTIMEOUT` if no start token arrives, then transfers the payload and CRC before deselecting/unlocking. If the final byte is the start token, Rust continues with success status, matching C. Added focused coverage for the extra idle-byte clock and the unconditional payload/CRC drain after timeout.

`cargo check -p ogc-gcsd --target powerpc-unknown-linux-gnu --tests` and scoped `git diff --check` passed. Test code compiled but did not run. No hardware verification was performed. Other GCSD API and default-state findings remain open.

### Independent re-review of token-timeout drain (`2207d41`)

**Result: PASS for this fix.** Reviewer: `/root/worker_b`. Implementer: `/root/worker_c`.

C clocks one more byte after the token timeout. If the token is still not `0xfe`, C records `CARDIO_ERROR_IOTIMEOUT`, then transfers the payload and two CRC bytes before cleanup; a CRC mismatch takes precedence (`../../C-sources/libogc/libogc/sdgecko_io.c:698–729`). Rust now performs the extra token transfer, retains timeout when it is not a start token, and runs the payload/CRC drain before returning that status. Transfer failures still return the I/O error before further work, and CRC mismatch returns the CRC error (`crates/services/ogc-gcsd/src/wii_backend.rs:180–207,725–778`). The regression verifies the extra clock, timeout result, unconditional drain, and final-byte start-token case (`:1526–1553`). The recorded timeout-drain mismatch is fixed. Other GCSD differences remain open.

No checks or tests were run during this independent pass. The handoff records a PowerPC test-target check and scoped `git diff --check` passed; tests were compiled but not run. No hardware verification was performed.


## Implementation pass: initialization defaults and public helper ABI (2026-10-06)

**BUG FIX.** The two named source blockers from the global parity scan are implemented. The per-drive speed, page-size, and addressing state now starts at C's zero-initialized values. `sdgecko_initIODefault` sets 16 MHz and byte addressing without changing page size, and later `sdgecko_initIO` attempts preserve the previous page size until a successful CSD read or explicit `sdgecko_setPageSize`, matching `sdgecko_io.c`.

The crate now also owns the defined public `card_io.h` and `card_buf.h` ABI surface: `g_CID`, `g_CSD`, `g_CardStatus`, `g_mCode`, `g_dCode`; init/pre-I/O, CID/CSD/status, sector I/O, unmount, callback, speed/page-size/addressing functions; and the three-node 514-byte buffer pool. CID, CSD, and card-status reads update the exported arrays. Sector I/O returns the underlying protocol status instead of collapsing it to a Boolean. `sdgecko_writeSector` is intentionally not exported because it is declared but has no definition in the checked-in C implementation.

Safety divergence: the exported Rust sector-I/O entry points reject null buffers with `CARDIO_ERROR_INTERNAL`, while the corresponding C code would dereference them and enter undefined behavior. `sdgecko_freeBuffer` also ignores pointers that are not exact pool-buffer starts rather than reproducing invalid-pointer arithmetic. These checks do not change valid-call behavior.

Validation in this sandbox is static only because no Rust toolchain is installed. `git diff --check` passes. Independent PowerPC compilation, probe/runtime validation, and source re-review are still required before `ogc-gcsd` can move to `reviewed`.
