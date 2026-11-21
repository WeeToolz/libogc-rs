# WPAD saved controller wipe check

The batch seeds one registered controller and one guest controller record in
the copied Wii configuration. It calls `wpad::wipe_saved_controllers`, reads
both settings back, and checks that both match their default values. It then
restores and verifies the original records.

The Dolphin batch runner uses a private NAND copy and removes that copy after
the run. The source NAND remains unchanged. The run also confirms that the
configuration setters return the number of bytes written (`1121` and `517`),
not zero; the probe checks these results as successful writes.

Dolphin 2609a passed 93 WPAD probe checks with zero failures. The selected
checks are in [batch-report.md](batch-report.md) and
[`wpad_probe/probe.log`, lines 107–114](wpad_probe/probe.log#L107). The
captured screen is [page 4](wpad_probe/pages/page-004.png).

The inventory changed from 191 to 190 unverified APIs. This checks saved
configuration behavior in Dolphin. It does not prove persistence on a physical
Wii.
