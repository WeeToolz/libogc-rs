# GameCube CARD diagnostic

Dolphin 2609a started the focused CARD DOL in GameCube mode. The DOL SHA-256
was `05231a2fcdf91ac9a33e3fcc467b0783614ffa8d6357ea6eef0ac84d4b71c755`.
The runner attached a private copy of a 16 MiB card image. The source and copy
both had SHA-256 `f3e41359a1e597e0efbf6e7d734698a58b4197ecf0d405b6850d4109f26dd4b8`
after the run. The source image was unchanged.

The probe reported no card, skipped save operations, reported VI retrace
timeouts, and stopped before its summary. See [the probe log](probe.log). The
CARD backend uses a Wii EXI register address in this build; the GameCube EXI
address and the probe startup must be checked against the C implementation. This
run does not verify CARD persistence or physical hardware.

After the CARD EXI address and probe return fixes, a second GameCube run used
DOL SHA-256 `5cb4efa85f8fba48b999a160730b360c558897b9b8c54368acee732b9793eb8f`.
The presence-value check passed. The mount submission returned busy, the VI
retrace check failed, and the probe stopped before its summary. See the
[second log](probe-parity.log). The mount and persistence checks remain open.
