# IOS reload version mismatch cleanup in Dolphin

Date: 2026-11-28 UTC. Dolphin 2609a. The existing `ios_reload_probe` passed
**138 checks and 0 failed checks**. The probe DOL SHA-256 is
`0b97c0ba4b24379205773dcb110a458808524f4f5577913373705a0863c1518a`.
The runner used the existing isolated private NAND profile and removed its
temporary NAND copy after the run.

The added mismatch checks call public `reload_ios` and allow it to launch the
installed IOS in Dolphin. A probe-only hook reports a different version to
the launch path after the real handoff. It rejects an invalid version input
and a second pending injection. The probe confirms `IOS_EMISMATCH`, checks
that the actual IOS version remains unchanged, opens and closes `/dev/fs`
after cleanup, and closes the new STM event hook. This verifies the mismatch
cleanup path after a real Dolphin handoff. The hook does not change IOS memory,
so this is not evidence that IOS launched an unexpected version.

The exact named checks are in
[`result.json`](ios_reload_probe/result.json). The run's suite metadata is
[`suite.json`](suite.json), and the Dolphin and probe logs are retained beside
the result. The runner recorded source revision
`bd61ac4c84b9623f0cb9625a75912d9300f49e9c` with a dirty worktree; the DOL hash
identifies the code that ran. Physical Wii behavior remains unverified.
