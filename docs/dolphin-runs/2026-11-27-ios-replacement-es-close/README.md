# IOS replacement and ES close follow-up

Date: 2026-11-27. Probe: `ios_reload_probe`. Dolphin: 2609a under x86_64
translation on the arm64 host. The graphical app's arm64 slice exits because
the VM does not expose NEON. The x86_64 slice ran when launched outside the
sandbox, which allowed its macOS services.

The probe passed **132 checks with zero failures**. The DOL SHA-256 is
`c6edc853dfc8c6db21c741b7230d33196ce75bd7e073144269374282f26be1bb`.
The pass includes these named checks:

- `replacement scope rejects competing thread sync async nested replacement ES close and descriptor close`
- `replacement scope worker stops IPC before scope release`
- `rejected descriptor close retains ownership for retry`
- `ES remains usable after rejected close`
- `ES closes after replacement scope ends`
- `failed IOS launch returns the injected ES error`
- `failed IOS launch leaves the IOS version word invalid like C`
- `failed IOS launch releases its scope and preserves pending requests`

The one-shot failure path first initializes ES. Before that correction, the
probe called `launch_new_ios` with ES closed and failed at `ES_ENOTINIT`, before
it reached the injected launch result.

The runner copied the user's Dolphin Wii NAND into a private profile. The
source NAND was 406 MiB. Its regular-file tree SHA-256, computed from sorted
`shasum -a 256` rows, is
`71a62fe4b93d7f1e9c8a8bd69202da45984f216d1085a0aa3fede70b3909ff7d`.
The installed IOS 58 subtree had a two-file tree SHA-256 of
`c080b35bf5bc34ed021faadcd0d45dcbc640a833fab0170e51b7b8caf140b26c`.
The System Menu TMD SHA-256 was
`1a5df7b0e72721c9ffae34664b00144eda2aeee0e327332deda84d3cfddb317b`; this
probe booted the test DOL directly and did not boot System Menu. Successful
System Menu handoffs use the separate TMD-matched fixture recorded in
[`stock-menu-relaunch`](../2026-11-27-stock-menu-relaunch/README.md).

The runner command was
`cargo xtask dolphin-validate --probe ios_reload_probe --dolphin /private/tmp/libogc-rs-dolphin-x86`.
The temporary wrapper invoked `/usr/bin/arch -x86_64` and was removed after the
run. The probe features were `dolphin-automation` and `ios-reload-probe`. The
runner used a private Dolphin profile and NAND copy. The report identified
source revision `367a0edad5d30bea5ed70d0059af9ed53c9125f2` and a dirty worktree.
Uncommitted `lwbt-sys` edits were present when the DOL was built; this probe
does not exercise Bluetooth connections.

The probe log is
[`probe.log`](ios_reload_probe/probe.log), SHA-256
`a0a31e67cf54c403c81690f15ad36b81853f248107e243c802115f8ae8f386d0`.
The transport capture is
[`probe-transport.bin`](ios_reload_probe/probe-transport.bin), SHA-256
`69db7b4f9be7b7b7a1c96939f180bf315d6c0f79caac5263ebdab5b7e17a55c7`.

This proves the named behavior in Dolphin. It does not prove a real IOS launch
error or physical Wii cache and DMA behavior.
