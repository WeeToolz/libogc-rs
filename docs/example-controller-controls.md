# Example DOL controller controls

Most example DOLs use GameCube PAD and USB HID input through the shared
controller manager. Their control modes differ by example. The 2048 game uses
the GameCube PAD path so it can run on an original GameCube as well as a Wii.

| Example | Manual controls |
| --- | --- |
| `autoplay_2048` | D-pad or left stick moves tiles and changes menu selection; A confirms, B goes back, and Start opens the title menu or pauses play. Left and right change audio levels in Options. Best score and audio levels save to GameCube memory-card slot A or B when a card is available. |
| `sudoku` | The D-pad moves the cell cursor, A advances a value, and B clears a value. |
| `snake-gc` | The D-pad changes direction; A or Start enters human mode. |

The default Wii builds enable Wii Remote input. Startup and discovery progress
run in the controller scan loop, so the game can continue while the adapter
starts and searches for controllers. Set `--no-default-features` to build
without Wii Remote support.

## Source and evidence boundary

The screen-specific Rust control actions and available C references are listed
in [`controller-action-matrices.md`](controller-action-matrices.md). The seven
Rust examples do not have matching C game sources in this checkout. Their
control descriptions are Rust source behavior, not C-to-Rust parity findings
or runtime results. The exact-device evidence baseline is in
[`config/controller-devices.csv`](../config/controller-devices.csv). A control
mapping, HID parser, or Bluetooth decoder in source does not prove that a named
physical controller is detected, mapped correctly, reconnects, or works on Wii.
