const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const { EventEmitter } = require("node:events");

const {
  SPLASH_MARK,
  SPLASH_PAGE,
  SPLASH_SHOW_WAIT_MS,
  lastSplashState,
  revealMaximized,
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
  // As a real BrowserWindow does, show() emits "show" there and then.
  win.show = () => {
    win.calls.push("show");
    win.emit("show");
  };
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
  const windowCalls = () => win.calls.filter((c) => !c.startsWith("js:"));
  assert.deepEqual(windowCalls(), ["load:index.html", "show"]);
  splash.close();
  assert.deepEqual(windowCalls(), ["load:index.html", "show", "close"]);
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
  const fallback = /^setTimeout\(\(\) => document\.body\.classList\.add\('shown'\), (\d+)\);$/m.exec(
    read("splash.js"),
  );
  assert.ok(fallback, "splash.js starts the motion on a timer");
  assert.ok(Number(fallback[1]) <= 1000, `fallback after ${fallback[1]} ms`);
});

const tick = () => new Promise((r) => setImmediate(r));

test("the splash is told it is shown when its window shows, and answers once it has painted", async () => {
  const win = fakeSplash();
  let painted;
  win.webContents.executeJavaScript = (code) => {
    win.calls.push(`js:${code}`);
    return new Promise((resolve) => (painted = resolve));
  };
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  splash.show();
  let up = false;
  const waiting = splash.whenShown().then(() => (up = true));
  win.emit("ready-to-show");
  const signal = win.calls.find((c) => c.startsWith("js:"));
  assert.ok(signal && signal.includes("classList.add('shown')"), JSON.stringify(win.calls));
  // It waits two animation frames, so it answers after a frame with
  // the splash in it has been drawn, not when the window is mapped.
  assert.equal((signal.match(/requestAnimationFrame/g) || []).length, 2);
  await tick();
  assert.equal(up, false, "mapped is not painted");
  assert.equal(lastSplashState().shown, false);
  painted(true);
  await waiting;
  assert.equal(up, true);
  assert.equal(lastSplashState().shown, true);
});

test("a splash that cannot be told it is shown keeps its fallback, is only logged, and holds nothing up", async () => {
  const win = fakeSplash();
  win.webContents.executeJavaScript = async () => {
    throw new Error("render frame was disposed");
  };
  const logged = [];
  const splash = splashController({ createWindow: () => win, logError: (...a) => logged.push(a) });
  splash.show();
  const waiting = splash.whenShown();
  win.emit("ready-to-show");
  await waiting;
  assert.equal(logged.length, 1);
});

test("a splash closed while the app's window waits for it releases the wait", async () => {
  const win = fakeSplash();
  win.webContents.executeJavaScript = () => new Promise(() => {});
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  splash.show();
  const waiting = splash.whenShown();
  win.emit("ready-to-show");
  win.closeFromOutside = () => {
    win.destroyed = true;
    win.emit("closed");
  };
  win.closeFromOutside();
  await waiting;
});

test("what happened to the launch's splash can be read back afterwards", async () => {
  // A test run starts fast enough that the splash has come and gone
  // before a harness can look at its windows; the end-to-end suite
  // reads this instead.
  const win = fakeSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  assert.deepEqual(lastSplashState(), { opened: false, shown: false, closed: false, suppressed: false });
  splash.show();
  assert.deepEqual(lastSplashState(), { opened: true, shown: false, closed: false, suppressed: false });
  win.emit("ready-to-show");
  await tick();
  splash.close();
  assert.deepEqual(lastSplashState(), { opened: true, shown: true, closed: true, suppressed: false });
  const failed = splashController({ createWindow: () => fakeSplash(), logError: () => {} });
  failed.show();
  failed.suppress();
  assert.deepEqual(lastSplashState(), { opened: true, shown: false, closed: false, suppressed: true });
});

test("the app's window can wait for the splash to be on screen, for a bounded time", async (t) => {
  // A fast start (a test run's, or a warm one) could otherwise show the
  // app's window before the splash had painted, and close a splash
  // nobody saw.
  const win = fakeSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  splash.show();
  let up = false;
  const waiting = splash.whenShown().then(() => (up = true));
  await new Promise((r) => setImmediate(r));
  assert.equal(up, false);
  win.emit("ready-to-show");
  await waiting;
  assert.equal(up, true);

  t.mock.timers.enable({ apis: ["setTimeout"] });
  const stuck = splashController({ createWindow: () => fakeSplash(), logError: () => {} });
  stuck.show();
  let gaveUp = false;
  const bounded = stuck.whenShown().then(() => (gaveUp = true));
  t.mock.timers.tick(SPLASH_SHOW_WAIT_MS - 1);
  await Promise.resolve();
  assert.equal(gaveUp, false);
  t.mock.timers.tick(1);
  await bounded;
  assert.equal(gaveUp, true);
});

test("there is nothing to wait for without a splash on its way", async () => {
  const none = splashController({ createWindow: () => null, logError: () => {} });
  none.show();
  await none.whenShown();
  const never = splashController({ createWindow: () => fakeSplash(), logError: () => {} });
  await never.whenShown();
  const gone = splashController({ createWindow: () => fakeSplash(), logError: () => {} });
  gone.show();
  gone.suppress();
  await gone.whenShown();
});

/** Settles `p` with no time allowed to pass: true if it settled. */
async function settlesAtOnce(t, p) {
  let done = false;
  p.then(() => (done = true));
  for (let i = 0; i < 5; i++) await tick();
  return done;
}

test("every way the splash cannot reach the screen releases the wait at once, not after its bound", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  // Closed while the signal is out (real disposal never answers it).
  const closing = fakeSplash();
  closing.webContents.executeJavaScript = () => new Promise(() => {});
  const a = splashController({ createWindow: () => closing, logError: () => {} });
  a.show();
  const aWait = a.whenShown();
  closing.emit("ready-to-show");
  closing.destroyed = true;
  closing.emit("closed");
  assert.equal(await settlesAtOnce(t, aWait), true, "closed");
  // The signal fails.
  const failing = fakeSplash();
  failing.webContents.executeJavaScript = async () => {
    throw new Error("render frame was disposed");
  };
  const b = splashController({ createWindow: () => failing, logError: () => {} });
  b.show();
  const bWait = b.whenShown();
  failing.emit("ready-to-show");
  assert.equal(await settlesAtOnce(t, bWait), true, "signal failed");
  // The signal answers.
  const fine = fakeSplash();
  const c = splashController({ createWindow: () => fine, logError: () => {} });
  c.show();
  const cWait = c.whenShown();
  fine.emit("ready-to-show");
  assert.equal(await settlesAtOnce(t, cWait), true, "painted");
  // The window cannot be made, or none is.
  for (const createWindow of [
    () => {
      throw new Error("no display");
    },
    () => null,
  ]) {
    const d = splashController({ createWindow, logError: () => {} });
    d.show();
    assert.equal(await settlesAtOnce(t, d.whenShown()), true, "never made");
  }
  // Still waiting while the signal is out and nothing has happened.
  const pending = fakeSplash();
  pending.webContents.executeJavaScript = () => new Promise(() => {});
  const e = splashController({ createWindow: () => pending, logError: () => {} });
  e.show();
  const eWait = e.whenShown();
  pending.emit("ready-to-show");
  assert.equal(await settlesAtOnce(t, eWait), false, "still out");
});

/** A fake of the app's window: records what is done to it. */
function fakeAppWindow() {
  const win = { calls: [], destroyed: false };
  win.isDestroyed = () => win.destroyed;
  win.maximize = () => win.calls.push("maximize");
  win.show = () => win.calls.push("show");
  return win;
}

test("revealing the app's window maps it maximized, shows it, then puts the splash away", () => {
  // Maximizing a hidden window maps it already maximized — the
  // open-maximized pattern; the listeners that act from the map on are
  // attached after it, and the splash goes once the window is shown,
  // not when its page later finishes loading.
  const app = fakeAppWindow();
  revealMaximized(app, {
    onMapped: () => app.calls.push("listening"),
    afterShow: () => app.calls.push("splash-closed"),
  });
  assert.deepEqual(app.calls, ["maximize", "listening", "show", "splash-closed"]);
});

test("an app window gone before it is revealed is left alone", () => {
  const app = fakeAppWindow();
  app.destroyed = true;
  revealMaximized(app, {
    onMapped: () => app.calls.push("listening"),
    afterShow: () => app.calls.push("splash-closed"),
  });
  assert.deepEqual(app.calls, []);
});

test("a splash the user closes asks the app to quit, and the boot reveals nothing", () => {
  // Once the app's window exists (hidden, loading), closing the splash
  // from the taskbar closes no last window, so no quit would follow of
  // itself; the user asked to stop the launch.
  const win = fakeSplash();
  let quits = 0;
  const splash = splashController({
    createWindow: () => win,
    logError: () => {},
    onDismissed: () => (quits += 1),
  });
  splash.show();
  win.emit("ready-to-show");
  assert.equal(splash.dismissed(), false);
  win.destroyed = true;
  win.emit("closed");
  assert.equal(quits, 1);
  assert.equal(splash.dismissed(), true);
});

test("closing or putting away the splash is not the user dismissing it", () => {
  for (const end of ["close", "suppress"]) {
    const win = fakeSplash();
    let quits = 0;
    const splash = splashController({
      createWindow: () => win,
      logError: () => {},
      onDismissed: () => (quits += 1),
    });
    splash.show();
    splash[end]();
    if (end === "suppress") {
      win.destroyed = true;
      win.emit("closed");
    }
    assert.equal(quits, 0, end);
    assert.equal(splash.dismissed(), false, end);
  }
});
