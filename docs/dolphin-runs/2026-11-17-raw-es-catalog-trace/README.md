# Raw ES catalog in Dolphin

Dolphin 2609a on macOS passed 229 checks and failed one. Ten raw ES entry
points passed checks for initialization and close, title and owned-title
lists, system menu TMD size and content IDs, and boot2 version. The expected
TMD size and IDs were read independently from the source NAND fixture. The
[result](runtime_probe/result.json), [probe log](runtime_probe/probe.log),
[screen](runtime_probe/pages/page-010.png), [suite](suite.json), and
[identities](identities.json) record the run. The source NAND identity was
measured after the run, not from the initial private copy.

The data-directory check failed in both this run and the [first run](../2026-11-17-raw-es-catalog/runtime_probe/probe.log).
Dolphin returned success and the 29 path bytes, but left the next byte at the
sentinel value instead of writing a null terminator. The local Dolphin source
copy writes the path length without a terminator; it is not proved identical
to the installed binary. The C libogc wrapper passes a 30-byte output buffer
without adding a terminator. This run retains the failure. The [final run](../2026-11-17-raw-es-catalog-final/README.md)
checks the observed 29-byte behavior; `get_data_dir` remains unverified. Do not treat the returned bytes as a C string without a
separate termination check. No production code changed.

The probe uses a private Dolphin profile and copied NAND. It does not establish
physical Wii behavior. HBC was not under test. The runner stops Dolphin after
the summary, so its process exit code is null.

Set the MP3 fixture variables described in `docs/dependencies.md`, then run:

```sh
cargo xtask build-dolphin-probes
cargo xtask dolphin-validate --no-build --probe runtime_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --artifact-dir docs/dolphin-runs/2026-11-17-raw-es-catalog-trace --timeout-seconds 180
```
