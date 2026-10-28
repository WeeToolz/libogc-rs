# Independent USB Storage Read/Write Audit

Date: 2026-10-01

Reviewer: `/root/reviewer_5`  
Scope: independent static cross-check of read/write transfer sequencing, result handling, and `DISC_INTERFACE` read/write callback results against libogc C. This addendum does not re-review close/flush lifecycle. No implementation code, shared review note, or ledger row was changed. No builds, tests, or hardware verification were performed.

## Sources inspected

- Rust: `crates/services/ogc-usbstorage-sys/src/wii_backend.rs`, `crates/services/ogc-usbstorage-sys/src/lib.rs`.
- C: `../../C-sources/libogc/libogc/usbstorage.c`, `../../C-sources/libogc/gc/ogc/usbstorage.h`.
- Existing grouped evidence: `docs/review/libusbstorage.md`.

## Read/write cycle and result behavior

### CSW status mismatch confirmed

C `USBStorage_Read` and `USBStorage_Write` convert a nonzero SCSI status to `USBSTORAGE_ESTATUS` only when `retval > 0` (`usbstorage.c:750-752,787-789`). The BOT cycle's normal success is `USBSTORAGE_OK` (zero), so C returns success for a completed cycle with nonzero CSW status. Rust checks `outcome.result >= 0 && outcome.status != 0` and returns `USBSTORAGE_ESTATUS` (`wii_backend.rs:1277-1280,1340-1343`). The result difference is confirmed and correctly recorded in the main note.

### Retry and reset ordering confirmed

C locks the per-handle mutex around the full cycle, retries up to four times, stops before a new attempt when the prior result is `USBSTORAGE_ETIMEDOUT`, and resets after a negative result (`usbstorage.c:286-349`). Rust acquires its transfer guard around the cycle (`wii_backend.rs:623-635`), retries four times, resets after CBW, data, or CSW errors, and does not special-case timeout (`:645-693`). Rust also converts any nonzero residue into an error before returning (`:688-691`), while C copies residue only to an optional out parameter and does not turn it into failure (`usbstorage.c:271-283,344-349`). These match the existing documented findings.

### Idle recovery and timeout ordering confirmed

C checks the global last-used time before each read/write; after more than 60 seconds it sets the timeout to 10 seconds and calls `USBStorage_MountLUN`, then attempts the transfer, records last-used time, and restores the default timeout (`usbstorage.c:743-757,780-794`). Rust performs input/LUN/size validation then goes directly to `cycle`; no last-used timestamp or timeout adjustment exists in the backend (`wii_backend.rs:1222-1282,1285-1345`). This confirms the behavior recorded in the main note. The C mount result is ignored and the read/write proceeds regardless; Rust has no equivalent wake attempt.

### Input and byte-count handling

C checks only the LUN and cached sector size in `USBStorage_Read`/`Write`; it does not reject a null buffer or zero sector count before entering `__cycle` (`usbstorage.c:740-750,777-787`). Rust rejects null buffers and zero sector counts (`wii_backend.rs:1232-1234,1295-1297`) and checks sector/byte arithmetic for overflow (`:1247-1252,1310-1315`). For a zero-sector request, Rust returns `USBSTORAGE_EINVAL`; C attempts a zero-length SCSI command. These are concrete stricter-validation behavior differences already within the scope described by the main note.

The C block interface callback passes `sec_t numSectors` to the `u16 n_sectors` API (`usbstorage.c:926-947`), which narrows counts above 65,535 for the single call. Rust's callback breaks larger counts into `u16::MAX` chunks, incrementing sector and buffer for each (`wii_backend.rs:1509-1527,1529-1583`). Thus large callback requests produce multiple transfers in Rust versus a narrowed single transfer in C. This is a behavioral difference (and a likely safer Rust handling), though practical filesystem request limits were not checked.

## Callback return and order

C `__usbstorage_ReadSectors`/`WriteSectors` first check `__mounted`, call the corresponding API, and return `retval >= 0` (`usbstorage.c:926-948`). Rust's corresponding `DISC_INTERFACE` callbacks call backend read/write helpers (`ogc-usbstorage-sys/src/lib.rs:366-401`); those return false when the disc is not mounted, and otherwise return the result of the sector-transfer loop, which fails only for negative per-transfer results (`wii_backend.rs:1509-1527,1529-1583`). For ordinary status results, both callbacks use nonnegative as success, so the bool mapping and basic order match. A successful SCSI command with nonzero CSW status remains a distinction because Rust's inner `read`/`write` maps that result to a negative error before the bool callback sees it; C's inner API retains its `retval > 0` condition.

The callback ABI shape also matches at the fields checked: Rust callback parameters use 32-bit sector/count values and the C `DISC_INTERFACE` operations use `sec_t` values (`usbstorage.h:65-86`; Rust `DiscInterface` and callback declarations in `ogc-usbstorage-sys/src/lib.rs:36-60,339-401`).

## Result

The main note's read/write findings for timeouts, retries, CSW status, residue, and validation are confirmed. The disc callback's ordinary nonnegative-result mapping and call order match C. One additional static behavior difference is that Rust splits callback counts above `u16::MAX` into multiple operations whereas C narrows the count at the API call. No additional callback result/order mismatch was established beyond the status behavior already documented. Keep the crate open for the previously listed differences.
