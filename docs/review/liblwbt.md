# lwbt-sys static review

Reviewer: `thread-current/reviewer_4`  
Pass: first-pass static review  
Scope: `lwbt-sys` public BTE/HID transport behavior on the PowerPC target, with the corresponding libogc BTE API and implementation.

## Current review status — 2026-10-05 broader parity scan

**Result: NEEDS_CHANGES.** The report-capacity, PCB-capacity, remote disconnect queue, callback ordering, partial-disconnect, MTU, and `senddata` framing fixes pass this source re-review. The two-stage channel sequence is present, but `connectasync` and `listenasync` still block until completion and invoke their callbacks before returning, unlike the C asynchronous API. See the independent re-review below.

The fixed-size Rust report storage remains intentionally bounded for memory safety. The current 672-byte caller-data limit is treated as an allowed safety deviation rather than exact C behavior. C itself does not reject `len > 672` in `bte_senddata`/`bte_sendmessage[async]`; its L2CAP writer limits the transmitted packet to the negotiated outbound MTU (`../../C-sources/libogc/lwbt/bte.c:907-991`; `lwbt/l2cap.c:1314-1319`).

`lwbt-sys` remains open for an async submission/callback timing fix. This re-review was static; it did not run tests or hardware checks.

## 2026-10-05 two-stage HID connection/listen parity fix

C completes HID setup in two observable stages. `bte_connectasync` establishes only the HID control L2CAP channel and runs its callback when that channel completes; `bte_connectasync_step2` then establishes the HID data channel and runs the replacement callback. `bte_listenasync` and `bte_listenasync_step2` use the same control-first/data-second sequence for accepted inbound channels (`../../C-sources/libogc/lwbt/bte.c:766-905,1118-1171`).

The PowerPC Rust backend now follows the same channel sequence. Stage 1 completes the baseband connection and only the control L2CAP channel before reporting completion. Stage 2 requires the control channel to be ready, then establishes only the interrupt/data channel and reports that completion. The listen path accepts and configures the inbound control channel in stage 1 and the inbound data channel in stage 2. The ACL dispatcher starts only after both stages complete, so it cannot consume stage-2 signaling while setup is still in progress.

This removes the last confirmed non-safety blocker from the broader `lwbt-sys` comparison. The change still needs independent re-review and target/hardware validation; it is not recorded as `reviewed` yet.

## 2026-10-05 HID L2CAP MTU parity fix

C advertises `L2CAP_MTU = HIDD_N + 1 = 673` in HID L2CAP configuration requests. Rust now derives `HID_L2CAP_MTU` from its 673-byte HIDP payload capacity and advertises that value for both control and interrupt channels instead of 64. A target regression asserts the derived value is 673. This closes the MTU-negotiation parity finding.

The crate remains `parity_blocked` because the two-stage control/data connect/listen behavior still differs from C. The `senddata` framing mismatch is fixed.

## Confirmed mismatch

### `sendmessage` and `sendmessageasync` use the interrupt data path instead of the HID control report queue

- C API evidence: `../../C-sources/libogc/gc/bte/bte.h:180-182` declares distinct `bte_senddata`, `bte_sendmessage`, and `bte_sendmessageasync` operations.
- C behavior: `../../C-sources/libogc/lwbt/bte.c:907-926` prefixes `bte_senddata` payloads with `HIDP_TRANS_DATA | HIDP_DATA_RTYPE_OUPUT` and writes them to the data L2CAP PCB. In contrast, `bte_sendmessageasync` at lines 929-956 and `bte_sendmessage` at lines 959-991 prefix payloads with `HIDP_TRANS_SETREPORT | HIDP_DATA_RTYPE_OUPUT`, enqueue them on the control request queue, and complete only after a HIDP handshake; the synchronous form waits for that result.
- Rust behavior: `crates/compatibility/lwbt-sys/src/lib.rs:889-898` routes `sendmessage` to `senddata`, including on PowerPC. `sendmessageasync` at lines 903-930 also routes to `senddata` and calls its completion callback immediately when the transport send returns `ERR_OK`. `crates/compatibility/lwbt-sys/src/powerpc_backend.rs:875-907` emits the interrupt-channel `HIDP_TRANS_DATA` report through `send_l2cap_hid_data`.
- Result: these APIs send a different HIDP transaction on the interrupt channel, and async completion means local transport acceptance rather than remote HIDP handshake completion. This differs from the C control-channel SET_REPORT protocol and its callback/wait ordering.

## Review state

The crate remains open with a confirmed mismatch requiring a fix. No implementation files were changed. No build, test, or hardware verification was performed, as instructed.

## 2026-10-02 HID messaging source-of-truth recheck

The uploaded Rust baseline already routes `sendmessage` and `sendmessageasync` through HID control SET_REPORT and waits for a successful HID handshake before completion. The older interrupt-channel messaging mismatch is stale for the current source-of-truth tree.

## 2026-10-02 queued control-message parity follow-up

The earlier SET_REPORT/channel fix is extended to C's asynchronous queue semantics. Current C allocates control requests from a 256-entry pool, appends requests per PCB, sends only the queue head, and advances to the next request after a successful HID handshake. Rust now uses the same 256-request global capacity and per-PCB FIFO heads/tails instead of allowing only one pending control message per PCB.

Synchronous requests wait on their own queued request result; asynchronous requests retain their callback until their HID handshake. Disconnect/error completion drains queued requests with `ERR_CLSD`, while successful handshakes pop one request and send the next. This removes the prior observable `ERR_MEM` result for a second otherwise-valid request merely because the first handshake was pending.

Historical finding at this point in the review: Rust still used fixed inline storage with a 64-byte BTE message limit. **Fixed later in this note** by the C-sized HID report change; this is no longer a current 64-byte blocker. The remaining bounded 672-byte limit is documented above as an intentional safety deviation.

## Independent re-review — queued control-message parity

**Result: NEEDS_CHANGES.** Reviewer: `/root/reviewer_f`. Implementer: `/root/worker_b`. Commits: `21bf10e`, `937a0b4`, and `8acd74a`.

For normal success, Rust matches C's HID control SET_REPORT transaction, FIFO queue, successful-handshake completion, and next-request dispatch (`C-sources/libogc/lwbt/bte.c:187–307, 929–991`; `lwbt-sys/src/powerpc_backend.rs:473–658, 1253–1269`). The 256-entry request pool also matches C (`btopt.h:277`; Rust `powerpc_backend.rs:33`).

Historical re-review result: two mismatches were still present at this point. **Both are fixed later in this note.** C drains queued requests with `ERR_CLSD` after both L2CAP channels disconnect (`bte.c:1059–1080`, queue drain `:187–204`). Rust drains on explicit local disconnect and global shutdown (`powerpc_backend.rs:214–224, 1147–1171`), but its ACL dispatcher ignores signaling packets and read errors only yield (`:412–470`); remote disconnect can leave synchronous waiters and async callbacks pending. Also, Rust rejected reports longer than 64 bytes (`powerpc_backend.rs:76, 1292–1293`), while C's configured L2CAP MTU is 673 bytes and permits substantially larger reports (`btopt.h:309–310, 339`). **The 64-byte limit is fixed later in this note.** Local disconnect also drains callbacks before issuing HCI disconnect, while C drains after L2CAP disconnection (`bte.c:747–763, 1059–1080`).

Current tests cover routing, channel readiness, and handshake status parsing, but not FIFO completion, disconnect draining, callback timing, or larger reports (`powerpc_backend.rs:1348–1397`). `cargo check -p lwbt-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled tests; tests did not run. Host `cargo test --no-run` failed because the host linker rejected PowerPC flags. No hardware verification was performed. C also configures 14 PCB slots while Rust had four (`btopt.h:243, 274`; `lwbt-sys/src/lib.rs:207`). **The PCB capacity and its per-PCB queue indexing are fixed later in this note.**

## Implementer handoff — Drain queued control requests on remote L2CAP disconnect

Implementer: `/root/worker_b`. Independent re-review: needed; review thread owns it.

C handles a remote L2CAP disconnection request by replying, closing the matching channel, and notifying BTE. BTE drains its queued control requests only after both HID control and data channels close, completing synchronous waiters and asynchronous callbacks with `ERR_CLSD` (`../../C-sources/libogc/lwbt/l2cap.c:670–706`; `bte.c:1059–1082`; queue cleanup `bte.c:187–204`). Rust's ACL dispatcher previously ignored signaling-channel packets, so remote disconnects did not reach its existing control queue cleanup. Rust now parses those requests, validates the local and remote CIDs, sends the disconnection response, and invokes the existing `ERR_CLSD` queue drain after the last channel closes. At this point in the history, report size and callback timing remained separate findings. **The 64-byte report limit and disconnect callback-order finding are fixed later below.**

Regression coverage closes the two channels in order and verifies queued control requests remain pending after the first close, then complete with `ERR_CLSD` after the second. `git diff --check` and `cargo check -p lwbt-sys --tests --target powerpc-unknown-linux-gnu` passed. The PowerPC test target compiled; tests were not run. `rustfmt --edition 2021 --check` reports existing formatting differences in the source file outside this change. No hardware verification was performed. The shared ledger is managed by the coordinator.

## Implementer handoff — Support C-sized HID reports

Implementer: `/root/worker_b`. Independent re-review: needed; review thread owns it.

C configures `HIDD_N` as 672 bytes and `L2CAP_MTU` as 673 bytes (`../../C-sources/libogc/lwbt/btopt.h:309–310, 339`). Both `bte_senddata` and `bte_sendmessage[async]` add one HIDP header byte before sending the caller's message (`bte.c:907–991`), so the largest report data payload is 672 bytes. Rust previously limited send data and control messages to 64 bytes, stored control payloads in 65-byte arrays with an 8-bit length, and used a 128-byte ACL scratch buffer. Rust now accepts up to 672 message bytes, stores the complete 673-byte HIDP control payload with a `u16` length, and uses an ACL buffer large enough for the payload and protocol headers. Interrupt-data framing now matches C and always adds the DATA|OUTPUT header before the caller buffer.

Regression coverage checks a report larger than 64 bytes, the 672-byte message / 673-byte wire payload boundary, and rejection above the C limit. `git diff --check` and `cargo check -p lwbt-sys --tests --target powerpc-unknown-linux-gnu` passed. The target test code compiled; tests were not run. `rustfmt --edition 2021 --check` reports existing formatting differences in other parts of the backend. No hardware verification was performed. The shared ledger is managed by the coordinator.

## Independent re-review: remote disconnect and HID report size (`9737bf7`, `35de063`)

**Result: PASS for these bounded fixes.** Reviewer: `worker_a`. Implementer: `worker_b`.

The ACL dispatcher now accepts signaling packets for a known handle, validates the requested local and remote CIDs against the active channel, replies with the matching L2CAP disconnection response, and closes that channel (`crates/compatibility/lwbt-sys/src/powerpc_backend.rs:436–538`). It drains queued control requests with `ERR_CLSD` after both control and interrupt channels close, matching C's `l2cap_disconnected_ind` and `__bte_close_ctrl_queue` behavior (`../../C-sources/libogc/lwbt/l2cap.c:670–706`; `bte.c:187–204,1059–1082`). The per-channel test covers keeping requests pending after the first close and draining them after the second. The payload update raises the caller-data limit to C's configured 672-byte `HIDD_N`, allocates room for the extra HIDP header, uses a `u16` stored length, and enlarges ACL scratch buffers (`btopt.h:309–310,339`; Rust `powerpc_backend.rs:34–35,72–91,1294–1428`). Its test covers 65 bytes, the 672-byte boundary, and rejection at 673 bytes.

I inspected both complete implementation diffs and the regression code. No checks or tests were run during this re-review; the implementer's note records a successful PowerPC test compilation, with tests not executed. The four-PCB versus C's fourteen-PCB capacity finding was still open at this point, but **is fixed by the later fourteen-entry PCB and high-slot queue changes below.** No hardware verification was performed.

## Implementer handoff: fourteen-entry BTE PCB capacity

Implementer: `thread-current/root`. Independent reviewer: pending assignment. The Rust BTE PCB allocator and PowerPC transport table now hold fourteen entries, matching the C capacity recorded in `btopt.h:243,274`. Shutdown, lookup, allocation, dispatch, and free paths use the same capacity. A target regression allocates fourteen unique PCBs, verifies the next allocation fails, and frees all allocated entries. The then-recorded callback-timing finding remained open at this point and was fixed by the later disconnect-order work. The broader transport differences listed in the current-status section above remain open. Independent static re-review is required.

Validation: `cargo check -p lwbt-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled target tests; tests did not run. Host `cargo test` is blocked because this crate and its dependencies are PowerPC-only. No hardware verification was performed.

### Independent re-review of fourteen-entry PCB capacity (`1dbc9aa`)

**Result: NEEDS_CHANGES.** Reviewer: `reviewer_gamma`. Implementer: `thread-current/root`.

C sets `MAX_NUM_CLIENTS` to six and `MEMP_NUM_BTE_PCB` to `2 + 2 * MAX_NUM_CLIENTS`, or fourteen (`../../C-sources/libogc/lwbt/btopt.h:243,274`). Rust's PCB allocator and transport table were expanded to fourteen, but the per-PCB `CONTROL_HEAD` and `CONTROL_TAIL` arrays remained four entries. Queue operations index those arrays using the PCB slot, so control requests for slots 4 through 13 could access memory out of bounds. The arrays are now sized from `crate::PCB_SLOTS`, and a target regression enqueues and pops a request on the final PCB slot.

## Follow-up handoff: high-slot control request queue

Implementer: `thread-current/root`. Independent reviewer: pending. Per-PCB control queue head and tail arrays now use the same fourteen-slot constant as the allocator and transport table. The regression exercises enqueue and pop on slot 13, then verifies that the queue is empty. Callback timing and other recorded Bluetooth differences remain open.

Validation: `cargo check -p lwbt-sys --tests --target powerpc-unknown-linux-gnu` passed and compiled target tests; tests did not run. Host tests are blocked by PowerPC-only dependencies. No hardware verification was performed.

### Independent re-review: high-slot control request queue (`d6c1e9b`)

**Result: PASS for the fourteen-entry pool and queue fix.** Reviewer: `reviewer_gamma`. Implementer: `thread-current/root`.

C's pool formula evaluates to fourteen entries, and Rust now uses that capacity for PCB allocation, transport state, and control queue heads/tails. The target regression exercises enqueue and pop on the final pool index and confirms it returns to the empty state. The then-recorded disconnect callback-timing finding was fixed later below. Other Bluetooth differences are now enumerated in the current-status section above. Target test code compiled; it did not run.

## BUG FIX handoff — wait for L2CAP disconnect responses before draining callbacks

Implementer: `thread-current/root`. Independent static re-review: `/root/review_lwbt` found the partial-disconnect send-failure state issue; follow-up fix is pending independent review.

C `bte_disconnect` requests the data-channel disconnect first, then requests the control-channel disconnect after the first L2CAP confirmation. After both channels close, C drains the queued control requests with `ERR_CLSD`, calls the disconnect confirmation callback, and then sends HCI disconnect (`../../C-sources/libogc/lwbt/bte.c:747-763,1059-1113`; L2CAP request format and confirmation handling: `../../C-sources/libogc/lwbt/l2cap.c:1254-1287`). Rust now sends the same sequential L2CAP requests. It matches responses by signal ID and both CIDs, closes one channel per response, and drains the request callbacks only after both channels close. It then calls the disconnect confirmation callback and submits HCI disconnect. A successful HIDP handshake during disconnect can finish the active request but cannot send the next queued request. A new send after disconnect starts returns `ERR_CLSD`. If a L2CAP request cannot be transmitted, Rust records the transport error in `pcb.err` and leaves queued request callbacks pending; it restores `CONNECTED` only if both L2CAP channels are still active. A partial disconnect stays in `DISCONNECTING`.

The target regression checks the production response-state helper: a mismatched signal ID leaves the channel active, the first valid response closes only the interrupt channel and does not complete disconnect, and the second valid response reports completion. The Dolphin Bluetooth probe records this same deterministic state check through a probe-only feature that uses the production helper.

`cargo check -p lwbt-sys --features dolphin-automation --tests --target powerpc-unknown-linux-gnu` passed and compiled target test code; tests did not run. The Bluetooth DOL build compiled the probe source but failed at link time on existing undefined `AddDevice`, `FindDevice`, and `RemoveDevice` symbols from `ogc-sysbase`; no DOL was produced or run. A plain `cargo check -p ogc-system-probe --target powerpc-unknown-linux-gnu` is blocked by the target's no-std unwind configuration; the `xtask dolphin-validate` build reached linking. No hardware verification was performed.

### Independent re-review: L2CAP disconnect callback timing (`a9e1c1b`)

**Result: NEEDS_CHANGES.** Reviewer: `/root/review_lwbt`. Implementer: `thread-current/root`.

The successful local disconnect sequence matches C's ordering: Rust requests the interrupt/data channel first, sends the control-channel request after the first matching response, waits for both channels to close, drains queued control callbacks, invokes `disconn_cfm`, and then submits HCI disconnect (`../../C-sources/libogc/lwbt/bte.c:747–763, 1059–1113`; Rust `crates/compatibility/lwbt-sys/src/powerpc_backend.rs:516–526, 992–1034, 1529–1563`). A successful HIDP handshake while disconnecting completes only the active request and does not dispatch the next queued request (`powerpc_backend.rs:475–490, 879–915`). The response handler matches the outstanding signal identifier and both CIDs, and ignores stale or mismatched responses (`powerpc_backend.rs:607–657`). The probe assertion calls the production response-state helper and checks that completion waits for the second channel response (`probes/ogc-system-probe/src/bluetooth_probe.rs:42–46`).

The review found one transport failure mismatch. After the first response closes the interrupt channel, failure to send the next control-channel request could clear the pending-disconnect state and set the PCB back to `HIDP_STATE_CONNECTED` even though only the control L2CAP channel remained active (`powerpc_backend.rs:521–526, 711–729, 935–978`). C leaves the BTE PCB in `STATE_DISCONNECTING` when an L2CAP disconnect request fails; `l2cap_disconnect_cfm` ignores that request's error (`bte.c:747–763, 1090–1107`). The next implementer handoff records the fix. The current regression covers response order, CIDs, and the production failure state update; DOL execution remains blocked as described below.

The PowerPC test target compiled in the implementer's validation, but its tests did not execute. The Dolphin probe source compiled, but DOL linking remains blocked by existing unresolved `ogc-sysbase` symbols `AddDevice`, `FindDevice`, and `RemoveDevice`; no DOL was produced or run. No hardware verification was performed.

## BUG FIX handoff — preserve partial disconnect state after transport failure

Implementer: `thread-current/root`. Independent reviewer: `/root/review_lwbt` identified this mismatch in commit `f65f377`; the follow-up passed independent review in commit `1779311`.

The independent review found that a failed control-channel disconnect request after the interrupt channel had closed called `abort_local_disconnect`, clearing `disconnecting` and setting the PCB to `CONNECTED` with only one active L2CAP channel. The error path now records `pcb.err` and preserves the private pending-disconnect flag and public `DISCONNECTING` state after any signaling send failure. `disconnect()` sets the public state before sending the first request, matching C. `senddata` returns `ERR_CLSD` while disconnecting. The target regression and Dolphin probe check the production error-recording helper.

### Independent re-review: partial disconnect send failure (`1779311`)

**Result: PASS for the partial-disconnect failure-state fix.** Reviewer: `/root/review_lwbt`. Implementer: `thread-current/root`.

Rust now sets the PCB to `HIDP_STATE_DISCONNECTING` before starting L2CAP teardown. A signaling send failure records `pcb.err` and keeps both the PCB and private transport state disconnecting. It no longer restores `CONNECTED` after the first channel has closed, or after the initial request fails. `senddata` and `sendmessage` return `ERR_CLSD` while disconnecting. This matches C's visible disconnecting state and its closed-error checks in `bte_disconnect` and the send APIs (`../../C-sources/libogc/lwbt/bte.c:747–763, 907–968`; Rust `crates/compatibility/lwbt-sys/src/powerpc_backend.rs:1520–1572, 1600–1614, 1700–1717`).

The production error-recording helper is covered by the disconnect-order regression and the Dolphin assertion. `cargo check -p lwbt-sys --features dolphin-automation --tests --target powerpc-unknown-linux-gnu` compiled the target tests; tests did not run. The Dolphin source compiled, but DOL linking remains blocked by existing unresolved `ogc-sysbase` symbols `AddDevice`, `FindDevice`, and `RemoveDevice`; no DOL was produced or run. No hardware verification was performed. The previously fixed 64-byte payload, PCB-capacity, remote-disconnect, callback-order, partial-disconnect, and 64-versus-673 L2CAP MTU findings are closed. The remaining non-safety difference is the two-stage connection/listen semantics listed in the current-status section above. The `senddata` framing difference is fixed.


## 2026-10-05 parity closure

The recorded non-safety lwBT parity findings are resolved. The HID L2CAP control and interrupt channels advertise C's 673-byte MTU, `senddata` always prepends the HIDP DATA|OUTPUT header, and outgoing and incoming HID setup use C's two-stage control-then-data completion sequence. Earlier report-capacity, PCB-capacity, remote-disconnect, disconnect-ordering, and partial-disconnect fixes remain in place. The 672-byte caller-data bound is retained as an explicit safety-only deviation. Target compilation for `lwbt-sys` tests and `xtask`, `cargo xtask review-status`, and `git diff --check` were reported passing on 2026-10-05. This closes the recorded lwBT review scope as `reviewed`; hardware acceptance remains a separate project gate.

## 2026-10-05 independent re-review — async connection and listen contract

**Result: NEEDS_CHANGES. Reviewer: `worker_d`.** I compared the complete two-stage implementation in `crates/compatibility/lwbt-sys/src/powerpc_backend.rs` with the C entry points and callbacks in `../../C-sources/libogc/lwbt/bte.c`. The report-size, 14-PCB capacity, queued-control disconnect, partial-disconnect state, 673-byte L2CAP MTU, and `senddata` DATA/OUTPUT framing fixes agree with their recorded C behavior (`powerpc_backend.rs:1699-1734`; `bte.c:907-926`).

The two-stage channel selection is correct, but the public asynchronous contract still differs. C `bte_connectasync` starts the control-channel request with `l2ca_connect_req` and returns its submission result (`bte.c:766-801`); `l2cap_connected` calls `conn_cfm` when that request completes (`bte.c:1122-1143`). C's `bte_listenasync` registers the control-channel listener and returns (`bte.c:839-870`); `l2cap_accepted` calls `conn_cfm` only when a peer connects (`bte.c:1146-1170`). Rust `connectasync` instead waits for the baseband event and control-channel configuration inside the function, then invokes `conn_cfm` before returning (`powerpc_backend.rs:1483-1534`). `listenasync` likewise waits for a peer and configures the channel before returning (`powerpc_backend.rs:1614-1665`). This changes call blocking and callback ordering and can re-enter a caller that expects completion after the function returns. Keep `lwbt-sys` open until the API returns after submission and completion arrives through the callback, or document a deliberate public API change.

The current 672-byte caller-data bound remains an accepted safety deviation: Rust rejects larger caller reports before forming a HIDP payload, while C's lower L2CAP layer limits packet transmission to the negotiated MTU (`../../C-sources/libogc/lwbt/bte.c:907-991`; `../../C-sources/libogc/lwbt/l2cap.c:1314-1319`). The broader Bluetooth event and device behavior remains runtime-unverified. No checks or tests were run in this re-review; no hardware verification was performed.

## 2026-10-05 async contract fix handoff

Implementer: `notOrrytrout`, commit `176b8ea`. The PowerPC backend no longer performs the connection/listen waits inside the public `connectasync`, `listenasync`, `connectasync_step2`, or `listenasync_step2` calls. Each entry point reserves the setup operation, submits the immediate transport work, schedules the remaining HCI/L2CAP setup on the Rust-owned connection worker, and returns the submission result. The worker owns the later baseband/L2CAP waits and invokes the stored `conn_cfm` with the final result. This removes the bounded blocking/callback-in-entry-point behavior identified by `worker_d` while keeping the two-stage control-then-data sequence.

The implementation does not link or call the C tree. The existing 673-byte MTU, HIDP DATA|OUTPUT framing, 14-PCB capacity, disconnect-ordering, and partial-disconnect fixes remain unchanged. The 672-byte caller-data bound remains an accepted safety deviation. Independent source re-review plus runtime validation of callback ordering and completion results is required before returning `lwbt-sys` to `reviewed`.
