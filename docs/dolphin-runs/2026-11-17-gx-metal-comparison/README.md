# GX backend comparison in Dolphin

Dolphin 2609a on macOS passed all 349 video checks on Metal. The exact same
DOL reported three GX EFB failures on OpenGL in the [earlier run](../2026-11-17-raw-video/README.md).
On Metal, the three depth and texture-copy checks passed. The Metal depth
reads matched `0x123456`, `0xabcdef`, and `0x654321`; the OpenGL run read
`0x891a2b`, `0xd5e6f8`, and `0xb2a190` for those values. The OpenGL values
are consistent with a half-range depth transformation, but the source of
that transformation has not been established.

The comparison used the same DOL SHA-256, Dolphin executable, copied NAND
source, and relevant runner settings, with the backend changed. Each run used a
separate private profile and GDB port. The temporary runner
change replaced `--video_backend=OGL` with `--video_backend=Metal`; its hash
and the original runner hash are in [identities](identities.json). The runner
was restored after the run. See the [log](video_probe/probe.log),
[result](video_probe/result.json), [suite](suite.json), and
[summary screen](video_probe/pages/page-028.png). The runner stops Dolphin
after the summary, so its process exit code is null.

This comparison identifies a backend-dependent result; it does not establish
whether Dolphin, the host graphics stack, or the probe caused the OpenGL
failure. Physical Wii behavior remains open. HBC was not under test. No
production code changed. The NAND source hash was recorded after the prior
run; it was not measured from the Metal run's initial private copy.
