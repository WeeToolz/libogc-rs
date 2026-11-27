# Linux socket options — Dolphin 2609b NoGUI

The offline Dolphin 2609b build at source revision
`3a5b3743b18da3e8f04cbc44f20dc50fbd3a9ef3` completed with the existing
[Linux network patch](../../../scripts/patches/dolphin-linux-network-options.patch)
and a separate [HID initialization guard](../../../scripts/patches/dolphin-linux-headless-hidapi.patch).
The guard reports failure to initialize physical HID scanning instead of
aborting. The build uses the Linux system shared libcurl and a Release Ninja
configuration with NoGUI and headless support, without Qt, X11, EGL, Vulkan,
SDL, ALSA, PulseAudio or cubeb. This build is qualified for the network
probe only, using the Null video backend. GDB reads of the guest framebuffer
remain required. This is not evidence of graphics rendering.

The initial patched run passed 82 of 84 checks. The receive-buffer and
receive-timeout checks expected exact equality with the requested values.
An independent native Linux TCP socket bound to an ephemeral local port
returned 32768 for a requested SO_RCVBUF of 16384, and (2, 28000) for a
requested SO_RCVTIMEO of (2, 25000). These host-specific results are not a
reason to change the production Rust socket implementation. The C reference
`network_wii.c` has a stub `net_getsockopt` returning -EINVAL; working
readback is an added Rust feature.

A Dolphin-automation-only build can opt in to an independently measured
receive buffer and timeout. It requires both expectations, still requires
successful set/get operations and exact output lengths, and checks exact
readback values. The original exact requested-value checks remain the
default. The following Bash commands measure the native values before
building the selected probe; do not reuse a calibrated DOL on another host
without measuring that host's expectations again.

```sh
read -r LIBOGC_DOLPHIN_EXPECT_RCVBUF LIBOGC_DOLPHIN_EXPECT_RCVTIMEO_USEC < <(
python3 - <<'PY'
import socket, struct, sys
assert sys.platform.startswith('linux')
with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as s:
    s.bind(('0.0.0.0', 0))
    s.setsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF, 16384)
    size = s.getsockopt(socket.SOL_SOCKET, socket.SO_RCVBUF)
    s.setsockopt(socket.SOL_SOCKET, socket.SO_RCVTIMEO, struct.pack('@ll', 2, 25000))
    sec, usec = struct.unpack('@ll', s.getsockopt(socket.SOL_SOCKET, socket.SO_RCVTIMEO, struct.calcsize('@ll')))
    assert sec == 2
    print(size, usec)
PY
)
export LIBOGC_DOLPHIN_EXPECT_RCVBUF LIBOGC_DOLPHIN_EXPECT_RCVTIMEO_USEC
cargo xtask dolphin-validate --probe network_probe --dolphin /path/to/dolphin-emu-nogui --nand-dir /path/to/test-nand --artifact-dir /path/to/new-results
unset LIBOGC_DOLPHIN_EXPECT_RCVBUF LIBOGC_DOLPHIN_EXPECT_RCVTIMEO_USEC
```

The same calibrated DOL passed 84/84 with the patched NoGUI executable
(seven valid guest framebuffer captures). Stock Linux Dolphin with its
OpenGL frontend passed 79/84 (eight valid captures). The five failing stock
checks were SO_REUSEADDR, IP_TTL, TCP_NODELAY, TCP_KEEPALIVE and SO_RCVTIMEO.
Both runs used the same test NAND. See [the result record](summary.json),
[initial patched checks](default-checks.log), [stock checks](stock-checks.log)
and [calibrated patched checks](patched-checks.log). The different frontends
are a control limitation; the result does not isolate each emulator patch
hunk or establish native Wii socket-option semantics. The full runtime suite
has not been rerun with a patched graphical frontend. Raw reports and
captures are archived privately without the NAND profile.

Host xtask tests passed 164/164, including the NoGUI argument test. No Rust
production network code changed. The coverage registry now names
`network_probe` for the network source probe; this is not full API coverage.
