#!/usr/bin/env bash
set -euo pipefail
dashboard_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$dashboard_root"
dashboard_rustup="${ZAD_RUSTUP:-$HOME/.cargo/bin/rustup}"
# Use rustup proxies for rustc, rustdoc and cargo-clippy together. Homebrew's
# rustc can otherwise be selected even when cargo itself is a rustup proxy.
env PATH="$HOME/.cargo/bin:$PATH" "$dashboard_rustup" run 1.88.0 cargo build --locked --release -p dashboard-host
env PATH="$HOME/.cargo/bin:$PATH" "$dashboard_rustup" run 1.88.0 cargo build --locked --release -p dashboard-plugin --target wasm32-wasip1
mkdir -p dist
cp target/release/dashboard-host dist/.dashboard-host.new
chmod 755 dist/.dashboard-host.new
mv -f dist/.dashboard-host.new dist/dashboard-host
cp target/wasm32-wasip1/release/dashboard-plugin.wasm dist/.agent-dashboard.wasm.new
mv -f dist/.agent-dashboard.wasm.new dist/agent-dashboard.wasm
printf 'Built %s/dist\n' "$dashboard_root"
