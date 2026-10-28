# ogc-ui

`ogc-ui` is a small retained-mode widget toolkit. The app creates widgets once,
passes input snapshots to `Ui::update`, handles returned events, and draws the
tree through a `Canvas`.

```rust
use ogc_ui::{Layout, Size, SliderSpec, Ui, UiEvent, UiInput};

let mut ui = Ui::new(640, 480)?;
let menu = ui.add_container(ui.root(), Layout::column(8, 12), Size::fill())?;
let audio = ui.add_checkbox(menu, "ENABLE SOUND", true, Size::fill_width(32))?;
let _stereo = ui.add_radio(menu, 1, "STEREO", true, Size::fill_width(32))?;
let _mono = ui.add_radio(menu, 1, "MONO", false, Size::fill_width(32))?;
let volume = ui.add_slider(
    menu,
    "VOLUME",
    SliderSpec { min: 0, max: 100, step: 5, value: 70 },
    Size::fill_width(48),
)?;
let entries = ui.add_list(menu, &["START", "OPTIONS", "QUIT"], 0, Size::fill())?;
let loading = ui.add_progress(menu, "LOADING", 0, Size::fill_width(36))?;

match ui.update(UiInput {
    activate: true,
    ..UiInput::default()
}) {
    Some(UiEvent::Toggled { widget, checked }) if widget == audio => {
        // Persist or apply the app setting here.
        let _ = checked;
    }
    _ => {}
}

// While the slider or list has focus, UiInput direction edges change its value
// or selected row and return ValueChanged or SelectionChanged.
let _ = (volume, entries);
ui.set_progress(loading, 25)?;
```

## Built-in controls

- `Button`: focus, pointer activation, and `Activated` event.
- `Checkbox`: focus, pointer activation, `checked`/`set_checked`, and `Toggled` event.
- `Radio`: exclusive selection for buttons with the same parent and group. Use
  `checked`/`set_checked` to read or change the selection.
- `Slider`: inclusive `u16` range, nonzero step, left/right adjustment, pointer value
  selection, and `ValueChanged` event.
- `List`: owns its labels, navigates with up/down, scrolls the selected row into view,
  supports pointer row selection, and reports `SelectionChanged`.
- `Progress`: non-focusable 0–100 percent bar, updated through `set_progress`.
- `Label`, `Container`, and `CustomWidget` for static text, layout, and app-specific
  controls. Custom widgets are non-focusable by default; override `focusable()`
  when the widget handles user input.

`UiInput` carries button edges and an optional screen-coordinate pointer. The app
owns controller polling, text entry, keyboard focus policy, and any persistence
behind control events. The optional `pad-input` feature provides a GameCube PAD
edge adapter. The optional `wpad-input` feature provides a Wii Remote, Nunchuk,
and Classic Controller adapter with IR pointer scaling.

Enable `controller-input` to map an `ogc-controller::ControllerSnapshot` to
`UiInput`. This adapter does not poll hardware. It uses each controller's kind
defaults and snapshot press/release edges. A caller can override defaults for a
controller kind or one source. Profile updates and mapping return `UiError`;
propagate allocation failures to the caller. The defaults use A/B for GameCube, Switch Pro,
and paired/right Joy-Cons. A left Joy-Con uses SR/SL. Unknown kinds use generic
Confirm/Back labels until the app supplies exact labels. The adapter combines
direction input and uses 25 percent stick activation with 20 percent release.
Its default repeat is 350 ms, then 80 ms, converted to frames at 60 Hz. Set the
actual frame rate in `ControllerInputConfig` when it differs. Call `reset()`
when a dialog opens or closes; a held stick stays blocked until it returns to
neutral.

The adapter keeps one valid pointer owner while that source remains valid. It
scales finite coordinates from the reported pointer resolution to the UI
viewport. An invalid or offscreen point does not become a click at a viewport
edge. Only the pointer owner's confirm button sets `pointer_activate`. A confirm
from another controller sets `activate` only when the pointer owner did not also
confirm in that frame. If both confirm, the pointer owner wins so one input frame
cannot target two widgets. Cancel clears both activation flags for that frame. `ControllerInputFrame::released` reports release edges to the app;
`UiInput` and the widget tree do not deliver release events.

USB keyboard and text entry adapters are not included yet. The GX backend draws
filled rectangles and the built-in bitmap font; image textures and broader font
support remain separate backend work.

Use `Ui::set_focus_scope(container)` while a modal is open. This limits keyboard
focus and pointer targeting to that container and its descendants. Restore the
root scope after the modal closes. Use `KeyRepeat` when a screen needs held-key
repeat. Pass that screen's delay and interval so its timing stays explicit.
