# MAD target arithmetic and ABI differential

Base: `273c2874f1b299b03bbb8f0fe8488b31bda6c68a`. C reference:
`7d4b8b3e7b845516f0cc2a85274879ddd8cbf142`. Rust nightly compiler:
`28221559263a3976766cf305940e80e30cf9ba8a` (local offline toolchain, not
the workspace source lock). Dolphin 2609b Linux x86_64 stock build. C
reference compiled by Clang for PowerPC 750 with the bundled FPM_PPC profile.

All 81 recorded source/fixture hashes were compared against the restored
baseline; no preimage mismatch was found. The previous baseline differential
was performed on an older snapshot with identical recorded preimages. See
`baseline-results.bin` and the kernel diagnostic logs. On the exact restored
baseline plus correction, the codec and mad-sys ABI decode/synthesis wrappers
were each run against C using two stereo MPEG-1 Layer III fixtures.

| Comparison | Values | Baseline mismatch | Corrected codec | Corrected ABI |
| --- | ---: | ---: | ---: | ---: |
| Header fields | 822 | 0 | 0 | 0 |
| Active subband values | 315648 | 126105 | 0 | 0 |
| PCM values | 315648 | 302911 | 0 | 0 |

Both implementations decoded 137 frames and had one recoverable error per
fixture, then buffer exhaustion. The earlier expanded IMDCT test compared
1152 values across four block modes: 544 differences before and zero after.
Host codec tests pass 117/117; mad-sys target compilation passes. A direct
native mad-sys test is rejected by its target guard; no target guard was
changed. The ABI run exercises stream, frame and synthesis initialization,
decode, state publication and cleanup, not all ABI functions or allocation
behavior. The bounded diagnostic arena does not reclaim allocations during
a fixture. The ABI run used 6,059,811 bytes at most; this is not a production
heap measurement. No physical Wii hardware was tested.

Reproduction: see `scripts/mad-ppc-differential/README.md`. The raw result
buffers are 25 big-endian words captured via Dolphin GDB after the completion
marker; the comparison checks each value in the guest, not only a checksum.
The correction uses the C long-block operation graph on PowerPC and retains
the existing host graph. No C is linked into the production library.

Remaining scope: independent source re-review of the new graph, other MPEG
variants, malformed input, remaining ABI functions, and physical audio/DMA.
Physical evidence is pending and is not a Dolphin completion gate.
