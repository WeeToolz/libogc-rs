# libdb TCP close states in Dolphin

The final `debugger_probe` build passed in Dolphin df4bd44. It reported 23
checks passed, zero failed, and four valid page captures. The DOL SHA-256 was
`9aba957539d40cb0bed2fd80a304dea93ca9ef058ec3e28939e493ac8b7eed86`.

The standard Dolphin profile has no EXI BBA. The probe checked the missing
adapter path and invalid TCP handles. It did not run a TCP handshake, FIN_WAIT_2,
or TIME_WAIT exchange. Host tests exercise the production close-state transition
and receive-sequence helpers, plus C timeout boundaries. Active BBA packet
behavior remains unverified.

The result is in `suite.json`; the probe transcript and final page are in
`debugger_probe/`. Physical Wii evidence remains pending.
