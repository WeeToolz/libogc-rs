# libmad larger parity pass — 2026-10-01

Scope: `crates/media/mad-sys/src/lib.rs`, compared with current libogc `libmad/frame.c`, `stream.c`, `decoder.c`, `synth.c`, `version.c`, and their public headers.

Implemented in this pass:

- header-only decode and C-style stream/frame boundaries;
- MPEG header field, flag, CRC-target, duration, free-format, and error-state population;
- frame decode reuse of `MAD_FLAG_INCOMPLETE` state;
- public frame sample visibility to filter callbacks;
- synthesis from public frame data so filter mutations affect output;
- `MAD_OPTION_HALFSAMPLERATE` sample-rate/count behavior;
- synchronous decoder callback-flow behavior and return values;
- per-run `decoder.sync` ownership and cleanup;
- sync-only `decoder_message` behavior;
- stream/frame/synth lifecycle corrections;
- full libmad error text and version metadata parity.

Known residual: the codec engine remains `nanomp3`, and the public `sbsample` compatibility plane contains decoded PCM arranged in the public frame storage rather than libmad's true subband-domain coefficients. Exact libmad filter mathematics and bit-exact synthesis are therefore still unverified and should remain an open algorithmic finding.

Validation for this handoff is static only because the local environment does not contain Rust/Cargo. The downstream build agent should run target formatting/checks/tests and correct any compiler diagnostics before accepting the patch.
