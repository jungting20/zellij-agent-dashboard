#!/usr/bin/env bash
set -euo pipefail
dashboard_root=$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")/.." && pwd)
cd "$dashboard_root"
dashboard_rustup="${ZAD_RUSTUP:-$HOME/.cargo/bin/rustup}"
export PATH="$HOME/.cargo/bin:$PATH"
"$dashboard_rustup" run 1.88.0 cargo fmt --all -- --check
"$dashboard_rustup" run 1.88.0 cargo test --locked --workspace
"$dashboard_rustup" run 1.88.0 cargo clippy --locked --workspace --all-targets -- -D warnings
"$dashboard_rustup" run 1.88.0 cargo clippy --locked -p dashboard-plugin --target wasm32-wasip1 -- -D warnings
bash -n scripts/build.sh scripts/dashboard.sh scripts/check.sh
python3 -c 'import ast, pathlib; ast.parse(pathlib.Path("scripts/smoke.py").read_text())'
