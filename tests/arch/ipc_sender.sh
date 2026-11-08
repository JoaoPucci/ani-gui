#!/bin/sh

# Architectural invariant: every IPC handler in the Electron main
# process is registered through the sender-checking wrapper.
#
# Every document the main window loads runs the preload and gets the
# bridge. The navigation guard keeps the window on the app's own
# document; the sender check is the second line behind it: a handler
# acts only for a call from the main frame holding that document
# (electron/lib/ipc-guard.cjs).
#
# The check lives in one wrapper, and so does the one handle on
# ipcMain: the wrapper takes it from Electron itself (mainProcessIpc),
# and everything else gets the guarded object. A registration that goes
# around the check needs ipcMain, or a contents' or frame's own IPC
# object, under whatever name it is passed around by — the app-ready
# wait once took ipcMain as a parameter called `ipc` and registered on
# it raw, which a search for `ipcMain.on(` could not see. So the rule
# is on getting hold of them, not on the call that registers.
#
# Specifically, outside `electron/lib/ipc-guard.cjs`, no main-process
# source under `electron/` (`*.js`, `*.cjs`, `*.mjs`; tests,
# `node_modules` and `dist` excluded) contains, anywhere on a line,
# comments and strings included:
#
#   - the text `ipcMain` — no import, destructure, parameter or member
#     access of it;
#   - a member named `ipc` (`.ipc` not followed by a name character) —
#     the per-contents and per-frame IPC objects (`webContents.ipc`,
#     `webFrameMain.ipc`), which Electron consults before ipcMain;
#   - the name of the `ipc-message` or `ipc-message-sync` event, which
#     a webContents emits for every message it receives.
#
# A syntactic constraint, per AGENTS.md §2: the text is there or it is
# not. What it does NOT establish, and cannot without a JavaScript
# parser: a name assembled at runtime (`electron["ipc" + "Main"]`), the
# per-contents object taken by destructuring (`const { ipc } =
# win.webContents`), or the event's name built from pieces. A green run
# says no file names these objects in the spelled-out forms above, not
# that no file reaches them at all. At runtime, the e2e case "every IPC
# channel refuses a sender that is not the app's document" calls every
# ipcMain handler on an `ani-gui:` channel; it does not see the
# per-contents or per-frame objects, or a listener removed before it
# runs (the app-ready wait's).

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

pattern='ipcMain|[.]ipc([^[:alnum:]_$]|$)|ipc-message'

hits=$(find electron -path electron/node_modules -prune -o -path electron/dist -prune -o \
    -type f \( -name '*.js' -o -name '*.cjs' -o -name '*.mjs' \) \
    ! -name '*.test.cjs' ! -name '*.test.mjs' ! -name '*.test.js' \
    ! -path "$WRAPPER" -print |
    sort |
    while IFS= read -r file; do
        grep -nE "$pattern" "$file" | sed "s|^|$file:|" || true
    done)

if [ -n "$hits" ]; then
    printf 'arch/ipc_sender FAIL: ipcMain or a per-contents IPC object named outside %s, whose sender check every handler goes through:\n' "$WRAPPER" >&2
    printf '%s\n' "$hits" >&2
    exit 1
fi

printf 'arch/ipc_sender: ipcMain and the per-contents IPC objects are named only in %s\n' "$WRAPPER"
