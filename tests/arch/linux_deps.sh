#!/bin/sh
# Architectural invariant: the Linux packages (.deb + AppImage) must
# stage the binaries the app spawns, and the .deb must declare
# `Recommends: ffmpeg` so apt pulls the heavy distro build
# automatically.
#
# Without them a clean Ubuntu / Fedora desktop fails at the two points
# that matter and says little about why: every play dies on the
# provider's TLS interstitial without the impersonating transport, and
# every download dies without a downloader. Bundling the small fast
# ones removes that dependency on the user's environment; ffmpeg is
# too large to stage, hence the `Recommends:`.
#
# This once described the script's dependencies — `dep_ch fzf`
# aborting at startup, aria2c for downloads. Those went when the app
# stopped running the script, and the staged set is now the transport
# plus yt-dlp. The assertions below did not change with them, which is
# the hazard this header exists to avoid: a check whose stated subject
# has been retired still runs green, and the green reads as coverage
# of something nobody is checking.
#
# The Windows installer has the same shape
# (`fetch-windows-deps.mjs` + NSIS bundle) and `windows_deps.sh` is
# its counterpart, including an inventory comparison that holds the
# two platforms to the same dependency set. One difference is worth
# knowing: the e2e workflow runs `pnpm run dist`, so a Linux package
# really is built in CI, while nothing builds the Windows installer.
# On this side the configuration is checked and then exercised; on
# that side only the configuration.
#
# Specifically:
#   - `gui/electron/scripts/fetch-linux-deps.mjs` must exist as the
#     fetch driver (mirror of fetch-windows-deps.mjs).
#   - `gui/electron/package.json` must list `build-resources/linux/bin`
#     under `build.linux.extraResources` so electron-builder copies
#     the staged binaries into both AppImage and .deb payloads.
#   - `build.deb.recommends` must include "ffmpeg".
#   - `dist` / `dist:release` scripts must chain `fetch:linux-deps`
#     so any invocation path (package, dist, e2e) gets the bin
#     dir populated before electron-builder runs.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

if [ ! -d gui/electron ]; then
    printf 'arch/linux_deps: gui/electron does not exist yet — skipping\n'
    exit 0
fi

failed=0
PKG=gui/electron/package.json

# 1. The fetch driver script must exist alongside its Windows sibling.
if [ ! -f gui/electron/scripts/fetch-linux-deps.mjs ]; then
    printf 'arch/linux_deps FAIL: missing gui/electron/scripts/fetch-linux-deps.mjs\n' >&2
    failed=1
fi

# 2. `build.linux.extraResources` must include the build-resources/linux/bin entry.
#    Use a literal substring check: jq isn't a hard dep of the arch tests.
if ! grep -q '"from": *"build-resources/linux/bin"' "$PKG"; then
    printf 'arch/linux_deps FAIL: %s missing build.linux.extraResources entry for build-resources/linux/bin\n' "$PKG" >&2
    failed=1
fi

# 3. `build.deb.recommends` must include "ffmpeg" so `apt install ./...deb`
#    auto-pulls the distro build. Match on the array entry to avoid
#    false-positives on freeform mentions.
if ! grep -Pzo '"recommends"\s*:\s*\[[^]]*"ffmpeg"' "$PKG" >/dev/null 2>&1; then
    printf 'arch/linux_deps FAIL: %s missing "ffmpeg" in build.deb.recommends\n' "$PKG" >&2
    failed=1
fi

# 4. `dist` / `dist:release` scripts must chain `fetch:linux-deps` so
#    callers that invoke dist directly (e2e workflow) still get the bin
#    populated. The fetch-by-package pattern alone leaves a gap.
for s in dist dist:release; do
    line=$(grep -E "\"$s\"" "$PKG" || true)
    case "$line" in
        *fetch:linux-deps*) ;;
        *)
            printf 'arch/linux_deps FAIL: %s "%s" script does not chain fetch:linux-deps\n' "$PKG" "$s" >&2
            failed=1
            ;;
    esac
done

# 5. The impersonating transport must be staged with the other
#    bundled deps. Native resolution speaks to a provider whose
#    TLS-fingerprinting protection rejects plain curl, so a package
#    without curl-impersonate bricks playback, availability, and
#    downloads on every machine that hasn't hand-installed it — the
#    exact footgun bundling exists to remove. The backend walks its
#    failover names (fetch.rs CURL_FAILOVER) through the bundled bin
#    dir first, so the staged set must carry the patched binary plus
#    at least the first failover wrapper.
FETCH=gui/electron/scripts/fetch-linux-deps.mjs
if [ -f "$FETCH" ]; then
    for needed in "'curl-impersonate'" "'curl_firefox135'"; do
        if ! grep -q "binary: $needed" "$FETCH"; then
            printf 'arch/linux_deps FAIL: %s does not stage %s — the native transport falls back to plain curl, which the provider 403s\n' "$FETCH" "$needed" >&2
            failed=1
        fi
    done
fi

if [ "$failed" -ne 0 ]; then
    exit 1
fi
printf 'arch/linux_deps: OK\n'
