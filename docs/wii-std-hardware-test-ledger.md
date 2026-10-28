# Wii std hardware test ledger

Updated: 2026-10-15. This ledger records the real-Wii std tests discussed in
the project session. A completed probe summary is a result. A Wiiload timeout
or a screen photo without a final report is recorded as incomplete and is not
counted as a pass or as a complete failure total.

## Hardware and evidence

The combined readiness and network runs below used the Wii at `10.0.0.242`.
Its native settings reported `RVL-001(USA)`, IOS 58 revision 6176, and
Hollywood revision `0x11`. The printed motherboard field was `unknown`; no PCB
label was available for these runs. The probe had a supplied display label of
`RVL-001 (USA)`, which is not a separate hardware measurement. The host was
`10.0.0.238`.

The user also reported a 54-check thread/runtime DOL pass on an RVL-101 with an
RVK-CPU-01 board. A separate older thread DOL pass was reported on an
RVL-CPU-20 console, likely an RVL-001. These are separate reports and do not
identify the motherboard of the console at `10.0.0.242`. See the
[thread evidence](wii-std-thread-evidence.txt) and its linked photos.

## Completed probe runs

| Run ID | Scope and DOL | Result | Failures or notable result |
| --- | --- | --- | --- |
| `1792070611-686276000` | Combined normal probe; SD and USB requested | **405 passed, 8 failed** | SD suite 78/0; USB IOS/callbacks 75/0; allocator 29/0; futex 33/0; synchronization 43/0; clocks 31/0; diagnostics 6/0; platform 5/0; controlled cleanup 22/0; final-return/TLS cleanup completed. Network failed four checks: local TCP acceptance, UDP edge acceptance, UDP bind to IOS, and HTTP bytes. USB FAT failed four checks: directory checks, path service checks, obtaining a new file, and owned-medium unmount. This was not an overall pass. |
| `1792071010-912955000` | Network-focused | **109 passed, 4 failed** | Local TCP acceptance, UDP edge acceptance, exact native HTTP diagnostic prefix, and HTTP exchange. |
| `1792071220-291193000` | Network-focused | **112 passed, 4 failed** | Same four network checks as the preceding run. |
| `1792071595-115194000` | Network-focused | **109 passed, 3 failed** | Local TCP acceptance, UDP edge acceptance, and HTTP exchange. Native diagnostic prefix had improved. |
| `1792071743-658606000` | Network-focused; shared receive-buffer build | **110 passed, 3 failed** | Local TCP acceptance, UDP broadcast option, and UDP edge acceptance. HTTP prefix and subsequent bytes through a clone passed. UDP send reached IOS error `-63` (newlib 95). |
| `1792072454-121609000` | Network-focused | **118 passed, 2 failed** | Local TCP acceptance and UDP edge acceptance. |
| `1792073187-135714000` | Network-focused with host peer | **114 passed, 2 failed** | Local TCP acceptance and UDP edge acceptance. |
| `1792073604-373083000` | Network-focused with host peer | **115 passed, 2 failed** | Local TCP acceptance and UDP edge acceptance. |
| `1792073807-545881000` | Network-focused with host peer | **115 passed, 2 failed** | Local TCP acceptance and UDP edge acceptance. |
| `1792074031-429038000` | Network-focused with host peer | **115 passed, 2 failed** | TCP listener byte exchange and UDP edge acceptance. |
| `1792074225-591932000` | Network-focused with host peer | **121 passed, 1 failed** | UDP edge acceptance. The host Python 3.9 UDP listener had exited after treating `socket.timeout` as an unhandled exception. |
| `1792074455-461341000` | Network-focused with host peer and activity indicator | **121 passed, 1 failed** | UDP edge acceptance remained; the host peer timeout handling was not yet corrected. |
| `1792074739-165610000` | Network-focused with corrected host peer and activity indicator | **130 passed, 0 failed** | All 41 network checks passed, including TCP callbacks, TCP_NODELAY, UDP sends and echoes for non-empty, empty, and connected datagrams, DNS, HTTP bytes, deadlines, and cleanup. The host peer log records all three UDP datagrams from the Wii. |

The combined-run report and framebuffer are preserved in
[the structured evidence](hardware-evidence/wii-std-readiness-rvl001-20261015.json)
and [the captured screen](hardware-evidence/wii-std-readiness-rvl001-20261015.png).
The final network pass has its own
[structured report](hardware-evidence/wii-std-network-rvl001-20261015.json),
[screen](hardware-evidence/wii-std-network-rvl001-20261015.png), and
[host peer log](hardware-evidence/wii-std-network-peer-rvl001-20261015.log).
Additional raw reports and screenshots are in each run directory under
`target/wii-runs/` while those build artifacts remain available.

The network-only runs used `--focus=network`. The presence of `--media=both`
in their launch arguments does not mean the SD or USB suites ran. They provide
no additional filesystem acceptance evidence.

## Incomplete Wiiload/report attempts

These attempts did not produce a complete final probe summary. Their reports
are useful for diagnosing startup and report transport, but they do not add
test passes or complete test totals.

| Run ID | Outcome |
| --- | --- |
| `1792002441-369240000` | Readiness check could not reach Wii port 4299; no DOL transfer. |
| `1792068957-789881000` | Readiness check could not reach Wii port 4299; no DOL transfer. |
| `1792069053-996356000` | DOL transferred and HBC endpoint went offline; no report connection was accepted within 60 seconds. |
| `1792069699-377733000` | DOL started and reached the early IOS pre-main report; no final report arrived within 180 seconds. The user observed the probe stop during execution. |
| `1792070013-611090000` | DOL started and reported startup progress; no final report arrived within 180 seconds. |
| `1792070280-570704000` | The network-focused DOL recorded four failures (local TCP acceptance, UDP edge acceptance, UDP bind to IOS, HTTP bytes), then the final report timed out. No complete total was received. |
| `1792072719-863116000` | No report connection was accepted. |
| `1792073310-428519000` | No report connection was accepted. |
| `1792074169-157874000` | Wii returned to HBC without a final report. |

The user's photos also document an early combined run that remained on a blank
page while the SD remount check was pending. Later media photos show the SD
file/path suite reaching the rename, canonicalization, recursive-removal, and
metadata checks, then finding that its exclusive `create_new` probe file
already existed. The full combined report above is the authoritative record
for the completed run and retains the USB FAT failures by name.

## Current evidence boundary

The network suite has a passing real-Wii run on the identified RVL-001 system.
The combined run proves the listed SD, thread, allocator, synchronization,
clock, USB IOS, platform-error, cleanup, and final-return cases passed in that
run. USB FAT had four failures and requires a clean rerun with its actual
medium and exclusive probe paths. The run does not test deliberate media
removal, interrupted writes, power loss, terminal panic/entropy modes, or
hardware revisions beyond those identified above. Entropy remains unavailable
and is not enabled for general std use.

See the [implementation checklist](wii-std-checklist.md) and
[combined probe guide](wii-std-readiness-probe.md) for current gates and
reproduction steps.
