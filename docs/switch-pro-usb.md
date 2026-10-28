# Nintendo Switch Pro USB input

The controller manager recognizes the exact Nintendo USB identity `057e:2009`.
It requires one HID interface with interrupt IN and OUT endpoints that each
accept a 64-byte transfer. Other USB HID devices continue through descriptor
based decoding. This support does not claim support for Joy-Con USB,
charging-grip USB, Bluetooth LE, HOGP, or Switch 2.

The setup sequence uses asynchronous interrupt transfers owned by the USB
controller bus. It sends handshake `80 02`, baud-rate `80 03`, a second
handshake `80 02`, and no-timeout `80 04`. Each of the first three commands
waits for its matching `81 <command>` response. The shared Switch setup engine
then sets report mode and reads user stick calibration. It reads factory
calibration only when the user calibration parser reports that its magic is
absent. Present but invalid user calibration fails setup. SPI ACKs must match
the requested address and length. Standard report padding is accepted after
the requested SPI bytes, but short data and mismatched echoes fail setup.

The controller becomes ready only after both Pro Controller sticks have
validated calibration. The first decoded input still passes through the
existing connection gate. A failed transfer, malformed response, NACK, or
elapsed setup deadline retires the device and records a failure. A deadline
does not cancel an IOS request; the request buffer and descriptor remain owned
until the callback completes and the bus drains it.

Host tests exercise the same setup engine used by Bluetooth and USB. They use
fixed ACK and calibration fixtures, including padded 49-byte Bluetooth and
64-byte USB SPI replies. They cannot confirm the USB adapter, controller
firmware revision, endpoint behavior, or real hardware timing. Hardware
acceptance remains open until a real `057e:2009` Pro Controller is tested.

Late setup acknowledgements do not change held controls after readiness.
Recognized USB response commands require their ID and command bytes; fields
with no defined contract are not required to contain zero. A short known reply
is malformed. Report `0x3F` and unknown input IDs are unsupported, rather than
decoded from a guessed layout. Host tests distinguish these results.

The implementation follows Linux `hid-nintendo.c` at commit
[`24a19b953e2a`](https://github.com/torvalds/linux/blob/24a19b953e2a3d0ea5a8c4951a0ab8f32720c907/drivers/hid/hid-nintendo.c)
for the two-byte USB commands and shared Switch subcommands/calibration flow.
The reverse-engineering notes are pinned to
[`b354f21ae81f`](https://github.com/dekuNukem/Nintendo_Switch_Reverse_Engineering/blob/b354f21ae81f7b0d1d060b2b61e66ad0d9bc1756/USB-HID-Notes.md).
The Rust USB transport is based on the local C reference in
`C-sources/libogc/libogc/usb.c`: `__usb_interrupt_bulk_message` (line 478)
passes the raw IOS result to the callback (`__usbv0_messageCB`, line 250),
and the public interrupt wrappers are at lines 1218–1230. Initialization
reports are delivered as raw reports only if an application observer is
installed; no separate setup trace is promised.
