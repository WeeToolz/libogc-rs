# ISFS `get_stats_async` closed-service probe

Dolphin 2609a passed the `runtime_probe` with 63 checks and 0 failures. This
initial run checked the error and output values. The later
[`*-pass` run](../2026-11-03-isfs-get-stats-async-pass/README.md) also checks
that the callback remains untouched at the call boundary.

The added direct check calls `ogc-filesystem::get_stats_async` after the probe
closes `/dev/fs`. It checks that the call returns `ISFS_EINVAL`, leaves all
seven `IsfsStats` fields unchanged, and does not call the completion callback.
The failed call returns before it submits an IOS request.

This checks the closed-service error path only. It does not check successful
ISFS stats retrieval or other successful NAND operations. The full probe log
and run details are in this directory.
