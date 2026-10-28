#!/bin/zsh
setopt pipefail

repo_dir="${0:A:h:h}"
cd "$repo_dir" || exit 1
mkdir -p target/wii-runs || exit 1

log_file="target/wii-runs/terminal-bluetooth-$(date +%Y%m%d-%H%M%S).log"
cargo run --release \
  --manifest-path Wiiload-rs/Cargo.toml \
  -- run target/xtask/probes-agent/bluetooth_probe.dol \
  --wii-ip 10.0.0.242 \
  --host-ip 10.0.0.238 \
  --timeout 210 \
  --recovery-reset-ms 180000 \
  --restart-after-probe \
  --output target/wii-runs 2>&1 | tee "$log_file"
run_status=${pipestatus[1]}
exit "$run_status"
