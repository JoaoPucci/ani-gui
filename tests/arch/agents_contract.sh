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
AGENTS_FILE="${2:-AGENTS.md}"

failed=0

if [ ! -f "$CLAUDE_FILE" ]; then
    printf 'arch/agents_contract: %s is missing\n' "$CLAUDE_FILE"
    exit 1
fi

# An import is `@path` alone on a line. A backticked or indented
# mention is documentation about the syntax, not an instance of it.
# Two conditions, and the first exists to make the second exact.
#
# CLAUDE.md may not contain a fenced block. It is a pointer file: a
# paragraph saying where the contract lives and the import that brings
# it in. Nothing in that needs a code fence, and allowing one costs
# more than it is worth — Claude Code does not evaluate `@path` inside
# a fence, so the checker would have to decide which lines are live,
# and deciding that correctly means implementing Markdown. Three
# review rounds went into successive versions of that: closing on the
# wrong delimiter, closing on a shorter run, closing on a run with an
# info string or one indented into a code block. Each was a real rule
# and each miss failed silently, accepting an inert import.
#
# Refusing fences outright removes the question. With no fenced region
# in the file, every line is live, and a plain match for the import is
# exact rather than a guess. If a fence is ever genuinely wanted here,
# this fails loudly and someone revisits the trade deliberately.
if grep -qE '^[[:space:]]*(```|~~~)' "$CLAUDE_FILE"; then
    printf 'arch/agents_contract: %s contains a fenced block — this file may not, because Claude Code does not evaluate `@path` inside one and the import must be unambiguously live\n' "$CLAUDE_FILE"
    failed=1
fi

if grep -qE '^@AGENTS\.md[[:space:]]*$' "$CLAUDE_FILE"; then
    printf 'arch/agents_contract: ok (CLAUDE.md imports AGENTS.md)\n'
else
    printf 'arch/agents_contract: CLAUDE.md does not import AGENTS.md — a prose pointer is not followed, so the contract never reaches the agent\n'
    failed=1
fi

# Tracked, not merely present. `git rm --cached` leaves the file on
# disk while taking it out of the index, and a symlink to something
# outside the repository passes a file test — both leave a fresh clone
# importing a contract it does not have, while the reviewer's checkout
# looks fine.
#
# The record check already answers this question, including the
# regular-blob requirement and the literal pathspec, so it is borrowed
# rather than restated: one definition of "arrives with the clone",
# hardened once.
__DEFERRAL_RECORD_LIB__=1
# shellcheck source=./deferral_record.sh
. "$REPO_ROOT/tests/arch/deferral_record.sh"
unset __DEFERRAL_RECORD_LIB__

if ! record_is_recoverable "$AGENTS_FILE"; then
    printf 'arch/agents_contract: %s %s — a clone would import a contract it does not have\n' "$AGENTS_FILE" "$(why_unrecoverable "$AGENTS_FILE")"
    failed=1
fi

[ "$failed" -eq 0 ] || { printf 'arch/agents_contract: FAILED\n'; exit 1; }
