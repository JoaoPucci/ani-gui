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
#   - the named file exists beside the manifest;
#   - its sha256 matches, and its byte size matches when the entry
#     records a `size`;
#   - both are taken over the file's bytes, or, when the entry declares
#     `"encoding": "base64"`, over the bytes its text decodes to. Any
#     other encoding value fails rather than being skipped.
#
# And every regular file beside a manifest is listed in it, the same
# two-way rule the anidb/ Rust test (`fixture_manifest_matches_the_
# fixtures`) applies to its own directory: a fixture added without an
# entry is one the manifest cannot vouch for.
#
# What this does not cover: a directory under tests/fixtures/ with no
# MANIFEST.json at all (history/ and arch/ hold hand-written inputs and
# carry none), and files in subdirectories of a manifest's directory.
# Size is checked only where an entry records it; anidb/'s entries
# record a digest alone.

set -eu

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
REPO_ROOT="$(cd "${SCRIPT_DIR}/../.." && pwd)"

# shellcheck disable=SC2016 # the ${...} are JavaScript template literals, not shell
if ! node -e '
const fs = require("fs");
const path = require("path");
const crypto = require("crypto");
const root = process.argv[1];
const fixtures = path.join(root, "tests/fixtures");
const manifests = fs.readdirSync(fixtures, { recursive: true })
  .filter((p) => path.basename(p) === "MANIFEST.json")
  .sort();
const fail = [];
for (const rel of manifests) {
  const file = path.join(fixtures, rel);
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
    const target = path.join(dir, name);
    if (!fs.existsSync(target) || !fs.statSync(target).isFile()) {
      fail.push(`${at}: listed file does not exist`); continue;
    }
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
    if (!f.isFile() || f.name === "MANIFEST.json") continue;
    if (!Object.hasOwn(entries, f.name)) {
      fail.push(`${where}: ${f.name} sits beside the manifest but is not listed`);
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
