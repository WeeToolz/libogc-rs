# Dolphin evidence report

Probe `runtime_probe` reported `pass`.
Passed checks 687. Failed checks 0.

| API | Required check | Result | Evidence |
| --- | --- | --- | --- |
| ogc-exi::attach | EXI attach rejects an invalid channel | PASS | [log L417](runtime_probe/probe.log#L417), [page-016](runtime_probe/pages/page-016.png) |
| ogc-exi::detach | EXI detach rejects an invalid channel | PASS | [log L418](runtime_probe/probe.log#L418), [page-016](runtime_probe/pages/page-016.png) |
| ogc-exi::get_id | EXI device ID query rejects an invalid channel | PASS | [log L419](runtime_probe/probe.log#L419), [page-016](runtime_probe/pages/page-016.png) |
| ogc-exi::read_id | EXI safe device ID query maps an invalid channel to an error | PASS | [log L421](runtime_probe/probe.log#L421), [page-017](runtime_probe/pages/page-017.png) |
| ogc-exi::try_attach | EXI try attach maps an invalid channel to an error | PASS | [log L422](runtime_probe/probe.log#L422), [page-017](runtime_probe/pages/page-017.png) |
| ogc-exi::try_detach | EXI try detach maps an invalid channel to an error | PASS | [log L423](runtime_probe/probe.log#L423), [page-017](runtime_probe/pages/page-017.png) |
| ogc-exi::try_probe | EXI try probe reports no device on an invalid channel | PASS | [log L424](runtime_probe/probe.log#L424), [page-017](runtime_probe/pages/page-017.png) |
| ogc-exi::try_probe_ex | EXI try extended probe maps an invalid channel to an error | PASS | [log L425](runtime_probe/probe.log#L425), [page-017](runtime_probe/pages/page-017.png) |

## Evidence gaps

- All listed check labels passed. Review the behavior and coverage separately.

## Coverage limits

- All calls use channel -1 and test argument rejection or result conversion. They do not test a connected EXI device or normal bus transfers.
