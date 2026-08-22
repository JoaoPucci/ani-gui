#!/bin/sh

# Architectural invariant: the retired `ani-cli` script is not
# reacquired.
#
# The repository no longer vendors the script, and AGENTS.md §4 says
# nothing may start to depend on it. A rule stated in the contract
# with no check is the failure §15 documents — an invariant that
# reads as covered while nothing enforces it — and reacquisition does
# not require the file to reappear in the tree: a packaging manifest
# can stage a fetched copy, a fetcher can download one into the
# bundled-bin directory, and a workflow can install one onto a
# runner. Those are the silent routes; re-vendoring the file itself
# is loud. This check carries the syntactic rules the retired
# `boundaries.sh` held, minus the ones whose subject was the vendored
# copy.
#
# What this does NOT check: a Rust spawn reaching an `ani-cli` binary
# by string, or a fetcher acquiring the script under another name.
# Deciding which string literal reaches a spawn means reading source
# for meaning, which AGENTS.md §2 puts out of bounds here; each rule
# below is something a grep either finds or does not.

set -eu

REPO_ROOT="${ARCH_REPO_ROOT:-$(cd "$(dirname "$0")/../.." && pwd)}"
cd "$REPO_ROOT"

if [ ! -d gui ]; then
    printf 'arch/no_reacquired_script: gui/ does not exist yet — skipping\n'
    exit 0
fi

failed=0

# Build artifacts (target/, node_modules/, bundles) are gitignored but
# may sit on disk and contain stale copies. Skip them.
GREP_EXCLUDE='--exclude-dir=target --exclude-dir=node_modules --exclude-dir=build --exclude-dir=dist --exclude-dir=.svelte-kit'

# 1. No sourcing. The cheapest way to reacquire the dependency is to
# pull the script's functions into a shell the app runs.
# shellcheck disable=SC2086
matches=$(grep -rnE $GREP_EXCLUDE '(^|[[:space:]])((source|\.)[[:space:]]+["'"'"']?[^"'"'"' ]*ani-cli)' gui/ 2>/dev/null || true)
if [ -n "$matches" ]; then
    printf 'arch/no_reacquired_script FAIL: gui/ sources ani-cli (forbidden):\n%s\n' "$matches" >&2
    failed=1
fi

# 2. The `__ANI_CLI_LIB__` source-guard was the seam the script-era
# tests used to source the script as a library. Nothing legitimate
# reads or writes it anywhere now; an occurrence means a layer
# started treating the script as a library again.
# shellcheck disable=SC2086
matches=$(grep -rn $GREP_EXCLUDE '__ANI_CLI_LIB__' gui/ 2>/dev/null || true)
if [ -n "$matches" ]; then
    printf 'arch/no_reacquired_script FAIL: the retired __ANI_CLI_LIB__ seam appears in gui/:\n%s\n' "$matches" >&2
    failed=1
fi

# 3. No packaging manifest may stage the script. This is the route
# that regressed historically: the packages carried the script long
# after anything read it, through exactly two `extraResources` lines.
# The pattern does not need the file in the tree — a manifest can
# stage a copy a build step fetched.
# shellcheck disable=SC2086
matches=$(grep -rnE $GREP_EXCLUDE '"from"[[:space:]]*:[[:space:]]*"([^"]*/)?ani-cli"' gui/ 2>/dev/null || true)
if [ -n "$matches" ]; then
    printf 'arch/no_reacquired_script FAIL: a gui/ packaging manifest stages ani-cli as a resource:\n%s\n' "$matches" >&2
    failed=1
fi

# 4. No workflow may put the script on a runner. A runner carrying a
# binary the packages do not ship means an accidental dependency on
# it passes in CI and fails on a user's machine — the one failure
# end-to-end coverage exists to catch. Syntactic: an installing
# command that names the script; a job arranging the same thing some
# other way is not covered, and a green run says nothing about it.
if [ -d .github/workflows ]; then
    matches=$(grep -rnE '(install|cp|ln|mv)[^|]*[[:space:]]ani-cli' .github/workflows || true)
    if [ -n "$matches" ]; then
        printf 'arch/no_reacquired_script FAIL: a workflow puts ani-cli on the runner, which the packages do not ship:\n%s\n' "$matches" >&2
        failed=1
    fi
fi

if [ "$failed" -eq 0 ]; then
    printf 'arch/no_reacquired_script PASS\n'
fi
exit "$failed"
