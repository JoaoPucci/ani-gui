"use strict";

const test = require("node:test");
const assert = require("node:assert");
const path = require("node:path");

const { isRevealableDirectory } = require("./reveal-folder.cjs");

/** statSync over a fixed table: dirs, files, and everything else missing. */
function fakeFs({ dirs = [], files = [] }) {
  return (p) => {
    if (dirs.includes(p)) return { isDirectory: () => true };
    if (files.includes(p)) return { isDirectory: () => false };
    const err = new Error(`ENOENT: ${p}`);
    err.code = "ENOENT";
    throw err;
  };
}

test("an existing absolute directory opens (POSIX)", () => {
  const statSync = fakeFs({ dirs: ["/home/me/Videos/ani-gui"] });
  assert.strictEqual(
    isRevealableDirectory("/home/me/Videos/ani-gui", { pathModule: path.posix, statSync }),
    true,
  );
});

test("an existing absolute directory opens (Windows)", () => {
  const statSync = fakeFs({ dirs: ["C:\\Users\\me\\Videos\\ani-gui", "D:\\Anime"] });
  for (const dir of ["C:\\Users\\me\\Videos\\ani-gui", "D:\\Anime"]) {
    assert.strictEqual(isRevealableDirectory(dir, { pathModule: path.win32, statSync }), true, dir);
  }
});

test("a file is never opened — openPath would open or run it", () => {
  const posix = fakeFs({ files: ["/home/me/Downloads/run.sh", "/etc/hostname"] });
  for (const p of ["/home/me/Downloads/run.sh", "/etc/hostname"]) {
    assert.strictEqual(isRevealableDirectory(p, { pathModule: path.posix, statSync: posix }), false, p);
  }
  const win = fakeFs({ files: ["C:\\Users\\me\\Downloads\\setup.exe", "C:\\x.lnk"] });
  for (const p of ["C:\\Users\\me\\Downloads\\setup.exe", "C:\\x.lnk"]) {
    assert.strictEqual(isRevealableDirectory(p, { pathModule: path.win32, statSync: win }), false, p);
  }
});

test("a relative path, a URL, a missing path or a non-string is refused", () => {
  const statSync = fakeFs({ dirs: ["Videos", "/x"] });
  for (const p of ["Videos", "./Videos", "", "file:///x", "https://example.com/", null, undefined, 42, {}]) {
    assert.strictEqual(isRevealableDirectory(p, { pathModule: path.posix, statSync }), false, String(p));
    assert.strictEqual(isRevealableDirectory(p, { pathModule: path.win32, statSync }), false, String(p));
  }
  assert.strictEqual(isRevealableDirectory("/missing", { pathModule: path.posix, statSync }), false);
});

test("a stat that throws for any reason refuses", () => {
  const statSync = () => {
    throw new Error("EACCES");
  };
  assert.strictEqual(isRevealableDirectory("/root", { pathModule: path.posix, statSync }), false);
});
