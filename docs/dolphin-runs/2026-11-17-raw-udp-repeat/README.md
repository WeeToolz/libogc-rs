# Raw UDP loopback in Dolphin

Dolphin 2609a on macOS with OpenGL passed 187 checks with zero failures in
this run and in the [first instrumented run](../2026-11-17-raw-udp-trace/runtime_probe/result.json).
Both used the same DOL. All 16 captured pages were valid in this run. See the
[probe log](runtime_probe/probe.log), [result](runtime_probe/result.json),
[suite](suite.json), [identities](identities.json), and
[page 16](runtime_probe/pages/page-016.png).

The probe initializes the raw network service and two UDP sockets on loopback.
It checks bind, assigned addresses, sendto and polling, connected send/receive,
write/read, exact bytes and buffer guards, socket close, and deinitialization.
These checks exercise selected public raw entry points with live loopback
traffic. They do not establish external networking or physical Wii behavior.

The [first run](../2026-11-17-raw-udp/runtime_probe/probe.log) passed the other
checks but failed the combined recvfrom byte-and-sender comparison. It did not
record which field differed. Two subsequent instrumented runs passed and
recorded matching bytes and sender. The original failure remains unexplained;
`net_recvfrom` is not counted as verified. No production code changed. HBC
was not under test. The runner stops Dolphin after the summary, so `exit_code`
is null. The stderr log can contain partial-send and alignment warnings.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
cargo xtask dolphin-validate --no-build --probe runtime_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --artifact-dir docs/dolphin-runs/2026-11-17-raw-udp-repeat --timeout-seconds 180
```
