# L2CAP disconnection response CID parity fix

Finding: `docs/review/libbt-sync.md`, confirmed mismatch 2.

Implementer: `worker_b`. Independent reviewer: `reviewer a` (primary reviewer); the fix passed independent static review below.

C sends the Disconnection Response CIDs in `[pcb->scid, pcb->dcid]` order (`C-sources/libogc/lwbt/l2cap.c:695-700`). For a peer request, those values correspond to the request's destination CID followed by its source CID. Rust passed those request CIDs to `build_l2cap_disconnection_response` in that order, but the builder serialized the second argument first. The builder now serializes `local_cid` then `remote_cid`; its regression test checks the parsed response preserves the request CID order.

Validation: `cargo test -p ogc-bt-sync` passed all 40 tests. `cargo check -p ogc-bt-sync --target powerpc-unknown-linux-gnu` passed. No hardware verification was performed.

## Independent re-review

Reviewer: `/root`.

I compared Rust `build_l2cap_disconnection_response` and its caller with C `l2cap.c:695-700`. C writes `pcb->scid` then `pcb->dcid`; for the incoming request, those are the request destination CID then source CID. Rust now passes and serializes those values in the same order. The regression test checks both serialized CID fields. I found no remaining mismatch in this fix's scope. The separate teardown-before-response issue remains open.

Other confirmed L2CAP findings remain open, including the peer disconnection teardown ordering, configuration continuation response handling, and configuration option accumulation. This crate remains open.
