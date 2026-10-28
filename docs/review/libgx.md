# GX Review

Date: 2026-10-01
Reviewer: `/root`
Initial file-loading change implementer: unknown; it was already present as an uncommitted worktree change when this review began. The alignment and on-demand loading fix implementer is `/root`.

Status: static review is in progress. `ogc-gx` and `ogc-gx-sys` are reviewed together. The file-backed TPL fixes in commits `f02e5f3` and `5234086` passed independent static re-review by `/root/reviewer_1`; the implementer was `/root`. The wider GX and TPL API review remains incomplete. The C reference is `C-sources/libogc/libogc/gx.c`, `tpl.c`, `C-sources/libogc/gc/ogc/gx.h`, and `tpl.h`. This note does not claim full parity or hardware verification.

## Scope checked

- Texture object register setup and TPL texture size calculation.
- Scissor register packing, lighting object initialization, and texture load register words.
- `ogc-gx-sys` constants and C-layout object declarations.
- TPL memory and file open paths against `tpl.c`.

The GX register and object paths checked match the corresponding C behavior in the reviewed cases. The remaining public GX and TPL APIs still need a full static review.

## Finding for the fix pass

At the start of review, `tpl_open_tpl_from_file` returned an error for every path. C opens the file and reads descriptor and image/palette headers in `TPL_OpenTPLFromFile`, then reads image and palette payloads on demand in `TPL_GetTexture` and `TPL_GetTextureCI` (`../../C-sources/libogc/libogc/tpl.c:86-140,226-240,264-298`).

- **File-backed payload alignment and load timing differed.** C uses `memalign(PPC_CACHE_ALIGNMENT, size)` for image payloads and palettes before cache flush and GX initialization (`../../C-sources/libogc/libogc/tpl.c:234-240,281-298`; `PPC_CACHE_ALIGNMENT` is 32 in `../../C-sources/libogc/gc/ogc/machine/asm.h:129`). Rust previously pointed into a `Vec<u8>` and read the whole file during open. Rust now reads descriptor and image/palette headers at open, keeps the file open, and seeks to read payload bytes only when a texture is requested. It copies each requested file payload into a 32-byte-aligned allocation and keeps that allocation until `tpl_close_tpl_file` (`crates/hardware/ogc-gx/src/lib.rs`, `read_tpl_file`, `tpl_align_owned_data`, `tpl_get_texture`, `tpl_get_texture_ci`, and `tpl_close_tpl_file`). The caller-owned memory API still uses the caller's payload pointers, as C does for `TPL_FILE_TYPE_MEM`.

## Validation boundary

## Fix pass — file-backed TPL payloads

**BUG FIX.** Implementer: `/root`; commits `f02e5f3` and `5234086`. File loading now reads only the TPL header, descriptor table, and per-texture headers when it opens a file. It stores those headers in owned allocations and keeps the file open. `tpl_get_texture` and `tpl_get_texture_ci` seek to the payload offsets and read the data on demand. Each file-backed image or palette payload is held in a 32-byte-aligned allocation, retained for the lifetime of the TPL descriptor, and released when the descriptor closes. This matches C's file-backed allocation and load timing while meeting C's cache alignment before `dc_flush_range` and GX initialization. Memory-backed TPL data continues to reference caller-owned buffers.

Rust checks seek and read results and returns an error if a payload read is incomplete. C ignores these `fseek` and `fread` results and may continue with incomplete data (`../../C-sources/libogc/libogc/tpl.c:237-240,284-298`). This is a deliberate error-handling fix, not a parity claim.

Regression coverage checks that an unaligned owned payload is copied to a 32-byte-aligned buffer, that its bytes are preserved, and that repeated use reuses the retained buffer. `cargo check -p ogc-gx -p ogc-video -p ogc-sysbase --tests --target powerpc-unknown-linux-gnu` passed, so the target test code compiled. Tests did not run. `git diff --check` passed. `cargo fmt --check -p ogc-gx -p ogc-sysbase` reports one pre-existing formatting difference in the `ogc-sysbase` crate-level `compile_error!`; no changed GX code is reported. No hardware verification was performed. Independent review is required for this fix.

## Independent re-review — file-backed TPL fixes

Reviewer: `/root/reviewer_1`. Implementer: `/root`. Reviewed commits `f02e5f3` and `5234086` against `../../C-sources/libogc/libogc/tpl.c` (`TPL_OpenTPLFromFile`, `TPL_GetTexture`, `TPL_GetTextureCI`, and `TPL_CloseTPLFile`) and `../../C-sources/libogc/gc/ogc/tpl.h`.

The fix passes for the reported file-backed payload timing and alignment mismatch. C reads the file header, descriptor table, and per-texture headers at open (`tpl.c:86-140`), then seeks to image and palette data and allocates the payloads with `memalign(PPC_CACHE_ALIGNMENT, ...)` when texture access is requested (`tpl.c:210-305`). Rust's `read_tpl_file` and `tpl_open_file_backing` retain parsed header allocations and the open `DeviceFile`; `tpl_get_texture` and `tpl_get_texture_ci` seek to payload offsets and retain 32-byte-aligned allocations; `tpl_close_tpl_file` drops the payload allocations and file owner (`crates/hardware/ogc-gx/src/lib.rs`). `ogc-gx-sys` has no implementation change in these commits. The caller-owned memory path remains separate.

The regression test `owned_tpl_payload_copy_has_cache_alignment` verifies aligned copying, byte preservation, and reuse of an already aligned buffer. It does not exercise file opening, deferred file reads, CI palette reads, or close-time file ownership. `cargo check -p ogc-gx --tests --target powerpc-unknown-linux-gnu` passed, which confirms test code compilation only; tests did not run. No hardware verification was performed.

Rust rejects malformed or truncated file headers and payload ranges and checks seek/read results, while C does not validate these ranges and ignores `fseek`/`fread` results. These are stricter error paths than C, consistent with the deliberate safety/error-handling change described above; this review does not claim parity for malformed files. No remaining mismatch was found in the reported valid-file on-demand loading and cache-alignment fix. The broader GX API scope remains open for review.

## Independent review — broader GX API audit

Result: **NEEDS_CHANGES**.

Reviewer: `/root/reviewer_d`. Implementer: no single implementation commit was assigned for this first-pass API audit; the existing file-backed TPL fix was implemented by `/root` in `f02e5f3` and `5234086`, and that bounded scope remains accepted as recorded above. This audit compared the public GX helpers and C-layout types in `ogc-gx` and `ogc-gx-sys` with the GX/TPL C sources and headers. It found a confirmed texture-coordinate state mismatch, so the broader scope remains open.

### Confirmed mismatch

- **`load_tex_obj` changes texture-coordinate scale state that C leaves unchanged.** Rust `load_tex_obj` writes the texture-object BP words and then unconditionally writes two more BP words from `texture_coord_scale_words(map_id, width, height, filter)` (`crates/hardware/ogc-gx/src/lib.rs:865-898`, especially 892-896; helper at `crates/hardware/ogc-gx/src/register_state.rs:52-64`). These are SU S-size and T-size registers, set to texture dimensions and wrap bits. C `GX_LoadTexObj` calls the region callback and delegates to `GX_LoadTexObjPreloaded` (`../../C-sources/libogc/libogc/gx.c:3433-3441`); `GX_LoadTexObjPreloaded` writes the texture mode, size, TMEM, image, and optional TLUT registers, updates the GX shadow state, and marks texture state dirty (`gx.c:3443-3478`). It does not write SU S/T-size registers. C exposes those writes separately through `GX_SetTexCoordScaleManually`, which updates `texCoordManually` and cached SU state, and writes the registers only when enabled (`gx.c:3666-3679`). Therefore every Rust texture load overwrites manual or previously configured coordinate scaling, while the corresponding C operation preserves it. The Rust helper also does not update C's `texCoordManually` or `suSsize`/`suTsize` shadow fields. Remove the extra writes from the parity loader or document and expose them as a separate operation before treating this API as C-compatible.

### Scope boundary and differences

The `ogc-gx-sys` declarations inspected use C representation for the reviewed public records: `GxColor`, `GxColorS10`, `GxVtxDesc`, `GxVtxAttrFmt`, `GxFifoObj`, texture/TLUT objects and regions, `GxLightObj`, and the callback signatures (`crates/hardware/ogc-gx-sys/src/lib.rs:104-215`; corresponding declarations in `../../C-sources/libogc/gc/ogc/gx.h`). The Rust raw texture and light object views use the corresponding field ordering and widths (`crates/hardware/ogc-gx/src/lib.rs:1381-1420`). Their limited PowerPC size tests compiled, but did not run.

The reviewed Rust API is a subset of the full `gx.h` API. For example, this crate defines light position, color, direction, and specular-direction helpers (`ogc-gx/src/lib.rs:2465-2508`), but not C's `GX_InitLightDistAttn`, `GX_InitLightAttn`, `GX_InitLightAttnA`, `GX_InitLightAttnK`, or `GX_InitLightSpot` (`../../C-sources/libogc/libogc/gx.c:4695-4818`). This is incomplete API coverage, not evidence that the implemented helpers mismatch. Custom 2D/frame helpers also add behavior beyond direct C entry points. This review does not claim that the full `gx.h` API has been ported.

The already accepted file-backed TPL loading scope was not reopened. Adjacent texture/TLUT initialization was compared with C `GX_InitTexObj`, `GX_InitTexObjLOD`, `GX_InitTlutObj`, the texture getters, and `TPL_GetTexture`/`TPL_GetTextureCI` (`gx.c:3000-3068,3226-3441`; `tpl.c:210-305`). No additional confirmed mismatch was found in those paths during this audit. Safer null/range checks and malformed-input rejection are deliberate Rust error-handling differences, not parity claims.

### Validation

`cargo check -p ogc-gx -p ogc-gx-sys --tests --target powerpc-unknown-linux-gnu` passed. This compiled the test code; tests did not run. No hardware verification was performed. The mismatch above is based on source and register-write comparison, not build success.

## Independent review — additional GX FIFO and TPL subset (2026-10-03)

Reviewer: `/root/reviewer_f`. Implementer: `notOrrytrout` (author of the current GX source history; no separate fix commit was assigned for this subset). Result: **NEEDS_CHANGES** for GX dirty-state handling. This section covers the primitive FIFO helpers, texture-object getters, TPL memory-backed paths, and adjacent texture-load interactions. It does not complete the wider GX/TPL review and does not re-review the previously recorded texcoord-scale writes.

### Finding: texture dirty state is not flushed before Rust primitives (fixed; needs independent re-review)

C `GX_LoadTexObjPreloaded` writes the texture state and sets `__gx->dirtyState |= 0x0001` (`../../C-sources/libogc/libogc/gx.c:3443-3478`). C `GX_Begin` checks `__gx->dirtyState`, calls `__GX_SetDirtyState()` when nonzero, then writes the primitive opcode and vertex count (`gx.c:2642-2650`). Rust now records the SU S/T scale words as pending texture dirty state in `load_tex_obj`; `begin` emits those words before the primitive opcode and count. This restores the deferred ordering for Rust's texture mapping path. The texture scale calculation follows C's `__SetSURegs` (`gx.c:838-865`) and uses the texture map's matching coordinate, as in the existing Rust 2D texture path. The Rust crate does not expose C's complete internal TEV/indirect-stage state, so this implementation covers the active mapping represented by this Rust loader and does not claim a full C dirty-state system.

## Fix handoff: `load_tex_obj` preserves coordinate scale

**BUG FIX.** Implementer: `worker_a`. Independent re-review is needed; the review thread will handle it.

Rust no longer writes SU S/T size registers while loading a texture object. The load path emits the texture object's BP words in the same order as C's `GX_LoadTexObjPreloaded`; it leaves existing texture-coordinate scale state unchanged until `begin` flushes pending dirty state. The regression checks the exact emitted texture BP words and order, and confirms that none target the SU scale register range (`0x30..=0x3f`). No separate manual-scale helper is exposed in these Rust crates, so this fix removes only the writes added to the texture-load path. The dirty-state handoff below records the deferred writes before `GX_Begin`.

Validation: `cargo check -p ogc-gx -p ogc-gx-sys --tests --target powerpc-unknown-linux-gnu` passed, compiling the regression test. Tests did not run. `git diff --check` passed. No hardware verification was performed.

## Fix handoff: GX texture dirty-state flush

**BUG FIX.** Implementer: `/root/worker_a`. Independent static re-review is needed; the review thread owns it.

`load_tex_obj` records the two SU S/T scale registers selected by the loaded texture. `begin` consumes that pending state and emits both BP writes before it emits the primitive byte and vertex count, matching C's `GX_LoadTexObjPreloaded` dirty bit and `GX_Begin` flush order (`gx.c:3443-3478,2642-2650`). The focused test checks the deferred register words for a wrapped 128×48 texture and confirms no words are selected when there is no pending dirty state. This is limited to the texture mapping represented by the Rust loader because the crate does not maintain C's full TEV and indirect-stage shadow state.

Validation: `cargo check -p ogc-gx -p ogc-gx-sys --tests --target powerpc-unknown-linux-gnu` passed. The tests were type-checked but not run. `git diff --check` passed. No hardware verification was performed.

### Other bounded comparisons

- The scalar FIFO writers for positions, normals, colors, texture coordinates, matrix indices, and primitive end/begin use the same GP FIFO widths and component order as C's inline `GX_Position*`, `GX_Normal*`, `GX_Color*`, `GX_TexCoord*`, `GX_MatrixIndex1x8`, `GX_Begin`, and no-op `GX_End` (`crates/hardware/ogc-gx/src/lib.rs:1611-1937`; `../../C-sources/libogc/gc/ogc/gx.h:2110-2338`, `gx.c:2642-2650`).
- The texture-object getters read the same format, mipmap flag, wrapped pointer, dimensions, S/T wrap, LOD nibbles, and filter conversion table as C (`ogc-gx/src/lib.rs:2260-2347`; `gx.c:3000-3067`). Rust also provides null-tolerant snapshot helpers beyond C's raw getter contracts.
- The memory-backed TPL path parses the descriptor and image/palette headers, retains caller-owned payload pointers, and uses them in texture initialization. This matches C's memory-backed behavior: C does not copy image or palette payloads when `tdf->type == TPL_FILE_TYPE_MEM` and passes the header pointers to GX (`tpl.c:86-184,210-305`; Rust `ogc-gx/src/lib.rs:3531-3562,3836-3917`). Rust validates magic, descriptor offsets, and payload ranges that C does not validate; this is stricter malformed-input handling and is not claimed as parity for invalid files.
- The earlier C-layout spot check remains limited: `ogc-gx-sys/src/lib.rs` uses `repr(C)` on the inspected public GX object records, and its test assertions cover selected object sizes. This subsection did not compare every constant, record, or public declaration in `gx.h`/`tpl.h`.

### Validation and coverage

`cargo check -p ogc-gx -p ogc-gx-sys --tests --target powerpc-unknown-linux-gnu` completed successfully. This compiled the test code only; no tests ran. Existing tests cover selected texture object layouts and file payload alignment, but do not exercise texture load followed by `begin` or the C dirty-state flush sequence. No hardware verification was performed. The previously documented broader-scope limitation and texcoord-scale finding remain open.

### Independent re-review of pending texture flush (`febf8b4`)

**Result: PASS for the configured textured 2D load-to-begin path.** Reviewer: `/root/reviewer_f`; implementer: `/root/worker_a`.

C marks texture state dirty when loading a preloaded texture object and flushes dirty state before writing the primitive opcode and vertex count in `GX_Begin` (`C-sources/libogc/libogc/gx.c:3443-3478,2642-2650`). Rust stores pending scale words after texture-object BP writes and emits them before `pipe_u8`/`pipe_u16` in `begin` (`crates/hardware/ogc-gx/src/lib.rs:892-900,1623-1640`). This matches the configured textured 2D path, which pairs map 0 with coordinate 0 before load/begin (`lib.rs:791,928+`). The regression checks computed words and dirty-state selection, not the complete FIFO sequence.

Scope limit: Rust does not model C's full TEV and indirect-stage dirty state. The map-to-coordinate assumption is not established for arbitrary public `load_tex_obj(map_id)` calls. Broader GX findings therefore remain open. Reviewer_f ran `git diff febf8b4^ febf8b4 --check`, which passed; no build or tests were run in that review. The existing PowerPC `cargo check --tests` compiled tests, but they were not run. No hardware verification was performed.

## BUG FIX handoff: keep texture scaling out of generic texture loads

Implementer: `/root/worker_a`. Independent reviewer: pending. Generic `load_tex_obj` now emits only the texture object BP words and does not schedule SU S/T scale writes, so it leaves existing coordinate scale state unchanged as C `GX_LoadTexObj` does. The configured `draw_textured_rect` helper explicitly schedules its map-0/coordinate-0 dimension scale after the load and before `begin`, preserving the separately accepted 2D load/begin behavior. Rust now exposes the C-compatible manual scale, cylindrical-wrap, and range-bias operations; generic texture loading continues to leave manually configured coordinate scale state unchanged.

The texture load-word regression checks the emitted BP words do not target SU scale registers. The deferred-scale regression checks the custom 2D words and pending flush selection. `cargo check -p ogc-gx -p ogc-gx-sys --tests --target powerpc-unknown-linux-gnu` passed and `git diff --check` passed. Tests were compiled but not run. No hardware verification was performed.

## GX progress diagnostic handoff

Implementer: `/root`. The frame-configuration progress callback now reports viewport calculation and each viewport FIFO write stage. This is Rust diagnostic behavior used by the existing probe and is not a C parity claim. `cargo xtask build-dols` passed. No hardware verification was performed.

### Independent re-review — generic texture scale state (`7502b43`)

**Result: PASS for the reported generic-load mismatch.** Reviewer: `/root/worker_b`. Implementer: `/root/worker_a`.

C `GX_LoadTexObjPreloaded` writes texture mode, size, TMEM, image, and optional TLUT registers and updates GX texture shadow state; it does not write the SU S/T size registers (`../../C-sources/libogc/libogc/gx.c:3443–3478`). Rust's generic `load_tex_obj` now emits only the texture-object BP words and does not queue SU scale writes (`crates/hardware/ogc-gx/src/lib.rs:890–914`). This preserves coordinate-scale state as C does. The custom `draw_textured_rect` path explicitly queues its map-0 scale before `begin`, retaining the separately reviewed 2D behavior (`:922–936`); `begin` emits pending scale words before the primitive command (`:1658–1670`). Tests check the generic load word sequence and scale-word scheduling, though they do not run a complete hardware FIFO trace. The reported mismatch is fixed. Broader GX state and API differences remain open.

No checks or tests were run during this independent pass. The handoff records that the PowerPC target test check and `git diff --check` passed; tests compiled but did not run. No hardware verification was performed.

## 2026-10-05 ABI declarations handoff

Implementer: `/root`. The `ogc-gx-sys` crate now declares `GxTexReg` and `GxrModeObj` with the field order and widths from `gx.h` and `gx_struct.h`. Target test assertions check their C sizes (4 and 60 bytes). `cargo check -p ogc-gx-sys --tests --target powerpc-unknown-linux-gnu` passed; tests were compiled, not run. Independent static review is needed. This adds two ABI declarations only; the broader GX API and state differences remain open, so this does not close `ogc-gx` or `ogc-gx-sys`.

## Independent static re-review: `ogc-gx-sys` ABI declarations (2026-10-05)

**Result: PASS for the two added record declarations only.** Reviewer: `worker_f`. Implementer: `/root`; commit `7001d38`.

The C `GXTexReg` contains one `u8 dummy[4]` field (`../../C-sources/libogc/gc/ogc/gx.h:1314-1316`), and Rust `GxTexReg` has a four-byte byte array under `repr(C)` (`crates/hardware/ogc-gx-sys/src/lib.rs:151-156`). The C `GXRModeObj` fields in `gx_struct.h` are `u32`, seven `u16` values, another `u32`, two `u8` values, a 12-by-2 byte sample pattern, and seven filter bytes (`../../C-sources/libogc/gc/ogc/gx_struct.h:58-72`). Rust `GxrModeObj` declares the same sequence and widths (`ogc-gx-sys/src/lib.rs:158-175`). On the PowerPC C ABI, both layouts are 60 bytes; the 32-bit alignment inserts two bytes before `xfbMode` and pads the final record to four-byte alignment. The existing size assertions check 4 and 60 bytes (`ogc-gx-sys/src/lib.rs:245-257`). I found no mismatch in these two declarations.

### Accepted safety deviations

None identified in this bounded type-declaration change.

### Uncovered scope and validation limits

This pass did not inventory or compare all `GX` public declarations, constants, or register behavior. The current GX note records wider API and state findings, and the two new records do not close either GX crate. The implementer reports that the PowerPC test code compiled; tests were not run. I did not run a fresh target build or C layout program. No hardware verification was performed.

## 2026-10-05 full-scope classification

`ogc-gx` and `ogc-gx-sys` remain `parity_blocked`. The current C reference is sufficient to name the remaining gaps. Rust still exposes only a subset of C `gx.h` and does not reproduce the full GX shadow/dirty-state machine for TEV, indirect stages, lighting, and related state. The previously named light initializer examples (`GX_InitLightDistAttn`, `GX_InitLightAttn`, `GX_InitLightAttnA`, `GX_InitLightAttnK`, and `GX_InitLightSpot`) are now implemented, together with `GX_InitSpecularDirHA`, `GX_LoadLightObj`, and `GX_LoadLightObjIdx`; broader API/state coverage remains the blocker. The earlier texture scale/dirty-state/TPL and `GxTexReg`/`GxrModeObj` fixes remain valid. No additional C source is needed for these blockers.

## 2026-10-05 light API coverage pass

The Rust GX layer now implements the previously named light API gap for `GX_InitLightDistAttn`, `GX_InitLightAttn`, `GX_InitLightAttnA`, `GX_InitLightAttnK`, and `GX_InitLightSpot`. The same pass also adds the related missing `GX_InitSpecularDirHA`, `GX_LoadLightObj`, and `GX_LoadLightObjIdx` operations. The distance-attenuation and spot-light coefficient formulas follow `gx.c` directly, including C's invalid-parameter fallback to the OFF modes. `GX_LoadLightObj` emits the complete 16-word XF light block at `0x600 + light_index*0x10`, and indexed loads emit the C `0x38` command word.

`ogc-gx-sys` now exposes the corresponding `GX_DA_*` and `GX_SP_*` constants. Focused target tests cover attenuation coefficient updates and C's light-ID fallback mapping; hardware FIFO execution still needs independent validation.

This is a bounded API-coverage improvement only. `ogc-gx` and `ogc-gx-sys` remain `parity_blocked`: the Rust public surface is still substantially smaller than `gx.h`, and the crate still does not reproduce C's complete TEV/indirect/light/related shadow and dirty-state machine. The five named light-initializer examples are therefore no longer open blockers, but the broader API/state blocker remains.

## 2026-10-05 texture-coordinate shadow-state pass

Rust now implements `GX_SetTexCoordScaleManually`, `GX_SetTexCoordCylWrap`, and `GX_SetTexCoordBias`. It maintains per-coordinate SU S/T shadow words for all eight texture coordinates, tracks the manual-scale bitmask, preserves the C lower-16-bit size update and bit-16/bit-17 bias/wrap updates, and emits BP writes only when C would emit them. Disabling manual scaling changes only the manual-state bit and emits no immediate register write, matching C.

The configured Rust 2D texture path also records its coordinate-0 scale words into the same SU shadow state so later manual operations observe the register values that Rust emitted. Focused tests cover the C bitfield update behavior.

This is still not a full replacement for libogc's `__gx` shadow state. Channel/light state, performance counters, FIFO lifecycle, vertex descriptor state, copy/pixel state, and many other `gx.h` entry points remain outside the current Rust state model, so both GX crates remain `parity_blocked`.

## 2026-10-05 TEV / indirect / texgen API pass

The 30-function TEV / indirect / texgen family identified by the public `gx.h` audit now has direct Rust counterparts. This includes `GX_SetTevOp`, TEV color/alpha inputs and operators, TEV order/count and konst/swap state, `GX_SetNumTexGens`, `GX_SetTexCoordGen`/`GX_SetTexCoordGen2`, `GX_SetZTexture`, the indirect-stage count/order/scale/matrix operations, and all `GX_SetTevInd*` helpers.

The implementation uses explicit BP/XF/CP shadow words that match the register IDs initialized by C: TEV color/alpha environments at `0xc0..0xdf`, raster/order state at `0x25..0x2f`, swap/konst state at `0xf6..0xfd`, and generation mode register `0x00`. `GX_SetNumTevStages`, `GX_SetNumIndStages`, `GX_SetNumTexGens`, and texgen matrix state use a deferred dirty path; Rust `begin()` flushes generation mode first, then XF texgen/count state and matrix-index CP/XF state before emitting the primitive command, matching the relevant ordering in C `__GX_SetDirtyState()`/`GX_Begin()`.

The existing configured 2D helper now synchronizes its direct generation-mode, stage-0 TEV, raster-order, swap-table, and matrix-index writes into these shared shadows. This prevents a later public TEV/texgen setter from preserving stale state that differs from what Rust actually emitted.

The uploaded public-API audit counted 150 C `GX_*` functions without direct Rust counterparts before this pass. These 30 direct counterparts reduce that mechanical missing-function backlog to 120; this count is only API-surface progress and does not imply behavioral parity for the remaining GX subsystem.

Accepted safety deviations in this bounded pass: invalid raster-color IDs that would index past C's private lookup table are mapped to the null/alpha-bump selection instead of performing an out-of-bounds read, and `set_ind_tex_matrix` returns without writing when given a null matrix pointer instead of dereferencing it as C would. Valid-input register behavior follows C.

Both `ogc-gx` and `ogc-gx-sys` remain `parity_blocked`. The remaining public API backlog still includes FIFO/synchronization, vertex descriptors/display lists, matrix loading, channel state, raster/depth/blend/pixel state, fog, EFB copy, PE poke/peek, texture preload/invalidation, metrics/thread/write-gather APIs, and four existing texture convenience functions whose Rust signatures are not C-shaped. Full `__gx` dirty/shadow behavior for those families is also still open.

## 2026-10-05 raster/depth/blend/pixel-state pass

The complete 15-function raster/depth/blend/pixel family from the uploaded GX API audit is now implemented: `GX_SetZMode`, `GX_SetZCompLoc`, `GX_SetLineWidth`, `GX_SetPointSize`, `GX_SetBlendMode`, `GX_SetCullMode`, `GX_SetCoPlanar`, `GX_EnableTexOffsets`, `GX_SetClipMode`, `GX_SetAlphaCompare`, `GX_SetColorUpdate`, `GX_SetAlphaUpdate`, `GX_SetPixelFmt`, `GX_SetDither`, and `GX_SetDstAlpha`.

The Rust state model now carries C-shaped PE Z-mode, color-mode 0/1, PE control, and line/point-width shadow words in addition to the existing generation-mode and SU shadows. Immediate BP/XF writes match `gx.c`; cull-mode and multisample generation-mode changes use the existing deferred generation-mode flush so later `GX_Begin` observes the same shadow update path. The configured 2D helper also seeds the PE Z/blend shadows when it writes those registers directly.

`GX_SetPixelFmt` reproduces libogc's `realfmt` mapping, including Y/U/V format selection through `peCMode1`, and updates the generation-mode multisample bit. `GX_EnableTexOffsets` shares the per-coordinate SU S-size shadow with the earlier manual scale/bias/wrap APIs, so these operations preserve one another's fields.

Two invalid-enum cases are intentionally safer than C undefined behavior: `set_cull_mode` ignores values outside the four-entry C lookup table, and `set_pixel_fmt` rejects pixel-format values outside the eight-entry `realfmt` table. Valid inputs retain C behavior.

This pass removes 15 more direct-function gaps. Combined with the earlier 30-function TEV/indirect/texgen pass, the uploaded audit's original 150-function missing backlog is now 105. Both GX crates remain `parity_blocked`; channel state, FIFO/synchronization, vertex/display-list, matrices, fog, EFB copy, PE poke/peek, texture preload/invalidation, metrics/thread/write-gather, four C-signature mismatches, and remaining shadow-state behavior are still open.

## 2026-10-05 channel and EFB copy pass

The four missing channel-state APIs are now implemented: `GX_SetChanCtrl`, `GX_SetChanAmbColor`, `GX_SetChanMatColor`, and `GX_SetNumChans`. Ambient/material partial updates share one preservation helper so color-only and alpha-only writes preserve the untouched channel component exactly as C does. Channel color/control state is deferred through one XF dirty flush matching `__GX_SetChanColor` and `__GX_SetChanCntrl`; `GX_SetNumChans` updates both generation-mode state and the XF channel-count register on the next dirty-state flush.

The 16-function EFB copy / field / pixel-sync family is also implemented. Display and texture copy state share the existing copy geometry/filter helpers plus one common copy-emission path for C's temporary clear/depth/pixel-engine overrides and restoration. The new API covers field masks/modes, C-compatible Y-scale calculation, display-copy source/destination/control/filter state, texture-copy source/destination state, display/texture copies, pixel-mode synchronization, and bounding-box clearing.

Safety additions are limited to undefined-pointer/input boundaries: enabled copy-filter tables must be non-null, null copy destinations are rejected instead of converted to a physical address, zero dimensions in `GX_GetYScaleFactor` return a benign `1.0`, and non-finite/non-positive `GX_SetDispCopyYScale` inputs return zero without issuing an invalid copy-scale write. Normal valid inputs preserve the C register/state behavior.

These 20 functions reduce the uploaded audit's original 150-function missing direct-API backlog from 105 after the raster/pixel pass to 85. Both GX crates remain `parity_blocked`: FIFO/synchronization, vertex/display-list, matrix, fog, PE poke/peek, texture preload/invalidation, metrics/thread/write-gather, initialization/FIFO, one standalone display-list call, four C-signature mismatches, and remaining shadow-state behavior are still open.

## 2026-10-05 fog and matrix API pass

This pass implements thirteen more direct `gx.h` counterparts: `GX_SetFog`, `GX_InitFogAdjTable`, `GX_SetFogRangeAdj`, `GX_SetFogColor`, `GX_SetViewportJitter`, `GX_LoadPosMtxImm`, `GX_LoadPosMtxIdx`, `GX_LoadNrmMtxImm`, `GX_LoadNrmMtxImm3x3`, `GX_LoadNrmMtxIdx3x3`, `GX_LoadTexMtxImm`, `GX_LoadTexMtxIdx`, and `GX_SetCurrentMtx`.

The matrix functions share row-major flattening helpers rather than duplicating FIFO packing. Immediate position and texture matrices reuse the same 3x4 encoder, normal matrices reuse one 3x3 encoder, and 2x4 texture matrices reuse one two-row encoder. `GX_SetCurrentMtx` uses the existing deferred matrix-index shadow/dirty path, so it composes with the texgen state already ported rather than adding a second matrix-index mechanism.

Fog register construction is likewise shared: the fog color BP word is built by one helper used by both `GX_SetFog` and `GX_SetFogColor`, and fog-range table emission loops over the five adjacent register pairs instead of duplicating five packing expressions. Perspective and orthographic fog coefficient calculations follow `gx.c`, including the odd-ball equal-near/far or equal-start/end behavior.

### Safety divergences in this pass

- `GX_InitFogAdjTable`: C dereferences both pointers unconditionally. Rust returns without writing when either `table` or `projection` is null. This avoids an invalid-pointer dereference and is an accepted safety divergence.
- `GX_InitFogAdjTable`: Rust also returns for `width == 0` rather than performing division by zero and then a float-to-integer conversion with platform-dependent/invalid results. Normal nonzero-width behavior follows C.
- `GX_SetFogRangeAdj`: when range adjustment is enabled, C dereferences `table` unconditionally. Rust returns without emitting the range-adjust registers when `table` is null. Disabled mode still ignores the table pointer exactly as C does.
- Matrix inputs use Rust references for immediate matrices, which makes null matrix pointers unrepresentable at this Rust API layer. This is a safety/API-shape improvement; the emitted XF data and indexed command words match the C operations for valid inputs.

No safety divergence is used to broaden accepted enum/index ranges. Matrix index fields retain C's low-bit masking/truncation behavior where the C macros define it.

`GX_AdjustForOverscan` and `GX_CallDispList` are deliberately not included in this commit. The former depends on the video-mode structure surface and the latter depends on FIFO/display-list flush context (`xfFlush`, `xfFlushExp`, `vcdClear`, save-DL context) that is not yet represented. Implementing either partially would create a new behavioral mismatch.

After the previously completed 65 functions, these 13 reduce the mechanical direct-function backlog from 85 to 72. `ogc-gx` and `ogc-gx-sys` remain `parity_blocked` because FIFO/initialization, vertex/display-list, overscan, PE poke/peek, texture preload/invalidation, metrics/thread/write-gather, four signature-shape mismatches, and broader state/ABI coverage remain open.

## 2026-10-05 PE poke / peek API pass

The complete 12-function PE poke/peek family from the uploaded public-API audit is now implemented: `GX_PokeAlphaMode`, `GX_PokeAlphaUpdate`, `GX_PokeColorUpdate`, `GX_PokeDither`, `GX_PokeBlendMode`, `GX_PokeAlphaRead`, `GX_PokeDstAlpha`, `GX_PokeARGB`, `GX_PeekARGB`, `GX_PokeZ`, `GX_PeekZ`, and `GX_PokeZMode`.

The implementation is deliberately shared rather than function-specific. A single PE-register accessor handles the 16-bit PE MMIO block, a single EFB address helper reproduces C's x/y/plane address packing, a one-bit update helper is reused by alpha/color/dither controls, and one blend-word encoder implements the shared PE blend register field updates. `GX_READ_00`, `GX_READ_FF`, and `GX_READ_NONE` are now present in both GX constant surfaces.

`GX_PokeZMode` preserves libogc's exact source behavior, including the final assignment that discards the compare-enable/function bits and leaves only the update-enable bit. Although that source sequence appears unusual, it is observable reference behavior and is not treated as a bug to be corrected by the Rust port.

### Safety divergences in this pass

- `GX_PeekARGB`: C unconditionally dereferences the output `GXColor*`. Rust accepts the raw pointer for ABI-shape parity but returns without reading/writing when it is null, avoiding an invalid write.
- `GX_PeekZ`: C unconditionally dereferences the output `u32*`. Rust returns without reading/writing when the pointer is null.

No other PE poke/peek behavior is intentionally changed. Coordinate masking, color/Z packing, PE register read-modify-write behavior, and blend/alpha mode field truncation follow C for valid inputs.

These 12 functions reduce the mechanical direct-function backlog from 72 to 60. `ogc-gx` and `ogc-gx-sys` remain `parity_blocked`; FIFO/initialization, vertex/display-list, overscan, texture preload/invalidation, metrics/thread/write-gather, the remaining standalone APIs, four C-signature mismatches, and broader state/type coverage remain open.

## 2026-10-05 texture / TLUT invalidation and preload pass

The complete five-function texture/TLUT family from the uploaded public-API audit is now implemented: `GX_InvalidateTexAll`, `GX_InvalidateTexRegion`, `GX_LoadTlut`, `GX_LoadTexObjPreloaded`, and `GX_PreloadEntireTexture`. The existing higher-level `invalidate_texture_cache` helper now delegates to the direct `GX_InvalidateTexAll` counterpart instead of maintaining a second invalidation sequence.

The implementation reuses the existing texture-object and region encodings rather than introducing a parallel representation. `GX_LoadTexObjPreloaded` assigns the per-map texture/TMEM register IDs directly into the existing object and region words, emits the same six base BP writes as C, and uses the installed TLUT-region callback for color-indexed textures. `GX_LoadTlut` updates the existing `GxTlutRegion` cached base/address/entry state after the C-ordered preload writes. `GX_PreloadEntireTexture` shares a dedicated tile-shift helper and preserves libogc's source behavior for mip levels, including the reference implementation's use of width for both derived mip dimensions.

Texture-state synchronization is also centralized. A shared `flush_texture_state` emits the current TEV indirect mask around invalidation, TLUT load, and preload commands, matching C's `__GX_FlushTextureState`. The indirect mask is recomputed through one deferred dirty path from the already-ported indirect-stage count/order state before `GX_Begin` or an explicit texture-state flush. This avoids hard-coding the initialization mask and keeps texture APIs consistent with later indirect-state changes.

### Safety divergences in this pass

- `GX_InvalidateTexRegion`: C dereferences the supplied region unconditionally. Rust returns `GxTextureError::InvalidRegion` when the pointer is null.
- `GX_LoadTlut`: C assumes a valid TLUT object and a configured callback returning a valid region. Rust returns `InvalidObject` or `InvalidRegion` for a null object, missing callback, or null callback result instead of dereferencing invalid memory.
- `GX_LoadTexObjPreloaded`: C indexes private eight-entry register tables with `mapid` and dereferences `obj`/`region` unconditionally. Rust rejects null object/region pointers and map IDs outside 0..=7. For color-indexed textures, a missing/null TLUT-region callback result is reported as `InvalidRegion` rather than dereferenced.
- `GX_PreloadEntireTexture`: C dereferences both pointers unconditionally. Rust rejects null object/region pointers before emitting preload commands.

These checks apply only to C undefined-behavior boundaries. Register assignment, invalidation sizing, TLUT state updates, mip/TMEM address progression, and valid-input FIFO ordering follow `gx.c`.

This family removes five more direct-function gaps, reducing the mechanical backlog from 60 to 55. Both `ogc-gx` and `ogc-gx-sys` remain `parity_blocked`; FIFO/initialization, vertex/display-list, overscan, metrics/thread/write-gather/diagnostics, standalone APIs, four C-signature mismatches, and broader state/type coverage remain open.

## 2026-10-05 GX public-surface completion pass

The uploaded `gx.h` audit's remaining direct public-function backlog is now exhausted. The final implementation pass adds the vertex/VAT/display-list family, overscan and the exact frame-to-field API spelling, FIFO initialization and access, draw/token/breakpoint synchronization and callbacks, `GX_CallDispList`, write-gather redirection/restoration, GP/XF/RAS/vcache metrics, GX-thread ownership, overflow accounting, bounding-box reads, and the remaining initialization/diagnostic entry points. A mechanical audit against all 243 public `GX_*` entries recorded in the uploaded checklist finds a direct Rust counterpart for every audited name.

The four previously identified signature-shape mismatches are also reconciled. `init_tex_obj`, `init_tex_obj_ci`, `init_tex_obj_lod`, and `get_tex_obj_all` now expose the C parameter shapes. The former Rust convenience forms remain available under explicit helper names (`init_tex_obj_from_init`, `init_tex_obj_ci_from_init`, `init_tex_obj_lod_from_params`, and `get_tex_obj_info`) so internal callers can stay DRY without occupying the C-compatible names.

### Shared state and DRY implementation

Vertex descriptors and formats use one VCD/VAT shadow model and one encoder/decoder path. Display-list recording redirects the existing low-level `pipe_u8`/`pipe_u16`/`pipe_u32`/`pipe_f32` command sink instead of duplicating each GX command emitter, so register loads, primitives, immediate-mode data, and later APIs record the same command bytes through one path. `GX_MT_DL_SAVE_CTX` now defaults enabled like C; one centralized `GxShadowSnapshot` captures and restores all modeled render shadows around display-list recording.

FIFO operations use one `FifoSnapshot` representation and one unaligned-safe serializer for the public 128-byte opaque `GxFifoObj`. CPU/GP FIFO programming, hardware refresh, link detection, write-gather redirection, and the public FIFO getters/setters all reuse this representation. The serializer intentionally avoids casting the byte-aligned public object to an aligned private C struct, which would create Rust alignment UB while providing no valid-input behavioral benefit.

CP/PE synchronization is likewise centralized. The CP interrupt handler now matches C's high-water/low-water policy: overflow increments the counter, disables high-water interrupts, enables low-water interrupts, and suspends the current GX thread; underflow resumes that thread and restores high-water monitoring. Draw-done waiting uses an LWP queue and PE-finish broadcast like C rather than busy-waiting during normal operation. Abort delays use the Rust-owned timebase/LWP tick source with the same 50-tick and 5-tick waits as `__GX_WaitAbort`. GX interrupt installation is performed with external IRQ delivery disabled so the three-handler set cannot be partially acquired by a scheduler/IRQ race.

The metrics family shares CP counter-pair readers and common perf-mode mapping/programming paths. C quirks are preserved where observable: `GX_SetVCacheMetric` remains a no-op, `GX_ReadGPMetric` leaves `cnt1` untouched because the C implementation does, and the unusual negative-distance assignment in `GX_InitFifoPtrs` is retained exactly.

### Safety divergences in the final surface pass

- Vertex descriptor/format list APIs reject null list pointers instead of dereferencing them. Vertex-format indices are restricted to the eight hardware VAT slots rather than allowing invalid C shifts/table accesses.
- `GX_BeginDispList` rejects null, zero-sized, or non-32-byte-aligned destination buffers and nested capture instead of programming an invalid FIFO. Overflow returns zero from `GX_EndDispList` as C does for a wrapped list.
- `GX_AdjustForOverscan` rejects null mode pointers and an input mode with zero XFB height instead of dereferencing or dividing by zero.
- Metric/status readers write only through non-null output pointers. `GX_ReadGPMetric` returns a benign zero clip ratio when the C denominator register is zero rather than performing integer division by zero.
- `GX_Init` rejects a null or non-32-byte-aligned FIFO base and FIFO sizes below the C minimum or not divisible by 32. It no longer imposes the higher-level Rust MEM2 helper's 1 MiB maximum on the C-shaped direct entry point.
- FIFO object functions tolerate null object/output pointers by returning/doing nothing rather than dereferencing them. The opaque object is read/written unaligned-safely as described above.
- GX initialization refuses to replace PE/CP interrupt handlers already owned by another subsystem. This prevents silently clobbering an existing IRQ owner; C assumes those interrupts are available. Registration itself is transactional with respect to scheduler/IRQ interleaving.
- `GX_EnableBreakPt` rejects a null breakpoint address rather than programming address zero.
- `GX_RedirectWriteGatherPipe` rejects null or non-32-byte-aligned destinations and refuses redirection during software display-list capture rather than programming malformed write-gather state.
- The C-shaped texture-object initialization names retain the pre-existing Rust safety behavior at undefined-input boundaries: null writable objects are rejected where the helper can do so safely, invalid min-filter values use the safe conversion fallback instead of indexing outside C's private filter table, and `GX_GetTexObjAll` skips null output pointers instead of dereferencing them.

These are accepted safety differences only. They do not justify treating unrelated valid-input behavioral differences as parity.

### Previously recorded GX behavioral blockers

The two previously recorded `ogc-gx` behavioral blockers were incomplete `GX_Init`/`__GX_InitGX` default-state parity and display-list CPU-FIFO visibility. Both are closed by the 2026-10-06 behavioral completion pass below. `ogc-gx-sys` remains separately `parity_blocked` pending a complete constants/types/layout audit against `gx.h`; the function surface is intentionally implemented by `ogc-gx` rather than duplicated as unresolved C externs in the sys crate.

## 2026-10-06 behavioral completion pass — GX_Init and display-list FIFO state

The two remaining recorded non-safety `ogc-gx` blockers are fixed at source level. `GX_Init` now follows libogc's complete `__GX_InitGX` default-state sequence after the existing low-level FIFO/register initialization. The sequence deliberately composes the already-ported public setters instead of duplicating their BP/CP/XF encodings: texture generators, VCD/cache state, line/point state, SU offsets, identity matrices, viewport/scissor, channel state, default texture/TLUT regions and callbacks, TEV/indirect defaults, fog, blend/depth/pixel state, field/copy state, PE poke defaults, and GP metric reset all flow through the same implementations used by normal callers. This keeps initialization behavior DRY with the public API surface.

The default texture-region callbacks now match both libogc platform variants. Wii/Hollywood uses the 24-entry fixed-region policy, including CI-format priority over mipmap selection. GameCube uses the rotating A/B region pools from the C callback. The corresponding 24 texture regions and 20 TLUT regions are initialized with the same TMEM layout before the callbacks are installed.

Display-list capture now mirrors the observable CPU-FIFO state from C. `GX_BeginDispList` saves the ordinary CPU FIFO and installs a synthetic display-list FIFO shadow whose base/end/write/count/wrap fields track the software capture sink. `GX_GetCPUFifo` therefore reports the active display-list FIFO during capture, including the live write/count state, while `GX_EndDispList` restores the saved CPU FIFO before returning. Command bytes still use the single shared low-level capture sink, so this parity fix does not duplicate command emission or program a second hardware FIFO path.

### Safety divergences retained in this pass

- The default texture-region callback returns null for a null texture object or a map ID outside 0..=7 instead of dereferencing/indexing invalid memory as C would.
- The default TLUT-region callback returns null for names outside the 20 initialized default regions instead of indexing out of bounds.
- Existing `GX_Init` input validation remains stricter than C at undefined-input boundaries: null/misaligned FIFO storage and invalid FIFO sizes are rejected, and existing interrupt owners are not overwritten.
- Existing display-list safety checks remain: null/misaligned/zero-size buffers and nested capture are rejected rather than programming invalid FIFO state.

These safety checks do not alter valid-input register/state behavior. No confirmed non-safety blocker remains in the recorded `ogc-gx` scope, so `ogc-gx` moves from `parity_blocked` to `needs_re_review`. Runtime FIFO/display-list behavior and the expanded initialization sequence still require independent PowerPC/hardware validation before `reviewed`. `ogc-gx-sys` remains separately `parity_blocked` for the outstanding complete `gx.h` constants/types/layout audit; no unresolved libogc C symbols are introduced.

## 2026-10-06 `ogc-gx-sys` constants/types/layout completion pass

The remaining sys-layer blocker is closed at source level. A mechanical comparison against the uploaded `gc/ogc/gx.h` now finds all **673** public object-like `GX_*` macros represented by `ogc-gx-sys`; the previous missing set of 180 aliases/constants is fully ported. Recursive numeric comparison found no value mismatches among resolvable definitions. The small set of C expressions that depend on private `_GX_TF_*` mask macros expands to the numeric texture-format values already present in Rust, and the TEV/TLUT aliases resolve identically.

Exact public C typedef spellings (`GXColor`, `GXColorS10`, `GXTexObj`, `GXTlutObj`, `GXTexRegion`, `GXTlutRegion`, `GXLightObj`, `GXVtxDesc`, `GXVtxAttrFmt`, `GXFifoObj`, `GXTexReg`, `GXFogAdjTbl`, `GXRModeObj`, and callback typedefs) now alias the existing single Rust layout definitions rather than duplicating structs. The PowerPC layout tests cover every public GX structure, including color, vertex descriptor/format, fog table, texture/TLUT objects and regions, light object, FIFO object, render mode, and `Vtx`.

The public `wgPipe` declaration is represented as a Rust constant pointer to the hardware write-gather address `0xCC008000`. This preserves the public data surface without adding an unresolved libogc data symbol, consistent with the repository rule that the C tree is reference-only.

No new safety divergence is introduced by this sys pass: it adds constants, aliases, layout checks, and the fixed hardware pointer only. `ogc-gx-sys` therefore moves from `parity_blocked` to `needs_re_review`, pending independent target compilation/review.

## 2026-10-07 HBC solid-quad vertex stream correction

HBC's C renderer enables direct POS, CLR0, and TEX0 attributes for every quad. Its `plot_vert_c` path writes a zero S/T pair after each solid vertex (`../../C-sources/hbc/channel/channelapp/source/gfx.c:310-313,146-157`). The Rust `configure_solid_2d` path previously disabled TEX0 and emitted only position and color. It now uses the same VAT/VCD layout and TEX0 generator as the textured path, and writes the matching zero S/T pair for solid vertices. The solid TEV operation still uses the raster color and does not sample a texture.

The PowerPC startup diagnostic build that uses this helper compiled and converted to DOL. `ogc-gx` unit tests contain fixed checks for the C-compatible vertex descriptor and VAT words, but could not run on the host because this workspace's GX dependencies require the Wii target. HBC's 228 host tests passed, but do not execute GX hardware commands. Hardware verification of the rendered frame is pending; the source fix is not yet confirmed to resolve the startup display failure.

## 2026-10-28 GX scale, FIFO, and API safety review

The current pass reviewed the video-probe failure, display-copy scaling and
framebuffer bounds, FIFO pointer initialization, the direct scale setter, and
raw-pointer texture conversion. It also scanned the remaining public GX source
for ignored inputs, wrapping arithmetic, fallback results, and unfinished paths.
This is not a new complete line-by-line parity review of every GX API. Earlier
review notes remain historical; the changes below require independent review.

- `GX_GetYScaleFactor(480, 240)` follows the C adjustment loop into unsigned
  wraparound because no scale yields fewer than 240 output lines. The Rust
  code could also divide by zero while reducing an odd scale. The production
  helper now bounds the search and rejects zero, out-of-range, and unreachable
  dimensions. The C-shaped `get_y_scale_factor` retains its documented 1.0
  fallback for rejected inputs. The reference C code has the same unbounded
  search and odd-scale divide-by-zero risks; avoiding them is a safety
  difference. `FrameConfig::new` also rejects a scale that would produce more
  lines than the declared framebuffer height. The output for 240 to 480 is
  scale 127 and 482 lines, not the earlier unexecuted test's expected 128.
- The direct scale setter previously used `256 > scale` for its control bit.
  C subtracts an unsigned value, so the bit is set for all scales except 256.
  The direct setter and frame-copy control now share that calculation.
- For wrapped FIFO pointers, C assigns a distance to the read pointer instead
  of keeping the original address. Rust reproduced that behavior and could
  program an invalid hardware address. The Rust path now validates both
  pointers against the FIFO and preserves them while computing the wrapped
  distance. This is a deliberate safety difference from the C source. Invalid
  pointers leave the object unchanged; the C-shaped void API cannot report an
  error.
- `make_texture565` dereferences caller-supplied raw pointers. It is now an
  unsafe function with its buffer, stride, and non-overlap requirements
  documented. No workspace call sites were found. This is an API change for
  downstream callers, who must supply the existing pointer guarantees.

`tests/ogc-gx-host-tests` exercises the same scale and FIFO rules used by the
production crate. It checks fixed C-compatible scale values, unsupported
sizes, the odd-scale case, scale control bits, and wrapped pointer addresses.
A target-only test also exercises the FIFO object update and rejection path.
The video probe now checks a valid scale and the rejected input separately, with
step records before both. The report-enabled video entry still lacks a qualified
activity indicator during its own framebuffer and callback changes; do not use
it on Wii until the instrumentation and recovery conditions in DEVELOPER.md are
met. Host and Dolphin results do not verify physical GX FIFO, framebuffer, or
recovery behavior.

Dolphin 2609a ran the current video probe with DOL SHA-256
`039f80311ec909b8f5f7631487d70265234a0b1e27911efad496d1dec5c7f55a`.
The report records 27 passes and no failures, including valid and rejected
scale inputs and a wrapped FIFO object with real probe-buffer addresses.
The report is retained at
`/Volumes/SSD/WiiDev/Devtools/reports/evidence/libogc-gx-review-video-final-v4-2026-10-28/suite.json`.
This proves the exercised DOL behavior in Dolphin; it does not prove physical
Wii GX behavior. No new Wii run was performed after the fixes.

The final expanded Dolphin probe also verifies that an out-of-range FIFO
pointer leaves the prior object unchanged. DOL SHA-256
`2b0f8974fd817255a9728140f033652f686be7968d12db9aa2a20289e0c42335`
reported 28 passes and no failures. Its report is
`/Volumes/SSD/WiiDev/Devtools/reports/evidence/libogc-gx-review-video-final-v5-2026-10-28/suite.json`.
This supersedes the earlier 27-check Dolphin run for this probe revision.
