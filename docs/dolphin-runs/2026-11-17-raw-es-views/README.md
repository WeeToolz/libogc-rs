# Raw ES TMD and ticket views in Dolphin

Dolphin 2609a on macOS passed 235 checks. The probe reads the system menu's
stored TMD and compares its SHA-1 digest to an independently hashed 808-byte
NAND fixture. It checks the output guard, ticket view count and title ID, TMD
view size, title ID, content count, and output guard. See the
[log](runtime_probe/probe.log), [check screen](runtime_probe/pages/page-011.png),
[result](runtime_probe/result.json), [suite](suite.json), and
[identities](identities.json).

The run used a copied NAND in a private Dolphin profile. The source NAND hash
was measured after the run, not from the initial private copy. No production
code changed. The checks cover this fixture and do not establish physical Wii
behavior. HBC was not under test. The runner stops Dolphin after the summary,
so its process exit code is null. The earlier data-directory terminator limit
remains open; see the [catalog report](../2026-11-17-raw-es-catalog-final/README.md).

Set the MP3 fixture variables described in `docs/dependencies.md`, then run:

```sh
cargo xtask build-dolphin-probes
cargo xtask dolphin-validate --no-build --probe runtime_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --artifact-dir docs/dolphin-runs/2026-11-17-raw-es-views --timeout-seconds 180
```
