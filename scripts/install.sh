#!/usr/bin/env bash
set -euo pipefail
dashboard_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
dashboard_plugins="$HOME/.config/zellij/plugins"
for dashboard_artifact in agent-dashboard.wasm dashboard-host; do
    if [[ ! -f "$dashboard_root/dist/$dashboard_artifact" ]]; then
        printf 'Run scripts/build.sh first.\n' >&2
        exit 1
    fi
done
mkdir -p "$dashboard_plugins"
for dashboard_artifact in agent-dashboard.wasm dashboard-host; do
    install -m 755 "$dashboard_root/dist/$dashboard_artifact" "$dashboard_plugins/.$dashboard_artifact.new"
    mv -f "$dashboard_plugins/.$dashboard_artifact.new" "$dashboard_plugins/$dashboard_artifact"
done
printf 'Installed dashboard to %s\n' "$dashboard_plugins"
