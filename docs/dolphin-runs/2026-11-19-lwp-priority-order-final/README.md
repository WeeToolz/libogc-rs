# LWP priority order probe

Both cases start two ready workers below the main thread. Each priority change
reverses the workers' initial priority order. The probe blocks the main thread,
then checks which worker ran first and joins both workers.

The earlier run in `../2026-11-19-lwp-priority-order/` did not reverse the
initial order in the ABI case. It is retained as an intermediate run, not as
evidence for that case. The final run records both checks as passing in the
probe log. Its result-page screenshot was captured as black.
