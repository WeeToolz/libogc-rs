# Development results

## 2026-10-29: ISO 9660 and SHA runtime probe in Dolphin

- Command: `LIBOGC_MAD_PROBE_MP3=<fixture> LIBOGC_MAD_REFERENCE_MP3=<fixture> cargo xtask dolphin-validate --probe runtime_probe --artifact-dir /Volumes/SSD/WiiDev/tmp/libogc-runtime-iso-20261029`.
- Fixture: `/Volumes/SSD/WiiDev/Devtools/libmad-rs/test-data/layer3/libmad-stereo-mpeg1.mp3`, SHA-256 `c1f5d2789060c3e3e1527ebd08c4a7777fb0ca55f4f46da720c919e518b9ae73`.
- Dolphin 2609a used a private profile and a copy of the configured Wii NAND. The runner used the OpenGL backend and USB Gecko TCP logging. HBC was not under test.
- The DOL ran. Its SHA-256 was `3a9dcf92e146dead0b5d61a9bcb12fe8e84a563ed6f7fb03bdf9461ef814fb00`. The build used revision `3c8ac8237c86f4c57c66e9623821558669862e74` and the uncommitted ISO probe and ELF test changes in this commit.
- The SHA context check and all four ISO 9660 checks passed. ISO used a controlled read-only medium and the production mount API. This does not validate a physical drive.
- The overall probe failed: 17 checks passed and two failed. The PowerPC timebase did not advance between consecutive reads; STM LED mode returned `-4`. These failures need separate investigation.
- The retained [probe log](docs/review/evidence/runtime-iso-20261029/probe.log) and [suite report](docs/review/evidence/runtime-iso-20261029/suite.json) identify the checks and configuration. The full temporary run directory contains screenshots and Dolphin logs.

The `cargo xtask audit-stubs` completeness audit failed with 2,400 API entries marked unverified. The inventory itself was current. These existing repository-wide findings remain open; this run did not suppress them.

## 2026-10-29: Controller model host tests

`cargo test -p ogc-controller` passed 78 unit tests and 12 integration tests.
The new checks exercise button masks, snapshot capacity, player assignment,
and held, pressed, and released button aggregation. The inventory records
18 directly exercised APIs as protocol evidence. These tests exercise the
production model on the host; they do not validate a physical controller or
Wii transport.

## 2026-10-29: Controller transitions and HID host tests

`cargo test -p ogc-controller` passed 80 unit tests and 12 integration tests.
New checks validate empty model values and transition identities across a
disconnect and reconnect. Existing tests exercise raw button transitions and
the production HID parser, decoding, report sizes, and application types.
The evidence registry records 14 directly exercised APIs. These host results
do not establish real controller, USB, Bluetooth, or Wii behavior.

## 2026-10-29: Xbox report and Bluetooth stage host tests

`cargo test -p ogc-controller` passed 82 unit tests and 12 integration tests.
New tests check invalid Xbox report lengths, fixed axis and trigger endpoints,
and the text for each Bluetooth lifecycle stage. The inventory records the
two directly exercised APIs. These host tests do not verify a physical Xbox
controller or Bluetooth transport.

## 2026-10-29: Switch HID and USB event host tests

`cargo test -p ogc-usb-events -p ogc-bt-sync` passed 9 USB event tests
and 103 Bluetooth tests. New fixed-byte checks cover Switch HIDP output, input
headers, and standard reports. The silent stop test checks that pending
callbacks do not survive a restart. Four API entries have host test evidence.
These results do not validate a physical controller or USB/Bluetooth transport.
