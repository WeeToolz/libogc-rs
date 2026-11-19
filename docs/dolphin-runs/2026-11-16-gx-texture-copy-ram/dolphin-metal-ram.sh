#!/bin/sh
set -eu
: "${DOLPHIN_METAL_BIN:?Set DOLPHIN_METAL_BIN to the Dolphin executable}"
exec "$DOLPHIN_METAL_BIN" "$@" -C Dolphin.Core.GFXBackend=Metal -C Logger.Logs.Video=True -C Graphics.Hacks.EFBToTextureEnable=False
