# Injected IOS launch failure probe

## Expected behavior

The `ios_reload_probe` runs the public `ogc_ios::launch_new_ios` entry point
with a one-shot failure injected at the ES launch call boundary. The probe
checks that the API returns the injected error, the version word stays invalid
after the failed launch as in C, a fresh `/dev/fs` request succeeds, and the
previously pending raw and formatted requests remain tracked without
callbacks. This checks Rust's launch-error cleanup path. It does not simulate
an error returned by IOS.

## Build

- Probe variant: `ios_reload_probe`
- Features: `dolphin-automation,ios-reload-probe`
- Target: `powerpc-nintendo-none`, release
- DOL size: 706,944 bytes
- DOL SHA-256: `24a05badb5b048d4da4ce4f1756db1fbbea9883878e581774632e43d71e43398`
- Build command: `CARGO_NET_OFFLINE=true cargo xtask build-dol ogc-system-probe --bin ios_reload_probe --probe-features dolphin-automation,ios-reload-probe`

## Runtime result

This first attempt did not run. The existing runner command was:

```sh
CARGO_NET_OFFLINE=true cargo xtask dolphin-validate --probe ios_reload_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --artifact-dir docs/dolphin-runs/2026-11-27-ios-injected-launch-failure --timeout-seconds 120
```

The sandboxed runner stopped before boot because Dolphin exited with `SIGABRT`
on `--version`. A direct arm64 version check outside the sandbox reported
Dolphin 2609a, and the runner later passed the injected probe. See the
[follow-up report](../2026-11-27-ios-injected-launch-failure-retry/README.md).
This first attempt provides no behavior evidence. A real
`ES_LaunchTitleBackground` failure and physical Wii behavior remain open as
separate cases.
