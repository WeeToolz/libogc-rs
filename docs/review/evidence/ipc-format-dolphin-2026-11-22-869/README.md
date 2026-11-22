# IPC formatter Dolphin evidence

The complete `runtime_probe` run passed 869 checks and failed zero checks on
Dolphin 2609a. Its base revision is
`c150545d983c88bbd09aa23489ac8c128b126885`. The [source patch](source.patch)
has SHA-256 `87b2c92adbc2a45cca55e2fad0282c447f0e543c6d20822251e8d04a3ce96602`.
The DOL SHA-256 is
`dedd12e5dae74e10177cc7ecbbcd4ed658da7728395c6298c9ab6b4dc193b58c`.
The runner used `--probe runtime_probe --timeout-seconds 600` and a private
profile. [suite.json](suite.json) records the full arguments and result.

Live IOS checks cover complete AES ciphertext and IV at 32 and 64 bytes with
offsets zero and one. They check adjacent canaries and the async callback.
The AES mixed-input check compares a direct 64-byte plaintext and a staged key
with the typed control after a tail change. ES error checks compare sync and
async completions with a typed control and check recovery. Three linked async
requests check separate userdata, output canaries, callbacks, and ledgers.

Controlled completion checks cover position-dependent output bytes and mixed
scalar and buffer outputs. The async maximum input fixture checks 31 staged
vectors and all 64 workspace events. Real allocator exhaustion covers async
Argv, second Scalar, FormatState, and first staged buffer. Sync and async late
staged-buffer exhaustion checks confirm the earlier allocations, the failed
final allocation, cleanup, and recovery. The re-entry failure fixture uses a
probe-only submission fault. The async ABI fixture checks stack and vector
argument decoding. No prior formatter PASS label from the earlier workspace
exhaustion run is missing from this run.

The first async workspace and heap exhaustion cases share their reservation
and recovery sequence with related cases. Individual recovery after each of
those failures remains to be checked. This Dolphin run does not prove
Broadway cache behavior or IOS DMA coherency. The eight hardware contracts
remain blocked and the two DMA copyback contracts remain partial.

Raw logs and transport bytes are copied without normalization. The original
run directory retains the page captures and the private profile.
