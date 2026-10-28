# Peer L2CAP disconnection handling fix

Finding: `docs/review/libbt-sync.md`, confirmed mismatch 1.

Implementer: `worker_b`. Independent reviewer: `/root`.

C handles a matching peer Disconnection Request by sending the L2CAP Disconnection Response, marking that L2CAP PCB closed, and reporting the channel disconnect (`C-sources/libogc/lwbt/l2cap.c:670-706`). It does not disconnect the HCI ACL link. Rust previously called `disconnect(slot)` before building the response. That removed the link and queued ACL data, so `queue_acl` could not send the response and Rust also issued an HCI disconnect.

Rust now queues the response first, then resets only the local HID channel named by the request's destination CID. It keeps the ACL link and the other HID channel. The link is no longer marked ready while one required HID channel is closed. It emits `HidEvent::ChannelDisconnected` with the closed control or interrupt channel, without emitting the whole-link `Disconnected` event.

Regression coverage checks matching CID response order, channel close event, and ACL-link retention. The unmatched-CID regression checks the Command Reject reason and CID payload, preserves both channel states and the ACL link, and emits no channel-close event.

Validation: `cargo test -p ogc-bt-sync` passed all 42 tests. `cargo check -p ogc-bt-sync --target powerpc-unknown-linux-gnu` passed. `git diff --check` passed. No hardware verification was performed. The remaining configuration mismatches need review. This crate remains open for independent static re-review.

## Independent re-review

The response order, identifier, and CIDs match C. Rust queues the response before it changes channel state. It preserves the ACL handle and other HID channel, and it does not queue an HCI disconnect. These changes fix the response-loss and whole-link teardown defects.

The re-review found that the channel close had no matching event for the caller. Rust now exposes `HidChannel::{Control, Interrupt}` and emits `HidEvent::ChannelDisconnected` after it closes the matching channel. `WiiHidHost::next_event` already forwards protocol events, so consumers receive this event through the existing API. The regression test checks the exact channel event and confirms that no whole-link event follows.

Implementer: `worker_b`. Independent reviewer: `reviewer a`; accepted in the re-review section below.

Validation after the event change: `cargo test -p ogc-bt-sync` passed all 41 tests, and `cargo check -p ogc-bt-sync --target powerpc-unknown-linux-gnu` passed. No hardware verification was performed.

## Independent re-review: channel close event

Reviewer: `/root`. Implementer: `worker_b`, commit `7204103`.

Rust now emits `HidEvent::ChannelDisconnected` with `HidChannel::Control` or `HidChannel::Interrupt` after it closes the matching local channel. This represents C's per-PCB `L2CA_ACTION_DISCONN_IND` callback without reporting an ACL disconnect. `WiiHidHost::next_event` forwards the event. The regression test checks the control-channel event and confirms that no whole-link disconnect event follows. This event fix matches the reviewed C behavior.

## Fix handoff: unmatched destination CID

Implementer: `worker_b`. Independent reviewer: `reviewer a`; accepted in the re-review section below.

C searches for a local PCB whose local CID matches the request destination CID. If none matches, it sends Command Reject with `L2CAP_INVALID_CID`, followed by the requested local CID and `L2CAP_NULL_CID` (`C-sources/libogc/lwbt/l2cap.c:670-693`, constants in `l2cap.h:74`). Rust now sends the same reject for unmatched control/interrupt CIDs and leaves the ACL link and channel states unchanged. Matching CIDs still receive a Disconnection Response, close only that channel, and produce `HidEvent::ChannelDisconnected`.

The new regression covers the reject code, identifier, reason, requested CID, null remote CID, preserved link/channel state, and absence of a channel-close event. All 42 host tests and the PowerPC target check passed. No hardware verification was performed. Other configuration findings remain open.

## Independent re-review: unmatched CID handling

Reviewer: `/root`. Implementer: `worker_b`, commit `a045e4d`.

For a destination CID outside the two Rust HID local CIDs, Rust emits the C Invalid CID Command Reject with the request identifier, requested destination CID, and null source CID. The regression checks this payload and confirms that the link and channel states do not change.

One state-dependent mismatch remains. Rust treats either reserved HID local CID as a matched channel even when that channel has no active L2CAP connection. `HidL2capState::new` stores both local CIDs while `connected` is false, but `close_local_channel` and the request handler check only the CID value. C searches `l2cap_active_pcbs` for a matching live PCB (`l2cap.c:681-687`) and sends Invalid CID when none exists. Thus a request for CID `0x0040` or `0x0041` before that channel is active gets a Disconnection Response and event in Rust where C sends Command Reject. Keep this finding open and test a reserved but inactive CID.

## Reserved CID activity handoff

Implementer: `worker_b`. Independent reviewer: `/root/reviewer_5` (re-review pending).

Rust now tracks whether each local HID L2CAP channel has an active PCB. The channel becomes active when Rust starts its outbound connection request or accepts an inbound HID channel request; a successful connection response also marks it active. Closing a channel resets it to inactive while retaining its reserved local CID. The peer Disconnection Request handler now responds and closes a channel only when the destination CID matches an active channel. If the CID is reserved but inactive, it sends the C-compatible Invalid CID Command Reject with the requested local CID and unknown remote CID.

Added a regression that first disconnects an active control channel, then sends a second request to the same reserved CID and checks for Invalid CID. The existing matching-active-channel test still checks the Disconnection Response, channel close, and channel event.

Validation: `cargo test -p ogc-bt-sync` passed all 43 tests. `cargo check -p ogc-bt-sync --target powerpc-unknown-linux-gnu` passed, and `git diff --check` passed. No hardware verification was performed.


## Current-source recheck

The reserved-CID activity handoff is present in the synchronized source. `local_channel_is_active` requires both the matching local CID and the channel's active flag, and the request handler uses that predicate before sending a Disconnection Response. A second request to a reserved CID after the channel is closed therefore receives the same Invalid CID reject that C emits when no active PCB matches. The response builder also writes local/destination CID before remote/source CID. No additional Bluetooth source edit was needed in this pass.

## Independent re-review: NEEDS_CHANGES

Reviewer: reviewer_d. Implementer: worker_b. Implementation commit: `1bfc6f9` (`Track active Bluetooth HID channels`).

The active/inactive reserved-CID change matches the C PCB lookup. C searches `l2cap_active_pcbs` for an entry whose local `scid` equals the request destination CID, and rejects with `L2CAP_INVALID_CID` plus the requested CID and `L2CAP_NULL_CID` when no active PCB matches (`C-sources/libogc/lwbt/l2cap.c:670-694`; `l2cap.h:74`). Rust only handles a reserved HID CID when its per-channel `active` flag is true (`crates/services/ogc-bt-sync/src/protocol.rs:334-365,387-390`; `hid_host.rs:1206-1244`). Rust sets this flag when it starts an outbound connection request and when it accepts an inbound HID connection, and also on a successful connection response (`hid_host.rs:813-831,1018-1045`; `protocol.rs:404-418`). Closing a channel resets it to inactive while preserving its reserved CID (`protocol.rs:392-401`). This represents pending and connected C PCBs as active. The new regression exercises a second disconnection request after closing the control channel and confirms the Invalid CID reject, preserved ACL link, and absence of a second close event (`hid_host.rs:1497-1529`).

One request-response mismatch remains for a matching active CID. C builds the Disconnection Response from the matched PCB's stored `pcb->scid` and `pcb->dcid` (`l2cap.c:678-704`). Rust instead passes the incoming request's `dest_cid` and `source_cid` directly into the response builder (`hid_host.rs:1206-1230`; `protocol.rs:727-738`). These are equal during a normal request, which is all the existing matching-CID test checks (`hid_host.rs:1404-1435`). If a peer sends a matching local destination CID but an incorrect source CID, C responds with its stored remote CID while Rust echoes the incorrect source CID. For example, if the active control channel stores local CID `0x0040` and remote CID `0x0050`, but the request contains destination `0x0040` and source `0x0051`, C responds with `(0x0040, 0x0050)` and Rust responds with `(0x0040, 0x0051)`. This is an observable response difference. Rust should source the response fields from the matched channel state to match C. The current tests do not exercise a mismatched request source CID. Keep the crate open for implementation and independent re-review.

Validation: `cargo check -p ogc-bt-sync --target powerpc-unknown-linux-gnu` passed. Targeted tests `inactive_reserved_peer_cid_gets_invalid_cid_reject` and `peer_channel_disconnect_queues_response_and_keeps_acl_link` both ran and passed. Full `cargo test -p ogc-bt-sync` ran 45 tests: 44 passed and the existing `continued_non_success_configuration_response_does_not_disconnect` test failed at `hid_host.rs:1932` because `next_event()` returned a queued `Connected` event instead of `None`; this test is outside the disconnect change. No hardware verification was performed. Result: `NEEDS_CHANGES`; the active/inactive behavior passes static review, but the response CID source mismatch remains.

## Independent check: active-channel response CID source

Reviewer: `reviewer_e`. Implementer: `worker_b` (`1bfc6f9`; commit author `notOrrytrout`). This bounded check verifies only reviewer_d's open request-response CID finding. The active/inactive state and per-channel event fixes are inspected as current context; they are not re-audited here.

**Conclusion: NEEDS_CHANGES; the mismatch is confirmed.** In C, the Disconnection Request handler reads the request destination CID, searches `l2cap_active_pcbs` for `pcb->scid == dcid`, and does not use the request's source CID for the matched response (`../../C-sources/libogc/lwbt/l2cap.c:670–694`). On a match it writes `pcb->scid` and `pcb->dcid` into the response payload (`:695–704`). Thus an active PCB with local `scid=0x0040` and remote `dcid=0x0050` replies `(0x0040, 0x0050)` even if the request carries a different source CID.

Rust's handler correctly requires the request destination CID to match an active reserved local channel (`crates/services/ogc-bt-sync/src/hid_host.rs:1206–1222`), and its channel state retains the local and peer CIDs (`protocol.rs:334–365, 387–401`). However, it builds the response by passing the incoming `dest_cid` and `source_cid` directly (`hid_host.rs:1222–1230`); the builder serializes those two values in order (`protocol.rs:727–738`). For a request `(dest=0x0040, source=0x0051)` against stored channel state `(local=0x0040, remote=0x0050)`, C responds `(0x0040, 0x0050)` and Rust responds `(0x0040, 0x0051)`. The active-channel state fix in `1bfc6f9` does not change this response construction.

The related response/event path queues the ACL response before closing the Rust channel, then emits a per-channel `ChannelDisconnected` event (`hid_host.rs:1222–1236`); C sends the response before marking the PCB closed and invoking its disconnect indication (`l2cap.c:695–704`). Those ordering/event changes do not correct the mismatched response source CID. The existing active-channel regression uses the stored peer CID in its request (`hid_host.rs:1397–1450`), so it cannot detect this case. The inactive-CID regression covers the separate rejection path (`:1497–1529`). No existing test supplies an incorrect request source CID while retaining a matching active destination CID.

Validation: `cargo test -p ogc-bt-sync peer_channel_disconnect_queues_response_and_keeps_acl_link` ran and passed 1 targeted test; that test exercises only equal request/stored source CIDs. `cargo check -p ogc-bt-sync --target powerpc-unknown-linux-gnu` passed. No hardware verification was performed. This finding remains open for implementation; no implementation or ledger file was changed.

## Fix handoff: matched active channel response CID

Implementer: `worker_a`. Independent reviewer: pending. Re-review is required.

Rust now builds a Disconnection Response from the matched active channel's stored local and remote CIDs. It no longer copies the request's source CID into the response. The active-channel regression now sends a source CID that differs from the channel's stored remote CID and checks that Rust replies with the stored value, matching C's use of `pcb->scid` and `pcb->dcid` (`C-sources/libogc/lwbt/l2cap.c:695–704`).

Validation: `cargo test -p ogc-bt-sync peer_channel_disconnect_queues_response_and_keeps_acl_link` passed (1 test). `cargo check -p ogc-bt-sync --target powerpc-unknown-linux-gnu` passed. `git diff --check` passed. No hardware verification was performed.

## Independent re-review and adjacent queue audit (2026-10-03)

**Bounded fix: PASSED. Remaining crate state: NEEDS_CHANGES.** Reviewer: `/root/reviewer_e`. Implementer: `/root/worker_a`. Fix commit: `5fccb49`.

The matched active-channel response CID fix matches C. C matches an active PCB by its local source CID and writes that PCB's stored local and remote CIDs into the response (`C-sources/libogc/lwbt/l2cap.c:670–704`). Rust now builds the response from the matched link's stored CIDs (`ogc-bt-sync/src/hid_host.rs:1206–1236`); the regression supplies a request source CID different from the stored peer CID. The normal response-before-close ordering and per-channel disconnect event also match the inspected C flow.

One separate queue-full error-path mismatch remains. If Rust's outgoing ACL queue is full, `queue_acl` calls `fail(QueueFull)`, which removes all links (`hid_host.rs:257–274, 833–851`). The peer-disconnection handler then unwraps that removed link (`:1230–1236`), which can panic before it completes channel cleanup or emits the disconnect event. In C, if `btpbuf_alloc` fails while constructing the Disconnection Response, the handler does not close the channel or emit the indication; it returns without the Rust panic (`C-sources/libogc/lwbt/l2cap.c:695–705`).

The adjacent audit used static source inspection only; it ran no checks or tests. The earlier bounded-fix test passed and PowerPC check passed as recorded above; those checks were not rerun. No hardware verification was performed. Keep the crate open for the queue-full behavior and any other recorded findings.
# Independent current-source confirmation of queue-full path (2026-10-03)

Reviewer: `/root/reviewer_f`. No follow-up code fix is present; latest implementation commit is `5fccb49` (matched response CID). On an active disconnection request, Rust queues the response before closing the channel (`crates/services/ogc-bt-sync/src/hid_host.rs:1206-1236`). If the queue is full, `queue_acl` calls `fail(QueueFull)`, which removes all links; the handler then unwraps the removed link and panics (`hid_host.rs:833-843,257-275,414-425,1232`). C only closes the PCB and reports the disconnect inside the successful `btpbuf_alloc` branch (`C-sources/libogc/libogc/l2cap.c:695-704`); allocation failure leaves the channel unchanged and does not panic. This finding remains confirmed. Reviewer_f ran no checks or tests; the targeted response-CID test does not exercise queue exhaustion. No hardware verification was performed.

## BUG FIX handoff: preserve channel state when the response queue is full

Implementer: `/root/worker_a`. Independent re-review is needed; the review thread handles it. The peer Disconnection Request path now treats failure to queue its Disconnection Response as a failed response attempt. It leaves the active channel, ACL link, and ready state unchanged and emits no `ChannelDisconnected` event. This matches C's path, which closes the channel and reports the indication only when response-buffer allocation succeeds. Other BT configuration and signaling findings remain separate.

Added a regression that fills the outgoing ACL queue, sends a matching peer Disconnection Request, and checks that the link and both channels remain ready, the queue stays full, no HCI disconnect is queued, and no disconnect event is emitted. `cargo test -p ogc-bt-sync full_outgoing_queue_leaves_peer_disconnection_channel_unchanged` passed (1 test). `cargo check -p ogc-bt-sync --target powerpc-unknown-linux-gnu` passed. `git diff --check` passed. No hardware verification was performed.

### Independent re-review of queue-full state fix `41d2b14`

**Result: NEEDS CHANGES.** Reviewer: `/root/reviewer_d`; implementer: `/root/worker_a`.

This supersedes the earlier queue-full finding, which compared against commit `5fccb49` before `41d2b14` changed the path. Rust now preserves the channel when it cannot enqueue the response into its ACL queue (`crates/services/ogc-bt-sync/src/hid_host.rs:837-855,1222-1241`). C leaves the channel unchanged only when allocating the response data pbuf fails. Once that allocation succeeds, C calls `l2cap_signal` and then closes the PCB and emits `DISCONN_IND` regardless of the send result (`C-sources/libogc/lwbt/l2cap.c:695-704`). Rust's queue-full case occurs after the response has already been built, so it is analogous to C's later signal/transmit failure, where C still closes. The regression asserts the mismatching Rust behavior and does not establish parity (`hid_host.rs:1464-1497`).

## BUG FIX handoff: close channel when response transmission queue is full

Implementer: `/root/worker_a`. Independent reviewer: pending. This supersedes the queue-full behavior in fix `41d2b14`: when Rust successfully builds the Disconnection Response but cannot enqueue it because the outgoing ACL queue is full, Rust now closes the matching channel and emits `ChannelDisconnected`. It retains the ACL link and the other HID channel. This matches C, which closes the PCB and reports the disconnect after response-buffer allocation regardless of the later `l2cap_signal` result (`C-sources/libogc/lwbt/l2cap.c:695-704`). A response-build failure still leaves channel state unchanged.

Updated the full-queue regression to verify channel closure and the disconnect event, preservation of the ACL link and other channel, a full ACL queue, and no HCI disconnect request. `cargo test -p ogc-bt-sync full_outgoing_queue_still_closes_peer_disconnection_channel` passed (1 test). `cargo check -p ogc-bt-sync --target powerpc-unknown-linux-gnu` and `git diff --check` passed. No hardware verification was performed.

### Independent re-review of queue-full disconnect behavior (`ab81639`)

**Result: PASS for this fix.** Reviewer: `/root/worker_b`. Implementer: `/root/worker_a`.

C allocates and builds the L2CAP Disconnection Response first. If that allocation succeeds, C calls `l2cap_signal`, then closes the matching PCB and issues its disconnect indication without using the signal return value to cancel cleanup (`../../C-sources/libogc/lwbt/l2cap.c:670–706`). Rust has already built the response before `try_queue_acl`; it now treats `QueueFull` as a failed transmission attempt, closes only the matching local channel, retains the ACL link and other channel, and emits `ChannelDisconnected` (`crates/services/ogc-bt-sync/src/hid_host.rs:1206–1242`). The updated queue-full regression checks these observable state and event results (`:1465–1506`). This resolves the recorded queue-full mismatch. Other Bluetooth findings in this note remain open.

No checks or tests were run during this independent pass. The handoff records that the focused test ran and passed, the PowerPC check passed, and `git diff --check` passed. No hardware verification was performed.

The earlier validation note below this handoff described the intermediate `41d2b14` behavior and is superseded by the independent re-review above and the latest handoff regression.


## Implementer handoff: six-controller host capacity

Implementer: `thread-current/root`. Independent reviewer: pending assignment. The HID host now allocates six simultaneous links, matching C's six WPAD slots (`../../C-sources/libogc/wiiuse/wpad.h:39-48`; Rust `crates/services/ogc-bt-sync/src/hid_host.rs`). A regression allocates slots 0 through 5 and verifies the seventh allocation fails. This fixes the four-link limit that prevented WPAD channels 4 and 5 from receiving HID events. The broader Bluetooth configuration and callback findings remain open. Independent static re-review is required.

Validation: `cargo test -p ogc-bt-sync controller_allocator_uses_all_six_wpad_slots` passed. `cargo check -p ogc-bt-sync --tests --target powerpc-unknown-linux-gnu` passed and compiled target tests. The full `cargo test -p ogc-bt-sync` run had 46 passes and one pre-existing failure: `continued_non_success_configuration_response_does_not_disconnect` received a queued `Connected` event instead of `None`. No hardware verification was performed.

### Independent re-review: six-controller allocation (`b69ca98`)

**Result: PASS for the six-slot capacity change.** Reviewer: `reviewer_beta`. Implementer: `thread-current/root`.

C defines six WPAD slots and initializes Wiiuse with `WPAD_MAX_DEVICES` (`../../C-sources/libogc/gc/wiiuse/wpad.h:39-48`, `wpad.c:93-95,1288`). Rust uses the six-slot host constant consistently for links, keys, allocation, bounds checks, and lifecycle loops. The regression fills slots 0 through 5 and confirms a seventh controller cannot be allocated. Other Bluetooth configuration and callback findings remain open. The PowerPC target test code compiled; it was not executed.

## Bluetooth test suite fixture correction

The full `cargo test -p ogc-bt-sync` suite now passes all 47 tests. The prior failure in `continued_non_success_configuration_response_does_not_disconnect` was caused by the test leaving the setup `Connected` event queued, then expecting no event after the response. The test now consumes and checks that initial event before exercising the continued response; it then verifies that no later event is emitted. This changes test setup only and does not weaken the behavior assertion.


## Independent re-review: matched active-channel response CIDs

**Result: PASS for the bounded response-CID fix.** Reviewer: `reviewer_beta`. Implementer: `worker_a` (commits `f40d1d7` / `5fccb49`).

C locates the active PCB by matching its local `scid` to the request destination CID. It then writes the PCB's stored `scid` and `dcid` into the Disconnection Response (`../../C-sources/libogc/lwbt/l2cap.c:670-704`). Rust now requires an active matching local CID and builds the response with that matched channel's stored local and remote CIDs (`crates/services/ogc-bt-sync/src/hid_host.rs:1206-1236`). The regression deliberately sends a request source CID that differs from the stored peer CID, then checks that the response contains the stored CID, preserves the ACL link, closes only the control channel, leaves the interrupt channel ready, and emits the per-channel disconnect event (`hid_host.rs:1404-1453`). I found no remaining mismatch in this bounded response-CID behavior.

The implementation handoff records that the focused test passed and the PowerPC target check passed. Those checks were not rerun for this re-review; I ran no tests or checks. No hardware verification was performed. Other Bluetooth callback and configuration findings remain open.

## 2026-10-05 completion review

The recorded `ogc-bt-sync` HCI/L2CAP scope is now `reviewed`. Peer disconnect handling, CID ordering, unknown-command rejection, configuration continuation, queue-full cleanup, six-controller capacity, and callback/configuration behavior have fixes or completed source reconciliation. No confirmed non-safety source mismatch remains in this crate. Real controller/IOS timing and firmware combinations remain hardware validation.
