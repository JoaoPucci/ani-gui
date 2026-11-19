#!/bin/sh

# Advisories that `-o all` enables and that do not apply to a script
# whose job is to inspect this repository and report what it finds:
#
#   SC2016 — the `$APPDATA` and `$LOCALAPPDATA` in the patterns are
#       NSIS text to be found in the installer script, not shell
#       expansions, so single quotes are what keeps them literal
#
# shellcheck disable=SC2016
# Architectural invariant: a real Windows uninstall leaves nothing of
# the app behind that the user did not ask to keep.
#
# The uninstaller's purge prompt asks "Also delete your ani-gui
# settings, cache, and watch history?". Answered Yes, it removed the
# backend's two folders under `thirdmovement\ani-gui` and left the
# Electron profile, `%APPDATA%\ani-gui`: the Chromium profile, the
# window state and the `tokens` folder holding the tracker sign-ins.
# And every install leaves a copy of its own installer at
# `%LOCALAPPDATA%\ani-gui-updater\installer.exe` (electron-builder's
# `APP_INSTALLER_STORE_FILE`, written on every install and removed by
# nothing), which the app never reads: it does not use electron-updater.
# Both survived an uninstall of the published 0.18.0, checked on a
# Windows machine, and the installer script has carried no removal of
# either since it gained the purge.
#
# Specifically, `electron/build-resources/installer.nsh` must carry:
#   - the purge removing the Electron profile, `$APPDATA\ani-gui`;
#   - an uninstall removing the installer copy, `$LOCALAPPDATA\ani-gui-updater`;
#   - a switch to the current user's shell context and back, since a
#     per-machine install's uninstaller otherwise resolves both
#     folders under C:\ProgramData.
#
# What this check does NOT establish. It finds each instruction as a
# whole line of the file once indentation is dropped, and reads nothing
# around it. So it cannot tell whether a line is compiled into the
# uninstaller at all: one inside a `/* ... */` block comment, a macro
# nothing inserts, `customInstall`, or an `!if 0` passes it. Nor can it
# tell whether the profile removal sits under the purge's Yes, whether
# the installer copy is left alone during an update, or whether the
# context switch brackets the removals. Placing them is the installer
# script's job, and nothing in this repository runs NSIS. A line
# carrying a trailing `;` comment fails it, which is stricter than NSIS.
# It also cannot see the profile renamed: the folder is named for the
# app's production name in `electron/main.js`, and a change there has to
# change this too.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

NSH=electron/build-resources/installer.nsh

if [ ! -f "$NSH" ]; then
    printf 'arch/windows_uninstall FAIL: missing %s\n' "$NSH" >&2
    exit 1
fi

failed=0

# Each instruction must be a whole line once leading indentation is
# dropped, so one written after a `;` comment marker does not count.
LINES=$(sed 's/^[[:space:]]*//' "$NSH")

if ! printf '%s\n' "$LINES" | grep -Fx -- 'RMDir /r "$APPDATA\ani-gui"' >/dev/null; then
    printf 'arch/windows_uninstall FAIL: %s never removes the Electron profile ($APPDATA\\ani-gui): a purge answered Yes leaves the app profile and the tracker sign-ins behind\n' "$NSH" >&2
    failed=1
fi

if ! printf '%s\n' "$LINES" | grep -Fx -- 'RMDir /r "$LOCALAPPDATA\ani-gui-updater"' >/dev/null; then
    printf 'arch/windows_uninstall FAIL: %s never removes the installer copy ($LOCALAPPDATA\\ani-gui-updater): every uninstall leaves the installer behind\n' "$NSH" >&2
    failed=1
fi

# A per-machine install runs its uninstaller in the all-users shell
# context, where $APPDATA and $LOCALAPPDATA both name C:\ProgramData.
# The profile, the backend's folders and the installer copy are the
# user's own, so the removals have to switch to the current user's
# context first and back after, as electron-builder's own app-data
# removal does (uninstaller.nsh, "electron always uses per user app
# data").
if ! printf '%s\n' "$LINES" | grep -Fx -- 'SetShellVarContext current' >/dev/null ||
    ! printf '%s\n' "$LINES" | grep -Fx -- 'SetShellVarContext all' >/dev/null; then
    printf 'arch/windows_uninstall FAIL: %s does not switch to the current user'"'"'s shell context for its removals: on a per-machine install they reach C:\\ProgramData instead of the user'"'"'s folders\n' "$NSH" >&2
    failed=1
fi

[ "$failed" -eq 0 ] || {
    printf 'arch/windows_uninstall: FAILED\n'
    exit 1
}

printf 'arch/windows_uninstall: ok\n'
