#!/bin/sh

# Architectural invariant: `gui/` does not depend on the `ani-cli`
# script.
#
# This used to read "may invoke it only via subprocess". That was the
# rule while the app drove the script for playback and downloads: the
# app shipped a vendored copy, spawned it, and kept it updated, so the
# boundary worth policing was *how* it reached it. Resolution is
# native now and the packages no longer carry the script at all, so
# the boundary is simply that nothing under `gui/` reaches for it —
# by sourcing it, by carrying the test seam, or by declaring it as a
# packaged resource.
#
# The script itself still ships for people who want the terminal flow,
# which is why this check has a subject at all. When it goes, so does
# this file.
#
# What this does NOT check: that no Rust code spawns a process named
# `ani-cli`. Deciding which string literal reaches a spawn means
# reading Rust with regular expressions, and AGENTS.md §2 is explicit
# that inferring meaning from source that way does not terminate. The
# three checks below are each something a `grep` either finds or does
# not.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

if [ ! -d gui ]; then
    printf 'arch/boundaries: gui/ does not exist yet — skipping\n'
    exit 0
fi

failed=0

# Build artifacts (target/, node_modules/, bundles) are gitignored but
# may sit on disk and contain copies of the script. Skip them.
GREP_EXCLUDE='--exclude-dir=target --exclude-dir=node_modules --exclude-dir=build --exclude-dir=dist --exclude-dir=.svelte-kit'

# 1. No sourcing. The cheapest way to reacquire the dependency is to
# pull the script's functions into a shell the app runs, which would
# make every upstream change a merge problem again.
# shellcheck disable=SC2086
matches=$(grep -rnE $GREP_EXCLUDE '(^|[[:space:]])((source|\.)[[:space:]]+["'"'"']?[^"'"'"' ]*ani-cli)' gui/ 2>/dev/null || true)
if [ -n "$matches" ]; then
    printf 'arch/boundaries FAIL: gui/ sources ani-cli (forbidden):\n%s\n' "$matches" >&2
    failed=1
fi

# 2. The `__ANI_CLI_LIB__` source-guard is a seam for `tests/bash/`,
# which sources the script as a library. It previously had two
# legitimate homes under gui/ — the auto-updater re-applied the guard
# after every `ani-cli -U`, and app.rs documented that duty. Both are
# gone along with the updater, so any occurrence here now means a
# layer started reading the guard back out of the script.
# shellcheck disable=SC2086
matches=$(grep -rn $GREP_EXCLUDE '__ANI_CLI_LIB__' gui/ 2>/dev/null || true)
if [ -n "$matches" ]; then
    printf 'arch/boundaries FAIL: __ANI_CLI_LIB__ appears in gui/ — the test seam is for tests/bash/ only:\n%s\n' "$matches" >&2
    failed=1
fi

# 3. No packaging manifest may stage the script. This is the one that
# actually regressed in reverse: the packages carried the script long
# after anything read it, and the entries that put it there were
# exactly two `extraResources` lines. Re-adding one would ship a file
# the app has no code to use — dead weight the boot sweep then deletes
# from the user's cache on the next launch.
# shellcheck disable=SC2086
matches=$(grep -rnE $GREP_EXCLUDE '"from"[[:space:]]*:[[:space:]]*"([^"]*/)?ani-cli"' gui/ 2>/dev/null || true)
if [ -n "$matches" ]; then
    printf 'arch/boundaries FAIL: a gui/ packaging manifest stages ani-cli as a resource:\n%s\n' "$matches" >&2
    failed=1
fi

# A fourth check used to live here: gui/ must not mention the script's
# internal globals (`search_anime`, `allanime_key`, `get_aa_req`, …),
# on the grounds that naming one meant re-implementing scraping that
# should have been a subprocess call. It is deliberately not carried
# forward. Re-implementing resolution natively is the architecture
# now, so the rule guarded against the intended design; and every
# symbol it named belonged to the allanime pipeline that upstream 5.0
# deleted, so it had no subject left either. It passed vacuously,
# which is the failure AGENTS.md §2 warns about — a green run that
# reads as coverage of something nobody is checking.

# 4. Reverse: the script must not grow references to the GUI. It is
# vendored from upstream and syncs by merge; a GUI reference in it
# would be a fork patch nobody declared.
if [ -f ani-cli ]; then
    matches=$(grep -nE '(gui/|tauri::|use ani_gui|svelte)' ani-cli || true)
    if [ -n "$matches" ]; then
        printf 'arch/boundaries FAIL: ani-cli script contains GUI references:\n%s\n' "$matches" >&2
        failed=1
    fi
fi

# 5. No workflow may put the script on a test runner's PATH.
#
# AGENTS.md §4 says nothing in `gui/**` may name the script in a
# packaging manifest. A workflow is not a manifest, and the end-to-end
# job installed the vendored script into /usr/local/bin anyway — for a
# reason its own comment gave, that `AppState::build` resolves it
# through `find_in_path`. That stopped being true, and the step
# outlived it.
#
# The cost is not the false comment. A runner carrying a binary the
# packages deliberately do not means an accidental dependency on it
# passes there and fails on a user's machine, which is the one failure
# end-to-end coverage exists to catch.
#
# Syntactic: an installing command that names the script, in a
# workflow. It does not read the workflows for meaning, and a job that
# arranges the same thing some other way is not covered.
if [ -d .github/workflows ]; then
    matches=$(grep -rnE '(install|cp|ln|mv)[^|]*[[:space:]]ani-cli' .github/workflows || true)
    if [ -n "$matches" ]; then
        printf 'arch/boundaries FAIL: a workflow puts ani-cli on the runner, which the packages do not ship:\n%s\n' "$matches" >&2
        failed=1
    fi
fi

if [ "$failed" -eq 0 ]; then
    printf 'arch/boundaries PASS\n'
fi
exit "$failed"
