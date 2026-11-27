# Live GX command processor interrupts — Linux Dolphin 2609b

C reference: `7d4b8b3e7b845516f0cc2a85274879ddd8cbf142`.
Rust baseline: `e0ffcfbc7d5a024fd0e720c606309fcb2f8eed4e`.
Stock graphical Dolphin 2609b, OpenGL, private stock-menu NAND. No
Dolphin source change was used for these tests.

## Investigation

The C GX handler clears spurious pending status, handles underflow before
overflow, and then handles an enabled breakpoint (`libogc/gx.c:386-425`).
The Rust handler has the same sequence. In the C scheduler,
`KThreadSuspend` calls `KThreadSwitchTo` for the running thread;
`KThreadSwitchTo` checks `PPCIsInExcpt` and defers a switch to exception
return (`tuxedo/thread.c:84-149`). Rust instead called the cooperative
`switch_locked` directly, even during the external IRQ handler on the
shared exception stack (`crates/abi/ogc-sys/src/lib.rs`,
`__ogc_irq_common_handler`).

Two combined breakpoint/watermark experiments timed out after their start
markers. Both debugger reports recorded PC `0x42000890`, outside the
loaded program. The isolated breakpoint experiment completed 41/41 checks:
callback count one, CP status `0x1a`, CP control `0x17` (breakpoint
interrupt enable cleared), and a subsequent PE finish. This established
that Dolphin can deliver the breakpoint IRQ without a scheduler change.

The correction marks execution of the external IRQ dispatcher and makes
LWP self-suspension defer its switch to the existing interrupt-return
scheduler. Normal thread-context suspension still uses the cooperative
switch. This is scoped to the external IRQ dispatcher, not a claim that
all exception-context blocking APIs are safe.

## Test method and results

The combined probe uses a separate 64 KiB FIFO, 64-byte high and 32-byte
low watermarks, a real producer LWP, and a breakpoint at the FIFO start.
The producer emits at most 16 32-byte NOP flushes. The control thread
checks suspension, one overflow, the breakpoint callback, and the low-only
interrupt mask. It removes the breakpoint to let the GPU retire work;
the producer must resume without manual intervention, join, and permit
PE completion. The probe then restores the original FIFO. Manual resume
is failure cleanup only and does not satisfy the recovery check.

The corrected probe completes 45/45 checks, with five valid capture pages.
During suspension, CP control is `0x1b` and CP status is `0x19`; after
recovery, CP control is `0x15`. A separate baseline run uses the same
combined probe and IRQ-context flag, but removes only the deferred
suspension branch. It times out after the pressure-test start marker.
The debugger reports PC `0x7ff67c7c`, outside the loaded program. The
branch is restored for the final verification suite.

The stock Dolphin command processor computes watermark status from
FIFO occupancy; `SetCpClearRegister` is intentionally a no-op. This
experiment verifies delivered interrupts and the guest scheduling path,
not hardware sticky-status clearing or exact interrupt timing. Physical
Wii verification and broader exception-context scheduling review remain
pending. GX abort-frame Wii-specific initialization and recovery remain
open. See `summary.json` for individual run outcomes and DOL hashes.

Final regression suite: GX 45/45, LWP 282/282, and bus 78/78,
all passing with 35 valid captured pages. The repeated GX DOL has the same
SHA-256 as the first corrected run. Host `ogc-sys` tests pass 10/10 and
xtask tests pass 164/164. The 3,419-entry implementation inventory is current.
The raw reports, logs, DOLs and captures are kept in the private evidence
archive identified by the SHA-256 in `summary.json`; no NAND data is included.
