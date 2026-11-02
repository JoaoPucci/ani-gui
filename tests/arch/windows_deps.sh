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
#   - ffmpeg must not be: the installer downloads it at install time,
#     as the fetcher's `INSTALL_TIME_DEPS` declares, and the installer
#     script must pin exactly what that declares and download nothing
#     else.
#   - every staged binary ships its licence text in the package, and
#     no licence text ships for a binary no package carries.
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
#    Two asymmetries are declared rather than tolerated, both for
#    ffmpeg. Linux takes it from the distribution — the .deb recommends
#    it — and says so in the Linux fetcher's `SYSTEM_DEPS`. Windows has
#    no package manager to recommend through, and the package does not
#    carry it either: redistributing gyan.dev's GPL build would oblige
#    an offer of corresponding source that cannot be made, because that
#    build does not publish the versions of the libraries it links. So
#    the installer downloads it at install time, and the Windows
#    fetcher says so in `INSTALL_TIME_DEPS`. Whatever Linux takes from
#    the system, Windows must stage or download; whatever Windows
#    downloads, Linux must stage or take from the system; and nothing is
#    both staged and downloaded on Windows, or both staged and taken
#    from the system on Linux. `linux_deps.sh` holds every `SYSTEM_DEPS`
#    entry to `build.deb.recommends`, and check 7 below holds every
#    `INSTALL_TIME_DEPS` entry to the installer script. The
#    declarations are lists the parser reads, not names this check
#    exempts, so neither can excuse a dependency a platform does not
#    provide at all.
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

windows_install_time=
if [ -f "$LINUX_FETCH" ] && [ -f "$WINDOWS_FETCH" ]; then
    inventory_ok=1
    linux_deps=$(node --input-type=module -e "$INVENTORY" inventory "$REPO_ROOT/$LINUX_FETCH" DEPS) || inventory_ok=0
    linux_system=$(node --input-type=module -e "$INVENTORY" inventory "$REPO_ROOT/$LINUX_FETCH" SYSTEM_DEPS) || inventory_ok=0
    windows_deps=$(node --input-type=module -e "$INVENTORY" inventory "$REPO_ROOT/$WINDOWS_FETCH" DEPS) || inventory_ok=0
    windows_install_time=$(node --input-type=module -e "$INVENTORY" inventory "$REPO_ROOT/$WINDOWS_FETCH" INSTALL_TIME_DEPS) || inventory_ok=0
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
            if ! printf '%s\n' "$windows_deps" "$windows_install_time" | grep -qx -- "$dep"; then
                printf 'arch/windows_deps FAIL: %s comes from the system on Linux, and Windows has no package manager to recommend it through — the Windows package must stage it or its installer download it\n' "$dep" >&2
                failed=1
            fi
        done
        for dep in $windows_deps; do
            if ! printf '%s\n' "$linux_deps" "$linux_system" | grep -qx -- "$dep"; then
                printf 'arch/windows_deps FAIL: %s is staged for Windows but not for Linux — a runtime dependency that lands on one packaged platform is unfinished\n' "$dep" >&2
                failed=1
            fi
            if printf '%s\n' "$windows_install_time" | grep -qx -- "$dep"; then
                printf 'arch/windows_deps FAIL: %s is both staged for Windows and downloaded by its installer — one of the two is wrong\n' "$dep" >&2
                failed=1
            fi
        done
        for dep in $windows_install_time; do
            if ! printf '%s\n' "$linux_deps" "$linux_system" | grep -qx -- "$dep"; then
                printf 'arch/windows_deps FAIL: %s is downloaded by the Windows installer but neither staged for Linux nor taken from the system there — a runtime dependency that lands on one packaged platform is unfinished\n' "$dep" >&2
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

# 6. ffmpeg is downloaded at install time, by name, and never staged.
#    The package must not carry it: gyan.dev's essentials build is GPL
#    and does not publish the versions of the libraries it links, so
#    the corresponding-source offer redistribution obliges cannot be
#    made. Keeping it out of the installer was also the original reason
#    for the install-time download — the zip is ~90 MB. Read from the
#    inventory above, and from package.json by its JSON parser: no
#    `build.win` resource or file entry may name it either, since a
#    stray entry would carry it past the fetcher.
if [ -f "$WINDOWS_FETCH" ]; then
    if ! printf '%s\n' "$windows_install_time" | grep -qx -- ffmpeg; then
        printf "arch/windows_deps FAIL: %s does not declare ffmpeg in INSTALL_TIME_DEPS — a Windows install would have no ffmpeg, and downloads need it\n" "$WINDOWS_FETCH" >&2
        failed=1
    fi
    if node --input-type=module -e "$INVENTORY" inventory "$REPO_ROOT/$WINDOWS_FETCH" DEPS 2>/dev/null | grep -qx ffmpeg; then
        printf "arch/windows_deps FAIL: %s stages ffmpeg into the Windows package — that redistributes gyan.dev's GPL build, whose corresponding source cannot be offered; the installer downloads it at install time instead\n" "$WINDOWS_FETCH" >&2
        failed=1
    fi
fi
if ! node -e '
const win = (require(process.argv[1]).build || {}).win || {};
const entries = [].concat(win.extraResources || [], win.extraFiles || [], win.files || []);
process.exit(/ffmpeg/i.test(JSON.stringify(entries)) ? 1 : 0);
' "$REPO_ROOT/$PKG"; then
    printf 'arch/windows_deps FAIL: %s build.win names ffmpeg in a resource or file entry — the package must not carry it; the installer downloads it at install time\n' "$PKG" >&2
    failed=1
fi

# 7. The installer script downloads what `INSTALL_TIME_DEPS` declares,
#    at the declared pin, and nothing else. Asserted as constraints on
#    the text, not an interpretation of it:
#
#    - package.json (parsed) points `nsis.include` at installer.nsh and
#      sets no `nsis.script`, so the file read here is the one that
#      ships; and installer.nsh pulls in no further file (`!include`,
#      `!addincludedir`), where a download could sit unread.
#    - For each entry, with KEY its `dep` upper-cased and `-` turned to
#      `_`, installer.nsh carries exactly one line
#      `!define KEY_VERSION "<version>"`, one `!define KEY_URL "<url>"`
#      and one `!define KEY_ARCHIVE_PATH "<archivePath>"` (with `/` as
#      `\`), each at column zero — so the pin the fetcher declares, and
#      the credits and the README are held to, is the pin the installer
#      fetches.
#    - Every line naming one of the NSIS download plug-ins (inetc,
#      NSISdl, NScurl, INetLoad — matched without regard to case, as
#      NSIS resolves them) names a declared `${KEY_URL}`, and every line
#      with a URL in it, comment or not, is one of the `KEY_URL` lines.
#
#    What it does not catch: a download routed through a spawned
#    program that is handed a URL built at run time, a plug-in outside
#    that list, and whether the macros around the download do what
#    their comments say — that the update keeps the copy, that the stamp
#    is honoured. Only Windows runs NSIS; those are the PR's manual
#    steps. A green run means the script pins what the fetcher declares
#    and carries no other usual marker of a download, not that the
#    download works.
NSH=electron/build-resources/installer.nsh
if ! node -e '
const n = (require(process.argv[1]).build || {}).nsis || {};
process.exit(n.include === "build-resources/installer.nsh" && n.script === undefined ? 0 : 1);
' "$REPO_ROOT/$PKG"; then
    printf 'arch/windows_deps FAIL: %s nsis.include is not exactly build-resources/installer.nsh, or nsis.script is set — the checks below would not be reading the installer script that ships\n' "$PKG" >&2
    failed=1
fi
if [ ! -f "$NSH" ]; then
    printf 'arch/windows_deps FAIL: %s is missing — nothing downloads what INSTALL_TIME_DEPS declares\n' "$NSH" >&2
    failed=1
else
    if grep -niE '^[[:space:]]*!(include|addincludedir)' "$NSH" >&2; then
        printf 'arch/windows_deps FAIL: %s pulls in another file (lines above) — keep the installer hooks in this one file, where the check below can read them\n' "$NSH" >&2
        failed=1
    fi
    if [ -f "$WINDOWS_FETCH" ] && ! node --input-type=module -e '
const [fetcher, nsh] = process.argv.slice(2);
const { INSTALL_TIME_DEPS } = await import(fetcher);
const { readFileSync } = await import("node:fs");
const lines = readFileSync(nsh, "utf8").split(/\r?\n/);
const bad = [];
const urlLines = new Set();
const urlRefs = [];
for (const d of Array.isArray(INSTALL_TIME_DEPS) ? INSTALL_TIME_DEPS : []) {
    const key = String(d.dep).toUpperCase().replace(/-/g, "_");
    const want = {
        VERSION: d.version,
        URL: d.url,
        ARCHIVE_PATH: typeof d.archivePath === "string" ? d.archivePath.replace(/\//g, "\\") : d.archivePath,
    };
    for (const [field, value] of Object.entries(want)) {
        if (typeof value !== "string" || !value) {
            bad.push(d.dep + ": INSTALL_TIME_DEPS entry has no " + field.toLowerCase());
            continue;
        }
        const line = "!define " + key + "_" + field + " \"" + value + "\"";
        const n = lines.filter((l) => l === line).length;
        if (n !== 1) bad.push("expected exactly one line [" + line + "], found " + n);
        if (field === "URL") urlLines.add(line);
    }
    urlRefs.push("$" + "{" + key + "_URL}");
}
lines.forEach((l, i) => {
    if (l.includes("://") && !urlLines.has(l)) bad.push("line " + (i + 1) + " names a URL INSTALL_TIME_DEPS does not declare: " + l.trim());
    if (/(inetc|nsisdl|nscurl|inetload)::/i.test(l) && !urlRefs.some((r) => l.includes(r))) bad.push("line " + (i + 1) + " calls a download plug-in on no declared URL: " + l.trim());
});
if (bad.length) { console.error(bad.join("\n")); process.exit(1); }
' inventory "$REPO_ROOT/$WINDOWS_FETCH" "$REPO_ROOT/$NSH"; then
        printf 'arch/windows_deps FAIL: %s does not download exactly what %s INSTALL_TIME_DEPS declares (above)\n' "$NSH" "$WINDOWS_FETCH" >&2
        failed=1
    fi
fi

# 8. Every binary the Windows packages bundle ships with its licence
#    text: `electron/build-resources/licenses/<dep>.txt`, non-empty,
#    for every `dep` in the fetcher's DEPS, copied into the package by
#    a `build.win.extraResources` entry from `build-resources/licenses`
#    to `licenses` whose filter, if it has one, names that file. The
#    licences of what these binaries bundle require their notice to
#    travel with every copy, and GPL object code needs its licence
#    beside it. Read with node: DEPS by import, package.json by its
#    JSON parser.
#
#    The other direction holds too: every file in that directory is the
#    licence of something a fetcher stages, for Linux or for Windows. A
#    licence text shipped for a binary no package carries — ffmpeg's,
#    say, which the installer downloads rather than carries — would tell
#    the reader the package redistributes it.
#
#    The filter is held to a constraint rather than evaluated: it may
#    be absent (everything is copied), or a name or list of names with
#    no `!` exclusion, and each licence file must be named in it
#    literally. A glob that would in fact match therefore fails here,
#    loudly; an exclusion is refused outright, since judging what it
#    removes would mean re-implementing electron-builder's matcher,
#    which the arch job has no node_modules to load. Not checked: that
#    a file's text is the right licence.
if ! node --input-type=module -e '
const [fetcher, linuxFetcher, pkg, root] = process.argv.slice(2);
const { DEPS } = await import(fetcher);
const { DEPS: LINUX_DEPS } = await import(linuxFetcher);
const { readFileSync, statSync, readdirSync } = await import("node:fs");
const extra = ((JSON.parse(readFileSync(pkg, "utf8")).build || {}).win || {}).extraResources || [];
const entry = extra.find((e) => typeof e === "object" && e.from === "build-resources/licenses" && e.to === "licenses");
const bad = [];
if (!entry) bad.push("no extraResources entry copies build-resources/licenses to licenses");
const filter = entry && entry.filter !== undefined ? [].concat(entry.filter) : null;
if (filter && filter.some((f) => typeof f !== "string" || f.startsWith("!"))) bad.push("the licenses filter has an exclusion or a non-string entry");
const dir = root + "/electron/build-resources/licenses";
for (const dep of new Set(DEPS.map((d) => d.dep))) {
    const file = dep + ".txt";
    let ok = false;
    try { const st = statSync(dir + "/" + file); ok = st.isFile() && st.size > 0; } catch {}
    if (!ok) bad.push(file + " is missing, empty, or not a file");
    if (filter && !filter.includes(file)) bad.push(file + " is not named in the filter");
}
const staged = new Set([...DEPS, ...LINUX_DEPS].map((d) => d.dep + ".txt"));
let present = [];
try { present = readdirSync(dir); } catch {}
for (const file of present) {
    if (!staged.has(file)) bad.push(file + " is a licence for something no fetcher stages — no package carries that binary");
}
if (bad.length) { console.error(bad.join("\n")); process.exit(1); }
' inventory "$REPO_ROOT/$WINDOWS_FETCH" "$REPO_ROOT/$LINUX_FETCH" "$REPO_ROOT/$PKG" "$REPO_ROOT"; then
    printf 'arch/windows_deps FAIL: a bundled binary does not ship its licence text, or a licence text ships for a binary no package carries (above)\n' >&2
    failed=1
fi

[ "$failed" -eq 0 ] || {
    printf 'arch/windows_deps: FAILED\n'
    exit 1
}

printf 'arch/windows_deps: ok\n'
