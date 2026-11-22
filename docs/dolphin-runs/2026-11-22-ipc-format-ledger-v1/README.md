# IPC formatter ledger run

This saved run used Dolphin 2609a and `runtime_probe`. The suite reports 830
passed checks and no failed checks. See [batch-report.md](batch-report.md) for
the formatter contracts, required check labels, log lines, captured pages, and
coverage limits. The run data records source revision `0d1b856f80c0cabb3d91a5ceab20affab919b793`
and a dirty worktree.

This run predates commits `941a1269` and `26f6994c`. It is retained as earlier
emulator evidence; it does not prove the final probe mapping validator or the
callback-less exact workspace ledger in those commits. The later focused
PowerPC build compiled `runtime_probe`, but no fresh Dolphin run was available
during that verification because the Dolphin executable was absent.

The workspace ledger requires one allocation and one free for every recorded
`(pointer, size)` pair. The saved run's passing formatter assertions do not
establish physical Wii cache or DMA behavior. Those checks remain open on
hardware.
