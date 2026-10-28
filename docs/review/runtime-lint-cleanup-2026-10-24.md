# Runtime lint and evidence cleanup — 2026-10-24

The cleanup keeps the configured lint levels and completeness requirements.
It repairs source references and changes the Rust APIs that exceeded Clippy's
argument limit to use parameter structs. The C entry point signatures remain
unchanged.

## Rust API changes

| Functions | Parameter type |
| --- | --- |
| `arq::post_request` and `arq::post_request_async` | `ArqTransfer`; the asynchronous callback remains a separate argument |
| `gx::init_tex_obj` and `gx::init_tex_obj_ci` | Existing `GxTexObjInit` |
| `gx::init_tex_obj_lod` | Existing `GxTexObjLod` |
| `gx::get_tex_obj_all` | `GxTexObjOutputs` |
| `gx::set_tev_indirect` | `GxTevIndirect` |
| `gx::set_tev_ind_tile` | `GxTevIndirectTile` |
| `filesystem::get_attr_async` | `FsAttributeOutputs` |
| `filesystem::set_attr_async` | `FsAttributes` |

The current SDK callers and HBC's texture initialization use these signatures.
The existing texture packing tests retain their independent expected words.

CARD submission errors keep their public accessors and ownership contract.
Their payload is now boxed to avoid returning a large error value on the stack.
The production error type is shared with host tests that check the status,
original payload allocation, and ownership transfer. Failed metadata-state
installation returns its existing box rather than copying the large payload.

## Evidence migration

The evidence file uses exact source lines to distinguish duplicate method
names. The refresh preserves each original function's order and visibility.
It does not match an overloaded API only by its name.

The old public USB `shutdown` function is no longer in the production source.
Its record is retired because shutdown now uses the slot-specific
`DiscInterface.shutdown` callbacks and the private `disc_shutdown(slot)`
backend. The storage probe already exercises shutdown through that interface.
The current public `flush_drive` API has a direct probe reference in
`usb_volumes_probe`. That probe builds; physical two-drive acceptance is pending.

Drive-state records now point to tests of the production `DriveState` engine.
The tests check initial state, independent device identities, removal,
publication failure after removal, and one drive surviving another's close.
The generated inventory and status report come from the existing generator.

## Verification scope

Check results and logs are recorded in
`/Volumes/SSD/WiiDev/Devtools/reports/DEVELOPMENT_RESULTS.md` and its evidence
directory. Compile checks, host tests, and a linked probe establish their stated
scope. They do not establish physical Wii acceptance.

The strict completeness check still requires evidence for every public API.
The previous generated report already contained 2,371 unverified entries.
This cleanup repairs freshness and adds valid evidence; it does not turn
the remaining verification gaps into passes.
