# Locked-cache controls in Dolphin

This batch checked `lc_disable`, `lc_enable`, and `lc_flush_queue` in Dolphin
2609a. The bus probe passed 55 checks with no failures. The three new checks
read HID2 after disabling and re-enabling the locked cache, and check that a
flush leaves an initially empty queue empty. The probe restores the saved HID2
value after the cache checks.

At batch completion, the exact API evidence reduced the unverified inventory
from 190 to 187. A later coverage review found that the earlier
`lc_alloc_tags` evidence only checked a zero tag count. Its scope was marked
partial until the [allocation batch](../2026-11-21-ogc-cache-allocations-v3/README.md)
checked non-zero tags. A later review also marked `lc_flush_queue` partial
because the check had no pending work in the queue. After the allocation batch
and all coverage corrections, the current inventory has 189 unverified APIs.
This run checks Dolphin's emulated register behavior. It does not verify
physical cache contents, timing, or queue behavior with pending DMA work.

Build command:

```sh
cargo xtask build-dol ogc-system-probe --bin bus_probe \
  --probe-features dolphin-automation --no-dev \
  --output target/xtask/probes-dolphin/bus_probe.dol
```

Run command:

```sh
python3 scripts/api-probe-batch.py run /tmp/ogc-cache-lock-controls-batch.json \
  docs/dolphin-runs/2026-11-21-ogc-cache-lock-controls \
  --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin \
  --nand-dir '/Users/user/Library/Application Support/Dolphin/Wii'
```

The DOL SHA-256 is
`e934c754d10a2c275ccf043c80305b3c7dc10e37a03e3d992c2695d3d431beb9`.
See [the batch report](batch-report.md), [probe log](bus_probe/probe.log),
[suite metadata](suite.json), and [completeness audit](completeness-audit.log).
