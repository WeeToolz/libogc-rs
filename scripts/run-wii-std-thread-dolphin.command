#!/bin/zsh
set -euo pipefail

repo_dir="${0:A:h:h}"
dolphin_bin="${DOLPHIN_BIN:-/Applications/Dolphin.app/Contents/MacOS/Dolphin}"
build_probe=1

usage() {
  cat <<'EOF'
Run the Wii std thread probe in Dolphin.

Usage: scripts/run-wii-std-thread-dolphin.command [options]
  --no-build       Use the existing probe DOL.
  --dolphin PATH   Use this Dolphin executable (or set DOLPHIN_BIN).
  -h, --help       Show these instructions.

The default action builds the probe first. Each run uses a separate Dolphin
profile and saves logs under target/dolphin-std-thread/. The probe holds its
result on screen. Close the Dolphin window to end the run.

A successful Dolphin exit is not a probe pass. Read the final PASS/FAIL summary.
Dolphin results do not close real-Wii acceptance checks.
EOF
}

while (( $# > 0 )); do
  case "$1" in
    --no-build) build_probe=0; shift ;;
    --dolphin)
      if (( $# < 2 )) || [[ -z "$2" ]]; then
        print -u2 -- "--dolphin requires an executable path."
        exit 2
      fi
      dolphin_bin="$2"
      shift 2
      ;;
    -h|--help) usage; exit 0 ;;
    *) print -u2 -- "Unknown option: $1"; usage >&2; exit 2 ;;
  esac
done

if [[ ! -x "$dolphin_bin" || -d "$dolphin_bin" ]]; then
  print -u2 -- "Dolphin executable not found: $dolphin_bin"
  print -u2 -- "Use --dolphin PATH or set DOLPHIN_BIN."
  exit 1
fi
dolphin_bin="${dolphin_bin:A}"
command -v python3 >/dev/null
cd "$repo_dir"
mkdir -p target/dolphin-std-thread
run_dir="$(mktemp -d "$repo_dir/target/dolphin-std-thread/run-$(date +%Y%m%d-%H%M%S)-XXXXXX")"
user_dir="$run_dir/user"
dol_path="$repo_dir/target/xtask/probes/wii_std_thread_probe.dol"
print -- "Run directory: $run_dir"

if (( build_probe )); then
  cargo xtask build-wii-std-probe 2>&1 | tee "$run_dir/build.log"
fi
if [[ ! -f "$dol_path" ]]; then
  print -u2 -- "Probe DOL not found: $dol_path"
  print -u2 -- "Run this script without --no-build to build the probe."
  exit 1
fi

# Use the shared framebuffer readback settings and a new profile for this run.
python3 - "$repo_dir/scripts" "$user_dir" <<'PY'
from pathlib import Path
import sys

sys.path.insert(0, sys.argv[1])
from dolphin_capture_config import configure_capture_profile

profile = Path(sys.argv[2])
configure_capture_profile(profile)
(profile / "Config" / "Dolphin.ini").write_text(
    "[Core]\nCPUThread = False\nEmulationSpeed = 1.0\nWiiSDCard = False\n"
    "[Interface]\nConfirmStop = False\n"
    "[Analytics]\nPermissionAsked = True\nEnabled = False\n"
    "[DSP]\nBackend = No Audio Output\n"
)
(profile / "Config" / "Logger.ini").write_text(
    "[Options]\nWriteToFile = True\nWriteToConsole = True\nVerbosity = 4\n"
    "[Logs]\nBOOT = True\nCORE = True\nOSREPORT = True\n"
    "IOS = True\nIOS_FS = True\n"
)
PY

{
  print -- "Dolphin executable: $dolphin_bin"
  "$dolphin_bin" --version
  print -- "Source commit:"
  git rev-parse HEAD
  git status --short
  shasum -a 256 "$dol_path"
  print -- "Dolphin user profile: $user_dir"
  print -- "Probe result: read the final on-screen PASS/FAIL summary."
  print -- "This is an emulator run; real-Wii acceptance remains open."
} > "$run_dir/run-info.txt"
cat "$run_dir/run-info.txt"
print -- "Close the Dolphin window after you read the final result."

if "$dolphin_bin" --user "$user_dir" --batch --video_backend=OGL \
    --exec "$dol_path" 2>&1 | tee "$run_dir/dolphin-console.log"; then
  print -- "Dolphin closed. Logs: $run_dir"
else
  run_status=$?
  print -u2 -- "Dolphin or log capture failed (status $run_status). Logs: $run_dir"
  exit "$run_status"
fi
