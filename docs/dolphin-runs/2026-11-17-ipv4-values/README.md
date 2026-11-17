# Raw IPv4 value operations in Dolphin

Dolphin 2609a on macOS with OpenGL passed 175 checks with zero failures. All
15 captured pages were valid. See the [probe log](runtime_probe/probe.log),
[result](runtime_probe/result.json), [suite](suite.json), [identities](identities.json),
and [pages 2](runtime_probe/pages/page-002.png) and [3](runtime_probe/pages/page-003.png).

The probe calls production IPv4 packing, four octet getters, TOS and
precedence masks, and `inet_addr`. Fixed words and octets check three address
patterns; fixed values check three mask inputs. The parser checks two valid
addresses, an invalid octet, and null input. The C header defines the address
and mask expectations. Null handling is a Rust safety behavior.

These are target value checks for selected inputs. They do not test packet
transport, other parser formats, or physical Wii behavior. HBC was not under
test. No production code changed. The runner stops Dolphin after the summary,
so `exit_code` is null. The stderr log can contain partial-send and alignment
warnings. All checks and pages completed.

To reproduce, set both MP3 fixture variables as described in
`docs/dependencies.md`, run `cargo xtask build-dolphin-probes`, and then run:

```sh
cargo xtask dolphin-validate --no-build --probe runtime_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --artifact-dir docs/dolphin-runs/2026-11-17-ipv4-values --timeout-seconds 180
```
