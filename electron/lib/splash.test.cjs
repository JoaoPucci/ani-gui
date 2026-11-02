const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const { SPLASH_MARK, SPLASH_PAGE, splashWindowOptions } = require("./splash.cjs");

test("the splash is a small frameless window above everything, shown once painted", () => {
  const o = splashWindowOptions();
  assert.equal(o.frame, false);
  assert.equal(o.show, false);
  assert.equal(o.alwaysOnTop, true);
  assert.equal(o.resizable, false);
  assert.equal(o.skipTaskbar, false);
  assert.ok(o.width > 0 && o.width <= 640 && o.height > 0 && o.height <= 480);
  assert.match(o.backgroundColor, /^#[0-9a-f]{6}$/i);
  assert.equal(o.webPreferences.contextIsolation, true);
  assert.equal(o.webPreferences.nodeIntegration, false);
  assert.equal(o.webPreferences.preload, undefined);
});

test("the splash page ships with the app", () => {
  assert.ok(fs.existsSync(SPLASH_PAGE), SPLASH_PAGE);
  const pkg = JSON.parse(fs.readFileSync(path.join(__dirname, "..", "package.json"), "utf-8"));
  assert.ok(pkg.build.files.includes("splash/**/*"), "electron-builder's files allowlist carries it");
  assert.equal(SPLASH_MARK, "aniGuiSplash");
});
