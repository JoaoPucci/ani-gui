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
#   - `electron/scripts/fetch-linux-deps.mjs` must exist as the
#     fetch driver (mirror of fetch-windows-deps.mjs).
#   - `electron/package.json` must list `build-resources/linux/bin`
#     under `build.linux.extraResources` so electron-builder copies
#     the staged binaries into both AppImage and .deb payloads.
#   - `build.deb.recommends` must include "ffmpeg".
#   - `dist` / `dist:release` scripts must chain `fetch:linux-deps`
#     so any invocation path (package, dist, e2e) gets the bin
#     dir populated before electron-builder runs.
#   - The Linux packages carry a backend built for
#     `x86_64-unknown-linux-musl`, statically linked, and the scripts
#     that package them build that one. A backend linked against the
#     build host's glibc refuses to start on any system with an older
#     one — v0.14.1, built on a 2026 Ubuntu, asked for glibc 2.39 and
#     failed on Ubuntu 22.04 and Debian 12 before the window had
#     anything to show. The rest of the payload asks for 2.25 at most.
#     The e2e workflow, which builds the Linux package, then runs the
#     packaged backend's own check that it needs no system C library.

set -eu

REPO_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
cd "$REPO_ROOT"

if [ ! -d electron ]; then
    printf 'arch/linux_deps: electron does not exist yet — skipping\n'
    exit 0
fi

failed=0
PKG=electron/package.json

# 1. The fetch driver script must exist alongside its Windows sibling.
if [ ! -f electron/scripts/fetch-linux-deps.mjs ]; then
    printf 'arch/linux_deps FAIL: missing electron/scripts/fetch-linux-deps.mjs\n' >&2
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
FETCH=electron/scripts/fetch-linux-deps.mjs
if [ -f "$FETCH" ]; then
    for needed in "'curl-impersonate'" "'curl_firefox135'"; do
        if ! grep -q "binary: $needed" "$FETCH"; then
            printf 'arch/linux_deps FAIL: %s does not stage %s — the native transport falls back to plain curl, which the provider 403s\n' "$FETCH" "$needed" >&2
            failed=1
        fi
    done
fi

# 6. The Linux packages take the static backend: package.json is read
#    with node's JSON parser (a real parser, not a pattern), and
#    `build.linux.extraResources` must place the musl build at
#    `ani-gui-backend`, while no entry the Linux packages receive —
#    the top-level list or the linux one — places anything else there.
#    electron-builder adds the platform's list to the top-level one.
if ! node -e '
const b = require(process.argv[1]).build;
const musl = "../backend/target/x86_64-unknown-linux-musl/release/ani-gui-backend";
const linux = [...(b.extraResources || []), ...((b.linux || {}).extraResources || [])];
const placed = linux.filter((e) => typeof e === "object" && e.to === "ani-gui-backend");
const ok = placed.length === 1 && placed[0].from === musl
  && ((b.linux || {}).extraResources || []).includes(placed[0]);
process.exit(ok ? 0 : 1);
' "$REPO_ROOT/$PKG"; then
    printf 'arch/linux_deps FAIL: %s does not place exactly the backend built for x86_64-unknown-linux-musl at ani-gui-backend in the Linux packages\n' "$PKG" >&2
    failed=1
fi

# 7. Every script that packages for Linux builds that backend first:
#    the `package` scripts, and `dist` / `dist:release`, which are run
#    on their own as well (the e2e workflow runs `dist`). Asserted on
#    the parsed scripts as exact text: each one begins with the build
#    followed by `&&`, so nothing runs before it and a failed build
#    stops the packaging, and the build script is exactly the musl
#    build. A reordered or merely mentioning script fails.
if ! node -e '
const s = require(process.argv[1]).scripts || {};
const build = "cd ../backend && cargo build --bin ani-gui-backend --release --target x86_64-unknown-linux-musl";
const first = "pnpm run build:backend:linux && ";
const bad = ["package", "package:release", "dist", "dist:release"]
  .filter((n) => typeof s[n] !== "string" || !s[n].startsWith(first));
if (s["build:backend:linux"] !== build) bad.push("build:backend:linux");
if (bad.length) { console.error(bad.join(" ")); process.exit(1); }
' "$REPO_ROOT/$PKG"; then
    printf 'arch/linux_deps FAIL: %s scripts above do not begin with the static backend build, or build:backend:linux is not exactly the musl build\n' "$PKG" >&2
    failed=1
fi

if [ "$failed" -ne 0 ]; then
    exit 1
fi
printf 'arch/linux_deps: OK\n'
