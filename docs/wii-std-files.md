# Wii std mounted FAT files

Updated: 2026-10-15. Filesystem and path adapters are connected. Source and build
checks do not establish Wii hardware acceptance or application readiness.

## Production path and ownership

`sys/fs/wii.rs` calls the application-linked `__wii_std_fs` provider in
`ogc-std-runtime`. The provider owns a typed `ogc-fat::FatFile`. It uses the
same FAT engine, block transports, media checks, mount references, and scheduler
locks as the existing production service. No C filesystem symbol is linked.
The adapter does not retain a raw pointer into the devoptab registration table.

A std file owns `Arc<Mutex<Handle>>`. `try_clone` retains the same open handle;
reads, writes, seeks, resize, metadata, and flush operations hold the shared
mutex. Therefore clones share one cursor and one final close. FAT keeps the
mount busy until the handle is released. The mutex uses the existing Wii std
futex backend and LWP transport. Hardware ordering and contention need Wii tests.

The existing FAT service rejects a second independent open when either handle
is a writer. It reports Newlib EBUSY. Clones are supported through shared
ownership. Coordinated independent writer opens remain a backend limit; this
adapter does not claim unrestricted desktop file semantics.

## Open flags, transfer counts, and errors

The six std OpenOptions fields are stored and translated once by the shared
`fs_abi.rs` rules. No alternative access mode is tried after an open failure.
Read/write access and append use exact Newlib flags. `create_new` sets
O_CREAT|O_EXCL and overrides truncate. Append with truncate is rejected unless
create_new is selected, as in the pinned Rust std implementation. Creation or
truncation without write or append access is rejected.

FAT offsets and file lengths are bounded by u32. Std's u64 and signed seek
arguments are checked before the cursor changes. Reads and writes return the
completed byte count. The shared sector-transfer engine returns an error when
no bytes completed; after a completed prefix it returns that prefix's count.
The next call attempts the remaining bytes and reports any continuing failure.
Zero-filled gaps and file extensions consume actual transfer counts. A retained
new start cluster stays pending in metadata if a later operation fails.
Allocation rollback errors are returned. A failed FAT link update keeps the
allocated target valid because a FAT mirror may already contain the link.

The typed FAT error is translated into the positive Newlib errno space. The
ABI returns its negative encoding, and std constructs `io::Error` with the
positive code. `raw_os_error()` preserves that Newlib code. IOS and LWP values
are not interpreted as Newlib values. Lower transport details that the current
FatError enum cannot represent remain an E01/E02 service gap.

## Flush, cleanup, and metadata

`flush`, `sync_data`, and `sync_all` reach the production file metadata and
transport path. The clean-file path also performs the transport flush. USB
uses the existing checked SCSI cache command. SD sector writes wait for their
transport completion; the service has no separate SD cache command. Dirty file
state clears only after metadata and transport operations succeed. The online
check runs before metadata writes and does not change dirty state. Clean
read-only files keep their size, cluster, and attributes; sync sends the USB
cache command without rewriting a directory entry. FAT32 FSInfo
remains advisory volume metadata handled by the existing unmount policy.

Typed close, typed Drop, and devoptab close use `close_file_state`. It calls
`flush_file_state`, then releases ownership even if the flush fails. Release
clears the magic value and mount pointer before removing the open-file registry
reference. A second close returns `BadDescriptor`; Drop cannot release twice.
The two operation paths are explicit flush and close, which reuses flush.
Failed typed close or FAT Drop updates `file_close_failures()` and
`last_file_close_errno()`. Std final Drop checks the close result and follows
its current panic-abort cleanup failure policy. Diagnostics and real cleanup
failures remain hardware gates. A checked explicit sync is required when an
application needs a returned persistence error before Drop.

Metadata returns the current file length, file/directory type, and read-only
state from the FAT entry and mounted volume. The open-file registry publishes
length changes after writes and resize, including partial changes on failure.
Path metadata uses that live length until final close, then reads persisted
metadata. A path query therefore sees a writer's new length before explicit flush. Root metadata checks the live
mount. Metadata reads stored access, modification, and creation dates. A zero
FAT date is absent and returns Unsupported. Invalid nonzero calendar fields
return an error. FAT dates use UTC in this port; FAT does not store a timezone.
Access dates have one-day resolution. Modification times have two-second
resolution. Creation times include the stored ten-millisecond fraction.

Path and file-handle permission changes update only the FAT read-only bit.
Timestamp setters validate both requested values before media access, preserve
unselected fields, and flush the medium. FAT supports years 1980 through 2107;
out-of-range requests return EINVAL. The port does not invent creation dates or
automatically obtain timestamps from an unavailable wall clock. Newly created
entries can have absent dates until explicitly set. Roots have no stored date
fields. Links and file locks remain Unsupported.

## Mounted path rules and remaining work

The Wii std path selector uses `/` separators and a Wii mount prefix. The
custom `Prefix::WiiMount` variant is an unstable addition to the pinned std
source; dependency assumptions about path prefixes still require a Wii review. For
example, `sd:/apps/file` has a prefix and a root, is absolute, and keeps `sd:`
when a rooted path is joined. Mount names contain ASCII letters, digits, `_`,
or `-`. OS strings retain their bytes. Filesystem operations reject embedded
NUL bytes, malformed prefixes, and extra colons.

The std adapter resolves relative paths through its application current
directory. The FAT provider still requires absolute mounted paths. Applications
must mount the required medium first. Current-directory initialization uses a
valid absolute loader path when available. See
[paths and environment](wii-std-paths-environment.md) for error behavior,
executable paths, HOME, TMPDIR, and semicolon-separated path lists.

Rename, recursive removal, and canonicalization are connected. Rename uses the
production volume lock, validates source and destination types, rejects open
handles and cross-volume moves, and permits replacement of a file or empty
directory. Same-entry rename succeeds without changing bytes. The replacement
retains the old target cluster chain until the source name is removed. Reported
write failures cause checked recovery of directory entries and parent pointers.
Recovery failures are returned. This is not a journal: power loss, an absent
medium, or a failed recovery can leave an alias or allocated orphan. Real-media
failure tests remain required.

Recursive removal uses an explicit iterator stack. It checks every child and
cleanup result and closes each iterator before it removes the directory. It
rejects mounted roots with EBUSY. A failure can leave a partly removed tree.
FAT has no links to follow. Canonicalization walks actual entries and returns
stored names, including lookup through generated short aliases. It validates
intervening directories rather than applying only lexical normalization.

## Directory traversal

`std::fs::read_dir` now owns a typed `FatDirectory` through the same provider.
The handle keeps its mount in use until final release. A mutex protects cursor
access and permits transfer between threads. The provider checks the output
buffer before it advances the cursor. It excludes `.` and `..`, checks each
returned name, and preserves filename bytes. A read error is returned once;
the std iterator then ends. End of directory remains stable on later calls.
Opening a file as a directory returns Newlib ENOTDIR.

Each entry owns its name and shares the input directory path. It does not keep
the directory handle open. `path()` and `file_name()` remain valid after the
iterator closes. `file_type()` returns the type read from FAT. `metadata()`
queries the current path through the production service and returns its errors.
There are no directory metadata writes during close. FAT Drop releases the
mount reference. Hardware acceptance and media-removal checks remain open.

## Verification and hardware probe

The shared host engines use independent expected flag values, byte arrays, and
seek positions. They test all 64 OpenOptions combinations, invalid paths and
widths, a controlled medium with partial read/write failures, metadata failure,
transport flush failure, dirty-state retention, retry, read-only metadata
preservation, offline checks, USB disconnection, and close release on failure.
The controlled medium
is explicit test input to the engine that production FAT calls directly.
It is not an acceptance model for FAT layout or hardware transport behavior.

`cargo xtask build-wii-std-probe` includes
`target/xtask/probes/wii_std_fs_probe.dol`. It uses actual std and typed FAT APIs.
The probe mounts SD when necessary and obtains
`sd:/wii-std-fs-probe.tmp` with create_new. An existing file causes failure and is
preserved. Only a file obtained by that exclusive open is removed by cleanup.
The probe also checks directory creation/removal, empty traversal, stored entry
names and bytes, entry metadata, stable end of traversal, ENOTDIR/ENOENT, mount
ownership, and entry use after the iterator closes. Removed paths must be absent. An existing probe directory causes failure and is preserved.
The probe checks fixed file bytes, flags, cursor sharing, a real std worker,
resize and zero fill, append after seek, sync, access errors, and reopen. If it
owns the mount, it also checks bytes after unmount and remount. A mount that
existed before the probe prevents remount acceptance; that case is a reported
failure rather than a pass. File removal and mount cleanup results are checked.

The real-Wii combined probe passed the SD file/path suite (78 checks, 0 failures),
including fixed bytes and timestamps after an owned unmount/remount. The USB FAT
suite failed 4 checks with EIO (errno 5) during directory, path, create, and
unmount operations. USB flush acceptance, media removal, interrupted writes,
transport errors, and independent writer-open support require further work. See
[wii-std-files-evidence.txt](wii-std-files-evidence.txt) for source/build results.

## Copy increment

`std::fs::copy` uses a two-path request. Both paths are resolved to mounted
paths and pass byte and NUL checks before either file opens. The provider opens
and validates the source before it opens the destination. A source read handle
prevents same-entry truncation through the FAT open registry, including path
aliases. Such a copy returns EBUSY. Independent writer opens remain restricted.

The shared transfer engine handles short reads and writes, rejects invalid
counts and zero writes, and returns the actual byte count only after EOF.
The provider flushes the destination and explicitly closes both handles. It
returns the first operation or cleanup error. FAT records each close failure
in its existing diagnostics. Copy then applies the source read-only permission
to the destination through real FAT attribute operations and checks the result.
It preserves other destination DOS attributes. Copy errors can leave a partial
destination, as with normal non-atomic file copy. No rollback is claimed.

The SD probe reserves its destination with create_new before it calls copy.
It checks fixed bytes and length, overwrite truncation, same-file protection,
and destination preservation after missing-source and directory-source errors.
The host engine tests inject read, write, and close failures. Neither these
checks nor the linked probe prove SD transport behavior on Wii.
