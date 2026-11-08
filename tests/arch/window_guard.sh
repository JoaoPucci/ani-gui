#!/bin/sh

# Architectural invariant: every window the Electron main process makes
# is made by the guarded maker, which installs the navigation guard and
# the permission handlers on it before anything loads.
#
# The guards were once installed by hand on the app's window alone, and
# the launch splash was made with a bare `new BrowserWindow` beside it
# with neither. Making every window in one place removes that choice.
#
# Specifically, outside `electron/lib/guarded-window.cjs`, no source
# under `electron/` (`*.js`, `*.cjs`, `*.mjs`, main process, preload,
# splash page and build scripts alike; tests, `node_modules` and `dist`
# excluded) has, on one line, the word `new` and then, before any `;`,
# the name `BrowserWindow`, `BrowserView` or `WebContentsView` — each
# makes a webContents of its own. That covers the class reached as a
# member (`new electron.BrowserWindow(…)`, `new
# (require("electron").BrowserWindow)(…)`) as well as by its bare name.
# Comments and strings count, and so does an unrelated `new` earlier on
# the line (`new Set([BrowserWindow.getFocusedWindow()])`): such a line
# fails the check and is reworded.
#
# A syntactic constraint, per AGENTS.md §2: the text is there or it is
# not. What it does NOT establish, and cannot without a JavaScript
# parser: a window made through another name for the class (`const W =
# BrowserWindow; new W(…)`), through `Reflect.construct`, or with `new`
# and the name on separate lines. A green run says no file makes a
# window or view in the spelled-out forms, not that every one is
# guarded. The end-to-end suite builds its own windows to drive the
# splash page by hand; it is TypeScript (`*.ts`), which this does not
# read.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

if [ ! -d electron ]; then
    printf 'arch/window_guard: electron does not exist yet — skipping\n'
    exit 0
fi

MAKER=electron/lib/guarded-window.cjs

if [ ! -f "$MAKER" ]; then
    printf 'arch/window_guard FAIL: %s is missing\n' "$MAKER" >&2
    exit 1
fi

pattern='(^|[^[:alnum:]_$])new([^[:alnum:]_$][^;]*)?[^[:alnum:]_$](BrowserWindow|BrowserView|WebContentsView)([^[:alnum:]_$]|$)'

hits=$(find electron -path electron/node_modules -prune -o -path electron/dist -prune -o \
    -type f \( -name '*.js' -o -name '*.cjs' -o -name '*.mjs' \) \
    ! -name '*.test.cjs' ! -name '*.test.mjs' ! -name '*.test.js' \
    ! -path "$MAKER" -print |
    sort |
    while IFS= read -r file; do
        grep -nE "$pattern" "$file" | sed "s|^|$file:|" || true
    done)

if [ -n "$hits" ]; then
    printf 'arch/window_guard FAIL: a window or view made outside %s, which guards every window it makes:\n' "$MAKER" >&2
    printf '%s\n' "$hits" >&2
    exit 1
fi

printf 'arch/window_guard: every spelled-out window and view is made in %s\n' "$MAKER"
