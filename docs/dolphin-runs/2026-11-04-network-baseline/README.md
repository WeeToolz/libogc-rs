# Network probe baseline

Dolphin 2609a ran `network_probe` with the existing runner and a private NAND
copy. The [explicit-protocol log](explicit-protocol.log) records 14 passes and
six failures. TCP and UDP socket creation each returned `-68`. Dolphin's IOS
socket implementation rejects nonzero protocols. The probe now also uses
protocol `0` for the main socket checks, as the HBC Wiiload listener does, and
records a separate explicit TCP attempt. The [default-protocol log](default-protocol.log)
records 41 passes and nine failures. Local IP addresses are redacted from the
saved logs. The raw logs and suite reports remain in the local run directories.

The second run passed network initialization, DHCP configuration, socket open
and close, bind, local address, poll, cleanup and reinitialization, and two
wireless scan calls. It did not test traffic with a peer. Its explicit TCP
socket still returned `-68`; that path is not counted as a success.

The remaining failures are six socket-option round trips, a short-address
connect error assertion, and two address-conflict mappings. The current
`lwip-sys` Wii `get_socket_option` implementation returns `-EINVAL`, so the
round-trip checks cannot pass as written. The public `connect` wrapper maps
negative errors to `-1`, while the probe expects `-22`. The bind conflict
returned `-3` from IOS; the probe expects `-98`. The C libogc source converts
IOS socket errors, and the installed PowerPC newlib header defines
`EADDRINUSE` as 112. These observations need separate API and error-domain
review before changing the SDK or assertions.

The HBC checkout uses this workspace's `ogc-network`; its Wiiload listener uses
protocol `0`. The user's successful Wii credential and upload experience is
useful integration evidence, but no specific build identity or per-API result
was captured for these runs. It does not prove each API listed here.

Reproduce with `cargo xtask build-dolphin-probes`, then `cargo xtask
dolphin-validate --no-build --probe network_probe --dolphin /path/to/Dolphin
--artifact-dir /path/to/new-run`. The diagnostic and default-protocol DOL
SHA-256 values were `f455080a68988ef5ab8473c66df81189b23ce1105b3d16bb5a916ee894a31d8e`
and `9b8034512843076a1e361bdb6b50b851aea5ce49f0f8f79bee637753106f431b`.
The runs do not establish physical Wii behavior or credential persistence.
