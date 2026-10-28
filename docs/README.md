# Documentation guide

Use this guide to find the current state of the project and the evidence behind it.

- [Implementation status](implementation-status.md) describes completed work,
  open work, and acceptance limits. The [generated inventory](generated/implementation-status.md)
  gives the detailed crate and API inventory.
- [Evidence and open work](evidence-summary.md) summarizes the ledgers and
  explains what each type of evidence can establish.
- [Architecture](architecture.md) and [crate roles](crate-roles.md) explain the
  workspace boundaries. [API documentation](api.md) describes the API surface.
- [License and source notices](source-licenses.md) identify the known upstream
  licenses and remaining provenance review.
- [Selected upstream notices](third-party-notices.md) retain attribution from
  the C reference files.
- [External dependencies](dependencies.md) records tools and source checkouts
  that are not included in this repository.
- [Wii hardware validation](wii-hardware-validation.md) explains the physical
  test procedure and evidence requirements. [Probe instructions](../probes/README.md)
  describe target probes. Dolphin results do not replace physical hardware results.
- [Wii standard library checklist](wii-std-checklist.md) tracks its implementation
  and acceptance work. [Contract gaps](wii-std-contract-gaps.md) records limits.
- [Review records](review/) contain detailed component findings and evidence.
  Use the implementation status to find current priorities before reading the
  individual records. [Hardware evidence](hardware-evidence/) contains saved
  run evidence; check its dates and scope before using it for a claim.

The top-level [README](../README.md) contains setup, build, and test commands.
For agent work, read [AGENTS.md](../AGENTS.md) and the parent instructions first.
