# ES lifecycle and Linux capture validation

Date: 2026-11-26. Dolphin 2609b, Linux x86_64, Xvfb, private imported Wii NAND.
C reference: `7d4b8b3e7b845516f0cc2a85274879ddd8cbf142`.
Rust base revision: `ae51d7e19a13b0d8dd6c75bf676c65dc347d2463` with the ES
changes and other pre-existing local modifications (the suite records a dirty
working tree). The offline toolchain was Rust 1.101.0-nightly, compiler commit
`28221559263a3976766cf305940e80e30cf9ba8a`, with a local source-lock override.
This is not a run of an unmodified checkout at either revision.

| Probe | Passed | Failed | Valid pages | Runtime |
| --- | ---: | ---: | ---: | ---: |
| general_probe | 94 | 0 | 7 | 14.169 s |
| ios_reload_probe | 21 | 0 | 3 | 8.627 s |

The original DOL SHA-256 hashes are in `suite.json` and each `result.json`.
The retained reports, USB Gecko transport and probe logs are unchanged.
All ten framebuffer pages were captured and validated by the runner.
Absolute paths in the reports refer to the original run. The private NAND,
console keys, runtime profile and raw Dolphin diagnostic logs (which can
contain console identifiers) are not part of this archive.

Run with the private NAND configured through `DOLPHIN_WII_NAND` and the
Dolphin executable through `DOLPHIN_BIN`:

```sh
CARGO_NET_OFFLINE=true xvfb-run -a cargo xtask dolphin-validate \
  --probe general_probe --probe ios_reload_probe \
  --artifact-dir target/es-review-fresh --timeout-seconds 90
```

Before the Linux TCP quick-ACK change, the previously built general probe
(SHA-256 `aa9f31e697c74f74993df527bbc694d43980205c725af7307857800f157fd04d`)
reached its first capture marker but the run exceeded an 18-second outer
timeout (15-second runner timeout). With the same DOL and runner timeout,
the capture change completed 84 checks and all six pages in 11.776 seconds.
No capture validation was disabled. This supports the delayed-ACK diagnosis;
it is not a packet-trace proof of the cause.

The new probes verify DI view size output on IOS error, descriptor-first
content validation, consumption count, IOS reload, identity rejection and
continued ES/ISFS availability after a rejected UID change. Stock Dolphin
rejects PPC DI verification requests; successful identity reopening and
successful privileged UID changes are covered only by host sequencing tests
and source comparison here. The probe does not install/delete titles, import
boot content or change console identity. No physical Wii evidence was added.
