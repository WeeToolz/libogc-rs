# CARD API by API runtime audit

## Scope and evidence

The generated inventory has 209 entries across `ogc-card` (63), `ogc-card-sys` (90), and `ogc-card-metadata` (56). This audit records the exact implementation entry, direct probe or test function, and the strongest result found for each row. An inventory mapping does not prove a runtime pass.

Dolphin has saved passing runs for invalid-channel behavior, saved-card transfers, synchronous mount lifecycle, format, and metadata bit helpers. The runs used private copies of a verified 16 MiB virtual card. They do not prove physical card behavior. See the [CARD accessor run](2026-11-03-card-accessor-update/README.md), [full-buffer run](2026-11-03-card-full-buffer/README.md), [sync mount lifecycle run](2026-11-03-card-sync-mount-lifecycle/README.md), [GameCube format run](2026-11-03-card-format-gamecube/README.md), [Wii format marker run](2026-11-03-card-format-marker/README.md), and [metadata helper log](2026-11-02-card-metadata/bus_probe/probe.log).

The current probe source adds a valid-card `find_first` and `find_next` scan. It also checks positive sys and facade `probe` and `probe_ex` results against detected media geometry; reads, changes, verifies, and restores the created save's attributes and status; submits raw async read and write operations with callback and data checks; creates and deletes temporary files through raw async file and directory-entry calls; checks synchronous sys and facade `create_entry` and `delete_entry`/`create` and `delete` flows, with handle close, reopen, entry absence, and free-block restoration; runs a second format cycle through raw `format_async`; checks mounted-card geometry, serial number, free space, and status through raw calls and facade wrappers; and requests one more allocation block than the card reports as free. The allocation check verifies `CARD_ERROR_INSSPACE`, an unchanged output handle and free-block count, and no directory entry. Both temporary names have preflight checks. The current GameCube `card_probe` DOL was rebuilt with the prepared PowerPC sysroot and `gamecube` cfg. Its SHA-256 is recorded in `DEVELOPMENT_RESULTS.md`. It has not run. The current Dolphin executable does not pass its version preflight in this environment, so these new success and failure checks have no runtime result. See the [preflight record](2026-11-03-card-api-audit-preflight.md). For `ogc-card-sys` backend entries, a probe call to the public sys wrapper also enters the backend. The backend row is not a separate call or test.

## Runtime check groups

| Probe or test function | Inventory rows | Saved result | What it proves |
| --- | ---: | --- | --- |
| `probe_card_invalid_channel_apis` | 23 | Pass: the saved GameCube CARD run contains all 33 named invalid-channel assertions. | Rejects invalid channels; checks outputs, cursors, inputs, and callback state. |
| `probe_card_save_data` | 97 | Saved run: 98 checks passed before these source updates. The directory scan, valid geometry and serial queries, valid status reads, allocation failure, synchronous sys and facade file lifecycles, attribute and status updates, raw async transfers, and raw async file and entry lifecycles await Dolphin. | Covers owned and raw async calls, sync/raw transfer round trips, persistence, status, attributes, delete, cleanup, directory search, allocation failure, and valid media queries. Some failure-only labels have no saved PASS result. |
| `mount_card_owned` | 4 | Pass: owned mount request and completion checks appear in successful CARD runs. | Checks request slot, status, completion, and attached state. |
| `probe_card_mount_lifecycles` | 6 | Pass: Wii storage run 227 checks; GameCube CARD run 108 checks. | Checks wrapper and sys mount/unmount, sector size, and no-card state after unmount. |
| `probe_card_format` | 5 | Saved runs: 111 GameCube checks and 231 Wii checks for the synchronous cycle. A second raw async format cycle is new and awaits Dolphin. | Checks synchronous and asynchronous format, unmount/remount, free blocks, marker removal, and cleanup. |
| `probe_card_metadata_bits` | 8 | Pass: Dolphin bus probe 16 checks. | Checks banner and icon format/speed bit fields, null inputs, and separate addresses. |
| `probe_media_hotplug` | 2 | Not run: `dolphin-automation` returns before the hotplug loop. | Card removal, stale file handles, and remount after physical removal remain open. |
| `open_or_create_hotplug_card` | 4 | Not run: called only by skipped hotplug probe. | Card removal and file reopen after reinsertion remain open. |
| `run` | 2 | The run log prints `card shutdown=0`; it does not emit a named PASS assertion. | Treat as observed status only. |
| `card_identifiers_round_trip_and_report_invalid_bytes` | 8 | Pass: `cargo test -p ogc-card-metadata` ran 18 tests with no failures. | Host logic only; this is not a Dolphin or card-device result. |
| `create_completion_only_exposes_a_file_on_success` | 3 | Pass: `cargo test -p ogc-card-metadata` ran 18 tests with no failures. | Host logic only; this is not a Dolphin or card-device result. |
| `error_preserves_status_and_original_payload` | 3 | Source test exists; `cargo test -p ogc-card --lib` stopped because dependencies require PowerPC. | No current test result for these compatibility accessors. |
| `metadata_completion_accessors_keep_failed_payloads_unavailable` | 11 | Pass: `cargo test -p ogc-card-metadata` ran 18 tests with no failures. | Host logic only; this is not a Dolphin or card-device result. |
| `owned_completion_accessors_preserve_status_and_payloads` | 18 | Pass: `cargo test -p ogc-card-metadata` ran 18 tests with no failures. | Host logic only; this is not a Dolphin or card-device result. |
| `protocol_apis_are_exercised_directly` | 16 | Pass: `cargo test -p ogc-card-metadata` ran 18 tests with no failures. | Host logic only; this is not a Dolphin or card-device result. |
| `taking_error_value_transfers_ownership_once` | 1 | Source test exists; `cargo test -p ogc-card --lib` stopped because dependencies require PowerPC. | No current test result for these compatibility accessors. |
| `card_identifiers_round_trip_and_report_invalid_bytes` | 8 | Pass: `cargo test -p ogc-card-metadata` ran 18 tests with no failures. | Host logic only; this is not a Dolphin or card-device result. |
| `create_completion_only_exposes_a_file_on_success` | 3 | Pass: `cargo test -p ogc-card-metadata` ran 18 tests with no failures. | Host logic only; this is not a Dolphin or card-device result. |
| `metadata_completion_accessors_keep_failed_payloads_unavailable` | 11 | Pass: `cargo test -p ogc-card-metadata` ran 18 tests with no failures. | Host logic only; this is not a Dolphin or card-device result. |
| `owned_completion_accessors_preserve_status_and_payloads` | 18 | Pass: `cargo test -p ogc-card-metadata` ran 18 tests with no failures. | Host logic only; this is not a Dolphin or card-device result. |
| `protocol_apis_are_exercised_directly` | 16 | Pass: `cargo test -p ogc-card-metadata` ran 18 tests with no failures. | Host logic only; this is not a Dolphin or card-device result. |

The current `probe_card_save_data` source contains 92 literal `logger.record` calls. Its allocation-boundary helper contains 14; the synchronous entry helper contains 7. Async helpers also generate outcome labels for each raw call. Across the saved run logs, 43 distinct labels have a PASS result. The following source checks have no saved PASS result:

- `card game identifier validation` and `card company identifier validation` are error-only records. The passing `card identifier validation` check covers the successful `RUST` and `RS` constructors.
- `card probe game identifier validation` and `card probe company identifier validation` are error-only records. The second identifier pair has no separate PASS label; later successful probe calls give only indirect evidence that construction succeeded.
- `card sector size supports transfers`, `card unmount after invalid sector size`, `card existing SAVE handle cleanup`, `card unmount after existing SAVE`, `card unmount after SAVE preflight`, `card close after failed create result`, and `card unmount after failed reopen` are failure or cleanup branches with no saved PASS result.
- `CARD sys find_first and find_next enumerate the created save` is a new success-path check in the current source. It has no Dolphin result because the current Dolphin preflight fails.
- `CARD sys probe detects the mounted slot`, `CARD facade probe detects the mounted slot`, and the two `probe_ex` geometry comparisons are new positive media checks. They have no Dolphin result because the current Dolphin preflight fails.
- `CARD sys get_attributes and set_attributes read back and restore flags` is a new success-path check in the current source. It has no Dolphin result because the current Dolphin preflight fails.
- `CARD sys write_async accepted`, `CARD sys write_async callback`, `CARD sys write_async callback after return`, and `CARD sys write_async result` are new success-path checks in the current source. They have no Dolphin result because the current Dolphin preflight fails.
- The equivalent `CARD sys read_async` outcome labels and `CARD sys read_async returns raw write data` are new checks in the current source. They have no Dolphin result because the current Dolphin preflight fails.
- The `CARD sys write_async restore` outcome labels and `CARD raw async read and write complete with restored data` are new checks in the current source. They have no Dolphin result because the current Dolphin preflight fails.
- `CARD sys create_async` and `CARD sys delete_async` each have accepted, callback, callback-after-return, and result labels. The probe also checks the created file handle, close result, unused test name, and deletion after callback. These new checks have no Dolphin result because the current Dolphin preflight fails.
- `CARD sys create_entry_async` and `CARD sys delete_entry_async` each have accepted, callback, callback-after-return, and result labels. The probe checks the returned file and directory entry, closes the file, and verifies that deletion removes the entry. These new checks have no Dolphin result because the current Dolphin preflight fails.
- `CARD sys set_status_async` and `CARD sys set_attributes_async` each have update and restore submissions, callback results, readback checks, and state restoration checks. They have no Dolphin result because the current Dolphin preflight fails.
- `CARD create beyond available blocks returns insufficient space`, `CARD failed oversized create preserves output handle and free space`, and `CARD failed oversized create leaves no directory entry` are new checks. They have no Dolphin result because the current Dolphin preflight fails.

## API inventory rows

Each row below has a separate inventory entry. The `source entry` column separates repeated symbols in different implementation files or locations. The evidence function and call path come from the generated evidence field. The result states whether a saved Dolphin or host run supports that row.

### `ogc-card` (63 entries)

| API | Source entry | Probe or test call | Saved result |
| --- | --- | --- | --- |
| `block_count` | `crates/compatibility/ogc-card/src/lib.rs:1577` | `probe_card_format` → `card::block_count` | Format checks passed in Wii and GameCube runs |
| `channel` | `crates/compatibility/ogc-card/src/lib.rs:34` | `probe_card_save_data` → `slot.channel` | Saved CARD runs pass; see the named-check gaps below |
| `create` | `crates/compatibility/ogc-card/src/lib.rs:1755` | `probe_card_sync_file_lifecycle` → `card::create` | Mounted-media handle, status, and cleanup checks are pending Dolphin |
| `create_async_owned` | `crates/compatibility/ogc-card/src/lib.rs:1106` | `probe_card_save_data` → `card::create_async_owned` | Saved CARD runs cover successful creation; new insufficient-space behavior is pending Dolphin |
| `create_entry_async_owned` | `crates/compatibility/ogc-card/src/lib.rs:1426` | `probe_card_save_data` → `card::create_entry_async_owned` | Saved CARD runs pass; see the named-check gaps below |
| `delete` | `crates/compatibility/ogc-card/src/lib.rs:1762` | `probe_card_invalid_channel_apis`; `probe_card_sync_file_lifecycle` → `card::delete` | Invalid-channel checks passed in the saved run; mounted-media delete and absence checks are pending Dolphin |
| `delete_async_owned` | `crates/compatibility/ogc-card/src/lib.rs:1117` | `probe_card_save_data` → `card::delete_async_owned` | Saved CARD runs pass; see the named-check gaps below |
| `delete_entry_async_owned` | `crates/compatibility/ogc-card/src/lib.rs:1467` | `probe_card_save_data` → `card::delete_entry_async_owned` | Saved CARD runs pass; see the named-check gaps below |
| `detach_result` | `crates/compatibility/ogc-card/src/lib.rs:289` | `probe_card_save_data` → `mounted.detach_result` | Saved CARD runs pass; see the named-check gaps below |
| `file_status` | `crates/compatibility/ogc-card/src/lib.rs:1593` | `probe_card_save_data` → `card::file_status` | Saved CARD runs pass; see the named-check gaps below |
| `free_blocks` | `crates/compatibility/ogc-card/src/lib.rs:1585` | `probe_card_save_data` → `card::free_blocks` | Saved CARD runs cover the current count; new unchanged-after-failure check is pending Dolphin |
| `get_block_count` | `crates/compatibility/ogc-card/src/lib.rs:1634` | `probe_card_save_data` → `card::get_block_count` | New mounted-geometry cross-check in source; pending Dolphin |
| `get_error_code` | `crates/compatibility/ogc-card/src/lib.rs:1620` | `probe_card_invalid_channel_apis` → `card::get_error_code` | Invalid-channel checks passed in the saved 98-check CARD run |
| `get_free_blocks` | `crates/compatibility/ogc-card/src/lib.rs:1641` | `probe_card_save_data` → `card::get_free_blocks` | New mounted free-space cross-check in source; pending Dolphin |
| `get_sector_size` | `crates/compatibility/ogc-card/src/lib.rs:1627` | `probe_card_save_data` → `card::get_sector_size` | New mounted-geometry cross-check in source; pending Dolphin |
| `get_serial_no` | `crates/compatibility/ogc-card/src/lib.rs:1776` | `probe_card_save_data` → `card::get_serial_no` | New mounted serial cross-check in source; pending Dolphin |
| `get_status` | `crates/compatibility/ogc-card/src/lib.rs:1769` | `probe_card_save_data` → `card::get_status` | New created-save status cross-check in source; pending Dolphin |
| `into_parts` | `crates/compatibility/ogc-card/src/metadata_submit_error.rs:30` | `error_preserves_status_and_original_payload` → `error.into_parts` | Host test source only; host build is blocked |
| `into_result` | `crates/compatibility/ogc-card/src/lib.rs:1024` | `probe_card_save_data` → `completion.into_result` | Saved CARD runs pass; see the named-check gaps below |
| `into_value` | `crates/compatibility/ogc-card/src/metadata_submit_error.rs:26` | `taking_error_value_transfers_ownership_once` → `error.into_value` | Host test source only; host build is blocked |
| `is_attached` | `crates/compatibility/ogc-card/src/lib.rs:285` | `probe_card_save_data` → `mounted.is_attached` | Saved CARD runs pass; see the named-check gaps below |
| `mount` | `crates/compatibility/ogc-card/src/lib.rs:1648` | `probe_card_mount_lifecycles` → `card::mount` | Sync lifecycle passed in dedicated Wii and GameCube runs |
| `mount_async` | `crates/compatibility/ogc-card/src/lib.rs:1655` | `probe_card_invalid_channel_apis` → `card::mount_async` | Invalid-channel checks passed in the saved 98-check CARD run |
| `mount_async_owned` | `crates/compatibility/ogc-card/src/lib.rs:325` | `mount_card_owned` → `card::mount_async_owned` | Owned mount checks passed in the saved CARD runs |
| `open` | `crates/compatibility/ogc-card/src/lib.rs:1748` | `probe_card_save_data` → `card::open` | Saved CARD runs pass; see the named-check gaps below |
| `probe` | `crates/compatibility/ogc-card/src/lib.rs:1609` | `probe_card_save_data` → `card::probe` | New positive mounted-media check in source; pending Dolphin |
| `probe_ex` | `crates/compatibility/ogc-card/src/lib.rs:1616` | `probe_card_save_data` → `card::probe_ex` | New output geometry comparison in source; pending Dolphin |
| `probe_info` | `crates/compatibility/ogc-card/src/lib.rs:1553` | `probe_card_invalid_channel_apis` → `card::probe_info` | Invalid-channel checks passed in the saved 98-check CARD run |
| `probe_slot` | `crates/compatibility/ogc-card/src/lib.rs:1548` | `probe_card_save_data` → `card::probe_slot` | Saved CARD runs pass; see the named-check gaps below |
| `read` | `crates/compatibility/ogc-card/src/lib.rs:1671` | `probe_card_save_data` → `card::read` | Saved CARD runs pass; see the named-check gaps below |
| `read_async_owned` | `crates/compatibility/ogc-card/src/lib.rs:738` | `probe_card_save_data` → `card::read_async_owned` | Saved CARD runs pass; see the named-check gaps below |
| `read_slice` | `crates/compatibility/ogc-card/src/lib.rs:1688` | `probe_card_save_data` → `card::read_slice` | Saved CARD runs pass; see the named-check gaps below |
| `sector_size` | `crates/compatibility/ogc-card/src/lib.rs:1569` | `probe_card_save_data` → `card::sector_size` | Saved CARD runs pass; see the named-check gaps below |
| `serial_number` | `crates/compatibility/ogc-card/src/lib.rs:1601` | `probe_card_save_data` → `card::serial_number` | New mounted serial cross-check in source; pending Dolphin |
| `set_status_async_owned` | `crates/compatibility/ogc-card/src/lib.rs:1507` | `probe_card_save_data` → `card::set_status_async_owned` | Saved CARD runs pass; see the named-check gaps below |
| `slot` | `crates/compatibility/ogc-card/src/lib.rs:181` | `mount_card_owned` → `request.slot` | Owned mount checks passed in the saved CARD runs |
| `slot` | `crates/compatibility/ogc-card/src/lib.rs:281` | `probe_card_save_data` → `mounted.slot` | Saved CARD runs pass; see the named-check gaps below |
| `slot` | `crates/compatibility/ogc-card/src/lib.rs:1040` | `probe_card_save_data` → `request.slot` | Saved CARD runs pass; see the named-check gaps below |
| `slot` | `crates/compatibility/ogc-card/src/lib.rs:1073` | `probe_card_save_data` → `request.slot` | Saved CARD runs pass; see the named-check gaps below |
| `status` | `crates/compatibility/ogc-card/src/lib.rs:185` | `mount_card_owned` → `request.status` | Owned mount checks passed in the saved CARD runs |
| `status` | `crates/compatibility/ogc-card/src/lib.rs:665` | `probe_card_save_data` → `request.status` | Saved CARD runs pass; see the named-check gaps below |
| `status` | `crates/compatibility/ogc-card/src/lib.rs:709` | `probe_card_save_data` → `request.status` | Saved CARD runs pass; see the named-check gaps below |
| `status` | `crates/compatibility/ogc-card/src/lib.rs:1020` | `probe_card_save_data` → `completion.status` | Saved CARD runs pass; see the named-check gaps below |
| `status` | `crates/compatibility/ogc-card/src/lib.rs:1044` | `probe_card_save_data` → `request.status` | Saved CARD runs pass; see the named-check gaps below |
| `status` | `crates/compatibility/ogc-card/src/lib.rs:1077` | `probe_card_save_data` → `request.status` | Saved CARD runs pass; see the named-check gaps below |
| `status` | `crates/compatibility/ogc-card/src/lib.rs:1337` | `probe_card_save_data` → `request.status` | Saved CARD runs pass; see the named-check gaps below |
| `status` | `crates/compatibility/ogc-card/src/lib.rs:1371` | `probe_card_save_data` → `request.status` | Saved CARD runs pass; see the named-check gaps below |
| `status` | `crates/compatibility/ogc-card/src/lib.rs:1401` | `probe_card_save_data` → `request.status` | Saved CARD runs pass; see the named-check gaps below |
| `status` | `crates/compatibility/ogc-card/src/metadata_submit_error.rs:18` | `error_preserves_status_and_original_payload` → `error.status` | Host test source only; host build is blocked |
| `try_complete` | `crates/compatibility/ogc-card/src/lib.rs:198` | `mount_card_owned` → `request.try_complete` | Owned mount checks passed in the saved CARD runs |
| `try_complete` | `crates/compatibility/ogc-card/src/lib.rs:669` | `probe_card_save_data` → `request.try_complete` | Saved CARD runs pass; see the named-check gaps below |
| `try_complete` | `crates/compatibility/ogc-card/src/lib.rs:713` | `probe_card_save_data` → `request.try_complete` | Saved CARD runs pass; see the named-check gaps below |
| `try_complete` | `crates/compatibility/ogc-card/src/lib.rs:1048` | `probe_card_save_data` → `request.try_complete` | Saved CARD runs pass; see the named-check gaps below |
| `try_complete` | `crates/compatibility/ogc-card/src/lib.rs:1081` | `probe_card_save_data` → `request.try_complete` | Saved CARD runs pass; see the named-check gaps below |
| `try_complete` | `crates/compatibility/ogc-card/src/lib.rs:1341` | `probe_card_save_data` → `request.try_complete` | Saved CARD runs pass; see the named-check gaps below |
| `try_complete` | `crates/compatibility/ogc-card/src/lib.rs:1375` | `probe_card_save_data` → `request.try_complete` | Saved CARD runs pass; see the named-check gaps below |
| `try_complete` | `crates/compatibility/ogc-card/src/lib.rs:1405` | `probe_card_save_data` → `request.try_complete` | Saved CARD runs pass; see the named-check gaps below |
| `unmount` | `crates/compatibility/ogc-card/src/lib.rs:295` | `probe_card_save_data` → `mounted.unmount` | Saved CARD runs pass; see the named-check gaps below |
| `unmount` | `crates/compatibility/ogc-card/src/lib.rs:1664` | `probe_card_mount_lifecycles` → `card::unmount` | Sync lifecycle passed in dedicated Wii and GameCube runs |
| `value` | `crates/compatibility/ogc-card/src/metadata_submit_error.rs:22` | `error_preserves_status_and_original_payload` → `error.value` | Host test source only; host build is blocked |
| `write` | `crates/compatibility/ogc-card/src/lib.rs:1678` | `probe_card_save_data` → `card::write` | Saved CARD runs pass; see the named-check gaps below |
| `write_async_owned` | `crates/compatibility/ogc-card/src/lib.rs:758` | `probe_card_save_data` → `card::write_async_owned` | Saved CARD runs pass; see the named-check gaps below |
| `write_slice` | `crates/compatibility/ogc-card/src/lib.rs:1723` | `probe_card_save_data` → `card::write_slice` | Saved CARD runs pass; see the named-check gaps below |

### `ogc-card-sys` (90 entries)

| API | Source entry | Probe or test call | Saved result |
| --- | --- | --- | --- |
| `close` | `crates/services/ogc-card-sys/src/lib.rs:396` | `probe_card_save_data` → `card::sys::close` | Saved CARD runs pass; see the named-check gaps below |
| `close` | `crates/services/ogc-card-sys/src/wii_backend.rs:2428` | `probe_card_save_data` → `close` | Saved CARD runs pass; see the named-check gaps below |
| `create` | `crates/services/ogc-card-sys/src/lib.rs:425` | `probe_card_sync_file_lifecycle` → `card::create` → `card::sys::create` | Mounted-media create and handle checks are pending Dolphin |
| `create` | `crates/services/ogc-card-sys/src/wii_backend.rs:3160` | `probe_card_sync_file_lifecycle` → `card::create` → `create` | Mounted-media create path is covered in source; pending Dolphin |
| `create_async` | `crates/services/ogc-card-sys/src/lib.rs:408` | `probe_card_save_data` → `card::sys::create_async` | Invalid-channel check passed in the saved run; raw async create and callback checks await Dolphin |
| `create_async` | `crates/services/ogc-card-sys/src/wii_backend.rs:3126` | `probe_card_save_data` → `create_async` | Invalid-channel check passed in the saved run; raw async create and callback checks await Dolphin |
| `create_entry` | `crates/services/ogc-card-sys/src/lib.rs:453` | `probe_card_invalid_channel_apis`; `probe_card_sync_entry_lifecycle` → `card::sys::create_entry` | Invalid-channel behavior passed in the saved run; valid synchronous create and handle initialization are pending Dolphin |
| `create_entry` | `crates/services/ogc-card-sys/src/wii_backend.rs:3258` | `probe_card_sync_entry_lifecycle` → `card::sys::create_entry` | Valid synchronous create path is covered in source; pending Dolphin |
| `create_entry_async` | `crates/services/ogc-card-sys/src/lib.rs:437` | `probe_card_save_data` → `card::sys::create_entry_async` | Invalid-channel check passed in the saved run; raw async entry creation and callback checks await Dolphin |
| `create_entry_async` | `crates/services/ogc-card-sys/src/wii_backend.rs:3229` | `probe_card_save_data` → `create_entry_async` | Invalid-channel check passed in the saved run; raw async entry creation and callback checks await Dolphin |
| `delete` | `crates/services/ogc-card-sys/src/lib.rs:474` | `probe_card_sync_file_lifecycle` → `card::delete` → `card::sys::delete` | Mounted-media delete and cleanup are pending Dolphin |
| `delete` | `crates/services/ogc-card-sys/src/wii_backend.rs:3289` | `probe_card_sync_file_lifecycle` → `card::delete` → `delete` | Mounted-media delete path is covered in source; pending Dolphin |
| `delete_async` | `crates/services/ogc-card-sys/src/lib.rs:464` | `probe_card_save_data` → `card::sys::delete_async` | Invalid-channel check passed in the saved run; raw async delete and callback checks await Dolphin |
| `delete_async` | `crates/services/ogc-card-sys/src/wii_backend.rs:3270` | `probe_card_save_data` → `delete_async` | Invalid-channel check passed in the saved run; raw async delete and callback checks await Dolphin |
| `delete_entry` | `crates/services/ogc-card-sys/src/lib.rs:495` | `probe_card_invalid_channel_apis`; `probe_card_sync_entry_lifecycle` → `card::sys::delete_entry` | Invalid-channel behavior passed in the saved run; valid synchronous delete and cleanup are pending Dolphin |
| `delete_entry` | `crates/services/ogc-card-sys/src/wii_backend.rs:3355` | `probe_card_sync_entry_lifecycle` → `card::sys::delete_entry` | Valid synchronous delete path is covered in source; pending Dolphin |
| `delete_entry_async` | `crates/services/ogc-card-sys/src/lib.rs:485` | `probe_card_save_data` → `card::sys::delete_entry_async` | Invalid-channel check passed in the saved run; raw async entry deletion and callback checks await Dolphin |
| `delete_entry_async` | `crates/services/ogc-card-sys/src/wii_backend.rs:3336` | `probe_card_save_data` → `delete_entry_async` | Invalid-channel check passed in the saved run; raw async entry deletion and callback checks await Dolphin |
| `find_first` | `crates/services/ogc-card-sys/src/lib.rs:505` | `probe_card_save_data` → `card::sys::find_first` | New positive directory check in source; pending Dolphin run |
| `find_first` | `crates/services/ogc-card-sys/src/wii_backend.rs:1975` | `probe_card_save_data` → `find_first` | New positive directory check in source; pending Dolphin run |
| `find_next` | `crates/services/ogc-card-sys/src/lib.rs:515` | `probe_card_save_data` → `card::sys::find_next` | New positive directory check in source; pending Dolphin run |
| `find_next` | `crates/services/ogc-card-sys/src/wii_backend.rs:1997` | `probe_card_save_data` → `find_next` | New positive directory check in source; pending Dolphin run |
| `format` | `crates/services/ogc-card-sys/src/lib.rs:709` | `probe_card_format` → `card::sys::format` | Format checks passed in Wii and GameCube runs |
| `format` | `crates/services/ogc-card-sys/src/wii_backend.rs:3427` | `probe_card_format` → `format` | Format checks passed in Wii and GameCube runs |
| `format_async` | `crates/services/ogc-card-sys/src/lib.rs:705` | `probe_card_format` → `card::sys::format_async` | Invalid-channel check passed in the saved run; valid async format and callback check await Dolphin |
| `format_async` | `crates/services/ogc-card-sys/src/wii_backend.rs:3408` | `probe_card_format` → `format_async` | Invalid-channel check passed in the saved run; valid async format and callback check await Dolphin |
| `get_attributes` | `crates/services/ogc-card-sys/src/lib.rs:693` | `probe_card_save_data` → `card::sys::get_attributes` | Invalid-channel checks passed in the saved run; valid read and restore check is pending Dolphin |
| `get_attributes` | `crates/services/ogc-card-sys/src/wii_backend.rs:2118` | `probe_card_save_data` → `get_attributes` | Invalid-channel checks passed in the saved run; valid read and restore check is pending Dolphin |
| `get_banner_fmt` | `crates/services/ogc-card-sys/src/lib.rs:549` | `probe_card_metadata_bits` → `card::sys::get_banner_fmt` | Metadata helper checks passed in the 16-check Dolphin bus run |
| `get_block_count` | `crates/services/ogc-card-sys/src/lib.rs:268` | `probe_card_save_data` → `card::sys::get_block_count` | New mounted-geometry check in source; pending Dolphin |
| `get_block_count` | `crates/services/ogc-card-sys/src/wii_backend.rs:1692` | `probe_card_save_data` → `get_block_count` | New mounted-geometry check in source; pending Dolphin |
| `get_directory` | `crates/services/ogc-card-sys/src/lib.rs:526` | `probe_card_save_data` → `card::sys::get_directory` | Saved CARD runs pass; see the named-check gaps below |
| `get_directory` | `crates/services/ogc-card-sys/src/wii_backend.rs:2037` | `probe_card_save_data` → `get_directory` | Saved CARD runs pass; see the named-check gaps below |
| `get_error_code` | `crates/services/ogc-card-sys/src/lib.rs:252` | `probe_card_invalid_channel_apis` → `card::sys::get_error_code` | Invalid-channel checks passed in the saved 98-check CARD run |
| `get_error_code` | `crates/services/ogc-card-sys/src/wii_backend.rs:1669` | `probe_card_invalid_channel_apis` → `get_error_code` | Invalid-channel checks passed in the saved 98-check CARD run |
| `get_free_blocks` | `crates/services/ogc-card-sys/src/lib.rs:276` | `probe_card_save_data` → `card::sys::get_free_blocks` | New mounted free-space check in source; pending Dolphin |
| `get_free_blocks` | `crates/services/ogc-card-sys/src/wii_backend.rs:1708` | `probe_card_save_data` → `get_free_blocks` | New mounted free-space check in source; pending Dolphin |
| `get_icon_fmt` | `crates/services/ogc-card-sys/src/lib.rs:571` | `probe_card_metadata_bits` → `card::sys::get_icon_fmt` | Metadata helper checks passed in the 16-check Dolphin bus run |
| `get_icon_speed` | `crates/services/ogc-card-sys/src/lib.rs:598` | `probe_card_metadata_bits` → `card::sys::get_icon_speed` | Metadata helper checks passed in the 16-check Dolphin bus run |
| `get_sector_size` | `crates/services/ogc-card-sys/src/lib.rs:260` | `probe_card_save_data` → `card::sys::get_sector_size` | New mounted-geometry check in source; pending Dolphin |
| `get_sector_size` | `crates/services/ogc-card-sys/src/wii_backend.rs:1676` | `probe_card_save_data` → `get_sector_size` | New mounted-geometry check in source; pending Dolphin |
| `get_serial_no` | `crates/services/ogc-card-sys/src/lib.rs:720` | `probe_card_save_data` → `card::sys::get_serial_no` | New mounted serial check in source; pending Dolphin |
| `get_serial_no` | `crates/services/ogc-card-sys/src/wii_backend.rs:3686` | `probe_card_save_data` → `get_serial_no` | New mounted serial check in source; pending Dolphin |
| `get_status` | `crates/services/ogc-card-sys/src/lib.rs:541` | `probe_card_save_data` → `card::sys::get_status` | New created-save status check in source; pending Dolphin |
| `get_status` | `crates/services/ogc-card-sys/src/wii_backend.rs:2073` | `probe_card_save_data` → `get_status` | New created-save status check in source; pending Dolphin |
| `get_status_ex` | `crates/services/ogc-card-sys/src/lib.rs:647` | `probe_card_save_data` → `card::sys::get_status_ex` | New created-save directory-entry check in source; pending Dolphin |
| `get_status_ex` | `crates/services/ogc-card-sys/src/wii_backend.rs:2093` | `probe_card_save_data` → `get_status_ex` | New created-save directory-entry check in source; pending Dolphin |
| `init` | `crates/services/ogc-card-sys/src/lib.rs:212` | `probe_card_save_data` → `card::sys::init` | Saved CARD runs pass; see the named-check gaps below |
| `init` | `crates/services/ogc-card-sys/src/wii_backend.rs:1602` | `probe_card_save_data` → `init` | Saved CARD runs pass; see the named-check gaps below |
| `mount` | `crates/services/ogc-card-sys/src/lib.rs:302` | `probe_card_mount_lifecycles` → `card::sys::mount` | Sync lifecycle passed in dedicated Wii and GameCube runs |
| `mount` | `crates/services/ogc-card-sys/src/wii_backend.rs:1819` | `probe_card_mount_lifecycles` → `mount` | Sync lifecycle passed in dedicated Wii and GameCube runs |
| `mount_async` | `crates/services/ogc-card-sys/src/lib.rs:287` | `probe_card_invalid_channel_apis` → `card::sys::mount_async` | Invalid-channel checks passed in the saved 98-check CARD run |
| `mount_async` | `crates/services/ogc-card-sys/src/wii_backend.rs:1797` | `probe_card_invalid_channel_apis` → `mount_async` | Invalid-channel checks passed in the saved 98-check CARD run |
| `open` | `crates/services/ogc-card-sys/src/lib.rs:375` | `probe_card_save_data` → `card::sys::open` | Saved CARD runs pass; see the named-check gaps below |
| `open` | `crates/services/ogc-card-sys/src/wii_backend.rs:1835` | `probe_card_save_data` → `open` | Saved CARD runs pass; see the named-check gaps below |
| `open_entry` | `crates/services/ogc-card-sys/src/lib.rs:386` | `probe_card_save_data` → `card::sys::open_entry` | Invalid-channel check passed in the saved run; open and close of the created raw entry await Dolphin |
| `open_entry` | `crates/services/ogc-card-sys/src/wii_backend.rs:1869` | `probe_card_save_data` → `open_entry` | Invalid-channel check passed in the saved run; open and close of the created raw entry await Dolphin |
| `probe` | `crates/services/ogc-card-sys/src/lib.rs:240` | `probe_card_save_data` → `card::sys::probe` | New positive mounted-media check in source; pending Dolphin |
| `probe` | `crates/services/ogc-card-sys/src/wii_backend.rs:1620` | `probe_card_save_data` → `card::sys::probe` | New positive mounted-media check in source; pending Dolphin |
| `probe_ex` | `crates/services/ogc-card-sys/src/lib.rs:248` | `probe_card_save_data` → `card::sys::probe_ex` | New output geometry comparison in source; pending Dolphin |
| `probe_ex` | `crates/services/ogc-card-sys/src/wii_backend.rs:1627` | `probe_card_save_data` → `card::sys::probe_ex` | New output geometry comparison in source; pending Dolphin |
| `read` | `crates/services/ogc-card-sys/src/lib.rs:335` | `probe_card_save_data` → `card::sys::read` | Saved CARD runs pass; see the named-check gaps below |
| `read` | `crates/services/ogc-card-sys/src/wii_backend.rs:1947` | `probe_card_save_data` → `read` | Saved CARD runs pass; see the named-check gaps below |
| `read_async` | `crates/services/ogc-card-sys/src/lib.rs:318` | `probe_card_save_data` → `card::sys::read_async` | Invalid-channel check passed in the saved run; raw async read and callback checks await Dolphin |
| `read_async` | `crates/services/ogc-card-sys/src/wii_backend.rs:1909` | `probe_card_save_data` → `read_async` | Invalid-channel check passed in the saved run; raw async read and callback checks await Dolphin |
| `set_attributes` | `crates/services/ogc-card-sys/src/lib.rs:701` | `probe_card_save_data` → `card::sys::set_attributes` | Invalid-channel checks passed in the saved run; valid update and restore check is pending Dolphin |
| `set_attributes` | `crates/services/ogc-card-sys/src/wii_backend.rs:3660` | `probe_card_save_data` → `set_attributes` | Invalid-channel checks passed in the saved run; valid update and restore check is pending Dolphin |
| `set_attributes_async` | `crates/services/ogc-card-sys/src/lib.rs:697` | `probe_card_save_data` → `card::sys::set_attributes_async` | Invalid-channel check passed in the saved run; raw async update and restore checks await Dolphin |
| `set_attributes_async` | `crates/services/ogc-card-sys/src/wii_backend.rs:3641` | `probe_card_save_data` → `set_attributes_async` | Invalid-channel check passed in the saved run; raw async update and restore checks await Dolphin |
| `set_banner_fmt` | `crates/services/ogc-card-sys/src/lib.rs:561` | `probe_card_metadata_bits` → `card::sys::set_banner_fmt` | Metadata helper checks passed in the 16-check Dolphin bus run |
| `set_comment_addr` | `crates/services/ogc-card-sys/src/lib.rs:635` | `probe_card_metadata_bits` → `card::sys::set_comment_addr` | Metadata helper checks passed in the 16-check Dolphin bus run |
| `set_company` | `crates/services/ogc-card-sys/src/lib.rs:235` | `probe_card_save_data` → `card::sys::set_company` | Saved CARD runs pass; see the named-check gaps below |
| `set_gamecode` | `crates/services/ogc-card-sys/src/lib.rs:228` | `probe_card_save_data` → `card::sys::set_gamecode` | Saved CARD runs pass; see the named-check gaps below |
| `set_icon_addr` | `crates/services/ogc-card-sys/src/lib.rs:625` | `probe_card_metadata_bits` → `card::sys::set_icon_addr` | Metadata helper checks passed in the 16-check Dolphin bus run |
| `set_icon_fmt` | `crates/services/ogc-card-sys/src/lib.rs:583` | `probe_card_metadata_bits` → `card::sys::set_icon_fmt` | Metadata helper checks passed in the 16-check Dolphin bus run |
| `set_icon_speed` | `crates/services/ogc-card-sys/src/lib.rs:610` | `probe_card_metadata_bits` → `card::sys::set_icon_speed` | Metadata helper checks passed in the 16-check Dolphin bus run |
| `set_status` | `crates/services/ogc-card-sys/src/lib.rs:673` | `probe_card_invalid_channel_apis` → `card::sys::set_status` | Invalid-channel checks passed in the saved 98-check CARD run |
| `set_status` | `crates/services/ogc-card-sys/src/wii_backend.rs:3559` | `probe_card_invalid_channel_apis` → `set_status` | Invalid-channel checks passed in the saved 98-check CARD run |
| `set_status_async` | `crates/services/ogc-card-sys/src/lib.rs:658` | `probe_card_save_data` → `card::sys::set_status_async` | Invalid-channel check passed in the saved run; raw async update and metadata restore checks await Dolphin |
| `set_status_async` | `crates/services/ogc-card-sys/src/wii_backend.rs:3515` | `probe_card_save_data` → `set_status_async` | Invalid-channel check passed in the saved run; raw async update and metadata restore checks await Dolphin |
| `set_status_ex` | `crates/services/ogc-card-sys/src/lib.rs:683` | `probe_card_invalid_channel_apis` → `card::sys::set_status_ex` | Invalid-channel checks passed in the saved 98-check CARD run |
| `set_status_ex` | `crates/services/ogc-card-sys/src/wii_backend.rs:3593` | `probe_card_invalid_channel_apis` → `set_status_ex` | Invalid-channel checks passed in the saved 98-check CARD run |
| `shutdown` | `crates/services/ogc-card-sys/src/lib.rs:724` | `run` → `card::sys::shutdown` | Shutdown status was logged; no separate PASS assertion |
| `shutdown` | `crates/services/ogc-card-sys/src/wii_backend.rs:3704` | `run` → `shutdown` | Shutdown status was logged; no separate PASS assertion |
| `unmount` | `crates/services/ogc-card-sys/src/lib.rs:306` | `probe_card_mount_lifecycles` → `card::sys::unmount` | Sync lifecycle passed in dedicated Wii and GameCube runs |
| `unmount` | `crates/services/ogc-card-sys/src/wii_backend.rs:1823` | `probe_card_mount_lifecycles` → `unmount` | Sync lifecycle passed in dedicated Wii and GameCube runs |
| `write` | `crates/services/ogc-card-sys/src/lib.rs:364` | `probe_card_save_data` → `card::sys::write` | Saved CARD runs pass; see the named-check gaps below |
| `write` | `crates/services/ogc-card-sys/src/wii_backend.rs:2446` | `probe_card_save_data` → `write` | Saved CARD runs pass; see the named-check gaps below |
| `write_async` | `crates/services/ogc-card-sys/src/lib.rs:347` | `probe_card_save_data` → `card::sys::write_async` | Invalid-channel check passed in the saved run; raw async write, callback, and restoration checks await Dolphin |
| `write_async` | `crates/services/ogc-card-sys/src/wii_backend.rs:3084` | `probe_card_save_data` → `write_async` | Invalid-channel check passed in the saved run; raw async write, callback, and restoration checks await Dolphin |

### `ogc-card-metadata` (56 entries)

| API | Source entry | Probe or test call | Saved result |
| --- | --- | --- | --- |
| `as_bytes` | `crates/services/ogc-card-metadata/src/lib.rs:58` | `card_identifiers_round_trip_and_report_invalid_bytes` → `game.as_bytes` | 18 host tests passed; no Dolphin run |
| `as_bytes` | `crates/services/ogc-card-metadata/src/lib.rs:80` | `card_identifiers_round_trip_and_report_invalid_bytes` → `company.as_bytes` | 18 host tests passed; no Dolphin run |
| `byte` | `crates/services/ogc-card-metadata/src/lib.rs:28` | `card_identifiers_round_trip_and_report_invalid_bytes` → `nul.byte` | 18 host tests passed; no Dolphin run |
| `bytes` | `crates/services/ogc-card-metadata/src/card_types.rs:245` | `owned_completion_accessors_preserve_status_and_payloads` → `write_error.bytes` | 18 host tests passed; no Dolphin run |
| `bytes` | `crates/services/ogc-card-metadata/src/card_types.rs:274` | `owned_completion_accessors_preserve_status_and_payloads` → `read_completion.bytes` | 18 host tests passed; no Dolphin run |
| `checksum` | `crates/services/ogc-card-metadata/src/lib.rs:129` | `protocol_apis_are_exercised_directly` → `checksum` | 18 host tests passed; no Dolphin run |
| `commit_pair` | `crates/services/ogc-card-metadata/src/lib.rs:335` | `protocol_apis_are_exercised_directly` → `commit_pair` | 18 host tests passed; no Dolphin run |
| `commit_system_blocks` | `crates/services/ogc-card-metadata/src/lib.rs:347` | `protocol_apis_are_exercised_directly` → `commit_system_blocks` | 18 host tests passed; no Dolphin run |
| `commit_verified` | `crates/services/ogc-card-metadata/src/lib.rs:361` | `protocol_apis_are_exercised_directly` → `commit_verified` | 18 host tests passed; no Dolphin run |
| `entry` | `crates/services/ogc-card-metadata/src/card_types.rs:329` | `metadata_completion_accessors_keep_failed_payloads_unavailable` → `create_entry.entry` | 18 host tests passed; no Dolphin run |
| `entry` | `crates/services/ogc-card-metadata/src/card_types.rs:351` | `metadata_completion_accessors_keep_failed_payloads_unavailable` → `delete_entry.entry` | 18 host tests passed; no Dolphin run |
| `erase` | `crates/services/ogc-card-metadata/src/lib.rs:318` | `protocol_apis_are_exercised_directly` → `media.erase` | 18 host tests passed; no Dolphin run |
| `file` | `crates/services/ogc-card-metadata/src/card_types.rs:214` | `owned_completion_accessors_preserve_status_and_payloads` → `read_error.file` | 18 host tests passed; no Dolphin run |
| `file` | `crates/services/ogc-card-metadata/src/card_types.rs:242` | `owned_completion_accessors_preserve_status_and_payloads` → `write_error.file` | 18 host tests passed; no Dolphin run |
| `file` | `crates/services/ogc-card-metadata/src/card_types.rs:271` | `owned_completion_accessors_preserve_status_and_payloads` → `read_completion.file` | 18 host tests passed; no Dolphin run |
| `file` | `crates/services/ogc-card-metadata/src/card_types.rs:298` | `owned_completion_accessors_preserve_status_and_payloads` → `write_completion.file` | 18 host tests passed; no Dolphin run |
| `file` | `crates/services/ogc-card-metadata/src/card_types.rs:332` | `metadata_completion_accessors_keep_failed_payloads_unavailable` → `create_entry.file` | 18 host tests passed; no Dolphin run |
| `from_parts` | `crates/services/ogc-card-metadata/src/card_types.rs:208` | `owned_completion_accessors_preserve_status_and_payloads` → `CardReadSubmitError::from_parts` | 18 host tests passed; no Dolphin run |
| `from_parts` | `crates/services/ogc-card-metadata/src/card_types.rs:232` | `owned_completion_accessors_preserve_status_and_payloads` → `CardWriteSubmitError::from_parts` | 18 host tests passed; no Dolphin run |
| `from_parts` | `crates/services/ogc-card-metadata/src/card_types.rs:261` | `owned_completion_accessors_preserve_status_and_payloads` → `CardReadCompletion::from_parts` | 18 host tests passed; no Dolphin run |
| `from_parts` | `crates/services/ogc-card-metadata/src/card_types.rs:292` | `owned_completion_accessors_preserve_status_and_payloads` → `CardWriteCompletion::from_parts` | 18 host tests passed; no Dolphin run |
| `from_parts` | `crates/services/ogc-card-metadata/src/card_types.rs:319` | `create_completion_only_exposes_a_file_on_success` → `CardCreateCompletion::from_parts` | 18 host tests passed; no Dolphin run |
| `from_parts` | `crates/services/ogc-card-metadata/src/card_types.rs:345` | `metadata_completion_accessors_keep_failed_payloads_unavailable` → `CardDeleteEntryCompletion::from_parts` | 18 host tests passed; no Dolphin run |
| `from_parts` | `crates/services/ogc-card-metadata/src/card_types.rs:364` | `metadata_completion_accessors_keep_failed_payloads_unavailable` → `CardSetStatusCompletion::from_parts` | 18 host tests passed; no Dolphin run |
| `index` | `crates/services/ogc-card-metadata/src/lib.rs:24` | `card_identifiers_round_trip_and_report_invalid_bytes` → `nul.index` | 18 host tests passed; no Dolphin run |
| `index` | `crates/services/ogc-card-metadata/src/lib.rs:272` | `protocol_apis_are_exercised_directly` → `CopyIndex::Primary.index` | 18 host tests passed; no Dolphin run |
| `into_bytes` | `crates/services/ogc-card-metadata/src/lib.rs:62` | `card_identifiers_round_trip_and_report_invalid_bytes` → `game.into_bytes` | 18 host tests passed; no Dolphin run |
| `into_bytes` | `crates/services/ogc-card-metadata/src/lib.rs:84` | `card_identifiers_round_trip_and_report_invalid_bytes` → `company.into_bytes` | 18 host tests passed; no Dolphin run |
| `into_file` | `crates/services/ogc-card-metadata/src/card_types.rs:217` | `owned_completion_accessors_preserve_status_and_payloads` → `read_error.into_file` | 18 host tests passed; no Dolphin run |
| `into_parts` | `crates/services/ogc-card-metadata/src/card_types.rs:248` | `owned_completion_accessors_preserve_status_and_payloads` → `write_error.into_parts` | 18 host tests passed; no Dolphin run |
| `into_parts` | `crates/services/ogc-card-metadata/src/card_types.rs:277` | `owned_completion_accessors_preserve_status_and_payloads` → `read_completion.into_parts` | 18 host tests passed; no Dolphin run |
| `into_parts` | `crates/services/ogc-card-metadata/src/card_types.rs:308` | `owned_completion_accessors_preserve_status_and_payloads` → `write_completion.into_parts` | 18 host tests passed; no Dolphin run |
| `into_parts` | `crates/services/ogc-card-metadata/src/card_types.rs:335` | `metadata_completion_accessors_keep_failed_payloads_unavailable` → `create_entry.into_parts` | 18 host tests passed; no Dolphin run |
| `into_parts` | `crates/services/ogc-card-metadata/src/card_types.rs:354` | `metadata_completion_accessors_keep_failed_payloads_unavailable` → `delete_entry.into_parts` | 18 host tests passed; no Dolphin run |
| `into_parts` | `crates/services/ogc-card-metadata/src/card_types.rs:373` | `metadata_completion_accessors_keep_failed_payloads_unavailable` → `set_status.into_parts` | 18 host tests passed; no Dolphin run |
| `into_result` | `crates/services/ogc-card-metadata/src/card_types.rs:301` | `create_completion_only_exposes_a_file_on_success` → `success.into_result` | 18 host tests passed; no Dolphin run |
| `new` | `crates/services/ogc-card-metadata/src/lib.rs:51` | `card_identifiers_round_trip_and_report_invalid_bytes` → `CardGameCode::new` | 18 host tests passed; no Dolphin run |
| `new` | `crates/services/ogc-card-metadata/src/lib.rs:73` | `card_identifiers_round_trip_and_report_invalid_bytes` → `CardCompanyCode::new` | 18 host tests passed; no Dolphin run |
| `other` | `crates/services/ogc-card-metadata/src/lib.rs:265` | `protocol_apis_are_exercised_directly` → `CopyIndex::Primary.other` | 18 host tests passed; no Dolphin run |
| `prepare_directory` | `crates/services/ogc-card-metadata/src/lib.rs:183` | `protocol_apis_are_exercised_directly` → `prepare_directory` | 18 host tests passed; no Dolphin run |
| `prepare_fat` | `crates/services/ogc-card-metadata/src/lib.rs:215` | `protocol_apis_are_exercised_directly` → `prepare_fat` | 18 host tests passed; no Dolphin run |
| `prepare_header` | `crates/services/ogc-card-metadata/src/lib.rs:160` | `protocol_apis_are_exercised_directly` → `prepare_header` | 18 host tests passed; no Dolphin run |
| `read` | `crates/services/ogc-card-metadata/src/lib.rs:320` | `protocol_apis_are_exercised_directly` → `media.read` | 18 host tests passed; no Dolphin run |
| `select_copy` | `crates/services/ogc-card-metadata/src/lib.rs:292` | `protocol_apis_are_exercised_directly` → `select_copy` | 18 host tests passed; no Dolphin run |
| `stat` | `crates/services/ogc-card-metadata/src/card_types.rs:370` | `metadata_completion_accessors_keep_failed_payloads_unavailable` → `set_status.stat` | 18 host tests passed; no Dolphin run |
| `status` | `crates/services/ogc-card-metadata/src/card_types.rs:211` | `owned_completion_accessors_preserve_status_and_payloads` → `read_error.status` | 18 host tests passed; no Dolphin run |
| `status` | `crates/services/ogc-card-metadata/src/card_types.rs:239` | `owned_completion_accessors_preserve_status_and_payloads` → `write_error.status` | 18 host tests passed; no Dolphin run |
| `status` | `crates/services/ogc-card-metadata/src/card_types.rs:268` | `owned_completion_accessors_preserve_status_and_payloads` → `read_completion.status` | 18 host tests passed; no Dolphin run |
| `status` | `crates/services/ogc-card-metadata/src/card_types.rs:295` | `owned_completion_accessors_preserve_status_and_payloads` → `write_completion.status` | 18 host tests passed; no Dolphin run |
| `status` | `crates/services/ogc-card-metadata/src/card_types.rs:326` | `create_completion_only_exposes_a_file_on_success` → `success.status` | 18 host tests passed; no Dolphin run |
| `status` | `crates/services/ogc-card-metadata/src/card_types.rs:348` | `metadata_completion_accessors_keep_failed_payloads_unavailable` → `delete_entry.status` | 18 host tests passed; no Dolphin run |
| `status` | `crates/services/ogc-card-metadata/src/card_types.rs:367` | `metadata_completion_accessors_keep_failed_payloads_unavailable` → `set_status.status` | 18 host tests passed; no Dolphin run |
| `validate_directory` | `crates/services/ogc-card-metadata/src/lib.rs:196` | `protocol_apis_are_exercised_directly` → `validate_directory` | 18 host tests passed; no Dolphin run |
| `validate_fat` | `crates/services/ogc-card-metadata/src/lib.rs:231` | `protocol_apis_are_exercised_directly` → `validate_fat` | 18 host tests passed; no Dolphin run |
| `validate_header` | `crates/services/ogc-card-metadata/src/lib.rs:169` | `protocol_apis_are_exercised_directly` → `validate_header` | 18 host tests passed; no Dolphin run |
| `write` | `crates/services/ogc-card-metadata/src/lib.rs:319` | `protocol_apis_are_exercised_directly` → `media.write` | 18 host tests passed; no Dolphin run |

## Open CARD checks

- `find_first` and `find_next` now have a direct positive scan check in the probe source. It must pass in Dolphin before it counts as runtime evidence. Their invalid-channel checks already pass.
- A valid `get_directory` call is covered by `CARD game and company setters select the created save`, which verifies the created file identifiers. Other directory APIs do not all have valid-media results.
- Raw sys `create_async`, `read_async`, `write_async`, `delete_async`, `create_entry_async`, and `delete_entry_async` now have direct success-path callback checks in the probe source. The transfer checks verify data; the file and entry lifecycle checks verify creation and removal. `open_entry` also has a valid entry open and close check. These checks await Dolphin. Owned wrappers have successful async checks.
- Raw synchronous `create_entry` and `delete_entry` now have a bounded create, close, delete, reopen, and free-block restoration check with a private filename. This new lifecycle awaits Dolphin.
- Facade `create` and `delete` now have a bounded create, close, reopen, status, delete, absence, and free-block restoration check with a private filename. This lifecycle awaits Dolphin.
- Raw sys `set_status_async` and `set_attributes_async` now have valid-media update, callback, readback, and restore checks in the probe source. These checks await Dolphin.
- `get_attributes` and `set_attributes` have valid-media read, update, and restore checks in source. The probe now also checks valid `get_status`, `get_status_ex`, geometry, free-space, and serial queries on mounted media. These checks need a Dolphin run. Synchronous status setters still have invalid-channel checks only.
- `format_async` now has a direct success-path callback check with marker removal after remount in the private-card probe source. It awaits Dolphin. Saved format runs still provide synchronous `format` evidence only.
- CARD removal, detach callback delivery, stale CARD handles, and reinsert/remount behavior are skipped in Dolphin automation. Physical card behavior remains unverified.
- Four `ogc-card` host-test inventory rows have test source but no current host pass because target-only dependencies block `cargo test -p ogc-card --lib` on this host.
- `ogc-card-metadata` has 18 passing host tests. Its 56 inventory entries still do not have Dolphin runtime evidence.
