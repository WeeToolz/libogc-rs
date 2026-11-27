# IPC replacement tracker origin

Date: 2026-11-27. Dolphin 2609a. The existing `ios_reload_probe` passed 121
checks and failed 0 checks. The runner built DOL SHA-256
`75253c43a6a639fb4adaae63595819f037d268efcdd90e1f6763f158396606e7` from a
dirty worktree at `84ea806d4e5ad9a1c6fca356fd78eecb47115032`. It used and then
removed a private copy of the Dolphin NAND. No System Menu handoff fixture was
used.

After an injected fresh-handshake failure, the two old request identities
remained tracked. A third request was present: IPC command `6` (`IOS_IOCTL`),
descriptor `2`, callback address `0x80049128`, and null userdata. The symbol
table for the probe ELF maps that callback address to
`ogc_stm::event_hook_handler`. `ogc-ios-sys::reload_ios` starts the IOS
subsystems after `_ios_launch_new_ios` returns. The new IOS STM event hook is
therefore added after `IosReplacement::finish` declines to retire the two old
requests. This explains the tracker count rising from two to three.

The probe closes the new STM event hook through `ogc_stm::close` before it
retries the old requests. That close leaves the two old request identities
tracked. A later real `/dev/es` open/close retry retires them. Both old
callbacks run once with `IPC_ENOENT`; formatter storage is freed without
copyback. The probe then opens and releases a new event hook successfully.

This identifies the extra tracker as valid work on the new IOS. The close
before retry is probe cleanup; ordinary production flow performs one
replacement retirement before starting new IOS subsystems. The injected gate
does not simulate an actual `/dev/es` open failure. It does not prove physical
Wii DMA retirement or cache behavior. Those checks remain open.

`suite.json` records the runner result, Dolphin version, NAND handling, and
DOL hash. `probe.log` records all 121 named checks. `probe-transport.bin` is
the captured USB Gecko stream. `dolphin.log.gz` preserves the raw Dolphin log.
The probe log SHA-256 is
`f60ae197d61f2cd937b556e89d4364ebaece5b9a5c60bcb4c59fc8fe94ca484e`; the
transport SHA-256 is
`bb6be0b2db4dfce8ab018d9254611bf0f2ccfd2e4c46503e52b506839da496bc`; the
uncompressed Dolphin log SHA-256 is
`4ee9a3b0faeee69cc2af12064a0a6cca72a052a2a65b1d864d89e1ed9dc46565`.
