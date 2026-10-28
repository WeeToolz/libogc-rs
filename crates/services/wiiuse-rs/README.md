# wiiuse-rs

This crate starts a Rust port of Wiiuse behavior for the Wii target. The first
ported components are Wii Remote, Nunchuk, Classic Controller, and Guitar Hero
3 button decoding and transitions, the ordered initial handshake, memory-read
assembly, accelerometer and Balance Board calibration decoding, Balance Board
load conversion, status-report decoding, and Bluetooth adapter firmware patch
sequence. The production WPAD and Bluetooth host layers use these components
and perform transport requests through libogc-rs.

The upstream C reference is
[`wiiuse/wiiuse` at `a9248aab73cb35c6ff5e4e7ad386c08a737da215`](https://github.com/wiiuse/wiiuse/tree/a9248aab73cb35c6ff5e4e7ad386c08a737da215).
The Wii adapter's startup order also follows the libogc Wiiuse backend in
[`devkitPro/libogc/wiiuse`](https://github.com/devkitPro/libogc/tree/ff53c4a7b7ba5587d82305e710ca65bce1092e1c/wiiuse).
This crate is not a complete port of Wiiuse. Its handshake component covers
the initial status, expansion ID, and accelerometer calibration sequence. The
calibration decoder requires all eight report bytes. It preserves WPAD's
existing nonzero fallback values when calibration data is zero or malformed.
This is a safety difference from the initial C handshake, which stores raw
values and can pass a zero divisor to orientation calculations.
The adapter patch follows `C-sources/libogc/lwbt/bte.c`: it sends the vendor
start command, the 184-byte and 92-byte patch blocks, waits for each successful
HCI completion, resets the adapter, and waits for host reconfiguration before
reporting completion. Bluetooth adapter I/O remains in `ogc-bt-sync`; this
crate does not depend on C code or call a C backend.
The patch bytes are copied from libogc's `lwbt/bte.c` and retain libogc's
permissive license in [`LICENSE-libogc`](LICENSE-libogc). The rest of the
Wiiuse-derived port retains the upstream GPL-3.0-or-later license in
[`LICENSE`](LICENSE).

The port is distributed under GNU GPL version 3 or, at your option, any later
version. Keep the copyright notices and `LICENSE` file with copies of this
crate. The upstream source uses GPL-3.0-or-later. This crate does not claim
libogc's special linking exception.

The button decoder uses the same `WIIMOTE_BUTTON_ALL` mask and big-endian
button word as `wiiuse_pressed_buttons` in `src/events.c`. It reports current,
held, newly pressed, and released button masks. WPAD uses this shared decoder
in both input reports and read reports.

The status decoder reads flags from byte 3 and battery level from byte 6 of
the full HID report. C's `event_status` handler receives a slice after the
report ID, so its payload offsets are bytes 2 and 5. The decoder rejects short
reports and report IDs other than `0x20`.

The Nunchuk button decoder handles the active-low Z and C bits with the
`NUNCHUK_BUTTON_ALL` mask and reports held, pressed, and released transitions.
WPAD calls it from the six-byte Nunchuk expansion report path.

Classic Controller and Guitar Hero 3 button words use Wiiuse's active-low
`0xfeff` mask and big-endian report bytes 4 and 5. Their expansion structs keep
the raw low-word state; WPAD shifts the current expansion buttons into the high
word of its combined button state.

The Balance Board decoder reads three calibration levels in top-right,
bottom-right, top-left, bottom-left order. It decodes the eight report bytes in
that same sensor order and applies Wiiuse's piecewise 0-to-34 load scale. A
calibration block with the upstream repeated `0xff` prefix is retried by WPAD;
a valid block with one prefix is decoded at byte offset 16.

The Guitar Hero 3 analog decoder maps five-bit touch-bar values to Wiiuse's
touch codes and computes the whammy value with the C calibration range. It
preserves values below zero and above one, as the libogc Wiiuse backend does.

The Nunchuk calibration decoder reads the packed accelerometer zero and one-g
values and joystick limits from the 224-byte expansion calibration block. It
uses the libogc defaults for zero fields, preserves C's 16-bit subtraction, and
rejects the repeated `0xff` prefix so WPAD can retry the calibration read.

The Classic Controller calibration decoder handles original and Pro stick
scales, Wii U Pro defaults, the offset-16 calibration prefix, and invalid
stick-range repair. WPAD uses this decoder to populate production stick state.

The Classic Controller report decoder supports the standard six-byte format and
the eleven-byte Wii U Pro format. It tracks button edges, decodes both stick
layouts and trigger values, and returns Wii U Pro charging, wired, and battery
state. WPAD uses it in the production input-report path.

The Motion Plus report decoder requires six payload bytes and reads pitch,
roll, and yaw from the packed axis words. It also returns the extension-data
and speed flags. WPAD uses this decoder in its production expansion-report
path. The axis values remain in the positive range of the C `short` fields.

The complete Guitar Hero 3 report decoder reads six payload bytes and returns
stick positions, raw touch and whammy readings, button transitions, and the
existing analog conversion. WPAD uses this decoder instead of keeping a second
copy of the report masks and field offsets. Short payloads are rejected.
