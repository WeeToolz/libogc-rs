# Nintendo USB GameCube Adapter

The controller runtime supports the Nintendo Wii U GameCube adapter with USB
identity `057e:0337`. The runtime treats its four controller ports as four
logical GameCube controllers while keeping one physical USB device and one
interrupt endpoint owner.

## Production path

The USB scan selects the adapter only for the exact vendor and product IDs. It
requires an interrupt input endpoint that can carry the 37-byte adapter
report and an interrupt output endpoint. After the interface is active, it
sends the required one-byte initialization report `13` through the output
endpoint. The transfer uses a 32-byte-aligned buffer. A negative IOS result
stops initialization and is returned as a controller error. The runtime does
not send the clone-specific HID `SET_PROTOCOL` workaround.

Input reports must be exactly 37 bytes and begin with report ID `21`. The
remaining 36 bytes contain four nine-byte port records. A port is connected
when the status byte has bit `10` set, or when bits `22` are both set, matching
the wired and WaveBird connection checks in `PADReadGC.c`. Buttons, sticks,
and triggers use the byte offsets in that C implementation. Each port has its
own transition identity, connection gate, metadata, and disconnect release.
Malformed adapter reports reset all four logical ports and preserve release
edges.

The implementation reuses the USB device lifecycle, reset, retirement, and
trace paths. Raw report trace events capture USB device ID, VID, and PID at
dispatch time. The four ports use `ControllerSource::UsbGameCube` with the
physical adapter slot and logical port number. Generic USB HID devices remain
on the descriptor-backed HID decoder.

## Source evidence

The implementation was compared with the original sources in
`/Volumes/SSD/projects/Wii/wiidev/RustWii/C-sources`:

- `Nintendont/kernel/HID.c` identifies `057e:0337` and sends initialization
  report `13`.
- `Nintendont/controllerconfigs/controllers/057E_0337.ini` defines the
  adapter's four-port report layout.
- `loader/source/ppc/PADReadGC/source/PADReadGC.c` reads four nine-byte
  records, applies the connection flags, and maps buttons, axes, and triggers.

Rust also validates the report ID and exact packet length before decoding.
This prevents initialization acknowledgements, truncated packets, and
unexpected padded packets from becoming controller input. The C path does not
perform this explicit validation.

## Verification status

Host regression tests exercise the same decoder, initialization-result
handling, four-port state mapping, transition edges, malformed-report resets,
disconnect releases, and reconnect generation used by production code. They
do not replace a Wii hardware probe. Physical adapter enumeration, IOS
interrupt transfer behavior, and all four ports remain pending hardware
acceptance.
