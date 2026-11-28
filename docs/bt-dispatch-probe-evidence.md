# Bluetooth dispatch probe evidence

Run: 2026-11-27, Dolphin 2609a, `bt_dispatch_probe`.

- Probe result: pass, 8 assertions, 0 failures.
- DOL SHA-256: `58f86077959225d9084696f75aa7223a8e4b60984eb5691a1de98964a4a9ce0e`.
- Assertions cover disconnect response ordering, setup-slot reservation, and
  routing a modeled peer event to its matching setup slot.
- The probe does not create a live Bluetooth connection or prove IOS transport
  behavior. Live connection setup and physical Wii Bluetooth behavior remain
  open.

The captured run used a private Dolphin user directory and the installed NAND.
It did not launch a System Menu or a test title, so no title handoff is claimed.
