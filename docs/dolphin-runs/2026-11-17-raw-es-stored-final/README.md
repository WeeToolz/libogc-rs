# ES stored and shared content in Dolphin

Dolphin 2609a passed 239 checks. The probe counted nine stored entries
from the System Menu TMD and 120 shared contents. Before the run, the source
NAND had all nine files referenced by the TMD and 120 shared content files.
The shared list returned the first hash from the source content map and
preserved the guard entry. The test checks only the first returned hash.
See the [check screen](runtime_probe/pages/page-011.png),
[probe log](runtime_probe/probe.log), [result](runtime_probe/result.json),
and [identities](identities.json).

Two earlier runs expected a successful stored ID list. The diagnostic run
returned `-2011` and left every output word unchanged. The local Dolphin
source shows a container verification step before the list. The controlled
test checks the error and the unchanged buffer for this fixture. Successful
listing from a supplied TMD remains unverified. Physical Wii behavior
requires separate evidence. See the
[first run](../2026-11-17-raw-es-stored-contents/runtime_probe/probe.log)
and the [diagnostic run](../2026-11-17-raw-es-stored-trace/runtime_probe/probe.log).

Build with the MP3 fixture variables in `docs/dependencies.md`, then run
the probe with a new artifact directory. The runner uses a private profile
and copied NAND.

```sh
cargo xtask build-dolphin-probes
cargo xtask dolphin-validate --no-build --probe runtime_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --artifact-dir docs/dolphin-runs/2026-11-17-raw-es-stored-final --timeout-seconds 180
```
