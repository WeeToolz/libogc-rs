# ES Wii filesystem independent audit

Status: **historical/superseded by the 2026-10-02 implementation.** The missing-filesystem finding below describes the pre-fix source and is retained for audit history.

- Reviewer: `/root/reviewer_5`
- Original review: `/root/worker_a`, `docs/review/libes.md`
- Scope: Wii `es:` devoptab registration, path parsing, and file operations. This audit excludes the separate explicit `init()`/device-open parity fix recorded in commit `456756d`.
- C source: `../../C-sources/libogc/libogc/es.c`
- Rust source: `crates/core/ogc-es/src/lib.rs`, `crates/core/ogc-es-sys/src/lib.rs`, and `crates/core/ogc-es-sys/src/imp.rs`
- Validation: static source inspection only. No builds, tests, or hardware verification were performed.

## Confirmed mismatch: Rust has no `es:` filesystem or path-operation surface

On Wii, C ES initialization calls `__ES_InitFS` after establishing the ES descriptor (`es.c:124-152`), and `__ES_InitFS` registers the `"es"` devoptab (`:1264-1289`). The table supplies open, close, read, seek, fstat, and stat callbacks; write and mutation callbacks are null (`:1264-1285`). ES close calls `__ES_DeinitFS`, which removes `"es:"` (`:154-169,1291-1293`). This is the filesystem registration/teardown effect; the explicit descriptor initialization fix is outside this audit.

C `_ES_decodepath` parses the documented forms `es:<index>`, `es:ID<content-id>`, `es:/<title-id>/<index>`, and `es:/<title-id>/ID<content-id>` (`es.c:952-958`). Relative paths resolve the current title with `ES_GetTitleID`; absolute paths parse a title ID. The helper obtains and validates the stored TMD, then resolves either the content index or content ID (`:960-1065`). `_ES_open_r` accepts read-only opens, selects `ES_OpenContent` or obtains ticket views and calls `ES_OpenTitleContent`, and returns EROFS for write modes (`:1068-1108`).

The rest of the C device operations provide a coherent file interface: `_ES_read_r` reads aligned buffers directly and stages unaligned reads in a 64 KiB aligned buffer (`:1129-1183`); `_ES_seek_r` delegates to `ES_SeekContent` (`:1185-1204`); `_ES_close_r` closes the content handle (`:1111-1127`); and `stat`/`fstat` report content metadata from the TMD (`:1206-1262`).

The Rust crates expose no `es:` path parser, devoptab interface, `AddDevice`/`RemoveDevice` equivalent, or filesystem callback implementation. `ogc-es` only reexports `ogc-es-sys` (`crates/core/ogc-es/src/lib.rs:1-7`). `ogc-es-sys` exposes lower-level content operations: `open_content(index)`, `open_title_content(title_id, views, index)`, `read_content`, `seek_content`, and `close_content` (`crates/core/ogc-es-sys/src/imp.rs:2071-2185`). These accept an already-resolved content index/title/ticket view and raw content descriptor; they do not implement C's path-to-TMD lookup, read-only filesystem open, stdio callbacks, stat metadata, or registration under `es:`. `get_data_dir` only returns a title data-directory path and does not implement the `es:` content filesystem (`imp.rs:482-509`). A source search of both Rust crates found no `es:` path or devoptab registration.

At the time of this audit, Rust callers could not use C-supported `es:` paths through ordinary filesystem operations, and no `es:` device was mounted or removed. The low-level ES content ioctl wrappers did not provide equivalent observable filesystem behavior. The 2026-10-02 implementation follow-up below supersedes this finding.

## Result

The existing Wii `es:` filesystem finding is confirmed independently. The explicit initialization fix does not add the missing filesystem/path behavior. The original `docs/review/libes.md` note is unchanged; this addendum contains the independent evidence. No implementation code changed.


## 2026-10-02 implementation follow-up

The confirmed finding above is now addressed in source. ES initialization registers an `es:` devoptab and close removes it. The shared path decoder implements the documented current-title/explicit-title and index/content-ID forms, validates stored TMD data, and resolves a `TmdContent`. The callback table now supplies read-only open, close, read, seek, fstat, and stat behavior. Unaligned reads use one lazily allocated 64 KiB aligned buffer per open file, and explicit-title opens obtain aligned ticket views before calling `open_title_content`.

The implementation deliberately reuses `ogc-sysbase` for devoptab registration, `Reent` errno writes, `Stat`, and errno constants rather than creating an ES-specific filesystem registry or duplicate ABI structures. `read_content` also now preserves C's positive byte-count return value. Independent re-review and Wii hardware validation remain outstanding.
