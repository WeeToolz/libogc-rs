# libdb TCP timer probe in Dolphin

The existing `debugger_probe` passed in Dolphin df4bd44. It reported 23 checks
passed, zero failed, and four valid page captures. The DOL SHA-256 was
`3ff82d658bdcefd1dab56d3628b06d5356be2f909cdc6380808eaf9ce8fab639`.

The standard profile has no EXI BBA. The probe confirmed that TCP setup reports
the missing adapter and returns no interface. It also checked invalid TCP
handles and exported interfaces. It did not enter the connection setup path or
verify timer retransmission behavior. The result is in `suite.json`; the
transcript and final page are in `debugger_probe/`.

A second run used the same DOL and appended `Dolphin.Core.SlotB=12` to enable
Dolphin's built-in Ethernet device. It timed out after 120 seconds, with no
probe summary or page captures. The boot diagnostic stopped in the probe's
video retrace wait. This run gives no BBA or TCP timer acceptance evidence.
Its failure report is in `../2026-11-26-libdb-timer-bba/suite.json`.

Physical Wii evidence remains pending. This report covers only the Dolphin
attempts above.
