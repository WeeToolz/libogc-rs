# ISO9660 file boundary checks

The existing Dolphin runner built `runtime_probe` from base revision `7b4c3b1` with the probe changes in the worktree. Dolphin 2609a ran the DOL with a private NAND profile. The external MP3 fixture matched `docs/dependencies.md`. The DOL SHA-256 was `5b13497ae3f8767061a2b4d088f6d238692f9ac2cd9baead30cf440f45ea374b`.

The probe mounted its controlled in-memory ISO9660 image through the C export and the Rust filesystem wrapper. For each mount, it read a file through EOF, checked that an EOF read returned zero without changing the output buffer, checked that a write returned `-ENOTSUP`, and checked that a missing file returned `-ENOENT`. It also checked file closure and unmount cleanup. The runner reported 105 passes and no failures; it recorded no process exit code. The check and summary captures were inspected and are readable.

The image is served by a probe callback. This verifies the ISO9660 file layer in Dolphin; it does not verify DVD transport, a physical disc, or partition operations. The same run used a private NAND copy for ISFS checks and reported `ISFS_EINVAL` for file rename.

To repeat, set `LIBOGC_MAD_PROBE_MP3` and `LIBOGC_MAD_REFERENCE_MP3` to the verified external fixtures, then run:

```sh
cargo xtask dolphin-validate --probe runtime_probe \
  --dolphin <Dolphin executable> --artifact-dir <temporary report directory>
```

See the [probe log](probe.log), [ISO checks](checks.png), and [summary](summary.png).
