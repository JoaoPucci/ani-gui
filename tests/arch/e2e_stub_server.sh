#!/bin/sh
# Architectural invariant: an e2e spec's fake backend answers over real
# HTTP, through the shared stub server, and never in place.
#
# Playwright can answer an intercepted request itself. In this app that
# answer reaches the page without its status or headers: once the main
# process registers a webRequest listener on the window's session,
# Electron sends every request through its own loader, and an answer
# Playwright makes up in place arrives with status 0. Every stubbed
# page then reads its data as unreachable. The specs instead hand each
# stubbed request to the stub server (electron/e2e/stubs.ts,
# electron/lib/e2e-stub-server.cjs), which answers it over HTTP with
# the backend's gate and CORS rules in front, as the backend would.
#
# A syntactic constraint, not interpretation: Playwright's in-place
# answer is `fulfill`, and the word appears in no text file under
# electron/ that git tracks or would track, comments included. The
# constraint is that the token is absent, not that it is unused.
#
# One kind of file is left out: the `.txt` files directly in
# electron/build-resources/licenses/, the licence texts the packages
# ship beside their bundled binaries. They are copied verbatim and may
# not be edited, and the GPL's text has "fulfilling" in it. Any other
# file there, a script included, is still searched.
#
# What this does not cover: a spec outside electron/, or Playwright's
# in-place answer reached under a name that never spells the word —
# none exists; this check would not notice one.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

# Text files git tracks or would track (untracked, not ignored): the
# fetched bundled binaries and the build output are ignored, and a
# binary can hold the word by chance. Git is pointed at this checkout
# whatever the environment names, and a git that cannot answer fails
# the check: only its "no match" (exit 1) passes.
status=0
found=$(env -u GIT_DIR -u GIT_WORK_TREE -u GIT_INDEX_FILE -u GIT_OBJECT_DIRECTORY \
    -u GIT_CEILING_DIRECTORIES -u GIT_DISCOVERY_ACROSS_FILESYSTEM \
    -u GIT_LITERAL_PATHSPECS -u GIT_GLOB_PATHSPECS -u GIT_NOGLOB_PATHSPECS \
    -u GIT_ICASE_PATHSPECS \
    git -C "$REPO_ROOT" grep -lIF --untracked 'fulfill' -- electron \
    ':(exclude,glob)electron/build-resources/licenses/*.txt') || status=$?
if [ "$status" -gt 1 ]; then
    printf 'arch/e2e_stub_server FAIL: git could not search %s (exit %s)\n' "$REPO_ROOT" "$status" >&2
    exit 1
fi
if [ -n "$found" ]; then
    printf 'arch/e2e_stub_server FAIL: Playwright'\''s in-place answer is named under electron/;\n' >&2
    printf 'answer a stubbed request through answer() in electron/e2e/stubs.ts:\n%s\n' "$found" >&2
    exit 1
fi
