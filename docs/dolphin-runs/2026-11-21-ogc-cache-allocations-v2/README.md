# Expanded locked-cache allocation attempt

This run passed 60 bus checks. It tested two adjacent tags and a two-line
`lc_alloc` request. The batch helper did not record it because
`zero-tag-count` was missing from the helper's replaceable partial-scope list.
The helper now includes that scope and has a regression test. The final run in
the [accepted allocation batch](../2026-11-21-ogc-cache-allocations-v3/README.md)
uses the same DOL contents and passed the helper audit.

This run was not recorded as API evidence because the helper rejected the
partial-evidence replacement.
