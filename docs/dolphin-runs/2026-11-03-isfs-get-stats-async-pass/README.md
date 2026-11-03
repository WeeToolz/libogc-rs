# ISFS `get_stats_async` closed-service probe

Dolphin 2609a passed the `runtime_probe` with 63 checks and 0 failures. The
DOL SHA-256 is
`10abd1f0720d39451d519861c3cd35c9209717946a71433866cefc576a04e5b2`.

The direct check calls `ogc-filesystem::get_stats_async` after the probe closes
`/dev/fs`. It checks that the call returns `ISFS_EINVAL`, leaves all seven
`IsfsStats` fields unchanged, and does not call the completion callback before
returning. The probe snapshots callback state at the call boundary so later
async checks cannot change this result.

This checks the closed-service error path only. It does not check successful
ISFS stats retrieval or other successful NAND operations. The probe log and
full run details are in this directory.
