#!/bin/bash
# Forgia PreToolUse hook: Concept-First gate (V2).
#
# Refuses Edit/Write on a project Rust source file if no
# `mcp__grepai__grepai_search` (or Grep/Glob) call has been recorded since
# the start of the session.
#
# Implements §3 of `.claude/rules/concept-first*.md` (step 2 "map" —
# discipline metric: grepai_stats must rise).
#
# Exit 0 = allowed, Exit 2 = blocked (stderr shown to Claude).
#
# Bypass: FORGIA_CONCEPT_FIRST_BYPASS=1 (urgency / explicit "go without grep").

INPUT=$(cat)

if [ "${FORGIA_CONCEPT_FIRST_BYPASS:-0}" = "1" ]; then
    exit 0
fi

if command -v jq >/dev/null 2>&1; then
    TOOL_NAME=$(echo "$INPUT" | jq -r '.tool_name // empty')
    FILE_PATH=$(echo "$INPUT" | jq -r '.tool_input.file_path // empty')
else
    TOOL_NAME=$(echo "$INPUT" | grep -oE '"tool_name"[[:space:]]*:[[:space:]]*"[^"]*"' | head -1 | sed 's/.*"//;s/"$//')
    FILE_PATH=$(echo "$INPUT" | grep -oE '"file_path"[[:space:]]*:[[:space:]]*"[^"]*"' | head -1 | sed 's/.*"//;s/"$//')
fi

case "$TOOL_NAME" in
    Edit|Write) ;;
    *) exit 0 ;;
esac

# V2 layout: any project Rust source under */src/* (covers crates/forgia-*/src/**
# AND the root binary src/main.rs). Non-source files (.md/.toml/.json) are free.
case "$FILE_PATH" in
    */src/*.rs|*\\src\\*.rs)
        ;;
    *)
        exit 0
        ;;
esac

# Session state file (shared with concept-first-track / -session-reset)
STATE_DIR="${COCKPIT_STATE_DIR:-$HOME/.claude/state}"
STATE_FILE="$STATE_DIR/concept-first-session.txt"

if [ ! -s "$STATE_FILE" ]; then
    cat >&2 <<MSG_END
CONCEPT-FIRST GATE — Edit BLOCKED

You are about to edit a project Rust file but no mapping has been done
in this session.

Protocol .claude/rules/concept-first.md §3 step 2 (mandatory):
  1. Identify the concept (water/combat/terrain/...)
  2. Run mcp__grepai__grepai_search on the concept word
  3. (Or classic Grep / Glob if grepai unavailable)
  4. Read 2-4 key files
  5. Verbalize producer/consumers/sensor BEFORE this Edit

Target file: ${FILE_PATH}

Bypass (urgency only): export FORGIA_CONCEPT_FIRST_BYPASS=1
MSG_END
    exit 2
fi

# State file exists -> at least 1 mapping recorded -> OK
exit 0