# IPv4 and descriptor-set helpers in Dolphin

Dolphin passed 249 checks. This batch adds direct evidence for 12
`ogc-network` APIs. The probe compares IPv4 packing, octet extraction,
type-of-service masks, and address parsing with fixed values. It also checks
all 16 descriptor bits and clears bits on both sides of a byte boundary.
The descriptor checks use in-memory sets. Socket readiness and physical Wii
behavior require separate evidence.

See the [probe log](runtime_probe/probe.log), [check screen](runtime_probe/pages/page-003.png),
[result](runtime_probe/result.json), and [identities](identities.json).

Build with the MP3 fixture variables in `docs/dependencies.md`. Run the
probe with a new artifact directory for each run.

```sh
cargo xtask build-dolphin-probes
cargo xtask dolphin-validate --no-build --probe runtime_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --artifact-dir docs/dolphin-runs/2026-11-17-network-ipv4-fdset --timeout-seconds 180
```
