# System Menu handoff for all four IPC relaunch APIs

Date: 2026-11-27. Dolphin 2609a on macOS. Rust source revision:
`21624b42b826ab44c2f4b1a4db4be09898ee99f6`; the worktree was clean during
all four runs. Each run used a fresh Dolphin profile copied from a temporary
private NAND fixture.

Before use, the System Menu TMD content record was parsed. It names boot
content ID `00000098`, index 8, size 3,867,904, and SHA-1
`26116613f624061ba99c8d1a299aaa6efa85670d`. The original boot content
`10000098.app` measured 3,867,904 bytes and matched that SHA-1. Its SHA-256 is
`47b9c1bb0ba1890256fb368b1b3272e33ea2467feadf39d20ce469d6de6e6c43`.
The original TMD SHA-1 is `e8f9657d591b305e300c109b5641630aa4e2318b`; its SHA-256 is
`9dbaf2e13d4cb76debacd3778bce33d9bc29172c2c9530692ef1a746f6034586`.
Only the temporary fixture copy used `title_or.tmd` and `10000098.app` to
replace `title.tmd` and `00000098.app`; the configured NAND was not changed.
The temporary fixture copy was deleted after the runs.

| API family | Mode | Dolphin result | DOL SHA-256 | Destination evidence |
| --- | --- | --- | --- | --- |
| C `IOS_IoctlvReboot` | Foreground | Pass | `08948a63408accfb28e7cfac14acb0bedeb259d6576b77d9c7b87c99121bec50` | title `0000000100000002`; boot `00000098.app`; PC `0x81534418` |
| C `IOS_IoctlvRebootBackground` | Background | Pass; returned `0` | `a0a9b14c188b20d34a98a3c995f56d68a247c41333e6de1b7bee1f525cb02438` | title `0000000100000002`; boot `00000098.app`; PC `0x81534418` |
| Rust `FileDescriptor::ioctlv_reboot` | Foreground | Pass | `bf2d636cf99541f2319505477323bd2cc6f986ae27266f876763d2045e8485bf` | title `0000000100000002`; boot `00000098.app`; PC `0x81534418` |
| Rust `FileDescriptor::ioctlv_reboot_background` | Background | Pass; returned `0` | `3d1e8b862ee2b0c0960b0a7425d792b3e8121ecdf53fe9f244123e22b8d03ee4` | title `0000000100000002`; boot `00000098.app`; PC `0x81534418` |

Each guest OSReport line records the API family, mode, and title ID. Dolphin's
IOS log records `BootstrapPPC` for `/title/00000001/00000002/content/00000098.app`.
The runner checks the destination instruction address and boot signature after
the source title is replaced. These are successful Dolphin title handoffs; they
do not prove later System Menu interaction or physical Wii behavior.

The selected handoff evidence and the SHA-256 of each original suite report are
preserved in [handoffs.json](handoffs.json). The raw reports, profiles, logs,
transport captures, and screenshots were temporary and have been removed. Each
original suite report recorded `status=pass` and a non-null destination handoff.
