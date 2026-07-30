#!/bin/sh
# Architectural invariant: the working contract is actually loaded.
#
# CLAUDE.md is the file an agent reads without being asked. AGENTS.md
# is where the contract lives. Claude Code follows `@path` imports out
# of CLAUDE.md but not English sentences, so a CLAUDE.md that says
# "see AGENTS.md" in prose leaves every rule in it — the TDD
# discipline, the staging rules, the PR conventions — outside the
# context of any agent that does not happen to open the file by hand.
#
# That is a silent failure. Nothing errors; the rules are simply not
# there, and the work proceeds on whatever the agent remembers. So the
# import is asserted rather than trusted, because the symptom of it
# being wrong is indistinguishable from an agent being careless.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

# Overridable so the self-test can drive fixtures instead of mutating
# the real file.
CLAUDE_FILE="${1:-$REPO_ROOT/CLAUDE.md}"

failed=0

if [ ! -f "$CLAUDE_FILE" ]; then
    printf 'arch/agents_contract: %s is missing\n' "$CLAUDE_FILE"
    exit 1
fi

# An import is `@path` alone on a line. A backticked or indented
# mention is documentation about the syntax, not an instance of it.
# Fenced regions are dropped first. Claude Code does not evaluate
# `@path` inside a code block, so a line that only appears in an
# example is documentation about the syntax rather than a use of it —
# and matching it anyway would let someone demote the live import to a
# sample and leave this check reporting the contract as loaded.
if awk '
    /^[[:space:]]*(```|~~~)/ { fenced = !fenced; next }
    !fenced
' "$CLAUDE_FILE" | grep -qE '^@AGENTS\.md[[:space:]]*$'; then
    printf 'arch/agents_contract: ok (CLAUDE.md imports AGENTS.md)\n'
else
    printf 'arch/agents_contract: CLAUDE.md does not import AGENTS.md — a prose pointer is not followed, so the contract never reaches the agent\n'
    failed=1
fi

if [ ! -f "$REPO_ROOT/AGENTS.md" ]; then
    printf 'arch/agents_contract: AGENTS.md is missing, so the import resolves to nothing\n'
    failed=1
fi

[ "$failed" -eq 0 ] || { printf 'arch/agents_contract: FAILED\n'; exit 1; }
