# ES closed-service guard coverage

Date: 2026-11-27. Dolphin 2609a. The current `runtime_probe` passed 908 checks
with zero failures. Its DOL SHA-256 is
`86498f4cade553817e15ad007ba460cf153d167f1e2b7d17c665efd4b5afe5f8`. The
[probe log](probe.log) records the checks.

Seven new checks cover `add_title_start`, `add_content_data`, `add_ticket`,
`export_title_init`, `export_content_data`, `reimport_title_init`, and
`delete_ticket`. Each call runs with ES closed and valid but incomplete
arguments. Pointer arguments use valid aligned storage; zero lengths trigger the
argument guards if service-state precedence regresses. It returns `ES_ENOTINIT`, before it checks or reads those arguments.
The probe does not submit any IOS request for these calls.

This evidence confirms closed-service error precedence. It does not verify
successful operations, operation errors with ES open, title mutation, ticket
mutation, export state, UID changes, or physical Wii behavior. No disposable
title fixture or physical recovery run was used.
