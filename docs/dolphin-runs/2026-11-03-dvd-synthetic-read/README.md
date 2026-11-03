# Synthetic DVD sector-read diagnostic

Dolphin 2609a detected a generated, sparse GameCube disc image. The image
contains a known `TST1` disc ID and a deterministic sector-zero header. The
probe passed these data checks:

- Disc ID bytes matched `TST1` and company code `TS`.
- Synchronous sector-zero read returned 2048 bytes and matched the expected
  header bytes.
- Asynchronous sector-zero read returned 2048 bytes, completed its callback,
  and matched the expected header bytes.

The [probe log](probe.log) reports 78 passes and 10 failures. DVD mount
returned `-1`, and later DI assertions still expected no disc. This diagnostic
proves that Dolphin returned the known disc ID and sector-zero bytes through
the tested read calls. It does not prove that the sparse image is a valid
GameCube filesystem image, that DVD mount works, or that partition operations
work. It is not a full passing probe.

Synthetic image SHA-256:
`6546ce0e4ca90e037ea961e6c13fd10ad24a83ade3be91d79003dba716ba4848`.
DOL SHA-256:
`f69e4acb2cd6f050315454dff885d6042e0aaf50b6be497ae0bd18cfa2bddb94`.
