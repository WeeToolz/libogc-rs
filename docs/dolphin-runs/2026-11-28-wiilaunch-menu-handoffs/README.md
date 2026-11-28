# Wii launch menu handoffs in Dolphin

Date: 2026-11-28. Dolphin 2609a. Both runs used the existing
`ipc_relaunch_probe` runner and a fresh private NAND copy. The repository base
was `0ba07913a56d36bd42a68bea5bdbc50944902d68`; the worktree was dirty with the
fixture preparation, probe selection, and runner checks recorded by these
runs.

Before each run, the runner checked the private copy's original System Menu
TMD backup and boot content. The TMD SHA-256 was
`9dbaf2e13d4cb76debacd3778bce33d9bc29172c2c9530692ef1a746f6034586`. Its boot
record names content `00000098`, index 8, size 3,867,904, and SHA-1
`26116613f624061ba99c8d1a299aaa6efa85670d`. The boot image SHA-256 was
`47b9c1bb0ba1890256fb368b1b3272e33ea2467feadf39d20ce469d6de6e6c43`. The
runner restored `title.tmd` and `00000098.app` only in the private copy, then
checked the resulting TMD record again before launching Dolphin. The source
NAND hashes stayed unchanged.

| Public API | Result | DOL SHA-256 | Destination evidence |
| --- | --- | --- | --- |
| `try_return_to_menu` | Pass | `4d1fa81066c40337d674c503396ec0e57886a854f565e111ae04da785712d79b` | Dolphin booted `0000000100000002`; PC `0x814da804`; entry signature at `0x80004c00` matched SHA-256 `ec98e69acf4414edf0381d178ce26bdfa1d42264c235da28964d32e702c3270d` |
| `return_to_menu` | Pass | `f4f8c8dc7f647766666de915e97a2d07a29c5e736032ac78ca93f5e79486a23f` | Dolphin booted `0000000100000002`; PC `0x814da7f0`; entry signature at `0x80004c00` matched SHA-256 `ec98e69acf4414edf0381d178ce26bdfa1d42264c235da28964d32e702c3270d` |

The probe logged the selected API and foreground mode. Dolphin logged the
System Menu title launch and `BootstrapPPC` for
`/title/00000001/00000002/content/00000098.app`. The runner checked execution
at the destination and matched its entry bytes. Both reports recorded
`status=pass` with no failed checks. These results prove the title handoff in
Dolphin. They do not prove later System Menu interaction or physical Wii
behavior.
