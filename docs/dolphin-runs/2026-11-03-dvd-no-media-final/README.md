# DVD no-disc Dolphin run

- Command: `cargo xtask dolphin-validate --no-build --probe dvd_probe`
- Run ID: `1793688177-5c6b7bf5bf51`
- Source revision: `5c6b7bf5bf51ac644763bfd78f51ad564aed27db`
- Worktree: clean
- Dolphin: 2609a
- DOL SHA-256: `8e41aad9be167ead5f99280e6782641204a4651fe2a3dce63af25f597e11d7c3`
- Result: 15 passes, 0 failures

The probe observed drive status 4, cover status 4, and no disc present. It
checked rejected mount, idle command-block state, auto-invalidation state
update and restore, pause and cancel state, reset state, stable static
information pointers, and shutdown. The probe used Dolphin's private profile
and copied NAND.

The run had no readable disc. Inquiry, asynchronous disc ID and sector reads,
and physical DVD drive behavior were not tested. The result applies to this
Dolphin run only.

The saved suite report is `suite.json`. `result.json` and `probe.log` hold the
structured result and full transcript. The directory also holds Dolphin logs,
the received USB Gecko bytes, the tested DOL, and the captured console pages.
