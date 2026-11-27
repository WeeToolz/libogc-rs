# `lwbt-sys::close` Dolphin evidence

- Date: 2026-11-27
- Pinned source revision: `717ef6371bd1427b8a7aeb436c0dfffbaa1c98cd`; the worktree had the documented `close` and probe changes during the run.
- Dolphin: 2609a (`/Applications/Dolphin.app/Contents/MacOS/Dolphin`)
- Probe: `runtime_probe`
- DOL SHA-256: `9aa922c0c704c8f6a91f64cd0c0141642c855ee6d6b4f26f6a65e6ffdda8bf01`
- Result: pass, 915 checks, 0 failures, 108207 ms.
- Named result: `PASS lwBT close preserves the live PCB instead of shutting down`.
- Command: `cargo xtask dolphin-validate --probe runtime_probe --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin --timeout-seconds 600 --artifact-dir target/dolphin-validation/lwbt-close-semantics-2026-11-27`

The probe creates an owned PCB, calls public `bte::close()`, then disconnects and frees that PCB. `disconnect` must accept the PCB after `close`, which checks that `close` did not perform the full shutdown and release the pool. The full runtime probe also passed its existing shutdown checks.

The preserved transport output is [probe-transport.bin](runtime_probe/probe-transport.bin), SHA-256 `820c6a89202a0a463272decb18f4242bf9b2ba651e40921a0875333e65a31879`.

This Dolphin result checks Rust state behavior. It does not observe a pending HCI command-completion callback or a physical Bluetooth adapter. Keep that callback contract and physical adapter behavior open.
