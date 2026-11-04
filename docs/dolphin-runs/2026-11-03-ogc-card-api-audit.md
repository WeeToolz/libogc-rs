# CARD API by API runtime audit

## Scope and evidence

The generated inventory has 209 entries across `ogc-card` (63), `ogc-card-sys` (90), and `ogc-card-metadata` (56). This audit records the exact implementation entry, direct probe or test function, and the strongest result found for each row. An inventory mapping does not prove a runtime pass.

Dolphin has saved passing runs for invalid-channel behavior, saved-card transfers, synchronous mount lifecycle, format, and metadata bit helpers. The runs used private copies of a verified 16 MiB virtual card. They do not prove physical card behavior. See the [CARD accessor run](2026-11-03-card-accessor-update/README.md), [full-buffer run](2026-11-03-card-full-buffer/README.md), [sync mount lifecycle run](2026-11-03-card-sync-mount-lifecycle/README.md), [GameCube format run](2026-11-03-card-format-gamecube/README.md), [Wii format marker run](2026-11-03-card-format-marker/README.md), and [metadata helper log](2026-11-02-card-metadata/bus_probe/probe.log).

The current probe source adds a valid-card `find_first` and `find_next` scan. It built as a GameCube `card_probe` DOL with SHA-256 `59cdbe31cc7c35eaa6fe97853bdf694dc51cbb219f2a7c0742a9f9028f2f6925`. The current Dolphin executable does not pass its version preflight in this environment, so no new CARD DOL ran. See the [preflight record](2026-11-03-card-api-audit-preflight.md). For `ogc-card-sys` backend entries, a probe call to the public sys wrapper also enters the backend. The backend row is not a separate call or test.

## Runtime check groups

| Probe or test function | Inventory rows | Saved result | What it proves |
| --- | ---: | --- | --- |
| `probe_card_invalid_channel_apis` | 63 | Pass: the saved GameCube CARD run contains all 33 named invalid-channel assertions. | Rejects invalid channels; checks outputs, cursors, inputs, and callback state. |
| `probe_card_save_data` | 57 | Saved run: 98 checks passed before this source update. One positive directory scan check is new and awaits Dolphin. | Covers owned async calls, sync/raw transfer round trips, persistence, delete, cleanup, and get_directory. Some failure-only labels have no saved PASS result. |
| `mount_card_owned` | 4 | Pass: owned mount request and completion checks appear in successful CARD runs. | Checks request slot, status, completion, and attached state. |
| `probe_card_mount_lifecycles` | 6 | Pass: Wii storage run 227 checks; GameCube CARD run 108 checks. | Checks wrapper and sys mount/unmount, sector size, and no-card state after unmount. |
| `probe_card_format` | 3 | Pass: 111 GameCube checks and 231 Wii checks. | Checks format, unmount/remount, free blocks, marker removal, and cleanup. |
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

The current `probe_card_save_data` source contains 57 literal `logger.record` calls. Across the saved run logs, 43 distinct labels have a PASS result. The following 12 labels do not have a saved PASS result:

- `card game identifier validation` and `card company identifier validation` are error-only records. The passing `card identifier validation` check covers the successful `RUST` and `RS` constructors.
- `card probe game identifier validation` and `card probe company identifier validation` are error-only records. The second identifier pair has no separate PASS label; later successful probe calls give only indirect evidence that construction succeeded.
- `card sector size supports transfers`, `card unmount after invalid sector size`, `card existing SAVE handle cleanup`, `card unmount after existing SAVE`, `card unmount after SAVE preflight`, `card close after failed create result`, and `card unmount after failed reopen` are failure or cleanup branches with no saved PASS result.
- `CARD sys find_first and find_next enumerate the created save` is a new success-path check in the current source. It has no Dolphin result because the current Dolphin preflight fails.

## API inventory rows

Each row below has a separate inventory entry. The `source entry` column separates repeated symbols in different implementation files or locations. The evidence function and call path come from the generated evidence field. The result states whether a saved Dolphin or host run supports that row.

### `ogc-card` (63 entries)

| API | Source entry | Probe or test call | Saved result |
| --- | --- | --- | --- |
| `block_count` | `crates/compatibility/ogc-card/src/lib.rs:1577` | `probe_card_format` → `card::block_count` | Format checks passed in Wii and GameCube runs |
| `channel` | `crates/compatibility/ogc-card/src/lib.rs:34` | `probe_card_save_data` → `slot.channel` | Saved CARD runs pass; see the named-check gaps below |
| `create` | `crates/compatibility/ogc-card/src/lib.rs:1755` | `open_or_create_hotplug_card` → `card::create` | Not run: called only by skipped hotplug probe |
| `create_async_owned` | `crates/compatibility/ogc-card/src/lib.rs:1106` | `probe_card_save_data` → `card::create_async_owned` | Saved CARD runs pass; see the named-check gaps below |
| `create_entry_async_owned` | `crates/compatibility/ogc-card/src/lib.rs:1426` | `probe_card_save_data` → `card::create_entry_async_owned` | Saved CARD runs pass; see the named-check gaps below |
| `delete` | `crates/compatibility/ogc-card/src/lib.rs:1762` | `probe_card_invalid_channel_apis` → `card::delete` | Invalid-channel checks passed in the saved 98-check CARD run |
| `delete_async_owned` | `crates/compatibility/ogc-card/src/lib.rs:1117` | `probe_card_save_data` → `card::delete_async_owned` | Saved CARD runs pass; see the named-check gaps below |
| `delete_entry_async_owned` | `crates/compatibility/ogc-card/src/lib.rs:1467` | `probe_card_save_data` → `card::delete_entry_async_owned` | Saved CARD runs pass; see the named-check gaps below |
| `detach_result` | `crates/compatibility/ogc-card/src/lib.rs:289` | `probe_card_save_data` → `mounted.detach_result` | Saved CARD runs pass; see the named-check gaps below |
| `file_status` | `crates/compatibility/ogc-card/src/lib.rs:1593` | `probe_card_save_data` → `card::file_status` | Saved CARD runs pass; see the named-check gaps below |
| `free_blocks` | `crates/compatibility/ogc-card/src/lib.rs:1585` | `probe_card_save_data` → `card::free_blocks` | Saved CARD runs pass; see the named-check gaps below |
| `get_block_count` | `crates/compatibility/ogc-card/src/lib.rs:1634` | `probe_card_invalid_channel_apis` → `card::get_block_count` | Invalid-channel checks passed in the saved 98-check CARD run |
| `get_error_code` | `crates/compatibility/ogc-card/src/lib.rs:1620` | `probe_card_invalid_channel_apis` → `card::get_error_code` | Invalid-channel checks passed in the saved 98-check CARD run |
| `get_free_blocks` | `crates/compatibility/ogc-card/src/lib.rs:1641` | `probe_card_invalid_channel_apis` → `card::get_free_blocks` | Invalid-channel checks passed in the saved 98-check CARD run |
| `get_sector_size` | `crates/compatibility/ogc-card/src/lib.rs:1627` | `open_or_create_hotplug_card` → `card::get_sector_size` | Not run: called only by skipped hotplug probe |
| `get_serial_no` | `crates/compatibility/ogc-card/src/lib.rs:1776` | `probe_card_invalid_channel_apis` → `card::get_serial_no` | Invalid-channel checks passed in the saved 98-check CARD run |
| `get_status` | `crates/compatibility/ogc-card/src/lib.rs:1769` | `probe_card_invalid_channel_apis` → `card::get_status` | Invalid-channel checks passed in the saved 98-check CARD run |
| `into_parts` | `crates/compatibility/ogc-card/src/metadata_submit_error.rs:30` | `error_preserves_status_and_original_payload` → `error.into_parts` | Host test source only; host build is blocked |
| `into_result` | `crates/compatibility/ogc-card/src/lib.rs:1024` | `probe_card_save_data` → `completion.into_result` | Saved CARD runs pass; see the named-check gaps below |
| `into_value` | `crates/compatibility/ogc-card/src/metadata_submit_error.rs:26` | `taking_error_value_transfers_ownership_once` → `error.into_value` | Host test source only; host build is blocked |
| `is_attached` | `crates/compatibility/ogc-card/src/lib.rs:285` | `probe_card_save_data` → `mounted.is_attached` | Saved CARD runs pass; see the named-check gaps below |
| `mount` | `crates/compatibility/ogc-card/src/lib.rs:1648` | `probe_card_mount_lifecycles` → `card::mount` | Sync lifecycle passed in dedicated Wii and GameCube runs |
| `mount_async` | `crates/compatibility/ogc-card/src/lib.rs:1655` | `probe_card_invalid_channel_apis` → `card::mount_async` | Invalid-channel checks passed in the saved 98-check CARD run |
| `mount_async_owned` | `crates/compatibility/ogc-card/src/lib.rs:325` | `mount_card_owned` → `card::mount_async_owned` | Owned mount checks passed in the saved CARD runs |
| `open` | `crates/compatibility/ogc-card/src/lib.rs:1748` | `probe_card_save_data` → `card::open` | Saved CARD runs pass; see the named-check gaps below |
| `probe` | `crates/compatibility/ogc-card/src/lib.rs:1609` | `probe_card_invalid_channel_apis` → `card::probe` | Invalid-channel checks passed in the saved 98-check CARD run |
| `probe_ex` | `crates/compatibility/ogc-card/src/lib.rs:1616` | `probe_card_invalid_channel_apis` → `card::probe_ex` | Invalid-channel checks passed in the saved 98-check CARD run |
| `probe_info` | `crates/compatibility/ogc-card/src/lib.rs:1553` | `probe_card_invalid_channel_apis` → `card::probe_info` | Invalid-channel checks passed in the saved 98-check CARD run |
| `probe_slot` | `crates/compatibility/ogc-card/src/lib.rs:1548` | `probe_card_save_data` → `card::probe_slot` | Saved CARD runs pass; see the named-check gaps below |
| `read` | `crates/compatibility/ogc-card/src/lib.rs:1671` | `probe_card_save_data` → `card::read` | Saved CARD runs pass; see the named-check gaps below |
| `read_async_owned` | `crates/compatibility/ogc-card/src/lib.rs:738` | `probe_card_save_data` → `card::read_async_owned` | Saved CARD runs pass; see the named-check gaps below |
| `read_slice` | `crates/compatibility/ogc-card/src/lib.rs:1688` | `probe_card_save_data` → `card::read_slice` | Saved CARD runs pass; see the named-check gaps below |
| `sector_size` | `crates/compatibility/ogc-card/src/lib.rs:1569` | `probe_card_save_data` → `card::sector_size` | Saved CARD runs pass; see the named-check gaps below |
| `serial_number` | `crates/compatibility/ogc-card/src/lib.rs:1601` | `probe_card_invalid_channel_apis` → `card::serial_number` | Invalid-channel checks passed in the saved 98-check CARD run |
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
| `create` | `crates/services/ogc-card-sys/src/lib.rs:425` | `probe_card_invalid_channel_apis` → `card::sys::create` | Invalid-channel checks passed in the saved 98-check CARD run |
| `create` | `crates/services/ogc-card-sys/src/wii_backend.rs:3160` | `open_or_create_hotplug_card` → `create` | Not run: called only by skipped hotplug probe |
| `create_async` | `crates/services/ogc-card-sys/src/lib.rs:408` | `probe_card_invalid_channel_apis` → `card::sys::create_async` | Invalid-channel checks passed in the saved 98-check CARD run |
| `create_async` | `crates/services/ogc-card-sys/src/wii_backend.rs:3126` | `probe_card_invalid_channel_apis` → `create_async` | Invalid-channel checks passed in the saved 98-check CARD run |
| `create_entry` | `crates/services/ogc-card-sys/src/lib.rs:453` | `probe_card_invalid_channel_apis` → `card::sys::create_entry` | Invalid-channel checks passed in the saved 98-check CARD run |
| `create_entry` | `crates/services/ogc-card-sys/src/wii_backend.rs:3258` | `probe_card_invalid_channel_apis` → `create_entry` | Invalid-channel checks passed in the saved 98-check CARD run |
| `create_entry_async` | `crates/services/ogc-card-sys/src/lib.rs:437` | `probe_card_invalid_channel_apis` → `card::sys::create_entry_async` | Invalid-channel checks passed in the saved 98-check CARD run |
| `create_entry_async` | `crates/services/ogc-card-sys/src/wii_backend.rs:3229` | `probe_card_invalid_channel_apis` → `create_entry_async` | Invalid-channel checks passed in the saved 98-check CARD run |
| `delete` | `crates/services/ogc-card-sys/src/lib.rs:474` | `probe_card_invalid_channel_apis` → `card::sys::delete` | Invalid-channel checks passed in the saved 98-check CARD run |
| `delete` | `crates/services/ogc-card-sys/src/wii_backend.rs:3289` | `probe_card_invalid_channel_apis` → `delete` | Invalid-channel checks passed in the saved 98-check CARD run |
| `delete_async` | `crates/services/ogc-card-sys/src/lib.rs:464` | `probe_card_invalid_channel_apis` → `card::sys::delete_async` | Invalid-channel checks passed in the saved 98-check CARD run |
| `delete_async` | `crates/services/ogc-card-sys/src/wii_backend.rs:3270` | `probe_card_invalid_channel_apis` → `delete_async` | Invalid-channel checks passed in the saved 98-check CARD run |
| `delete_entry` | `crates/services/ogc-card-sys/src/lib.rs:495` | `probe_card_invalid_channel_apis` → `card::sys::delete_entry` | Invalid-channel checks passed in the saved 98-check CARD run |
| `delete_entry` | `crates/services/ogc-card-sys/src/wii_backend.rs:3355` | `probe_card_invalid_channel_apis` → `delete_entry` | Invalid-channel checks passed in the saved 98-check CARD run |
| `delete_entry_async` | `crates/services/ogc-card-sys/src/lib.rs:485` | `probe_card_invalid_channel_apis` → `card::sys::delete_entry_async` | Invalid-channel checks passed in the saved 98-check CARD run |
| `delete_entry_async` | `crates/services/ogc-card-sys/src/wii_backend.rs:3336` | `probe_card_invalid_channel_apis` → `delete_entry_async` | Invalid-channel checks passed in the saved 98-check CARD run |
| `find_first` | `crates/services/ogc-card-sys/src/lib.rs:505` | `probe_card_save_data` → `card::sys::find_first` | New positive directory check in source; pending Dolphin run |
| `find_first` | `crates/services/ogc-card-sys/src/wii_backend.rs:1975` | `probe_card_save_data` → `find_first` | New positive directory check in source; pending Dolphin run |
| `find_next` | `crates/services/ogc-card-sys/src/lib.rs:515` | `probe_card_save_data` → `card::sys::find_next` | New positive directory check in source; pending Dolphin run |
| `find_next` | `crates/services/ogc-card-sys/src/wii_backend.rs:1997` | `probe_card_save_data` → `find_next` | New positive directory check in source; pending Dolphin run |
| `format` | `crates/services/ogc-card-sys/src/lib.rs:709` | `probe_card_format` → `card::sys::format` | Format checks passed in Wii and GameCube runs |
| `format` | `crates/services/ogc-card-sys/src/wii_backend.rs:3427` | `probe_card_format` → `format` | Format checks passed in Wii and GameCube runs |
| `format_async` | `crates/services/ogc-card-sys/src/lib.rs:705` | `probe_card_invalid_channel_apis` → `card::sys::format_async` | Invalid-channel checks passed in the saved 98-check CARD run |
| `format_async` | `crates/services/ogc-card-sys/src/wii_backend.rs:3408` | `probe_card_invalid_channel_apis` → `format_async` | Invalid-channel checks passed in the saved 98-check CARD run |
| `get_attributes` | `crates/services/ogc-card-sys/src/lib.rs:693` | `probe_card_invalid_channel_apis` → `card::sys::get_attributes` | Invalid-channel checks passed in the saved 98-check CARD run |
| `get_attributes` | `crates/services/ogc-card-sys/src/wii_backend.rs:2118` | `probe_card_invalid_channel_apis` → `get_attributes` | Invalid-channel checks passed in the saved 98-check CARD run |
| `get_banner_fmt` | `crates/services/ogc-card-sys/src/lib.rs:549` | `probe_card_metadata_bits` → `card::sys::get_banner_fmt` | Metadata helper checks passed in the 16-check Dolphin bus run |
| `get_block_count` | `crates/services/ogc-card-sys/src/lib.rs:268` | `probe_card_invalid_channel_apis` → `card::sys::get_block_count` | Invalid-channel checks passed in the saved 98-check CARD run |
| `get_block_count` | `crates/services/ogc-card-sys/src/wii_backend.rs:1692` | `probe_card_invalid_channel_apis` → `get_block_count` | Invalid-channel checks passed in the saved 98-check CARD run |
| `get_directory` | `crates/services/ogc-card-sys/src/lib.rs:526` | `probe_card_save_data` → `card::sys::get_directory` | Saved CARD runs pass; see the named-check gaps below |
| `get_directory` | `crates/services/ogc-card-sys/src/wii_backend.rs:2037` | `probe_card_save_data` → `get_directory` | Saved CARD runs pass; see the named-check gaps below |
| `get_error_code` | `crates/services/ogc-card-sys/src/lib.rs:252` | `probe_card_invalid_channel_apis` → `card::sys::get_error_code` | Invalid-channel checks passed in the saved 98-check CARD run |
| `get_error_code` | `crates/services/ogc-card-sys/src/wii_backend.rs:1669` | `probe_card_invalid_channel_apis` → `get_error_code` | Invalid-channel checks passed in the saved 98-check CARD run |
| `get_free_blocks` | `crates/services/ogc-card-sys/src/lib.rs:276` | `probe_card_invalid_channel_apis` → `card::sys::get_free_blocks` | Invalid-channel checks passed in the saved 98-check CARD run |
| `get_free_blocks` | `crates/services/ogc-card-sys/src/wii_backend.rs:1708` | `probe_card_invalid_channel_apis` → `get_free_blocks` | Invalid-channel checks passed in the saved 98-check CARD run |
| `get_icon_fmt` | `crates/services/ogc-card-sys/src/lib.rs:571` | `probe_card_metadata_bits` → `card::sys::get_icon_fmt` | Metadata helper checks passed in the 16-check Dolphin bus run |
| `get_icon_speed` | `crates/services/ogc-card-sys/src/lib.rs:598` | `probe_card_metadata_bits` → `card::sys::get_icon_speed` | Metadata helper checks passed in the 16-check Dolphin bus run |
| `get_sector_size` | `crates/services/ogc-card-sys/src/lib.rs:260` | `probe_card_invalid_channel_apis` → `card::sys::get_sector_size` | Invalid-channel checks passed in the saved 98-check CARD run |
| `get_sector_size` | `crates/services/ogc-card-sys/src/wii_backend.rs:1676` | `open_or_create_hotplug_card` → `get_sector_size` | Not run: called only by skipped hotplug probe |
| `get_serial_no` | `crates/services/ogc-card-sys/src/lib.rs:720` | `probe_card_invalid_channel_apis` → `card::sys::get_serial_no` | Invalid-channel checks passed in the saved 98-check CARD run |
| `get_serial_no` | `crates/services/ogc-card-sys/src/wii_backend.rs:3686` | `probe_card_invalid_channel_apis` → `get_serial_no` | Invalid-channel checks passed in the saved 98-check CARD run |
| `get_status` | `crates/services/ogc-card-sys/src/lib.rs:541` | `probe_card_invalid_channel_apis` → `card::sys::get_status` | Invalid-channel checks passed in the saved 98-check CARD run |
| `get_status` | `crates/services/ogc-card-sys/src/wii_backend.rs:2073` | `probe_card_invalid_channel_apis` → `get_status` | Invalid-channel checks passed in the saved 98-check CARD run |
| `get_status_ex` | `crates/services/ogc-card-sys/src/lib.rs:647` | `probe_card_invalid_channel_apis` → `card::sys::get_status_ex` | Invalid-channel checks passed in the saved 98-check CARD run |
| `get_status_ex` | `crates/services/ogc-card-sys/src/wii_backend.rs:2093` | `probe_card_invalid_channel_apis` → `get_status_ex` | Invalid-channel checks passed in the saved 98-check CARD run |
| `init` | `crates/services/ogc-card-sys/src/lib.rs:212` | `probe_card_save_data` → `card::sys::init` | Saved CARD runs pass; see the named-check gaps below |
| `init` | `crates/services/ogc-card-sys/src/wii_backend.rs:1602` | `probe_card_save_data` → `init` | Saved CARD runs pass; see the named-check gaps below |
| `mount` | `crates/services/ogc-card-sys/src/lib.rs:302` | `probe_card_mount_lifecycles` → `card::sys::mount` | Sync lifecycle passed in dedicated Wii and GameCube runs |
| `mount` | `crates/services/ogc-card-sys/src/wii_backend.rs:1819` | `probe_card_mount_lifecycles` → `mount` | Sync lifecycle passed in dedicated Wii and GameCube runs |
| `mount_async` | `crates/services/ogc-card-sys/src/lib.rs:287` | `probe_card_invalid_channel_apis` → `card::sys::mount_async` | Invalid-channel checks passed in the saved 98-check CARD run |
| `mount_async` | `crates/services/ogc-card-sys/src/wii_backend.rs:1797` | `probe_card_invalid_channel_apis` → `mount_async` | Invalid-channel checks passed in the saved 98-check CARD run |
| `open` | `crates/services/ogc-card-sys/src/lib.rs:375` | `probe_card_save_data` → `card::sys::open` | Saved CARD runs pass; see the named-check gaps below |
| `open` | `crates/services/ogc-card-sys/src/wii_backend.rs:1835` | `probe_card_save_data` → `open` | Saved CARD runs pass; see the named-check gaps below |
| `open_entry` | `crates/services/ogc-card-sys/src/lib.rs:386` | `probe_card_invalid_channel_apis` → `card::sys::open_entry` | Invalid-channel checks passed in the saved 98-check CARD run |
| `open_entry` | `crates/services/ogc-card-sys/src/wii_backend.rs:1869` | `probe_card_invalid_channel_apis` → `open_entry` | Invalid-channel checks passed in the saved 98-check CARD run |
| `probe` | `crates/services/ogc-card-sys/src/lib.rs:240` | `probe_media_hotplug` → `card::sys::probe` | Not run: automated Dolphin mode skips media hotplug |
| `probe` | `crates/services/ogc-card-sys/src/wii_backend.rs:1620` | `probe_media_hotplug` → `probe` | Not run: automated Dolphin mode skips media hotplug |
| `probe_ex` | `crates/services/ogc-card-sys/src/lib.rs:248` | `probe_card_invalid_channel_apis` → `card::sys::probe_ex` | Invalid-channel checks passed in the saved 98-check CARD run |
| `probe_ex` | `crates/services/ogc-card-sys/src/wii_backend.rs:1627` | `probe_card_invalid_channel_apis` → `probe_ex` | Invalid-channel checks passed in the saved 98-check CARD run |
| `read` | `crates/services/ogc-card-sys/src/lib.rs:335` | `probe_card_save_data` → `card::sys::read` | Saved CARD runs pass; see the named-check gaps below |
| `read` | `crates/services/ogc-card-sys/src/wii_backend.rs:1947` | `probe_card_save_data` → `read` | Saved CARD runs pass; see the named-check gaps below |
| `read_async` | `crates/services/ogc-card-sys/src/lib.rs:318` | `probe_card_invalid_channel_apis` → `card::sys::read_async` | Invalid-channel checks passed in the saved 98-check CARD run |
| `read_async` | `crates/services/ogc-card-sys/src/wii_backend.rs:1909` | `probe_card_invalid_channel_apis` → `read_async` | Invalid-channel checks passed in the saved 98-check CARD run |
| `set_attributes` | `crates/services/ogc-card-sys/src/lib.rs:701` | `probe_card_invalid_channel_apis` → `card::sys::set_attributes` | Invalid-channel checks passed in the saved 98-check CARD run |
| `set_attributes` | `crates/services/ogc-card-sys/src/wii_backend.rs:3660` | `probe_card_invalid_channel_apis` → `set_attributes` | Invalid-channel checks passed in the saved 98-check CARD run |
| `set_attributes_async` | `crates/services/ogc-card-sys/src/lib.rs:697` | `probe_card_invalid_channel_apis` → `card::sys::set_attributes_async` | Invalid-channel checks passed in the saved 98-check CARD run |
| `set_attributes_async` | `crates/services/ogc-card-sys/src/wii_backend.rs:3641` | `probe_card_invalid_channel_apis` → `set_attributes_async` | Invalid-channel checks passed in the saved 98-check CARD run |
| `set_banner_fmt` | `crates/services/ogc-card-sys/src/lib.rs:561` | `probe_card_metadata_bits` → `card::sys::set_banner_fmt` | Metadata helper checks passed in the 16-check Dolphin bus run |
| `set_comment_addr` | `crates/services/ogc-card-sys/src/lib.rs:635` | `probe_card_metadata_bits` → `card::sys::set_comment_addr` | Metadata helper checks passed in the 16-check Dolphin bus run |
| `set_company` | `crates/services/ogc-card-sys/src/lib.rs:235` | `probe_card_save_data` → `card::sys::set_company` | Saved CARD runs pass; see the named-check gaps below |
| `set_gamecode` | `crates/services/ogc-card-sys/src/lib.rs:228` | `probe_card_save_data` → `card::sys::set_gamecode` | Saved CARD runs pass; see the named-check gaps below |
| `set_icon_addr` | `crates/services/ogc-card-sys/src/lib.rs:625` | `probe_card_metadata_bits` → `card::sys::set_icon_addr` | Metadata helper checks passed in the 16-check Dolphin bus run |
| `set_icon_fmt` | `crates/services/ogc-card-sys/src/lib.rs:583` | `probe_card_metadata_bits` → `card::sys::set_icon_fmt` | Metadata helper checks passed in the 16-check Dolphin bus run |
| `set_icon_speed` | `crates/services/ogc-card-sys/src/lib.rs:610` | `probe_card_metadata_bits` → `card::sys::set_icon_speed` | Metadata helper checks passed in the 16-check Dolphin bus run |
| `set_status` | `crates/services/ogc-card-sys/src/lib.rs:673` | `probe_card_invalid_channel_apis` → `card::sys::set_status` | Invalid-channel checks passed in the saved 98-check CARD run |
| `set_status` | `crates/services/ogc-card-sys/src/wii_backend.rs:3559` | `probe_card_invalid_channel_apis` → `set_status` | Invalid-channel checks passed in the saved 98-check CARD run |
| `set_status_async` | `crates/services/ogc-card-sys/src/lib.rs:658` | `probe_card_invalid_channel_apis` → `card::sys::set_status_async` | Invalid-channel checks passed in the saved 98-check CARD run |
| `set_status_async` | `crates/services/ogc-card-sys/src/wii_backend.rs:3515` | `probe_card_invalid_channel_apis` → `set_status_async` | Invalid-channel checks passed in the saved 98-check CARD run |
| `set_status_ex` | `crates/services/ogc-card-sys/src/lib.rs:683` | `probe_card_invalid_channel_apis` → `card::sys::set_status_ex` | Invalid-channel checks passed in the saved 98-check CARD run |
| `set_status_ex` | `crates/services/ogc-card-sys/src/wii_backend.rs:3593` | `probe_card_invalid_channel_apis` → `set_status_ex` | Invalid-channel checks passed in the saved 98-check CARD run |
| `shutdown` | `crates/services/ogc-card-sys/src/lib.rs:724` | `run` → `card::sys::shutdown` | Shutdown status was logged; no separate PASS assertion |
| `shutdown` | `crates/services/ogc-card-sys/src/wii_backend.rs:3704` | `run` → `shutdown` | Shutdown status was logged; no separate PASS assertion |
| `unmount` | `crates/services/ogc-card-sys/src/lib.rs:306` | `probe_card_mount_lifecycles` → `card::sys::unmount` | Sync lifecycle passed in dedicated Wii and GameCube runs |
| `unmount` | `crates/services/ogc-card-sys/src/wii_backend.rs:1823` | `probe_card_mount_lifecycles` → `unmount` | Sync lifecycle passed in dedicated Wii and GameCube runs |
| `write` | `crates/services/ogc-card-sys/src/lib.rs:364` | `probe_card_save_data` → `card::sys::write` | Saved CARD runs pass; see the named-check gaps below |
| `write` | `crates/services/ogc-card-sys/src/wii_backend.rs:2446` | `probe_card_save_data` → `write` | Saved CARD runs pass; see the named-check gaps below |
| `write_async` | `crates/services/ogc-card-sys/src/lib.rs:347` | `probe_card_invalid_channel_apis` → `card::sys::write_async` | Invalid-channel checks passed in the saved 98-check CARD run |
| `write_async` | `crates/services/ogc-card-sys/src/wii_backend.rs:3084` | `probe_card_invalid_channel_apis` → `write_async` | Invalid-channel checks passed in the saved 98-check CARD run |

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
- Raw sys `create_async`, `create_entry_async`, `delete_entry_async`, `read_async`, `write_async`, and metadata async APIs have invalid-channel checks. Owned wrappers have successful async checks, but their raw sys success paths are not isolated in separate assertions.
- `get_attributes`, `set_attributes`, and several status APIs have invalid-channel checks only. Their valid-media behavior needs direct assertions.
- `format_async` has an invalid-channel check. Saved format runs exercise synchronous `format`, not successful async format completion.
- CARD removal, detach callback delivery, stale CARD handles, and reinsert/remount behavior are skipped in Dolphin automation. Physical card behavior remains unverified.
- Four `ogc-card` host-test inventory rows have test source but no current host pass because target-only dependencies block `cargo test -p ogc-card --lib` on this host.
- `ogc-card-metadata` has 18 passing host tests. Its 56 inventory entries still do not have Dolphin runtime evidence.
