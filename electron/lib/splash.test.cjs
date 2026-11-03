const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");

const { EventEmitter } = require("node:events");

const {
  SPLASH_MARK,
  SPLASH_PAGE,
  SPLASH_MIN_ON_SCREEN_MS,
  SPLASH_MIN_ON_SCREEN_REDUCED_MS,
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

test("the splash holds still until it is told to move, and its fallback never starts motion", () => {
  // The splash opens before the app knows whether reduced motion is on,
  // so the page must not move on its own: its fallback, for a signal
  // that never comes, settles on the finished still frame.
  const css = read("splash.css");
  assert.match(css, /body:not\(\.shown\)[^{]*\{[^}]*animation-play-state: paused/);
  // The still frame applies to any page told to rest, so if the two
  // ever met, resting would win; the fallback itself (below) does not
  // rest a page the main process has set moving.
  assert.match(css, /body\.reduced \*[^{]*\{[^}]*animation: none !important/);
  assert.doesNotMatch(css, /body\.reduced:not\(\.shown\)/);
  const fallback =
    /^setTimeout\(\(\) => \{\n\s+if \(!document\.body\.classList\.contains\('shown'\)\) document\.body\.classList\.add\('reduced'\);\n\}, (\d+)\);$/m.exec(
      read("splash.js"),
    );
  assert.ok(fallback, "splash.js falls back to the still frame on a timer");
  assert.ok(Number(fallback[1]) <= 1000, `fallback after ${fallback[1]} ms`);
  assert.doesNotMatch(read("splash.js"), /classList\.add\('shown'\)/);
});

const tick = () => new Promise((r) => setImmediate(r));

/** A fake splash window whose shown signal answers when `answer` is
 *  called, recording the signal's code. */
function answeringSplash() {
  const win = fakeSplash();
  win.answer = () => {};
  win.webContents.executeJavaScript = (code) => {
    win.calls.push(`js:${code}`);
    return new Promise((resolve) => (win.answer = resolve));
  };
  return win;
}
const signals = (win) => win.calls.filter((c) => c.startsWith("js:"));

test("the splash opens at once, and is told to move only once the motion preference is known", async () => {
  const win = answeringSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  splash.show();
  win.emit("ready-to-show");
  await tick();
  assert.deepEqual(signals(win), [], "no signal before the preference");
  splash.motionPreference(false);
  const [signal] = signals(win);
  assert.ok(signal.includes("classList.add('shown')"), signal);
  // It answers after two animation frames: a frame with the splash in
  // it has been rendered, which the window being mapped does not mean.
  assert.equal((signal.match(/requestAnimationFrame/g) || []).length, 2);
});

test("a preference known before the window shows is sent as it shows", () => {
  const win = answeringSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  splash.show();
  splash.motionPreference(false);
  assert.deepEqual(signals(win), []);
  win.emit("ready-to-show");
  assert.equal(signals(win).length, 1);
});

test("under reduced motion the splash is told to rest on its still frame, never to move", () => {
  const win = answeringSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  splash.show();
  win.emit("ready-to-show");
  splash.motionPreference(true);
  const [signal] = signals(win);
  assert.ok(signal.includes("classList.add('reduced')"), signal);
  assert.ok(!signal.includes("classList.add('shown')"), signal);
});

test("the app's window waits until the splash has been on screen long enough for its opening to read", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  assert.equal(SPLASH_MIN_ON_SCREEN_MS, 500);
  const win = answeringSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {}, clock: () => 0 });
  splash.show();
  let up = false;
  const waiting = splash.whenShown().then(() => (up = true));
  win.emit("ready-to-show");
  splash.motionPreference(false);
  win.answer(false);
  for (let i = 0; i < 5; i++) await tick();
  assert.equal(lastSplashState().shown, true);
  assert.equal(up, false, "painted, but not yet on screen long enough");
  t.mock.timers.tick(SPLASH_MIN_ON_SCREEN_MS - 1);
  for (let i = 0; i < 5; i++) await tick();
  assert.equal(up, false);
  t.mock.timers.tick(1);
  await waiting;
  assert.equal(up, true);
});

test("a splash resting on its still frame needs a shorter time on screen", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  assert.equal(SPLASH_MIN_ON_SCREEN_REDUCED_MS, 300);
  const win = answeringSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {}, clock: () => 0 });
  splash.show();
  let up = false;
  const waiting = splash.whenShown().then(() => (up = true));
  win.emit("ready-to-show");
  splash.motionPreference(true);
  // The page answers whether it is resting: here it is.
  win.answer(true);
  for (let i = 0; i < 5; i++) await tick();
  t.mock.timers.tick(SPLASH_MIN_ON_SCREEN_REDUCED_MS - 1);
  for (let i = 0; i < 5; i++) await tick();
  assert.equal(up, false);
  t.mock.timers.tick(1);
  await waiting;
  assert.equal(up, true);
});

test("a splash that paints late still gets its time on screen, and the wait stays bounded", async (t) => {
  // The paint is waited for at most SPLASH_SHOW_WAIT_MS; a paint inside
  // that is followed by the minimum, so the wait never exceeds both.
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const win = answeringSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {}, clock: () => 0 });
  splash.show();
  let up = false;
  const waiting = splash.whenShown().then(() => (up = true));
  win.emit("ready-to-show");
  splash.motionPreference(false);
  t.mock.timers.tick(SPLASH_SHOW_WAIT_MS - 100);
  win.answer(false);
  for (let i = 0; i < 5; i++) await tick();
  t.mock.timers.tick(100);
  for (let i = 0; i < 5; i++) await tick();
  assert.equal(up, false, "the bound on the paint does not cut the minimum short");
  t.mock.timers.tick(SPLASH_MIN_ON_SCREEN_MS - 100);
  await waiting;
  assert.equal(up, true);
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
  splash.motionPreference(false);
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
  splash.motionPreference(false);
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
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const win = fakeSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  splash.show();
  let up = false;
  const waiting = splash.whenShown().then(() => (up = true));
  await new Promise((r) => setImmediate(r));
  assert.equal(up, false);
  win.emit("ready-to-show");
  splash.motionPreference(false);
  for (let i = 0; i < 5; i++) await new Promise((r) => setImmediate(r));
  t.mock.timers.tick(SPLASH_MIN_ON_SCREEN_MS);
  await waiting;
  assert.equal(up, true);

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
  b.motionPreference(false);
  assert.equal(await settlesAtOnce(t, bWait), true, "signal failed");
  // The signal answers.
  const fine = fakeSplash();
  const c = splashController({ createWindow: () => fine, logError: () => {} });
  c.show();
  const cWait = c.whenShown();
  fine.emit("ready-to-show");
  c.motionPreference(false);
  // Painted: it then has its minimum time on screen to serve.
  assert.equal(await settlesAtOnce(t, cWait), false, "painted, serving its minimum");
  t.mock.timers.tick(SPLASH_MIN_ON_SCREEN_MS);
  assert.equal(await settlesAtOnce(t, cWait), true, "painted, minimum served");
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

test("a launch cancelled by closing the splash reveals nothing", () => {
  const app = fakeAppWindow();
  revealMaximized(app, {
    cancelled: () => true,
    onMapped: () => app.calls.push("listening"),
    afterShow: () => app.calls.push("splash-closed"),
  });
  assert.deepEqual(app.calls, []);
});

test("a launch's log says when the splash was asked for, shown, painted and closed", async (t) => {
  let now = 1000;
  const lines = [];
  const win = fakeSplash();
  const splash = splashController({
    createWindow: () => win,
    logError: () => {},
    log: (line) => lines.push(line),
    clock: () => now,
  });
  splash.show();
  now = 1180;
  win.emit("ready-to-show");
  now = 1230;
  splash.motionPreference(false);
  await tick();
  now = 1600;
  splash.close();
  assert.deepEqual(lines, [
    "[main] splash asked for (+0 ms)",
    "[main] splash shown (+180 ms)",
    "[main] splash painted (+230 ms)",
    "[main] splash closed by the boot (+600 ms, on screen 370 ms after painting)",
  ]);
});

test("the log says how a splash ended when the boot did not close it", () => {
  for (const [end, expected] of [
    ["suppress", "[main] splash put away: the boot failed (+0 ms)"],
    ["dismiss", "[main] splash closed before the boot was done (by the user or a quit): the launch is cancelled (+0 ms)"],
  ]) {
    const lines = [];
    const win = fakeSplash();
    const splash = splashController({
      createWindow: () => win,
      logError: () => {},
      log: (line) => lines.push(line),
      clock: () => 0,
    });
    splash.show();
    if (end === "suppress") splash.suppress();
    else {
      win.destroyed = true;
      win.emit("closed");
    }
    assert.equal(lines.at(-1), expected, end);
  }
});

test("a splash that never opens says so", () => {
  const lines = [];
  const splash = splashController({
    createWindow: () => null,
    logError: () => {},
    log: (line) => lines.push(line),
    clock: () => 0,
  });
  splash.show();
  assert.deepEqual(lines, ["[main] splash asked for (+0 ms)", "[main] splash not opened: no window was made"]);
});

test("a splash whose window cannot be made says so in the log, not only as an error", () => {
  const lines = [];
  const splash = splashController({
    createWindow: () => {
      throw new Error("no display");
    },
    logError: () => {},
    log: (line) => lines.push(line),
    clock: () => 0,
  });
  splash.show();
  assert.deepEqual(lines, [
    "[main] splash asked for (+0 ms)",
    "[main] splash not opened: the window could not be made",
  ]);
});

test("a splash closed before it painted says it never painted", () => {
  let now = 0;
  const lines = [];
  const win = fakeSplash();
  const splash = splashController({
    createWindow: () => win,
    logError: () => {},
    log: (line) => lines.push(line),
    clock: () => now,
  });
  splash.show();
  now = 90;
  splash.close();
  assert.deepEqual(lines, [
    "[main] splash asked for (+0 ms)",
    "[main] splash closed by the boot (+90 ms, never painted)",
  ]);
});

test("a window ready long after the splash painted pays nothing for the minimum", async (t) => {
  // The minimum counts from the paint, not from when the window asks.
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let now = 0;
  const win = answeringSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {}, clock: () => now });
  splash.show();
  win.emit("ready-to-show");
  splash.motionPreference(false);
  win.answer(false);
  for (let i = 0; i < 5; i++) await tick();
  now += 2000;
  t.mock.timers.tick(2000);
  assert.equal(await settlesAtOnce(t, splash.whenShown()), true);
});

test("a window ready part-way through the minimum waits only the rest of it", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let now = 0;
  const win = answeringSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {}, clock: () => now });
  splash.show();
  win.emit("ready-to-show");
  splash.motionPreference(false);
  win.answer(false);
  for (let i = 0; i < 5; i++) await tick();
  now += 300;
  t.mock.timers.tick(300);
  const waiting = splash.whenShown();
  assert.equal(await settlesAtOnce(t, waiting), false);
  now += SPLASH_MIN_ON_SCREEN_MS - 300;
  t.mock.timers.tick(SPLASH_MIN_ON_SCREEN_MS - 300);
  assert.equal(await settlesAtOnce(t, waiting), true);
});

test("a splash put away during its minimum releases the wait at once", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const win = answeringSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  splash.show();
  const waiting = splash.whenShown();
  win.emit("ready-to-show");
  splash.motionPreference(false);
  win.answer(false);
  for (let i = 0; i < 5; i++) await tick();
  splash.suppress();
  assert.equal(await settlesAtOnce(t, waiting), true);
});

test("the shorter minimum follows the page's answer, which also reflects the system's own setting", async (t) => {
  // Where Chromium itself reports reduced motion (Windows, say), the
  // main process may pass no preference while the page still rests.
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const win = answeringSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {}, clock: () => 0 });
  splash.show();
  const waiting = splash.whenShown();
  win.emit("ready-to-show");
  splash.motionPreference(false);
  win.answer(true);
  for (let i = 0; i < 5; i++) await tick();
  t.mock.timers.tick(SPLASH_MIN_ON_SCREEN_REDUCED_MS - 1);
  assert.equal(await settlesAtOnce(t, waiting), false);
  t.mock.timers.tick(1);
  assert.equal(await settlesAtOnce(t, waiting), true);
});

test("a clock stepping back after the paint never stretches the wait past the minimum", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let now = 10_000;
  const win = answeringSplash();
  const splash = splashController({ createWindow: () => win, logError: () => {}, clock: () => now });
  splash.show();
  win.emit("ready-to-show");
  splash.motionPreference(false);
  win.answer(false);
  for (let i = 0; i < 5; i++) await tick();
  now -= 60_000; // the system clock set back a minute
  const waiting = splash.whenShown();
  // Let the wait reach its timer before time moves.
  assert.equal(await settlesAtOnce(t, waiting), false);
  t.mock.timers.tick(SPLASH_MIN_ON_SCREEN_MS);
  assert.equal(await settlesAtOnce(t, waiting), true);
});

