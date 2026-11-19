# LWP external waits

The probe starts one worker at a priority above the main thread, yields so it
can block, and tests five wake or cancellation paths. A one-second decrementer
watchdog broadcasts the key if a wake does not complete. A passing check
requires the watchdog to remain unused and the worker to return the expected
value.

The first run in `../2026-11-19-lwp-external-waits/` attempted the wake before
explicitly yielding to the worker. It failed all five new checks and did not
record the worker state. The final run adds the yield and records the worker,
timer, submission, result, and watchdog values. This covers one waiter at a
time, not multi-waiter selection or a pending-wake race.
