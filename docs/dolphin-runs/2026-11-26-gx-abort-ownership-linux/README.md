# GX abort FIFO ownership and GameCube recovery — Linux, 2026-11-26

Stock Dolphin 2609b (Linux x86_64, OpenGL). No emulator patch or production
Rust change. The new probes exercise the real PowerPC production APIs.
The local Git base is `2ba6ed42`; the user reports upstream `168f83e7`.
The upstream tree was not independently fetched.

| Run | Pass | Fail | Valid pages |
| --- | ---: | ---: | ---: |
| Wii GX, initial detached/ownership checks | 94 | 0 | 9 |
| Wii GX, wrap and queued PE checks | 105 | 0 | 10 |
| Wii GX, strengthened pending detached GP work | 105 | 0 | 10 |
| Wii GX, final with already-raised PE interrupts | 109 | 0 | 10 |
| Wii video, before final PE-only extension | 414 | 0 | 32 |
| Wii LWP | 282 | 0 | 20 |
| GameCube GX abort | 12 | 0 | 1 |

The detached case holds 96 bytes in the GP FIFO at a breakpoint while the
separate CPU FIFO write pointer crosses the ring boundary. Abort discards
GP work without replacing the CPU pointers. The linked case verifies
revision commands across wraparound. Queued token and finish commands are
discarded, and new commands deliver their callbacks once. In a separate
case, both PE cause bits (0x600) remain set across abort with interrupts
disabled, and both callbacks execute once after interrupts are restored.
Changing the designated producer does not change ownership of the GX
suspension; an extra application suspension remains until released.

The GameCube probe uses a distinct GameCube DOL and checks the FIFO with
96 pending bytes, CP control 0x15 after abort, no Wii revision commands,
preservation of the pending VAT update, and new PE completion. Dolphin's
boot log confirms GameCube mode. The initial GameCube build required an
explicit unsafe address expression; the corrected build and run passed.

The checked results, DOL hashes and capture counts are in `summary.json`.
Selected guest check lines and the GameCube boot mode are recorded in the
adjacent logs. Unmodified raw results, transport, logs and captures are
archived privately; the archive checksum is in the summary. No user
profiles, NAND, or console credentials are included in the commit.

Host xtask tests: 165/165. Inventory: 3,419 entries, current. Source audit
passed with the existing keyboard audit-debt rows. The full runtime suite
was not repeated. This is scoped runtime evidence, not a full independent
GX API review. Physical CP sticky status and PE memory-counter timing
remain pending. Six GX inventory entries still reference host tests;
review their target evidence mappings and edge cases separately.
