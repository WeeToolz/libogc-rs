# GCSD final operating-condition attempt fix

Finding: `docs/review/libgcsd.md`, confirmed finding 8.

Implementer: `worker_b`. Independent reviewer: `reviewer a` (primary reviewer). The fix passed independent static review.

C `__card_sendopcond` repeats ACMD41/CMD1 while the response reports IDLE. When the 1,500 ms timeout expires, it issues one final operating-condition command, reads and checks its response, then returns `CARDIO_ERROR_IOERROR` if IDLE remains (`C-sources/libogc/libogc/sdgecko_io.c:918-963`). Rust previously returned `CARDIO_ERROR_IOERROR` immediately after the timed loop, without this last command.

Rust now routes the normal polling attempts and the final attempt through a shared helper. After timeout it invokes exactly one final attempt. The final attempt propagates command/response failures, returns success when the response clears IDLE, and returns I/O error when IDLE remains. For SDHC, the final ACMD result is ignored as in C; the separate existing Rust/C difference for ACMD results during normal polling remains open.

Regression tests force timeout after one IDLE response and check the final attempt can return ready, remain idle and return I/O error, or report a response error. Independent review compared the implementation with C `__card_sendopcond` (`../../C-sources/libogc/libogc/sdgecko_io.c:918-963`) and found the command order and result handling match. `cargo check -p ogc-gcsd --target powerpc-unknown-linux-gnu --tests` passed and compiled the tests; they did not run. No hardware verification was performed. Other GCSD findings remain open.
