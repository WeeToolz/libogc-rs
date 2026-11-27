# IPC formatter Dolphin coverage

The `runtime_probe` run completed with 863 passes and zero failures in Dolphin
2609a. Its base revision is `c150545d983c88bbd09aa23489ac8c128b126885`.
The [source patch](source.patch) has SHA-256
`9fe52fbaf7bbf42e8522e7bd72cf793ec8e1e37166147478445c33652d98ca30`.
The DOL SHA-256 is
`d3102140a75c06a5f37aadcc4e269bd9ef560738587b0f50d6908b01c98725cd`.
The runner used `--probe runtime_probe --timeout-seconds 600` with a private
Dolphin profile. [suite.json](suite.json) records the full arguments.

The live IOS checks cover full AES outputs at 32 and 64 bytes, with offsets
zero and one, in sync and async calls. They check the ciphertext, IV, and
adjacent canaries. The async callback checks the output and IV before return.
An unsupported ES command is checked against a typed control, including sync,
async, callback-less cleanup, and recovery. A callback re-entry case checks a
rejected second submission.

Controlled completion checks write a position-dependent output pattern and
check the async callback after formatter copyback. The async 31-vector case
checks staged inputs and all 64 workspace events. Real allocator exhaustion
checks cover async Argv, second Scalar, FormatState, and first staged buffer.
The async mixed ABI fixture checks scalar and buffer argument decoding.

The later staged-buffer OOM case, three-request callback chain, mixed scalar
and buffer output completion, and mixed direct and staged AES input comparison
remain open. The eight physical cache and DMA contracts remain blocked. The
two DMA copyback contracts remain partial. Dolphin does not prove physical
Broadway cache behavior or IOS DMA coherency.

The raw logs and transport bytes in this directory are copied without
normalization. The source run directory retains the page captures and profile.
