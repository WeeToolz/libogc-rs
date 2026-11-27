# IOS lifecycle parity probe

Date: 2026-11-27. Dolphin 2609a on macOS. The selected probe was built from
base revision `74ddd0d439b4f68074918617b025cc0cd232ba63`, with the probe
changes in this directory's task diff.

Run command:

```sh
cargo xtask dolphin-validate --probe ios_reload_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --timeout-seconds 120 --artifact-dir target/dolphin-validation/ios-lifecycle-parity-2026-11-27-final
```

The runner reported 112 checks passed, zero failed, and ten valid captured
pages. The probe DOL SHA-256 was
`db1f09cd635783f798f5dfcfc564cf47f509bb163c638393c83b382c260ca832`.

The probe called all five public IOS lifecycle APIs:

| API | Observed result |
| --- | --- |
| `shutdown_subsystems` | `0`; pass |
| `initialize_subsystems` | `0`; pass |
| `load_startup_ios` | `0`; pass; pinned C has the startup launch code disabled with `#if 0`, so this confirms the default no-op contract only |
| `launch_new_ios(2)` | `-12544`, equal to `IOS_EBADVERSION`; pass |
| `reload_ios(58)` | `0`; IOS 58 remained active, and `/dev/fs` opened and closed after reload; pass |

The same probe also checked all 24 ES operation argument guards with ES closed
and open. That set includes `import_boot` with null pointers and zero lengths.
The closed call returned `ES_ENOTINIT`; the open call returned `ES_EINVAL`
before any IOS submission. It does not test a successful boot import.

The Dolphin IOS log records an IOS 58 start during the reload. The run used
the runner's private Dolphin profile. It does not prove a real Wii reload,
System Menu handoff, cache behavior, or DMA behavior.

The runner's full report and raw transport output were inspected, then removed
because they contain local machine paths, emulator profile data, and raw
captures. The DOL digest and probe assertions above preserve the result needed
for the implementation inventory.
