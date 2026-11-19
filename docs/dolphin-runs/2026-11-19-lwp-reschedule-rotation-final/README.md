# Equal-priority reschedule regression

Two ready workers at the same priority each record a marker, call a
reschedule entry point, and record a second marker. The main thread blocks
until both workers finish. Either alternating order is accepted.

Before the change, both entry points produced `0x2211`: the first worker
ran twice before the second worker. The failed run is retained in
`../2026-11-19-lwp-reschedule-rotation/`. The scheduler advanced the
priority cursor in `reschedule` and again during selection. The change
leaves the cursor immediately before the chosen thread, so selection
advances to that thread once. The final Dolphin run produced `0x2121` for
both entry points and passed the other LWP probe checks.

This checks two equal-priority ready threads. It does not establish physical
Wii timing or all priority combinations. The local C compatibility source
marks `LWP_Reschedule` as not implementable; this is a Rust scheduler
behavior, not a comparison with a C implementation.
