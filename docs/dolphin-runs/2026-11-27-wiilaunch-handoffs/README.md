# Wii launch API handoffs in Dolphin

Date: 2026-11-27. Dolphin 2609a. The runner used a new private NAND copy for
each probe and removed each copy after the run. It checked the HBC title ID,
boot content size, and boot content SHA-1 against the title TMD before launch.

The installed OHBC test title has ID `000100014f484243`. Its TMD is 556 bytes
with SHA-256 `f84e20c6e30d6dfe77002a66260d4fce42888d3e567cbb81b1990cee6e2effb4`.
The TMD boot record names content `00000001.app`, size 2,454,528, and SHA-1
`8636c3acb85f20080a0baa18c324ac0feb7ea717`. The boot content SHA-256 is
`5d3f61d8e7e4304461d361b269b93777de4fb08c03683413cc85b8310fd4f610`.

| Public API | Result | DOL SHA-256 | Destination evidence |
| --- | --- | --- | --- |
| `try_launch_title` | Pass | `c3844e48e41b29cdcce8f188f50285e5dc0bed39c05d169c7d74b9dfaf28869b` | IOS boot log and HBC startup marker for `000100014f484243` |
| `launch_title` | Pass | `9d7da6139c79ea3b42f320e447224eb40a79ecb0b032c4f8d1c0431a30eea9a4` | IOS boot log and HBC startup marker for `000100014f484243` |

Both probes initialized Wii launch before they tested title ID zero. The typed
probe recorded the IOS error `-4100` from `try_launch_title(0)`. The legacy
probe recorded the same error from both `try_launch_title(0)` and
`launch_title(0)`. The runner then required an OSReport line
with the selected API and title, Dolphin's `BootstrapPPC` log for the HBC boot
content, and the HBC marker `HBC: startup complete; entering render loop`.
Both full-suite reports passed. See the [typed API run](../2026-11-27-wiilaunch-try-handoff/suite.json)
and the [legacy API run](../2026-11-27-wiilaunch-legacy-handoff/suite.json).

This proves the selected successful handoff and unknown-title rejection in
Dolphin. It does not verify launch arguments or launch-state persistence,
other destination titles, later HBC interaction, or physical Wii behavior.
