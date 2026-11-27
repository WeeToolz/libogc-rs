# IOS replacement helper verification

Date: 2026-11-27. Dolphin 2609a on macOS. This run used the current working
source after moving the IOS replacement result helper into `ios_logic.rs`,
which is shared by production and host tests.

The `ios_reload_probe` completed 112 checks with zero failures and captured ten
valid pages. Its DOL SHA-256 is
`69e4f9b9bee3e6888c8030cb7df475fc30f09652ba4ff6e0e0d28792846e0439`. The
[probe log](probe.log) records the check results. The normal IOS 58 reload
passed. This run compiles and exercises the matching-version production path;
it does not create an unexpected IOS version, so it does not test actual
mismatch cleanup or a failed fresh handshake.

The runner used a private Dolphin profile and the local NAND source. This is
Dolphin evidence only. It does not prove a Wii reload, System Menu handoff,
physical cache behavior, or DMA behavior.
