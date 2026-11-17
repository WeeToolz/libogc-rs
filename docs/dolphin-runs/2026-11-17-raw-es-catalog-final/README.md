# Raw ES catalog in Dolphin

Dolphin 2609a on macOS passed 230 checks. The probe checks raw ES initialization
and close, title and owned-title lists, system menu TMD size and content IDs,
and boot2 version. The expected TMD size and IDs were read independently
from the source NAND fixture. See the [probe log](runtime_probe/probe.log),
[result](runtime_probe/result.json), [suite](suite.json),
[identities](identities.json), and [summary screen](runtime_probe/pages/page-019.png).

The [first run](../2026-11-17-raw-es-catalog/runtime_probe/probe.log) and
[diagnostic run](../2026-11-17-raw-es-catalog-trace/README.md) failed a
null-terminator expectation. Dolphin returned the 29 path bytes and left the
next byte unchanged. The local Dolphin source copy writes only the path bytes;
its identity with the installed binary is not proved. The C libogc wrapper
passes a 30-byte output buffer without adding a terminator. The revised check
asserts those 29 bytes and the unchanged trailing bytes explicitly. It does
not establish the physical IOS terminator contract. `get_data_dir` remains
unverified; do not use the output as a C string without a termination check.

The probe uses a private Dolphin profile and copied NAND. The recorded NAND
source identity was measured after the diagnostic run, not from this run's
initial copy. No production code changed. Physical Wii behavior remains open.
HBC was not under test. The runner stops Dolphin after the summary, so its
process exit code is null.

Set the MP3 fixture variables described in `docs/dependencies.md`, then run:

```sh
cargo xtask build-dolphin-probes
cargo xtask dolphin-validate --no-build --probe runtime_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --artifact-dir docs/dolphin-runs/2026-11-17-raw-es-catalog-final --timeout-seconds 180
```
