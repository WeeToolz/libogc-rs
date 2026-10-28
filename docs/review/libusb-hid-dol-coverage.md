# USB HID descriptor DOL coverage

This note records DOL coverage for the HID descriptor conversion in commit `218397a` and the v5 open preflight in commit `6a0f860`.

`hid_descriptor_probe` calls the production `convert_hid_descriptor_lengths` helper with fixed descriptor bytes. Its expected output is a fixed independent byte array containing the big-endian HID version and all three complete subordinate lengths. It also calls the production HID preflight helper with deterministic callbacks. The success case verifies best-effort resume, descriptor retrieval, and descriptor release order. The failure case verifies that the descriptor error is returned and suspension runs without releasing an unavailable descriptor.

Validation:

- `cargo check -p ogc-usb-sys --target powerpc-unknown-linux-gnu` passed.
- `cargo xtask build-dolphin-probes` passed and produced `target/xtask/probes-dolphin/hid_descriptor_probe.dol`.
- `cargo xtask dolphin-validate --no-build --probe hid_descriptor_probe` passed under Dolphin 2603-415. The log reports 5 passes and 0 failures, including all three HID conversion/preflight checks.

The probe exercises production policy and byte conversion with fixed inputs. It does not call IOS HID descriptor transport and does not establish hardware behavior.

Follow-up: the DOL now also tests the truncated trailing-entry case. Dolphin 2603-415 reported 6 passes and 0 failures for `hid_descriptor_probe`; the DOL SHA-256 is `8602985506489a670033235050133987c2b6cb0df752bdb4356adf120f039a19`. This executed the truncated-entry regression that previously only compiled. It remains a fixed-input DOL check, not a hardware test.
