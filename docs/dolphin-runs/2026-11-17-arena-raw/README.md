# Raw and C ABI arena operations in Dolphin

Dolphin 2609a on macOS with OpenGL passed 13 checks with zero failures. All
three captured pages were valid. See the [probe log](arena_probe/probe.log),
[result](arena_probe/result.json), [suite](suite.json), [identities](identities.json),
and [page 3](arena_probe/pages/page-003.png).

The probe calls raw MEM1 and MEM2 arena bounds, size, and low-allocation APIs
and the six MEM2 C ABI entry points. It checks temporary low and high bounds,
size arithmetic, aligned allocation, invalid allocation without advancement,
and restoration of both saved bounds. Expected addresses come from the saved
bounds and fixed alignment arithmetic. Reserved memory is not read or written.
The C reference and the Rust allocation safety rules inform the cases.

These are Dolphin results for selected inputs. Concurrent allocation, other
alignment and exhaustion cases, and physical Wii behavior remain open. HBC was
not under test. No production code changed. The runner stops Dolphin after the
summary, so `exit_code` is null. The stderr log can contain partial-send and
alignment warnings. All checks and pages completed.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
cargo xtask dolphin-validate --no-build --probe arena_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --artifact-dir docs/dolphin-runs/2026-11-17-arena-raw --timeout-seconds 180
```
