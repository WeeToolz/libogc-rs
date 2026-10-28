# Independent review: USB storage LUN count range

Date: 2026-10-05
Reviewer: `worker_a`
Implementer: `worker_b`
Commit reviewed: `38344b5` (`Support representable USB storage LUN counts`)

## Result

**PASS for the changed LUN count range.** Rust now accepts the C-representable count range of 1 through 255 and rejects a reported maximum index of 255 instead of wrapping the count to zero. This is a deliberate safe difference for the C ABI overflow case. The crate remains open for the separate GetMaxLUN timeout mismatch below.

C reads the one-byte highest LUN index, stores `index + 1` in the `u8 max_lun` field, and allocates that many sector-size entries (`C-sources/libogc/libogc/usbstorage.c:499-515`; `C-sources/libogc/gc/ogc/usbstorage.h:38`). Rust's `lun_count_from_reported_max` uses checked addition (`crates/services/ogc-usbstorage-sys/src/lib.rs:30-40`), and `open` rejects the unrepresentable 256 count (`src/wii_backend.rs:1187-1194`). The static slot now has 255 sector-size entries, and indexing remains guarded by `lun < max_lun` (`src/wii_backend.rs:45-70,1223,1324-1329`). The loop and BOT command paths retain the `u8` LUN values used by C.

The unit test and probe use independent expected counts at 0, 7, 8, 254, and 255. Both call the same conversion helper used by the production open path. The probe does not issue a real GetMaxLUN request; its note correctly states this limit.

## Adjacent open mismatch: GetMaxLUN timeout

C falls back to one LUN for a negative GetMaxLUN result, but it then checks `retval == USBSTORAGE_ETIMEDOUT` and returns through cleanup with the timeout (`usbstorage.c:499-509`). Rust maps every error from `class_get_max_lun` to one LUN and continues to report `USBSTORAGE_OK` (`crates/services/ogc-usbstorage-sys/src/wii_backend.rs:1187-1207`). This behavior predates `38344b5`, but the new handoff's statement that a failed request continues with one LUN “matching C” needs a timeout exception. Rust should preserve and return `USBSTORAGE_ETIMEDOUT` while aborting open, as C does.

## Validation evidence

I ran `cargo xtask dolphin-validate --no-build --probe usbstorage_capacity_probe`. The report at `target/dolphin-validation/1791173337-fe5290d62488/suite.json` records Dolphin 2603-415, `status=pass`, and 6 passes / 0 failures. The DOL SHA-256 is `5ab64489faf8a5052345f421ce8e26f9157beeaf6473740d5eae2a3f7a46e6e4`. The probe log records the full representable GetMaxLUN range check as passed. This run validates the helper in Dolphin; it does not validate a USB device or live GetMaxLUN transport. No hardware verification is claimed.
