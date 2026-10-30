# HBC Dolphin integration trace, 2026-10-30

This is a preliminary trace. It does not verify individual libogc APIs. The
HBC checkout was `898e8bbbf49319a49d7be735c9316f54676f7cc0` and the
libogc checkout was `36bfa030cc000d223b7de15a39ba1bbac69992e1` during
this review. The recorded builds include local modifications. The manifests
list modified paths but do not contain their diffs. The recorded libogc base
commits are not present in this checkout. Therefore the current call sites
below are candidates, not proof of the exact source in either DOL.

| Recorded result | Observed behavior | Current source leads and limits |
| --- | --- | --- |
| [UI acceptance r28](../../../hbc-rs/docs/review/evidence/ui-acceptance-dolphin-2026-10-23-r28/result.json) | Emulated Wii Remote pointer, bubble pop, date and name ordering, network settings and a scan error notice; receiver returned to Listening. | `src/platform/input.rs` uses `ControllerManager`, `wpad_sys::set_data_format` and `set_v_res`. `src/catalog.rs` sorts the visible catalog in HBC. `src/platform/network_scan.rs` calls `ogc_wd::default_scan_parameters` and `scan_once_detailed_result`. The result does not isolate their return values, prove a successful scan, or prove network settings persistence. |
| [Wiiload candidate r1](../../../hbc-rs/docs/review/evidence/ui-candidate-wiiload-dolphin-2026-10-23-r1/result.json) | Sender exited zero; transfer control and child startup were recorded; the child listener reached Listening. | Current `src/platform/wiiload.rs` uses `ogc_network` initialization, sockets, bind, listen, accept, read and close, and `ogc_lwp` threads. `src/main.rs` stages and transfers the executable. This sequence does not isolate each API or prove a physical Wii launch or return. |

The UI DOL SHA-256 is `a218f1a76d14fdbb010738188a382c36e6ad82f55ffe2aebd341ea4cb2b01442`;
its manifest names HBC base `4f3c222ba478eb85761e35ab2aaca18683ff9f5d`
and libogc base `7e7ccf13e9d3d4385c80853e9033a2daa3913a59`. The
Wiiload DOL SHA-256 is `c20ed3d888c2fb2609fdc206afa8c40b484eb9cb5508ac295b54bab2f9f968a4`;
its manifest names HBC base `5dd9fa89cc8fda6ebd2b5ea24d6edcf0f2f94cd9`
and libogc base `e716e77aa5b5ada0f2b7bb9fc66c697de15cff5d`. Both
manifests explicitly record modified source. They do not establish byte-level
source equivalence with the current checkout.

The GX canvas dimension check in `ui_input_probe` remains open. It is part of
an interactive monitor that also requires controller observations and shutdown.
The registered `dolphin-validate` runner does not register this probe. A
standalone runtime-probe pass does not execute this canvas check. Add a focused
Dolphin-compatible check or capture the monitor result with a suitable existing
harness, and retain the transcript and run identity before recording a pass.
Physical Wii results remain separate.
