// Which file of the packaged bundle an app:// request names.
//
// The protocol handler serves the SvelteKit bundle from disk. A request
// may name only a file inside the bundle directory, on Linux and on
// Windows, and a path that does not decode is a bad request, not an
// error thrown out of the handler (which would answer with no policy
// header at all).

const test = require("node:test");
const assert = require("node:assert/strict");
const path = require("node:path");
const fc = require("fast-check");

const { bundleTarget } = require("./bundle-path.cjs");

const POSIX = "/opt/ani-gui/resources/frontend/build";
const WIN = "C:\\Users\\u\\AppData\\Local\\Programs\\ani-gui\\resources\\frontend\\build";

test("the root is the index", () => {
  assert.deepEqual(bundleTarget(POSIX, "/", path.posix), {
    kind: "file",
    file: `${POSIX}/index.html`,
    route: false,
  });
  assert.deepEqual(bundleTarget(POSIX, "", path.posix).file, `${POSIX}/index.html`);
});

test("an asset is its file; a path with no extension is a route the index answers", () => {
  assert.deepEqual(bundleTarget(POSIX, "/_app/immutable/entry/start.js", path.posix), {
    kind: "file",
    file: `${POSIX}/_app/immutable/entry/start.js`,
    route: false,
  });
  assert.deepEqual(bundleTarget(POSIX, "/anime/12", path.posix), {
    kind: "file",
    file: `${POSIX}/anime/12`,
    route: true,
  });
});

test("a path that does not decode is a bad request", () => {
  assert.deepEqual(bundleTarget(POSIX, "/%E0%A4%A", path.posix), { kind: "bad" });
});

test("nothing outside the bundle, sibling directories included", () => {
  for (const p of ["/../secret", "/..%2fsecret", "/%2e%2e/secret", "/../build-x/a.js", "/..%2Fbuild-x%2Fa.js"]) {
    assert.deepEqual(bundleTarget(POSIX, p, path.posix), { kind: "forbidden" }, p);
  }
});

test("Windows: backslashes and drive paths stay inside the bundle", () => {
  assert.equal(bundleTarget(WIN, "/_app/x.js", path.win32).file, `${WIN}\\_app\\x.js`);
  for (const p of ["/..%5Csecret", "/..%5C..%5Cx", "/../build-x/a.js", "/C:%5CWindows%5Cwin.ini"]) {
    assert.notEqual(bundleTarget(WIN, p, path.win32).kind, "file", p);
  }
});

test("property: whatever is asked, a file it names lies inside the bundle", () => {
  for (const [dir, mod] of [
    [POSIX, path.posix],
    [WIN, path.win32],
  ]) {
    fc.assert(
      fc.property(
        fc.array(fc.constantFrom("a", "..", ".", "%2e", "%2f", "%5c", "/", "\\", "build-x", "C:", "%"), {
          maxLength: 8,
        }),
        (parts) => {
          const out = bundleTarget(dir, `/${parts.join("")}`, mod);
          if (out.kind !== "file") return;
          const rel = mod.relative(dir, out.file);
          // A name that merely starts with dots (`..a`) is inside.
          const escapes = rel === ".." || rel.startsWith(`..${mod.sep}`) || mod.isAbsolute(rel);
          assert.ok(!escapes, `${parts.join("")} → ${out.file}`);
        },
      ),
    );
  }
});
