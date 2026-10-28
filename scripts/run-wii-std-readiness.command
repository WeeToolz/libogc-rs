#!/bin/zsh
# Agent launch: use AppleScript to open this file with Terminal via /usr/bin/open.
# Uses the built package's actual arguments. No build or device write is done here.
set -eu
# File tests can have pending writes. Keep automatic reset disabled.
export WII_PROBE_RECOVERY_RESET_MS=0
export WII_PROBE_RESTART_AFTER=0
export WII_PROBE_TIMEOUT="${WII_PROBE_TIMEOUT:-180}"
script_dir="${0:A:h}"
exec python3 - "$script_dir/.." <<'PYTHON'
import hashlib
import json
import os
from pathlib import Path
import subprocess
import sys
from datetime import datetime, timezone
import xml.etree.ElementTree as ET

root = Path(sys.argv[1]).resolve()
package = root / "target/xtask/apps/wii-std-readiness-probe"
dol = package / "boot.dol"
manifest = json.loads((package / "build.manifest.json").read_text())
if hashlib.sha256(dol.read_bytes()).hexdigest() != manifest.get("dol_sha256"):
    raise SystemExit("DOL does not match the packaged build manifest")
arguments = [item.text for item in ET.parse(package / "meta.xml").findall("arguments/arg")]
if any(not argument for argument in arguments):
    raise SystemExit("Invalid empty loader argument in meta.xml")
launcher = root.parents[1] / "run-wii-probe.command"
host_ip = os.environ.get("HOST_IP", "10.0.0.238")
peer_log_path = root / "target/wii-runs" / (
    "network-peer-" + datetime.now(timezone.utc).strftime("%Y%m%d-%H%M%S") + ".log"
)
peer_log_path.parent.mkdir(parents=True, exist_ok=True)
peer_log = peer_log_path.open("w", encoding="utf-8")
peer = subprocess.Popen(
    ["/usr/bin/python3", str(root / "scripts/wii-std-network-peer.py"), "--host-ip", host_ip],
    stdout=subprocess.PIPE,
    stderr=peer_log,
    text=True,
)
try:
    print(f"Network peer diagnostics: {peer_log_path}", flush=True)
    settings = json.loads(peer.stdout.readline())
    arguments.append(
        f"--network-peer={host_ip}:{settings['tcp_port']}:{settings['udp_port']}"
    )
    raise SystemExit(subprocess.call([str(launcher), str(dol), *arguments]))
finally:
    peer.terminate()
    try:
        peer.wait(timeout=5)
    except subprocess.TimeoutExpired:
        peer.kill()
        peer.wait()
    peer_log.close()
PYTHON
