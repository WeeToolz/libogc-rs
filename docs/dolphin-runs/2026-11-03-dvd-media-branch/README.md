# DVD probe media branches

`dvd_probe` now runs the direct DI no-disc checks only when the drive reports
no disc. This keeps media-presence results from being judged against no-disc
expectations.

The [no-disc log](no-disc-probe.log) passed 178 checks with no failures. The
[synthetic-disc log](media-probe.log) passed its disc ID and known-data reads.
It skipped the no-disc DI checks and reported one failure: DVD mount returned
`-1`. The generated sparse image still does not verify a valid mounted disc,
filesystem, or partition. The read checks remain useful partial evidence only.
