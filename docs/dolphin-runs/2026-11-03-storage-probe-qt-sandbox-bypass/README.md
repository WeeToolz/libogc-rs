# Initial storage probe run

This is the first Dolphin run of `storage_probe` on 2026-11-03. It is kept as
historical evidence for the failures that led to the media-aware probe update.

- Dolphin: `Dolphin 2609a`
- Result: `probe_fail`; 94 checks passed and 4 failed
- DOL SHA-256: `124eb10c1c17eae30e7b42493881fdd2c960aab1745a74fef3b6311f817b2a71`
- Suite report: [`suite.json`](suite.json)
- Probe log: [`storage_probe/probe.log`](storage_probe/probe.log)

The probe treated a direct launch with no argument block as an argv failure.
It also treated absent USB storage as a failure and attempted to use the
asynchronous card result as if a card were present. Dolphin had no USB device
and no memory-card image in its private profile. The later
[media-aware run](../2026-11-03-storage-probe-media-aware/README.md) checks
those missing-device results and skips operations that need the absent media.
