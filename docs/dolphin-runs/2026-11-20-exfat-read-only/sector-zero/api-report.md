# Dolphin evidence report

Probe `exfat_sd_probe` reported `pass`.
Passed checks 27. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-exfat::mount | exFAT mounted from Wii SD | PASS | [log L77](exfat_sd_probe/probe.log#L77), [page-002](exfat_sd_probe/pages/page-002.png) |
| ogc-exfat::mount | exFAT rejects a corrupted boot checksum | PASS | [log L94](exfat_sd_probe/probe.log#L94), [page-003](exfat_sd_probe/pages/page-003.png) |
| ogc-exfat::mount | exFAT rejects a corrupted directory entry checksum | PASS | [log L95](exfat_sd_probe/probe.log#L95), [page-003](exfat_sd_probe/pages/page-003.png) |
| ogc-exfat::mount | exFAT propagates a failed device read | PASS | [log L96](exfat_sd_probe/probe.log#L96), [page-003](exfat_sd_probe/pages/page-003.png) |
| ogc-exfat::stat | exFAT stat preserves name and size with case-insensitive lookup | PASS | [log L80](exfat_sd_probe/probe.log#L80), [page-002](exfat_sd_probe/pages/page-002.png) |
| ogc-exfat::stat | exFAT stat rejects a missing path | PASS | [log L81](exfat_sd_probe/probe.log#L81), [page-002](exfat_sd_probe/pages/page-002.png) |
| ogc-exfat::stat | exFAT rejects an invalid path component | PASS | [log L83](exfat_sd_probe/probe.log#L83), [page-003](exfat_sd_probe/pages/page-003.png) |
| ogc-exfat::visit_dir | exFAT root lists the expected file | PASS | [log L78](exfat_sd_probe/probe.log#L78), [page-002](exfat_sd_probe/pages/page-002.png) |
| ogc-exfat::visit_dir | exFAT lists a nested directory | PASS | [log L79](exfat_sd_probe/probe.log#L79), [page-002](exfat_sd_probe/pages/page-002.png) |
| ogc-exfat::visit_dir | exFAT rejects listing a file | PASS | [log L84](exfat_sd_probe/probe.log#L84), [page-003](exfat_sd_probe/pages/page-003.png) |
| ogc-exfat::open | exFAT opened the expected file | PASS | [log L86](exfat_sd_probe/probe.log#L86), [page-003](exfat_sd_probe/pages/page-003.png) |
| ogc-exfat::open | exFAT opens a fragmented file | PASS | [log L91](exfat_sd_probe/probe.log#L91), [page-003](exfat_sd_probe/pages/page-003.png) |
| ogc-exfat::open | exFAT rejects opening a directory as a file | PASS | [log L85](exfat_sd_probe/probe.log#L85), [page-003](exfat_sd_probe/pages/page-003.png) |
| ogc-exfat::read | exFAT read matches the independent payload | PASS | [log L87](exfat_sd_probe/probe.log#L87), [page-003](exfat_sd_probe/pages/page-003.png) |
| ogc-exfat::read | exFAT follows a nonadjacent FAT link and zero fills the uninitialized tail | PASS | [log L92](exfat_sd_probe/probe.log#L92), [page-003](exfat_sd_probe/pages/page-003.png) |
| ogc-exfat::read | exFAT end-of-file read preserves the output buffer | PASS | [log L89](exfat_sd_probe/probe.log#L89), [page-003](exfat_sd_probe/pages/page-003.png) |
| ogc-exfat::seek | exFAT seek and read use the requested offset | PASS | [log L88](exfat_sd_probe/probe.log#L88), [page-003](exfat_sd_probe/pages/page-003.png) |
| ogc-exfat::seek | exFAT can seek backward after following a FAT link | PASS | [log L93](exfat_sd_probe/probe.log#L93), [page-003](exfat_sd_probe/pages/page-003.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.
