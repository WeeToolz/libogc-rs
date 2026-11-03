# Console scroll capture in Dolphin

Dolphin 2609a ran `usbstorage_capacity_probe` with the blank-page check enabled.
The DOL SHA-256 was
`75e869abd1535ba755290ccc564a0a84c05eb5cca80b6a83878024426794ec39`.
The run passed 50 checks and captured five pages. The [first page](page-001.png)
shows startup text after scrolling; the [final page](page-005.png) shows the
summary. See the [probe log](probe.log).

Before the change, the same probe stopped on a green page. Diagnostic builds
counted 12,543 bright framebuffer words before IOS subsystem initialization
and zero after it. With aligned word copies for console scrolling, the count
remained nonzero. The final run removed the diagnostic instrumentation. The
result does not establish physical Wii video behavior.
