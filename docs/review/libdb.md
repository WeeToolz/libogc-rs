# `libdb-sys` callback and TCP read DOL probe

The existing `debugger_probe` covers the copied-interface callback through production code. It starts the `libdb-sys` listener, copies its interface while `fhndl` is negative, and invokes the read and write callbacks through the copied callback fields. It also checks the shared TCP read-size validator that `iface_read` uses.

The probe does not replace or simulate the network backend. Dolphin's IOS network backend did not provide a connected TCP peer for the listener. A probe attempt to connect to the reported host IP timed out, so a DOL check cannot invoke a real TCP read on an accepted socket. The negative-size guard remains covered by the callback's shared validator and the PowerPC-target regression test. The DOL probe checks this production helper directly; it does not claim to validate an accepted socket or a network read.

## Timer and connection lifecycle review

C `opentcpip` accepts a connection, then calls `tcpip_starttimer` for that accepted socket. `tcpip_starttimer` sets the shared `tcpip_time` only when it is zero and an active or time-wait PCB exists. The C poll callback advances TCP timers after `UIP_TCP_TMR_INTERVAL` and clears `tcpip_time` when no such PCB remains. `waittcpip` calls `tcpip_stoptimer`; `closetcpip` stops the timer, closes the PCB, and sets `fhndl` to -1.

Rust starts `TcpClock` when it receives the first SYN, before the connection is accepted, and `iface_open` calls `tcpip_starttimer` again after accept. The second start is idempotent. This matches C's `UIP_TCP_REG` call for the new active PCB, which invokes `tcpip_tmr_needed` before the handshake completes. Rust close removes the connection state and its pending expiry; the shared clock remains available for other connections. A poll-order gap was found and fixed: C drains available BBA frames before it advances TCP timers, while Rust previously advanced timers first and read only one frame. Rust now drains available frames before timer work. Host tests cover the drain order and error continuation.

The first BBA-profile attempt did not apply the requested slot setting: its wrapper put the override after `--exec`, and the recorded runner arguments show the standard `Dolphin.Core.SlotB=7`. Its timeout diagnostic in `docs/dolphin-runs/2026-11-26-libdb-bba-init/` is not BBA evidence. A corrected wrapper places `Dolphin.Core.SlotB=12` before `--exec`; the wrapper is saved with the run evidence in `docs/dolphin-runs/2026-11-26-libdb-bba-slot12-retry/`. This DOL reached the probe summary, then timed out while `ProbeDisplay::capture_page` sent a page through USB Gecko. It produced no summary, so it does not prove the TCP checks passed. The standard Dolphin profile passed the debugger probe with 22 checks and zero failures. It reported no usable EXI BBA, so it verified the missing-adapter path only. Evidence: `docs/dolphin-runs/2026-11-26-libdb-timer-order/suite.json`.

Probe source: `probes/ogc-system-probe/src/debugger_probe.rs`. Run with:

```sh
cargo xtask dolphin-validate --probe debugger_probe
```

`cargo xtask dolphin-validate --probe debugger_probe` passed on 2026-10-04. Report: `target/dolphin-validation/1791171565-3b913c3450aa/suite.json`; result: `status=pass`, `summary_line=PASS summary passes=13 fails=0`. The probe verifies copied no-handle callbacks and the shared size validator. It does not verify a TCP accept or a network read because the Dolphin IOS backend did not establish a local peer.
