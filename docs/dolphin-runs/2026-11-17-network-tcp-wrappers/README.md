# TCP network wrappers in Dolphin

Dolphin 2609a on macOS with OpenGL passed 219 checks and zero failures. The
probe initializes the network, connects two local TCP clients, accepts both,
and checks socket names, flags, handle lookup and release. It sends distinct
payloads through the raw-pointer and slice send, receive, write, and read
wrappers. It checks byte counts, bytes, guard space, readiness, and cleanup.
It uses socket protocol 0. See the [log](runtime_probe/probe.log),
[result](runtime_probe/result.json), [suite](suite.json),
[identities](identities.json), and [final screen](runtime_probe/pages/page-018.png).

No production code changed. These results cover local Dolphin traffic, not
external networks or physical Wii hardware. HBC was not under test. The
[protocol 6 socket failure](../2026-11-17-raw-tcp-default/README.md) and the
[earlier UDP recvfrom mismatch](../2026-11-17-raw-udp-repeat/README.md) remain
unexplained. The runner stops Dolphin after the summary, so its process exit
code is null. The NAND identity was measured from the source after the run,
not from the initial private copy.

Set the MP3 fixture variables described in `docs/dependencies.md`, then run:

```sh
cargo xtask build-dolphin-probes
cargo xtask dolphin-validate --no-build --probe runtime_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --artifact-dir docs/dolphin-runs/2026-11-17-network-tcp-wrappers --timeout-seconds 180
```
