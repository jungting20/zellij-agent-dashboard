#!/usr/bin/env bash
set -euo pipefail
dashboard_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
dashboard_session="${1:-${ZELLIJ_SESSION_NAME:-}}"
if [[ -z "$dashboard_session" ]]; then
    printf 'Usage: %s SESSION_NAME\n' "$0" >&2
    exit 1
fi
dashboard_state="${ZAD_STATE_DIR:-${XDG_STATE_HOME:-$HOME/.local/state}/zellij-agent-dashboard}"
dashboard_host="$dashboard_root/dist/dashboard-host"
dashboard_wasm="$dashboard_root/dist/agent-dashboard.wasm"
if [[ ! -x "$dashboard_host" || ! -f "$dashboard_wasm" ]]; then
    printf 'Run scripts/build.sh first.\n' >&2
    exit 1
fi
# Zellij CLI configuration uses comma-separated key/value pairs.
if [[ "$dashboard_host$dashboard_state$dashboard_wasm" == *','* || "$dashboard_state" != /* ]]; then
    printf 'Plugin paths must be absolute and cannot contain commas.\n' >&2
    exit 1
fi
dashboard_configuration="host_path=$dashboard_host,state_dir=$dashboard_state"
zellij --session "$dashboard_session" action start-or-reload-plugin \
    "file:$dashboard_wasm" --configuration "mode=collector,$dashboard_configuration" </dev/null
zellij --session "$dashboard_session" plugin --floating \
    --configuration "mode=dashboard,$dashboard_configuration" -- "file:$dashboard_wasm" </dev/null
