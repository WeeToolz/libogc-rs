# Parity batch — 2026-10-02 (DVD, keyboard, LWP, IPC, umbrella)

Source of truth: uploaded `libogc-master.zip`. Rust baseline: uploaded `libogc-rs-12.24-8-1.zip`.

## Verified as already fixed in the uploaded Rust baseline

- CONF save preserves the `SCv0` signature; the prior Rust-only checksum write is absent.
- ISFS async `read_dir` copies `*num` only when the IOS completion result is exactly `0`.
- `lwbt-sys` HID `sendmessage`/`sendmessageasync` use control-channel SET_REPORT and complete on the HID handshake instead of sending through the interrupt data path.

No duplicate implementation changes were made for those three items.

## Implemented in this batch

- DVD StopStreamAtEnd audio-stream command and C-stage automatic cache invalidation.
- Wii keyboard F-key keysyms and dynamically growing FIFO event buffering; prior polling/lock-key behavior rechecked.
- LWP paired-single context preservation and C initial stack back-chain sentinel.
- C-style 4096-byte default IPC heap, 16 public heaps, runtime low/high bounds from the Wii IPC linker arena, and centralized heap allocation state. Existing ISFS/USB callers were adjusted so they do not consume a new heap slot on every operation.
- `gccore.h` compatibility surface: memory-card export, cast helpers, color constants, and Wii/GameCube export gating.

## Validation boundary

`git diff --check` and source-level assertions were used locally. The environment has no Rust compiler, so the downstream compile-capable agent must run the full `-D warnings` build. DVD/keyboard/LWP/IPC behavior still needs hardware validation.


## User compile status

The user reports that the DVD/keyboard/LWP/IPC/umbrella batch and the subsequent follow-on patch compile cleanly in the full repository build. This note records that external validation; hardware behavior remains separately unverified.
