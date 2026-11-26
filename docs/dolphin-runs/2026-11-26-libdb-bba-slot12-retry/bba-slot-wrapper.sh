#!/bin/bash
set -eu
: "${DOLPHIN_METAL_BIN:?Set DOLPHIN_METAL_BIN to DolphinQt}"
args=("$@")
for ((i = 0; i < ${#args[@]}; i++)); do
  if [[ "${args[$i]}" == "--exec" ]]; then
    args=("${args[@]:0:i}" -C Dolphin.Core.SlotB=12 "${args[@]:i}")
    break
  fi
done
exec "$DOLPHIN_METAL_BIN" "${args[@]}"
