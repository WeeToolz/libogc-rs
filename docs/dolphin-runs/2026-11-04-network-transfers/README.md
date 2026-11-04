# HBC TCP and SDK UDP transfer checks

The standard and development network DOLs each passed **66 checks, zero
failures**, including 14 new UDP checks. HBC's existing TCP input test and its
extended control mode also passed. The [results](results.json) record the
DOL hashes and runner results; the [standard](standard.log) and
[development](development.log) logs have redacted local IP addresses.

The UDP checks use two SDK sockets bound to distinct loopback ports through
Dolphin's native host networking. They send two independent datagrams, verify
exact binary bytes, packet boundaries, sender addresses and ports, then send
and verify a reply. Polling bounds the wait before each receive, and socket
cleanup results are checked. This is a real loopback transport test, not a
separate host server or a physical wireless test.

HBC's production TCP protocol test sends split writes, checks exact reply
bytes, rejects malformed packets, opens new connections, and verifies
completion in the main loop. Extended control mode checks operation errors,
duplicate-command results, and invalid release packets for every controller
family. The input/control extensions have no C HBC protocol counterpart; no
independent C result is claimed. The existing HBC DOL's build-source revision
was not established, so its result is integration evidence, not proof of the
current SDK TCP implementation. Its exact DOL hash is in the results.

The HBC runner now accepts NAND, Dolphin, and output paths as command-line
arguments, validates that the host address belongs to the Mac, checks that
port 4299 is free, and owns the launched process for cleanup. The test copies
the source NAND to a private profile. Raw profiles and logs remain outside the
repositories. No physical Wii, user drive, or saved credentials were changed.

## Reproduce

Use the patched macOS Dolphin build from the
[network option fix](../2026-11-04-network-fixed/README.md). Build the standard
probe with `cargo xtask build-dolphin-probes`, then run:

```sh
cargo xtask dolphin-validate --no-build --probe network_probe \
  --dolphin "$DOLPHIN_BIN" --artifact-dir "$RUN_DIR"
```

Build the development probe with `--dev --probe-features dolphin-automation`
as described in the probe guide. Set `LIBOGC_DOLPHIN_DEV_DOL` and
`LIBOGC_DOLPHIN_DEV_ELF` to matching files and use a new run directory.
The build needs the external MP3 fixtures described in the dependency guide.

From the HBC repository, run its existing test with a new output directory:

```sh
python3 tools/test_remote_input_dolphin.py --host "$HOST_IPV4" \
  --dol "$HBC_DOL" --nand "$NAND_DIR" --dolphin "$DOLPHIN_BIN" \
  --output "$HBC_RUN_DIR" --control
```

The HBC tools change is recorded as revision `cff30a0` in its repository.
The SDK evidence registry adds the exact `send_to_slice` and `recv_from_slice`
paths exercised by the new probe. Other APIs retain their existing status.
Separate host-peer SDK TCP tests, DNS, credential persistence, SMB, and physical
Wii network coverage remain outside this batch.
