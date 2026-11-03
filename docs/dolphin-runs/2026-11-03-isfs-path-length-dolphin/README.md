# ISFS path length validation

Dolphin 2609a passed `runtime_probe` with 65 checks and 0 failures. The DOL
SHA-256 is
`2cca14b5250d65e270ff39fb95bf8dfd21e102d0f2e8889aa900a43b992c53a5`.

The ISFS path probe supplied a 64-byte path followed by NUL while the ISFS
service was initialized. It checked 10 synchronous and 10 asynchronous calls:
directory read, directory and file creation, attribute get and set, usage
query, delete, rename with the long path in each position, and open. Each call
returned `ISFS_EINVAL`. The probe also checked that count, usage, and attribute
outputs kept their sentinel values and that the async callback did not run.

This verifies local path length rejection in Dolphin before IOS submission. It
does not verify successful NAND operations, valid path behavior, or physical
Wii behavior. The full output is in `suite.json` and `runtime_probe/`.
