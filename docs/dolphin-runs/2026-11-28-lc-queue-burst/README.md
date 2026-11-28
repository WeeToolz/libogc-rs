# Locked cache queue burst in Dolphin

Date: 2026-11-28. Dolphin 2609a ran `bus_probe` from source revision
`d46093512502b60f7bfa4c71b38f9925712c6610` with the probe change in the
working tree. The DOL SHA-256 is
`7121b8d20f529fcf9e86219c8a207f43eb3bc4f40c133f3ac57bdc5c8df7992f`.
The runner used a private Dolphin user directory. No extra device fixture was
needed.

The probe submits 15 one-line loads to 15 distinct locked-cache lines, then
reads `lc_queue_length`. Dolphin completed the DMA before that read. The log
records `UNTESTED nonzero locked cache queue depth: DMA completed before
sampling`; this run does not verify a nonzero queue length. After the flush,
all 15 cache lines matched the source data and the queue was empty. The probe
restored the saved cache bytes and HID2 state.

Result: 78 checks passed, 0 failed. The full [probe log](bus_probe/probe.log),
[result report](bus_probe/result.json), and runner report (`suite.json`) record
the run. The private Dolphin user directory was removed after the run.

To repeat the probe build and run:

```sh
cargo xtask build-dol ogc-system-probe --bin bus_probe \
  --probe-features dolphin-automation --no-dev \
  --output target/xtask/probes-dolphin/bus_probe.dol
cargo xtask dolphin-validate --no-build --probe bus_probe \
  --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin \
  --artifact-dir docs/dolphin-runs/2026-11-28-lc-queue-burst
```

This is partial Dolphin evidence. Nonzero queue depth and physical Wii cache
and DMA behavior remain open.
