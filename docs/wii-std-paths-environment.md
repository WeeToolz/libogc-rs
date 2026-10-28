# Wii std paths and environment

Updated: 2026-10-15. The adapters compile for Wii. Hardware acceptance is open.

## Current directory and relative paths

The std adapter resolves relative paths before it calls the FAT provider. All
provider requests still use an absolute mounted path. An application must mount
its medium through the production FAT service before it uses that medium.

`set_current_dir` resolves the target on the medium and checks that it is a
directory before it publishes the new path. A failed change preserves the old
path. A mutex protects the process current directory and serializes relative
directory changes. Other threads see the process directory, as on desktop Rust.
The directory is stored as a path. It does not hold a FAT directory handle or
keep the volume mounted. A removed directory or absent mount produces an error
on the next lookup.

If no directory was set, `current_dir` uses the parent of the loader's absolute
`argv[0]`. It resolves that parent on the mounted medium before it caches the
path. An absent loader path returns ENOENT. A mount that is not yet available
is not cached as a failure. The application can mount it and retry, or call
`set_current_dir` with an explicit mounted directory. There is no assumed SD
mount and no NAND substitution.

`/file` uses the current directory's mount root. `../file` uses its parent.
`sd:file` remains Unsupported: Wii has no separate current directory per mount.
Empty filesystem paths return ENOENT. Embedded NUL bytes and malformed prefixes
return EINVAL before provider access. Path bytes are preserved. The existing
FAT engine restricts creation to its supported ASCII filenames.

`canonicalize` walks real entries under the production volume lock. It returns
stored long names and resolves short aliases, dot components, and parent
components. Each intervening component must exist and be a directory. A file
followed by a slash or parent component returns ENOTDIR. Parent components at
the mounted root stay at that root. Canonicalization does not permit links:
this FAT service has no link implementation.

## Executable, home, temporary directory, and path lists

`current_exe` returns a validated absolute loader `argv[0]`. It does not resolve
relative loader names against a later directory change. This is loader data;
it is not an authenticated executable identity. A loader that supplies no usable
absolute path produces an error.

`home_dir` returns a nonempty HOME value, or None when HOME is absent or empty.
Wii has no operating-system account home. `temp_dir` returns nonempty TMPDIR.
If TMPDIR is absent or empty, it returns the real current directory. If neither
path is available, the infallible public API panics with a specific message.
These functions do not create directories. Applications must select a writable
path and use exclusive creation when they create temporary files.

Path lists use `;`, because `:` is part of the mount prefix. `join_paths` rejects
an embedded `;`. `split_paths` and its borrowed variant preserve mount colons,
empty entries, and non-UTF-8 bytes. No quote or escape syntax is defined.

## Environment variables

HBC/IOS supplies no inherited environment block to this runtime. Each
application starts with an empty environment. The std environment adapter owns
its variables for the life of that application. It uses a mutex for lookup,
replacement, removal, and iterator snapshots. Each iterator owns its bytes and
is unaffected by later mutations. Values can be empty and can contain non-UTF-8
bytes. Keys are case sensitive. Empty keys, NUL bytes, and `=` in keys are
invalid. NUL bytes are invalid in values. Failed validation preserves state.

The standard Rust unsafe contract on `set_var` and `remove_var` still applies.
Environment changes do not write IOS settings, disk files, or another library's
private environment. HOME and TMPDIR are ordinary variables in this environment.

## Verification

`tests/wii-std-paths/adapter.rs` compiles the actual environment and path-list
adapters against host std. It checks validation, replacement, snapshots, removal,
thread visibility, byte preservation, path lists, and absent loader arguments.
Its shared filesystem ABI tests also run. The host Path type cannot prove Wii
mount-prefix or current-directory behavior.

The combined real-Wii run passed its SD file and path suite with 78 checks and
no failures. It exercised current-directory changes, relative paths,
canonicalization, alias lookup, rename replacement, attributes, timestamps,
recursive removal, and persisted bytes and times after unmount/remount. The USB
FAT suite failed four checks with EIO (errno 5). These calls use the production
std adapter and FAT transport. USB acceptance remains open. See
[filesystem evidence](wii-std-files-evidence.txt).
