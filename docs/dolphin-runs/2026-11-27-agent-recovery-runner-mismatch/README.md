# Agent-report recovery runner mismatch

## Expected behavior

Run the report-enabled system probe with Dolphin's private NAND profile. The
probe should write its startup report and complete its checks, then cancel the
recovery alarm. This run did not reach that result.

## Run details

- Dolphin: 2609a
- DOL: `target/xtask/probes-agent/ogc-system-probe.dol`
- DOL SHA-256: `398b57fabc1811240c8c3cfaf3dade871ffcf779bf22b0d240c0f1f714c2344f`
- Runner: `cargo xtask dolphin-validate --no-build --probe ogc_system_probe --timeout-seconds 90`
- NAND: private copy made by the runner
- Result: **inconclusive; crash, 0 frames, no probe summary**

The agent DOL used the Wiiload TCP report path and tried to connect to
`127.0.0.1:4298`. The Dolphin runner expected the development probe and its USB
Gecko transport. The NAND also did not contain the title TMD that the
development loader requested (`OpenFile(.../title.tmd): Result -106`). The
runner stopped after 41.2 seconds. These errors show that the selected DOL and
runner modes do not match. They do not show whether the recovery worker ran.

The raw runner output was inspected and removed from `/Volumes/SSD/WiiDev/tmp`
after these details were recorded. Keep worker scheduling, timeout recovery,
and return behavior open until a runner supports the agent-report DOL's actual
transport and launch path. Do not use this result to qualify physical recovery.
