# Independent `iso9660` finding audit

Status: **historical/superseded**. HEAD `4c36294` now registers/removes the ISO9660 devoptab, implements the file/directory/stat callbacks, and exports the C ABI entry points. The original audit is retained below as historical evidence.

Reviewer: `/root/reviewer_6`  
Original reviewer: `/root/reviewer_4` (finding in `docs/review/libiso9660.md`).  
Scope: independent source verification of devoptab registration/callbacks and C ABI exports.  
Validation: source inspection only; no build or tests run. No hardware verification was performed.

## Result

At the time this audit was written, the prior findings were confirmed against the then-current C and Rust sources. The status and correction sections now supersede that conclusion.

### Historical finding: Rust mount did not register a devoptab or provide file operations

C defines `dotab_iso9660` with open, close, read, seek, fstat, stat, chdir, directory open/reset/next/close, and statvfs callbacks (`../../C-sources/libogc/libiso9660/iso9660.c:749-774`). `ISO9660_Mount` allocates and copies that table, stores the mount descriptor in `deviceData`, and calls `AddDevice` (`:946-993`). The file and directory callbacks resolve the registered device through the devoptab table, for example `_ISO9660_getMountDescrFromPath` calls `GetDeviceOpTab` and checks the registered open callback (`:776-790`). `ISO9660_Unmount` calls `RemoveDevice` and frees the directory tree and mount descriptor (`:997-1021`).

Rust `mount_impl` only checks the disc callbacks, parses a volume identifier, and inserts a `name -> Mount { volume_id }` entry into `MOUNTS` (`crates/media/iso9660/src/lib.rs:238-269`). There is no devoptab registration, filesystem callback table, or path/file/directory implementation in this crate. `unmount_impl` only removes the map entry (`:271-278`). Thus Rust can report a successful mount and return a label, but it does not create the C-visible `name:/...` filesystem device or provide its file and directory operations.

### Historical finding: Rust did not export the declared C API symbols or calling convention

The C header declares `ISO9660_Mount`, `ISO9660_Unmount`, and `ISO9660_GetVolumeLabel` with C linkage (`../../C-sources/libogc/gc/iso9660.h:15-25`), and the C implementation defines those symbols (`../../C-sources/libogc/libiso9660/iso9660.c:946,997,1024`). Rust has similarly purposed functions `iso9660_mount`, `iso9660_unmount`, and `iso9660_get_volume_label` (`crates/media/iso9660/src/lib.rs:303-341`), but they use Rust ABI and Rust symbol mangling: they have neither `extern "C"` nor `#[no_mangle]`/`export_name`, and the names differ from the C declarations. The manifest declares no `staticlib` crate type (`crates/media/iso9660/Cargo.toml:1-11`). A C caller cannot link the documented names to these Rust functions.

## Conclusion

Both findings in reviewer `/root/reviewer_4`'s note were open at the time. The current-source correction below supersedes that status. This addendum is separate from `docs/review/libiso9660.md` to avoid editing another reviewer's note. No implementation code was changed.


## 2026-10-02 current-source correction

Both findings in this historical audit are closed in the current source. `mount_impl` builds and registers a `Devoptab`, `unmount_impl` removes it, and the callback table implements open/close/read/seek/fstat/stat/chdir/directory/statvfs behavior. The public `ISO9660_Mount`, `ISO9660_Unmount`, and `ISO9660_GetVolumeLabel` C symbols are also present. Real-media validation remains open, but the missing-source-surface conclusion above is stale.
