# GCSD residual mismatch audit

Reviewer: `/root/reviewer_4`  
Date: 2026-10-01  
Scope: independent static audit of remaining `ogc-gcsd` mismatches, excluding the already-reviewed command fix. The workspace contains `ogc-gcsd` but no `ogc-gcsd-sys` crate. No implementation changes, builds, tests, or hardware checks were performed. The shared `libgcsd.md` note and ledger were not edited.

## Confirmed residual mismatches

1. **EXI lock contention returns instead of waiting.** Rust `select_slot` returns `None` immediately if `exi::lock` fails (`crates/services/ogc-gcsd/src/wii_backend.rs:286-296`), causing callers to return a no-card error. C `__exi_wait` retries `EXI_Lock` and sleeps on the per-drive LWP queue when another owner holds the lock (`C-sources/libogc/libogc/sdgecko_io.c:265-273`). It is used before command and data exchanges, including `__card_dataread`/`__card_multidatawrite`/`__card_multiwritestop`. This is a confirmed observable contention behavior difference.

2. **CRC transfer delay is missing.** Rust `data_read` transfers the block and immediately requests its two CRC bytes (`wii_backend.rs:546-609`); Rust `data_write` sends the block and immediately writes the CRC (`:610-651`). C sleeps for one microsecond between those transfers for reads and writes (`sdgecko_io.c:714-729` and `:772-783`). This is an explicit protocol-timing difference in the source.

3. **Resolved: operating-condition timeout attempt.** Commit `8f0df8b` adds one final ACMD41/CMD1 attempt after the timed loop. Independent review confirms Rust reads and checks the response, returns success when IDLE clears, and returns I/O error when IDLE remains, matching C `__card_sendopcond` (`sdgecko_io.c:918-963`; Rust `wii_backend.rs:833-884`). The target check passed and compiled the tests; they did not run.

4. **Write-protect mode is incomplete outside the accepted CMD0 fix.** Rust's write-protect retry is limited to CMD0 wire format. Its generic command path still sends ordinary command bytes, CRC, and `0xff` idle bytes (`wii_backend.rs:707 onward`); C `__card_writecmd` inverts the command bytes, complements the CRC, and uses `_ioClrFlag` (zero in write-protect mode) for its idle bytes (`sdgecko_io.c:349-382`). Rust multiwrite stop also always sends `0xfd` (`wii_backend.rs:654 onward`), while C changes that token to `0x02` when `_ioWPFlag` is set (`sdgecko_io.c:807-809`). The multiwrite data token remains `0xfc` in both implementations. Thus the residual gap is specifically generic command encoding and multiwrite-stop encoding when write-protect mode is active; the CMD0 fix alone does not cover them.

5. **The public C helper surface is not represented.** `card_io.h` declares initialization/pre-I/O, CID/CSD/status, unmount, speed, page-size, and addressing helpers, plus CSD/CID arrays (`C-sources/libogc/gc/sdcard/card_io.h:30-58`). The Rust crate exposes the three `DiscInterface` descriptors and their callbacks (`crates/services/ogc-gcsd/src/lib.rs:35-125`), with the protocol helpers private in `wii_backend.rs`. Therefore consumers of those C helper APIs do not have direct Rust equivalents. This is an API coverage difference; some listed C callbacks have no public setter in the checked-in headers, and `sdgecko_writeSector` is declared without a definition in this C tree, so those specific items need external-consumer context before assigning impact.

## Checked differences not reported as residual bugs

- EXI probe waiting matches C: Rust loops while `EXI_ProbeEx` returns zero (`wii_backend.rs:143-157`); C does the same in `__card_check` (`sdgecko_io.c:1140 onward`).
- Rust's stricter `send_opcond` handling of failed APP_CMD is observable: Rust propagates the result (`wii_backend.rs:838-842`), while C ignores it before issuing ACMD41 (`sdgecko_io.c:924-929`). This favors explicit error handling and is recorded as a Rust-side behavior correction, not a confirmed parity defect.
- `clear_status` differs intentionally: the C wrapper returns `true` with a FIXME (`gcsd.c:92-95`); Rust clears its stored error state and calls SD status (`wii_backend.rs:1234-1242`).
- The slot descriptors' device type, slot feature bits, callback routing, and C wrapper startup/read/write/shutdown flow match the checked C descriptors (`gcsd.c:41-100,104-226`; Rust `lib.rs:35-125`).

## Result

The lock-wait, CRC delay, and remaining write-protect protocol differences are upheld by direct source comparison. The final operating-condition attempt is fixed and independently reviewed. The helper API omission is confirmed at the declaration/export level, with impact dependent on whether compatibility with those low-level public C helpers is in scope. Keep the GCSD review open. The residual audit itself ran no compile or tests; the separate final-attempt review compiled its tests but did not run them. No hardware verification was performed.

## 2026-10-02 residual write-protect parity follow-up

The residual protocol findings were rechecked against the current source. EXI lock waiting and the one-microsecond CRC transfer delays were already fixed in the uploaded Rust baseline, so the older items 1 and 2 above are stale.

The remaining write-protect wire-format gap is now fixed. Generic commands use one shared formatter that applies the C command-bit setup and CRC first, then inverts all command bytes and complements the CRC in write-protect mode while transmitting zero-valued idle bytes. Multiwrite stop now sends `0x02` instead of `0xfd` in write-protect mode and uses the mode-specific idle byte for the four clock transfers and busy polling. Normal mode remains unchanged.

The low-level public helper/API-surface difference remains separate. Hardware SD Gecko validation is still required.
