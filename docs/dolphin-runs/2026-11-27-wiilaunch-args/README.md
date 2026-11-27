# Wii launch with arguments in Dolphin

Date: 2026-11-27. Dolphin 2609a. Each run used a fresh private NAND copy. The runner removed the copy after the run. It verified the installed OHBC title against its TMD before launching it.

The fixture title is `000100014f484243`. Its TMD is 556 bytes with SHA-256 `f84e20c6e30d6dfe77002a66260d4fce42888d3e567cbb81b1990cee6e2effb4`. Its boot content is `00000001.app`, size 2,454,528, SHA-1 `8636c3acb85f20080a0baa18c324ac0feb7ea717`, and SHA-256 `5d3f61d8e7e4304461d361b269b93777de4fb08c03683413cc85b8310fd4f610`.

| API | Result | Probe DOL SHA-256 |
| --- | --- | --- |
| `try_launch_title_with_args` | Pass | `5037020813879d5ac0a5bf00b0ef652c6cefda4a1de3a7d7d10295de5ade00cc` |
| `launch_title_with_args` | Pass | `58bc1c33320cd5b6c973925c64f55a1384532eea7ec024f0baa632635e7b0826` |

For both runs the probe supplied launch code `0x4a2` and arguments `parity-probe` and `value=42`. The runner checked the complete NANDBOOTINFO checksum, launch code, application type, title type, argv count, title string in argv[0], both argument strings, and the null argv pointer. It also checked the complete state.dat checksum and `TYPE_RETURN` / `RETURN_TO_ARGS` fields. Both suite reports passed and Dolphin logged the destination HBC startup marker.

The evidence proves successful argument-state persistence and a handoff to this test title in Dolphin. It does not prove that the destination application consumed its arguments. It does not prove other titles or physical Wii behavior. System Menu remains open because its installed boot image does not match its TMD. Physical cache and DMA checks remain open separately. See each variant's `result.json`, `probe.log`, `dolphin.log`, and screenshot.
