# IOS IPC runtime parity in Dolphin

Date: 2026-11-27. Dolphin 2609a. The current `runtime_probe` passed **901 checks and 0 failed checks**. The probe DOL SHA-256 is `9771e5380ab5a3bae12474f74ea215046257d3f41db9e20c8e9b191d118bbf87`. The runner used a fresh private NAND copy and removed it after the run.

The read-only file-stat and mixed file-attribute requests used the System Menu `setting.txt` fixture (256 bytes, SHA-256 `9eeb543b7a25494c098c7dce7fa6abb6faa417fed9560b3c6c49e13078ebe22c`). Its `title.tmd` is 808 bytes, SHA-256 `1a5df7b0e72721c9ffae34664b00144eda2aeee0e327332deda84d3cfddb317b`. These hashes identify the source fixture; the probe ran on the runner's private NAND copy.

## Raw IOS ioctl behavior

The run passed exact file-stat output checks for C `IOS_Ioctl`, C `IOS_IoctlAsync`, and the Rust ioctl wrappers, including untouched bytes past the output range. It passed invalid argument rejection for sync and async C ioctl and observed no callback for the rejected async submission. The mixed file-attribute request passed for both an existing file and a missing path: the valid request returned zero, the missing request returned `ISFS_ENOENT`, async reply codes matched, input path bytes stayed unchanged, output tail canaries stayed intact, and callback count and userdata matched.

## Raw IOS ioctlv behavior

The run passed MAC output checks for C `IOS_Ioctlv`, C `IOS_IoctlvAsync`, and safe and raw Rust methods. It passed invalid argument rejection with no callback, restored the C vector pointers on an injected async submission failure, and confirmed vector pointers stayed stable through successful calls. Mixed input/output ES requests passed for title-directory output and an invalid-command error (`-4`); they preserved input title bytes, output canaries, vector pointers, callback count, and userdata.

## Formatter and bridge behavior

The run passed the formatter assertions for ABI register and stack decoding, scalar widths, strings, mixed vectors, output copyback, callback ordering, errors, cleanup, and recovery. A script matched each of the 82 `verified` rows in `config/ipc-format-contracts.csv` to a passing label in the raw probe transport log. The other 8 rows remain blocked on physical Wii cache and DMA evidence. No Dolphin result changes that limit.

The raw probe log, full transport log, Dolphin log, suite report, result report, and one captured page are retained beside this note. The reports retain the execution detail. No modified NAND copy or large transient stderr log is retained.
