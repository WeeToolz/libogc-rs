# ISFS asynchronous format diagnostic

Dolphin 2609a ran `runtime_probe` in a private NAND copy. The DOL SHA-256 was
`8c230d5d624103464b8752c0e47e2ae1145cf5740ee896868c69dc85494dcdec`. The
probe recorded 86 checks and 1 failure.

`format_async` returned submission status `0`. The marker file and directory
were absent after the request. The callback did not complete within the probe's
bounded wait, so the probe failed the completion check. Cleanup checks passed.

This run does not verify callback delivery or final IOS result for async format.
It also does not prove behavior on a physical Wii. Keep async format open until
a target that returns the callback result passes.
