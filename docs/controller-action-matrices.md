# Controller action matrices

This document records behavior visible in source. It does not say that a physical
controller is accepted by the Wii. It separates application actions from
controller detection, raw reports, mapped input, reconnect, and Wii acceptance.
The exact-device evidence baseline is [`config/controller-devices.csv`](../config/controller-devices.csv).

## Evidence rules

- **Rust source** means the action is present in the active Rust files listed in
the source column. It does not prove runtime input from a device.
- **C source** means a matching source path exists and was read. It does not
prove parity by itself.
- **C app source absent** means this checkout has no C source for that example.
Do not invent C controls from a title, binary, manual, or Rust behavior.
- **Runtime** means a real controller was detected and the action was observed
on a Wii. No such claim is made by these tables.
- A press is a newly-down event. A hold remains active across frames. A release
marks the transition to up. A repeat is an application timing rule, not a
controller protocol guarantee.

## Example DOL action matrix

The seven in-scope example DOLs use source-specific actions below. The shared
adapter in `examples/controller_input.rs` combines connected controller button
masks and exposes press, held, release, timestamp, and source identity. Most
default builds initialize GameCube PAD and USB HID. The default
`autoplay_2048` build initializes GameCube PAD only. The optional
`wii-controllers` feature adds WPAD and Classic Bluetooth through the shared
production manager. application retains its existing controller initialization and
player profiles. See the listed source files for exceptions and timing.

| Example and screen/state | Input action | Event/timing and combinations | Rust source | C reference |
| --- | --- | --- | --- | --- |
| `autoplay_2048`, title/menu/options/game/score screens | D-pad or left stick chooses direction; A and Start confirm/open; B backs out; Start pauses; left/right adjusts audio levels in Options | D-pad direction can use down or held state; a newly-held edge becomes a press. Direction repeat uses `DirectionRepeat` state. The source applies screen-specific branches and one-shot button edges. PAD-only initialization by default; the `wii-controllers` build enables the shared Wii input path. | `examples/autoplay_2048/autoplay_2048.rs` (`direction`, `run_autoplay_2048`), `examples/controller_input.rs` | C app source absent. |
| `sudoku`, puzzle | D-pad moves cell cursor; A advances the current editable value; B clears it | Cursor and digit changes use down edges, so a held direction does not repeat in this function. Movement is processed before editing. If the selected cell is fixed, A/B do nothing. Left/right/up/down use an `if/else if` chain, so only the first matching direction in that order moves the cursor in one call. A/B also use an `if/else if` chain. | `examples/sudoku/sudoku.rs` (`manual_input`) | C app source absent. |
| `snake-gc`, title/mode and human game | A or Start enters human mode; D-pad changes direction | Title start accepts held or down state. During play, a newly-down D-pad direction takes priority; otherwise held direction is used. The queue rejects the direction opposite the current direction. Other simultaneous direction bits resolve by the source's `if/else if` order. | `examples/snake-gc/snake-gc.rs` (`pressed_start`, `update_human_direction`, `queue_direction`) | C app source absent. |

### Shared example input boundary

`examples/controller_input.rs` projects controller snapshots into press, held,
release, axis, connection, timestamp, and identity fields. The global-manager
adapter in `examples/controller_global_input.rs` scans and enables USB HID;
the `wii-controllers` feature also enables Classic Bluetooth. The PAD-only
2048 and application defaults keep their original transport setup. Source
attribution is present only when one known identity supplies the active input;
merged or unknown contributors remain unattributed. This is a Rust input
source fact. It is not evidence that all HID, Bluetooth, or named controllers
work. The shared manager and its per-device decoders must be assessed
separately.

The optional `action-trace` feature records production raw/decoded events and
app actions after the app applies them. The checked capture has a fixed capacity;
loss is reported as a failure. These records show software behavior for a run,
not acceptance of an unidentified controller.

application uses canonical stick axes from -32767 through 32767. Its Switch-family
threshold is 9599, which preserves the earlier 600-of-2048 threshold ratio.
Other controller kinds retain 8192. Values equal to either threshold stay
neutral. The projection uses the checked shared controller helper. Existing
application presets and player assignments are unchanged.

### Joy-Con half profiles in example apps

With `wii-controllers`, the six apps below use the shared half-controller
projector. A left Joy-Con has no right stick; its D-pad remains the
app's normal direction source. A right Joy-Con keeps its normalized A/B/X/Y,
Plus/Start, and Home buttons, and its right stick supplies neutral-gated
direction press, hold, and release edges at threshold 8192. application uses its
own configurable button bindings and right-stick direction path.

| Rust app | Left SR | Left SL | Left Minus/Select | Source |
| --- | --- | --- | --- | --- |
| 2048 | A | B | Start | `examples/autoplay_2048/autoplay_2048.rs` |
| Snake | A | Unmapped | Unmapped | `examples/snake-gc/snake-gc.rs` |
| Sudoku | A | B | Unmapped | `examples/sudoku/sudoku.rs` |

These are Rust app aliases. They are not claims that an unidentified USB or
Bluetooth device has those physical labels or that the Wii accepts it.


application direction tracking now assigns a distinct source key to each physical
source, each USB adapter port, and each Bluetooth pair. The old pair encoding
used four slots and could collide when the host used its six Bluetooth slots.
Host regressions compile the same direction module as the app and check source
keys, stick thresholds, disconnect, and replacement behavior. The current
application DOL builds. Runtime and C parity evidence remain pending.

## HBC-RS logical controller map

HBC-RS maps GameCube PAD and Wii Remote button masks into logical actions. It
also maps normalized USB and Classic Bluetooth controllers through the same
production manager. The original PAD/WPAD mappings remain as listed below. The
Rust path also tracks presses, holds, and releases, selects a Wii Remote with a
valid IR pointer as pointer owner, and reads PAD channel zero. PAD channel-zero
buttons remain included. When a pointer owner exists, that Remote supplies the
Remote button events. With no owner, connected Wii Remote button events are
combined. The input scan and mapping are in `RustPorts/hbc-rs/src/platform/input.rs`.

| Logical action | GameCube PAD source mask | Wii Remote source mask | Rust event/use |
| --- | --- | --- | --- |
| Up/down/left/right | D-pad UP/DOWN/LEFT/RIGHT | D-pad UP/DOWN/LEFT/RIGHT | Press, hold, and release are carried in `InputFrame`. |
| Confirm | A | A | Press/hold drives confirm. For pointer use, A presses or holds the IR pointer only when an IR owner exists. |
| Back | B | B | Press, hold, and release. |
| Open options | X | 1 | Press, hold, and release. |
| Home | Start | Home | Press, hold, and release; screen behavior depends on current HBC screen. |
| Toggle browser layout | Y | 2 | Press, hold, and release. |
| Previous page | L | Minus | Press, hold, and release. |
| Next page | R | Plus | Press, hold, and release. |
| Vertical memo scroll | PAD channel-zero stick Y; supported Nunchuk/Classic/Guitar Hero 3 extension stick Y | Same Remote channel's supported extension stick Y | Dead zone is applied, then the result is scaled to logical scroll pixels. It is pointer-owner restricted when a pointer owner exists. |
| Screenshot | Not mapped on PAD | Nunchuk C + Z held; Classic ZL + ZR held | Screenshot is requested when the screenshot button is newly pressed while the network-init button is held. Guitar Hero 3 has no screenshot chord. |
| Network initialization chord | Not mapped on PAD | Nunchuk Z or Classic ZR | Used only as the held part of the screenshot chord in this input map. |

Classic Controller and Guitar Hero 3 extension buttons are promoted into the
Remote mask before the shared map runs. Nunchuk buttons are already separate
from the Remote mask, so the screenshot chord has a dedicated extension map.
The source also maps Classic stick and shoulder/digital controls as shown by the
conversion function. A particular extension revision still needs hardware
evidence.

### Additional HBC controller mappings

These are explicit Rust app defaults in `hbc-rs/src/platform/input.rs`, rather
than C parity claims for new transports. Press, hold, and release masks use the
production controller transitions, including reset edges. Generic button names
are logical bindings; they do not identify a physical button on an unknown HID
device.

| Normalized controller kind | Confirm / Back | Options / Layout | Previous / Next page | Home | Memo scroll |
| --- | --- | --- | --- | --- | --- |
| GameCube through USB adapter | A / B | X / Y | L / R | Start | Left stick Y |
| Switch Pro or Joy-Con pair | A / B | X / Y | Minus / Plus | Home | Left stick Y |
| Joy-Con left alone | SR / SL | L-stick click / Capture | L / ZL | Minus | Left stick Y |
| Joy-Con right alone | A / B | X / Y | R / Plus | Home | Right stick Y |
| Other normalized kinds | Logical A / B | Logical X or Select / Y | Logical L / R | Logical Start | Left stick Y |

HBC enables USB input while it keeps the existing WPAD/Classic Bluetooth
ownership path. The screenshot chord remains the WPAD extension chord; it is
not added to the new controller defaults. Hardware runs must confirm the actual
unit, transport, labels, and action result.

HBC resolves pointer hit targets before it updates the app runtime. A pointer
press on an actual target consumes competing page actions. A pointer press on
blank space does not consume them. Home and Back screen transitions take
priority. Host regressions cover these decisions, but a real Wii run is still
required.

## HBC C-to-Rust input-map comparison

The following comparison covers the logical button conversion in
`C-sources/hbc/channel/channelapp/source/controls.c` and `controls.h` against
`RustPorts/hbc-rs/src/platform/input.rs`. The comparison is source review only.
The observed controller behavior on a Wii remains unverified.

| Physical input | C action mask | Rust logical action | Source comparison |
| --- | --- | --- | --- |
| GameCube D-pad | UP/DOWN/LEFT/RIGHT | Up/Down/Left/Right | Same PAD masks and directions. |
| GameCube A/B | Confirm/Back | Confirm/Back | Same PAD masks. |
| GameCube X/Y | Open options/Toggle browser layout | Open options/Toggle browser layout | `PADS_1` maps X to options; `PADS_2` maps Y to browser mode. Rust uses the same X/Y PAD values. |
| GameCube Start | Home | Home | Same PAD mask. |
| GameCube L/R triggers | Previous/Next page | Previous/Next page | `PADS_MINUS` uses PAD L and `PADS_PLUS` uses PAD R; Rust maps the same trigger masks. |
| Wii Remote D-pad | Up/Down/Left/Right | Up/Down/Left/Right | Same WPAD direction masks. |
| Wii Remote A/B | Confirm/Back | Confirm/Back | Same WPAD masks. |
| Wii Remote Minus/Plus | Previous/Next page | Previous/Next page | Same WPAD masks; C handles both page inputs each frame while held. |
| Wii Remote 1/2 | Open options/Toggle browser layout | Open options/Toggle browser layout | Same WPAD masks. |
| Wii Remote Home | Home | Home | Same WPAD mask. |
| Classic Controller | Directions, A/B, X/Y to 1/2, Minus/Plus, Home; ZR network-init and ZL screenshot chord | Same action set after promotion to the Remote mask | The source maps the same named Classic controls to Remote masks. Rust combines Classic L with Minus and R with Plus as C does. |
| Nunchuk | Z network-init; C screenshot chord | Same chord | Both require a new screenshot press while network-init stays held. |
| Guitar Hero 3 extension | Strum to up/down; green/red to A/B; yellow/blue to left/right; plus/minus and orange to 1/2 | Same common actions; no screenshot chord | The source maps the same common controls. Neither source assigns the special screenshot chord to this extension. |
| Pointer owner and button source | Keep a valid IR owner; otherwise combine Remote buttons | Keep a valid IR owner; otherwise combine Remote buttons | Same owner rule for Remote buttons. PAD channel-zero buttons remain included in both paths; Rust skips non-owner Remote channels while an owner exists. |
| Press/hold/release | C scans separate down, held, and up masks | Rust stores `pressed`, `held`, and `released` lists | Same event categories at the input boundary. Runtime parity still requires Wii observation. |

This comparison does not cover every detail of pointer positioning, scroll
scaling, rumble transfer, or failure handling. It also does not establish that
a given physical Classic Controller or other extension has the reported type or
button fields. Those facts need exact-device captures and real-Wii checks.

## HBC screen action matrix

The HBC app layer accepts the logical actions from `src/platform/input.rs`.
Actions below come from `src/app.rs` and `src/runtime.rs`. Hold repeat applies
to repeatable navigation inputs. One-shot actions such as Home and confirm do
not repeat just because they remain held. Release events are captured by the
platform but are not dispatched as separate `AppController::handle` actions.

| HBC screen or state | Actions | Timing and input details | Rust source | C reference |
| --- | --- | --- | --- | --- |
| Browser list/grid | Up/down/left/right select apps; Confirm opens selected app details or launches according to selection state; Back/Home enters main menu; ToggleBrowseMode changes list/grid; PreviousPage/NextPage changes page; OpenOptions opens options | Directional navigation can repeat while held. Other actions are press events. IR pointer can select app/page targets and confirm them. | `src/app.rs` (`handle`, `handle_pointer_frame`), `src/runtime.rs`, `src/platform/input.rs` | `C-sources/hbc/channel/channelapp/source/controls.c`, `controls.h`, `main.c`, and dialog/view handlers under `source/`. |
| Main menu | Up/down changes focus; Confirm activates the focused item; Back returns to browser; Home exits; pointer menu target confirms | Focus moves on actions; activation is a press. BootMii availability changes the menu item count. | `src/app.rs` | `C-sources/hbc/channel/channelapp/source/controls.c`, `controls.h`; C menu source under `source/`. |
| App details | Up/down moves detail focus or scrolls; Back returns; Confirm activates the focused action; pointer can choose detail target; pointer drag scrolls memo | Directional input repeats. Pointer memo drag uses a held confirm while pointer moves. | `src/app.rs` | C channel app controls and detail screen source under `C-sources/hbc/channel/channelapp/source/`. |
| Delete confirmation | Left/right changes Yes/No; Back cancels; Confirm accepts the selected choice | Confirmation is a press action. | `src/app.rs` | C channel app delete dialog under `source/`. |
| WiiLoad ZIP confirmation | Up/down or left/right selects; Back rejects; Confirm accepts or rejects according to the selected option | Confirmation is a press action. | `src/app.rs`, `src/runtime.rs` | C channel app WiiLoad dialog under `source/`. |
| Options | Up/down selects an option; left/right changes the pending value; Back returns; Home exits; Confirm applies | Navigation may repeat; apply is a press. | `src/app.rs` | C channel app options source under `source/`. |
| Network settings | Up/down selects fields; left/right adjusts supported values; Confirm opens or activates a field/action; Back returns; Home exits | Confirm may load or save config, test connection, scan, or open the keyboard according to field. Save without backup has a confirmation screen. | `src/app.rs` (`handle_network_settings` and handlers), `src/runtime.rs` | No C Rust-port match is claimed; compare the C channel-app network code in `source/` per operation. |
| Network scan | Up/down selects a network; Confirm selects it; Back returns | Selection is press based; network results come from runtime state. | `src/app.rs` | C channel-app network scan source under `source/`. |
| Network keyboard | Directions select key; Confirm inserts or activates key; Back exits or edits; pointer can choose keys | Repeated directional movement is supported. Key insertion is one-shot. | `src/app.rs` | C channel-app keyboard source under `source/`. |
| About | Up/down scroll; Back/Home returns or exits; pointer drag scrolls memo | Scroll repeats. Pointer drag requires confirm held after a pointer press. | `src/app.rs`, `src/platform/input.rs` | C channel-app About source under `source/`. |
| Notice/error/warning | Confirm or Back dismisses; Home follows screen-specific behavior; Up/down scrolls long text; pointer confirm dismisses | Warning and error notices close after five seconds without input; input resets the idle timer. A timed notice consumes the first pressed input before normal dispatch. | `src/app.rs`, `src/runtime.rs` | C channel-app message/dialog source under `source/`. |
| Wiiload receiving/installing | Up/down scroll long text; confirm/back behavior depends on current transfer state; transfer confirmation is separate | Transfer completion and notice timing are runtime/service state. A hold does not itself report operation completion. | `src/app.rs`, `src/runtime.rs`, `src/main.rs` | C channel-app WiiLoad source under `source/`. |

The C path scans PAD channel zero and all probed Wii Remote channels. It keeps an
IR pointer owner while that Remote remains valid, uses that owner's buttons, and
otherwise combines Remote button events. It transforms extension controls and
tracks press, hold, and release masks. Compare with
`C-sources/hbc/channel/channelapp/source/controls.c` and `controls.h`.

## Status boundary

These matrices are source-backed. They do not establish physical device
detection, transport mode, raw report format, controller-to-action correctness
on a Wii, reconnect behavior, or Wii acceptance. Those evidence fields remain
separate in [`config/controller-devices.csv`](../config/controller-devices.csv).
No Wii acceptance result is added by this document.

## Source register

- Rust HBC: `RustPorts/hbc-rs/src/platform/input.rs`, `src/app.rs`, `src/runtime.rs`, and `src/main.rs`.
- C HBC: `C-sources/hbc/channel/channelapp/source/controls.c`, `controls.h`, and screen handlers under `source/`.
- C libogc PAD and WPAD API: `C-sources/libogc/gc/ogc/pad.h`, `libogc/pad.c`, `gc/wiiuse/wpad.h`, and `wiiuse/wpad.c`.
- No matching C sources for the seven Rust game examples were found under `C-sources/`.

## External controller evidence

Use exact identity and physical captures before assigning a protocol or
compatibility result. The supplied controller research report is stored at
`/tmp/codex-remote-attachments/01a13313-b9e9-7530-987d-796bced5bd1b/DC1FA6D9-5373-4889-87CA-05962069B6E5/1-deep-research-report-2.md`.

- USB HID 1.11 defines HID descriptors and reports; it does not prove that a
named controller uses HID: [USB-IF HID 1.11](https://www.usb.org/sites/default/files/documents/hid1_11.pdf).
- HID Usage Tables define usage meanings: [USB-IF HID Usage Tables 1.7](https://usb.org/document-library/hid-usage-tables-17).
- Bluetooth HID Classic and HID over GATT are distinct profiles:
[Bluetooth HID Profile](https://www.bluetooth.com/specifications/specs/human-interface-device-profile1-1-2ed2/),
[Bluetooth HID over GATT Profile](https://www.bluetooth.com/specifications/specs/hid-over-gatt-profile/).
- Nintendo distinguishes Joy-Con model suffixes `HAC-015` / `HAC-015(-01)`
and `HAC-016` / `HAC-016(-01)` in its [support article](https://www.nintendo.com/en-za/Support/Troubleshooting/Battery-replacement/How-to-identify-whether-your-Joy-Con-controller-contains-a-user-replaceable-battery-3096478.html).
- Nintendo provides user-level Switch Pro pairing steps in its
[controller support article](https://www.nintendo.com/en-gb/Support/Troubleshooting/How-to-Pair-the-Nintendo-Switch-Pro-Controller-1497998.html).

These standards and manufacturer pages do not establish the report format,
firmware, pairing behavior, or Wii acceptance of the physical devices in the
inventory. Keep unknown values unknown until a source or capture establishes
them.
