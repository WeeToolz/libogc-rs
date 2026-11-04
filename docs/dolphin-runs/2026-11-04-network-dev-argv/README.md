# Development DOL startup in Dolphin

This is a historical run. See the [current network result](../2026-11-04-network-fixed/README.md) for the final standard and development DOL results.

The prior development DOL was launched without the arguments required by its
wrapper. The same DOL entered the probe when the runner supplied a report
endpoint, run ID, DOL SHA-256 build ID, and recovery deadline. The [log](probe.log)
records 43 passes and eight network failures, including passing debugger bind
fixture checks. This is a probe failure, not a full network pass. The local IP
is redacted. The DOL SHA-256 was
`8a3796fd1507a2fa669cc79fc227a2a8bfd5c7a77aa009c63281bfcea02c83c0`.

The runner now accepts matching development DOL and ELF paths through the
variables documented in `probes/README.md`. It does not receive or verify the
wrapper's UDP reports; it collects the Dolphin USB Gecko transcript. This run
shows that the DOL reaches the probe with valid arguments, not that the report
or recovery path works on a Wii.
