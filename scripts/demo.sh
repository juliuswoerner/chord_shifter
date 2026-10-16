#!/usr/bin/env bash
# Start the app locally in demo mode (sample songs and a demo user, no server needed).
#
#   scripts/demo.sh            # dev server on http://localhost:8080
#   scripts/demo.sh --port N   # use another port
#
# Everything is stored in the browser's localStorage. Use the "Reset demo data"
# button on the login screen, or clear site data, to start over.

set -euo pipefail

PORT=8080
while [[ $# -gt 0 ]]; do
    case "$1" in
        --port)
            PORT="${2:?--port needs a value}"
            shift 2
            ;;
        -h|--help)
            sed -n '2,9p' "$0" | sed 's/^# \{0,1\}//'
            exit 0
            ;;
        *)
            echo "Unknown option: $1 (try --help)" >&2
            exit 1
            ;;
    esac
done

cd "$(dirname "$0")/.."

fail=0
if ! command -v cargo >/dev/null 2>&1; then
    echo "Missing: Rust (install from https://rustup.rs/)" >&2
    fail=1
fi
if ! command -v dx >/dev/null 2>&1; then
    echo "Missing: Dioxus CLI (cargo install dioxus-cli --version 0.6.3 --locked)" >&2
    fail=1
fi
if command -v rustup >/dev/null 2>&1 \
    && ! rustup target list --installed | grep -qx wasm32-unknown-unknown; then
    echo "Missing: WASM target (rustup target add wasm32-unknown-unknown)" >&2
    fail=1
fi
[[ $fail -eq 0 ]] || exit 1

cat <<MSG

  Demo URL:  http://localhost:${PORT}/?demo
  Login:     demo@example.com / Demo-Sheetwave-2026!
             (or click "Log in as demo user" on the login screen)

MSG

exec dx serve --platform web --port "${PORT}"
