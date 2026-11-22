# IPC formatter recovery follow-up

The complete `runtime_probe` run passed 873 checks and failed zero checks on
Dolphin 2609a. The base revision is
`8b09197702ec8b3cece5320b91160d925865aec6`. The [source patch](source.patch)
has SHA-256 `2052b2e55945e643c0bd2450e8e2dd1b3e54bf4bedcc2d2b327f177d7407c4b3`.
The DOL SHA-256 is
`39524043095aacdafcb22965194e30f47b3b535ab935e25bc3670d0df56a97b4`.

The runner used `--probe runtime_probe --timeout-seconds 600` and a private
profile. [suite.json](suite.json) records the full arguments and result. The
original run directory is
`docs/dolphin-runs/2026-11-22-ipc-format-recovery-final-Y6YOyA`.

Each async Argv, second Scalar, FormatState, and first staged-buffer allocator
failure is followed by a live formatted ES title-count request. The workspace
cases release and verify the held reservations before that request. The heap
cases use the default IPC heap for recovery because the small test heaps remain
too small for the request. All four recovery checks passed. The controlled
output boundary fixture reports a failed length and offset as `length=offset`.

All 71 formatter PASS labels selected from the prior 869-check archive also
occur in this run. This run does not prove Broadway cache behavior or IOS DMA
coherency. The eight hardware contracts remain blocked and the two DMA output
copyback contracts remain partial.

The probe log, transport bytes, stdout, and stderr were copied without
normalization. The original run directory retains the page captures and the
private profile.
