# Checked initialization

Dolphin checked repeat initialization after successful startup. The test
compares the completed status and stage mask before and after the call.

A separate source review found an open stdio issue. The target
`sys_stdio_report` records a flag, but the C `SYS_STDIO_Report` installs
UART output for stderr and optionally stdout. The Rust standard-output
backend currently writes to the video console. The API remains unverified.
A fix needs a stream-specific output path and a Dolphin check of bytes sent
through each stream.
