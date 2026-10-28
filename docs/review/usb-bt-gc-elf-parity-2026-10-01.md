# USB Gecko, USB HID, Bluetooth L2CAP, and gc-elf parity pass

Date: 2026-10-01

This pass rechecked four concrete areas against the current C sources while keeping fixes localized and shared behavior centralized.

- **USB Gecko flash programming:** no new source change was required. The synchronized Rust source already uses one shared byte-program helper that emits the four C commands (`AA@0x5555`, `55@0x2AAA`, `A0@0x5555`, target/value), followed by the previously aligned 20 microsecond delay and readback verification.
- **USB v5 HID open:** `open_device` now uses one v5 preparation helper. Vendor devices use the C best-effort resume behavior. HID devices additionally perform C's descriptor read/free preflight, and a descriptor failure suspends the device before returning the error.
- **Bluetooth L2CAP peer disconnect:** no new source change was required. The synchronized source already queues the response before changing channel state, writes destination/local CID before source/remote CID, preserves the ACL link, closes only the active local channel, and rejects reserved-but-inactive CIDs as invalid. Historical review text is supplemented rather than deleted.
- **`gc-elf` legacy BSS:** the legacy profile now reproduces C `add_bss` exactly: preserve program-header order, establish the first BSS range, and extend only on exact end-to-start adjacency. Later disjoint, overlapping, or out-of-order ranges are ignored. The Rust-only `Libogc` profile is unchanged.

The code changes are intentionally small: one shared USB v5 open helper and one legacy BSS accumulator. This avoids duplicating platform or protocol policy across call sites. Hardware verification is still required for the USB paths. `gc-elf` is host-side and can be validated with its unit tests.
