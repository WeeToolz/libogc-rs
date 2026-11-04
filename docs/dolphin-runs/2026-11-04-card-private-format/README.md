# Private virtual CARD format

Dolphin 2609a ran `card_probe` with `--format-private-card` on a private copy
of a 16 MiB card. The [log](probe.log) records 227 passes and zero failures.
The synchronous and raw asynchronous format cycles each created a marker,
formatted the card, remounted it, checked that the marker was gone, and checked
free blocks. The async cycle also checked the callback and completion result.
The [suite report](suite.json) has the DOL hash and runner result.

The source card SHA-256 was
`39fb604967328a744686921b0c0920ed657dbe71c8e7e7eb71d87c1b327fa39a`
after the run, matching its recorded pre-run hash. This does not establish
physical card format or hotplug behavior.
