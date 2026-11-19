# LWP synchronization ABI in Dolphin

Dolphin 2609a on macOS with OpenGL passed 172 checks and zero failures. The
new checks exercise mutex initialization, lock, try-lock and release; a
condition timeout and mutex reacquisition; semaphore permit changes; and a
message queue with ordered sends, two front insertions, empty receive and
close. The probe calls the legacy ABI entry points and the public raw queue
jam function. It checks results, values, order, timeout duration and cleanup.
See the [probe log](lwp_probe/probe.log), [result](lwp_probe/result.json),
[suite](suite.json), [identities](identities.json), and
[summary screen](lwp_probe/pages/page-013.png).

No production code changed. These checks do not establish behavior on a
physical Wii or cover all scheduling and contention cases. HBC was not under
test. The runner stops Dolphin after the summary, so its process exit code is
null. The NAND identity was measured from the source after the run, not from
the initial private copy.

Set the MP3 fixture variables described in `docs/dependencies.md`, then run:

```sh
cargo xtask build-dolphin-probes
cargo xtask dolphin-validate --no-build --probe lwp_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --artifact-dir docs/dolphin-runs/2026-11-17-lwp-sync-abi --timeout-seconds 180
```
