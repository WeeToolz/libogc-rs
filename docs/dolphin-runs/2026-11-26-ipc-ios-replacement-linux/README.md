# Outstanding IPC during IOS replacement

The stock Linux x86_64 Dolphin 2609b AppImage, the restored private System
Menu NAND, and the offline nightly `28221559263a3976766cf305940e80e30cf9ba8a`
were used on 2026-11-26 (local date). Rust base `8de576690b226d027487e52297a54960e0d7dfc1`
plus the probe changes and pre-existing local changes; C reference
`7d4b8b3e7b845516f0cc2a85274879ddd8cbf142`. This was not a clean base build.

The `ios_reload_probe` completed 42 checks with zero failures and five valid
framebuffer pages in 17.681 seconds. The [guest transcript](probe.log) and
[summary](summary.json) record the result and DOL digest. Full raw runner
artifacts contain private NAND data and are not included in Git. Two prior
attempts did not collect a guest transcript; an orphaned Dolphin process
owned the USB Gecko port. The passing run used a fresh isolated Xvfb display
and no competing test process. No checks or panic detection were disabled.

## Result

The test closes the library STM hook, installs an independent async event
hook with static aligned buffers, and confirms that IOS returns `-2` for a
duplicate hook while the first callback remains undelivered. A same-version
IOS 58 reload destroys the old emulated IOS instance. The old callback is
not delivered by the new IOS. A new independent hook completes once when
released, and the probe restores the library STM hook and accepts fresh IPC.

At four checkpoints, the probe allocates request-heap records until
`IPC_ENOMEM`, releases every record it acquired, reserves interrupt-deferred
slots until exhaustion, and cancels every reservation it acquired. It does
not reclaim the old IOS request by guessing an address or forcing a callback.

| Checkpoint | Available request records | Available deferred slots |
| --- | ---: | ---: |
| Before old request | 39 | 32 |
| Old request pending | 38 | 31 |
| After IOS replacement | 38 | 31 |
| After fresh request completes | 38 | 31 |

C `libogc/ipc.c::__IPC_Reinitialize` resets mailbox/relaunch/queue state,
not the request heap. C `__ipc_replyhandler` frees an async request after
its callback. Rust `reinitialize` likewise does not free old requests;
`submit_async_request` also reserves a bounded deferred slot, and
`deferred_request_callback` releases the pending count and frees the request
only when completion runs. The measurements demonstrate retained capacity,
not a cleanup pass. The reservation is Rust-specific; repeated abandoned
requests can exhaust completion capacity. No production correction is made.

Do not free outstanding records in generic PPC `reinitialize`: a posted
reply can survive it, as the prior [held-reply test](../2026-11-26-ipc-followup-linux/README.md)
shows. A separate IOS-replacement cancellation design must account for
queued, sent, posted and deferred requests, user data and formatter-owned
resources, and the proof that no old IOS DMA or late reply can reference
freed records. IPC remains parity-blocked for this resource-lifetime gap.
Physical Wii behavior remains pending; Dolphin does not prove it.
