# Network GDB exception and scheduling audit

Date: 2026-10-21

## Scope and result

This is a source-only audit of the PowerPC exception entry, the LWP scheduler,
the Wii socket path, and HBC's network worker. It does not change debugger
execution behavior and does not qualify Wii runtime behavior.

**Result: do not connect GDB exception handling to the HBC network worker.**
The current general exception path calls its registered callback synchronously
and restores the same exception frame. It does not invoke the LWP interrupt
return scheduler hook. The callback's GDB loop does not yield. The vector
preserves the interrupted context's EE state, so decrementer preemption may
schedule the worker when EE was enabled at the exception point. It cannot do so
when EE was disabled. Source review does not establish that the worker always
runs or that socket operations are safe in exception context.

The HBC network feature currently follows a separate path: Wiiload's LWP
worker polls `WiiNetworkTransport`; network mode does not call `libdb_sys::debug_init`
and does not install the GDB exception callback. Connection state therefore
does not stop application execution.

## Exception entry and register capture

`crates/abi/ogc-sys/src/lib.rs:500-518` contains the general exception vector
stub. It saves the interrupted stack pointer and SRR0/SRR1 in shared low
memory, then transfers to the common handler. The common handler at lines
521-595 switches to the dedicated 16 KiB exception stack, saves a 0x2c0-byte
PPC context plus the exception ID, and calls
`__ogc_sys_general_exception_dispatch(exid, frame)`.

`__ogc_sys_general_exception_dispatch` loads the registered callback and calls
it directly (`crates/abi/ogc-sys/src/lib.rs:1088-1095`). The callback receives
a writable live exception frame. The shared restore routine restores the
saved registers and returns with `rfi` (`crates/abi/ogc-sys/src/lib.rs:315-364`).
The general exception path branches directly to that restore routine after the
callback (`:589-595`); it does not call
`__ogc_sys_interrupt_return`.

`libdb_sys::debug_exception_handler` is the callback installed by
`debug_init` (`crates/compatibility/libdb-sys/src/lib.rs:679-711,
1372-1388`). It reads or edits the saved frame for PC, MSR, registers, and
breakpoint handling. It then runs a command loop before returning. For example,
the `g` and `m` paths read register or memory state and the `Z`/`z` paths can
insert or remove breakpoints (`:713-766`). Continue and step commands alter the
saved MSR and return from the handler (`:952-970`). These operations are not
part of the currently armed network transport path, but they show that the
legacy handler is an execution-control loop, not a deferred notification.

The stub's transition sets selected SRR1 bits with `ori ..., 0x2030`
(`crates/abi/ogc-sys/src/lib.rs:507-517`). It does not clear MSR.EE in the
saved SRR1 value. The common handler therefore inherits the interrupted
context's EE state, with the other bits in that mask added. If EE was enabled,
decrementer interrupts can potentially preempt the synchronous callback and
the LWP interrupt-return hook can select a ready thread. If EE was disabled,
that route cannot run. The exact behavior, including nested exception and
context restoration safety, requires Wii runtime validation.

## LWP scheduler and handoff limits

LWP initialization registers `decrementer_tick` and `interrupt_return`, then
installs the decrementer vector (`crates/core/ogc-lwp-driver/src/runtime.rs:949-955`).
The decrementer callback processes timeouts (`:1495-1502`). The interrupt
return callback snapshots the interrupted context, checks for a higher-priority
ready thread, and selects the next context (`:1504-1535`). This callback is
reached from the specialized decrementer/IRQ return assembly path, not directly
from the general exception path described above. If EE remains enabled in the
general handler, a later decrementer exception may enter that specialized
path. This is a conditional preemption route, not an explicit handoff made by
the general exception callback.

Normal explicit scheduling calls such as `yield_thread` and `sleep_ticks`
disable interrupts, update scheduler state, and switch contexts
(`crates/core/ogc-lwp-driver/src/runtime.rs:2625-2668`). The PowerPC context
switch saves the current machine state before restoring another thread
(`crates/core/ogc-lwp-driver/src/arch/powerpc.rs:62-145`). Those APIs are
ordinary scheduler operations; they are not proof that calling them from a
general exception callback is safe. The runtime explicitly rejects exception
context for checked thread operations, TLS access, and futex waits
(`runtime.rs:1892-1900, 2106, 3331-3345`).

**Handoff conclusion:** a shared flag alone does not provide a handoff. With EE
enabled, a worker may run through ordinary decrementer preemption, but the
source does not guarantee when it runs, that the worker's locks and network
state are safe, or that the original exception context can resume correctly.
With EE disabled, the ordinary decrementer route is unavailable. The current
general exception path has no explicit scheduler handoff.

## Network worker, socket calls, and locks

HBC creates one worker thread. `worker_main` initializes networking, opens the
Wiiload listener, binds the optional network GDB transport, and polls that
transport in the same loop (`RustPorts/hbc-rs/src/platform/wiiload.rs:259-317`).
It also accepts Wiiload connections and sleeps/yields between nonblocking polls
(`:369-419, 854-862`). Thus the network GDB transport and Wiiload use the same
worker, while their listeners use separate ports and transport state.

The debugger transport sets listener and accepted sockets nonblocking. Its
read and write operations call `ogc_network::recv_slice` and `write_slice`
(`crates/compatibility/libdb-sys/src/wii_network.rs:59-90, 92-164`). Those
wrappers synchronously call the Wii backend (`crates/services/ogc-network/src/lib.rs:685-736`).
On the PowerPC backend, the operations route to the lwIP implementation
(`crates/services/ogc-network-sys/src/lib.rs:1341-1401, 2042-2119`).
Nonblocking mode limits socket wait for data or buffer capacity; it does not
establish exception safety for the full call path.

The socket facade has a global spin lock for socket bookkeeping
(`crates/services/ogc-network/src/lib.rs:285-317`). Most socket operations
acquire it only to translate a public descriptor, then call the backend after
releasing it (`:344-347, 503-528, 596-605`). `close` holds the lock across
`net_close` (`:802-815`), and `soc_exit` holds it while closing sockets
(`:406-425`). If a synchronous exception interrupts a thread while it holds
this lock, and exception code calls a path that needs the lock, the single CPU
can spin forever. The same risk applies to any other lock held by the
interrupted code.

The HBC worker's payload lock is also an atomic spin loop
(`RustPorts/hbc-rs/src/platform/wiiload.rs:70-75`). It protects the shared
payload (`:407-413, 854-858`). A callback must not acquire it unless the
interrupted owner and lock state are known. No source evidence establishes
that all exception points are outside these critical regions.

The legacy TCP GDB interface is a distinct implementation from
`WiiNetworkTransport`. Its `tcpip_accept`, `tcpip_read`, and `tcpip_write`
paths repeatedly poll a private network stack until progress occurs
(`crates/compatibility/libdb-sys/src/lib.rs:1582-1614, 1619-1655, 1665-1685`).
`get_packet` also waits in a loop for a complete packet and checksum
(`:371-409`). These loops run inside `debug_exception_handler` when that
interface is selected. They do not explicitly schedule the HBC network worker;
only the conditional decrementer preemption described above could run it.

## Unsafe or unproven paths

1. **Exception callback to GDB TCP polling:** synchronous callback enters a
   packet loop. It has no LWP yield or scheduler handoff. The worker cannot be
   relied on to run, and the wait has no disconnect timeout in this packet
   loop.
2. **Exception callback to the ordinary Wii socket facade:** socket calls are
   synchronous. Nonblocking socket mode does not protect against a lock held by
   the interrupted thread or prove that backend/runtime services work in this
   context.
3. **Exception callback to worker-owned transport state:** `WiiNetworkTransport`
   is a mutable value owned by `worker_main`. Sharing it with an exception
   callback would violate its current ownership model and would create races
   over socket lifecycle, packet state, and disconnect cleanup.
4. **Worker while callback waits:** the general exception path does not itself
   invoke the scheduler hook. Decrementer preemption is possible only when the
   inherited EE state permits it. Source alone does not show that the worker
   continues for every debugger exception or that callback-side operations are
   safe during preemption.
5. **Disconnect and resume:** the transport handles disconnect during normal
   polling. That does not demonstrate safe disconnect handling during a
   synchronous exception or prove that the saved application context can
   resume.

## Design constraints for the next stage

Before a debugger exception can use network transport, a design must provide
all of these properties:

- Capture the minimum processor context in the exception entry and keep its
  storage alive while the application is stopped.
- Enter a known scheduler-safe service context without calling socket APIs from
  the exception callback.
- Define how the LWP scheduler and required interrupts operate while stopped.
- Keep socket and debugger packet state owned by one normal-context worker, or
  use a separately reviewed synchronization design that cannot deadlock when
  the exception interrupts a lock owner.
- Define disconnect, detach, continue, and Reset behavior, including cleanup
  failures and recovery from a partially completed handoff.
- Prove that return restores the correct saved context and interrupt state.

The existing interrupt-return hook is a possible source of design material,
but the current general exception vector does not use it. Reusing that hook
requires a separate design and controlled runtime validation; it is not a
safe path established by this audit.

## Open items for Phase 4B.3

Source review cannot establish these runtime facts:

- Whether the network worker runs during any proposed debug-stop mechanism.
- Whether IOS/lwIP calls can complete under the exact interrupt state used by
  that mechanism.
- Whether an exception can interrupt socket or payload lock ownership in a
  way that causes deadlock.
- Whether disconnect, detach, continue, and Reset recover the original
  application state.
- Whether the target's actual exception vector state matches the source-level
  interrupt analysis.

Until those points pass a controlled Wii runtime test with a qualified recovery
procedure, keep network exception handlers and GDB execution control disabled.

## Review evidence and repository state

The audit reviewed the source paths cited above in both linked repositories.
Both repositories were on `feature/network-gdb-debugging`. Existing staged,
unstaged, and untracked work remains preserved. The verified backup remains at
`/Volumes/SSD/WiiDev/tmp/network-gdb-state`. This document is the only file
added by this audit.
