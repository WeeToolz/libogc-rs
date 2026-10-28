# UI input probe

The optional `ui_input_probe` DOL checks the production controller path:
`ControllerManager::scan_at` -> `ControllerInputAdapter` -> `Ui::update` ->
`GxCanvas` -> GX and VI. The PowerPC monitor uses hardware input only. The
host fixture in `probes/ui-input-probe-tests` calls the same mapper and retained
widget tree with explicit snapshots; it is not hardware evidence.

The probe initializes PAD, WPAD/Bluetooth HID, and USB HID. It draws a list, a
grid, a disabled button, an open-dialog button, and a modal dialog. Use the
connected controller to move focus, activate controls, move a valid pointer,
activate a pointer target, open and close the dialog, and finish the collection.
The on-screen status line shows the last source and UI event. The monitor uses
the selected video mode dimensions for both input mapping and GX rendering.

Build the DOL with the normal project builder:

```sh
cargo xtask build-dol ogc-system-probe --bin ui_input_probe --probe-features ui-input-probe
```

The monitor stops after 9,000 display frames if the sequence is incomplete.
It records UI observations and raw events through the existing input-probe
trace queue and checked agent capture. The monitor records a
`collection_candidate` after observations and shutdown. ProbeLogger's final
summary owns the durable PASS/FAIL result after log synchronization. The probe
always reports `hardware_verdict=PENDING`; a host test or DOL build cannot
establish physical controller behavior. A missing device, widget action,
pointer activation, focus scope, disabled-item check, or finish action leaves
the collection candidate incomplete. Capture, trace, shutdown, or display
restore failures also leave it incomplete.
Agent capture has a fixed capacity. A full capture stops the run as incomplete;
use short, focused runs and preserve the logger file when the run produces more
raw input records than the capture can hold.

The dialog changes the focus scope after its opening action. The monitor resets
mapper repeat and stick gates at that boundary. Each event records the actual
focus before and after `Ui::update`, the target returned by the same
focus-scope-aware tree hit test, the event and widget ID, mapped input, source
and controller generation when available, labels, pointer owner, and whether
the source identity was observed. Controller errors include their operation,
failure, and cleanup code. Missing identity is recorded as unobserved.

The VI retrace callback is saved and disabled while GX owns the display, then
restored with the prior framebuffer after the monitor exits. A failed VI or
GX cleanup is recorded as a collection failure. On exit, the probe requests
USB and Bluetooth shutdown and waits for both transports to finish for up to
six seconds; an error or timeout leaves the collection incomplete. This probe
does not certify USB, Bluetooth, controller mapping, or application behavior as
accepted.
