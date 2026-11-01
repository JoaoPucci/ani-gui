#!/bin/sh

# Architectural invariant: every IPC handler in the Electron main
# process is registered through the sender-checking wrapper.
#
# Every document the main window loads runs the preload and gets the
# bridge. The navigation guard keeps the window on the app's own
# document; the sender check is the second line behind it: a handler
# acts only for a call from the main frame holding that document
# (electron/lib/ipc-guard.cjs). The check lives in one wrapper, so the
# way to skip it is to call ipcMain directly — and that is what this
# looks for.
#
# Specifically: outside `electron/lib/ipc-guard.cjs`, no main-process
# source under `electron/` (`*.js`, `*.cjs`, `*.mjs`, tests and
# `node_modules` excluded) contains `ipcMain.` followed by a method
# that registers a listener — `handle`, `handleOnce`, `on`, `once`,
# `addListener`, `prependListener`, `prependOnceListener`.
#
# A syntactic constraint, per AGENTS.md §2: the text is there or it is
# not. What it does NOT establish, and cannot without a JavaScript
# parser: a registration reached through an alias (`const m = ipcMain;
# m.on(…)`), a computed member (`ipcMain["on"]`), a destructured method,
# or the electron module required under another name. A green run says
# no file registers through the spelled-out member, not that no file
# registers at all. The e2e case "every IPC channel refuses a sender
# that is not the app's document" covers the runtime side: it walks
# every channel the running app registered and expects each to refuse.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

if [ ! -d electron ]; then
    printf 'arch/ipc_sender: electron does not exist yet — skipping\n'
    exit 0
fi

WRAPPER=electron/lib/ipc-guard.cjs

if [ ! -f "$WRAPPER" ]; then
    printf 'arch/ipc_sender FAIL: %s is missing\n' "$WRAPPER" >&2
    exit 1
fi

pattern='ipcMain[[:space:]]*\.[[:space:]]*(handle|handleOnce|on|once|addListener|prependListener|prependOnceListener)[[:space:]]*\('

hits=$(find electron -path electron/node_modules -prune -o -path electron/dist -prune -o \
    -type f \( -name '*.js' -o -name '*.cjs' -o -name '*.mjs' \) \
    ! -name '*.test.cjs' ! -name '*.test.mjs' ! -name '*.test.js' \
    ! -path "$WRAPPER" -print |
    sort |
    while IFS= read -r file; do
        grep -nE "$pattern" "$file" | sed "s|^|$file:|" || true
    done)

if [ -n "$hits" ]; then
    printf 'arch/ipc_sender FAIL: IPC handlers registered on ipcMain directly, bypassing the sender check in %s:\n' "$WRAPPER" >&2
    printf '%s\n' "$hits" >&2
    exit 1
fi

printf 'arch/ipc_sender: every ipcMain registration goes through %s\n' "$WRAPPER"
