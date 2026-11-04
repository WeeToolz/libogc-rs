# Machine-specific path removal from history

The repository history was rewritten to remove recorded machine-specific
paths from every reachable version, including NUL-containing evidence logs.
The rewrite preserved all 221 existing commits, their order and parent graph,
authors, committers, timestamps, and messages. One original commit kept its ID;
220 changed IDs because their content or ancestry changed.

Every rewritten file tree was compared with the corresponding original tree.
Only the defined path substitutions were permitted. No source, test result,
report, or commit was removed. Historical reports retain their recorded DOL
hashes and source revision IDs. Use the [commit map](history-rewrite-map.csv)
to find the corresponding rewritten source revision.

The substitutions use the variables described in
[paths in saved evidence](dolphin-runs/path-placeholders.md). The example
capture script now selects the LLVM symbol reader with `LLVM_NM` or finds
`llvm-nm` on `PATH`, instead of requiring one machine's installation directory.
Actual machine paths belong in the untracked local guide.

Validation covered all reachable Git objects, including commit content and
NUL-containing logs. Git object checks passed, the changed capture script
passed Python syntax compilation, and all 221 tree and metadata comparisons
passed. Wii guest filesystem paths, relative repository dependency paths,
and agent task IDs were preserved because they are not host filesystem paths.

The branch was replaced using an explicit force-with-lease check against its
previous remote head. Existing clones need to use the rewritten branch;
merging the old branch history can reintroduce the original blobs. Copies and
cached objects outside the rewritten repository refs are not changed by this
operation.
