#!/bin/sh
# Architectural invariant: every fixture manifest under tests/fixtures/
# describes the files beside it.
#
# A MANIFEST.json records each fixture's sha256 (and, where the
# manifest carries one, its size) so a reviewer can tell a deliberate
# re-capture from an accidental edit. Nothing read those records, so a
# fixture could change under its manifest and the manifest went on
# vouching for bytes that no longer existed. This check reads them.
#
# Each manifest is parsed with node's JSON parser — a real parser, not
# a pattern. Its entries are the `fixtures` object when the manifest
# has one, and otherwise the whole object (the flat form anidb/ uses).
# For every entry:
#
#   - its name is a plain file name — no `/`, `\` or `:`, not `.` or `..`,
#     not absolute — so it cannot reach a file outside the manifest's
#     directory, on Linux or on Windows;
#   - the named file exists beside the manifest and is a regular file,
#     not a symbolic link (lstat, so a link is never read through);
#   - its sha256 matches, and its byte size matches when the entry
#     records a `size`;
#   - both are taken over the file's bytes, or, when the entry declares
#     `"encoding": "base64"`, over the bytes its text decodes to. Any
#     other encoding value fails rather than being skipped.
#
# And nothing beside a manifest escapes it. Every regular file there is
# listed in it, the same two-way rule the anidb/ Rust test
# (`fixture_manifest_matches_the_fixtures`) applies to its own
# directory: a fixture added without an entry is one the manifest
# cannot vouch for. Every subdirectory there carries a MANIFEST.json
# of its own, which is checked in turn. Anything else there — a
# symbolic link, a fifo, a socket — fails.
#
# Manifests are found by walking tests/fixtures/ through real
# directories only; a symbolic link is never followed, and a
# MANIFEST.json that is not a regular file fails.
#
# What this does not cover: a directory under tests/fixtures/ with no
# MANIFEST.json and no manifest-carrying parent (history/ and arch/
# hold hand-written inputs and carry none) — its contents, links
# included, are not examined. Size is checked only where an entry
# records it; anidb/'s entries record a digest alone.

set -eu

REPO_ROOT="${ARCH_REPO_ROOT:-$(cd "$(dirname "$0")/../.." && pwd)}"

# shellcheck disable=SC2016 # the ${...} are JavaScript template literals, not shell
if ! node -e '
const fs = require("fs");
const path = require("path");
const crypto = require("crypto");
const root = process.argv[1];
const fixtures = path.join(root, "tests/fixtures");
const fail = [];
const notRegular = (s) => (s.isSymbolicLink() ? "is a symbolic link" : "is not a regular file");
// Walk real directories only: a Dirent describes the entry itself, so
// a symbolic link to a directory is never descended into.
const manifests = [];
const walk = (d) => {
  for (const f of fs.readdirSync(d, { withFileTypes: true })) {
    const p = path.join(d, f.name);
    if (f.isDirectory()) walk(p);
    else if (f.name === "MANIFEST.json") {
      if (f.isFile()) manifests.push(p);
      else fail.push(`${path.relative(root, p)}: ${notRegular(f)}`);
    }
  }
};
walk(fixtures);
manifests.sort();
for (const file of manifests) {
  const dir = path.dirname(file);
  const where = path.relative(root, file);
  let m;
  try { m = JSON.parse(fs.readFileSync(file, "utf8")); }
  catch (e) { fail.push(`${where}: does not parse: ${e.message}`); continue; }
  if (m === null || typeof m !== "object" || Array.isArray(m)) {
    fail.push(`${where}: is not a JSON object`); continue;
  }
  const entries = Object.hasOwn(m, "fixtures") ? m.fixtures : m;
  if (entries === null || typeof entries !== "object" || Array.isArray(entries)) {
    fail.push(`${where}: its entries are not an object`); continue;
  }
  for (const [name, e] of Object.entries(entries)) {
    const at = `${where}: ${name}`;
    if (e === null || typeof e !== "object" || typeof e.sha256 !== "string") {
      fail.push(`${at}: entry has no sha256`); continue;
    }
    if (name === "" || name === "." || name === ".." || /[\/\\:]/.test(name)
      || path.isAbsolute(name) || path.basename(name) !== name) {
      fail.push(`${at}: entry name is not a file beside the manifest`); continue;
    }
    const target = path.join(dir, name);
    let st;
    try { st = fs.lstatSync(target); }
    catch { fail.push(`${at}: listed file does not exist`); continue; }
    if (!st.isFile()) { fail.push(`${at}: ${notRegular(st)}`); continue; }
    let bytes = fs.readFileSync(target);
    if (e.encoding === "base64") bytes = Buffer.from(bytes.toString("latin1"), "base64");
    else if (e.encoding !== undefined) {
      fail.push(`${at}: unknown encoding ${JSON.stringify(e.encoding)}`); continue;
    }
    const of = e.encoding ? "decoded " : "";
    if (e.size !== undefined && e.size !== bytes.length) {
      fail.push(`${at}: ${of}size is ${bytes.length}, manifest says ${e.size}`);
    }
    const sha = crypto.createHash("sha256").update(bytes).digest("hex");
    if (sha !== e.sha256) {
      fail.push(`${at}: ${of}sha256 is ${sha}, manifest says ${e.sha256}`);
    }
  }
  for (const f of fs.readdirSync(dir, { withFileTypes: true })) {
    // Listed names were judged above; MANIFEST.json is this file.
    if (f.name === "MANIFEST.json" || Object.hasOwn(entries, f.name)) continue;
    const at = `${where}: ${f.name}`;
    if (f.isFile()) {
      fail.push(`${where}: ${f.name} sits beside the manifest but is not listed`);
    } else if (f.isDirectory()) {
      const own = path.join(dir, f.name, "MANIFEST.json");
      let ok = false;
      try { ok = fs.lstatSync(own).isFile(); } catch {}
      // A subdirectory answers to its own manifest, which the walk
      // checks; without one its files would answer to nothing.
      if (!ok) fail.push(`${at}: is a directory with no MANIFEST.json`);
    } else {
      fail.push(`${at}: ${notRegular(f)}`);
    }
  }
}
if (manifests.length === 0) fail.push("tests/fixtures: no MANIFEST.json found");
for (const f of fail) console.error(`arch/fixture_manifests FAIL: ${f}`);
if (fail.length) process.exit(1);
console.log(`arch/fixture_manifests: OK (${manifests.length} manifests)`);
' "${REPO_ROOT}"; then
    exit 1
fi
