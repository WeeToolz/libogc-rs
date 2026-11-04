# CARD API audit Dolphin preflight

This check did not run a DOL. It checked whether the configured Dolphin
executable could start in either architecture mode. Both checks aborted before
the probe runner could launch a DOL.

## Commands and results

Native arm64 command:

```sh
arch -arm64 ${DOLPHIN_BIN} --version
```

Exit status: `134`.

```text
Incompatible processor. This Qt build requires the following features:
    neon
```

Translated x86_64 command:

```sh
arch -x86_64 ${DOLPHIN_BIN} --version
```

Exit status: `134`.

```text
PasteBoard: Error creating pasteboard: com.apple.pasteboard.clipboard [-4960]
PasteBoard: Error creating pasteboard: com.apple.pasteboard.find [-4960]
2026-11-03 20:37:36.696 Dolphin[38602:11779067] Error received in message reply handler: Connection invalid
2026-11-03 20:37:36.696 Dolphin[38602:11779162] Connection Invalid error for service com.apple.hiservices-xpcservice.
```

The cause of both start failures is unknown. No DOL ran and this check gives no
new API evidence. The latest saved CARD Dolphin results remain in the
[API audit](2026-11-03-ogc-card-api-audit.md). These preflight observations
were recorded at `2026-11-04 01:43:29 UTC`.
