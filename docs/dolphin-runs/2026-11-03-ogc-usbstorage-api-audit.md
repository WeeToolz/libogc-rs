# `ogc-usbstorage` API runtime audit

## Scope and result

The generated implementation inventory contains 30 `ogc-usbstorage` rows,
65 `ogc-usbstorage-sys` rows, and 20 `ogc-usbstorage-lifecycle` rows. The 65
sys rows name 48 distinct symbols. They include both public sys wrappers and
their private Wii backend functions. A source mapping shows where code runs;
it does not prove that a runtime assertion passed.

The current [`usbstorage_capacity_probe` run](usb-capacity-api-current/README.md)
passed 52 checks in Dolphin 2609a with no USB mass-storage device. The
[interface callback run](usb-interface-no-device/README.md) passed separate
checks for both public drive slots. These runs prove the listed no-device,
argument, output-preservation, and shared-policy behavior only. They do not
prove successful USB transfers.

## Safe facade: 30 inventory entries

Each table row lists the raw function and its `try_` wrapper. The probe calls
every listed `try_` wrapper. The wrapper calls the raw function, then maps its
result. A separate raw result is checked only when the assertion names the raw
call. Some assertions combine calls; none proves device-backed success.

| APIs | Dolphin assertion | Result scope |
| --- | --- | --- |
| `initialize`, `try_initialize` | `safe USB initialize reports success or no-Wii error`; `usb raw startup` | Startup and error mapping without storage media. |
| `deinitialize`, `try_deinitialize` | `safe USB deinitialize completes after the no-device probe`; `usb storage deinitialize` | No-device teardown result. |
| `open`, `try_open` | `safe USB open preserves the raw no-device error and cleans its handle` | Open error and handle cleanup with no matching device. |
| `close`, `try_close` | `raw and safe USB close reject null handles` | Null-handle error mapping. |
| `flush`, `try_flush` | `raw and safe USB flush map null handle to interface error` | Null-handle error mapping. |
| `flush_default`, `try_flush_default` | `default USB flush reports no mounted storage` | No-mounted-drive result. |
| `reset`, `try_reset` | `safe USB reset maps null handle to interface error` | Null-handle error mapping. |
| `get_max_lun`, `try_get_max_lun` | `safe USB GetMaxLUN maps null handle to interface error`; `usb storage null handle rejected` | Null-handle error mapping. |
| `mount_lun`, `try_mount_lun` | `safe USB mount maps null handle to interface error` | Null-handle error mapping. |
| `suspend`, `try_suspend` | `safe USB suspend maps null handle to interface error` | Null-handle error mapping. |
| `is_dvd`, `try_is_dvd` | `safe USB DVD query maps the raw no-device result` | No-device result and typed bool conversion. |
| `read_capacity`, `try_read_capacity` | `safe USB capacity maps null handle to interface error`; `raw safe USB capacity rejects a null handle and preserves outputs` | Null-handle error and output preservation. |
| `read`, `try_read` | `raw and safe USB reads reject null handles and preserve output` | Null-handle error and buffer preservation. |
| `write`, `try_write` | `raw and safe USB writes reject null handles` | Null-handle error mapping. |
| `start_stop`, `try_start_stop` | `safe USB start-stop reports a missing storage interface` | Missing-interface result and typed error conversion. |

## Sys wrappers and C ABI

The current probe calls the sys Rust wrappers, the exported C functions, and
the shared Wii backend through those public paths. Backend rows are not separate
success-path tests. Their current direct results are limited to no-device and
invalid-input behavior.

| APIs | Assertion in `usbstorage_capacity_probe` | Result scope |
| --- | --- | --- |
| `c_initialize` | `C USB initialize reports success or no-Wii error` | No-device initialization result. |
| `c_deinitialize`, `c_last_deinitialize_status`, `last_deinitialize_status` | `C USB deinitialize records its result for C callers` | C and Rust status handoff. |
| `c_open` | `C USB open rejects a null storage handle` | Null-handle error. |
| `c_close`, `c_reset` | `C USB close and reset reject null storage handles` | Null-handle errors. |
| `c_get_max_lun`, `c_mount_lun` | `C USB GetMaxLUN and mount reject null storage handles` | Null-handle errors. |
| `c_suspend`, `c_start_stop` | `C USB suspend and start-stop reject null handles` | Null-handle errors. |
| `c_inquiry` | `C USB inquiry rejects a null storage handle` | Null-handle error. |
| `c_read_capacity` | `C USB capacity rejects null handle and preserves outputs` | Error and preservation of both output values. |
| `c_read`, `c_write` | `C USB read and write reject null storage handles` | Null-handle errors. |
| `c_is_dvd`, `is_dvd` | `C USB DVD query reports no mounted DVD`; `raw USB DVD query reports no mounted DVD` | No-mounted-DVD result. |
| `ioctl`, `__ogc_usbstorage_ioctl_bridge` | `USB ioctl and C bridge reject an unmounted request` | Unmounted request rejection. |
| `flush_drive` | `USB drive flush reports absent media and rejects invalid slots` | Both valid slots return no-interface; slot 2 returns invalid-argument. No mounted-drive flush is tested. |
| `clear_status` and the public interface callbacks | `USB disc-interface clear-status callback succeeds`; see the [slot A and B run](usb-interface-no-device/README.md) | Clear-status on the no-device path; each slot checks startup, insertion, reads, writes, shutdown, and restart. |
| `open`, `close`, `flush`, `flush_default`, `reset`, `get_max_lun`, `mount_lun`, `suspend`, `inquiry`, `read_capacity`, `read`, `write`, `start_stop` | Matching `raw and safe`, `raw USB`, and `USB inquiry` checks in both no-device sections | Wrapper/backend error paths only; no valid handle reaches BOT/SCSI media I/O. |

## Shared USB policy functions

The probe calls each helper with fixed inputs and checks fixed expected results.
These checks exercise shared production logic, but they do not send USB
commands:

| APIs | Assertion |
| --- | --- |
| `open_requires_bot_reset` | `open reset decision matches the C USB mode predicate` |
| `idle_remount_required` | `USB idle remount uses a strict sixty-second boundary` |
| `lun_count_from_reported_max` | `GetMaxLUN supports the full representable C count` |
| `lun_count_after_get_max_lun` | `GetMaxLUN timeout aborts while other errors default to LUN zero` |
| `copy_read_result` | `short USB read copies its received prefix before returning residue`; `zero-byte USB read returns data residue` |
| `transfer_result` | `zero-byte USB write result returns data residue`; `USB data transfer preserves non-timeout transport errors` |
| `bot_cycle_retry_error` | `BOT retries preserve cycle errors unless reset or transfer times out` |
| `read_write_cycle_result` | `USB read/write cycle status preserves C result rules` |
| `bot_csw_cycle_result`, `decode_read_capacity` | `ReadCapacity accepts CSW status and residue, then decodes capacity`; `ReadCapacity preserves a negative BOT cycle result` |
| `async_control_cleanup_failures` | `USB async control cleanup counter remains clear` |

`probe_status` has a host test in the generated inventory. This audit does not
claim a Dolphin assertion for it. `ogc-usbstorage-lifecycle` has 20 inventory
rows mapped to host tests. `cargo test -p ogc-usbstorage-lifecycle` passed 21
tests in this audit. These tests do not replace Dolphin timing and transport
evidence.

## Remaining USB evidence

- No USB mass-storage device is attached in Dolphin. BOT/SCSI inquiry,
  capacity, read, write, reset, suspend, resume, cache flush, removal, and
  successful unmount on media remain unverified at runtime.
- `usb_volumes_probe` requires two devices with known fixture files. It has not
  passed in Dolphin, so two-device selection and `flush_drive` on mounted
  media remain open.
- Physical USB callback timing and hardware behavior remain unverified.
