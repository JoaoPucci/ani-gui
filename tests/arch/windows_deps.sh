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
#   - `gui/electron/scripts/fetch-windows-deps.mjs` must exist as the
#     fetch driver (mirror of fetch-linux-deps.mjs).
#   - `gui/electron/package.json` must list `build-resources/win/bin`
#     under `build.win.extraResources` so electron-builder copies the
#     staged binaries into the NSIS payload.
#   - `dist:win` must chain `fetch:win-deps`, so invoking it directly
#     populates the bin dir the way `dist` does on Linux.
#   - the impersonating transport must be among the staged binaries.
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

if [ ! -d gui/electron ]; then
    printf 'arch/windows_deps: gui/electron does not exist yet — skipping\n'
    exit 0
fi

failed=0
PKG=gui/electron/package.json

# 1. The fetch driver script must exist alongside its Linux sibling.
if [ ! -f gui/electron/scripts/fetch-windows-deps.mjs ]; then
    printf 'arch/windows_deps FAIL: missing gui/electron/scripts/fetch-windows-deps.mjs\n' >&2
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
LINUX_FETCH=gui/electron/scripts/fetch-linux-deps.mjs
WINDOWS_FETCH=gui/electron/scripts/fetch-windows-deps.mjs

# The distinct dependencies a fetcher declares, one per line. An entry
# with no `dep` is an error rather than an anonymous member of the set:
# a silent `undefined` would compare equal across platforms and hide
# exactly what this is here to find.
canonical_deps() {
    node --input-type=module -e "
        import { DEPS } from '$REPO_ROOT/$1';
        const missing = DEPS.filter((d) => typeof d.dep !== 'string' || !d.dep);
        if (missing.length) {
            console.error('entries without a dep field: ' + missing.map((d) => d.name).join(', '));
            process.exit(1);
        }
        console.log([...new Set(DEPS.map((d) => d.dep))].sort().join('\n'));
    "
}

if [ -f "$LINUX_FETCH" ] && [ -f "$WINDOWS_FETCH" ]; then
    linux_deps=$(canonical_deps "$LINUX_FETCH")
    windows_deps=$(canonical_deps "$WINDOWS_FETCH")
    for dep in $linux_deps; do
        if ! printf '%s\n' "$windows_deps" | grep -qx -- "$dep"; then
            printf 'arch/windows_deps FAIL: %s is staged for Linux but not for Windows — a runtime dependency that lands on one packaged platform is unfinished\n' "$dep" >&2
            failed=1
        fi
    done
    for dep in $windows_deps; do
        if ! printf '%s\n' "$linux_deps" | grep -qx -- "$dep"; then
            printf 'arch/windows_deps FAIL: %s is staged for Windows but not for Linux — a runtime dependency that lands on one packaged platform is unfinished\n' "$dep" >&2
            failed=1
        fi
    done
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
FETCH=gui/electron/scripts/fetch-windows-deps.mjs
if [ -f "$FETCH" ]; then
    if ! grep -q "binary: 'curl-impersonate.exe'" "$FETCH"; then
        printf "arch/windows_deps FAIL: %s does not stage 'curl-impersonate.exe' — the native transport falls back to plain curl, which the provider 403s\n" "$FETCH" >&2
        failed=1
    fi
fi

[ "$failed" -eq 0 ] || {
    printf 'arch/windows_deps: FAILED\n'
    exit 1
}

printf 'arch/windows_deps: ok\n'
