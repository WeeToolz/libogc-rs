# WPAD status callback in Dolphin

This batch checks `set_status_callback` by registering a callback before WPAD
shutdown. The callback records channel notifications independently and rejects
channel values outside the WPAD device range. Dolphin passed 94 checks with no
failures. The inventory moved from 180 to 179 unverified APIs.

This confirms callback registration and valid channel arguments for shutdown
notifications in Dolphin. It does not test every status change or physical Wii
behavior.

See the [batch report](batch-report.md), [probe log](wpad_probe/probe.log),
[suite metadata](suite.json), and [completeness audit](completeness-audit.log).
