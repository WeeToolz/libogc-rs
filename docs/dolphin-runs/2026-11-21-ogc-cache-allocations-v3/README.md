# Locked-cache allocation paths in Dolphin

This accepted batch checks four cache allocation APIs in Dolphin 2609a:

- `lc_alloc_tags` zeros two adjacent tags and preserves guard lines.
- `lc_alloc_one_tag` zeros one tag and preserves adjacent lines.
- `lc_alloc` enables locked cache and zeros two requested lines.
- `lc_alloc_no_invalidate` zeros one requested line.

The probe restores the saved HID2 value and all bytes in its aligned test
buffer. Dolphin passed 60 bus checks with no failures. The inventory moved from
188 to 184 unverified APIs. These checks cover the listed allocation paths in
Dolphin. They do not prove physical cache contents, timing, or Wii hardware
behavior.

The first allocation attempt checked one tag. Review expanded the checks to
two adjacent tags and a two-line `lc_alloc` request. The batch helper then
rejected replacement of the earlier `zero-tag-count` partial evidence because
its replaceable-scope list was incomplete. The helper now recognizes that
scope. A regression test checks replacement, and an old assertion now matches
the helper's current error wording. All five helper tests pass. This final run
was recorded by the corrected helper.

Build command:

```sh
cargo xtask build-dol ogc-system-probe --bin bus_probe \
  --probe-features dolphin-automation --no-dev \
  --output target/xtask/probes-dolphin/bus_probe.dol
```

The run used the saved batch manifest. Copy it outside the new artifact
directory before running the helper:

```sh
cp docs/dolphin-runs/2026-11-21-ogc-cache-allocations-v3/batch.json \
  /tmp/ogc-cache-allocations-batch-v2.json
python3 scripts/api-probe-batch.py run /tmp/ogc-cache-allocations-batch-v2.json \
  docs/dolphin-runs/2026-11-21-ogc-cache-allocations-v3 \
  --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin \
  --nand-dir '/Users/user/Library/Application Support/Dolphin/Wii'
```

The DOL SHA-256 is
`2bc77ff10f2b43cc19eaccb9d285cdb415ab7e7e795162d51b5c6e2695cffede`.
See [the batch report](batch-report.md), [probe log](bus_probe/probe.log),
[suite metadata](suite.json), and [completeness audit](completeness-audit.log).
