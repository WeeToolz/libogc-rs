# Injected IOS launch failure probe

## Run details

- Probe: `ios_reload_probe`
- Probe features: `dolphin-automation,ios-reload-probe`
- Dolphin: 2609a, launched as arm64
- Probe DOL SHA-256: `1fd7cd8191bbd30e308d073b251088d8e46a4fd2b56f8a0ee9e253e6dede1354`
- Run duration: 21.9 seconds
- Result: **PASS, 125 checks, 0 failures**
- Page captures: 13; runner marked all captures valid
- Probe log SHA-256: `aa590b14fd112c6004109fbebe54ea3b4dad590c5ec18b006e0cbe4adc692d84`
- Command: `cargo xtask dolphin-validate --probe ios_reload_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --timeout-seconds 120 --artifact-dir docs/dolphin-runs/2026-11-27-ios-injected-launch-failure-retry`

The probe checks the public `launch_new_ios` path with a one-shot negative
result injected at the ES launch call. The recorded assertions confirm the
exact injected error, an invalid IOS version word after failure, release of the
replacement scope, retention of pending raw and formatted requests, and a
later successful replacement. The run also confirms cancellation cleanup and
STM hook release after the later replacement.

The runner copied the NAND into the probe's private Dolphin profile. This run
does not test a System Menu or test-title handoff. Injection does not simulate
a real error returned by IOS, and Dolphin does not prove physical Wii behavior.
Keep the real ES launch failure and physical cache/DMA checks open.

The retained `ios_reload_probe/probe.log` contains the named check results.
The runner's raw `suite.json` and per-process logs were removed after review
because they contain machine paths, Dolphin profile details, and the emulated
MAC address. The runner's generated private NAND profile and duplicate binary
transport log were also removed. The 13 page captures remain with this report.
