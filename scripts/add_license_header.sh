#!/usr/bin/env bash
# add_license_header.sh — Prepends the APSOS copyright header to a source file
# if it doesn't already contain it. Supports .rs, .toml, .sh, .js, .ts, .css files.
# Usage: add_license_header.sh <file> [<file> ...]

set -euo pipefail

HEADER_LINE="Copyright (c) 2026 APSOS — App and Software Solutions Wörner. All rights reserved."

add_header_rs() {
    local file="$1"
    if grep -qF "APSOS" "$file" 2>/dev/null; then
        return 0
    fi
    local tmp
    tmp=$(mktemp)
    printf '// %s\n\n' "$HEADER_LINE" | cat - "$file" > "$tmp"
    mv "$tmp" "$file"
    echo "  + header added: $file"
}

add_header_hash() {
    local file="$1"
    if grep -qF "APSOS" "$file" 2>/dev/null; then
        return 0
    fi
    local tmp
    tmp=$(mktemp)
    printf '# %s\n\n' "$HEADER_LINE" | cat - "$file" > "$tmp"
    mv "$tmp" "$file"
    echo "  + header added: $file"
}

for file in "$@"; do
    [[ -f "$file" ]] || continue
    case "$file" in
        *.rs)           add_header_rs   "$file" ;;
        *.toml|*.sh|*.py|*.yml|*.yaml) add_header_hash "$file" ;;
        *)              ;;  # skip unknown types
    esac
done
