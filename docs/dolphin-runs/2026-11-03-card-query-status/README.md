# CARD typed query status diagnostic

This run used `storage_probe` in Dolphin 2609a with no GameCube memory card
image. The probe reached the CARD query calls. The log records:

| API | Result |
| --- | ---: |
| `card::probe_slot(CardSlot::A)` | `CARD_ERROR_BUSY` (`-1`) |
| `card::probe_info(CardSlot::A)` | `CARD_ERROR_BUSY` (`-1`) |
| `card::sector_size(CardSlot::A)` | `CARD_ERROR_NOCARD` (`-3`) |
| `card::block_count(CardSlot::A)` | `CARD_ERROR_NOCARD` (`-3`) |
| `card::free_blocks(CardSlot::A)` | `CARD_ERROR_NOCARD` (`-3`) |
| `card::file_status(CardSlot::A, 0)` | `CARD_ERROR_NOCARD` (`-3`) |

The four typed query assertions in this run expected the raw slot-probe error,
so they failed. The probe source now expects `CARD_ERROR_NOCARD` for these
typed queries when the slot reports busy or no card. This diagnostic is not a
passing test. The corrected assertions need a fresh Dolphin run. A later
attempt on 2026-11-03 stopped at the runner's `Dolphin --version` check with
`SIGABRT`; it did not start the probe.

This run has no inserted card. It does not check successful queries, card
geometry, file status on a mounted card, or physical Wii behavior. The
[probe log](storage_probe/probe.log) retains the diagnostic output, including
the four failed assertions. The earlier
[`no-media-pass`](../2026-11-03-card-query-no-media-pass/suite.json) run
passed its existing checks but skipped these query assertions. The
[`error-propagation`](../2026-11-03-card-query-error-propagation/suite.json)
run repeated the same mistaken expectation, and the
[`state`](../2026-11-03-card-query-state/suite.json) run recorded statuses
without asserting them. Keep those reports as diagnostic history; none proves
the corrected checks.
