# `ogc-bt-sync` static parity review

Date: 2026-10-01

## Scope and method

This is a read-only source review of every Rust source module in `crates/services/ogc-bt-sync/src` and the related C Bluetooth code in `C-sources/libogc/lwbt`. The review traced HCI setup, pairing and ACL handling, L2CAP signaling and channel state, HID report paths, IOS transport ownership, and shutdown. It checked the earlier fixes recorded in this file's prior version. No source code or ledger was changed. No build, test, or hardware check was run for this review.

The C code provides the generic Bluetooth HCI/L2CAP stack and the OH1 USB physical bus interface. It does not provide the Rust Switch-controller HID host, its report decoding, or its Wii IOS polling wrapper. Findings below compare only behavior with a corresponding C path. They do not claim parity for the Rust-only controller behavior.

## Rust module and API coverage

| Rust source | Public surface and behavior reviewed | C comparison |
|---|---|---|
| `src/lib.rs:1-24` | `no_std`, module exports, PowerPC gates, public re-exports | No direct C API equivalent. |
| `src/protocol.rs:4-440` | Public HCI/L2CAP/HIDP constants, address/class/event types, packet and signal models, channel state | HCI definitions and events in `lwbt/hci.h`, `hci.c`; L2CAP definitions/state in `lwbt/l2cap.h`, `l2cap.c`. |
| `src/protocol.rs:443-580` | ACL/L2CAP/signal/HIDP parsers, including signal payload variants and unknown codes | `l2cap_input` and signal dispatch in `lwbt/l2cap.c:785-840, 350-749`; HCI event parsing/dispatch in `lwbt/hci.c`. |
| `src/protocol.rs:582-922` | HCI command, ACL/L2CAP signaling and HIDP builders; HCI event parser | C command builders and event dispatch in `lwbt/hci.c`; L2CAP signaling builders and dispatch in `lwbt/l2cap.c`. |
| `src/hid_host.rs:5-139` | Public host/event/failure/diagnostic API and private link/command state | Generic HCI/L2CAP state in `lwbt/hci.c`, `lwbt/l2cap.c`; Switch HID policy has no C counterpart. |
| `src/hid_host.rs:141-422` | Host construction, reset/configuration sequence, search, disconnect, slot control | `bte.c:1374-1430,1553-1598`; `hci.c` command procedures; C tracks generic connections, not the same four Rust HID slots. |
| `src/hid_host.rs:423-778` | HCI event dispatch, pairing/key handling, inquiry and connection transitions | HCI event structures and callbacks in `hci.h`, `hci.c`; startup in `bte.c`. |
| `src/hid_host.rs:780-1207` | ACL credits and fragmentation/reassembly, L2CAP/HID channel state, signaling and HID reports | ACL/L2CAP framing and signal state in `l2cap.c:350-749,785-840`; HIDP has no matching host state machine in this C scope. |
| `src/request.rs:1-110` | Owned async request buffers, callback result and transfer copy rules | `physbusif.c:180-249,299-330` uses C callbacks and fixed transfer pools; ownership model differs. |
| `src/transport.rs:1-1013` | USB/IOS transport, asynchronous and synchronous APIs, command helpers, pending-event ring, IOS ioctlv paths | OH1 endpoints, async reads/writes, and HCI callback paths in `physbusif.c:180-249,299-330`; device lifecycle in `bte.c`. |
| `src/wii_host.rs:1-275` | Polling service, pending event/ACL reads and writes, deadlines and shutdown | C `BTE_Shutdown` and `physbusif` lifecycle; Rust's IOS polling/service design has no one-to-one C wrapper. |

Protocol unit-test source in `protocol.rs:924-1420` and host unit-test source in `hid_host.rs:1209-1634` were read as part of source review only. They were not run.

## Shared behavior and API coverage

- Rust's HCI opcode values and command setup flow map to the corresponding C HCI operations. The Rust host performs reset, buffer-size setup, local version/address/features reads, key setup, event-mask setup, and inquiry/page scan configuration (`hid_host.rs:141-302`); C startup is in `bte.c:1374-1430,1553-1598`. The two stacks have different ownership and callback models.
- Rust parses and builds little-endian HCI ACL and L2CAP headers (`protocol.rs:443-500,582-731`). C consumes ACL data and reconstructs L2CAP frames in `l2cap.c:785-840`. Rust's ACL reassembly and outbound fragmentation are in `hid_host.rs:794-967`; its channel dispatch is in `hid_host.rs:968-1207`.
- Inquiry result field offsets and the 14-byte record stride match C's inquiry-result handling (`hci.c:1975-2030`) for the supported non-RSSI result format. Rust also parses the RSSI form. That format is not used to claim a mismatch.
- Rust's transport opens the Wii OH1 device and uses endpoint numbers corresponding to the C default device and endpoints (`physbusif.c:180-235`). C continuously manages USB callbacks and fixed pools; Rust submits IOS ioctlv requests and owns buffers through callbacks (`request.rs:1-110`, `transport.rs:21-197,229-418`). These are different API layers and lifecycle designs.
- Rust adds four HID slots, the Wii controller pin rule, SSP responses, Switch controller discovery, HID control/interrupt channel setup, and HID report events (`hid_host.rs:423-778,968-1207`). These additions have no corresponding generic `lwbt` HID-host API to compare.

## Confirmed remaining mismatches

1. **Peer L2CAP disconnection tears down the HCI link before the response can be sent.** Rust handles a peer `DisconnectionRequest` by calling `disconnect(slot)` first (`hid_host.rs:1139-1152`). That queues an HCI disconnect and removes the link and queued ACL packets immediately (`hid_host.rs:369-397`). It then builds an L2CAP response, but `queue_acl` cannot resolve the removed handle (`hid_host.rs:794-812,399-408`). C sends the L2CAP response, marks only that L2CAP PCB closed, and reports the channel disconnect (`l2cap.c:670-706`). Thus Rust can fail to send the required response and closes the whole ACL connection instead of only the channel.

2. **L2CAP disconnection response CIDs are reversed.** For a request with destination CID then source CID, Rust calls the response builder with `(dest_cid, source_cid)` (`hid_host.rs:1144-1150`), but `build_l2cap_disconnection_response` serializes its second argument before its first (`protocol.rs:696-707`). This produces `[request source CID, request destination CID]`. C serializes `[pcb->scid, pcb->dcid]`, which corresponds to `[request destination CID, request source CID]` (`l2cap.c:695-700`). The Rust protocol test at `protocol.rs:1328-1347` expects the reversed ordering, so it records the mismatch.

3. **Unknown L2CAP signaling command codes received no Command Reject.** The Rust parser preserves unknown codes as `Unknown` (`protocol.rs:564-568`), but the host ignored them (`hid_host.rs:1154`). C's default signaling-command case sends `L2CAP_CMD_REJ` with `L2CAP_CMD_NOT_UNDERSTOOD` (`l2cap.c:739-745`). The fix now handles this case, including Information command codes `0x0A` and `0x0B`; `/root` independently accepted the correction.

4. **Resolved: Configuration Response continuation with a non-success result.** The original Rust path disconnected before processing `flags`; the current path sends C's empty continuation request when continuation is set. The fix and independent review are recorded below.

5. **Resolved: Configuration Request options across continuation segments.** The original Rust path did not keep a per-channel option accumulator. The current path stores pending response options and evaluates them when the final segment arrives, matching C. The fix and independent review are recorded below.

### BUG FIX: reject unknown L2CAP signaling command codes

The Rust host ignored unknown signaling commands. It now queues an L2CAP Command Reject with reason `L2CAP_CMD_NOT_UNDERSTOOD` and preserves the received command identifier, matching C's default signaling-command case (`lwbt/l2cap.c:739-745`). It excludes only Command Reject and Echo Request/Response, which C handles outside its default case. Information Request/Response codes `0x0A` and `0x0B` now receive rejects. Host regression coverage checks the response channel, command, identifier, and reason for both Information codes and an unknown code. `cargo test -p ogc-bt-sync` and the PowerPC target `cargo check` passed after this correction. No hardware verification was performed. Implementer: `/root/worker_a`. Independent reviewer: `/root` accepted the correction.

## Earlier fixes re-reviewed

- `WiiHidHost::shutdown` records an immediate reset submission error and continues adapter cleanup (`wii_host.rs:102-120`). The error path is present.
- `HidHost::disconnect` now checks `slot >= HID_SLOTS` before indexing (`hid_host.rs:369-372`). The bounds fix is present.
- A peer Configuration Request only marks `remote_configured` after the final segment clears the continuation bit (`protocol.rs:389-399`), matching C's final-request handling (`l2cap.c:510-540`). The current source also accumulates response options across those segments, as confirmed in the independent re-review below.
- Configuration Response `source_cid` is matched against the stored peer CID (`protocol.rs:400-409`), and the continued empty request is addressed to that peer CID (`hid_host.rs:1124-1136`). This is consistent with the CID roles in the response and request. C's continuation branch writes `pcb->scid` (`l2cap.c:657-668`), which appears to use its local CID; this is a C/Rust difference and was not treated as a Rust defect.
- A single-segment unknown mandatory configuration option is reported as `L2CAP_CFG_UNKNOWN` with the option echoed (`hid_host.rs:1050-1103,1158-1207`). C has the same behavior (`l2cap.c:484-501,530-540`). Multi-segment accumulation is confirmed in the independent re-review below.

## Uncertain differences and limits

- **External USB HCI and BLE status:** production transport still opens the Wii IOS OH1 Bluetooth device at `/dev/usb/oh1/57e/305`; there is no USB descriptor discovery, generic USB host-controller driver, or BLE HCI/L2CAP/GATT implementation in this crate. `HciEndpointAddresses` validates endpoint address shape and direction before the configurable OH1 transport uses addresses. This is a boundary check only: it does not discover descriptors, verify interrupt/bulk transfer types, or provide support for arbitrary USB Bluetooth dongles. BLE support remains open until a real controller transport and the required LE host procedures are implemented and exercised. The existing Rust Switch HID path is Bluetooth Classic HID and must not be described as BLE support.
- Primary implementation sources assessed for a possible BLE port: [TrouBLE](https://github.com/embassy-rs/trouble) documents central scan/connect, GATT client operations, and SMP security; [bt-hci](https://github.com/embassy-rs/bt-hci) defines the reusable HCI controller boundary and lists a USB transport crate. Trouble is async Embassy-based and documents Rust 1.97 as its current stable MSRV. These sources establish a possible stack reuse path, not compatibility with this repository's polling IOS runtime or proof that USB host descriptor discovery is available on Wii.
- `HciEndpointAddresses` rejects endpoint zero, reserved address bits, incorrect IN/OUT direction, and duplicate event/ACL IN addresses. `BtSyncHost::open_with_endpoints` validates before opening the device. The descriptor constructor closes its consumed descriptor if validation fails and reports a close failure. Portable protocol tests cover the validation contract; the IOS transfer path remains hardware-unverified.
- Rust's IOS ioctlv direction/count choices for USB reads and writes (`transport.rs:355-418,949-1013`) cannot be proven equivalent to C's `USB_Read*`/`USB_Write*` calls from these source paths alone. The IOS OH1 ioctl contract and runtime behavior were not checked.
- C uses fixed Arena buffers and pool sizes in `physbusif.c`; Rust allocates owned buffers and caps its event/ACL transfers (`request.rs:1-110`, `transport.rs:21-62`). The behavioral impact under device load was not measured.
- Rust's event parser intentionally models only a subset of HCI events as named variants and preserves other events as unknown (`protocol.rs:198-254,733-922`). All event-specific host actions were traced in `hid_host.rs:423-778`, but this review does not prove behavior for every controller/firmware event ordering.
- Rust has stricter packet length checks in `protocol.rs:443-568` than the C pbuf paths. Padding/trailing-byte behavior in all IOS and controller combinations was not measured.
- C pairing and generic L2CAP services expose broader callback/procedure coverage than this HID host. APIs and events without a Rust peer, and Rust-specific HID APIs without a C peer, are outside direct parity claims.
- No build, tests, emulator, Wii adapter, timing, throughput, or memory-pressure checks were run. Hardware-only and runtime-dependent behavior remains unchecked.

## Completeness check

The module inventory was checked with `rg --files crates/services/ogc-bt-sync/src`; every listed Rust module is in the coverage table. Public declarations and relevant state-machine branches were inspected across `protocol.rs`, `hid_host.rs`, `transport.rs`, `request.rs`, `wii_host.rs`, and `lib.rs`. The corresponding shared C paths were traced through `lwbt/hci.h`, `lwbt/hci.c`, `lwbt/l2cap.h`, `lwbt/l2cap.c`, `lwbt/physbusif.h`, `lwbt/physbusif.c`, and `lwbt/bte.c`. The review did not inspect unrelated C Bluetooth profiles as if they were part of this crate's shared implementation.


## Independent re-review: unknown signaling command reject

Reviewer: `/root`; implementer: `/root/worker_a`. The initial re-review found that the first implementation also excluded Information Request/Response codes `0x0A` and `0x0B`, which C sends through its default reject path. The implementation has been corrected to exempt only Command Reject and Echo Request/Response. Regression coverage now includes `0x0A`, `0x0B`, and an unassigned code. `/root` independently confirmed that the corrected condition, response wire format, identifier, reason, and regression coverage match C.

`hid_host.rs` builds the reject payload through `protocol.rs:710-718`. The corrected match sends rejects for `0x0A` and `0x0B`, in line with C's default case; `l2cap.h:37-38` identifies these as Information commands, and `l2cap.c:739-745` rejects them. C handles Command Reject and Echo Request/Response in explicit cases (`l2cap.c:295-302,721-728`), so those remain excluded. `/root` accepted the corrected condition and regression coverage.

Validation after correction: `cargo test -p ogc-bt-sync` and the PowerPC target `cargo check` passed. No hardware verification was performed.


## 2026-10-01 peer-disconnect parity consolidation

The first two peer-disconnect findings in the historical section above are resolved in the current source. A matching peer Disconnection Request queues the L2CAP response before channel state is changed, closes only the matching active HID channel, preserves the ACL link and the other channel, and emits the per-channel disconnect event. `build_l2cap_disconnection_response` serializes the request destination/local CID first and the request source/remote CID second, matching C's `scid`/`dcid` response payload. Reserved HID CIDs that are no longer active now take the Invalid CID Command Reject path.

At that review point, the configuration-response continuation and multi-segment option-accumulation findings still needed resolution. Both fixes and their independent review are now recorded below.

## 2026-10-02 configuration continuation parity follow-up

The two L2CAP configuration findings below passed independent review of the current source.

- Configuration Request options are accumulated per HID channel across continuation segments. Intermediate segments still receive C's empty success response. When the final segment arrives, the accumulated unknown/non-hint response options are returned together and the channel is marked remotely configured only after a final successful result.
- Configuration Response handling no longer disconnects merely because an intermediate response is non-success. With the continuation bit set, Rust sends the empty continuation request for success, unacceptable, unknown, reject, and other result codes as C does. A final `L2CAP_CFG_UNACCEPT` retries configuration; final reject/unknown failures disconnect.

Focused host tests cover an unknown mandatory option arriving in a non-final request segment and a continued non-success response. The implementation remains subject to hardware/controller interoperability validation.

### Independent re-review: L2CAP configuration continuation

**Result: PASS for the two configuration continuation findings.** Reviewer: `thread-current/root`. Implementer: `notOrrytrout` (`f4cd27b`, `e7b04b8`).

The C request path stores response options in `pcb->cfg.opt` across continuation segments, sends an empty success response for each non-final request segment, and evaluates the accumulated options when the final segment arrives (`../../C-sources/libogc/lwbt/l2cap.c:445-540`). Rust keeps the accumulated options and unknown-option state in the pending per-channel configuration, sends the empty success response on continuation, and includes the accumulated unknown options in the final response (`crates/services/ogc-bt-sync/src/hid_host.rs:1103-1164`). The regression sends an unknown mandatory option in the first segment and checks that the final response returns it with `L2CAP_CFG_UNKNOWN` (`hid_host.rs:1944-1991`).

C's response path allows non-success continuation responses to continue and sends an empty configuration request while the continuation bit is set; final unacceptable responses retry, while final reject/unknown results disconnect (`l2cap.c:603-668`). Rust now follows that order and result handling (`hid_host.rs:1170-1205`). Its regression sends a continued `L2CAP_CFG_UNKNOWN`, then checks that the link remains and a continuation request is queued (`hid_host.rs:1994-2020`).

Validation: `cargo test -p ogc-bt-sync` passed all 47 tests. `cargo check -p ogc-bt-sync --tests --target powerpc-unknown-linux-gnu` passed and compiled target tests. No hardware verification was performed. Other `ogc-bt-sync` findings remain open.

## 2026-10-13 endpoint validation follow-up

`HciEndpointAddresses` is a portable, tested contract used by the production OH1 transport constructors. It validates nonzero endpoint numbers, reserved address bits, transfer direction roles, and distinct event/ACL IN addresses. `open_with_endpoints` validates before opening the IOS path. `from_file_descriptor` validates before it stores the descriptor and closes the consumed descriptor on invalid input; a close error is returned as an IPC error.

Validation: `cargo test -p ogc-bt-sync` passed 87 tests. `cargo check -p ogc-bt-sync --target powerpc-unknown-linux-gnu` passed. `cargo fmt --check` and `git diff --check` passed. No Wii hardware or external USB HCI device was available, so the IOS transport behavior remains hardware-unverified. This change does not implement USB descriptor discovery, arbitrary external USB HCI support, Bluetooth LE, or GATT.
