# ogc-ui

`ogc-ui` is a small retained-mode widget toolkit for Wii and GameCube apps.
The app creates widgets once. The toolkit keeps their labels, visibility,
enabled state, focus, and layout. The app sends input snapshots and draws the
tree through a `Canvas`.

The core crate does not select a controller or own the video loop. This keeps
apps in control of polling, frame timing, and presentation. It also makes the
same widget tree usable with another input source or drawing backend.

## First screen

```rust,ignore
use ogc_ui::{Layout, Size, Ui};

let mut ui = Ui::new(640, 480)?;
let panel = ui.add_container(ui.root(), Layout::column(8, 12), Size::fill())?;
ui.add_label(panel, "Settings", Size::fill_width(32))?;
let save = ui.add_button(panel, "Save", Size::fill_width(40))?;

// Each frame: convert the app's controller state to UiInput, then:
if let Some(event) = ui.update(input) {
    // Handle Activated(save) in the application.
}
ui.draw(&mut canvas)?;
// Present the frame through the application's existing video path.
```

Layouts use a row or column. A child can use `Dimension::Pixels` or `Fill` on
each axis. Padding and gaps are in pixels. Hidden parents also hide their
children. Focus skips disabled and hidden widgets. `Ui::update` returns at most
one event for each input snapshot.

## Custom content

Implement `CustomWidget` for content that changes every frame, such as a graph
or animation. The toolkit gives it bounds and a clipped canvas. The widget can
draw only inside those bounds. Its `update` method receives the same input
snapshot when it has focus. This provides immediate drawing within a retained
screen without making the application rebuild the whole screen.

## Wii backends

Enable `gx-backend` to use `GxCanvas`. Create it after the application has
initialized GX and the video mode. Its constructor is unsafe because the app
must own and preserve the GX state while it draws. Then present the frame with
the application's normal video path.

Enable `pad-input` to use `PadInputAdapter`. Keep one adapter per PAD channel.
It maps A to activate, B to cancel, and the D-pad to spatial focus movement.
Wii Remote apps can map their WPAD state to `UiInput` directly.

Enable `controller-input` to map a normalized `ControllerSnapshot` to
`UiInput`. The feature does not poll hardware. It uses kind defaults and the
snapshot's button edges; apps can override a controller kind or a specific
source. Profile updates and mapping return `UiError`; propagate allocation
failures to the caller. Defaults use A/B for GameCube and Switch controllers. A
left Joy-Con
uses SR/SL. Unknown kinds use Confirm/Back labels until the app provides exact
button names. The stick direction activates at 25 percent and releases at 20
percent. Repeat starts after 350 ms and repeats every 80 ms, converted to
frames using the configured frame rate. Call `reset()` when a modal opens or
closes. A held stick stays blocked until it returns to neutral.

Pointer coordinates use the valid controller metadata resolution and the UI
viewport. Invalid or offscreen samples do not clamp to an edge. Only the chosen
pointer owner's confirm edge becomes `pointer_activate`. A confirm from another
controller becomes `activate` only when the pointer owner did not also confirm.
If both confirm, the pointer owner wins so one input frame cannot target two
widgets. Cancel suppresses both activation flags. The mapped frame
reports release edges for the application, but `UiInput` does not pass release
events into widgets.

The built-in GX canvas uses a compact uppercase 5 by 7 bitmap font. The shared
`glyph_rows` function is also used by HBC-RS. Apps can implement `Canvas` to
use their own fonts, textures, or visual style.

All widget IDs are stable for the lifetime of a `Ui`. Methods return
`UiError` for invalid IDs, invalid state, or allocation failure. Keep the IDs
returned by the add methods instead of looking up widgets by label.
