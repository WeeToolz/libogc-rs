# Nintendo Switch Controllers Bluetooth HID Protocol

Nintendo Switch Joy-Cons, Pro Controllers, and Switch 2 Joy-Cons present custom Bluetooth HID behavior. Joy-Con 1 and Switch Pro Controller use standard L2CAP HID after pairing: PSM `0x11` for control and PSM `0x13` for interrupt. They also require special initialization subcommands and report formats.

This note summarizes reverse-engineering sources including dekuNukem, Linux `hid-nintendo`, SDL, joycond, and community reports.

## Pairing Flow And Link Keys

- Sync button / Bluetooth pairing: Joy-Cons and Switch Pro Controllers enter pairing mode by holding the tiny SYNC button. They appear as `Joy-Con (L)`, `Joy-Con (R)`, or `Pro Controller`. No PIN is normally required; if prompted, common defaults like `0000` or `1234` may work.
- Manual pairing subcommands: Nintendo controllers support a manual host pair flow through subcommand `0x01`. The host writes its BDADDR and retrieves the controller link key. Sequence: send `0x01 01 [BDADDR]`, wait for ACK, send `0x01 02`, wait for ACK, then send `0x01 03` and read the link key.
- Switch 2 Joy-Cons: Joy-Con 2 uses Bluetooth Low Energy, not classic HID. It uses a proprietary BLE HID-over-GATT protocol with custom services and packed 12-bit reports. Wii Bluetooth cannot support BLE, so Joy-Con 2 is not expected to work on Wii without different hardware and a BLE stack.

## L2CAP / HID Connection Sequence

After pairing, the host queries SDP and opens two L2CAP channels:

- HID control: PSM `0x11`
- HID interrupt: PSM `0x13`

On connect, the controller starts sending input reports at its default rate. The host should then send initialization subcommands. The Switch sends subcommand `0x08 00` after each connection to disable low-power mode and enable fast connect.

## Initialization Commands

Bluetooth output uses HID output report ID `0x01`, which includes 8 bytes of rumble data plus one subcommand.

Important subcommands:

- `0x08 00`: disable low-power mode.
- `0x03 30`: set report mode to standard full input report mode.
- `0x40 01`: enable IMU.
- `0x48 01`: enable rumble.
- `0x30 [mask]`: set player LEDs.

Output report `0x01` format:

| Byte | Field | Description |
| --- | --- | --- |
| 0 | `0x01` | Output report ID for subcommand plus rumble |
| 1 | Packet number | `0x00` to `0x0F`, increment each packet |
| 2-9 | Rumble data | 8 bytes, can be neutral |
| 10 | Subcommand ID | Example: `0x03`, `0x40`, `0x48`, `0x30` |
| 11+ | Subcommand data | Arguments for the subcommand |

The controller replies on the interrupt channel with report ID `0x21`.

## Player LEDs

Subcommand `0x30` takes one byte:

- Low nibble bits `0-3`: solid LEDs 1-4.
- High nibble bits `4-7`: flashing LEDs 1-4.

Example: `0x05` turns LEDs 1 and 3 on.

If the same LED is set in both nibbles, solid overrides flashing.

## Input Report Formats

### Report ID 0x30: Joy-Con Standard Full Report

The standard full report is 50 bytes.

| Offset | Field |
| --- | --- |
| 0 | Report ID `0x30` |
| 1 | Rolling timer |
| 2 | Battery and connection flags |
| 3-5 | Button status |
| 6-11 | Analog stick values, 12-bit little-endian |
| 12-48 | IMU samples |

Button bits:

| Byte/Bit | 0x01 | 0x02 | 0x04 | 0x08 | 0x10 | 0x20 | 0x40 | 0x80 |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Byte 3, Right | Y | X | B | A | SR | SL | R | ZR |
| Byte 4, Shared | - | - | - | - | R-stick | L-stick | Home | Capture |
| Byte 5, Left | Down | Up | Right | Left | SR | SL | L | ZL |

IMU data in mode `0x30` carries three consecutive 6-axis samples.

### Report ID 0x21: Subcommand ACK / Reply

Byte 13 has MSB set for ACK and clear for NACK. Byte 14 echoes the subcommand ID. Reply data starts at byte 15.

### Report ID 0x3F: Simple button event report

The [dekuNukem Bluetooth HID notes](https://github.com/dekuNukem/Nintendo_Switch_Reverse_Engineering/blob/master/bluetooth_hid_notes.md#input-0x3f)
describe report ID at byte 0, button bits at bytes 1–2, a hat at byte 3,
and four little-endian 16-bit stick fields at bytes 4–11 for the Pro
Controller. The same stick bytes are filler for Joy-Con. This layout differs
from the standard report: it has no documented timer or battery field in those
bytes. The old statement that this was a 15-byte standard-style report was
incorrect. These notes are reverse-engineering evidence, not a manufacturer
specification or evidence for every hardware revision.

The production path requests standard report mode `0x30`. It does not decode
`0x3F` as a standard report. A dedicated simple-mode decoder and exact device
captures remain open work. The [Linux Nintendo HID driver](https://github.com/torvalds/linux/blob/master/drivers/hid/hid-nintendo.c)
defines the report ID, but that definition alone does not validate a decoder.

### Report IDs 0x31-0x33

- `0x31`: IMU plus NFC/IR report.
- `0x32` and `0x33`: unused or unknown.

## Output Report Formats

- `0x01`: rumble plus subcommand.
- `0x10`: rumble only, useful as keepalive.
- `0x11`: MCU data.
- `0x12` and `0x13`: MCU/IR formats.
- `0x80`: reserved for USB-HID mode, not Bluetooth.

Every `0x01` and `0x10` packet includes 8 rumble bytes. Packet counter byte 1 increments mod 16.

## Joy-Con L/R, Combined Mode, And Pro Controller

- Joy-Con L and R use the same protocol with different button interpretation.
- Joy-Con L+R combined mode is host-side software. Each Joy-Con remains a separate HID device and report stream.
- Switch Pro Controller uses the same subcommand and rumble protocol, with different report interpretation.
- Switch 2 Joy-Con uses BLE and a new protocol. It is out of scope for Wii Bluetooth.

## Keepalive And ACK Behavior

The host should send periodic output packets, often neutral rumble-only report `0x10`, to prevent disconnection. A practical interval is about 100-200 ms.

For subcommands, wait for report `0x21`. Drivers commonly wait up to about 1 second and retry a few times before treating the command as failed.

## Timing And Rates

- Joy-Con standard mode `0x30`: about 15 ms per report, roughly 66 Hz.
- Pro Controller over Bluetooth: about 8-11 ms per report.
- Two Joy-Cons paired as one logical controller still send independent reports.

## Wii Implementation Constraints

- Wii must support HID L2CAP control and interrupt channels.
- Output reports must support at least the `0x01` format size.
- Link key/encryption handling may block Joy-Con input if pairing is incomplete.
- Wii Bluetooth hardware does not support BLE, so Switch 2 Joy-Con and likely Pro 2 are not feasible through built-in Bluetooth.
- Joy-Con L+R pairing must be implemented in software in the controller assignment layer.

## Minimal Bring-Up Sequence

1. Pair the Joy-Con with SYNC and open HID control plus interrupt L2CAP channels.
2. Send `0x08 00` to disable low-power mode.
3. Send `0x03 30` to enable full input reports.
4. Read interrupt reports and confirm report ID `0x30`.
5. Send neutral `0x10` rumble-only keepalive about every 100 ms.

This proves basic input without rumble, IMU processing, or LED setup.

## Current libogc-rs Hooks

- `ogc-system-probe::controller` has reusable builders for Switch subcommand packets, rumble keepalive packets, player LED packets, report `0x30` parsing, report `0x21` ACK parsing, and Bluetooth HIDP output wrapping.
- `ogc-system-probe::controller` accepts either stripped HID report bytes or raw Bluetooth HIDP input payloads such as `a1 30 ...`.
- `ogc-bt-sync` has L2CAP HID readiness tracking for control PSM `0x11` and interrupt PSM `0x13`, plus a config-request builder for the recommended HID MTU of 64 bytes.
- `bt_sync_menu.dol` shows the prepared init/keepalive/HIDP packet bytes on screen.
- `bt_sync_menu.dol` stores the HCI connection handle after pairing and shows it in the BT slot line.
- `bt_sync_menu.dol` has an opt-in `Y` action that sends L2CAP connection requests for HID control and interrupt channels. It does not yet wait for or process ACL responses in the menu loop.
- ACK/report debug counters are tracked per Bluetooth HID slot once report bytes are fed into `ControllerManager::set_bluetooth_report`.

Next hardware checkpoint:

1. Press `A` to open/scan.
2. Press `B` on a selected Joy-Con or Pro Controller and confirm a nonzero `h=0x....` handle appears.
3. Press `Y` and confirm status becomes `HID`.
4. Next code step after that is to poll ACL responses, feed them into `HidL2capState`, configure both HID channels with the libogc-compatible MTU 673, then send the `0x08 00` and `0x03 30` init packets on the control channel.

Latest real Wii observations:

- `bt-sync-menu.dol` reaches the dashboard with visible green/corrupted console output on the tested TV path.
- Pressing `A` in `bt-sync-menu.dol` caused the screen to go black with green tearing rather than progressing to a usable scan UI.
- `input_probe.dol` showed similar lower-screen green tearing, so the current issue is not isolated to one Bluetooth menu path.
- Because of that display corruption, the next useful hardware step is to stabilize the probe rendering path before using `bt-sync-menu.dol` as the primary Joy-Con pairing diagnostic.

## References

- dekuNukem `Nintendo_Switch_Reverse_Engineering` notes.
- Linux `hid-nintendo` driver source.
- joycond README and pairing behavior.
- SDL Nintendo controller support.
- Community reports on Joy-Con 2 BLE protocol and Switch 2 support.
