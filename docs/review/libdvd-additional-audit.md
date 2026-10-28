# Additional DVD parity audit

Reviewer: `/root/reviewer_6`  
Scope: source-only audit for additional `ogc-dvd` / `ogc-dvd-sys` mismatches beyond the previously recorded BS recovery and GameCube transport findings. The shared `libdvd.md` and ledger were not changed.

## Confirmed mismatches

- **Stop-stream-at-end performs a different command:** C `DVD_StopStreamAtEndAsync` sets command `0x08` and queues it (`../../C-sources/libogc/libogc/dvd.c:2355-2363`). C's command state issues `DVD_LowAudioStream(0, 0, 0, ...)` for command 8, which stops audio streaming (`dvd.c:1518-1522`). Rust sets the same command number but dispatches `RequestKind::StopStream` to `di::stop_motor()` (`crates/compatibility/ogc-dvd-sys/src/lib.rs:1333-1347,658-663`). Stopping the disc motor is not the C audio-stream stop operation, so the Rust API does not preserve this behavior on its Wii backend.
- **Automatic cache invalidation is applied at a different point and does not cover the same submitted commands:** C `__issuecommand` invalidates the caller buffer before placing the command in the queue for commands `0x0001`, `0x0004`, `0x0005`, and `0x000e` (`../../C-sources/libogc/libogc/dvd.c:1742-1758`). Rust checks `AUTO_INVALIDATION` only inside `RequestKind::Read`, and invalidates immediately before the worker sends the read ioctl (`crates/compatibility/ogc-dvd-sys/src/lib.rs:582-615`). The Rust `DiskId` path issues command `0x0005` through `di::read_disc_id_bytes` and does not consult that setting (`lib.rs:565-580`). Thus toggling `set_auto_invalidation(0)`/nonzero does not control the disk-ID command in the same way as C, and for queued reads the invalidation is deferred from submission until execution.

## Review limits

This pass did not reassess the known Wii BS recovery or GameCube transport findings. It did not build or run tests and did not perform hardware verification. The findings above are based on current Rust and C source paths cited inline.
