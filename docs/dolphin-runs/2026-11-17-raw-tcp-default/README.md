# Raw TCP loopback in Dolphin

Dolphin 2609a on macOS with OpenGL passed 201 checks and zero failures. The
probe opens stream sockets with protocol 0, enables and reads `SO_REUSEADDR`,
sets and clears nonblocking mode, listens, connects, accepts a loopback peer,
checks received bytes and buffer guards, and checks EOF after write shutdown.
The [log](runtime_probe/probe.log), [result](runtime_probe/result.json),
[suite](suite.json), [identities](identities.json), and
[final screen](runtime_probe/pages/page-017.png) record the run.

Two earlier runs used protocol 6 and failed to open the sockets. The
[diagnostic run](../2026-11-17-raw-tcp-trace/runtime_probe/probe.log) records
`-68` for both calls. The reason is not established. Protocol 6 remains an
open case; the passing run uses protocol 0. The earlier unrelated UDP
recvfrom discrepancy also remains unexplained. No production code changed.
These checks cover local Dolphin traffic, not external networks or physical
Wii hardware. HBC was not under test. The runner stops Dolphin after the
summary, so the process exit code is null. The NAND identity was measured
from the source after the run, not from the initial private copy.

Build with both MP3 fixture variables described in `docs/dependencies.md`,
then run:

```sh
cargo xtask build-dolphin-probes
cargo xtask dolphin-validate --no-build --probe runtime_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --artifact-dir docs/dolphin-runs/2026-11-17-raw-tcp-default --timeout-seconds 180
```
