# WAD behavior parity

Generated from `config/wad-parity.csv` with `cargo xtask wad-parity-refresh`.

Implementation, source review, and verification are separate states. Host and emulator checks do not prove physical Wii behavior. See [scope and open capabilities](../review/libgc-wad.md).

| ID | Behavior | Implementation | Source review | Verification | Follow-up |
| --- | --- | --- | --- | --- | --- |
| WAD-001 | Channel header sections and footer | implemented | source_reviewed | reference_pass | none |
| WAD-002 | Decrypt and check all content hashes | implemented | source_reviewed | host_pass | none |
| WAD-003 | TMD records versions and boot index | implemented | source_reviewed | host_pass | none |
| WAD-004 | Ticket title key and title ID IV | implemented | source_reviewed | host_pass | none |
| WAD-005 | AES CBC content index IV and padding | implemented | source_reviewed | reference_pass | none |
| WAD-006 | Explicit legacy fakesigning | implemented | source_reviewed | reference_pass | none |
| WAD-007 | Complete RSA signature check | implemented | source_reviewed | host_pass | Require complete PKCS1 encoding instead of reference digest suffix check |
| WAD-008 | Channel real mode entry vector | implemented | source_reviewed | hardware_pending | Verify channel boot on Wii with the required recovery procedure |
| WAD-009 | Verified atomic WAD file publication | implemented | source_reviewed | host_pass | Refuse to overwrite outputs and preserve caller work on failure |
| WAD-010 | Private key signing and DPKI | partial | source_reviewed | open | Add explicit private key signing and verify the result against supplied certificates |
| WAD-011 | EC metadata signature verification | partial | source_reviewed | open | Implement Wii EC verification before accepting EC signed metadata for packing |
| WAD-012 | Certificate root trust | partial | source_reviewed | open | Add caller supplied trusted root validation and chain checks |
| WAD-013 | Boot2 and system title WADs | out_of_scope | source_reviewed | out_of_scope | Requires a separate firmware preparation and recovery contract |

Source, Rust, and evidence references for each contract are in the CSV ledger.
