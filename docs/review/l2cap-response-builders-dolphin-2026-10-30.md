# L2CAP response and rejection builders in Dolphin, 2026-10-30

The runtime probe now checks the production packet builders for connection
responses, configuration responses, disconnection responses, command rejects,
and invalid CID rejects. Each check compares fixed expected packet bytes,
checks a sentinel byte after the output, checks a short output buffer, and
checks an invalid ACL handle. The expected bytes are constants in the probe and
do not come from the builders.

The probe DOL SHA-256 is
`303524b4704f9f6e31ed3e19bce5484a05187edcb1f8a5ee18dbaddb8eb46abc`.
The command was `cargo xtask dolphin-validate --no-build --probe
runtime_probe --artifact-dir
docs/review/evidence/l2cap-replies-dolphin-2026-10-30`. The transcript reports
all five new checks as PASS and `PASS summary passes=32 fails=0`. The run
report, transcript, raw transport, logs, and screenshots are in
`docs/review/evidence/l2cap-replies-dolphin-2026-10-30/runtime_probe/`.

This verifies the builders under Dolphin. It does not establish Bluetooth
controller or physical Wii behavior.
