# Dolphin run records

These files preserve probe results and runtime logs. Local machine paths use
portable labels. For example, `[repo]` marks the repository root,
`[workspace temp]` marks a temporary workspace directory, and
`[Dolphin user data]` marks Dolphin's user-data directory. These labels are
redactions. They are not literal paths. Other labels identify external tools,
temporary evidence, or a connected device volume. `[Dolphin executable]` marks
the Dolphin program used for a run. The reports keep relative artifact paths
when those paths identify files in this repository.

Three archived DOL files had local build paths in their debug data. The DOL
files are removed. Their run reports keep the SHA-256 values that identify the
original files used for those runs. The logs and probe results remain.
