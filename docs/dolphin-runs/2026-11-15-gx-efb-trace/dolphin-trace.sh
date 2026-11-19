#!/bin/sh
# Add EFB memory and video logs to the existing probe runner.
set -eu
: "${DOLPHIN_TRACE_BIN:?Set DOLPHIN_TRACE_BIN to the Dolphin executable}"
exec "$DOLPHIN_TRACE_BIN" "$@" -C Logger.Options.Verbosity=5 -C Logger.Logs.MI=True -C Logger.Logs.Video=True -C Logger.Logs.PE=True
