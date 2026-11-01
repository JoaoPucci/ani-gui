#!/bin/sh

# Architectural invariant: the Windows package must ship the same
# runtime dependencies the Linux packages do, including the
# impersonating transport native resolution needs.
#
# `linux_deps.sh` has asserted the Linux half of this since the
# bundling landed, and says in its own header that the Windows
# installer has the same shape. It had no counterpart, so the
# provider migration staged the transport for Linux alone and nothing
# went red: a Windows build installs, opens and browses metadata, then
# fails every play and every download, because the resolver falls back
# to a plain curl the provider answers with its interstitial. An
# invariant written for two platforms and enforced on one is worse
# than none — its green run reads as coverage of both.
#
# Specifically:
#   - `electron/scripts/fetch-windows-deps.mjs` must exist as the
#     fetch driver (mirror of fetch-linux-deps.mjs).
#   - `electron/package.json` must list `build-resources/win/bin`
#     under `build.win.extraResources` so electron-builder copies the
#     staged binaries into the NSIS payload.
#   - `dist:win` must chain `fetch:win-deps`, so invoking it directly
#     populates the bin dir the way `dist` does on Linux.
#   - the impersonating transport must be among the staged binaries.
#   - ffmpeg must be among them too, and the installer script must
#     fetch nothing at install time.
#
# What this check does NOT establish: that the fetch succeeds, that
# the archive still contains the entry the dep names, or that the
# staged binary runs. Those want a Windows packaging job, and there is
# none — CI runs `cargo test` on windows-latest and never builds the
# installer, which is the other half of why the gap shipped unnoticed.
# So a green run here means the package is *configured* to carry a
# transport, not that a built installer has one.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

if [ ! -d electron ]; then
    printf 'arch/windows_deps: electron does not exist yet — skipping\n'
    exit 0
fi

failed=0
PKG=electron/package.json

# 1. The fetch driver script must exist alongside its Linux sibling.
if [ ! -f electron/scripts/fetch-windows-deps.mjs ]; then
    printf 'arch/windows_deps FAIL: missing electron/scripts/fetch-windows-deps.mjs\n' >&2
    failed=1
fi

# 2. `build.win.extraResources` must include the build-resources/win/bin
#    entry. Literal substring check: jq isn't a hard dep of the arch tests.
if ! grep -q '"from": *"build-resources/win/bin"' "$PKG"; then
    printf 'arch/windows_deps FAIL: %s missing build.win.extraResources entry for build-resources/win/bin\n' "$PKG" >&2
    failed=1
fi

# 3. `dist:win` must chain `fetch:win-deps`. `package:win` chains it,
#    but a caller invoking dist:win directly gets an empty bin dir and
#    a package that silently lacks its transport — the Linux side
#    closes this same gap on `dist` and `dist:release`.
line=$(grep -E '"dist:win"' "$PKG" || true)
case "$line" in
    *fetch:win-deps*) ;;
    *)
        printf 'arch/windows_deps FAIL: %s "dist:win" script does not chain fetch:win-deps\n' "$PKG" >&2
        failed=1
        ;;
esac

# 4. The two fetchers must declare the same dependencies. Checking one
#    hard-coded name only guards that name: every other bundled
#    dependency could land on one platform alone and both checks would
#    still pass, which is the failure this file exists to end rather
#    than to reproduce one binary at a time.
#
#    A dependency may be declared once per build — Linux stages the
#    impersonate binary plus a wrapper per browser out of one archive —
#    so each entry names the dependency it belongs to in its own `dep`
#    field, and the comparison is over those. That identity is
#    declared, not deduced: an earlier version of this check folded a
#    name into any shorter name it started with, which would have
#    swallowed a genuinely separate `aria2-helper` into `aria2` and
#    passed. Reading meaning out of a name's shape is the trap
#    `AGENTS.md` §2 describes, so the inventory is read with a real
#    parser — node imports the module and reports what it declares.
#    The fetchers guard their own entry point, so importing one costs
#    nothing and downloads nothing.
#
#    One asymmetry is declared rather than tolerated: Linux takes ffmpeg
#    from the distribution — the .deb recommends it — and says so in
#    `SYSTEM_DEPS`. Windows has no package manager to recommend through,
#    so whatever Linux takes from the system, Windows must stage, and
#    `linux_deps.sh` holds every such entry to `build.deb.recommends`.
#    The declaration is a list the parser reads, not a name this check
#    exempts, so it cannot excuse a dependency Linux neither stages nor
#    recommends.
LINUX_FETCH=electron/scripts/fetch-linux-deps.mjs
WINDOWS_FETCH=electron/scripts/fetch-windows-deps.mjs

# The distinct dependencies a fetcher declares under one export, one
# per line. An entry with no `dep` is an error rather than an anonymous
# member of the set: a silent `undefined` would compare equal across
# platforms and hide exactly what this is here to find. So is a missing
# export, for the same reason.
#
# Run as `node -e "$INVENTORY" inventory <module> <export>`: the
# arguments travel as argv, so a path is never spliced into the program
# text. The leading `inventory` is a placeholder, and it is load-bearing:
# a fetcher runs itself when `process.argv[1]` names it, so the module
# must never sit in that slot, or reading the inventory downloads it.
INVENTORY='
const [file, name] = process.argv.slice(2);
const mod = await import(file);
const list = mod[name];
if (!Array.isArray(list)) {
    console.error(file + " does not export " + name + " as an array");
    process.exit(1);
}
const missing = list.filter((d) => typeof d.dep !== "string" || !d.dep);
if (missing.length) {
    console.error("entries without a dep field: " + missing.map((d) => d.name).join(", "));
    process.exit(1);
}
console.log([...new Set(list.map((d) => d.dep))].sort().join("\n"));
'

if [ -f "$LINUX_FETCH" ] && [ -f "$WINDOWS_FETCH" ]; then
    inventory_ok=1
    linux_deps=$(node --input-type=module -e "$INVENTORY" inventory "$REPO_ROOT/$LINUX_FETCH" DEPS) || inventory_ok=0
    linux_system=$(node --input-type=module -e "$INVENTORY" inventory "$REPO_ROOT/$LINUX_FETCH" SYSTEM_DEPS) || inventory_ok=0
    windows_deps=$(node --input-type=module -e "$INVENTORY" inventory "$REPO_ROOT/$WINDOWS_FETCH" DEPS) || inventory_ok=0
    if [ "$inventory_ok" -eq 0 ]; then
        printf 'arch/windows_deps FAIL: a fetcher inventory could not be read (see above)\n' >&2
        failed=1
    else
        for dep in $linux_deps; do
            if ! printf '%s\n' "$windows_deps" | grep -qx -- "$dep"; then
                printf 'arch/windows_deps FAIL: %s is staged for Linux but not for Windows — a runtime dependency that lands on one packaged platform is unfinished\n' "$dep" >&2
                failed=1
            fi
            if printf '%s\n' "$linux_system" | grep -qx -- "$dep"; then
                printf 'arch/windows_deps FAIL: %s is both staged for Linux and declared as taken from the system there — one of the two is wrong\n' "$dep" >&2
                failed=1
            fi
        done
        for dep in $linux_system; do
            if ! printf '%s\n' "$windows_deps" | grep -qx -- "$dep"; then
                printf 'arch/windows_deps FAIL: %s comes from the system on Linux, and Windows has no package manager to recommend it through — the Windows installer must stage it\n' "$dep" >&2
                failed=1
            fi
        done
        for dep in $windows_deps; do
            if ! printf '%s\n' "$linux_deps" "$linux_system" | grep -qx -- "$dep"; then
                printf 'arch/windows_deps FAIL: %s is staged for Windows but not for Linux — a runtime dependency that lands on one packaged platform is unfinished\n' "$dep" >&2
                failed=1
            fi
        done
    fi
fi

# 5. The impersonating transport must be staged by name. Parity alone
#    would be satisfied by both platforms dropping it together, and
#    this is the one dependency whose absence has already shipped.
#    Windows stages the
#    bare patched binary and no wrappers: upstream ships the per-browser
#    entries as `.bat` files, and the resolver's suffix table is
#    deliberately narrower than PATHEXT so it never names something the
#    spawn cannot treat as curl. The binary carries its fingerprint via
#    `--impersonate` instead, which is why the failover list pairs a
#    target with it (fetch.rs CURL_FAILOVER).
FETCH=electron/scripts/fetch-windows-deps.mjs
if [ -f "$FETCH" ]; then
    if ! grep -q "binary: 'curl-impersonate.exe'" "$FETCH"; then
        printf "arch/windows_deps FAIL: %s does not stage 'curl-impersonate.exe' — the native transport falls back to plain curl, which the provider 403s\n" "$FETCH" >&2
        failed=1
    fi
fi

# 6. ffmpeg must be staged by name. Every Windows release through
#    0.17.0 fetched it from the installer script instead, into the
#    install directory but outside the package's own files — and
#    electron-builder's upgrade path runs the old version's uninstaller,
#    which empties that directory, before the new payload lands. So
#    each update deleted ffmpeg and downloaded it again (~90 MB), and an
#    update run offline ended with none. Shipped in the payload, it is
#    replaced with the rest of the app and never fetched.
if [ -f "$FETCH" ]; then
    if ! grep -q "binary: 'ffmpeg.exe'" "$FETCH"; then
        printf "arch/windows_deps FAIL: %s does not stage 'ffmpeg.exe' — the installer would have to fetch it, and an update deletes whatever the installer fetched\n" "$FETCH" >&2
        failed=1
    fi
fi

# 7. The installer script fetches nothing at install time, for the
#    reason above: anything it downloads lives in the install directory
#    outside the package's manifest, and the next update removes it.
#    Asserted as a constraint on the text, not an interpretation of it:
#    the file names no NSIS download plug-in and no URL at all, comment
#    or not. It does not catch a download routed through a spawned
#    program that is handed a URL built at run time; the installer has
#    no business building one either.
NSH=electron/build-resources/installer.nsh
if [ -f "$NSH" ]; then
    if grep -nE '(inetc|NSISdl|INetC)::|://' "$NSH" >&2; then
        printf 'arch/windows_deps FAIL: %s downloads at install time (lines above) — what it fetches is deleted by the next update; stage the file through fetch-windows-deps.mjs instead\n' "$NSH" >&2
        failed=1
    fi
fi

[ "$failed" -eq 0 ] || {
    printf 'arch/windows_deps: FAILED\n'
    exit 1
}

printf 'arch/windows_deps: ok\n'
