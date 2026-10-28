# License and source notices

This project is a Rust port with code and assets from several sources. The
workspace manifest declares `Zlib` for crates that inherit its license. The
root [LICENSE](../LICENSE) reproduces the notice from the libogc C reference,
including its additional attribution for threading code. This Rust port is an
altered implementation and is not the original libogc distribution.

Some components have separate notices and license declarations:

- [`wiiuse-rs`](../crates/services/wiiuse-rs/Cargo.toml) declares
  `GPL-3.0-or-later`; its [GPL notice](../crates/services/wiiuse-rs/LICENSE)
  and [libogc notice](../crates/services/wiiuse-rs/LICENSE-libogc) are included.
- The `gc-binary`, `gc-elf`, and `gc-wad` tool crates declare
  `GPL-2.0-or-later`; see [their license](../crates/tools/LICENSE).
- `mad-sys` and `mp3player-sys` declare `GPL-2.0-or-later`;
  `wpad-sys` declares `GPL-3.0-only`; `tinysmb-sys` declares
  `LGPL-2.1-or-later`; and `lwbt-sys` and `lwip-sys` declare
  `BSD-3-Clause`. These are package metadata declarations; the source and
  required notices still need a component-by-component provenance review.
- The vendored [`libmad-rs`](../vendor/libmad-rs/Cargo.toml) decoder declares
  `GPL-2.0-or-later`; its [license](../vendor/libmad-rs/LICENSE) is included.
  The `mad-sys` ABI implementation files were copied from the same local source
  repository and retain the crate's `GPL-2.0-or-later` declaration.
- The native DSP asset has its own [license notice](../crates/media/aesnd-sys/dsp/LICENSE).

The libogc reference used for the current review is
[`devkitPro/libogc`](https://github.com/devkitPro/libogc) at
`ff53c4a7b7ba5587d82305e710ca65bce1092e1c`. The Wiiuse reference is
[`wiiuse/wiiuse`](https://github.com/wiiuse/wiiuse); the GameCube tools reference
is [`devkitPro/gamecube-tools`](https://github.com/devkitPro/gamecube-tools).
See each component's source and review notes for its specific origin.

This is a record of known notices, not a completed audit of every file and
asset. Check the original source and retain its notices before adding or
redistributing third-party material. The root license does not replace a
component's separate license.

The repository also contains binary assets. The GC ELF review records
`crates/tools/gc-elf/src/channel-entry.bin` as built from the adjacent assembly
source, which identifies its GPL reference.
The native DSP binary has adjacent source and a notice. The WAD signature tests
use `crates/tools/gc-wad/tests/fixtures/rsa-cert.bin` and
`crates/tools/gc-wad/tests/fixtures/rsa-ticket.bin`. The Letter Flip example
uses `examples/letter-flip/icon.png`. The repository owner confirmed that
these fixtures and this icon are suitable for public distribution. Their
creation and original source are not recorded in this checkout, so this
confirmation does not establish their authorship or provenance.

[Selected upstream source notices](third-party-notices.md) preserve the
notices found in the C reference for TinySMB, lwBT, lwIP, and WPAD. The
reference `wpad.c` contains a Zlib-style notice. The `wpad-sys` Rust source
uses the separately declared `wiiuse-rs` crate, which declares
`GPL-3.0-or-later`, while the `wpad-sys` manifest declares `GPL-3.0-only`.
These facts do not establish the correct license expression for all code in
`wpad-sys`. Keep this as an open source review item; do not infer that the
`wpad.c` notice applies to the Wiiuse-derived code or that a dependency's
license alone determines the crate's declaration.
