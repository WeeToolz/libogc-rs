# Dolphin blank page audit

An audit of tracked PNG files found 137 identical blank captures in 98 run
directories. Each listed file has SHA-256
`891c49902dfe57bcb46ad30b66147dbcf6b2eaa4c830309704df7c78ad66bb4d`.
The image contains a green rectangle and a black border, with no readable
probe text. The [file list](blank-green-pages.txt) identifies the affected
captures. These images do not support a visual claim about probe behavior.

The historical images and reports remain available to preserve the original
run record. Their `pages_valid` field meant that a complete PNG was written;
it did not establish that the page contained readable evidence. Review the
other captures and probe logs separately before relying on a run. A probe log
can support its recorded checks; this audit does not establish visual output.

The Dolphin runner now rejects captured pages made entirely of black and green
pixels. A diagnostic USB probe run stopped with a page capture error on its
first page. A later console scroll correction produced readable pages in the
[Dolphin follow-up](../../../dolphin-runs/2026-11-03-console-scroll/README.md).
Do not treat the failed diagnostic run as a probe pass. This audit detects exact copies of the known
image; other blank or misleading images may remain.
