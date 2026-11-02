const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const { EventEmitter } = require("node:events");

const {
  SPLASH_MARK,
  SPLASH_PAGE,
  lastSplashState,
  splashController,
  splashWindowOptions,
} = require("./splash.cjs");

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
  win.webContents = {
    executeJavaScript: async (code) => {
      win.calls.push(`js:${code}`);
    },
  };
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

const splashDir = path.dirname(SPLASH_PAGE);
const read = (name) => fs.readFileSync(path.join(splashDir, name), "utf-8");

test("the splash is the pull-down: a film strip of the mark, a reel title and a counter", () => {
  const html = read("index.html");
  assert.match(html, /class="track"/);
  assert.match(html, /class="wordmark"/);
  assert.match(html, /class="counter"/);
  for (const file of ["splash.css", "splash.js", "mark.svg", "sprocket.svg"]) {
    assert.ok(fs.existsSync(path.join(splashDir, file)), file);
  }
});

test("the mark comes from mark.svg alone, so a new identity swaps one file", () => {
  const html = read("index.html");
  const srcs = [...html.matchAll(/<img [^>]*src="([^"]+)"/g)].map((m) => m[1]);
  assert.ok(srcs.length > 0);
  assert.deepEqual([...new Set(srcs)], ["mark.svg"]);
  assert.doesNotMatch(read("splash.css"), /mark\.svg/);
});

test("under reduced motion nothing on the splash moves", () => {
  assert.match(read("splash.css"), /@media \(prefers-reduced-motion: reduce\)[^{]*\{[^}]*animation: none !important/);
});

test("the splash's motion waits for the window to show, with a timer only as the fallback", () => {
  const css = read("splash.css");
  assert.match(css, /body:not\(\.shown\)[^{]*\{[^}]*animation-play-state: paused/);
  assert.match(read("splash.js"), /classList\.add\('shown'\)/);
});

test("the splash is told it is shown when its window shows", () => {
  const win = fakeSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  splash.show();
  win.emit("ready-to-show");
  win.emit("show");
  assert.ok(
    win.calls.includes("js:document.body.classList.add('shown')"),
    JSON.stringify(win.calls),
  );
});

test("a splash that cannot be told it is shown keeps its fallback, and is only logged", async () => {
  const win = fakeSplash();
  win.webContents.executeJavaScript = async () => {
    throw new Error("render frame was disposed");
  };
  const logged = [];
  const splash = splashController({ createWindow: () => win, logError: (...a) => logged.push(a) });
  splash.show();
  win.emit("show");
  await new Promise((r) => setImmediate(r));
  assert.equal(logged.length, 1);
});

test("what happened to the launch's splash can be read back afterwards", () => {
  // A test run starts fast enough that the splash has come and gone
  // before a harness can look at its windows; the end-to-end suite
  // reads this instead.
  const win = fakeSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  assert.deepEqual(lastSplashState(), { opened: false, shown: false, closed: false, suppressed: false });
  splash.show();
  assert.deepEqual(lastSplashState(), { opened: true, shown: false, closed: false, suppressed: false });
  win.emit("ready-to-show");
  win.emit("show");
  splash.close();
  assert.deepEqual(lastSplashState(), { opened: true, shown: true, closed: true, suppressed: false });
  const failed = splashController({ createWindow: () => fakeSplash(), logError: () => {} });
  failed.show();
  failed.suppress();
  assert.deepEqual(lastSplashState(), { opened: true, shown: false, closed: false, suppressed: true });
});
