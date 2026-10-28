# `ogc-system` Hardware Validation Checklist

Purpose: verify the Rust-side `ogc-system` helpers and current probe binaries against actual runtime behavior. Real Wii hardware is the remaining signoff gate.

Current harness: `cargo xtask build-dols` writes probe binaries under `target/xtask/probes/`.

Hardware validation owner: `codex`

Status: hardware validation pending

## Scope

- SRAM getter parity against locked SRAM snapshots
- SRAM setter behavior with restore-to-original flow
- arena low-allocation behavior for MEM1 and MEM2
- reset callback registration behavior
- power callback throttle behavior
- startup and pre-main observations that can be checked without replacing bootstrap/runtime code

## Required environments

- Real Wii hardware with Homebrew Channel or equivalent loader

## Build path

```sh
cargo xtask build-dols
```

Run `target/xtask/probes/ogc-system-probe.dol` for the aggregate probe, or the focused probe DOLs listed in the README.

## Required evidence

- One real Wii run log
- Notes for any mismatch between emulator and hardware
- Confirmation that original SRAM values were restored after setter tests

Current probe artifact paths:

- Aggregate DOL: `target/xtask/probes/ogc-system-probe.dol` (generated locally)
- Focused DOLs: `target/xtask/probes/*_probe.dol`
- Probe log: beside the launched app on `sd:/` or `usb:/` only if a FAT-backed mounted-device devoptab has been registered first; otherwise `/tmp/ogc-system-probe.log` on the target NAND

## Suggested probe flow

The probe should use the public `ogc-system` Rust APIs only, plus existing libogc callbacks where required.

Current harness note: the probe logic is Rust-owned. Durable logs prefer the mounted app device path only when a real mounted-device filesystem backend exists. The current tree has raw SD/USB block interfaces but no FAT mount layer, so the expected default is the ISFS fallback.
The probe console also pauses briefly before clearing a full page so on-device output remains readable without Dolphin screenshots.

### SRAM getter parity

1. Call `snapshot_sram()` and `snapshot_sram_ex()`.
2. Compare helper-derived values against live getters:
   - `sram_counter_bias` vs `get_counter_bias`
   - `sram_display_offset_h` vs `get_display_offset_h`
   - `sram_eu_rgb60` vs `get_eu_rgb60`
   - `sram_language` vs `get_language`
   - `sram_progressive_scan` vs `get_progressive_scan`
   - `sram_sound_mode` vs `get_sound_mode`
   - `sram_video_mode` vs `get_video_mode`
   - `sram_ex_wireless_id(..., ch)` vs `get_wireless_id(ch)`
   - `sram_ex_gbs_mode` vs `get_gbs_mode`
3. Recompute the checksum with `sram_checksum`.
4. Confirm `refresh_sram_checksum` reproduces the same checksum pair after normalization.

Expected result: all getter comparisons match and checksum math matches the live snapshot.

### SRAM setter behavior

1. Snapshot both SRAM structs before any write.
2. Pick reversible test values that differ from the current values.
3. Run setter calls one field at a time:
   - `set_counter_bias`
   - `set_display_offset_h`
   - `set_eu_rgb60`
   - `set_language`
   - `set_progressive_scan`
   - `set_sound_mode`
   - `set_video_mode`
   - `set_wireless_id`
   - `set_gbs_mode`
4. Read back each field with the live getter and compare against the Rust helper rule for the same write.
5. Restore the original values immediately after each case or at the end of the batch.
6. Confirm the final post-restore snapshot equals the original snapshot.

Expected result: libogc applies the same normalization rules as the Rust helpers, especially for video mode bits and invalid GBS values.

Risk note: this step writes persistent SRAM state. Do not leave altered settings behind.

### Arena allocation behavior

1. Record `get_arena1_lo`, `get_arena1_hi`, `get_arena2_lo`, and `get_arena2_hi`.
2. For a small set of `(size, align)` pairs, compute expected values with `arena_low_alloc_from`.
3. Call `alloc_arena1_mem_lo` and `alloc_arena2_mem_lo`.
4. Compare the returned pointers and new low bounds against the predicted values.
5. Restore the original arena low pointers before exiting the probe.

Expected result: returned pointers and updated low bounds match libogc's alignment math exactly.

### Reset callback behavior

1. Register several reset functions with distinct priorities.
2. Trigger a reset-button event in Dolphin and on real Wii without actually completing a destructive reset if the probe can intercept the callback path early enough.
3. Record callback order and count.
4. Repeat after unregistering one middle-priority callback.

Expected result: callbacks are observed in ascending priority order, and unregistering removes only the targeted record.

Note: if the available environment cannot safely observe callback order without a full reset, record that limitation and keep `sys_reset_system` deeper runtime replacement blocked.

### Power callback throttle behavior

1. Install a counting power callback.
2. Invoke `do_power_cb` twice back-to-back.
3. Wait at least 60 ms.
4. Invoke `do_power_cb` again.
5. Repeat with actual front-panel or emulator power events if available.

Expected result: the first immediate call is accepted, the second rapid call is suppressed, and the post-delay call is accepted.

### Startup and pre-main observations

1. Launch the probe from a clean boot path.
2. Record `time()` shortly after startup.
3. Confirm the boot-time path is initialized and monotonically increasing.
4. On Wii, note whether IOS startup services, STM registration, and Wii init are all available by the time the probe begins interacting with callbacks.

Expected result: observed runtime state matches the modeled `system_init_sequence(true, false, false)` and `system_premain_sequence(true)` ordering assumptions.

## Exit criteria

- Wii logs show matching SRAM getter parity
- Reversible SRAM setter tests restore the original values cleanly
- Arena allocation math matches predicted results on both MEM1 and MEM2
- Power callback throttle behavior is confirmed on Wii
- Reset callback behavior is either confirmed or explicitly blocked with a written reason
- Any mismatch is reflected back into [docs/review/libogc.md](../../docs/review/libogc.md) and [docs/stub-audit.md](../../docs/stub-audit.md)
