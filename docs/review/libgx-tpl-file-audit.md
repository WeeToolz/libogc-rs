# GX TPL file-backed texture audit

Reviewer: `/root/reviewer_4`  
Date: 2026-10-01  
Scope: independent validation of the file-backed texture/palette alignment and loading-time difference in `ogc-gx`. No implementation changes, builds, tests, or hardware checks were performed. `libgx.md` and the ledger were not edited.

## Alignment difference confirmed

C keeps file-backed texture data separate from parsed headers. `TPL_GetTexture` saves the on-disk data offset, replaces `imghead->data` with `memalign(PPC_CACHE_ALIGNMENT, size)`, reads the data into that aligned allocation, flushes it, and passes it to `GX_InitTexObj` (`C-sources/libogc/libogc/tpl.c:210-249`). `TPL_GetTextureCI` does the same for image data and also allocates its palette with `memalign(PPC_CACHE_ALIGNMENT, nitems * sizeof(u16))` before reading, flushing, and initializing the TLUT/texture objects (`tpl.c:252-308`). `PPC_CACHE_ALIGNMENT` is 32 (`C-sources/libogc/gc/ogc/machine/asm.h:127-129`).

Rust's file open path reads bytes into `Vec<u8>`, boxes the vector, and calls the memory parser (`crates/hardware/ogc-gx/src/lib.rs:3424-3457,3460-3472`). That parser rewrites image/palette offsets to pointers into the backing byte vector (`:3390-3414`, helper `tpl_copy_memory_texture_headers`). For owned file data, `tpl_get_texture` and `tpl_get_texture_ci` flush and pass those pointers directly to the GX initializers (`:3522-3569,3601-3669`). A `Vec<u8>` allocation only has a Rust-guaranteed alignment of 1, and the stored data offsets do not add an allocator guarantee of 32-byte alignment. Thus Rust does not preserve the explicit 32-byte alignment guarantee C supplies for file-loaded image buffers and CI palettes. This finding concerns the file-backed path; C's memory-backed path also uses caller-provided pointers directly.

## Loading-time and file-observation difference confirmed

C `TPL_OpenTPLFromFile` opens and retains the `FILE*`, then reads the TPL header, descriptor array, and each image/palette header only (`tpl.c:86-145`). Image and palette payloads are fetched later by each `TPL_GetTexture` or `TPL_GetTextureCI` call (`:231-239,278-296`). `TPL_CloseTPLFile` closes the retained file and frees payload allocations made by the GetTexture calls (`:311-345`).

Rust `tpl_open_tpl_from_file` reads the complete file into a vector before parsing and returns an owned-memory TPL object (`lib.rs:3424-3457,3460-3472`). `tpl_get_texture` / `_ci` then use the already-loaded bytes and do not perform file reads (`:3522-3569,3601-3669`). Rust therefore performs all file I/O eagerly during open and snapshots the file contents; C performs payload I/O lazily at texture retrieval. This shifts when open/read errors occur, reads payloads for textures never requested, and means later on-disk changes can affect C's deferred reads but not Rust's owned snapshot. This is an observable loading and error-timing difference, separate from the alignment mismatch.

## Result

Both parts of the recorded TPL file-path finding are upheld by direct source comparison: Rust lacks C's guaranteed 32-byte file-payload allocations, and it changes file payload loading from lazy per-texture reads to a full-file read at open. The separate memory-backed TPL path is not included in this finding. No tests were run or compiled and no hardware verification was performed.
