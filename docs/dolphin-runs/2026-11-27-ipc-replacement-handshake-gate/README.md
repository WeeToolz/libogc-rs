# IOS replacement handshake gate

Date: 2026-11-27. Dolphin 2609a on macOS. The current `ios_reload_probe`
completed 112 checks with zero failures. Its DOL SHA-256 is
`173d7f6eb6d2a44c9f50da4e15d7ca54e4f79403acea956b7d5b7f8ca671c67b`.
The [probe log](probe.log) records the checks.

The run replaced IOS 58 while two asynchronous requests were pending. It
confirmed that successful replacement restored all request and deferred
callback capacity, cancelled the old raw and formatted requests once, avoided
staged output copyback, and accepted fresh IPC requests afterward.

The production cancellation decision now calls the shared
`retire_after_fresh_handshake` helper. Its host tests passed for both cases:
failed handshake skips retirement, and successful handshake calls retirement.
The Dolphin run covers the successful handshake path only. It does not prove
behavior after an actual failed `/dev/es` handshake.

This is Dolphin evidence. It does not prove physical Wii cache, DMA, loader, or
replacement behavior.
