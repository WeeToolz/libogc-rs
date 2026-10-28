# Milestone 1 LWP runtime

## Status

The PowerPC implementation is present and the source-level structural checks are complete. A user-run `cargo xtask milestone1-verify` completed target compilation and produced `target/xtask/probes/lwp_probe.dol` on 2026-07-11. Real-Wii acceptance remains pending.

Hardware acceptance requires running `target/xtask/probes/lwp_probe.dol` on a real Wii and retaining its complete log.

## Ownership and initialization

`ogc-lwp-driver` is the sole scheduler owner. It lazily initializes on the first LWP operation and creates:

- the current application thread;
- one idle/deadline thread;
- one high-priority deferred-callback worker.

`ogc-lwp-sys` translates the historical handle API. `ogc-lwp` contains safe handle wrappers. `tuxedo-sys` maps its existing ABI objects to LWP handles and does not own another current-thread pointer, ready list, context-switch routine, timer queue, or callback worker.

Initialization is idempotent. The scheduler is globally exclusive and has no shutdown operation because the application thread, idle thread, and IOS interrupt integration remain process-lifetime resources.

## PowerPC context

The context structure is 32-byte aligned and `0x2c0` bytes, matching the established PowerPC/Tuxedo field ordering:

| Offset | State |
| --- | --- |
| `0x000` | PC, restored through SRR0 |
| `0x004` | MSR, restored through SRR1 |
| `0x008` | CR |
| `0x00c` | LR |
| `0x010` | CTR |
| `0x014` | XER |
| `0x018` | GPR0–GPR31 |
| `0x098` | FPSCR |
| `0x0a0` | FPR0–FPR31 |
| `0x1a0` | GQR0–GQR7 |
| `0x1c0` | reserved paired-single shadow area |

The assembly switch saves the application stack, small-data registers, callee state, floating-point state, and GQRs, clears the processor load-reserve state so `lwarx`/`stwcx.` cannot span threads, then restores the selected context with `rfi`. Floating point is enabled for scheduler-created contexts because the driver saves FPR/FPSCR eagerly rather than relying on a separate lazy-FPU owner.

The paired-single shadow area is reserved for ABI/layout compatibility but is not yet claimed as independently saved. Code that depends on paired-single secondary-lane preservation must be included in the hardware follow-up before that behavior is declared complete.

## Stacks

Caller-provided stack bases must be at least four-byte aligned and at least 1 KiB. The initial SP is rounded down to 32-byte alignment. When no stack is supplied, the driver uses a fixed 32 KiB slot and honors the requested size up to that capacity; a zero size selects 8 KiB.

A guard word is installed at the low end of every stack whose base is known. It is checked at scheduler switches. Failures increment `stack_guard_failures` and emit debug event 10. The Tuxedo prepare ABI supplies only a stack-top pointer, so its adapter cannot install a low-end guard without an ABI-visible stack size.

## Scheduling

LWP priorities range from 0 through 127, with 127 highest. Runnable threads are selected by effective priority. Equal-priority threads rotate at explicit scheduling points.

- Yield, reschedule, blocking waits, sleep, timed waits, and thread exit select another runnable thread.
- External IRQ and decrementer handlers publish newly ready threads. The shared interrupt-return hook saves the interrupted context and selects a strictly higher-priority ready thread before `rfi`.
- Equal- and lower-priority wakeups keep the interrupted thread running, matching C `KThreadReschedule`.
- There is no periodic quantum for equal-priority threads. Long-running equal- or lower-priority work requires a scheduling point before it can run.

Source comparison covers this decision and the context handoff. Wii and GameCube runtime verification remains required for interrupt timing and complete register restoration.

## Thread lifecycle

Implemented operations include creation, current-thread lookup, suspend, resume, priority changes, yield, sleep, exit, and join. A joined thread slot is returned to the fixed pool after its exit value is collected. Joining the current thread, an internal scheduler thread, an unused thread, or a thread that already has another joiner returns `LWP_CLOSED`.

The fixed capacity is 32 thread slots, including the application, idle, and deferred threads. Resource exhaustion returns a compatibility error; it never reports success without a slot.

## Synchronization

### Queues

There are 64 LWP queue objects. Queue signals use a bounded permit count so an IOS completion arriving immediately before `thread_sleep` is not lost. Close wakes blocked threads as cancelled.

### Mutexes

There are 64 mutex objects. Normal and recursive mutexes are supported. Contended locks block in the scheduler. The owner inherits the highest effective priority of its waiters; releasing a mutex recomputes inherited priority across all mutexes owned by that thread.

Milestone 1 implements basic priority inheritance, not a formally verified transitive inheritance graph. The probe covers ordinary contention and condition-variable interaction; nested inversion stress remains useful follow-up hardware coverage.

### Condition variables

There are 64 condition variables. Waiting validates ownership, records recursion depth, releases the mutex while interrupts are masked, enters the condition wait queue, and reacquires the mutex before returning. Relative timed waits use the PowerPC timebase. The driver returns `-116` on timeout; `ogc-lwp-sys::cond_timed_wait` maps it to positive Newlib `ETIMEDOUT` (`116`).

### Atomic address waits for Rust std

The driver also supplies futex wait and wake operations over live 32-bit atomic
words. It uses each thread's existing wait record and scheduler deadline, with
no separate lock or condition handle. The pinned Rust std mutex, condition,
reader/writer lock, and once algorithms use this transport. These mutexes do
not provide the compatibility mutexes' priority inheritance. See
[the synchronization design and verification limits](wii-std-sync.md).

### Semaphores

There are 64 semaphores. Waiters block rather than spin. A post transfers the wake directly to the highest-priority waiter; otherwise it increments the bounded count.

### Message queues

There are 16 queues with up to 16 messages each. Blocking send, jam, receive, nonblocking operations, close cancellation, and direct wakeups are implemented with fixed storage.

## Timers

`ogc-core::powerpc::read_timebase` is the shared monotonic primitive. LWP uses `60,750,000` ticks per second for Wii time conversion.

The deadline service contains 64 fixed timer records and supports one-shot and periodic callbacks. The PowerPC decrementer is programmed for the next thread or timer deadline. Its handler wakes timed waiters and runs due timer callbacks with interrupts disabled, matching Tuxedo's tick-task context. Timer callbacks must not block. A timed waiter with strictly higher effective priority can run on interrupt return. Equal- or lower-priority waiters run at a later scheduling point.

`LwpClock`, `LwpTimerService`, and `LwpInterruptWake` implement the contracts in `ogc-runtime`.

## Deferred callbacks

The application deferred queue contains 64 fixed entries. A separate 32-entry interrupt-reserved queue prevents application work from consuming the capacity required by IOS completion callbacks. Producers can enqueue while interrupts are masked. The dedicated worker drains interrupt completions first and executes callbacks with interrupts enabled and outside IOS or hardware interrupt context.

Neither queue allocates. Overflow returns `LWP_CLOSED`, increments `dropped_deferred_callbacks`, and does not pretend the callback was accepted. `ogc-ipc` additionally caps outstanding asynchronous callbacks at 16, below the reserved queue capacity, so an accepted async request has a bounded completion slot.

## IPC wakeups

Synchronous `ogc-ipc` requests allocate an LWP queue before handing the request to IOS. The caller blocks on that queue. The IPC reply or relaunch-ack handler:

1. stores the request result;
2. marks the synchronous ownership fallback complete;
3. records `note_ipc_wake`;
4. signals the LWP queue.

The queue permit closes the reply-before-sleep race. If the scheduler rejects a wait after IOS already owns the request, the code uses the existing ownership flag and yields while polling rather than returning fake success.

Synchronous IPC serialization uses an LWP mutex. A second caller blocks in the scheduler instead of spinning, and basic priority inheritance raises a lower-priority owner while it is asleep awaiting the IOS reply. Releasing the serialization mutex introduces an immediate scheduling point for a higher-priority waiter.

Asynchronous IOS replies enqueue their callback and request-release work onto the interrupt-reserved deferred queue. User callback code therefore runs on the LWP deferred worker rather than in the PI/ACR interrupt handler.

## Tuxedo compatibility

Tuxedo priority 0 is highest and `0x7f` lowest. The adapter maps it to LWP with:

```text
lwp_priority = 0x7f - tuxedo_priority
```

Thread prepare creates a suspended LWP thread. Resume, suspend, priority, join, yield, sleep, and exit delegate to LWP. Mutex and condition operations use LWP objects. Tuxedo wait queues use external LWP waits. Wait metadata publication and entry into the LWP block operation occur in one interrupt critical section. A wake token arriving after wait intent is published but before the thread actually blocks is latched and consumed by the subsequent block call.

Tick tasks use the shared LWP decrementer service. Their callbacks run in decrementer exception context.

The existing Rust `tuxedo-sys` public layouts were retained to avoid an unrelated ABI migration. Exact comparison against the selected C Tuxedo headers remains part of the project-wide ABI report.

## Debug facilities

`DebugCounters` records:

- context switches;
- threads created and exited;
- blocking waits and wakeups;
- timeouts;
- deferred callbacks;
- IPC interrupt wakeups;
- stack guard failures;
- dropped trace events and deferred callbacks.

A 256-entry event ring records initialization, create, switch, block, wake, timeout, exit, deferred execution, IPC wake, and stack guard failure events.

The optional hook may run with interrupts masked. It must not allocate, block, perform IOS calls, or acquire application locks. The probe hook performs bounded atomic stores only.

## Probe procedure

Build:

```sh
cargo xtask milestone1-verify
# or rebuild only the focused DOL after a successful core check:
cargo xtask build-lwp-probe
```

The DOL is:

```text
target/xtask/probes/lwp_probe.dol
```

Run on a real Wii and retain:

- the full visible-console result;
- `/tmp/ogc-system-probe.log` when available;
- any mounted log produced by the aggregate probe logger;
- final counters;
- the first failure, if any;
- Wii model, IOS environment, loader, and repeat count.

A successful run must show no stack-guard failure, successful joins and return values, a real IPC wakeup delta, two differently prioritized synchronous IPC callers completing through scheduler-aware serialization, an asynchronous IPC callback running on the deferred worker, condition timeout, deferred callback completion, and Tuxedo queue wake propagation.


### Boot-stage diagnostics

The probe initializes VI before IOS pre-main work and displays three fixed boot markers:

- `BOOT 1/3: system and video initialized`;
- `BOOT 2/3: entering IOS pre-main`;
- `BOOT 3/3: IOS pre-main completed`.

Framebuffers are flushed through their cached K0 addresses and converted to uncached K1 aliases before VI and console use. This prevents dirty CPU cache lines or invisible cached writes from leaving the external framebuffer black. A panic after console initialization displays `FATAL: probe panic` and keeps VI active.

The exported `OGC_PROBE_BOOT_STAGE` symbol contains 1, 2, or 3 for the corresponding visible marker and `0xffffffff` in the panic path. The LWP probe also exports `lwp_probe_debug_hook_count` and `lwp_probe_debug_last_event`; the scheduler hook is installed before IOS pre-main begins.

If execution stops at `BOOT 2/3`, collect IPC debug state because the stall occurred during `sys_pre_main`. If all three markers appear but no probe records follow, inspect log creation and the first LWP operation. A completely black screen after this revision indicates failure before or during VI initialization rather than an invisible cached console buffer.

## Known limitations pending hardware feedback

- no involuntary time slicing;
- paired-single secondary-lane preservation is not yet validated;
- fixed object capacities rather than dynamic allocation;
- only one joiner per thread;
- priority inheritance has not yet passed nested inversion stress;
- context assembly and interrupt wake behavior require real-Wii validation;
- no scheduler teardown, thread detach API, or cancellation points beyond compatibility operations currently exposed.
