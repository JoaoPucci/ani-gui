const test = require("node:test");
const assert = require("node:assert/strict");

const { E2E_BACKEND, E2E_OUTPUT, e2eConfig } = require("./e2e-package.cjs");
const manifest = require("../package.json");

const release = manifest.build;
const linuxBackend = (build) =>
  build.linux.extraResources.filter((r) => r.to === "ani-gui-backend");

test("the e2e package is written where no release is", () => {
  const e2e = e2eConfig(release);
  assert.equal(e2e.directories.output, E2E_OUTPUT);
  assert.notEqual(e2e.directories.output, release.directories.output);
  assert.equal(e2e.directories.buildResources, release.directories.buildResources);
});

test("the e2e package is an unpacked tree only, never an installer", () => {
  assert.deepEqual(e2eConfig(release).linux.target, ["dir"]);
});

test("the e2e package carries the test-run backend, and only it", () => {
  const e2e = e2eConfig(release);
  assert.deepEqual(
    linuxBackend(e2e).map((r) => r.from),
    [E2E_BACKEND],
  );
  // Everything else the Linux package carries is the release's.
  const rest = (build) =>
    build.linux.extraResources.filter((r) => r.to !== "ani-gui-backend");
  assert.deepEqual(rest(e2e), rest(release));
  assert.deepEqual(e2e.files, release.files);
  assert.deepEqual(e2e.extraResources, release.extraResources);
});

test("no release configuration reads the test-run backend", () => {
  const all = JSON.stringify(release);
  assert.ok(!all.includes("target-e2e"), "package.json's build names target-e2e");
  assert.ok(!all.includes(E2E_OUTPUT), `package.json's build names ${E2E_OUTPUT}`);
});

test("the release configuration is left as it was", () => {
  const before = JSON.stringify(release);
  e2eConfig(release);
  assert.equal(JSON.stringify(release), before);
});

test("the test-run backend is the one build:backend:e2e builds", () => {
  // Split into words; the script is a plain `cd … && cargo build …`.
  const words = manifest.scripts["build:backend:e2e"].split(/\s+/);
  const after = (flag) => words[words.indexOf(flag) + 1];
  assert.ok(words.includes("--features") && after("--features") === "test-run-key");
  assert.equal(after("cd"), "../backend");
  const built = `../backend/${after("--target-dir")}/${after("--target")}/release/ani-gui-backend`;
  assert.equal(built, E2E_BACKEND);
});

test("a release configuration without its Linux backend is refused", () => {
  const without = {
    ...release,
    linux: {
      ...release.linux,
      extraResources: release.linux.extraResources.filter(
        (r) => r.to !== "ani-gui-backend",
      ),
    },
  };
  assert.throws(() => e2eConfig(without), /backend/);
});

test("a release backend from somewhere unexpected is refused", () => {
  const moved = {
    ...release,
    linux: {
      ...release.linux,
      extraResources: release.linux.extraResources.map((r) =>
        r.to === "ani-gui-backend" ? { ...r, from: "../elsewhere/ani-gui-backend" } : r,
      ),
    },
  };
  assert.throws(() => e2eConfig(moved), /backend/);
});

test("the electron-builder config file is this configuration", () => {
  assert.deepEqual(require("../electron-builder.e2e.cjs"), e2eConfig(release));
});
