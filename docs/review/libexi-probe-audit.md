# Independent EXI insertion debounce audit

Reviewer: `/root/reviewer_6`  
Implementer: `/root/di_review`; relevant fix commit `b9f5d7b` (`BUG FIX: align EXI probe and select behavior`).  
Scope: only device-zero insertion probe timing after the fix; other EXI behavior is outside this audit.  
Result: the missing insertion debounce finding is confirmed.

## C behavior

C keeps a per-channel `last_exi_idtime` timestamp (`../../C-sources/libogc/libogc/exi.c:87`). In `__exi_probe`, when the external-device bit is first observed, C records `SYS_Time()` (`:182-185`). It returns absent while `diff_usec(last_exi_idtime, time) + 10 < 30`, which means a positive probe is withheld until at least 20 microseconds have elapsed after first detection (`:183-191`). An external interrupt clears the timestamp before checking presence (`:177-181`); losing the external-device bit also clears it (`:192-196`). The public `EXI_Probe` calls `__exi_probe` and only then reads the device ID if the probe is positive (`:696-709`). `EXI_ProbeReset` clears timestamps before probing channels 0 and 1 (`:719-730`).

## Rust behavior after `b9f5d7b`

Rust's `device_zero_probe_locked` reads the channel register, clears/handles a pending external interrupt, and otherwise accepts the device immediately when `EXI_EXT_BIT` is set and `EXI_EXT_IRQ` is clear (`crates/hardware/ogc-exi-sys/src/lib.rs:651-682`). `get_id` calls that helper before returning a cached ID or starting an ID transaction (`:1003-1014`), and `probe` delegates directly to `get_id` (`:1124-1135`). The newly added `id_cache` stores a completed ID and is cleared on external IRQ/reset, but it has no timestamp field or elapsed-time check (`:66-89`, `:492-510`, `:660-682`). Consequently, on a just-inserted device with the presence bit set and no pending IRQ, Rust can begin and complete the ID read and return present on the first probe. C returns absent during the first 20 microseconds after its first observed presence bit.

The fix commit adds an EXI register-base selection, presence/IRQ checks, cache reset, and ID caching; it does not add the C timestamp or debounce threshold. The regression tests check absent and pending-IRQ register states, but do not exercise elapsed insertion time (`lib.rs:1308-1313`).

## Conclusion

The specific C insertion debounce remains missing after `b9f5d7b`. Keep `ogc-exi` and `ogc-exi-sys` open with a finding for this observable insertion-window difference. No implementation changes, builds, tests, or hardware verification were performed.

## Current-source re-review: debounce added in `00a3df5`

**Result: PASSED; the prior missing-debounce conclusion is stale for current source.** Reviewer: `/root/reviewer_f`. Commit `00a3df5` adds the elapsed-time gate after the earlier `b9f5d7b` audit.

C waits 20 microseconds after first observing `EXI_EXT_BIT` (`C-sources/libogc/libogc/exi.c:177–196`). Rust `device_zero_probe_locked` now resets the timer on pending external IRQ or absence, bypasses the wait for attached devices, and reports present only after `probe_debounce_elapsed` succeeds (`crates/hardware/ogc-exi-sys/src/lib.rs:743–776`). The threshold is 810 ticks at 40.5 MHz for GameCube and 1,215 ticks at 60.75 MHz for Wii, matching 20 microseconds; elapsed time uses wrapping subtraction (`lib.rs:705–711`). External IRQ and probe-reset paths also reset the timer.

Regression assertions cover the threshold and tick rollover (`lib.rs:1414–1422`). Reviewer_f ran no checks or tests and found no validation record specific to `00a3df5`; no hardware verification was performed. The broader EXI fixes remain in their previously recorded reviewed state.
