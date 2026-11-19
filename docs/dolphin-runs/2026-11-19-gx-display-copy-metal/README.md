# GX display-copy commands in Dolphin

The video probe passed 351 checks on Dolphin's Metal backend. The new checks
capture display-list commands for two display-copy configurations and compare
the bytes and line counts with fixed values. They do not execute the captured
copies. The [batch report](batch-report.md) lists the eight APIs and their
coverage limits. Physical Wii behavior remains open.

An [OpenGL run](../2026-11-19-gx-display-copy-fixtures/batch-report.md) passed
both new checks but failed three existing EFB checks. The same failures were
previously observed on OpenGL and passed on Metal; their cause remains open.
For this run, the runner's backend argument was changed temporarily from
OpenGL to Metal and restored after the batch audit. The actual argument is in
the [result](video_probe/result.json). The runner otherwise used its normal
private profile and copied NAND setup. HBC was not under test.
