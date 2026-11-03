# ISFS reopen and FAT remount checks

The existing Dolphin runner built the probes from base revision `6c76f85` with the ISFS and FAT probe changes in the worktree. It ran Dolphin 2609a with private NAND and virtual SD profiles. The external MP3 fixture matched the hash in `docs/dependencies.md`.

| Probe | DOL SHA-256 | Result |
| --- | --- | --- |
| `runtime_probe` | `08392a1488a7a9d7693a96992aa1fff68325cc7c84fce568237c46a6ca980838` | 99 passed, 0 failed |
| `fat_sd_probe` | `a3c785f5d394178cc775e156808ecba239a0bad3391382f540e479bf015fe0d3` | 41 passed, 0 failed |

The ISFS probe closed and reopened files after synchronous and asynchronous writes, checked the bytes, closed the reopened handles, and removed the test files. The FAT probe closed its truncated file, unmounted and remounted the virtual SD volume, checked the file length and bytes, closed it, and removed it. Both logs report the new checks as passing.

The runner reported both probes as passing and recorded no process exit code for either. Its page validation reported success, but the `runtime_probe` summary capture was black on inspection. The readable ISFS check pages and the full log are retained here; the log reports the 99-check summary. The FAT remount and summary pages were also inspected and retained.

Dolphin returned `ISFS_EINVAL` for file rename; the test checked the resulting file state and cleanup. The ISFS format checks used the runner's private NAND copy. No SD Gecko adapter was present. These results do not verify physical hardware.

To repeat, set `LIBOGC_MAD_PROBE_MP3` and `LIBOGC_MAD_REFERENCE_MP3` to the verified external fixtures, then run:

```sh
cargo xtask dolphin-validate --probe runtime_probe --probe fat_sd_probe \
  --dolphin <Dolphin executable> --artifact-dir <temporary report directory>
```

- ISFS [log](runtime_probe/probe.log), [sync check](runtime_probe/sync-check.png), and [async check](runtime_probe/async-check.png)
- FAT [log](fat_sd_probe/probe.log), [remount check](fat_sd_probe/remount-check.png), and [summary](fat_sd_probe/summary.png)
