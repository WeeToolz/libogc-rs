# `ogc-di` API runtime audit

This audit covers the 59 `ogc-di` rows in
`docs/generated/implementation-inventory.csv`. The latest no-disc evidence is
[`dvd-di-no-disc-after-media`](dvd-di-no-disc-after-media/probe.log). Dolphin
2609a passed 178 checks with no failures. The probe checks no-disc errors,
argument errors, output preservation, callback timing, transport state, and
cleanup. It does not prove successful DI reads or partition operations.

The inventory rows name 32 Rust functions:

`init`, `load_dvdx`, `use_cache`, `set_init_callback`, `set_mount_callback`,
`mount`, `close`, `get_status`, `identify`, `check_dvd_support`,
`read_disc_id`, `read_disc_id_bytes`, `get_error`, `get_cover_register`,
`reset`, `stop_motor`, `audio_stream`, `eject`, `kill_drive`, `read_dvd`,
`read_dvd_async`, `read`, `unencrypted_read`, `read_dvd_config`,
`read_dvd_copyright`, `read_dvd_disc_key`, `read_dvd_physical`, `read_bca`,
`report_key`, `open_partition`, `close_partition`, and the public `di_fd`
static.

The current probe source adds direct checks for the legacy `di_fd` value after
Rust and C initialization and after close. These checks are not in the saved
178-check Dolphin log. The current Dolphin executable aborts during its version
preflight, so the new assertions still need a runtime result.

The media probe source now submits both Rust `read_dvd_async` and C
`DI_ReadDVDAsync` calls when the direct DI path returns the known `TST1TS`
fixture ID and sector payload. It checks callback result, callback buffer
identity, and returned sector bytes. The saved synthetic-disc run did not
provide a verified direct DI fixture, so this path did not run there. The new
Wii `dvd_probe` DOL built with SHA-256
`a90c41cdcc835cf9564dd4b278d6a23b3a36a6dfbb4f65ddc34217a936118d25`. This DOL
has not run. Dolphin still fails during its version preflight, and a valid
direct-DI media setup is needed to reach the success checks.

The inventory also names 27 C exports:

`DI_Init`, `DI_LoadDVDX`, `DI_UseCache`, `DI_SetInitCallback`, `DI_Mount`,
`DI_Close`, `DI_GetStatus`, `DI_Identify`, `DI_CheckDVDSupport`,
`DI_ReadDiscID`, `DI_GetError`, `DI_GetCoverRegister`, `DI_Reset`,
`DI_StopMotor`, `DI_Eject`, `DI_KillDrive`, `DI_ReadDVD`, `DI_ReadDVDAsync`,
`DI_Read`, `DI_UnencryptedRead`, `DI_ReadDVDConfig`, `DI_ReadDVDCopyright`,
`DI_ReadDVDDiscKey`, `DI_ReadDVDPhysical`, `DI_Read_BCA`, `DI_ReportKey`,
`DI_OpenPartition`, and `DI_ClosePartition`.

## Evidence in the no-disc run

The log has passing assertions for these named behaviors:

| Evidence | APIs covered by the probe source | Scope of the result |
| --- | --- | --- |
| `di init opens transport`, `di init state has no disc` | `init`, `DI_Init` | Rust init and status assertion; C init callback check covers no-disc behavior. |
| `di no-disc mount callback`, `di mount leaves no-disc state`, `DI_Mount delivers no-disc callback` | `mount`, `set_mount_callback`, `DI_Mount` | Callback status and state after no-disc mount. These checks share state. |
| `di identify returns a drive record`, `C DI identify matches Rust output` | `identify`, `DI_Identify` | Rust and C output parity for Dolphin's drive record. |
| `di dvd support reports Dolphin drive`, `C DI support matches Rust result` | `check_dvd_support`, `DI_CheckDVDSupport` | Rust result and C/Rust parity. |
| `di disc id errors preserve output`, `C DI disc ID matches Rust result and output`, `di disc id bytes errors preserve output` | `read_disc_id`, `DI_ReadDiscID`, `read_disc_id_bytes` | No-disc error and output preservation. |
| `di error output is valid or preserved`, `Rust and C DI error calls return data and preserve success` | `get_error`, `DI_GetError` | Error output and C/Rust parity. |
| `di cover result reports no inserted disc`, `C DI cover status matches Rust result and output` | `get_cover_register`, `DI_GetCoverRegister` | No-disc cover state and C/Rust parity. |
| `di config output is valid or preserved`, `C DI configuration matches Rust result and output` | `read_dvd_config`, `DI_ReadDVDConfig` | Output validity or preservation; C/Rust parity. |
| `di copyright output is valid or preserved`, `C DI copyright matches Rust result and output` | `read_dvd_copyright`, `DI_ReadDVDCopyright` | Output validity or preservation; C/Rust parity. |
| `di direct sector read reports no-media error`, `DI_ReadDVD reports no-media error`, `DI_ReadDVDAsync reports no mounted media` | `read_dvd`, `read_dvd_async`, `DI_ReadDVD`, `DI_ReadDVDAsync` | Saved Dolphin runs prove no-disc errors. The source now also checks Rust and C async callbacks and known sector bytes when the direct-DI fixture is valid; this success path has no Dolphin result yet. |
| `di raw read rejects null buffer`, `di raw read rejects unaligned buffer and preserves bytes`, `DI_Read rejects null buffer` | `read`, `DI_Read` | Argument validation before media I/O. |
| `di unencrypted read rejects null buffer`, `DI_UnencryptedRead rejects null buffer` | `unencrypted_read`, `DI_UnencryptedRead` | Null argument errors; no successful read. |
| `DI_ReadDVDDiscKey rejects null buffer`, `DI_ReadDVDPhysical rejects null buffer`, `DI_Read_BCA rejects null buffer`, `DI_ReportKey rejects null buffer` | `read_dvd_disc_key`, `read_dvd_physical`, `read_bca`, `report_key` and C exports | Null argument validation. Valid-buffer media results are not established. |
| `DI_Reset completes`, `DI_StopMotor completes`, `DI_Eject completes`, `DI_KillDrive completes`; related Rust checks | `reset`, `stop_motor`, `eject`, `kill_drive` and C exports | Dolphin command completion in this state. This does not verify physical drive effects. |
| `di audio stream without media reports an error` | `audio_stream` | No-media error only. |
| `di close succeeds`, `di raw read after close rejects missing descriptor`, `di close restores initial status`, `DI_Close restores initial status`, `di reinitializes after close`, `di final close succeeds` | `close`, `DI_Close`, and lifecycle calls | Descriptor close, error after close, and reinitialization. |
| `C DI partition APIs reject a closed descriptor` | `DI_OpenPartition`, `DI_ClosePartition` | C calls reject a closed descriptor. The log does not provide separate assertions for each result. Rust partition calls and successful partition operations are not established. |

The source calls `load_dvdx`, `use_cache`, and `set_init_callback`, but the
saved log does not give each function its own named result. The source now
checks the legacy `di_fd` open and closed states, but the saved log predates
those assertions. The inventory maps the other listed calls to
`probe_di_no_disc`; that mapping is source evidence and does not replace an
isolated assertion for each API.

The newer synthetic-disc run is recorded in
[`di-synthetic-media`](di-synthetic-media/README.md). It shows that this
Dolphin setup does not expose the generated GameCube image through the direct
`/dev/di` path. Successful IOS reads, DI async completion on media, valid Wii
partition open/read/close, and real hardware behavior remain open.
