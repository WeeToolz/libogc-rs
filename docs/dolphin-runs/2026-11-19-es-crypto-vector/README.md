# ES AES-CBC Dolphin evidence

Dolphin 2503 on Linux ran `general_probe` with **63 passes and zero failures** and five valid captured pages. The [transcript](checks.txt) and [screenshot](crypto-checks.png) show the encryption and decryption checks. DOL SHA-256: `8935ee0bbdf4f92908e225c54b3cb2c2606ef7dc9c7abfec5e6e74e005ca1762`. The source was based on `28fd1cade114` with the probe change in this commit.

The probe uses the retail common-key handle 4 with two fixed AES-CBC blocks. An independent OpenSSL calculation provided the expected ciphertext. It checks encryption and decryption separately against fixed bytes, output guards, and the final IV equal to the last ciphertext block. The ES service opens and closes around the checks.

This validates the named inputs against Dolphin's IOS implementation. Other key handles, larger inputs, overlapping input and output, and physical Wii behavior remain open.

Run `cargo xtask dolphin-validate --probe general_probe --timeout-seconds 600` with a display and configured NAND. This run used software OpenGL and a private NAND copy.
