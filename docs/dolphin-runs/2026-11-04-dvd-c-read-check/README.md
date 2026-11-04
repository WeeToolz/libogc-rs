# Synthetic DVD with corrected C read check

Dolphin 2609a ran the existing synthetic disc fixture against `dvd_probe`.
The [log](probe.log) records 34 passes and five failures. DVD mount, direct DI
inserted state, direct DI disc ID, and the Rust and C direct DI reads failed.
Both direct reads returned 1. The C read check now requires the fixture header
and a successful result; the previous probe could pass when both read buffers
and errors matched. DVD sector reads through the DVD API still passed. The
[suite report](suite.json) records the runner result and DOL hash.

This fixture does not contain a signed Wii partition or a filesystem. It does
not establish successful mount or direct DI media operation. Async direct DI
reads were skipped after fixture verification failed.
