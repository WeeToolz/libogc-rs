# LWP condition signal and broadcast in Dolphin

Dolphin 2609a on macOS with OpenGL passed 214 checks and zero failures. Two
workers wait on the same condition with a mutex. The first cycle checks that
an ABI signal wakes one worker and an ABI broadcast wakes the other. The
second cycle checks that the raw broadcast wakes both. Both raw and ABI wait
calls return, each worker releases the mutex and joins, and the condition and
mutex are destroyed. See the [log](lwp_probe/probe.log),
[result](lwp_probe/result.json), [suite](suite.json), [identities](identities.json),
and [summary screen](lwp_probe/pages/page-015.png).

No production code changed. These checks do not establish physical Wii
behavior or all contention cases. HBC was not under test. The runner stops
Dolphin after the summary, so its process exit code is null. The NAND source
identity was measured after the run, not from the initial private copy.

Set the MP3 fixture variables described in `docs/dependencies.md`, then run:

```sh
cargo xtask build-dolphin-probes
cargo xtask dolphin-validate --no-build --probe lwp_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --artifact-dir docs/dolphin-runs/2026-11-17-lwp-cond-broadcast --timeout-seconds 180
```
