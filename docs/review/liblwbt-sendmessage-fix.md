# lwbt `sendmessage` report-routing fix handoff

Implementer: `/root/worker_b`.

## BUG FIX

C `bte_sendmessage` and `bte_sendmessageasync` prefix the report with `HIDP_TRANS_SETREPORT | HIDP_DATA_RTYPE_OUPUT` (`0x52`) and send it on the control L2CAP PCB. Rust now routes both APIs to the control channel's remote CID and uses the same SET_REPORT header. `senddata` continues to use HIDP DATA (`0xa2`) on the interrupt channel. Target regression coverage checks both route/header pairs and rejects a control channel that is not ready.

## HIDP completion handoff

The PowerPC backend now starts one LWP ACL dispatcher after L2CAP setup completes. The dispatcher routes HIDP input reports from the interrupt CID to the registered receive callback and observes packets on the control CID. It completes a pending SET_REPORT only on `HIDP_TRANS_HANDSHAKE` with `HIDP_HSHK_SUCCESSFULL`. `sendmessage` waits cooperatively for this result; `sendmessageasync` returns after local submission and invokes its callback when the dispatcher receives the successful handshake. A local send error calls the async callback with that error. A disconnect completes a pending request with `ERR_CLSD`.

The implementation permits one outstanding SET_REPORT per PCB and returns `ERR_MEM` if another arrives before the first handshake. C queues multiple control requests, so request queuing remains a parity difference for independent review.

## Validation

Validation: `cargo check -p lwbt-sys --target powerpc-unknown-linux-gnu` passed, including with `RUSTFLAGS="-D warnings"`. `cargo test -p lwbt-sys --target powerpc-unknown-linux-gnu --no-run` compiled the tests but could not link them because the configured host linker does not accept the PowerPC linker options. Tests did not run. The target-only regression checks the SET_REPORT and interrupt data routes and verifies that only a successful HIDP handshake packet counts as completion. No hardware verification was performed.

Implementer: `worker_b`. Independent reviewer: `/root` (pending).


## 2026-10-03 follow-up: queued control-request parity

The earlier statement that Rust permits only one outstanding SET_REPORT per PCB is superseded by commit `8acd74a`. The PowerPC backend now allocates from a 256-request pool, keeps FIFO queues per PCB, sends only the queue head, and advances after a successful HIDP handshake. Synchronous calls wait for their request result. Asynchronous callbacks run after handshake completion. This matches the inspected C queue and completion order.

The fixed inline payload capacity remains smaller than the C pbuf capacity and remains open. Implementation commits: `21bf10e`, `937a0b4`, and `8acd74a`. Implementer: `worker_b`. Independent static re-review is needed. The recorded target test compilation did not run tests, and no hardware verification was done.
