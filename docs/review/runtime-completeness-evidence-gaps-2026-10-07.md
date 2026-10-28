# Runtime completeness audit: exact evidence gaps

Updated: 2026-10-07

This is a historical snapshot of the evidence gaps on that date. Its counts
are not current. Use [the generated inventory summary](../generated/implementation-status.md)
and `cargo xtask audit-stubs` for current results.

## Audit rule

Each evidence record must match the crate, crate-relative source path, public symbol, declaration line, and declared classification. Its kind must be `hardware-probe` for `hardware`, `ios`, and `scheduler` APIs. Other allowed classes can use a `host-test` or `hardware-probe`. The named test or probe function must contain a direct call to that API. The validator checks this syntactically; it does not prove behavior or hardware acceptance.

## Progress

The audit first reported 2,565 APIs without exact evidence. I added exact records for 328 APIs already called by existing production-path probes. I also added runtime checks for GX texture descriptors and their getters to the Wii video probe. The inventory now has 616 APIs with exact evidence and 2,237 remaining unverified APIs. No audit rule or classification was weakened.

## Remaining APIs by interface kind

| API kind | Remaining |
| --- | ---: |
| `extern-c` | 127 |
| `rust-function` | 1763 |
| `rust-method-or-module-function` | 324 |
| `trait-method` | 23 |

## Remaining APIs by crate and interface kind

Counts below are the current remaining unverified APIs. The API kind counts identify which interface surface needs exact evidence.

| Crate | Total | API kind counts |
| --- | ---: | --- |
| `aesnd` | 31 | `rust-function`: 6; `rust-method-or-module-function`: 25 |
| `aesnd-sys` | 19 | `rust-function`: 19 |
| `asnd-sys` | 20 | `rust-function`: 13; `rust-method-or-module-function`: 7 |
| `iso9660` | 3 | `extern-c`: 3 |
| `libdb-sys` | 1 | `extern-c`: 1 |
| `lwbt-sys` | 40 | `rust-function`: 40 |
| `lwip-sys` | 32 | `rust-function`: 16; `rust-method-or-module-function`: 16 |
| `modplay-capi` | 24 | `rust-function`: 24 |
| `mp3player-sys` | 11 | `rust-function`: 11 |
| `ogc` | 1 | `rust-function`: 1 |
| `ogc-aram` | 26 | `rust-function`: 25; `rust-method-or-module-function`: 1 |
| `ogc-audio-sys` | 22 | `rust-function`: 22 |
| `ogc-bt-sync` | 108 | `rust-function`: 16; `rust-method-or-module-function`: 92 |
| `ogc-cache` | 28 | `rust-function`: 28 |
| `ogc-card` | 30 | `rust-function`: 9; `rust-method-or-module-function`: 21 |
| `ogc-card-sys` | 70 | `rust-function`: 64; `rust-method-or-module-function`: 6 |
| `ogc-conf` | 32 | `rust-function`: 32 |
| `ogc-di` | 55 | `extern-c`: 28; `rust-function`: 27 |
| `ogc-dsp` | 15 | `rust-function`: 15 |
| `ogc-dsp-sys` | 18 | `rust-function`: 15; `rust-method-or-module-function`: 3 |
| `ogc-dvd` | 20 | `rust-function`: 20 |
| `ogc-dvd-sys` | 21 | `rust-function`: 21 |
| `ogc-es-sys` | 72 | `rust-function`: 72 |
| `ogc-exi` | 27 | `rust-function`: 27 |
| `ogc-exi-sys` | 14 | `rust-function`: 14 |
| `ogc-fat` | 28 | `extern-c`: 8; `rust-function`: 13; `rust-method-or-module-function`: 7 |
| `ogc-filesystem` | 29 | `rust-function`: 21; `rust-method-or-module-function`: 8 |
| `ogc-gcsd` | 43 | `extern-c`: 19; `rust-function`: 24 |
| `ogc-gu` | 45 | `rust-function`: 45 |
| `ogc-gx` | 264 | `rust-function`: 257; `rust-method-or-module-function`: 7 |
| `ogc-input` | 20 | `rust-function`: 20 |
| `ogc-input-sys` | 28 | `rust-function`: 28 |
| `ogc-ios-sys` | 10 | `rust-function`: 10 |
| `ogc-ipc` | 40 | `extern-c`: 5; `rust-function`: 7; `rust-method-or-module-function`: 28 |
| `ogc-irq` | 13 | `rust-function`: 13 |
| `ogc-irq-sys` | 10 | `rust-function`: 10 |
| `ogc-lwp` | 40 | `rust-function`: 3; `rust-method-or-module-function`: 37 |
| `ogc-lwp-driver` | 55 | `extern-c`: 1; `rust-function`: 53; `rust-method-or-module-function`: 1 |
| `ogc-lwp-sys` | 56 | `extern-c`: 35; `rust-function`: 21 |
| `ogc-network` | 49 | `rust-function`: 47; `trait-method`: 2 |
| `ogc-network-sys` | 71 | `rust-function`: 39; `rust-method-or-module-function`: 32 |
| `ogc-runtime` | 22 | `rust-method-or-module-function`: 1; `trait-method`: 21 |
| `ogc-si` | 22 | `rust-function`: 22 |
| `ogc-si-sys` | 28 | `rust-function`: 28 |
| `ogc-stm` | 14 | `rust-function`: 14 |
| `ogc-sysbase` | 21 | `extern-c`: 5; `rust-function`: 8; `rust-method-or-module-function`: 8 |
| `ogc-system` | 74 | `rust-function`: 69; `rust-method-or-module-function`: 5 |
| `ogc-system-sys` | 99 | `extern-c`: 6; `rust-function`: 92; `rust-method-or-module-function`: 1 |
| `ogc-usb` | 59 | `rust-function`: 54; `rust-method-or-module-function`: 5 |
| `ogc-usb-events` | 1 | `rust-method-or-module-function`: 1 |
| `ogc-usb-sys` | 42 | `rust-function`: 42 |
| `ogc-usbgecko` | 7 | `rust-function`: 7 |
| `ogc-usbstorage` | 23 | `rust-function`: 23 |
| `ogc-usbstorage-sys` | 45 | `extern-c`: 15; `rust-function`: 30 |
| `ogc-video` | 22 | `rust-function`: 22 |
| `ogc-video-sys` | 34 | `rust-function`: 34 |
| `ogc-wd` | 19 | `rust-function`: 14; `rust-method-or-module-function`: 5 |
| `ogc-wiilaunch` | 16 | `rust-function`: 14; `rust-method-or-module-function`: 2 |
| `ogc-wiisd` | 4 | `rust-method-or-module-function`: 4 |
| `tinysmb-sys` | 39 | `rust-function`: 39 |
| `tuxedo-sys` | 34 | `extern-c`: 1; `rust-function`: 32; `rust-method-or-module-function`: 1 |
| `wiikeyboard-sys` | 4 | `rust-function`: 4 |
| `wpad-sys` | 67 | `rust-function`: 67 |
