// The splash's stuck state: a launch whose first page has not said it
// rendered 15 s after the splash painted keeps the splash up, says it
// is taking longer than usual, and offers Restart and Quit. Nothing
// ever reveals a window that may still be empty.

const test = require("node:test");
const { mock } = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const vm = require("node:vm");
const { EventEmitter } = require("node:events");
const fc = require("fast-check");

const { SPLASH_PAGE, SPLASH_SHOW_WAIT_MS, lastSplashState, splashController, splashPageQuery } = require("./splash.cjs");
const { SPLASH_FLASH_MS, SPLASH_RELOADS, SPLASH_STUCK_MS, STUCK_CALL } = require("./splash-stuck.cjs");
const { APP_READY_CHANNEL, appReadyBound, appReadyWaiter, revealGate } = require("./app-window.cjs");
const { guardedIpc } = require("./ipc-guard.cjs");
const { appDocumentFrom } = require("./app-document.cjs");

/** The guarded IPC main.js hands the app-ready wait (lib/ipc-guard.cjs);
 *  `emit` sends as the app's main frame unless the event names another. */
function appIpc() {
  const raw = new EventEmitter();
  const ipc = guardedIpc(raw, { app: appDocumentFrom({ isDev: false }), logError: () => {} });
  ipc.emit = (channel, event) => raw.emit(channel, { senderFrame: { url: "app://localhost/", parent: null }, ...event });
  return ipc;
}
const { splashWords } = require("./splash-progress.cjs");
const { messagesFor } = require("./main-messages.cjs");

const tick = () => new Promise((r) => setImmediate(r));
const flush = async () => {
  for (let i = 0; i < 8; i++) await tick();
};

/** A fake splash BrowserWindow whose page answers the shown signal
 *  when told (`paint`), each step with its count, and the stuck call
 *  with a choice when one is made (`choose`). */
function stuckWindow() {
  const win = new EventEmitter();
  win.calls = [];
  win.destroyed = false;
  win.focused = false;
  win.visible = false;
  win.isDestroyed = () => win.destroyed;
  win.isFocused = () => win.focused;
  win.isVisible = () => win.visible;
  win.show = () => {
    win.calls.push("show");
    win.visible = true;
    win.emit("show");
  };
  win.showInactive = () => {
    win.calls.push("showInactive");
    win.visible = true;
  };
  win.hide = () => {
    win.calls.push("hide");
    win.visible = false;
  };
  win.close = () => {
    win.calls.push("close");
    win.destroyed = true;
    win.emit("closed");
  };
  win.setAlwaysOnTop = (flag) => win.calls.push(`onTop:${flag}`);
  win.flashFrame = (flag) => win.calls.push(`flash:${flag}`);
  win.loadFails = null;
  win.loadFile = async (_file, options) => {
    win.loadOptions = options;
    if (win.loadFails) throw win.loadFails;
  };
  let painted = () => {};
  let chosen = null;
  win.complete = false;
  win.paint = (resting = false) => painted(resting);
  win.failPaint = () => painted(Promise.reject(new Error("gone")));
  win.choose = (choice) => {
    const pick = chosen;
    chosen = null;
    if (pick) pick(choice);
    return Boolean(pick);
  };
  win.asked = () => win.calls.filter((c) => c === `js:${STUCK_CALL}`).length;
  win.webContents = Object.assign(new EventEmitter(), {
    reload: () => win.calls.push("reload"),
    setWindowOpenHandler: (fn) => (win.openHandler = fn),
    executeJavaScript: (code) => {
      win.calls.push(`js:${code}`);
      const m = /splashProgress\.set\((\d+)\)/.exec(code);
      if (m) {
        // The page's own rule: once the rail is whole the actions go
        // inert, and a choice can no longer be made.
        if (Number(m[1]) >= 3) {
          win.complete = true;
          chosen = null;
        }
        return Promise.resolve(Number(m[1]));
      }
      if (code === STUCK_CALL) {
        if (win.complete) return Promise.resolve(null);
        return new Promise((resolve) => (chosen = resolve));
      }
      return new Promise((resolve) => (painted = (v) => resolve(v)));
    },
  });
  return win;
}

/** A controller over a fresh stuckWindow, on a mocked clock, opened
 *  and (unless `paint` is false) painted at `paintAt`. */
async function stuckSplash(t, { paintAt = 0, paint = true, onRestart = () => {}, focused = false, resting = false } = {}) {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let now = 0;
  const win = stuckWindow();
  win.focused = focused;
  const logs = [];
  const errors = [];
  let dismissed = 0;
  const restarts = [];
  const splash = splashController({
    createWindow: () => win,
    logError: (...a) => errors.push(a.map(String).join(" ")),
    log: (line) => logs.push(line),
    clock: () => now,
    onDismissed: () => (dismissed += 1),
    onRestart: () => {
      restarts.push(now);
      return onRestart();
    },
  });
  splash.show();
  win.emit("ready-to-show");
  splash.motionPreference(resting);
  const later = async (ms) => {
    for (let left = ms; left > 0; left -= 250) {
      const step = Math.min(250, left);
      now += step;
      t.mock.timers.tick(step);
      await flush();
    }
  };
  if (paint) {
    await later(paintAt);
    win.paint(resting);
    await flush();
  }
  return {
    splash,
    win,
    logs,
    errors,
    later,
    restarts,
    dismissed: () => dismissed,
    now: () => now,
  };
}

test("the stuck threshold is 15 s", () => {
  assert.equal(SPLASH_STUCK_MS, 15_000);
  assert.equal(STUCK_CALL, "window.splashProgress && window.splashProgress.stuck && window.splashProgress.stuck()");
});

test("15 s after the splash painted with the page still pending, the splash goes stuck", async (t) => {
  const { splash, win, logs, later } = await stuckSplash(t, { paintAt: 400 });
  await splash.progress("backend");
  await splash.progress("flags");
  await later(SPLASH_STUCK_MS - 250);
  assert.equal(win.asked(), 0, "not yet");
  assert.equal(lastSplashState().stuck, false);
  await later(250);
  assert.equal(win.asked(), 1);
  assert.equal(lastSplashState().stuck, true);
  // It no longer covers everything else, and its taskbar entry flashes
  // once.
  assert.ok(win.calls.includes("onTop:false"), JSON.stringify(win.calls));
  assert.ok(win.calls.includes("flash:true"));
  assert.equal(logs.filter((l) => l.includes("splash stuck")).length, 1);
  assert.match(logs.find((l) => l.includes("splash stuck")), /page step.*15000 ms after the splash painted/);
  splash.close();
});

test("the threshold counts from the paint whatever step is pending: a slow backend goes stuck too", async (t) => {
  for (const landed of [[], ["backend"]]) {
    const { splash, win, logs, later } = await stuckSplash(t, { paintAt: 300 });
    for (const step of landed) await splash.progress(step);
    await later(SPLASH_STUCK_MS);
    assert.equal(win.asked(), 1, `after ${landed.length} steps`);
    const expected = landed.length === 0 ? "backend" : "flags";
    assert.match(logs.find((l) => l.includes("splash stuck")), new RegExp(`${expected} step`));
    splash.close();
    t.mock.timers.reset();
  }
});

test("a step landing does not restart the count", async (t) => {
  const { splash, win, later } = await stuckSplash(t);
  await later(10_000);
  await splash.progress("backend");
  await later(4_000);
  await splash.progress("flags");
  await later(1_000);
  assert.equal(win.asked(), 1);
  splash.close();
});

test("a page that renders before the threshold never sees the stuck state", async (t) => {
  const { splash, win, logs, later } = await stuckSplash(t);
  await splash.progress("backend");
  await splash.progress("flags");
  await later(SPLASH_STUCK_MS - 1_000);
  splash.pageReady(true);
  await later(10_000);
  assert.equal(win.asked(), 0);
  assert.equal(lastSplashState().stuck, false);
  assert.ok(!win.calls.includes("onTop:false"));
  assert.equal(logs.some((l) => l.includes("stuck")), false);
  splash.close();
});

test("Restart arranges the relaunch first, then ends this launch the way closing the splash does", async (t) => {
  const order = [];
  const { splash, win, logs, later, restarts, dismissed } = await stuckSplash(t, {
    onRestart: () => order.push(win.destroyed ? "after close" : "before close"),
  });
  await later(SPLASH_STUCK_MS);
  assert.equal(win.choose("restart"), true);
  await flush();
  assert.deepEqual(order, ["before close"]);
  assert.equal(restarts.length, 1);
  assert.ok(win.calls.includes("close"));
  assert.equal(dismissed(), 1, "the launch is cancelled, so the app quits");
  assert.equal(splash.dismissed(), true, "nothing is revealed");
  assert.equal(lastSplashState().choice, "restart");
  assert.equal(logs.filter((l) => l.includes("Restart chosen")).length, 1);
});

test("Quit ends the launch the way closing the splash does, and restarts nothing", async (t) => {
  const { splash, win, logs, later, restarts, dismissed } = await stuckSplash(t);
  await later(SPLASH_STUCK_MS);
  win.choose("quit");
  await flush();
  assert.equal(restarts.length, 0);
  assert.ok(win.calls.includes("close"));
  assert.equal(dismissed(), 1);
  assert.equal(splash.dismissed(), true);
  assert.equal(lastSplashState().choice, "quit");
  assert.equal(logs.filter((l) => l.includes("Quit chosen")).length, 1);
});

test("a restart that cannot be arranged keeps the splash and its choices", async (t) => {
  const { splash, win, errors, later, dismissed } = await stuckSplash(t, {
    onRestart: () => {
      throw new Error("no relauncher");
    },
  });
  await later(SPLASH_STUCK_MS);
  win.choose("restart");
  await flush();
  assert.ok(!win.calls.includes("close"), "still up");
  assert.equal(dismissed(), 0);
  assert.equal(splash.dismissed(), false);
  assert.ok(errors.some((e) => e.includes("no relauncher")));
  assert.equal(win.asked(), 2, "the choices are offered again");
  // Quit still works.
  win.choose("quit");
  await flush();
  assert.equal(dismissed(), 1);
});

test("an answer that is neither a choice nor nothing does nothing", async (t) => {
  // null and undefined mean the page has nothing to offer (a loss: "a
  // stuck splash whose page has nothing to offer is lost").
  for (const answer of ["", "reload", 1, { choice: "quit" }]) {
    const { splash, win, later, dismissed, restarts } = await stuckSplash(t);
    let lost = null;
    splash.lost().catch((err) => (lost = err));
    await later(SPLASH_STUCK_MS);
    win.choose(answer);
    await flush();
    assert.equal(dismissed(), 0, String(answer));
    assert.equal(restarts.length, 0);
    assert.equal(lost, null, String(answer));
    assert.ok(!win.calls.includes("close"));
    splash.close();
    t.mock.timers.reset();
  }
});

test("a page that renders after the stuck state hands over as usual, and says how late it was", async (t) => {
  const { splash, win, logs, later, dismissed } = await stuckSplash(t);
  await splash.progress("backend");
  await splash.progress("flags");
  await later(SPLASH_STUCK_MS + 2_000);
  splash.pageReady(true);
  await flush();
  assert.equal(
    win.calls.filter((c) => c.includes("splashProgress.set")).at(-1),
    "js:window.splashProgress && window.splashProgress.set(3)",
  );
  assert.equal(win.choose("restart"), false, "the actions went inert with the rail complete");
  assert.equal(splash.dismissed(), false);
  assert.match(logs.find((l) => l.includes("rendered")), /2000 ms after the splash went stuck/);
  splash.close();
  assert.equal(dismissed(), 0, "the boot closed it: no quit");
});

test("a choice that reaches the main process before the reveal wins over a page that landed meanwhile", async (t) => {
  const { splash, win, later, dismissed } = await stuckSplash(t);
  await later(SPLASH_STUCK_MS);
  // The click was made first: its answer is in flight as the page
  // lands.
  win.choose("quit");
  splash.pageReady(true);
  await flush();
  assert.equal(splash.dismissed(), true, "the reveal is cancelled");
  assert.equal(dismissed(), 1);
});

test("a choice that arrives once the boot is past the splash is ignored", async (t) => {
  for (const end of ["close", "suppress"]) {
    const { splash, win, later, dismissed, restarts } = await stuckSplash(t);
    await later(SPLASH_STUCK_MS);
    const pending = win.choose.bind(win);
    splash[end]();
    pending("restart");
    await flush();
    assert.equal(restarts.length, 0, end);
    assert.equal(dismissed(), 0, end);
    t.mock.timers.reset();
  }
});

test("a splash put away, closed by the user, or closed by the boot before the threshold never goes stuck", async (t) => {
  for (const end of ["close", "suppress", "user"]) {
    const { splash, win, later } = await stuckSplash(t);
    if (end === "user") win.close();
    else splash[end]();
    await later(SPLASH_STUCK_MS * 2);
    assert.equal(win.asked(), 0, end);
    assert.ok(!win.calls.includes("onTop:false"), end);
    t.mock.timers.reset();
  }
});

test("the taskbar flash is one flash, and none for a splash that already has focus", async (t) => {
  const { splash, win, later } = await stuckSplash(t);
  await later(SPLASH_STUCK_MS);
  assert.deepEqual(
    win.calls.filter((c) => c.startsWith("flash:")),
    ["flash:true"],
  );
  await later(SPLASH_FLASH_MS);
  assert.deepEqual(
    win.calls.filter((c) => c.startsWith("flash:")),
    ["flash:true", "flash:false"],
  );
  splash.close();
  t.mock.timers.reset();
  const focused = await stuckSplash(t, { focused: true });
  await focused.later(SPLASH_STUCK_MS + SPLASH_FLASH_MS);
  assert.deepEqual(focused.win.calls.filter((c) => c.startsWith("flash:")), []);
  assert.equal(focused.win.asked(), 1);
  focused.splash.close();
});

test("a splash whose paint never answers still goes stuck, counted from the paint's own bound", async (t) => {
  const { splash, win, later } = await stuckSplash(t, { paint: false });
  await later(SPLASH_SHOW_WAIT_MS + SPLASH_STUCK_MS - 250);
  assert.equal(win.asked(), 0);
  await later(250);
  assert.equal(win.asked(), 1);
  splash.close();
});

test("a splash that could not be told it is shown goes stuck 15 s from then", async (t) => {
  const { splash, win, later } = await stuckSplash(t, { paint: false });
  await later(200);
  win.failPaint();
  await flush();
  await later(SPLASH_STUCK_MS - 250);
  assert.equal(win.asked(), 0);
  await later(250);
  assert.equal(win.asked(), 1);
  splash.close();
});

test("a splash that is not on screen when it goes stuck is shown, without taking focus", async (t) => {
  const { splash, win, later } = await stuckSplash(t);
  win.visible = false;
  await later(SPLASH_STUCK_MS);
  assert.ok(win.calls.includes("showInactive"));
  splash.close();
});

test("the splash stands in for the window while it is up, and only then", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const never = splashController({ createWindow: () => null, logError: () => {} });
  assert.equal(never.standsIn(), false, "never opened");
  never.show();
  assert.equal(never.standsIn(), false, "no window was made");
  const thrown = splashController({
    createWindow: () => {
      throw new Error("no");
    },
    logError: () => {},
  });
  thrown.show();
  assert.equal(thrown.standsIn(), false);
  for (const end of ["close", "suppress", "user"]) {
    const win = stuckWindow();
    const splash = splashController({ createWindow: () => win, logError: () => {} });
    splash.show();
    assert.equal(splash.standsIn(), true, end);
    if (end === "user") win.close();
    else splash[end]();
    assert.equal(splash.standsIn(), false, end);
  }
});

test("the page is opened with its language, its name, and the stuck state's words", () => {
  const words = splashWords("pt-BR", messagesFor("pt-BR"));
  assert.deepEqual(words, {
    lang: "pt-BR",
    label: messagesFor("pt-BR").splashStarting,
    stuck: "Está demorando mais que o normal.",
    restart: "Reiniciar",
    quit: "Sair",
  });
  assert.deepEqual(splashPageQuery({ version: "0.18.0", stallTexts: [], words }), {
    v: "0.18.0",
    st: "700",
    l: "pt-BR",
    al: messagesFor("pt-BR").splashStarting,
    sl: "Está demorando mais que o normal.",
    sr: "Reiniciar",
    sq: "Sair",
  });
  const win = stuckWindow();
  const splash = splashController({ createWindow: () => win, logError: () => {}, version: "1", pageWords: () => words });
  splash.show();
  assert.equal(win.loadOptions.query.sl, words.stuck);
  assert.equal(win.loadOptions.query.l, "pt-BR");
  splash.close();
  // A failing read keeps the rest of the page.
  const broken = stuckWindow();
  const errors = [];
  const other = splashController({
    createWindow: () => broken,
    logError: (m) => errors.push(m),
    version: "1",
    pageWords: () => {
      throw new Error("x");
    },
  });
  other.show();
  assert.equal(broken.loadOptions.query.v, "1");
  assert.equal(broken.loadOptions.query.sl, undefined);
  assert.equal(errors.length, 1);
  other.close();
});

test("a splash whose page crashes is reloaded, and a stuck one offers its choices again", async (t) => {
  const { splash, win, logs, later } = await stuckSplash(t);
  win.webContents.emit("render-process-gone", {}, { reason: "crashed" });
  assert.ok(win.calls.includes("reload"), "reloaded before the stuck state");
  assert.ok(logs.some((l) => l.includes("splash's page went (crashed)")));
  await later(SPLASH_STUCK_MS);
  assert.equal(win.asked(), 1);
  win.webContents.emit("render-process-gone", {}, { reason: "oom" });
  win.webContents.emit("did-finish-load");
  await flush();
  assert.equal(win.asked(), 2, "the reloaded page offers the choices again");
  win.choose("quit");
  await flush();
  assert.equal(splash.dismissed(), true);
});

test("a splash page that keeps crashing is reloaded a bounded number of times", async (t) => {
  const { win, logs } = await stuckSplash(t);
  for (let i = 0; i < 6; i++) win.webContents.emit("render-process-gone", {}, { reason: "crashed" });
  assert.equal(win.calls.filter((c) => c === "reload").length, SPLASH_RELOADS);
  assert.ok(logs.some((l) => l.includes(`crashed ${SPLASH_RELOADS + 1} times: not reloaded again`)), JSON.stringify(logs));
});

test("a splash whose crash recovery runs out says it is lost, so the boot can fail instead of waiting forever", async (t) => {
  const { splash, win } = await stuckSplash(t);
  let lost = null;
  splash.lost().catch((err) => (lost = err));
  for (let i = 0; i < SPLASH_RELOADS; i++) {
    win.webContents.emit("render-process-gone", {}, { reason: "crashed" });
    await flush();
    assert.equal(lost, null, `after ${i + 1} crash(es) it is reloaded instead`);
  }
  win.webContents.emit("render-process-gone", {}, { reason: "crashed" });
  await flush();
  assert.ok(lost instanceof Error);
  assert.match(lost.message, new RegExp(`crashed ${SPLASH_RELOADS + 1} times`));
  splash.suppress();
});

/** A painted stuck-ready splash whose loss is watched. */
async function watchedLoss(t, options) {
  const made = await stuckSplash(t, options);
  made.lost = null;
  made.losses = () => made.logs.filter((l) => l.includes("[main] splash lost")).length;
  made.splash.lost().catch((err) => (made.lost = err));
  return made;
}

test("a splash page that fails to load is lost, so the page gets its bounded wait", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const win = stuckWindow();
  win.loadFails = Object.assign(new Error("ERR_FILE_NOT_FOUND (-6) loading 'file:///…/index.html'"), { code: "ERR_FILE_NOT_FOUND" });
  const logs = [];
  const splash = splashController({ createWindow: () => win, logError: () => {}, log: (l) => logs.push(l) });
  let lost = null;
  splash.lost().catch((err) => (lost = err));
  splash.show();
  await flush();
  assert.match(String(lost), /could not load/);
  assert.equal(logs.filter((l) => l.includes("[main] splash lost")).length, 1);
  splash.suppress();
});

test("a superseded load is not a lost splash", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const win = stuckWindow();
  win.loadFails = Object.assign(new Error("ERR_ABORTED (-3)"), { code: "ERR_ABORTED" });
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  let lost = null;
  splash.lost().catch((err) => (lost = err));
  splash.show();
  await flush();
  assert.equal(lost, null);
  splash.suppress();
});

test("a main-frame load failure is a lost splash; a subframe's, or an aborted one, is not", async (t) => {
  const made = await watchedLoss(t);
  made.win.webContents.emit("did-fail-load", {}, -3, "ERR_ABORTED", "file:///x", true);
  made.win.webContents.emit("did-fail-load", {}, -6, "ERR_FILE_NOT_FOUND", "file:///y", false);
  await flush();
  assert.equal(made.lost, null);
  made.win.webContents.emit("did-fail-load", {}, -6, "ERR_FILE_NOT_FOUND", "file:///x", true);
  await flush();
  assert.match(String(made.lost), /could not load/);
  made.splash.suppress();
});

test("the splash never navigates away or opens windows; a page that leaves anyway is lost", async (t) => {
  const made = await watchedLoss(t);
  let prevented = false;
  made.win.webContents.emit("will-navigate", { preventDefault: () => (prevented = true) }, "file:///home/me/dropped.png");
  assert.equal(prevented, true);
  assert.deepEqual(made.win.openHandler({ url: "https://example.com" }), { action: "deny" });
  // A reload of the splash's own page is not leaving it.
  made.win.webContents.emit("did-navigate", {}, `${require("node:url").pathToFileURL(SPLASH_PAGE).href}?v=1`);
  await flush();
  assert.equal(made.lost, null);
  made.win.webContents.emit("did-navigate", {}, "file:///home/me/dropped.png");
  await flush();
  assert.match(String(made.lost), /left its page/);
  made.splash.suppress();
});

test("the splash's own page is recognised in every shape Electron commits it in", async (t) => {
  // loadFile builds the URL with url.format, which leaves a bare % in a
  // path as it is; Chromium keeps it so.
  const urlLib = require("node:url");
  const shapes = [
    urlLib.format({ protocol: "file", slashes: true, pathname: "/home/a b/100%/app.asar/splash/index.html", search: "?v=1" }),
    urlLib.format({ protocol: "file", slashes: true, pathname: "/C:/Users/Jo%ão P/AppData/Local/Programs/ani-gui/resources/app.asar/splash/index.html", search: "?l=pt-BR" }),
    "file:///C:/Users/100%/splash/index.html",
    "file:///opt/ani-gui/resources/app.asar/splash/index.html",
  ];
  for (const url of shapes) {
    const made = await watchedLoss(t);
    made.win.webContents.emit("did-navigate", {}, url);
    await flush();
    assert.equal(made.lost, null, url);
    made.splash.suppress();
    t.mock.timers.reset();
  }
});

test("a splash page whose renderer exited while standing in is lost, though not reloaded", async (t) => {
  const made = await watchedLoss(t);
  made.win.webContents.emit("render-process-gone", {}, { reason: "clean-exit" });
  await flush();
  assert.ok(!made.win.calls.includes("reload"));
  assert.match(String(made.lost), /exited/);
});

test("a stuck splash whose page has nothing to offer is lost", async (t) => {
  // Its words never reached it, or its script did not load: the stuck
  // call answers null or undefined while the page is still late.
  for (const answer of [null, undefined]) {
    const made = await watchedLoss(t);
    made.win.webContents.executeJavaScript = ((orig) => (code) =>
      code.includes("stuck()") ? (made.win.calls.push(`js:${code}`), Promise.resolve(answer)) : orig(code))(
      made.win.webContents.executeJavaScript,
    );
    await made.later(SPLASH_STUCK_MS);
    assert.match(String(made.lost), /no way out/, String(answer));
    t.mock.timers.reset();
  }
});

test("a splash whose page completed just before the stuck state was due is neither asked nor lost", async (t) => {
  const made = await watchedLoss(t);
  await made.splash.progress("backend");
  await made.splash.progress("flags");
  await made.later(SPLASH_STUCK_MS - 250);
  made.splash.pageReady(true);
  await made.later(250);
  await flush();
  assert.equal(made.lost, null);
  made.splash.close();
});

test("a splash window closed before the boot is past it is lost as well as cancelling the launch", async (t) => {
  const made = await watchedLoss(t);
  made.win.close();
  await flush();
  assert.equal(made.dismissed(), 1);
  assert.match(String(made.lost), /closed/);
});

test("a splash closed by the boot, or after a choice, is not lost", async (t) => {
  const byBoot = await watchedLoss(t);
  byBoot.splash.close();
  await flush();
  assert.equal(byBoot.lost, null);
  t.mock.timers.reset();
  const byChoice = await watchedLoss(t);
  await byChoice.later(SPLASH_STUCK_MS);
  byChoice.win.choose("quit");
  await flush();
  assert.equal(byChoice.lost, null);
});

test("a splash page that crashes before it ever loaded is reloaded, then lost, like any crash", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const win = stuckWindow();
  const logs = [];
  const splash = splashController({ createWindow: () => win, logError: () => {}, log: (l) => logs.push(l) });
  let lost = null;
  splash.lost().catch((err) => (lost = err));
  splash.show();
  for (let i = 0; i <= SPLASH_RELOADS; i++) win.webContents.emit("render-process-gone", {}, { reason: "crashed" });
  await flush();
  assert.equal(win.calls.filter((c) => c === "reload").length, SPLASH_RELOADS);
  assert.match(String(lost), /crashed/);
  splash.suppress();
});

test("however many ways a splash loses its page, the loss is reported once", async (t) => {
  const made = await watchedLoss(t);
  made.win.webContents.emit("did-fail-load", {}, -6, "ERR_FILE_NOT_FOUND", "file:///x", true);
  made.win.webContents.emit("did-navigate", {}, "file:///elsewhere");
  for (let i = 0; i <= SPLASH_RELOADS; i++) made.win.webContents.emit("render-process-gone", {}, { reason: "crashed" });
  made.win.close();
  await flush();
  assert.equal(made.losses(), 1);
  assert.match(String(made.lost), /could not load/, "the first cause is the one reported");
});

test("a splash that is never lost never says so, and crashes past the boot do not count", async (t) => {
  const { splash, win } = await stuckSplash(t);
  let lost = null;
  splash.lost().catch((err) => (lost = err));
  splash.close();
  for (let i = 0; i < 5; i++) win.webContents.emit("render-process-gone", {}, { reason: "crashed" });
  await flush();
  assert.equal(lost, null);
  // A controller that never opened a splash is never lost either, and
  // asking does not leave an unhandled rejection behind.
  const none = splashController({ createWindow: () => null, logError: () => {} });
  let noneLost = null;
  none.lost().catch((err) => (noneLost = err));
  await flush();
  assert.equal(noneLost, null);
});

test("a splash page that exited cleanly is not reloaded; one the system killed is", async (t) => {
  // On Linux the kernel's out-of-memory killer is reported as "killed":
  // that is the one-off crash the reload is for.
  for (const [reason, reloaded] of [["clean-exit", false], ["killed", true], ["oom", true], ["crashed", true]]) {
    const { splash, win } = await stuckSplash(t);
    win.webContents.emit("render-process-gone", {}, { reason });
    assert.equal(win.calls.includes("reload"), reloaded, reason);
    splash.close();
    t.mock.timers.reset();
  }
});

test("a first load, or a page that crashed before the motion preference was known, is not told to move", async (t) => {
  const painted = await stuckSplash(t);
  const adds = (w) => w.calls.filter((c) => c.startsWith("js:") && c.includes("classList.add(")).length;
  const before = adds(painted.win);
  painted.win.webContents.emit("did-finish-load");
  await flush();
  assert.equal(adds(painted.win), before, "the first load");
  painted.splash.close();
  t.mock.timers.reset();
  // No preference yet: nothing may start the motion.
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const win = stuckWindow();
  const splash = splashController({ createWindow: () => win, logError: () => {} });
  splash.show();
  win.emit("ready-to-show");
  win.webContents.emit("render-process-gone", {}, { reason: "crashed" });
  win.webContents.emit("did-finish-load");
  await flush();
  assert.ok(win.calls.includes("reload"));
  assert.equal(adds(win), 0);
  splash.close();
});

test("a reloaded splash page is told to move again, or to rest, as it was", async (t) => {
  for (const resting of [false, true]) {
    const { splash, win } = await stuckSplash(t, { resting });
    const starts = () => win.calls.filter((c) => c.startsWith("js:") && c.includes("classList.add(")).length;
    const before = starts();
    win.webContents.emit("render-process-gone", {}, { reason: "crashed" });
    win.webContents.emit("did-finish-load");
    await flush();
    const told = win.calls.filter((c) => c.startsWith("js:") && c.includes("classList.add(")).at(-1);
    assert.equal(starts(), before + 1, String(resting));
    assert.match(told, resting ? /add\('reduced'\)/ : /add\('shown'\)/);
    // Only the start: not a second paint.
    assert.doesNotMatch(told, /requestAnimationFrame/);
    splash.close();
    t.mock.timers.reset();
  }
});

test("a splash page that crashes once the boot is past the splash is left alone", async (t) => {
  const { splash, win } = await stuckSplash(t);
  splash.suppress();
  win.webContents.emit("render-process-gone", {}, { reason: "crashed" });
  assert.ok(!win.calls.includes("reload"));
});

test("a choice that arrives after the person closed the splash says so in the log", async (t) => {
  const { win, logs, later } = await stuckSplash(t);
  await later(SPLASH_STUCK_MS);
  const pending = win.choose.bind(win);
  win.close();
  pending("quit");
  await flush();
  assert.ok(logs.some((l) => l.includes("Quit came after the splash had closed")), JSON.stringify(logs));
});

// The reveal's whole path, as main.js wires it: the app-ready wait has
// no bound while the splash stands in, so a page that never says it
// rendered is never revealed.
test("a page that never says it rendered is never revealed: the splash stays, stuck, until a choice", async (t) => {
  const { splash, win, later, dismissed } = await stuckSplash(t);
  await splash.progress("backend");
  await splash.progress("flags");
  const ipc = appIpc();
  const contents = { id: 7 };
  assert.equal(appReadyBound(splash), null);
  const waiter = appReadyWaiter(contents, ipc, { timeoutMs: appReadyBound(splash) });
  let open = false;
  revealGate(splash, waiter).then(() => (open = true));
  await later(120_000);
  assert.equal(open, false, "nothing is revealed");
  assert.ok(!win.calls.includes("close") && !win.calls.includes("hide"));
  assert.equal(win.asked(), 1);
  assert.equal(
    win.calls.filter((c) => c.includes("splashProgress.set")).at(-1),
    "js:window.splashProgress && window.splashProgress.set(2)",
    "the page's stretch stays pending",
  );
  win.choose("quit");
  await flush();
  assert.equal(dismissed(), 1);
  // A word that comes now opens the gate, but the reveal is cancelled
  // (revealMaximized's `cancelled`, main.js).
  ipc.emit(APP_READY_CHANNEL, { sender: contents });
  await later(1_000);
  assert.equal(splash.dismissed(), true);
  const revealed = [];
  const { revealMaximized } = require("./splash.cjs");
  revealMaximized(
    { isDestroyed: () => false, maximize: () => revealed.push("maximize"), show: () => revealed.push("show") },
    { cancelled: () => splash.dismissed(), onMapped: () => {}, afterShow: () => {} },
  );
  assert.deepEqual(revealed, []);
});

// Every order of paint, render and click, on the clock.
test("property: the stuck state comes exactly when the page is 15 s late, and the first of render and choice decides", async () => {
  await fc.assert(
    fc.asyncProperty(
      fc.record({
        paintAt: fc.option(fc.integer({ min: 0, max: 2_500 }), { nil: undefined }),
        landAt: fc.option(fc.integer({ min: 0, max: 40_000 }), { nil: undefined }),
        clickAt: fc.option(fc.integer({ min: 0, max: 40_000 }), { nil: undefined }),
        choice: fc.constantFrom("restart", "quit"),
      }),
      async ({ paintAt, landAt, clickAt, choice }) => {
        // The controller's clock is the mocked Date, so it reads the
        // same instant the mocked timers fire at.
        mock.timers.enable({ apis: ["setTimeout", "Date"], now: 0 });
        try {
          let now = 0;
          const win = stuckWindow();
          let quits = 0;
          let restarts = 0;
          const splash = splashController({
            createWindow: () => win,
            logError: () => {},
            clock: () => Date.now(),
            onDismissed: () => (quits += 1),
            onRestart: () => (restarts += 1),
          });
          splash.show();
          win.emit("ready-to-show");
          splash.motionPreference(false);
          await flush();
          let stuckAt;
          let clicked = false;
          const events = [];
          if (paintAt !== undefined) events.push([paintAt, "paint"]);
          if (landAt !== undefined) events.push([landAt, "land"]);
          if (clickAt !== undefined) events.push([clickAt, "click"]);
          // The harness also stops exactly at the instants the timers are
          // due: a timer set inside a firing callback is scheduled from
          // the end of the mocked tick, not from the instant it fired.
          const armedAt = Math.min(paintAt ?? Infinity, SPLASH_SHOW_WAIT_MS);
          const dueAt = armedAt + SPLASH_STUCK_MS;
          events.push([SPLASH_SHOW_WAIT_MS, "tick"], [dueAt, "tick"]);
          events.sort((a, b) => a[0] - b[0]);
          const advance = async (to) => {
            while (now < to) {
              const step = Math.min(250, to - now);
              now += step;
              mock.timers.tick(step);
              await flush();
              if (stuckAt === undefined && win.asked() > 0) stuckAt = now;
            }
          };
          for (const [at, kind] of events) {
            await advance(at);
            if (kind === "paint") win.paint(false);
            if (kind === "land") splash.pageReady(true);
            if (kind === "click") clicked = win.choose(choice);
            await flush();
          }
          await advance(45_000);
          // Timers due at an instant fire before what happens at it.
          const shouldStick = landAt === undefined || landAt >= dueAt;
          assert.equal(win.asked() > 0, shouldStick, "stuck exactly when the page is late");
          if (shouldStick) {
            // Within a step of when it was due.
            assert.ok(stuckAt >= dueAt && stuckAt < dueAt + 250, `stuck at ${stuckAt}, due ${dueAt}`);
          }
          const choiceCounts =
            shouldStick && clickAt !== undefined && clickAt >= stuckAt && (landAt === undefined || clickAt < landAt);
          assert.equal(clicked, choiceCounts, "a click can only be made on the stuck state, before the page lands");
          assert.equal(splash.dismissed(), choiceCounts);
          assert.equal(quits, choiceCounts ? 1 : 0);
          assert.equal(restarts, choiceCounts && choice === "restart" ? 1 : 0);
          splash.close();
        } finally {
          mock.timers.reset();
        }
      },
    ),
    { numRuns: 60 },
  );
});

// ── The page side (splash/progress.js) ──────────────────────────────

const splashDir = path.dirname(SPLASH_PAGE);
const read = (name) => fs.readFileSync(path.join(splashDir, name), "utf-8");

function classes() {
  const set = new Set();
  return {
    add: (c) => set.add(c),
    remove: (c) => set.delete(c),
    contains: (c) => set.has(c),
  };
}

/** progress.js and words.js on a page opened with `search`, with the
 *  elements the stuck state reaches. */
function stuckPage(search) {
  const segments = [0, 1, 2].map(() => ({ classList: classes() }));
  const status = { textContent: "", classList: classes() };
  const button = (act) => {
    const b = new EventEmitter();
    b.dataset = { act };
    b.textContent = "";
    b.addEventListener = (type, fn) => b.on(type, fn);
    b.click = () => b.emit("click");
    return b;
  };
  const restart = button("restart");
  const quit = button("quit");
  const actions = {
    hidden: true,
    inert: false,
    querySelectorAll: (sel) => (sel === "button[data-act]" ? [restart, quit] : []),
  };
  let top = 120;
  const title = {
    classList: classes(),
    style: { setProperty: (k, v) => (title[k] = v) },
    getBoundingClientRect: () => ({ top }),
    addEventListener: () => {},
  };
  const main = { attrs: {}, setAttribute: (k, v) => (main.attrs[k] = v) };
  const body = { classList: classes() };
  const html = { lang: "en" };
  const context = {
    location: { search },
    URLSearchParams,
    Number,
    Math,
    Promise,
    setTimeout: () => ({}),
    clearTimeout: () => {},
    document: {
      body,
      documentElement: html,
      querySelectorAll: (sel) => (sel === ".rail i" ? segments : []),
      querySelector: (sel) =>
        ({ ".status": status, ".actions": actions, ".title": title, main })[sel] || null,
    },
  };
  context.window = context;
  vm.runInNewContext(read("progress.js"), context);
  vm.runInNewContext(read("words.js"), context);
  // The stuck state's line and actions join the column, which the
  // stage re-centres: the title moves up.
  const enterLayout = () => (top = 100);
  return {
    progress: context.splashProgress,
    segments,
    status,
    actions,
    restart,
    quit,
    title,
    body,
    main,
    html,
    enterLayout,
  };
}

const stuckQuery =
  "?st=700&t1=a&t2=b&t3=c&sl=Taking%20longer%20than%20usual%20to%20start.&sr=Restart&sq=Quit&l=pt-BR&al=O%20ani-gui%20est%C3%A1%20iniciando";

test("the page enters the stuck state: its line, the late stretch, and the actions in its language", async () => {
  const page = stuckPage(stuckQuery);
  page.progress.set(2);
  const choice = page.progress.stuck();
  assert.ok(choice && typeof choice.then === "function");
  assert.equal(page.status.textContent, "Taking longer than usual to start.");
  assert.ok(page.status.classList.contains("on"));
  assert.ok(page.segments[2].classList.contains("late"));
  assert.ok(!page.segments[1].classList.contains("late"));
  assert.equal(page.actions.hidden, false);
  assert.equal(page.actions.inert, false);
  assert.equal(page.restart.textContent, "Restart");
  assert.equal(page.quit.textContent, "Quit");
  assert.ok(page.body.classList.contains("stuck"));
  page.restart.click();
  assert.equal(await choice, "restart");
  assert.equal(page.actions.inert, true, "one choice: the actions go inert");
});

test("Quit answers quit, and a second click changes nothing", async () => {
  const page = stuckPage(stuckQuery);
  const choice = page.progress.stuck();
  page.quit.click();
  page.restart.click();
  assert.equal(await choice, "quit");
});

test("asked again, the page offers its choices again", async () => {
  const page = stuckPage(stuckQuery);
  const first = page.progress.stuck();
  page.restart.click();
  await first;
  const second = page.progress.stuck();
  assert.equal(page.actions.inert, false);
  page.quit.click();
  assert.equal(await second, "quit");
});

test("the rail completing after the stuck state makes the actions inert and lights the late stretch", () => {
  const page = stuckPage(stuckQuery);
  page.progress.set(2);
  let settled = false;
  page.progress.stuck().then(() => (settled = true));
  page.progress.set(3);
  assert.equal(page.actions.inert, true);
  assert.ok(page.segments[2].classList.contains("lit"));
  assert.ok(!page.segments[2].classList.contains("late"));
  page.restart.click();
  return new Promise((r) => setImmediate(r)).then(() => assert.equal(settled, false));
});

test("a page already complete, or opened without the words, offers nothing", () => {
  const done = stuckPage(stuckQuery);
  done.progress.set(3);
  assert.equal(done.progress.stuck(), null);
  assert.equal(done.actions.hidden, true);
  const bare = stuckPage("?st=700");
  assert.equal(bare.progress.stuck(), null);
  assert.equal(bare.actions.hidden, true);
});

test("the pulse follows the pending stretch when a step lands during the stuck state", () => {
  const page = stuckPage(stuckQuery);
  page.progress.stuck();
  assert.ok(page.segments[0].classList.contains("late"), "the backend's stretch");
  page.progress.set(1);
  assert.ok(!page.segments[0].classList.contains("late"));
  assert.ok(page.segments[0].classList.contains("lit"));
  assert.ok(page.segments[1].classList.contains("late"), "now the flags' stretch");
  page.progress.set(2);
  assert.ok(!page.segments[1].classList.contains("late"));
  assert.ok(page.segments[2].classList.contains("late"));
});

test("the stuck line stays: no stall line takes over from it", () => {
  const page = stuckPage("?st=700&t1=a&t2=b&t3=c&tg=g&sl=S&sr=R&sq=Q");
  page.progress.stuck();
  page.progress.set(1);
  page.progress.set(2);
  assert.equal(page.status.textContent, "S");
});

test("the page speaks its language, and names itself in it", () => {
  const page = stuckPage(stuckQuery);
  assert.equal(page.html.lang, "pt-BR");
  assert.equal(page.main.attrs["aria-label"], "O ani-gui está iniciando");
  // Anything that is not a language tag is ignored.
  const odd = stuckPage("?l=%22%3E%3Cx&al=");
  assert.equal(odd.html.lang, "en");
  assert.equal(odd.main.attrs["aria-label"], undefined);
});

test("the actions are in the page, empty until the stuck state fills them, after the line", () => {
  const html = read("index.html");
  assert.match(
    html,
    /<div class="actions" hidden>\s*<button type="button" class="act act-restart" data-act="restart"><\/button>\s*<button type="button" class="act act-quit" data-act="quit"><\/button>\s*<\/div>/,
  );
  // Restart comes first, so Tab reaches it first, then Quit; nothing is
  // focused for the person: no autofocus, and no key handler (Escape
  // does nothing, Enter and Space are the buttons' own).
  assert.doesNotMatch(html, /autofocus|tabindex/);
  assert.doesNotMatch(read("progress.js"), /\.focus\(|keydown|keyup|keypress/);
  assert.match(html, /<script src="words\.js"><\/script>/);
  const css = read("splash.css");
  // The buttons take clicks inside the window's drag region.
  assert.match(css, /\.actions \{[^}]*-webkit-app-region: no-drag;/);
  assert.match(css, /\.act:focus-visible \{[^}]*outline: 2px solid/);
  // The late stretch pulses, and rests under reduced motion.
  assert.match(css, /\.rail i\.late::after \{[^}]*animation: late/);
  assert.match(css, /body\.reduced \.rail i\.late::after/);
  assert.doesNotMatch(css, /STUDY/);
});
