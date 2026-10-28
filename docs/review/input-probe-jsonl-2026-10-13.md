# Input probe JSON event records

The input probe now uses `ControllerManager::scan_with_raw_at` and installs
the production input trace observer. It writes one JSON object per line through
the same `ProbeLogger` used by the other probe records.
It initializes PAD/WPAD, enables USB discovery, and tries the production
Joy-Con pairing path while it scans.

The `run` record contains a unique ID derived from the Wii timebase and the
start tick. It records the DOL hash, device model, and firmware as `null`
because the probe cannot read those values from its current runtime interface.
Do not replace those fields with guessed values.

The probe writes these record types:

- `raw_report`: USB or Bluetooth HID report bytes in hex, source slot,
  timestamp, Bluetooth generation when available, and USB VID/PID when the
  manager still has that device record. `slot_a` is the source slot. `slot_b`
  is the second Joy-Con slot for a pair; 255 means it does not apply.
- `decoded`: normalized buttons, edges, axes, triggers, pointer state,
  capabilities, and source-specific PAD or WPAD fields when the state changes.
- `reset`: source generation, reset reason, and buttons released.
- `error`: source, operation, input failure, and cleanup error when present.

The observer callback only copies a bounded report into a fixed queue. It does
not call the logger or block while the controller manager scans. The probe
drains the queue after the scan. If the queue is full or a report exceeds the
buffer, the observer returns an error; the manager exposes that error in the
snapshot. The logger separately reports file, console, capacity, and sync
failures.

This probe records controller-layer events. It does not know which game action
or UI widget consumes an input. It does not include the C game implementations
because matching C game sources are absent from the reference checkout. The
probe was built for PowerPC with:

```text
cargo xtask build-dol ogc-system-probe --bin input_probe \
  --features ogc-system-probe/dolphin-automation \
  --output target/controller-input-trace/input_probe.dol
```

The build passed, but the new probe has not run on a Wii. Dolphin does not
prove USB passthrough or real Wii Bluetooth behavior.
