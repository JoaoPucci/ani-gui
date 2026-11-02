#!/bin/sh
# Architectural invariant: every e2e launch of Electron goes through
# the one shared launch helper, electron/e2e/launch.ts.
#
# That helper passes `--mute-audio`, and refuses a display someone may
# be looking at (electron/lib/e2e-display.cjs). A headless test launch still
# opens the host's audio device, and test runs have played sound on
# the desktop of whoever was running them. Muting is cheap to forget
# in a new spec and invisible when forgotten — nothing fails, the
# machine just makes noise — so it lives in one place, and this check
# keeps it there.
#
# Two halves, neither of them interpretation:
#
#   - this check, a syntactic constraint: Playwright's Electron driver
#     is `_electron`, and that token appears in exactly one file under
#     electron/ — the helper. Comments count; the constraint is that
#     the token is absent, not that it is unused.
#   - smoke.spec.ts runs the subject: it launches the packaged app
#     through the helper and asks the running app whether the switch
#     is on its command line.
#
# What this does not cover: a launch that reaches the binary without
# the driver — `child_process.spawn` on the packaged path, say — the
# driver reached under a name that never spells `_electron`, or a
# Playwright suite outside electron/, which is the only tree scanned.
# None exists; this check would not notice one.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

HELPER='electron/e2e/launch.ts'
failed=0

if [ ! -f "$HELPER" ]; then
    printf 'arch/e2e_launch FAIL: the shared launch helper %s does not exist\n' "$HELPER" >&2
    failed=1
fi

others=$(grep -rlF \
    --exclude-dir=node_modules --exclude-dir=dist \
    --exclude-dir=playwright-report --exclude-dir=test-results \
    '_electron' electron | grep -vxF "$HELPER" || true)
if [ -n "$others" ]; then
    printf 'arch/e2e_launch FAIL: Playwright'\''s Electron driver is named outside %s;\n' "$HELPER" >&2
    printf 'launch through the helper, which mutes audio and refuses a visible display:\n%s\n' "$others" >&2
    failed=1
fi

exit "$failed"
