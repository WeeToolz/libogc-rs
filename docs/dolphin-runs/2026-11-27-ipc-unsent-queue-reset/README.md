# IPC unsent queue reset probe

Date: 2026-11-27. Dolphin 2609a on macOS. The probe used the production
`IOS_OpenAsync`, `IOS_IoctlvFormatAsync`, and `ogc_ipc::reinitialize` paths.

The fixture disabled interrupts, submitted four requests, and counted one
actual request send before reset. The sent open completed with descriptor 4.
The other two opens and the formatter request were still in the software
queue. Their callbacks followed queue order.
After reset, each unsent callback ran once with `IPC_ENOENT`. The formatter
callback saw its original userdata, and its staged output remained unchanged.
The fixture advanced the queue ring before the test, so the three unsent
requests crossed the ring boundary. The probe passed 33 checks with zero
failures. Its DOL SHA-256 was
`50ddb843d4f83b550d3a4235977821db11c40d838771e9970fbfbc5e2ea73d9b`.

This is a Rust safety difference from pinned C: C clears queue indices without
completing software-queued requests. Rust now returns their owned request and
callback resources without touching IOS buffers. Already sent requests remain
tracked and can still complete after the PPC client reset. This does not test
physical Wii cache or DMA behavior.
