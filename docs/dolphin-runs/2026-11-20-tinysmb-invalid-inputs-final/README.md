# TinySMB invalid inputs

This Dolphin run checks 15 public `tinysmb-sys` APIs with null paths, missing
credentials, or an unknown connection handle. Each call returns before it opens
an SMB session. The run also checks zero or invalid results for search, path,
file-open, directory, mutation, and disk queries.

Dolphin passed 19 checks. The audit reduced the unverified count from 456 to
441. The inventory refresh also removed 21 `pub` functions from the private
`powerpc_backend` module as separate public API entries. Two old evidence rows
matched `network::init` and `network::connect`, not TinySMB calls, so the audit
no longer accepts them.

This run does not test file reads or writes because the unsafe API requires a
live file handle. Successful SMB sessions, file I/O, directory operations, and
server interoperability remain unverified.

See the [batch report](batch-report.md), [suite result](suite.json), and
[completeness audit](completeness-audit.json).
