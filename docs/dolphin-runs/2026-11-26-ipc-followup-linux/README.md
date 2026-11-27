# IPC loader bounds, held reply and destination-aware relaunch

Date: 2026-11-26. Dolphin 2609b (stock Linux x86_64 AppImage), Xvfb.
Rust base `975ce952628416920f9a6cae0f5521f5c6a2f1ea` with the changes in
this review and pre-existing local worktree changes. C reference
`7d4b8b3e7b845516f0cc2a85274879ddd8cbf142`. Nightly compiler
`28221559263a3976766cf305940e80e30cf9ba8a`, local source lock and offline
Cargo sources. These are not clean-checkout builds of the base revision.
The private System Menu fixture is the restored stock fixture from the
[previous review](../2026-11-26-ipc-reply-relaunch-linux/README.md).

## IPC evidence

| Run | Passed | Failed | Capture pages |
| --- | ---: | ---: | ---: |
| Both nonzero bounds (initial) | 26 | 0 | 4 |
| Low override only | 26 | 0 | 4 |
| High override only | 26 | 0 | 4 |
| Held IOS reply, default bounds | 26 | 0 | 4 |
| Both overrides and held reply (combined) | 29 | 0 | 4 |
| Foreground relaunch | Handoff | 0 reported | 1 |
| Background relaunch | Handoff | 0 reported | 1 |

The debugger writes the low and high loader words at `0x80003130` and
`0x80003134` before continuing the guest. It reads back both words.
The test uses only the original aligned, reserved linker arena:
`0x933e0000..0x93400000`. The low override advances the base by 4096
bytes; the high override reduces the end by 4096 bytes. Each zero word
selects its linker default. The probe tests rejection outside the selected
bounds and restores the live allocator watermarks with interrupts masked.
The runner requires all three guest checks for every requested injection.
No production code change was necessary.

C `__IPC_Reinitialize` resets mailbox, relaunch and queue state under an
interrupt mask; it does not specify draining callbacks across replacement of
IOS. The held-reply probe disables PowerPC interrupts, submits a real async
`/dev/fs` open, waits for a posted IOS reply, and checks that the callback has
not yet run. It calls production `reinitialize`, restores interrupts, checks
one callback and a valid descriptor, closes that descriptor and opens/closes
a new descriptor. The combined run also verifies the callback count again
after recovery. This proves delivery across the PowerPC client reset with
an already-posted reply, not survival of queued or in-flight requests when
the old IOS instance is destroyed. The previous IOS 58 reload test covers
reload without this outstanding request. The latter combined case and
physical hardware remain pending; the package is not promoted.

The new relaunch runner requires a passing source ticket check, explicit mode
and handoff marker, the expected IOS title and boot-content log entries,
valid source capture pages, no reported failure, and a live GDB observation
of destination bootstrap text. It compares the first 64 bytes at the boot
entry with the installed boot DOL and checks that the PC is outside source
DOL text. Background additionally requires a zero return and passing
request-accepted check. Panic-abort remains enabled. It does not invent a
guest summary when the source title is replaced. Both modes passed in
separate sequential runs. The initial debugger observation failed because
Dolphin returned an empty register-read response; reading PC from the GDB
stop packet resolved that setup error. The report preserves this failed
attempt. These results verify title handoff, not subsequent menu interaction.

Reproduce with a private matching stock menu fixture and the configured
offline toolchain:

```sh
LIBOGC_DOLPHIN_IPC_BOUNDS=low cargo xtask dolphin-validate --probe ipc_async_probe
LIBOGC_DOLPHIN_IPC_BOUNDS=high cargo xtask dolphin-validate --probe ipc_async_probe
LIBOGC_DOLPHIN_IPC_BOUNDS=both cargo xtask dolphin-validate --probe ipc_async_probe
cargo xtask dolphin-validate --probe ipc_relaunch_probe
LIBOGC_DOLPHIN_RELAUNCH_BACKGROUND=1 cargo xtask dolphin-validate --probe ipc_relaunch_probe
```

Run one Dolphin process at a time for USB Gecko capture. `--no-build` requires
the DOL to match the selected relaunch mode. Do not use the diagnostic
panic-suppression option as passing handoff evidence.

## Runtime fixture corrections

The prior runtime result was 876 passed and 13 failed. Eleven failures
concerned VI mode or framebuffer registers. The probe allocated both
framebuffers using the preferred mode at early startup, before CONF was
initialized. Later it used the current preference as if it were the mode
used for that allocation. The fresh Linux run records actual startup mode
`0` and later preferred mode `22`. C `VIDEO_GetPreferredMode` reads the
configuration, not the active VI mode. This is a probe expectation error,
not evidence for changing production VI behavior.

The probe now owns a stable copy of the allocation mode and byte count.
Register checks, buffer-region checks, console restoration and system
snapshots use the actual allocation mode. The preferred-mode API test
still independently checks the current preference and caller storage.
The default console test restores VI configuration, not just the pointer.
The first corrected run passed all eleven original video checks but
exposed three snapshot checks with stale mode-address expectations and
two ES fixture assumptions. These were corrected without removing checks.

The ES probe recognizes only the two exact TMD SHA-1 values from the
private original and restored NANDs. The Priiloader-modified TMD must
still return `-2011` with unchanged stored-ID output; the original TMD
must return the nine expected IDs with the guard intact. The full stored
TMD digest and trailing guard are required in both cases. No production
ES behavior changed.

The two remaining Linux failures are `lwIP reuse option set and read`
and `Raw TCP reuse option is enabled`. In Dolphin 2609b
`Source/Core/Core/IOS/Network/IP/Top.cpp`, `MapWiiSockOptNameToNative`
translates Wii `SO_REUSEADDR` (`0x4`) to native Linux `SO_REUSEADDR` (2).
`HandleGetSockOptRequest` later compares the *untranslated* option to
native `SO_ERROR` (4), and overwrites the readback with the last error.
A native Linux socket control set reuse to 1 and read back 1, while the
native error option returned 0. This confirms the numeric collision;
libogc and Rust both use `0x4` for Wii reuse. The existing
`scripts/patches/dolphin-linux-network-options.patch` adds explicit guest-to-native mappings and big-endian payload
handling for these options before the original handler. Its dry-run
applied cleanly to the supplied Dolphin 2609b source. No patched Dolphin
binary was built: the uploaded source archive omits required submodule
contents, and the installed AppImage is stock. These two checks are still
failures in the runtime report, not skipped or promoted. A complete Linux
Dolphin build with that existing patch and a new runtime run are required.

Both final runtime runs used the same DOL SHA-256
`4b70867c95ca357969022dc0433cc2e3a5ef859e5645529babcd9871e8de8eac`.

| Runtime run | Passed | Failed | Capture pages |
| --- | ---: | ---: | ---: |
| Previous baseline | 876 | 13 | 51 |
| First corrected video fixture, stock NAND | 882 | 7 | 51 |
| Final stock NAND | 887 | 2 | 51 |
| Final original modified NAND | 887 | 2 | 51 |

Both final runs have exactly the two named reuse failures. The stock run
verifies the accepted TMD IDs; the modified run verifies rejection with
unchanged IDs. All eleven original video failures are resolved and no new
failure remains. Neither full runtime run is a suite pass.

Host `cargo test -p xtask --offline` passed 163/163; `cargo xtask check-group
core`, `cargo xtask check-inventory` (3416 entries), `cargo xtask
review-status` and Git whitespace checks passed. The static ledger still
reports 66 reviewed, 4 parity-blocked, 14 requiring re-review and 8 incomplete.

## Evidence handling

`summary.json` is a derived selection of report fields with hashes of the
original result and suite reports. The selected `*-checks.log` lines are
unchanged guest output. Raw logs, transport and pages are preserved in a
private archive outside Git; no Wii NAND, keys, private profiles or Nintendo
boot content are included. The original reports are not rewritten.
