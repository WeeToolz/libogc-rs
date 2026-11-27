# CAST GQR and conversion checks in Dolphin

Date: 2026-11-27. Dolphin 2609a. The current `runtime_probe` passed 912
checks with zero failures. Its DOL SHA-256 is
`81699b46689f22656af7193d036ee401c25a01afb01ffa04cb56cc949f9939ee`.
The [probe log](probe.log) records the checks.

The probe compared `CAST_Init` output for GQR2 through GQR5 with the C
defaults. It checked signed scale masking in GQR6 and GQR7. It checked all
four integer-to-float and all four float-to-integer conversions with values
that fit their declared widths. The probe saved and restored GQR2 through
GQR7 around the checks.

This verifies the CAST register setup and scalar conversion behavior in
Dolphin. It does not verify performance or physical Wii behavior.
