# WPAD speaker encoding reference

The C reference harness uses `wencdata` and its tables from the local libogc
`wiiuse/speaker.c` (SHA-256
`39cfca9a7b57e3ea43cb4eeb7d43c7c03e65a6728c8cdfd9a0f70bf26ac08d9c`).
It calls the encoder in the same order as `WPAD_EncodeData` and prints the
encoded bytes, predictor, and step. Build and run it with:

```sh
cc -O0 c-reference.c -o c-reference
./c-reference
```

The second vector gives C an explicit zero after the third sample. Rust uses
zero padding for an odd sample count; C reads a second input sample. The
Dolphin probe also checks that split encoding with `WPAD_ENC_CONT` gives the
same bytes and state as one call. This does not test speaker transport.
