#!/usr/bin/env bash
# Git pre-commit hook — adds the APSOS copyright header to every staged
# source file that doesn't already contain it.

set -euo pipefail

SCRIPT="$(git rev-parse --show-toplevel)/scripts/add_license_header.sh"

if [[ ! -x "$SCRIPT" ]]; then
    echo "pre-commit: add_license_header.sh not found or not executable, skipping."
    exit 0
fi

# Collect staged files (added or modified, not deleted).
mapfile -t STAGED < <(git diff --cached --name-only --diff-filter=ACM)

if [[ ${#STAGED[@]} -eq 0 ]]; then
    exit 0
fi

MODIFIED=()
for file in "${STAGED[@]}"; do
    before=$(md5 -q "$file" 2>/dev/null || md5sum "$file" 2>/dev/null | awk '{print $1}')
    "$SCRIPT" "$file"
    after=$(md5 -q "$file" 2>/dev/null || md5sum "$file" 2>/dev/null | awk '{print $1}')
    if [[ "$before" != "$after" ]]; then
        MODIFIED+=("$file")
    fi
done

# Re-stage any files that had a header added.
if [[ ${#MODIFIED[@]} -gt 0 ]]; then
    git add "${MODIFIED[@]}"
fi
