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
# answer is `fulfill`, and the word appears nowhere under electron/,
# comments included. The constraint is that the token is absent, not
# that it is unused.
#
# What this does not cover: a spec outside electron/, or Playwright's
# in-place answer reached under a name that never spells the word —
# none exists; this check would not notice one.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

found=$(grep -rlF \
    --exclude-dir=node_modules --exclude-dir=dist \
    --exclude-dir=playwright-report --exclude-dir=test-results \
    'fulfill' electron || true)
if [ -n "$found" ]; then
    printf 'arch/e2e_stub_server FAIL: Playwright'\''s in-place answer is named under electron/;\n' >&2
    printf 'answer a stubbed request through answer() in electron/e2e/stubs.ts:\n%s\n' "$found" >&2
    exit 1
fi
