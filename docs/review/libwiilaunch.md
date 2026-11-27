# Wii launch parity review

Status: static source review complete for both claimed crates. Host tests, strict Clippy, Dolphin rejection checks, and four successful test-title handoffs passed on 2026-11-27. No physical Wii run was done.

## Scope and source map

Reviewed all production Rust source in:

- `crates/core/ogc-wiilaunch/src/lib.rs` (lines 1–519; target-specific API tests at 520–562).
- `crates/core/ogc-wiilaunch-state/src/lib.rs` (lines 1–259; host protocol tests at 260–621).

The complete C implementation is `../../C-sources/libogc/libogc/wiilaunch.c` (lines 29–409). Public declarations, legacy error codes, and settings paths are in `../../C-sources/libogc/gc/ogc/wiilaunch.h` (lines 29–79). The C implementation is gated by `HW_RVL`; Wii hardware verification is outside this source review.

## Confirmed mismatches

No unintentional parity mismatch was confirmed in the launch structures, checksum calculation, argument buffer layout, title selection, or ordering of successful launch-state commits. The differences below change behavior and are recorded as Rust additions or stricter error handling.

## Error handling and observable differences

- **Initialization:** C `WII_Initialize` reads `state.dat` and `NANDBOOTINFO`, ignores both read results, marks the subsystem initialized, then returns 0 (`wiilaunch.c:196–202`). Rust `initialize` also defaults each failed read independently, marks initialized, and returns 0 (`ogc-wiilaunch/src/lib.rs:261–271`). Rust additionally offers `try_initialize`, which stops on the first read error and leaves `initialized` unchanged (`lib.rs:273–279`). This is a Rust-only fallible API.
- **Read validation:** C checks open, exact byte count, and checksum. It clears the corresponding global structure when any check fails (`wiilaunch.c:108–154`). Rust reads into a default temporary, checks exact byte count and checksum, then `initialize` substitutes a default value on error (`ogc-wiilaunch/src/lib.rs:209–225, 267–269`). Both close after a successful open; Rust ignores close errors here, as C does.
- **Persistence:** C sets the checksum, opens read/write, writes the full struct, closes, and checks the write length. It ignores close errors and does not read back the file (`wiilaunch.c:156–194`). Rust sets the checksum, checks exact write length and close status, then reopens and reads the entire structure and verifies both checksum and byte equality (`ogc-wiilaunch-state/src/lib.rs:199–247`). Rust therefore can stop a launch on close/readback failure where C proceeds after an ignored close failure, and Rust detects storage corruption that C does not check. The successful operation order remains state flags (when requested), NAND boot info, then launch (`state/src/lib.rs:249–258`).
- **Launch failures:** C ignores write failures in the public launch paths, then continues (`wiilaunch.c:268–287, 323–337, 340–365`). Rust propagates persistence errors and does not call ES launch after a failed persistence step (`ogc-wiilaunch/src/lib.rs:326–369`, `state/src/lib.rs:249–258`). This is a stricter Rust behavior, not a C parity claim.
- **Argument errors:** C's variadic `WII_LaunchTitleWithArgs` ignores the result of `__WII_SetArgs`, `__WII_WriteStateFlags`, and `__WII_WriteNANDBootInfo`, and only resets `args_set` after a negative launch result (`wiilaunch.c:290–338`). Rust accepts a slice instead of a variadic null-terminated list, returns `ArgsTooLarge` for serialization overflow, and propagates persistence failures (`ogc-wiilaunch/src/lib.rs:380–429`). This avoids C's ignored-error path and is a deliberate API/behavior difference.
- **Ticket view count:** Both implementations reject more than four views with `WII_EINTERNAL` and propagate negative ES errors (`wiilaunch.c:246–264`; `ogc-wiilaunch/src/lib.rs:290–300`). Rust stores four default-initialized aligned views; C allocates four aligned stack entries. Both pass the resulting buffer to ES launch.
- **URL lookup:** C maps `ES_GetNumTitles` and `ES_GetTitles` failures to `WII_EINTERNAL`, searches for title IDs where `(tid & ~0xff) == 0x0001000148414400`, and maps missing title or TMD-size failure to `WII_EINSTALL` (`wiilaunch.c:368–407`). Rust uses the same mask/base and error mapping (`ogc-wiilaunch/src/lib.rs:489–518`). Rust adds checked allocation-size arithmetic and uses an aligned `Vec`; C uses `memalign(32, titlecount * sizeof(u64) + 32)` without checking allocation failure (`wiilaunch.c:370–390`).

## State and layout comparison

- `NANDBootInfo`: C fields are checksum, argsoff, four bytes, launchcode, two `u32`s, launcher `u64`, and 0x1000-byte argument buffer (`wiilaunch.c:50–61`). Rust uses the same fields and order (`ogc-wiilaunch-state/src/lib.rs:14–27`). C PowerPC layout is 4128 bytes; Rust's target test asserts 4128 bytes (`ogc-wiilaunch/src/lib.rs:524–532`, `state/src/lib.rs:409–418`).
- `StateFlags`: C uses checksum, four one-byte fields, and six `u32`s (`wiilaunch.c:63–70`). Rust uses the same field order and width (`state/src/lib.rs:46–55`); expected size is 32 bytes.
- Checksum: C sums native big-endian 32-bit words after the first checksum word (`wiilaunch.c:84–105`). Rust sums big-endian words from byte offset four with wrapping addition, and writes/validates the leading checksum in big-endian byte order (`state/src/lib.rs:112–146`). On the Wii's big-endian PowerPC, the byte representation and arithmetic agree.
- Argument serialization: C includes the title string as argv[0], computes aligned string storage plus count/pointers/null entry, places argv metadata at the end of the 0x1000-byte buffer, and stores offsets as `0x1000 + buffer_offset` (`wiilaunch.c:204–244, 290–326`). Rust's `set_args` uses the same buffer sizing, zero-fill, argument order, and offsets (`state/src/lib.rs:148–197`); `try_launch_title_with_args` formats the title ID as 16 lowercase hex digits and prepends it (`ogc-wiilaunch/src/lib.rs:393–429`). Rust checks arithmetic overflow and does not mutate the buffer when it returns `TooLarge`; C returns `WII_E2BIG` when computed size exceeds the buffer.
- Launch state transitions: normal launch persists NAND boot info before network cleanup/video blackout/ES launch (`wiilaunch.c:246–288`). Rust does the same through `commit_and_launch`, then calls network cleanup, blackens and flushes video, and launches ES (`ogc-wiilaunch/src/lib.rs:326–369`). Argument launch writes state flags, then NAND boot info, before launch; Rust's `commit_and_launch(..., true)` has this order (`state/src/lib.rs:249–258`) and `try_launch_title_with_args_inner` sets `TYPE_RETURN`/`RETURN_TO_ARGS` first (`ogc-wiilaunch/src/lib.rs:393–429`). Menu/settings launch similarly set the type/return code and persist flags before the NAND boot info (`wiilaunch.c:340–357`; `ogc-wiilaunch/src/lib.rs:431–465`).

## Public API coverage

Every C header entry has a Rust counterpart at the semantic level:

| C declaration (`wiilaunch.h`) | Rust API |
| --- | --- |
| `WII_Initialize` (65) | `initialize` / `try_initialize` (`ogc-wiilaunch/src/lib.rs:261–279`) |
| `WII_ReturnToMenu` (66) | `return_to_menu` / `try_return_to_menu` (`lib.rs:431–438`) |
| `WII_ReturnToSettings` (67) | `return_to_settings` / `try_return_to_settings` (`lib.rs:440–447`) |
| `WII_ReturnToSettingsPage` (68) | `return_to_settings_page` / `try_return_to_settings_page` (`lib.rs:467–477`) |
| `WII_LaunchTitle` (69) | `launch_title` / `try_launch_title` (`lib.rs:281–288`) |
| `WII_LaunchTitleWithArgs` (70) | `launch_title_with_args` / `try_launch_title_with_args` (`lib.rs:380–429`) |
| `WII_OpenURL` (71) | `open_url` / `try_open_url` (`lib.rs:479–518`) |

Rust also exposes the C settings strings and error values with equivalent contents (`wiilaunch.h:37–59`; `ogc-wiilaunch/src/lib.rs:25–29, 88–94`). Its state crate exposes reusable structs, checksum and argument helpers, verified persistence traits, error enums, and launch sequencing (`ogc-wiilaunch-state/src/lib.rs:8–110, 124–258`). These are Rust-only abstractions; C keeps the state structures and helper routines private to `wiilaunch.c`.

## Uncertain or unchecked

- No runtime test on Wii was done. ES behavior, IOS filesystem write semantics, network cleanup effects, and video black/flush timing are not confirmed on hardware.
- The C compiler's exact PPC structure alignment is inferred from its field sequence, PowerPC ABI expectations, and C's `ATTRIBUTE_ALIGN(32)` on the global objects; the checked-in C source does not include static size/offset assertions. Rust has target-gated layout assertions, but they were not run (`ogc-wiilaunch/src/lib.rs:520–532`).
- C `WII_LaunchTitleWithArgs` receives an unchecked variadic list and assumes a null sentinel; Rust's slice API has different input bounds and cannot match malformed C call behavior (`wiilaunch.c:290–311`).
- C may continue after state-file writes fail because it ignores helper return codes. Rust instead aborts before launching. Hardware and compatibility expectations for the safer Rust behavior are not verified.
- Rust's persistent launch state is protected by a lazily initialized LWP mutex (`ogc-wiilaunch/src/lib.rs:107–207`); C uses unguarded file-static state. Concurrent C calls are therefore not directly comparable, and concurrent Rust call ordering was not runtime-tested.
- Target-gated PowerPC layout assertions were not run by the host test command (`ogc-wiilaunch/src/lib.rs:520–532`). The state crate host tests did run as part of `cargo xtask test-host`.

## Dolphin evidence review (2026-11-21)

The archived `2026-11-20-wiilaunch-uninitialized-guards` batch does not prove
uninitialized-state behavior. The system startup probe calls
`wiilaunch::initialize` before the runtime probe runs
(`probes/ogc-system-probe/src/lib.rs:475–485` and
`crates/core/ogc-system-sys/src/lib.rs:2094`). Its passing or failing checks
must not be used as evidence for uninitialized guards. At that time the launch
APIs remained unverified in the inventory. Do not call the raw launch, menu, settings, or URL
operations to test this state because a successful call can leave the probe or
launch another title. A valid check needs an isolated state fixture that does
not initialize the production singleton.

## Dolphin rejection evidence (2026-11-27)

The `2026-11-27-wiilaunch-guard-cases-v2` run passed all 901 runtime checks in
Dolphin 2609a. It verified that an unknown title returns a negative IOS error
through both `try_launch_title` and the legacy `launch_title`, and that an
oversized argument list returns `ArgsTooLarge` or `WII_E2BIG` through the typed
and legacy APIs. The oversized input fails before persistence or launch.

These checks cover rejection behavior only. They do not verify menu/settings
return or URL launch. See the archived run for the exact check log, Dolphin log,
and screenshots.

The state crate's host tests passed. These tests verify serialization,
buffer preservation on oversized input, persistence error handling, readback,
and commit order. They do not prove that IOS launches a destination title.

## Successful handoff and argument persistence (2026-11-27)

The `2026-11-27-wiilaunch-handoffs` run proves successful `try_launch_title`
and `launch_title` handoffs to the TMD-verified OHBC test title. The
`2026-11-27-wiilaunch-args` run proves successful
`try_launch_title_with_args` and `launch_title_with_args` handoffs. Its runner
checks the complete persisted NANDBOOTINFO and state.dat checksums, launch code,
title type, argv count and strings, null argv pointer, and return-to-arguments
state. Both runs record the test title startup marker.

This does not prove that OHBC consumes the arguments. It does not prove other
destination titles or physical Wii behavior. The System Menu fixture remains
open because the available boot image does not match its TMD. Physical cache
and DMA evidence also remains open. See
`../dolphin-runs/2026-11-27-wiilaunch-args/README.md` for fixture hashes and
per-variant reports.
