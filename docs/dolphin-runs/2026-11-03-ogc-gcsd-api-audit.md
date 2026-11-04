# `ogc-gcsd` API by API Dolphin audit

## Scope and run

The generated inventory contains 44 `ogc-gcsd` rows: five Rust wrapper APIs,
19 C entry points, and 20 Wii backend functions. This audit records the direct
probe call, its named Dolphin check, and the limit of that result. A source
mapping alone does not count as runtime evidence.

The latest successful `fat_sd_probe` run used Dolphin 2609a and had 45 passes
with no failures. Its DOL SHA-256 is
`495942312134003a4c376e2fa6d4f778fbc056b840ddbc0492288408cd3989aa`.
The run used the no-adapter GameCube SD path and Dolphin's virtual Wii SD for
the separate FAT checks. It did not use an EXI SD Gecko. See the [suite
report](2026-11-03-gcsd-setter-api-final/suite.json),
[probe transcript](2026-11-03-gcsd-setter-api-final/fat_sd_probe/probe.log),
and [DOL result](2026-11-03-gcsd-setter-api-final/fat_sd_probe/result.json).
A fresh build produced the same DOL hash. A later Dolphin rerun stopped at its
version preflight before it started the DOL. See the
[preflight failure record](2026-11-03-gcsd-setter-api-reviewed/README.md).

## Rust wrapper APIs

| API | Direct call and assertion | Evidence and limit |
| --- | --- | --- |
| `sdgecko_get_speed` | Invalid drive returns `None`; slot 0 getter is compared with C getter. | `Rust SD Gecko getters reject an invalid drive`; `SD Gecko backend getters return slot A state`; `C SD Gecko getters expose valid slot configuration`. Slot 0 config does not require an adapter. |
| `sdgecko_set_speed` | Invalid drive returns false; valid slot changes and restores speed. | `Rust SD Gecko setters reject an invalid drive`; `Rust SD Gecko speed setter changes drive state`; `C SD Gecko speed setter updates and restores slot state`. This verifies stored configuration only. |
| `sdgecko_get_page_size` | Invalid drive returns `None`; slot 0 state is read before and after setting. | `Rust SD Gecko getters reject an invalid drive`; `SD Gecko backend getters return slot A state`; `C SD Gecko getters expose valid slot configuration`. |
| `sdgecko_set_page_size` | Invalid drive returns `None`; no-card result is negative, and old slot state is restored. | `Rust SD Gecko setters reject an invalid drive`; `SD Gecko page-size setter restores slot state after no-card errors`. No CMD16 success is established. |
| `sdgecko_get_addressing_type` | Invalid drive returns `None`; slot 0 reports byte addressing. | `Rust SD Gecko getters reject an invalid drive`; `SD Gecko backend getters return slot A state`; `C SD Gecko getters expose valid slot configuration`. This is configured state, not a card inquiry. |

## C entry points

| API | Direct call and assertion | Evidence and limit |
| --- | --- | --- |
| `sdgecko_initIODefault` | Calls default initialization and checks speed restoration. | `C SD Gecko default initialization restores the original speed`. |
| `sdgecko_initIO` | Invalid drive and adapter-absent slot calls return no-card. | `C SD Gecko lifecycle and metadata APIs reject an invalid drive`; `SD Gecko init and metadata APIs report no card in slot A`. |
| `sdgecko_preIO` | Invalid drive and adapter-absent slot calls return no-card. | `C SD Gecko lifecycle and metadata APIs reject an invalid drive`; `SD Gecko init and metadata APIs report no card in slot A`. |
| `sdgecko_readCID` | Invalid drive and adapter-absent slot calls return no-card. | `C SD Gecko lifecycle and metadata APIs reject an invalid drive`; `SD Gecko init and metadata APIs report no card in slot A`. CID bytes are not read. |
| `sdgecko_readCSD` | Invalid drive and adapter-absent slot calls return no-card. | `C SD Gecko lifecycle and metadata APIs reject an invalid drive`; `SD Gecko init and metadata APIs report no card in slot A`. CSD bytes are not read. |
| `sdgecko_readStatus` | Invalid drive and adapter-absent slot calls return no-card. | `C SD Gecko lifecycle and metadata APIs reject an invalid drive`; `SD Gecko init and metadata APIs report no card in slot A`. |
| `sdgecko_readSectors` | Invalid drive and adapter-absent slot preserve the no-card error. | `C SD Gecko sector APIs reject an invalid drive`; `SD Gecko sector result APIs preserve no-card errors`. No successful transfer is tested. |
| `sdgecko_writeSectors` | Invalid drive and adapter-absent slot preserve the no-card error. | `C SD Gecko sector APIs reject an invalid drive`; `SD Gecko sector result APIs preserve no-card errors`. No successful write is tested. |
| `sdgecko_doUnmount` | Invalid drive returns no-card; slot A unmount succeeds with no card. | `C SD Gecko lifecycle and metadata APIs reject an invalid drive`; `SD Gecko unmount succeeds when slot A has no card`. |
| `sdgecko_insertedCB` | Called for slot 0 while checking speed state remains unchanged. | `C SD Gecko callbacks and invalid speed setter preserve drive state`. The callback table has no installed callbacks; this does not prove user callback delivery. |
| `sdgecko_ejectedCB` | Called for slot 0 while checking speed state remains unchanged. | `C SD Gecko callbacks and invalid speed setter preserve drive state`. The callback table has no installed callbacks; this does not prove user callback delivery. |
| `sdgecko_getSpeed` | Invalid drive returns the documented default; valid slot is compared with the Rust getter. | `C SD Gecko getters use documented invalid-drive defaults`; `C SD Gecko getters expose valid slot configuration`. |
| `sdgecko_setSpeed` | Invalid drive is ignored; valid slot speed changes and is restored through the Rust setter. | `C SD Gecko callbacks and invalid speed setter preserve drive state`; `C SD Gecko speed setter updates and restores slot state`. The C API has no return status. |
| `sdgecko_getPageSize` | Invalid drive returns the documented default; valid slot is compared with the Rust getter. | `C SD Gecko getters use documented invalid-drive defaults`; `C SD Gecko getters expose valid slot configuration`. |
| `sdgecko_setPageSize` | Invalid drive returns no-card; valid slot returns a negative backend CMD16 result, stores the requested setting, then restores the original setting. | `C SD Gecko getters use documented invalid-drive defaults`; `C SD Gecko page-size setter reports no card and restores slot state`. This does not prove CMD16 success. |
| `sdgecko_getAddressingType` | Invalid drive returns sector-addressing default; valid slot is compared with the Rust getter. | `C SD Gecko getters use documented invalid-drive defaults`; `C SD Gecko getters expose valid slot configuration`. |
| `sdgecko_initBufferPool` | Initializes the fixed pool before allocation. | `C SD Gecko buffer pool allocates, exhausts, frees, and reuses buffers`. |
| `sdgecko_allocBuffer` | Gets three distinct buffers, observes exhaustion, frees them, then observes reuse. | `C SD Gecko buffer pool allocates, exhausts, frees, and reuses buffers`. |
| `sdgecko_freeBuffer` | Frees only buffers returned by this pool, then confirms a freed slot can be reused. | `C SD Gecko buffer pool allocates, exhausts, frees, and reuses buffers`. |

## Wii backend functions

| API | Direct call and assertion | Evidence and limit |
| --- | --- | --- |
| `get_speed` | Reads slot 0's configured speed. | `SD Gecko backend getters return slot A state`; no adapter is needed for the software setting. |
| `set_speed` | Changes the slot 0 setting and restores it. | `Rust SD Gecko speed setter changes drive state`; no EXI command is issued. |
| `get_page_size` | Reads slot 0's configured page size before and after a set. | `SD Gecko backend getters return slot A state`; software state only. |
| `set_page_size` | Updates page size; captures the no-card CMD16 result; restores the previous value. | `SD Gecko page-size setter restores slot state after no-card errors`; successful CMD16 remains open. |
| `get_addressing_type` | Reads slot 0 byte-addressing mode. | `SD Gecko backend getters return slot A state`; software state only. |
| `init_io_default` | Restores default speed configuration. | `C SD Gecko default initialization restores the original speed`. |
| `init_io` | Invalid drive and slot A without an adapter return no-card. | `C SD Gecko lifecycle and metadata APIs reject an invalid drive`; `SD Gecko init and metadata APIs report no card in slot A`. |
| `pre_io` | Invalid drive and slot A without an adapter return no-card. | `C SD Gecko lifecycle and metadata APIs reject an invalid drive`; `SD Gecko init and metadata APIs report no card in slot A`. |
| `read_cid_api` | Invalid drive and slot A without an adapter return no-card. | `C SD Gecko lifecycle and metadata APIs reject an invalid drive`; `SD Gecko init and metadata APIs report no card in slot A`. |
| `read_csd_api` | Invalid drive and slot A without an adapter return no-card. | `C SD Gecko lifecycle and metadata APIs reject an invalid drive`; `SD Gecko init and metadata APIs report no card in slot A`. |
| `read_status_api` | Invalid drive and slot A without an adapter return no-card. | `C SD Gecko lifecycle and metadata APIs reject an invalid drive`; `SD Gecko init and metadata APIs report no card in slot A`. |
| `do_unmount` | Invalid drive returns no-card; absent-card slot A returns success. | `SD Gecko unmount succeeds when slot A has no card`. |
| `startup` | Calls the slot A interface startup. | `slot A interface callbacks report no card and clean up`; result is false without an adapter. |
| `is_inserted` | Calls the slot A insertion check. | `slot A interface callbacks report no card and clean up`; result is false without an adapter. |
| `read_sectors_result` | Reads one sector with an absent adapter and checks no-card. | `SD Gecko sector result APIs preserve no-card errors`; success remains untested. |
| `read_sectors` | Calls the slot A interface read callback with an absent adapter. | `slot A interface callbacks report no card and clean up`; result is false. |
| `write_sectors_result` | Writes one sector with an absent adapter and checks no-card. | `SD Gecko sector result APIs preserve no-card errors`; success remains untested. |
| `write_sectors` | Calls the slot A interface write callback with an absent adapter. | `slot A interface callbacks report no card and clean up`; result is false. |
| `clear_status` | Calls the slot A status cleanup callback with no adapter. | `slot A interface callbacks report no card and clean up`; result is false. |
| `shutdown` | Calls the slot A shutdown callback after startup/read/write attempts. | `slot A interface callbacks report no card and clean up`; result is true and confirms cleanup. |

## Remaining checks

All 44 inventory rows have source-level evidence, and the checks above ran in
Dolphin. The assertion audit shows that grouped checks do not mean successful
card I/O. The adapter is absent in Dolphin, so SD Gecko CID/CSD/status reads,
CMD16 success, sector reads and writes, insertion events, and physical callback
timing remain unverified. Dolphin's Wii virtual SD tests exercise `ogc-wiisd`,
not this EXI/GameCube SD Gecko backend. A physical SD Gecko probe is required
for those paths.
