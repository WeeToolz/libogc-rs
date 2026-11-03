# ISFS async rename result and cleanup

Dolphin 2609a passed `runtime_probe` with 83 checks and 0 failures. The DOL
SHA-256 is
`2ab9698af3ac01917c2860c825032a019b525c1a9aa033780d04397572b090f4`.

The probe created a test file and called `rename_async` with a valid source and
destination in Dolphin's private NAND copy. The callback completed with
`ISFS_EINVAL` (`-101`). The source stayed present and the destination stayed
absent. The probe then deleted the source and confirmed that both paths were
absent.

This records Dolphin's async rename error and checks its state and cleanup. It
does not verify successful rename behavior or physical Wii NAND behavior. The
full output is in `suite.json` and `runtime_probe/`.
