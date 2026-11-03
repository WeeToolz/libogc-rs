# ISFS `get_stats_async` probe: failed callback-state check

This run had 62 passing checks and 1 failure. The probe read shared callback
state after later async checks had reused it. This was a probe-order error;
the result did not show a failure in `get_stats_async`. The next run snapshots
the callback state immediately after the call and passes all 63 checks. See
the [`*-pass` run](../2026-11-03-isfs-get-stats-async-pass/README.md).
