"use strict";

// The update check offers a release to an install only when the
// release carries a file whose name ends the way that platform's
// packages end (backend/src/meta/github.rs, `Platform::carries`). The
// endings live in backend/src/meta/release_assets.json, which the
// backend embeds at compile time. Should the packaging start naming
// its files any other way, every install would see "no update" from
// then on, without an error anywhere. This test holds the two
// together: it asks electron-builder itself what each packaged target
// is named, and requires every name to end with that target's entry.

const test = require("node:test");
const assert = require("node:assert/strict");

const {
  ENDINGS_FILE,
  packagedArtifacts,
  mismatches,
  readEndings,
} = require("./release-asset-names.cjs");
const pkg = require("../package.json");

test("every packaged file ends the way the update check expects", () => {
  assert.deepEqual(mismatches(pkg, readEndings()), []);
});

test("the names come from electron-builder, with its own arch spelling per format", () => {
  const names = packagedArtifacts(pkg).map((a) => `${a.platform}/${a.target}: ${a.name}`);
  assert.deepEqual(names.sort(), [
    `linux/AppImage: ani-gui-${pkg.version}-x86_64.AppImage`,
    `linux/deb: ani-gui-${pkg.version}-amd64.deb`,
    `windows/nsis: ani-gui-${pkg.version}-x64-setup.exe`,
  ]);
});

test("the endings file names a platform and target for every one it lists", () => {
  const endings = readEndings();
  assert.deepEqual(Object.keys(endings).sort(), ["linux", "windows"]);
  for (const [platform, targets] of Object.entries(endings)) {
    for (const [target, ending] of Object.entries(targets)) {
      assert.equal(typeof ending, "string", `${platform}/${target}`);
      assert.ok(ending.length > 0, `${platform}/${target}`);
    }
  }
  assert.match(ENDINGS_FILE, /release_assets\.json$/);
});

/** A copy of the real manifest with `edit` applied to its build config. */
function withBuild(edit) {
  const copy = structuredClone(pkg);
  edit(copy.build);
  return copy;
}

test("an artifact name that drops the arch is caught", () => {
  const renamed = withBuild((b) => {
    b.linux.artifactName = "${productName}-${version}.${ext}";
  });
  const found = mismatches(renamed, readEndings());
  assert.deepEqual(
    found.map((m) => m.target).sort(),
    ["AppImage", "deb"],
  );
});

test("a Windows installer renamed without -setup is caught", () => {
  const renamed = withBuild((b) => {
    b.win.artifactName = "${productName}-${version}-${arch}.${ext}";
  });
  assert.deepEqual(
    mismatches(renamed, readEndings()).map((m) => `${m.platform}/${m.target}`),
    ["windows/nsis"],
  );
});

test("a target-level name overrides the platform's, and is checked too", () => {
  const renamed = withBuild((b) => {
    b.deb.artifactName = "${name}_${version}_${arch}.${ext}";
  });
  assert.deepEqual(
    mismatches(renamed, readEndings()).map((m) => `${m.platform}/${m.target}: ${m.name}`),
    [`linux/deb: ani-gui_${pkg.version}_amd64.deb`],
  );
});

test("a packaged target the update check has no ending for is caught", () => {
  const added = withBuild((b) => {
    b.linux.target = [...b.linux.target, "rpm"];
  });
  assert.deepEqual(
    mismatches(added, readEndings()).map((m) => `${m.platform}/${m.target}`),
    ["linux/rpm"],
  );
});

test("an ending no packaged target produces is caught", () => {
  const endings = readEndings();
  endings.linux.snap = "-amd64.snap";
  assert.deepEqual(
    mismatches(pkg, endings).map((m) => `${m.platform}/${m.target}`),
    ["linux/snap"],
  );
});

test("a pattern left to electron-builder's default is refused, not guessed", () => {
  const unnamed = withBuild((b) => {
    delete b.win.artifactName;
  });
  const found = mismatches(unnamed, readEndings());
  assert.deepEqual(found.map((m) => `${m.platform}/${m.target}`), ["windows/nsis"]);
  assert.match(found[0].reason, /artifactName/);
});

test("the release builds take their targets and arches from the config this check reads", () => {
  // Targets named on electron-builder's command line replace the
  // config's `target` lists and their `arch`, so a release script
  // naming them would build files this check never expands. Each
  // release script's one electron-builder call names its platform and
  // nothing else. A syntactic check on the two scripts as written.
  const calls = {
    "dist:release": " && electron-builder --linux && ",
    "dist:win": " && electron-builder --win",
  };
  for (const [script, call] of Object.entries(calls)) {
    const line = pkg.scripts[script];
    assert.equal(line.split("electron-builder").length - 1, 1, `${script}: one electron-builder call`);
    assert.ok(line.includes(call), `${script}: ${line}`);
    if (script === "dist:win") assert.ok(line.endsWith(call), `${script}: ${line}`);
  }
});
