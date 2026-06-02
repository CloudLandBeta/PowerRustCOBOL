#!/usr/bin/env bash
# SPDX-License-Identifier: Apache-2.0
# Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
#
# Licensed under the Apache License, Version 2.0.
# See the LICENSE file in the project root for full license information.

# check_bugs.sh — PowerRustCOBOL automated bug scanner
#
# Usage:
#   ./tools/check_bugs.sh          # scan and update BUGS.md
#   ./tools/check_bugs.sh --quiet  # suppress stdout, only update file
#
# What it does:
#   1. Runs `cargo check --workspace` and captures compiler output.
#   2. Parses every "error[E…]" line into structured records.
#   3. Merges new bugs into BUGS.md (skips duplicates by crate+error code+summary).
#   4. Exits with code 0 if no open bugs, 1 if bugs were found.
#
# Dependencies: bash, cargo, awk, sed, date (all standard on macOS / Linux).

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
BUGS_FILE="$PROJECT_ROOT/BUGS.md"
QUIET=false
[[ "${1:-}" == "--quiet" ]] && QUIET=true

log() { $QUIET || echo "$@"; }

# ── 1. Run cargo check ────────────────────────────────────────────────────────
log "🔍 Running cargo check --workspace …"
cd "$PROJECT_ROOT"

# Capture stderr (compiler output); allow non-zero exit so we can read errors.
RAW_OUTPUT="$(cargo check --workspace --message-format short 2>&1 || true)"

# ── 2. Parse errors ───────────────────────────────────────────────────────────
# Lines look like:
#   crates/cobolt-runtime/src/interpreter.rs:1212:64: error[E0599]: no method `integer`
#   error[E0308]: mismatched types

declare -a BUG_CRATES BUG_CODES BUG_SUMMARIES
TODAY="$(date +%Y-%m-%d)"

while IFS= read -r line; do
    # Match lines containing "error[E…]"
    if [[ "$line" =~ error\[([EW][0-9]+)\]:?[[:space:]]*(.*) ]]; then
        CODE="${BASH_REMATCH[1]}"
        SUMMARY="${BASH_REMATCH[2]}"
        # Trim summary to 80 chars
        SUMMARY="${SUMMARY:0:80}"

        # Extract crate from path prefix (e.g. crates/cobolt-runtime/...)
        CRATE="workspace"
        if [[ "$line" =~ crates/([^/]+)/ ]]; then
            CRATE="${BASH_REMATCH[1]}"
        fi

        BUG_CRATES+=("$CRATE")
        BUG_CODES+=("$CODE")
        BUG_SUMMARIES+=("$SUMMARY")
    fi
done <<< "$RAW_OUTPUT"

NUM_BUGS=${#BUG_CRATES[@]}

if [[ $NUM_BUGS -eq 0 ]]; then
    log "✅ No compiler errors found."
    # Ensure BUGS.md open section says all clear
    if grep -q "^| BUG-" "$BUGS_FILE" 2>/dev/null; then
        log "⚠️  BUGS.md still has open entries — mark them resolved manually."
        exit 1
    fi
    exit 0
fi

# ── 3. Read existing open bugs to avoid duplicates ────────────────────────────
EXISTING_OPEN="$(grep "^| BUG-" "$BUGS_FILE" 2>/dev/null || true)"

# Find next BUG ID
LAST_ID="$(grep -oE 'BUG-[0-9]+' "$BUGS_FILE" 2>/dev/null | grep -oE '[0-9]+' | sort -n | tail -1 || echo "0")"
NEXT_ID=$((LAST_ID + 1))

NEW_ROWS=""
NEW_COUNT=0

for i in "${!BUG_CRATES[@]}"; do
    CRATE="${BUG_CRATES[$i]}"
    CODE="${BUG_CODES[$i]}"
    SUMMARY="${BUG_SUMMARIES[$i]}"

    # Skip if already tracked (match on crate + code + first 40 chars of summary)
    SHORT="${SUMMARY:0:40}"
    if echo "$EXISTING_OPEN" | grep -qF "$CODE" && echo "$EXISTING_OPEN" | grep -qF "$SHORT"; then
        continue
    fi

    BUG_ID="$(printf 'BUG-%03d' $NEXT_ID)"
    NEXT_ID=$((NEXT_ID + 1))
    NEW_COUNT=$((NEW_COUNT + 1))

    # Escape pipes in summary
    SAFE_SUMMARY="${SUMMARY//|/∣}"
    NEW_ROWS+="| $BUG_ID | $TODAY | \`$CRATE\` | \`$CODE\` | $SAFE_SUMMARY |"$'\n'
done

# ── 4. Inject new rows into BUGS.md ──────────────────────────────────────────
if [[ $NEW_COUNT -gt 0 ]]; then
    log "📝 Adding $NEW_COUNT new bug(s) to BUGS.md …"

    # Replace the "No open bugs" placeholder line if present
    TMP="$(mktemp)"
    awk -v new_rows="$NEW_ROWS" '
    /^_No open bugs/ {
        # Replace placeholder with the new rows (already have the header above)
        printf "%s", new_rows
        next
    }
    /^\| ID \|/ {
        print
        # If next line is the separator, print it then inject rows after
        next_is_sep=1
        print
        next
    }
    {
        if (next_is_sep && /^\|----/) {
            print
            printf "%s", new_rows
            next_is_sep=0
            next
        }
        print
    }
    ' "$BUGS_FILE" > "$TMP"

    # Simpler fallback: just append before the "---" line after the open table
    # if awk didn't insert (file may vary). Use Python for reliability.
    python3 - "$BUGS_FILE" "$NEW_ROWS" "$EXISTING_OPEN" << 'PYEOF'
import sys, re

bugs_path = sys.argv[1]
new_rows   = sys.argv[2]
existing   = sys.argv[3]

text = open(bugs_path).read()

# Find the open-bugs table block and append new rows before the closing blank/---
# Pattern: header row | separator row | (existing rows) | placeholder or blank
placeholder = "_No open bugs — all clear! ✅_"

if placeholder in text:
    # Replace placeholder with new rows + placeholder (so it re-appears if all fixed)
    text = text.replace(placeholder, new_rows.rstrip() + "\n")
else:
    # Append after last existing open-bug row (before next ---)
    # Find the open table section
    open_section = re.search(
        r'(## Open Bugs\n.*?\n\|[-| ]+\|\n)(.*?)(^---)',
        text, re.DOTALL | re.MULTILINE
    )
    if open_section:
        insertion = open_section.group(1) + open_section.group(2) + new_rows + open_section.group(3)
        text = text[:open_section.start()] + insertion + text[open_section.end():]

open(bugs_path, 'w').write(text)
PYEOF

    log "✅ BUGS.md updated."
else
    log "ℹ️  All $NUM_BUGS error(s) already tracked in BUGS.md."
fi

# ── 5. Summary ────────────────────────────────────────────────────────────────
TOTAL_OPEN="$(grep -c "^| BUG-" "$BUGS_FILE" 2>/dev/null || echo "0")"
log ""
log "┌─────────────────────────────────────────┐"
log "│  PowerRustCOBOL Bug Scanner — Summary   │"
log "├─────────────────────────────────────────┤"
log "│  Compiler errors found : $NUM_BUGS"
log "│  New bugs added        : $NEW_COUNT"
log "│  Total open in BUGS.md : $TOTAL_OPEN"
log "└─────────────────────────────────────────┘"

exit 1
