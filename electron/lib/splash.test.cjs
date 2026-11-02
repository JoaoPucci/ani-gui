const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const { EventEmitter } = require("node:events");

const { SPLASH_MARK, SPLASH_PAGE, splashController, splashWindowOptions } = require("./splash.cjs");

test("the splash is a small frameless window above everything, shown once painted", () => {
  const o = splashWindowOptions();
  assert.equal(o.frame, false);
  assert.equal(o.show, false);
  assert.equal(o.alwaysOnTop, true);
  assert.equal(o.resizable, false);
  assert.equal(o.skipTaskbar, false);
  assert.ok(o.width > 0 && o.width <= 640 && o.height > 0 && o.height <= 480);
  // The ink the app and the splash page paint (tokens.css --ink-000),
  // so nothing of another colour flashes before the page does.
  assert.equal(o.backgroundColor, "#0c0a09");
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

test("the splash page allows nothing but its own files", () => {
  const html = fs.readFileSync(SPLASH_PAGE, "utf-8");
  const csp = /http-equiv="Content-Security-Policy" content="([^"]*)"/.exec(html);
  assert.ok(csp, "the page carries a CSP");
  assert.equal(csp[1], "default-src 'self'");
});

test("the splash can be dragged out of the way", () => {
  const css = fs.readFileSync(path.join(path.dirname(SPLASH_PAGE), "splash.css"), "utf-8");
  assert.match(css, /-webkit-app-region:\s*drag/);
  assert.equal(splashWindowOptions().movable, true);
});

/** A fake BrowserWindow, enough for the controller. */
function fakeSplash() {
  const win = new EventEmitter();
  win.calls = [];
  win.destroyed = false;
  win.isDestroyed = () => win.destroyed;
  win.show = () => win.calls.push("show");
  win.hide = () => win.calls.push("hide");
  win.close = () => {
    win.calls.push("close");
    win.destroyed = true;
    win.emit("closed");
  };
  win.loadFile = async (file) => win.calls.push(`load:${path.basename(file)}`);
  return win;
}

test("a splash shows once it has painted, marked as the splash", () => {
  const win = fakeSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  splash.show();
  assert.equal(win[SPLASH_MARK], true);
  assert.deepEqual(win.calls, ["load:index.html"]);
  win.emit("ready-to-show");
  assert.deepEqual(win.calls, ["load:index.html", "show"]);
  splash.close();
  assert.deepEqual(win.calls, ["load:index.html", "show", "close"]);
});

test("a splash suppressed before it painted never comes back on top of the failure report", () => {
  const win = fakeSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  splash.show();
  splash.suppress();
  win.emit("ready-to-show");
  assert.ok(!win.calls.includes("show"), JSON.stringify(win.calls));
  assert.ok(win.calls.includes("hide"));
  // Hidden, not closed: closing the last window would start a quit.
  assert.ok(!win.calls.includes("close"));
});

test("closing or suppressing a splash that never opened, or is gone, does nothing", () => {
  const none = splashController({ createWindow: () => null, logError: () => {} });
  none.close();
  none.suppress();
  const win = fakeSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  splash.show();
  win.destroyed = true;
  splash.close();
  splash.suppress();
  assert.deepEqual(win.calls, ["load:index.html"]);
});

test("a splash asked to show after the boot closed or suppressed it opens nothing", () => {
  // main.js opens the splash once the reduced-motion read settles, so a
  // boot can be over — window shown, or failed — before it asks.
  for (const end of ["close", "suppress"]) {
    let created = 0;
    const splash = splashController({
      createWindow: () => {
        created += 1;
        return fakeSplash();
      },
      logError: () => {},
    });
    splash[end]();
    splash.show();
    assert.equal(created, 0, end);
  }
});

test("a splash window that cannot be created is logged, and nothing else happens", () => {
  const logged = [];
  const splash = splashController({
    createWindow: () => {
      throw new Error("no display for a second window");
    },
    logError: (...args) => logged.push(args.join(" ")),
  });
  assert.doesNotThrow(() => splash.show());
  assert.equal(logged.length, 1);
  splash.close();
});
