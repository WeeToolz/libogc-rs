# IOS58 missing-ticket failed-launch probe retry

Date: 2026-11-27. This attempt prepared the existing `ios_reload_probe` for a
private NAND fixture with the IOS58 ticket removed. It did not run the DOL.

## Fixture

The fixture is `target/nand-ios58-no-ticket`, made as a copy-on-write copy of
the installed Dolphin NAND. The source NAND still has
`ticket/00000001/0000003a.tik`; its SHA-256 is
`c39f5787e63891eedef3d43c3ff1466e150669d55cf57e6a7d2ddd0788ea943a`.
That ticket is absent from the fixture. The IOS58 `title.tmd` and
`00000018.app` in the fixture match the source byte-for-byte. Their SHA-256
values are `010ade26a0d961f92039feeeb88cb715fa9ff69ea9c9569aaad271a512c4ceb7`
and `8a96a90404d390f97a3fa44248a60e74a7ae655237ce5173d5b4e83013f77421`.

This establishes only that the fixture omits the installed IOS58 ticket while
preserving the IOS58 TMD and content. The probe checks the active IOS first.
This is required because the missing ticket is for the active IOS58 title. It
does not prove that Dolphin ES returns an error for `ES_GetNumTicketViews`, or
that `reload_ios` takes its failed launch path.

## Build and runner result

The probe DOL was built with:

```sh
cargo xtask build-dol ogc-system-probe --bin ios_reload_probe \
  --probe-features ios-reload-probe,dolphin-automation \
  --output target/ios_reload_probe-missing-ticket.dol
```

Its SHA-256 is
`7fd08dcf38e17c2ced0c6bcce9565116c8109a85a9f58ec5075f46f08f0eb973`.

The runner command was:

```sh
cargo xtask dolphin-validate --probe ios_reload_probe \
  --dolphin /Applications/Dolphin.app/Contents/MacOS/Dolphin \
  --nand-dir target/nand-ios58-no-ticket \
  --artifact-dir docs/dolphin-runs/2026-11-27-ios58-missing-ticket-retry
```

The runner stopped before booting the DOL. Dolphin exited on its `--version`
check with `SIGABRT` and printed:
`Incompatible processor. This Qt build requires the following features: neon`.
The probe result is therefore **not run**. Failed IOS launch handling remains
open until the same probe runs in a working Dolphin build or on a qualified
physical Wii test setup.
