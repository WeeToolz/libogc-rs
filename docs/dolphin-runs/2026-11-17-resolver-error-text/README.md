# Resolver error text in Dolphin

The batch helper built and ran the runtime probe. Dolphin passed 261
checks. The probe checked code `-3` against the expected resolver text
and code `42` against the fallback text. This check covers string lookup.
DNS resolution and physical Wii behavior require separate evidence.

See the [manifest](batch.json), [run identity](batch-run.json),
[check screen](runtime_probe/pages/page-003.png), [probe log](runtime_probe/probe.log),
and [result](runtime_probe/result.json).
