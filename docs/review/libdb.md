# `libdb-sys` callback and TCP read DOL probe

The existing `debugger_probe` covers the copied-interface callback through production code. It starts the `libdb-sys` listener, copies its interface while `fhndl` is negative, and invokes the read and write callbacks through the copied callback fields. It also checks the shared TCP read-size validator that `iface_read` uses.

The probe does not replace or simulate the network backend. Dolphin's IOS network backend did not provide a connected TCP peer for the listener. A probe attempt to connect to the reported host IP timed out, so a DOL check cannot invoke a real TCP read on an accepted socket. The negative-size guard remains covered by the callback's shared validator and the PowerPC-target regression test. The DOL probe checks this production helper directly; it does not claim to validate an accepted socket or a network read.

Probe source: `probes/ogc-system-probe/src/debugger_probe.rs`. Run with:

```sh
cargo xtask dolphin-validate --probe debugger_probe
```

`cargo xtask dolphin-validate --probe debugger_probe` passed on 2026-10-04. Report: `target/dolphin-validation/1791171565-3b913c3450aa/suite.json`; result: `status=pass`, `summary_line=PASS summary passes=13 fails=0`. The probe verifies copied no-handle callbacks and the shared size validator. It does not verify a TCP accept or a network read because the Dolphin IOS backend did not establish a local peer.
