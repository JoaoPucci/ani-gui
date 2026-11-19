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
# Both survived an uninstall on every release through 0.18.0.
#
# Specifically, `electron/build-resources/installer.nsh` must carry:
#   - the purge removing the Electron profile, `$APPDATA\ani-gui`;
#   - an uninstall removing the installer copy, `$LOCALAPPDATA\ani-gui-updater`.
#
# What this check does NOT establish. It finds the two instructions as
# lines of the file, uncommented; it does not read the NSIS around
# them, so it cannot tell whether the profile removal sits under the
# purge's Yes, or whether the installer copy is spared during an update
# (an update's uninstaller runs with --updated, and that copy is the
# running installer's own). Placing them is the installer script's job,
# and nothing in this repository runs NSIS. It also cannot see a
# profile renamed: the folder is named for the app's production name
# in `electron/main.js`, and a change there has to change this too.

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

[ "$failed" -eq 0 ] || {
    printf 'arch/windows_uninstall: FAILED\n'
    exit 1
}

printf 'arch/windows_uninstall: ok\n'
