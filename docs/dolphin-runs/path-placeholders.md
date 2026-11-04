# Paths in saved Dolphin evidence

Saved reports and logs use portable placeholders in place of machine-specific
absolute paths. This changes path text only. Probe results, numeric fields,
DOL hashes, fixture hashes, and source revisions retain their recorded values.

- `${REPO_ROOT}`: root of this repository.
- `${WIIDEV_ROOT}`: external Wii development workspace.
- `${USER_HOME}`: home directory used for the run.
- `${TMPDIR}`: temporary directory used for the run.
- `${DOLPHIN_BIN}`: Dolphin executable selected for the run.
- `${DEVKITPRO}`: devkitPro installation root.
- `${LLVM_NM}`: LLVM symbol reader executable.

In JSON and saved log text these placeholders are descriptive; a reader does
not need to expand them to inspect the results. Set the corresponding variables
before using them in a shell command. Keep actual machine-specific paths in
the untracked `DOLPHIN.local.md` guide.

The reachable history was also rewritten to remove the original path text.
See the [history cleanup record](../history-rewrite.md) and its commit map.
