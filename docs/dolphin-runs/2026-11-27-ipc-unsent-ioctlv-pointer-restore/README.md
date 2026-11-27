# Unsent raw IOCTLV pointer restoration follow-up

Date: 2026-11-27. This is a build record. Dolphin did not run.

The existing `ipc_async_probe` now queues a raw `IOS_IoctlvAsync` request with
interrupts disabled before `ogc_ipc::reinitialize`. Its callback checks the
exact original input and output vector pointers, the `IPC_ENOENT` result, one
callback, and an unchanged output buffer. The request uses the existing ES
fixture and does not reach IOS.

Production cleanup now uses the same cancellation preparation path for unsent
requests as for requests retired after IOS replacement. The path restores
non-empty vector pointers to cached K0 form. It does not invalidate or write
unsent DMA buffers. The C implementation clears queued requests without
completing them; this Rust behavior remains a safety difference.

Build command:

```text
CARGO_NET_OFFLINE=true cargo xtask build-dol ogc-system-probe --bin ipc_async_probe --probe-features dolphin-automation
```

The PowerPC DOL build passed. Artifact:
`target/xtask/custom-dols/ogc_system_probe-ipc_async_probe.dol`

SHA-256: `a3559a59752c82002eb8e32ac604001e81e83cca82aa0d50c1edeabfac08f920`

The build manifest records source revision `e6dc75111566381b84c163655bdadd5f2b962abe`
and both changed source files.

Runtime attempt:

```text
cargo xtask dolphin-validate --no-build --probe ipc_async_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin
```

The runner stopped at Dolphin `--version`: the process exited with SIGABRT
(signal 6). No DOL ran, so the pointer restoration check remains open. The
default runner lookup also did not find Dolphin. The full Dolphin probe build
could not update its isolated Cargo registry because this environment could
not resolve `index.crates.io`; the targeted offline DOL build above succeeded.

The `cargo xtask check-group probes` command also remains unusable in this
checkout: its package list contains `easycap-patterns`, which Cargo cannot
find. The IPC probe itself compiled in the targeted DOL build.

Other verification:

- `cargo xtask check-group core`: passed with warnings denied.
- `cargo fmt --all --check` and `git diff --check`: passed.
- `cargo xtask check-inventory`: passed with 3,425 entries after refreshing
  line references affected by the production source edit.
- `LIBOGC_MAD_REFERENCE_MP3=/Volumes/SSD/WiiDev/Devtools/libogc-rs-fixtures/libmad-stereo-mpeg1.mp3 cargo xtask test-host`:
  passed after it ran with local socket access. The fixture SHA-256 matches
  the pinned value in `docs/dependencies.md`.
- `LIBOGC_MAD_REFERENCE_MP3=/Volumes/SSD/WiiDev/Devtools/libogc-rs-fixtures/libmad-stereo-mpeg1.mp3 LIBOGC_MAD_PROBE_MP3=/Volumes/SSD/WiiDev/Devtools/libogc-rs-fixtures/layer3-stereo.mp3 cargo xtask clippy`:
  passed for host packages, all 85 Wii runtime packages, and all 25 GameCube
  packages.
- `cargo xtask audit-stubs`: source-structure audit passed, but the runtime
  completeness audit still reports 133 disallowed API implementations.

The host suite first failed when the sandbox denied loopback socket binds.
It passed on a second run with normal local socket access. A first host run
without the reference MP3 also failed because that external fixture was not
set. These failures were environmental; neither run verifies the Dolphin-only
callback behavior above.
