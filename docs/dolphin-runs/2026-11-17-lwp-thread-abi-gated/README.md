# LWP thread ABI in Dolphin

Dolphin 2609a on macOS with OpenGL passed 186 checks and zero failures. A
semaphore-gated worker reports its thread handle. The probe suspends it, posts
work, checks it stays stopped, resumes it, checks progress, and joins it with
a fixed result. It tests both legacy ABI and raw suspension calls. See the
[log](lwp_probe/probe.log), [result](lwp_probe/result.json), [suite](suite.json),
[identities](identities.json), and [summary](lwp_probe/pages/page-013.png).

The [earlier run](../2026-11-17-lwp-thread-abi/lwp_probe/probe.log) timed out
after thread creation with a continuously yielding worker. Its cause is not
established. The passing probe gates the worker on a semaphore. Yield is not
counted as verified because this probe does not show a context switch caused
by yield. No production code changed. Results do not establish physical Wii
behavior. HBC was not under test. The runner stops Dolphin after the summary,
so its process exit code is null. The NAND source identity was measured after
the run, not from the initial private copy.

Set the MP3 fixture variables described in `docs/dependencies.md`, then run:

```sh
cargo xtask build-dolphin-probes
cargo xtask dolphin-validate --no-build --probe lwp_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --artifact-dir docs/dolphin-runs/2026-11-17-lwp-thread-abi-gated --timeout-seconds 180
```
