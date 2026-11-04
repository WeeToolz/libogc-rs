# Paths in saved Dolphin evidence

Saved reports and logs use portable placeholders in place of machine-specific
absolute paths. This changes path text only. Probe results, numeric fields,
DOL hashes, fixture hashes, and source revisions retain their recorded values.

- `${REPO_ROOT}`: root of this repository.
- `${WIIDEV_ROOT}`: external Wii development workspace.
- `${USER_HOME}`: home directory used for the run.
- `${TMPDIR}`: temporary directory used for the run.

In JSON and saved log text these placeholders are descriptive; a reader does
not need to expand them to inspect the results. Set the corresponding variables
before using them in a shell command. Keep actual machine-specific paths in
the untracked `DOLPHIN.local.md` guide.

This cleanup affects the current files. Earlier Git commits can still contain
the original path text; no repository history was rewritten.
