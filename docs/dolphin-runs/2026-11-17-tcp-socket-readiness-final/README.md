# TCP socket readiness and lifecycle in Dolphin

Dolphin passed 260 checks with local TCP traffic. The probe checks both raw
poll entry points, pointer-based accept, socket table exit and init, and
nonblocking receive. The first run expected FIONREAD to succeed and failed.
The Wii C reference implements FIONBIO and rejects other ioctl commands.
The final run checks that FIONREAD fails without writing output, FIONBIO
enables nonblocking receive on an empty stream, and disabling it succeeds.
Physical Wii behavior requires separate evidence.

See the [first run](../2026-11-17-tcp-socket-readiness/runtime_probe/probe.log),
[final log](runtime_probe/probe.log), [check screen](runtime_probe/pages/page-021.png),
[result](runtime_probe/result.json), and [identities](identities.json).
