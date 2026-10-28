# Bluetooth controller test inventory

The exact-device inventory and separate evidence fields live in
[`config/controller-devices.csv`](../config/controller-devices.csv). Do not copy
controller status values into this document or infer support from source code,
model shape, an emulator, or a host driver.

The operator supplied photos on 2026-09-29. Current physical records cover a
Microsoft Xbox model 1708, PDP REMATCH model 500-202, and left and right
Joy-Con whose rear labels are not in the supplied photos. The CSV keeps the
physical units separate. Planned target categories are also separate from
physical devices. Model and firmware details remain unknown until labels or
captures establish them.

The PDP controller is a third-party Switch controller. Do not identify it as a
Nintendo Pro Controller from its shape or intended console. Its [500-202 quick
start guide](https://cdn.turtlebeach.com/device/qig/rematch-wireless--motion/rematch-wireless--motion_394-0630-v1.pdf)
is product-family documentation. Confirm the physical label and transport
before using its contents as device-specific evidence.

The Xbox firmware version matters. The model label does not identify the
installed firmware. Microsoft announced a firmware update for some Xbox One
controllers with Bluetooth Low Energy improvements in its
[2021 announcement](https://news.xbox.com/en-us/2021/09/08/xbox-controller-firmware-update-rolling-out-to-insiders-starting-today/).
The current Rust host's Classic Bluetooth path and its limits are implementation
facts; they do not establish the transport of the physical model-1708 unit.

## Test order

1. Photograph the rear label of each Joy-Con. Record the complete model and
   suffix. Test each half alone before testing a pair.
2. Test the PDP 500-202. Record the complete label and SKU. Capture discovery,
   raw reports, and command replies before selecting a decoder.
3. Test the Xbox 1708. Record the installed firmware if the controller reports
   it. Determine its actual USB and Bluetooth behavior separately.
4. For every device, keep detection, raw input, mapped actions, reconnect, and
   Wii acceptance as separate observations. A decoder in source does not mean
   the device was detected or accepted.
5. Record discovery, pairing, input mode changes and acknowledgements, button
   press/hold/release, stick movement, disconnect, reconnect, and transport
   close results. Save exact run identity and raw evidence.

The current source and hardware evidence boundary is summarized in
[`controller-action-matrices.md`](controller-action-matrices.md).

## Adapter setup and capture rules

The Bluetooth probe and adapter setup are described in the Rust host source and
its controller evidence. Preserve the failed startup stage, HCI command and
status where applicable, connection mode, service/channel data, report bytes,
input counts, and cleanup result. HIDP output on the interrupt channel and
control-channel SET_REPORT are different operations; see the
[Linux HIDP implementation](https://github.com/torvalds/linux/blob/master/net/bluetooth/hidp/core.c)
for an implementation reference. This source does not establish Wii behavior.

For each physical controller, keep these layers separate:

1. **Identity:** complete label, revision, region, regulatory IDs, and known
   firmware.
2. **Detection:** what the Wii reports at its actual transport boundary.
3. **Raw capture:** descriptors, services, command traffic, acknowledgements,
   and untouched input reports.
4. **Mapped actions:** the observed relationship between controls and logical
   actions.
5. **Reconnect:** behavior after disconnect, power cycle, and renewed pairing.
6. **Wii acceptance:** result from a real Wii using the production input path.

A PC capture or Dolphin test can help explain a device. It does not prove real
Wii acceptance. An earlier controller research report was supplied as a local
attachment and is not included in this repository. See the linked controller
research documents in this directory for available sources and protocol gaps.
