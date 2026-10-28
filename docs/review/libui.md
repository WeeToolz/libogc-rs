# `ogc-ui` static review

Date: 2026-10-15.

## Scope

Compare the public Rust UI APIs and production behavior with checked-in libogc
contracts. The C tree has no retained widget toolkit or matching widget APIs;
the widget tree, canvas abstraction, built-in font, key repeat, and UI action
policy are Rust-only additions. The optional PAD, WPAD, controller, and GX
adapters delegate hardware access to their existing driver crates. This review
does not claim that Rust-specific widget behavior is C parity.

## Review result

The scope is complete for this Rust-only crate. `tree.rs` owns widget state,
layout, focus, input dispatch, and drawing through `Canvas`; `font.rs` provides
the built-in bitmap glyphs; `repeat.rs` provides caller-configured key repeat.
The optional PAD, WPAD, and controller adapters map existing production input
snapshots, and `gx.rs` delegates rectangle and text drawing to `ogc-gx`.
Allocation and drawing errors are returned through `UiError` and
`CanvasError`. The crate does not poll a second hardware backend.

Search of the checked-in libogc headers and implementations found controller
and GX APIs, but no matching widgets, focus tree, retained layout, or input
repeat contract. Those are therefore not C parity claims. Source review found
no unresolved duplicated hardware implementation in `ogc-ui`.

Validation: `cargo test -p ogc-ui --features controller-input` passed 30 unit
tests. Doc tests had zero cases. This is host validation of Rust-only behavior;
it does not validate PAD, WPAD, or GX hardware behavior.
