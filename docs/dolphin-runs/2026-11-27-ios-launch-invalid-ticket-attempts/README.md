# Invalid IOS launch ticket attempts

Date: 2026-11-27. Dolphin 2609a. Each run used the existing
`ios_reload_probe` runner and a fresh private NAND copy. The probe held one raw
and one formatted IPC request while it called `reload_ios(58)` through the
production path.

Two feature-gated variants changed one field in the ticket view returned by
`IOS_GetTicketViews` immediately before the real ES launch request:

| Variant | Probe DOL SHA-256 | Result |
| --- | --- | --- |
| `ticketid = u64::MAX` | `798f096609b874b0d22c092505ba0039659c162ec6d7d0a56a99ed61301645db` | ES accepted the request; `reload_ios` returned `0`. |
| `view = u32::MAX` | `f778c195096220519c0b560aaf3d3bd413310967a4ecb693c9cb17c2e96cd603` | ES accepted the request; `reload_ios` returned `0`. |

Both attempts failed the expected negative-result assertion. The runner stopped
that probe iteration after the unexpected successful launch. Neither run tests
the failed-launch branch or proves what happens to old requests after an ES
launch error. The altered ticket fields do not provide a reliable way to cause
that error in Dolphin, so the actual failed IOS launch case remains open.

The runner saved the probe and Dolphin logs here. The corresponding `suite.json`
files recorded `probe_fail` with 48 passes and one failed assertion. No
System Menu fixture was used. The private NAND copies were removed by the
runner. These attempts are negative evidence about this injection method only.

A separate safe preflight searched the installed Dolphin NAND for an IOS title
whose `ES_GetNumTicketViews` call returned an error. No candidate appeared in
the version range 3 through 255, so the probe stopped before it attempted a
reload. Its summary was 48 passes and one failed fixture-availability check;
the DOL SHA-256 was
`54d424af0a025781d651ceb2c740258ef06280e498f9b47512599479833b0af5`. The
probe log and Dolphin log are `probe-unavailable-title-scan.log` and
`dolphin-unavailable-title-scan.log`. This does not show that all real IOS
titles have ticket views; it records only the result from this Dolphin NAND.
