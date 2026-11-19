# ES IV input snapshot Dolphin evidence

Dolphin 2503 on Linux ran `general_probe` with **59 passes and zero failures** and five valid captured pages. The [transcript](checks.txt) and [screenshot](iv-checks.png) show the invalid-key checks. DOL SHA-256: `96b38ee9d03404fff1095dd656a715f5664a798203147cea57c8eb3062f4d275`. The source was based on `7f5a9440813d` with the fix and probe changes in this commit.

The ES encrypt and decrypt implementations now copy the 16-byte input IV before creating the writable output vector. This avoids overlapping Rust shared and mutable references to the same IV during the IOS call. The probe sends aligned, non-overlapping buffers with an invalid key handle through each production API, verifies the IOS error and checks that the IV and destination remain unchanged. The observed error was -2000 for both calls.

The check covers IOS error propagation and output preservation in Dolphin. Successful IOS encryption and decryption, IV update behavior, and physical hardware remain unverified.

Run `cargo xtask dolphin-validate --probe general_probe --timeout-seconds 600` with a display and configured NAND. This run used software OpenGL and a private NAND copy.
