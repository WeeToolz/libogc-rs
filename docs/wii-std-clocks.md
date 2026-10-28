# Wii std clocks and deadlines

Updated: 2026-10-14. Source adapters and probe builds are complete. Hardware
acceptance is open. See [the checklist](wii-std-checklist.md) and
[clock evidence](wii-std-clock-evidence.txt).

## Findings and source policy

The old Wii pre-main path read the RTC, silently substituted zero for a failed
counter-bias read, and wrote the resulting calendar ticks into the hardware
timebase. LWP already used that timebase for active deadlines. A later pre-main
call could therefore move both monotonic readings and deadlines.

Wii now keeps the hardware timebase stable. System initialization records one
monotonic origin after system setup. Pre-main captures wall-clock data in a
separate application-memory anchor. It does not change the Wii RTC, SRAM,
SYSCONF, hardware timebase, or loader boot-time field. The existing GameCube
compatibility path remains separate from Wii std.

The RTC has a 32-bit seconds counter. Console counter bias supplies local
calendar time relative to 2000-01-01. SYSCONF describes `IPL.CB` as a difference
between RTC and local time; it does not supply the local UTC offset.
[RTC hardware notes](https://www.wiibrew.org/wiki/Hardware/MX23L4005) and
[SYSCONF settings](https://wiibrew.org/wiki//shared2/sys/SYSCONF) record these facts.
The reference-only C `system.c::__SYS_SetBootTime` uses 32-bit bias addition;
`timesupp.c::__libogc_gettod_r` adds 946,684,800 seconds for the Unix epoch.
The Rust provider calls no C implementation.

A local calendar cannot be treated as UTC without an offset. This implementation
uses an explicit application setting: seconds east of UTC for the console's
calendar. It does not infer an offset from the development Mac, country, language,
or a guessed time zone. No network time service is accepted yet. An application
must update the setting if its local offset changes, including a daylight-saving
change. This work does not add a time-zone database.

## Monotonic time (C01)

`ogc-system-sys` owns `ClockState`. A bounded LWP interrupt mask protects its
in-memory state on the single Wii CPU. The production upper/lower/upper timebase
read gives a stable sample. `monotonic_time` subtracts the recorded origin and
converts quotient and remainder at 60,750,000 ticks per second. Division comes
before nanosecond multiplication, so a full `u64` counter does not overflow.

Repeated system initialization or wall refresh keeps the origin. A backward
hardware counter change returns `TimebaseReversed` and permanently invalidates
the application clock. The implementation does not clamp the failure to a
successful old reading. Direct external timebase writes are unsupported;
forward jumps cannot be distinguished from elapsed time. The runtime does not
make those writes on Wii. Full 64-bit counter rollover is outside the supported
application lifetime and is reported as a backward change.

The std `Instant` adapter uses the real fallible provider. It has checked
addition, subtraction, and duration differences. It does not require RTC,
console configuration, or UTC offset data.

## Wall clock and validity (C02–C03)

The EXI transaction engine checks lock, select, command transfer, command
completion, counter transfer, counter completion, deselect, and unlock. It
preserves the failed operation plus deselect/unlock failure bits. It requires
two matching counter reads, with at most 16 pairs. No failing transfer publishes
a valid sample. The command buffer remains live through completion.

After console configuration is initialized, `synchronize_clock` reads the real
RTC and `IPL.CB`. Failed bias reads retain their original code. They do not use
zero bias. The completed sample stores local calendar seconds and its raw
counter anchor. Bias addition follows the console's 32-bit calendar-counter
representation. UTC seconds are local seconds plus 946,684,800 minus the explicit
UTC offset. Real elapsed ticks extend that sample between refreshes.

RTC resolution is one second. The captured subsecond phase is unknown, so wall
time has up to one second of sampling uncertainty. Interpolation uses real
elapsed ticks; it does not claim nanosecond RTC accuracy. A successful refresh
can change wall time in either direction and leaves monotonic time unchanged.
Calendar-counter rollover has the console's 32-bit behavior. The service does
not certify the console's date against an external reference.

`clock_snapshot` exposes initialization, monotonic failure, wall validity,
last successful RTC/bias values, and the explicit offset. Last sample fields can
remain for diagnosis after a failed refresh; `wall_error` then marks them invalid.
A completed RTC or bias failure invalidates the current wall anchor. A valid
refresh can restore it. A rejected context or busy refresh returns its error
before bus access. Pre-main's legacy void path records errors in the snapshot.

Wall-clock use requires checked system startup, initialized console configuration
(the existing pre-main sequence performs this), a real RTC/bias sample, and the
explicit UTC offset. Applications can prepare it with:

```rust
fn prepare_wall_clock(offset_seconds_east_of_utc: i32)
    -> Result<(), ogc_system::ClockError>
{
    // Call after console configuration has initialized successfully.
    ogc_system::set_clock_utc_offset(offset_seconds_east_of_utc)?;
    ogc_system::synchronize_clock()?;
    ogc_system::wall_time()?;
    Ok(())
}
```

Offsets outside -86,399 through 86,399 seconds are rejected without changing the
setting. This setting and clock refresh affect application memory only.

The internal clock API is fallible. The std ABI validates non-null, aligned,
non-overlapping output slots and leaves outputs unchanged on failure. The std
infallible `Instant::now` and `SystemTime::now` stop with the returned OS error
under the target's panic-abort policy if their clock is unavailable. They do not
return zero or a substitute date. Detailed failures remain in the system snapshot.
Full panic-output diagnostics remain a separate open platform contract.

`SystemTime` supports checked arithmetic on both sides of `UNIX_EPOCH`. Its
range is bounded so every difference fits `Duration`; extreme additions and
subtractions return `None`. The legacy `sys_time` remains boot-adjusted tick
arithmetic. It is not a UTC API and is not used by the std clock adapter.

## Existing timed operations (C04)

Sleep and futex waits retain the shared production `TickBudget`: round up to
hardware ticks, keep large durations in `u128`, use bounded signed-half-range
deadlines, and reduce the remaining budget only by actual elapsed ticks. Zero
and infinite waits remain distinct. The stable Wii counter now protects those
deadlines from pre-main calendar changes.

The pinned std condition-variable algorithms can now use real `Instant`
readings to retain an absolute deadline across notifications and retries.
Parking keeps its upstream token and timeout rules. The clock probe measures
sleep and condition timeout bounds with the raw counter and compares a longer
wait with actual RTC seconds.

Std socket adapters and their millisecond/timeval deadline handling remain open
under N05. This clock work does not claim completed std socket deadlines.

## Probe and next work

`cargo xtask build-wii-std-probe` now builds three std DOLs. The clock artifact is
`target/xtask/probes/wii_std_clock_probe.dol`. It has a normal Rust main and holds
the visible result after return. `WII_STD_CLOCK_PROBE_RESULT` is zero only when
all acceptance checks pass.

Supply `--utc-offset-seconds=<seconds east of UTC>` for the test Wii. If the
argument is absent or invalid, wall-clock acceptance is reported as not run and
the overall probe fails. A skipped wall test is not a pass. Do not supply zero
unless the console's local calendar is actually set to UTC.

No probe has run on Wii. Record the model, revision, offset source, current
console date, independent UTC reference, toolchain, probe commit, DOL hash,
output, and measured bounds before hardware acceptance. Real RTC/EXI failures
also need controlled hardware validation. Next source work is the audit of
unsupported platform selectors, followed by network/DNS and the remaining
file, stream, path, exit, and entropy contracts in the checklist.
