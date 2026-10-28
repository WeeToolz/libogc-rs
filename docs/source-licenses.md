# License and source notices

This project is a Rust port with code and assets from several sources. The
workspace manifest declares `Zlib` for crates that inherit its license. The
root [LICENSE](../LICENSE) reproduces the notice from the libogc C reference,
including its additional attribution for threading code. This Rust port is an
altered implementation and is not the original libogc distribution.

## Package and source terms

Cargo uses `AND` for packages with portions under different terms. Each
portion keeps its own terms. A dependency's license does not replace a
package's source license. Review the linked code separately for each binary
distribution.

| Component | Package declaration | Source and notices |
| --- | --- | --- |
| `wpad-sys` | `GPL-3.0-only AND Zlib` | The existing GPLv3-only declaration covers the Rust port. The derived WPAD code retains the [notice from libogc's `wpad.c`](../crates/services/wpad-sys/LICENSE-libogc). The [GPLv3 text](../crates/services/wpad-sys/LICENSE-GPL) accompanies the crate. See the scope note below. |
| `wiiuse-rs` | `GPL-3.0-or-later AND Zlib` | The Wiiuse port retains [Wiiuse attribution](../crates/services/wiiuse-rs/NOTICE-Wiiuse) and the [GPLv3 text](../crates/services/wiiuse-rs/LICENSE). Keep the [libogc notice](../crates/services/wiiuse-rs/LICENSE-libogc) with the adapter patch sourced from `lwbt/bte.c`. |
| `gc-binary`, `gc-elf` | `GPL-2.0-or-later` | The tools retain their existing declarations and [GPLv2 text](../crates/tools/LICENSE). `gc-elf` draws on GameCube Tools and uses an adapted HBC Reborn real-mode stub. |
| `gc-wad` | `GPL-2.0-only` | The Rust port retains the [version-specific grant and copyright](../crates/tools/gc-wad/NOTICE-pywii) from HBC Reborn's `pywii/Common/pywii/wii.py`. The [GPLv2 text](../crates/tools/gc-wad/LICENSE) comes from HBC Reborn. Its repository-wide “or later” statement and the broader tools license do not override the pywii file notice. |
| `modplay`, `modplay-capi` | `BSD-3-Clause AND Zlib` | Keep Christian Nowak's notices for the [tables](../crates/media/modplay/LICENSE-BSD) and [mixer and player port](../crates/abi/modplay-capi/LICENSE-BSD). |
| `ogc-filesystem-sys`, `ogc-gcsd`, `ogc-wiisd` | `BSD-3-Clause AND Zlib` | Keep the [DiscInterface](../crates/services/ogc-filesystem-sys/LICENSE-BSD), [GameCube SD](../crates/services/ogc-gcsd/LICENSE-BSD), and [Wii SD](../crates/services/ogc-wiisd/LICENSE-BSD) source notices. |
| `ogc-video-sys`, `asnd-sys` | `BSD-3-Clause AND Zlib` | Keep the [video](../crates/hardware/ogc-video-sys/LICENSE-BSD) and [ASND](../crates/media/asnd-sys/LICENSE-BSD) source notices. The separate native AESND DSP asset has [its own notice](../crates/media/aesnd-sys/dsp/LICENSE). |
| `wiikeyboard-sys` | `BSD-2-Clause AND Zlib` | Keep the [NetBSD key symbol and map notices](../crates/compatibility/wiikeyboard-sys/LICENSE-NetBSD) and the [libogc keyboard notices](../crates/compatibility/wiikeyboard-sys/LICENSE-libogc). |
| `lwip-sys`, `lwbt-sys` | `BSD-3-Clause AND Zlib` for lwIP, `BSD-3-Clause` for lwBT | The [upstream notices](third-party-notices.md) cover the BSD stack code. The Wii network port retains the [libogc `network_wii.c` notice](../crates/services/lwip-sys/LICENSE-libogc). That notice is also in [`ogc-network-sys`](../crates/services/ogc-network-sys/LICENSE-libogc). |
| `tinysmb-sys` | `LGPL-2.1-or-later` | The [TinySMB notice](third-party-notices.md#tinysmb) names version 2.1 or later. The package includes the [LGPLv2.1 text](../crates/compatibility/tinysmb-sys/LICENSE-LGPL). |
| `mad-sys`, `mp3player-sys`, vendored `libmad-rs` | `GPL-2.0-or-later` | The copied ABI implementation and [vendored decoder](../vendor/libmad-rs) retain the original decoder terms. See the [libmad attribution](../crates/media/mad-sys/NOTICE-libmad), [GPLv2 text](../crates/media/mad-sys/LICENSE), and [import record](dependencies.md). The `mp3player.c` attribution is separate from the decoder grant. |

The WPAD package expression preserves the documented libogc grant and the
existing GPL declaration. It does not assign either grant to all source files
or extend libogc's separate Wiiuse linking exception. A file that incorporates
material from both sources must preserve the applicable notices and conditions.
The [libogc exception text](https://github.com/devkitPro/libogc/blob/7d4b8b3e7b845516f0cc2a85274879ddd8cbf142/wiiuse/license_libogc.txt#L623-L639)
documents the upstream scope.

Rust standard-library excerpts in `rust-std/patches` retain their upstream
`MIT OR Apache-2.0` terms. The [source lock](../rust-std/source-lock.json)
identifies the exact revision. Preserve the Rust license texts and notices
when distributing the patched standard library.

The libogc reference used for the current review is
[`devkitPro/libogc`](https://github.com/devkitPro/libogc) at
`7d4b8b3e7b845516f0cc2a85274879ddd8cbf142`. Its README credits Michael
Wiedenbauer (`shagkur`) and Dave Murphy (`WinterMute`). The Wiiuse reference is
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
The native DSP binary has adjacent source and a notice. The [ISO 9660 source notice](../crates/media/iso9660/NOTICE-libogc) records the authors of the reference implementation. The WAD signature tests
use `crates/tools/gc-wad/tests/fixtures/rsa-cert.bin` and
`crates/tools/gc-wad/tests/fixtures/rsa-ticket.bin`. The Letter Flip example
uses `examples/letter-flip/icon.png`. The repository owner confirmed that
these fixtures and this icon are suitable for public distribution. Their
creation and original source are not recorded in this checkout, so this
confirmation does not establish their authorship or provenance.

## Open source records

- The libogc checkout identifies the adapter patch bytes in `lwbt/bte.c` and
  the console font in `libogc/console_font_8x16.c`. Those files do not record
  the original authorship or grant for the embedded bytes. Keep the libogc
  notice and verify original provenance before treating either asset as
  cleared for redistribution.
- The MP3 player reference credits Daniel “nForce” Bloemendal and records
  Hermes's modifications. It has no separate grant in `libmad/mp3player.c`.
  The Rust crate retains its existing declaration while the original player
  grant is checked. The decoder's grant applies to the decoder.
- For WPAD, the package metadata accounts for the two known sets of terms.
  The complete file-by-file attribution for the Rust port remains open.
- TinySMB's DES and MD4 reference files have no explicit license header.
  Verify their original source before treating the SMB notice as a complete
  attribution for these routines.
