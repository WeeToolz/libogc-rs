# IPC reply decoding and System Menu handoff

Date: 2026-11-26. Dolphin 2609b, Linux x86_64, Xvfb. C reference
`7d4b8b3e7b845516f0cc2a85274879ddd8cbf142`; Rust base
`8a461a2cad6e1e256b2758dfaa1c489a208227ae`, dirty local worktree. Compiler
`28221559263a3976766cf305940e80e30cf9ba8a`, local source lock and offline
Cargo dependencies. The baseline and corrected builds are not clean-checkout
builds at the recorded base revision. The working tree also contained
pre-existing unrelated modifications.

| Run | Checks passed | Failed | Valid capture pages |
| --- | ---: | ---: | ---: |
| Async reply regression, before fix | 22 | 1 | 5 |
| Async reply regression, after fix | 23 | 0 | 4 |
| Runtime baseline | 876 | 13 | 51 |
| Runtime after fix | 876 | 13 | 51 |
| IOS reload after fix | 21 | 0 | 3 |

The async regression compares the raw output vector address inside the
completion callback with its original CPU pointer. The fix uses reply word
2 (original operation) rather than word 0 (`IPC_REPLY`). Both runtime runs
have identical unrelated video and network failures; neither full run passed.
The live ES formatter sync and async output copyback checks passed, at 31,
32, 33, 63, 64 and 65 bytes and offsets zero and one. See the selected
[formatter checks](runtime-formatter-checks.log). The physical cache and DMA
coherency contracts stay blocked.

## Relaunch diagnostic and control

1. On the imported NAND, both foreground and background probes reached the
   System Menu title path, then Dolphin aborted. A direct boot of title
   `0000000100000002` without the Rust DOL also exited 134.
2. A separate direct-boot diagnostic disabled automatic abort on panic and
   paused at the error. The [dialog](direct-boot-panic.png) reports an invalid
   read from `0x00000000` at PC `0x8134d700`. Enabling MMU in a separate
   control kept the process alive but produced only a black screen.
3. A diagnostic copy with user channels and System Menu data moved aside
   displayed Priiloader v0.10.1. The original NAND contains a patched boot
   pair and original backups. The patched boot file has SHA-1
   `3497fb8aa59130edc5d9226166d0c2d4fd3ef262`. The original TMD records boot
   index 8, content ID `00000098`, size 3867904, SHA-1
   `26116613f624061ba99c8d1a299aaa6efa85670d`, matching `10000098.app`.
4. In a fresh private NAND copy, restore the original TMD and boot content
   only. Do not change the source NAND. With panic-abort enabled, direct boot
   reached the [Health and Safety screen](direct-boot-stock.png).
5. The same foreground DOL (SHA-256
   `faa8e9867430d78e2fc07cce34edf73b379d9323973f2149ce9d355281e8e6ee`)
   reached the [destination screen](foreground-destination.png). The same
   background DOL (SHA-256
   `e4a760c386c960ae0e77ac9dcbe86d53626d511ed95a6a093e9f96b740da8ca7`)
   returned zero in a sequential repeat and reached the
   [destination screen](background-destination.png). Both raw runner results
   are `timeout`, not `pass`: the old probe cannot finish after the title
   replaces it. The IOS logs identify the title and boot content. The
   first parallel background run reached the screen but did not retain a
   USB Gecko log; the sequential repeat resolves that evidence gap.

This demonstrates a title handoff, not full menu interaction. Restoring
both boot files is sufficient to avoid the observed abort; the precise guest
failure mechanism in the patched menu remains unknown. No panic suppression
was used in the successful controls or handoffs. Outstanding replies during
reload and physical Wii verification remain pending.

For this specific NAND, the following procedure reproduces the fixture.
Use a new destination; it contains private console data. The source files
must exist and the hash must match before copying over the destination files.

```sh
cp -R "$DOLPHIN_WII_NAND" "$PRIVATE_STOCK_NAND"
cd "$PRIVATE_STOCK_NAND/title/00000001/00000002/content"
# Verify 10000098.app against the original TMD boot record, including size.
# Expected SHA-1: 26116613f624061ba99c8d1a299aaa6efa85670d
# Only after verification, restore the two files in the PRIVATE COPY.
cp 10000098.app 00000098.app
cp title_or.tmd title.tmd
```

`summary.json` is a derived selection of report fields, including SHA-256
hashes of the original reports, not an edited runner result. The `*.log`
files contain selected unchanged lines from the raw logs. Raw reports,
transport, logs and capture pages are retained privately outside the repo
because some of them contain console identifiers. No NAND, keys, Nintendo
boot files or complete private profiles are included in this evidence.
