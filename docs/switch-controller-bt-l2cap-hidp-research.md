# Switch Controller Bluetooth L2CAP/HIDP Research

## Overview

Bluetooth communication with Switch controllers follows standard HID-over-L2CAP patterns with Nintendo-specific subcommands. In practice, the host, such as Wii, performs normal Bluetooth SSP pairing while the Joy-Con or Pro Controller is in discoverable mode from the SYNC button. No user PIN is normally required. If a PIN is needed by legacy Bluetooth, `0000` will typically work.

After pairing, the host opens two L2CAP HID channels:

- Control: PSM `0x0011`
- Interrupt: PSM `0x0013`

In general the host initiates both L2CAP connections in sequence, often control first and then interrupt, then sends HID `SET_REPORT` output packets on the control channel. The Joy-Con or Pro Controller sends input reports on the interrupt channel.

Both channels must be opened and configured with a sufficiently large MTU, for example at least 64 bytes, and normal L2CAP parameters before full HID operation works. Otherwise large input reports such as mode `0x30` may be truncated.

## Pairing And Link Keys

- Pairing mode: Put a Joy-Con or Pro Controller into discoverable mode by holding SYNC until LEDs flash. The host then initiates Bluetooth pairing. These controllers support Bluetooth Secure Simple Pairing. If only legacy PIN is available, try `0000`.
- Link keys and encryption: After pairing, the host and controller exchange link keys. The Joy-Con firmware can store pairing info in flash through a special subcommand `0x01`, but over-the-air pairing normally handles this automatically. In general, once paired the link is encrypted and authenticated by default, as typical for Bluetooth HID.
- Link key requests: A classic Bluetooth host stack should supply a previously stored key on Link Key Request, or negative-reply if none exists. Initial legacy pairing may use `0000`.
- Wii compatibility: Wii IOS/libogc does not natively know these controllers. Custom code must treat Joy-Cons and Pro Controllers like HID devices and handle link keys.

## L2CAP/HID Channel Setup

- Host should open HID control PSM `0x11` first, then HID interrupt PSM `0x13`.
- On each channel the host sends an L2CAP Config Request.
- Use MTU at least 64 bytes. Reports around 50 bytes need more than sample-code defaults such as MTU 48.
- Default or infinite flush timeout is acceptable. No special QoS is expected.
- There is no special CID requirement. Each L2CAP side picks normal local/remote CIDs.
- Both channels should be open for full operation.
- Output reports use control channel.
- Input reports use interrupt channel.

Failure by channel:

- Only control open: output commands may work, but no async input reports arrive.
- Only interrupt open: input may appear in some states, but commands cannot be sent reliably.

## HIDP Packet Format

Joy-Cons use standard HIDP Data PDUs.

- Host to controller output data: HIDP `0xA2`.
- Controller to host input data: HIDP `0xA1`.

The HID report ID is the first byte after the HIDP header.

### Output Report 0x01

Report `0x01` is rumble plus subcommand:

```text
Byte 0:  0x01
Byte 1:  Packet counter, low 4 bits
Bytes 2-9: RumbleData[8]
Byte 10: Subcommand ID
Byte 11+: Subcommand data
```

Example enable vibration with no rumble:

```text
01 00 00 00 00 00 00 00 00 00 48 01
```

When sent over Bluetooth, this is wrapped in HIDP output data:

```text
a2 01 00 00 00 00 00 00 00 00 00 48 01
```

No normal HIDP handshake exchange is required for subcommands. The controller ACK/NACK arrives inside an input report.

### Output Report 0x10

Report `0x10` is rumble-only. It has the same packet counter and 8-byte rumble section, without a subcommand.

### Input Reports

Input arrives on the interrupt L2CAP channel with HIDP header `0xA1`, then the report bytes.

Known report IDs:

- `0x21`: subcommand reply.
- `0x30`: full input report with buttons, sticks, and IMU.
- `0x31`: NFC/IR mode.
- `0x32`, `0x33`: unknown or unused, apparently standard-like data.
- `0x3F`: simple button-only report.

## Full Report 0x30

Report `0x30` in standard full mode is roughly:

```text
Byte 0: Report ID 0x30
Byte 1: Timer
Byte 2: Battery/connection info
Bytes 3-5: Button bits
Bytes 6-8: Left stick, 12-bit X/Y little-endian
Bytes 9-11: Right stick, 12-bit X/Y little-endian
Byte 12: Rumble/vibrator status
Bytes 13-48: IMU samples in full input reports
```

For subcommand reply report `0x21`:

```text
Byte 13: ACK/NACK and data type. MSB set means ACK.
Byte 14: Echoed subcommand ID
Bytes 15+: Reply data
```

All multi-byte numeric fields are little-endian.

## Controller Initialization

Once both L2CAP channels are up, configure the controller through subcommands.

Minimal sequence:

1. Optionally send low-power mode `0x08 00`. The research claims the controller itself sends/handles this immediately after connection, so host-side need is uncertain.
2. Send set input report mode `0x03 30`.
3. Optionally send enable IMU `0x40 01`.
4. Optionally send set player LEDs `0x30 [mask]`.
5. Optionally send enable vibration `0x48 01`.
6. Optionally read SPI calibration with subcommand `0x10`.

ACK timing:

- Wait for ACK in an input report after each subcommand.
- Use timeout around 500 ms to 1 second.
- Retry a few times on missing ACK/NACK.
- Spacing subcommands 100-200 ms apart is conservative.

Keepalive:

- Full mode should produce continuous input.
- If idle too long, the controller may sleep.
- Some hosts send harmless periodic output or subcommands as keepalive.

## Input Report Delivery

- Full reports begin after mode `0x03 30`.
- Inputs arrive on interrupt channel PSM `0x13`.
- Each incoming L2CAP packet has HIDP header `0xA1`, then report ID.
- Example framing: `a1 30 ...`.
- Left and right Joy-Con reports remain independent. Combined L+R mode is host-side software.

## Joy-Con, Pro Controller, And Switch 2 Notes

- Joy-Con L and R use the same protocol but different button interpretation.
- Joy-Con L+R pairing is host-side merging. The controllers remain two separate Bluetooth devices.
- Switch Pro Controller uses the same HID protocol and subcommands. Report `0x30` is similar, and rumble/LED subcommands are shared.
- This research claims Switch 2 Joy-Cons appear to use an almost identical HID protocol. This conflicts with earlier research that said Switch 2 Joy-Cons use BLE HID-over-GATT and are not viable through Wii internal Bluetooth. Treat Switch 2 support as unproven until primary sources confirm the transport.

## Example Packets

Set full mode:

```text
01 00 00 00 00 00 00 00 00 00 03 30
```

Enable vibration:

```text
01 01 00 00 00 00 00 00 00 00 48 01
```

Set player LED 1:

```text
01 02 00 00 00 00 00 00 00 00 30 01
```

Enable IMU:

```text
01 03 00 00 00 00 00 00 00 00 40 01
```

Sample full input:

```text
30 10 82 41 00 82 02 40 78 00 a0 6c 70 80 ...
```

Values are illustrative.

## Wii Caveats

- Wii IOS Bluetooth stack was designed around Wiimotes.
- Custom HCI/L2CAP/HIDP handling is needed.
- Need two L2CAP channels and HIDP framing.
- Wii BT hardware is classic Bluetooth 2.0-era, but original Joy-Cons and Pro Controller should be backward-compatible.

## Minimal Diagnostic Sequence

1. Pair with SYNC.
2. Open HID control PSM `0x11`.
3. Open HID interrupt PSM `0x13`.
4. Configure both L2CAP channels with MTU at least 64.
5. Send report `0x01` subcommand `0x03 30` on control channel as HIDP `0xA2`.
6. Listen for report `0x21` ACK and report `0x30` input on interrupt channel as HIDP `0xA1`.

## Sources Cited By Research

- Linux `hid-nintendo` driver.
- dekuNukem Nintendo Switch reverse-engineering notes.
- Wireshark captures of official controllers.
