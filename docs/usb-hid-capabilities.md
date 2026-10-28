# USB HID Controller Capabilities

The descriptor-backed USB decoder reports controller capabilities from the
union of input fields in all controller report IDs. It filters by the HID
Joystick, Game Pad, and Multi-axis Controller application collections. Fields
from Keyboard and Mouse collections do not contribute.

The decoder reports a stick only when the descriptor declares both axes for
that stick. It reports buttons for controller button usages, a hat switch, or
D-pad usages. It reports triggers for the supported Generic Desktop and
Simulation Controls usages. This summary stays stable when a device sends
axes and buttons in different report IDs. The decoder still updates the
current controller state only from fields present in each received report.

Button usages keep their descriptor identity. A generic HID Button usage has
no fixed GameCube or Wii button meaning. A device profile must bind a usage to
a logical button before the decoder maps it to that button.

## C source comparison

The original source tree contains HID USB setup and device handling, including
`C-sources/Nintendont/kernel/HID.c` and libogc's USB descriptor routines. It
does not contain a general HID report-descriptor parser that derives stable
controller capabilities across report IDs. Nintendont selects controller
behavior through device identity and controller configuration. This
descriptor-wide capability summary is a Rust runtime API behavior; it does
not claim parity with a C capability parser.

Host tests cover split axis and button report IDs, preserved merged input,
explicit button binding, and mixed Keyboard/Mouse collections. These tests do
not establish device-specific or hardware acceptance.

## Public decoding boundary

Use `UsbHidDecoder` with the real interface descriptor and an explicit physical
button profile. The old `map_usb_hid_report` helper is removed because it guessed
field offsets from report bytes. Switch-family reports require
`map_switch_hid_report_calibrated` and validated calibration from the active
connection. `map_bluetooth_hid_report` retains the explicit Xbox format and
rejects uncalibrated Switch reports. These changes do not claim hardware
acceptance for any model or firmware.
