# TinySMB connected operations in Dolphin

Dolphin 2609a, OpenGL backend, ran `tinysmb_probe` against the local SMB1 fixture on `127.0.0.1:14445`. The fixture used Impacket 0.13.1, share `OGC`, and disposable directory `/Volumes/SSD/WiiDev/tmp/tinysmb-fixture-20261126`. The DOL enabled the compile-time fixture port and used the production TinySMB and lwIP PowerPC paths.

Result: 34 passes, 0 failures. The probe verified network init/deinit; nonblocking socket return values; authenticated connection; file create; known-byte write/read; closed file rejection; archive search; successful search close and local search ID invalidation; file deletion; and closed connection rejection.

The fixture adapts two server limits. It strips libogc's legacy `0x04` ASCII OPEN_ANDX buffer marker because Impacket treats it as part of the path. Impacket has no FIND_CLOSE2 handler, so the fixture checks for a nonzero search ID and sends a success response; the production client must parse success and clear the ID. The fixture server source records both adaptations.

The two corrected production mismatches were confirmed against Dolphin and the local libogc source: PowerPC socket errors use IOS values (`EINPROGRESS=-26`, `EISCONN=-30`, `EALREADY=-7`, `EAGAIN=-6`), and SMB negotiate's dialect byte count is 12 for the 12 bytes sent. The source is `/Volumes/SSD/WiiDev/C-sources/libogc/libtinysmb/smb.c`.

This is Dolphin-only evidence. Physical Wii behavior remains pending. Search invalidation is verified after FIND_CLOSE2; broader server-side behavior and handle invalidation after device/connection close remain open.
