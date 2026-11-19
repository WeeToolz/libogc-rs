# ES certificate type regression

The C `ES_NextCert` checks the certificate type and returns null when the
certificate size is zero. The first Dolphin run used a readable RSA-2048
signature and certificate header with an unknown certificate type. It failed
because the Rust function returned the signature payload address. Its artifacts
are in `../2026-11-19-es-invalid-cert-before/`.

After the fix, Dolphin passed the same check, the supported layout matrix, and
checks for an invalid signature and a null input. The test uses synthetic
memory; it does not authenticate certificates or call IOS.
