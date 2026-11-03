# ISFS asynchronous format with a bounded wait

Dolphin 2609a ran `runtime_probe` with a private copy of the Wii NAND. The DOL
SHA-256 was `aeef6aae51d3fcd57f8c349863a034c466386aa20341bed47b53aca253babf6a`.
The probe passed 88 checks with no failures. Async format returned submission
status 0 and callback status 0. After the callback and service restart, the
marker file and directory were absent. Cleanup passed. See the [probe log](probe.log).

The [earlier run](../2026-11-03-isfs-format-async-dolphin/README.md) did not
observe the callback within 3,000 one-tick sleeps. This run uses up to 1,000
ten-millisecond sleeps. Scheduling overhead means neither loop is an exact
elapsed-time measurement. The result does not establish physical Wii behavior.
