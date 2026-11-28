# gc-wad parity review

## Scope

The reference is HBC Reborn's `pywii/Common/pywii/wii.py`, `wadpack.py`, and `retitle.py`. These are Python reference tools in the C project. The production port is the host Rust crate `crates/tools/gc-wad`. It does not link Python or C. It prepares channel WAD files. It does not install a title or write physical NAND.

The behavior ledger is `config/wad-parity.csv`. Run `cargo xtask wad-parity-refresh` after a ledger change and `cargo xtask wad-parity` to check references and report freshness. Implementation, source review, and verification remain separate.

The current format scope is standard `Is` application channel WADs with title IDs in 00010001. Boot2 and system titles need a separate firmware preparation and recovery contract. They are rejected. Disc, archive, and save formats in the reference module are outside this WAD contract.

## Source comparison

The port preserves header fields, section order, 64-byte section alignment, the CRL, footer, ticket title-key encryption, content-index IVs, AES-CBC encryption, content order, SHA-1 records, title version, and boot index. Retitle decrypts the title key with the old title ID IV and encrypts the same key with the new title ID IV. It checks the prepared serialized WAD by decrypting every content before it returns success.

Legacy fakesigning requires an explicit policy. It uses a zero RSA signature and searches the reference's unused 16-bit field for a SHA-1 body digest that starts with zero. It reports `LegacyFakesigned`. This is not a valid RSA signature. The final HBC debug WAD is byte-for-byte equal to Reborn output for identical inputs. The comparison script uses the actual reference packer with CommonCrypto AES as the missing PyCryptodome backend.

The real-mode channel vector is in `gc-elf`. It is native PowerPC assembly. See `libgc-elf.md` for its port and physical verification limit.

## Deliberate safety differences

The reference RSA checker checks the final digest bytes after modular exponentiation. Rust checks the complete PKCS#1 v1.5 SHA-1 encoding and rejects a signature outside the modulus. This prevents malformed signatures from being reported as valid RSA. An independent OpenSSL signature fixture passes, and changed signatures and bodies fail.

The reference file packer replaces its destination. Rust publishes a complete, synced, read-back file with a hard link that refuses to replace an existing path. It checks cleanup errors. Tests prove that existing output and existing partial files survive failures. An interrupted unpack can leave a partial new directory; the command reports failure. It never overwrites an existing directory.

Rust checks input bounds, duplicate content IDs and indices, boot index presence, title agreement, content hashes, AES padding, certificate structure, and unsupported key types. Invalid input returns an error. Keys come from a caller-supplied file and are not printed.

## Verification

Host tests cover NIST AES-CBC and independent SHA-1 vectors, WAD serialization and reload, title-key preservation, malformed metadata, wrong keys, content corruption, truncation, RSA signatures, channel DOL ranges, and publication failures. The HBC artifact includes full ELF debug information and the `debugger-network` feature. Dolphin and physical Wii results are recorded with HBC artifact evidence. Emulator startup does not prove Priiloader return or IOS behavior on a Wii.

## Open capabilities

- Private-key signing and the reference DPKI workflow are not implemented. The API supports preservation of valid RSA metadata or explicitly requested legacy fakesigning. Add a real private-key signing policy and verify its output before claiming that scope complete.
- EC metadata verification is not implemented. It reports `Unsupported`; packing rejects EC metadata. The reference EC signing path is also unimplemented.
- Certificate root trust and certificate-chain signatures are not checked. `ValidRsa` means the metadata signature matches the supplied certificate. It does not mean the supplied certificate is trusted. Add caller-supplied trust anchors and chain validation.
- Physical channel boot, Priiloader-to-stub dispatch, and the physical return failure remain unverified. Do not treat the prepared WAD as a confirmed hardware fix.


## 2026-11-28 follow-up

The host tests do not exercise the Reborn differential harness. The reference
checkout is available at `../../C-sources/hbc-reborn`, commit
`90025e8fe503795f5246392e044e010527879cc6`; its `wii.py`, `wadpack.py`, and
`retitle.py` files have no local changes. The checkout has unrelated changes
outside these files. The inputs required by `tests/reference_reborn.py` (a WAD
metadata template, content files, and the common key) are still absent from
the gc-wad test fixtures. The RSA test fixtures are OpenSSL verification
fixtures, not signing inputs for the Reborn packer. Do not promote WAD
byte-for-byte or private-key signing coverage from the host tests.
`cargo test -p gc-wad` passed all eight tests on 2026-11-28, and
`cargo xtask wad-parity` passed; these checks do not run the Python
differential harness. Private-key signing, EC verification, and trust-chain
validation remain open. Keep the package `review_incomplete` until the missing
differential inputs are available and the source review is complete.
