// The app's window paints the app's dark background before its page
// does, and the reveal waits for the renderer to say its first page has
// rendered — a slow first render (a cold Vite compile, a cold disk)
// showed a white frame between the splash and Home.

const test = require("node:test");
const assert = require("node:assert/strict");
const fs = require("node:fs");
const path = require("node:path");
const { EventEmitter } = require("node:events");

const {
  APP_BACKGROUND,
  APP_READY_CHANNEL,
  APP_READY_WAIT_MS,
  abortWhenLost,
  appReadyBound,
  appReadyWaiter,
  appWindowOptions,
  lastAppReady,
  revealGate,
} = require("./app-window.cjs");
const { splashWindowOptions } = require("./splash.cjs");

test("the app's window is made with the app's own dark background, the splash's too", () => {
  const tokens = fs.readFileSync(
    path.join(__dirname, "..", "..", "frontend", "src", "lib", "design", "tokens.css"),
    "utf-8",
  );
  const ink = /--ink-000:\s*(#[0-9a-f]{6})/i.exec(tokens);
  assert.ok(ink, "tokens.css defines --ink-000");
  assert.equal(APP_BACKGROUND.toLowerCase(), ink[1].toLowerCase());
  // The splash page paints its own copy of the same ink over its window.
  const splashCss = fs.readFileSync(path.join(__dirname, "..", "splash", "splash.css"), "utf-8");
  const splashInk = /--ink-000:\s*(#[0-9a-f]{6})/i.exec(splashCss);
  assert.ok(splashInk, "splash.css defines --ink-000");
  assert.equal(splashInk[1].toLowerCase(), APP_BACKGROUND.toLowerCase());
  const o = appWindowOptions({ width: 1920, height: 1048, preload: "p", additionalArguments: [] });
  assert.equal(o.backgroundColor, APP_BACKGROUND);
  assert.equal(splashWindowOptions().backgroundColor, APP_BACKGROUND);
  assert.equal(o.show, false);
  assert.equal(o.frame, false);
  assert.deepEqual([o.width, o.height], [1920, 1048]);
  assert.equal(o.webPreferences.preload, "p");
  assert.equal(o.webPreferences.contextIsolation, true);
});

// A Linux window's icon is the one it sets itself: an AppImage run
// without desktop integration has no installed .desktop file for the
// desktop to match its app id against, and KDE's Wayland task bar
// showed a generic "W" for it. The icon has to be a file the packaged
// app carries: electron-builder's `build.files` is an explicit
// allowlist, and nothing under build-resources is packaged unless it
// is named there.
const ICON = path.join(__dirname, "..", "build-resources", "icon.png");

test("on Linux the app's window and the splash carry the app's icon, from a file the package ships", () => {
  const app = appWindowOptions({ width: 1, height: 1, preload: "p", additionalArguments: [], platform: "linux" });
  assert.equal(app.icon, ICON);
  assert.equal(splashWindowOptions({ platform: "linux" }).icon, ICON);
  const pkg = JSON.parse(fs.readFileSync(path.join(__dirname, "..", "package.json"), "utf-8"));
  assert.ok(pkg.build.files.includes("build-resources/icon.png"), "electron-builder's files allowlist carries it");
  // A PNG, and square: Chromium hands a Wayland compositor square icons
  // only (xdg-toplevel-icon-v1) and drops any other.
  const png = fs.readFileSync(ICON);
  assert.equal(png.subarray(1, 4).toString("latin1"), "PNG");
  assert.equal(png.readUInt32BE(16), png.readUInt32BE(20));
});

test("on Windows and macOS the windows set no icon of their own: the executable's or the bundle's stands", () => {
  for (const platform of ["win32", "darwin"]) {
    const app = appWindowOptions({ width: 1, height: 1, preload: "p", additionalArguments: [], platform });
    assert.equal(app.icon, undefined, platform);
    assert.equal(splashWindowOptions({ platform }).icon, undefined, platform);
  }
});

let ids = 0;
const sender = () => ({ id: (ids += 1) });

test("the reveal waits for the renderer's word that its first page has rendered", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const ipc = new EventEmitter();
  const contents = sender();
  const waiter = appReadyWaiter(contents, ipc);
  let ready = null;
  waiter.ready.then((v) => (ready = v));
  await Promise.resolve();
  assert.equal(ready, null);
  ipc.emit(APP_READY_CHANNEL, { sender: sender() }); // another window's
  await Promise.resolve();
  assert.equal(ready, null);
  // The same window's renderer, through another wrapper of it.
  ipc.emit(APP_READY_CHANNEL, { sender: { id: contents.id } });
  for (let i = 0; i < 3; i++) await Promise.resolve();
  assert.equal(ready, true, "the word from the window's own renderer counts");
  assert.equal(ipc.listenerCount(APP_READY_CHANNEL), 0, "it stops listening");
  assert.deepEqual(lastAppReady(), { rendered: true, gaveUp: false, beforeReveal: true });
});

test("a renderer that never says so holds the reveal no longer than the bound", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  // A safety net for a page that never says it rendered, not the time a
  // render is expected to take: a cold Vite start on the maintainer's
  // machine had the first page render past the 4 s this used to be, and
  // the window showed black after the splash.
  assert.equal(APP_READY_WAIT_MS, 15_000);
  const ipc = new EventEmitter();
  const waiter = appReadyWaiter(sender(), ipc);
  let ready = null;
  waiter.ready.then((v) => (ready = v));
  t.mock.timers.tick(APP_READY_WAIT_MS - 1);
  await Promise.resolve();
  assert.equal(ready, null);
  t.mock.timers.tick(1);
  assert.equal(await waiter.ready, false);
  assert.equal(ipc.listenerCount(APP_READY_CHANNEL), 0);
  assert.deepEqual(lastAppReady(), { rendered: false, gaveUp: true, beforeReveal: false });
});

test("a word that comes late, inside the bound, still counts as rendered", async (t) => {
  // A cold dev start: Vite compiles the first page for many seconds.
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const ipc = new EventEmitter();
  const contents = sender();
  const waiter = appReadyWaiter(contents, ipc);
  let ready = null;
  waiter.ready.then((v) => (ready = v));
  t.mock.timers.tick(APP_READY_WAIT_MS - 1);
  await Promise.resolve();
  assert.equal(ready, null, "still waiting one millisecond before the bound");
  ipc.emit(APP_READY_CHANNEL, { sender: contents });
  assert.equal(await waiter.ready, true);
  t.mock.timers.tick(1);
  assert.deepEqual(lastAppReady(), { rendered: true, gaveUp: false, beforeReveal: true });
});

test("a word that comes after the wait gave up changes nothing", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const ipc = new EventEmitter();
  const contents = sender();
  const waiter = appReadyWaiter(contents, ipc);
  t.mock.timers.tick(APP_READY_WAIT_MS);
  assert.equal(await waiter.ready, false);
  ipc.emit(APP_READY_CHANNEL, { sender: contents });
  assert.equal(await waiter.ready, false);
  assert.deepEqual(lastAppReady(), { rendered: false, gaveUp: true, beforeReveal: false });
});

test("the word can come before the wait is looked at, and still counts", async () => {
  const ipc = new EventEmitter();
  const contents = sender();
  const waiter = appReadyWaiter(contents, ipc);
  ipc.emit(APP_READY_CHANNEL, { sender: contents });
  assert.equal(await waiter.ready, true);
});

test("the record says whether the word came while the window was still hidden", async () => {
  const ipc = new EventEmitter();
  let shown = true;
  const late = sender();
  const waiter = appReadyWaiter(late, ipc, { isShown: () => shown, timeoutMs: 50 });
  ipc.emit(APP_READY_CHANNEL, { sender: late });
  await waiter.ready;
  assert.equal(lastAppReady().beforeReveal, false, "the window was already up");
  shown = false;
  const early = sender();
  const again = appReadyWaiter(early, ipc, { isShown: () => shown, timeoutMs: 50 });
  ipc.emit(APP_READY_CHANNEL, { sender: early });
  await again.ready;
  assert.equal(lastAppReady().beforeReveal, true, "the window was still hidden");
});

test("the reveal waits for the renderer's word even once the splash is done", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let splashDone;
  const splashShown = new Promise((resolve) => (splashDone = resolve));
  const ipc = new EventEmitter();
  const contents = sender();
  const waiter = appReadyWaiter(contents, ipc);
  let open = false;
  revealGate(gateSplash(splashShown), waiter).then(() => (open = true));
  splashDone();
  for (let i = 0; i < 5; i++) await Promise.resolve();
  assert.equal(open, false, "the splash is done but the page has not rendered");
  ipc.emit(APP_READY_CHANNEL, { sender: contents });
  for (let i = 0; i < 5; i++) await Promise.resolve();
  assert.equal(open, true);
});

test("the reveal waits for the splash even once the page has rendered", async () => {
  let splashDone;
  const splashShown = new Promise((resolve) => (splashDone = resolve));
  const ipc = new EventEmitter();
  const contents = sender();
  const waiter = appReadyWaiter(contents, ipc, { timeoutMs: 50 });
  ipc.emit(APP_READY_CHANNEL, { sender: contents });
  let open = false;
  revealGate(gateSplash(splashShown), waiter).then(() => (open = true));
  for (let i = 0; i < 5; i++) await Promise.resolve();
  assert.equal(open, false);
  splashDone();
  for (let i = 0; i < 5; i++) await Promise.resolve();
  assert.equal(open, true);
});

/** A splash for the gate: done when `shown` settles, recording what it
 *  is told about the page and holding that for `hold`. */
function gateSplash(shown, hold = Promise.resolve()) {
  const told = [];
  return { told, whenShown: () => shown, pageReady: (rendered) => (told.push(rendered), hold) };
}

test("a page that never says it rendered still has the splash complete its rail, and the reveal waits for it", async () => {
  // The wait for the render gives up (here at once); the window is then
  // revealed anyway, but only after the splash has lit its last stretch
  // and held it.
  const ipc = new EventEmitter();
  const contents = sender();
  const waiter = appReadyWaiter(contents, ipc, { timeoutMs: 1 });
  let release;
  const splash = gateSplash(Promise.resolve(), new Promise((r) => (release = r)));
  let open = false;
  revealGate(splash, waiter).then(() => (open = true));
  await new Promise((r) => setTimeout(r, 10));
  assert.deepEqual(splash.told, [false], "told the page did not say it rendered");
  assert.equal(open, false, "held for the rail");
  release();
  for (let i = 0; i < 5; i++) await Promise.resolve();
  assert.equal(open, true);
});

test("a page that rendered has the splash complete its rail the same way", async () => {
  const ipc = new EventEmitter();
  const contents = sender();
  const waiter = appReadyWaiter(contents, ipc, { timeoutMs: 50 });
  let release;
  const splash = gateSplash(Promise.resolve(), new Promise((r) => (release = r)));
  let open = false;
  revealGate(splash, waiter).then(() => (open = true));
  ipc.emit(APP_READY_CHANNEL, { sender: contents });
  for (let i = 0; i < 5; i++) await Promise.resolve();
  assert.deepEqual(splash.told, [true]);
  assert.equal(open, false);
  release();
  for (let i = 0; i < 5; i++) await Promise.resolve();
  assert.equal(open, true);
});

// The splash's stuck state (lib/splash-stuck.cjs) stands in for the
// window while its page is late, so the wait for the page has no bound
// then: past one the window was revealed whether or not it had
// rendered, and sometimes it had not.
test("with no bound the wait never gives up, and the word still counts however late", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const ipc = new EventEmitter();
  const contents = sender();
  const waiter = appReadyWaiter(contents, ipc, { timeoutMs: null });
  let ready = null;
  waiter.ready.then((v) => (ready = v));
  t.mock.timers.tick(APP_READY_WAIT_MS * 100);
  for (let i = 0; i < 5; i++) await Promise.resolve();
  assert.equal(ready, null);
  ipc.emit(APP_READY_CHANNEL, { sender: contents });
  assert.equal(await waiter.ready, true);
});

test("a renderer that goes, or a window destroyed, ends the wait without a render and without completing the rail", async () => {
  for (const event of ["render-process-gone", "destroyed"]) {
    const ipc = new EventEmitter();
    const contents = Object.assign(new EventEmitter(), sender());
    const waiter = appReadyWaiter(contents, ipc, { timeoutMs: null });
    contents.emit(event, {}, { reason: "crashed" });
    assert.equal(await waiter.ready, false, event);
    assert.equal(waiter.gone(), true, event);
    assert.deepEqual(lastAppReady(), { rendered: false, gaveUp: false, beforeReveal: false }, event);
    // Nothing listens any more.
    assert.equal(ipc.listenerCount(APP_READY_CHANNEL), 0, event);
    assert.equal(contents.listenerCount("render-process-gone"), 0, event);
    assert.equal(contents.listenerCount("destroyed"), 0, event);
    // The rail is not completed for a page that is gone: the boot is
    // about to fail (lib/startup.cjs waitBeforeShow).
    const splash = gateSplash(Promise.resolve());
    await revealGate(splash, waiter);
    assert.deepEqual(splash.told, [], event);
  }
});

test("a word, or the bound, takes the renderer's listeners down too", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const ipc = new EventEmitter();
  const contents = Object.assign(new EventEmitter(), sender());
  const waiter = appReadyWaiter(contents, ipc, { timeoutMs: 50 });
  ipc.emit(APP_READY_CHANNEL, { sender: contents });
  assert.equal(await waiter.ready, true);
  assert.equal(waiter.gone(), false);
  assert.equal(contents.listenerCount("render-process-gone"), 0);
  assert.equal(contents.listenerCount("destroyed"), 0);
});

test("the wait is unbounded exactly while a splash stands in for the window", () => {
  assert.equal(appReadyBound({ standsIn: () => true }), null);
  // No splash (a later window, or one that could not be made): the
  // bound stays, so the window is not left hidden for good.
  assert.equal(appReadyBound({ standsIn: () => false }), APP_READY_WAIT_MS);
});

// A splash lost to crashes (lib/splash.cjs `lost`) no longer stands in
// for the window. The page gets back the bound it has without a splash:
// one that renders within it is revealed as usual, one that does not
// fails the boot (main.js passes this as openFirstPage's `abort`).
test("a lost splash gives the page the bound back: rendered within it, nothing fails", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let lose;
  const lost = new Promise((_r, reject) => (lose = reject));
  let render;
  const ready = new Promise((resolve) => (render = resolve));
  let failed = null;
  abortWhenLost(lost, ready).catch((err) => (failed = err));
  lose(new Error("lost"));
  for (let i = 0; i < 5; i++) await Promise.resolve();
  t.mock.timers.tick(APP_READY_WAIT_MS - 1);
  render(true);
  for (let i = 0; i < 5; i++) await Promise.resolve();
  t.mock.timers.tick(APP_READY_WAIT_MS * 2);
  for (let i = 0; i < 5; i++) await Promise.resolve();
  assert.equal(failed, null);
});

test("a lost splash and a page that does not render within the bound fail the boot", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let lose;
  const lost = new Promise((_r, reject) => (lose = reject));
  let failed = null;
  abortWhenLost(lost, new Promise(() => {})).catch((err) => (failed = err));
  lose(new Error("the launch splash's page crashed 3 times"));
  for (let i = 0; i < 5; i++) await Promise.resolve();
  t.mock.timers.tick(APP_READY_WAIT_MS - 1);
  for (let i = 0; i < 5; i++) await Promise.resolve();
  assert.equal(failed, null, "not before the bound");
  t.mock.timers.tick(1);
  for (let i = 0; i < 5; i++) await Promise.resolve();
  assert.match(String(failed), /crashed 3 times/);
});

test("a page that rendered before the splash was lost is never failed by it", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let lose;
  const lost = new Promise((_r, reject) => (lose = reject));
  let failed = null;
  abortWhenLost(lost, Promise.resolve(true)).catch((err) => (failed = err));
  for (let i = 0; i < 5; i++) await Promise.resolve();
  lose(new Error("lost"));
  t.mock.timers.tick(APP_READY_WAIT_MS * 2);
  for (let i = 0; i < 5; i++) await Promise.resolve();
  assert.equal(failed, null);
});

test("a splash never lost never fails anything", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let failed = null;
  abortWhenLost(new Promise(() => {}), new Promise(() => {})).catch((err) => (failed = err));
  t.mock.timers.tick(APP_READY_WAIT_MS * 10);
  for (let i = 0; i < 5; i++) await Promise.resolve();
  assert.equal(failed, null);
});

test("a lost splash leaves a page whose renderer went to the boot's own renderer-gone failure", async (t) => {
  // That guard (lib/startup.cjs waitBeforeShow) names the real cause;
  // the splash's loss would otherwise race it and be blamed.
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let lose;
  const lost = new Promise((_r, reject) => (lose = reject));
  let gone;
  const ready = new Promise((resolve) => (gone = resolve));
  let failed = null;
  abortWhenLost(lost, ready).catch((err) => (failed = err));
  lose(new Error("lost"));
  gone(false);
  for (let i = 0; i < 5; i++) await Promise.resolve();
  t.mock.timers.tick(APP_READY_WAIT_MS * 2);
  for (let i = 0; i < 5; i++) await Promise.resolve();
  assert.equal(failed, null);
});

test("a page that renders only after the bound has run out does not undo the failure", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  let lose;
  const lost = new Promise((_r, reject) => (lose = reject));
  let render;
  const ready = new Promise((resolve) => (render = resolve));
  let failed = null;
  abortWhenLost(lost, ready).catch((err) => (failed = err));
  lose(new Error("lost"));
  for (let i = 0; i < 5; i++) await Promise.resolve();
  t.mock.timers.tick(APP_READY_WAIT_MS);
  for (let i = 0; i < 5; i++) await Promise.resolve();
  render(true);
  for (let i = 0; i < 5; i++) await Promise.resolve();
  assert.match(String(failed), /lost/);
});

// Under native Wayland without GPU compositing (a VM, a blocklisted or
// failed GPU, --disable-gpu) a window that is not on screen gets no
// frames after its first, sometimes not even that: requestAnimationFrame
// never fires, so the page's rendered word never comes and the reveal
// waits for good. The page says it has mounted first, which needs no
// frame; when the rendered word does not follow within FRAME_WAKE_MS,
// the window is mapped (behind the splash) so its frames resume.
const { APP_MOUNTED_CHANNEL, FRAME_WAKE_MS, frameWake, lastWake, wakeHidden } = require("./app-window.cjs");

/** A settable stand-in for the waiter's `ready`. */
function readyLatch() {
  let settle;
  const ready = new Promise((resolve) => (settle = resolve));
  return { ready, settle };
}

test("a page that mounted but has not drawn its frames within the bound wakes its window, once", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const ipc = new EventEmitter();
  const contents = sender();
  const { ready } = readyLatch();
  let wakes = 0;
  const wake = frameWake(contents, ipc, { ready, wake: () => (wakes += 1) });
  assert.equal(APP_MOUNTED_CHANNEL, "ani-gui:app-mounted");
  ipc.emit(APP_MOUNTED_CHANNEL, { sender: contents });
  t.mock.timers.tick(FRAME_WAKE_MS - 1);
  assert.equal(wakes, 0);
  assert.equal(wake.woken(), false);
  t.mock.timers.tick(1);
  assert.equal(wakes, 1);
  assert.equal(wake.woken(), true);
  assert.deepEqual(lastWake(), { mounted: true, woken: true });
  ipc.emit(APP_MOUNTED_CHANNEL, { sender: contents });
  t.mock.timers.tick(FRAME_WAKE_MS * 2);
  assert.equal(wakes, 1, "one wake per window");
});

test("a page that draws its frames in time wakes nothing: the reveal is the hidden-window one", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const ipc = new EventEmitter();
  const contents = sender();
  const { ready, settle } = readyLatch();
  let wakes = 0;
  const wake = frameWake(contents, ipc, { ready, wake: () => (wakes += 1) });
  ipc.emit(APP_MOUNTED_CHANNEL, { sender: contents });
  settle(true);
  await ready;
  await Promise.resolve();
  t.mock.timers.tick(FRAME_WAKE_MS * 2);
  assert.equal(wakes, 0);
  assert.equal(wake.woken(), false);
  assert.deepEqual(lastWake(), { mounted: true, woken: false });
  assert.equal(ipc.listenerCount(APP_MOUNTED_CHANNEL), 0, "the listener goes with the wait");
});

test("only the window's own page wakes it, and a page that never mounts wakes nothing", (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const ipc = new EventEmitter();
  const contents = sender();
  const { ready } = readyLatch();
  let wakes = 0;
  frameWake(contents, ipc, { ready, wake: () => (wakes += 1) });
  ipc.emit(APP_MOUNTED_CHANNEL, { sender: sender() });
  ipc.emit(APP_MOUNTED_CHANNEL, {});
  t.mock.timers.tick(FRAME_WAKE_MS * 10);
  assert.equal(wakes, 0);
  assert.deepEqual(lastWake(), { mounted: false, woken: false });
});

test("a wake stopped (the reveal came, the window went) or a launch cancelled wakes nothing", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  for (const how of ["stop", "cancelled", "ready-false"]) {
    const ipc = new EventEmitter();
    const contents = sender();
    const { ready, settle } = readyLatch();
    let wakes = 0;
    const wake = frameWake(contents, ipc, {
      ready,
      wake: () => (wakes += 1),
      cancelled: () => how === "cancelled",
    });
    ipc.emit(APP_MOUNTED_CHANNEL, { sender: contents });
    if (how === "stop") wake.stop();
    if (how === "ready-false") settle(false);
    // The waiter's answer reaches the wake as a settled promise does.
    await Promise.resolve();
    await Promise.resolve();
    t.mock.timers.tick(FRAME_WAKE_MS);
    assert.equal(wakes, 0, how);
  }
});

/** A fake of the app's window for the wake: records what is done to it. */
function wakeableWindow({ visible = false, destroyed = false } = {}) {
  const win = new EventEmitter();
  win.calls = [];
  win.isDestroyed = () => destroyed;
  win.isVisible = () => visible;
  win.showInactive = () => {
    win.calls.push("showInactive");
    visible = true;
  };
  win.maximize = () => win.calls.push("maximize");
  return win;
}

test("a woken window is mapped without focus, then maximized, its granted size recorded from there", () => {
  // maximize() on a hidden window maps it and activates it; mapped
  // inactive first, it takes no focus from wherever the person is.
  const win = wakeableWindow();
  const detach = wakeHidden(win, {
    revealing: () => win.calls.push("revealing"),
    restack: () => {},
    platform: "linux",
  });
  assert.deepEqual(win.calls, ["revealing", "showInactive", "maximize"]);
  assert.equal(typeof detach, "function");
});

test("a woken window that takes the focus puts the splash back on top, until the reveal", () => {
  // Native Wayland ignores always-on-top: the compositor stacks the
  // woken window over the splash and, the app being the focused one,
  // focuses it. Mapping the splash again puts it back on top.
  const win = wakeableWindow();
  let restacks = 0;
  const detach = wakeHidden(win, { restack: () => (restacks += 1) });
  win.emit("focus");
  assert.equal(restacks, 1);
  detach();
  win.emit("focus");
  assert.equal(restacks, 1, "after the reveal, focus is the window's own");
});

test("a window already on screen, or gone, is not woken", () => {
  for (const options of [{ visible: true }, { destroyed: true }]) {
    const win = wakeableWindow(options);
    assert.equal(wakeHidden(win, { restack: () => {} }), null);
    assert.deepEqual(win.calls, []);
  }
});

test("only Linux maximizes the woken window: elsewhere maximizing a mapped window activates it, so the reveal does it", () => {
  // On Windows maximize() of a visible window is ShowWindow(SW_MAXIMIZE),
  // which activates it: the wake would take the focus. There the reveal
  // maximizes it (revealMaximized's up path), as it shows it anyway.
  // The granted-size recorder is armed where the maximize happens: at
  // the wake on Linux, at the reveal elsewhere, or a reveal more than its
  // settling time after the wake would record nothing.
  for (const [platform, expected] of [
    ["linux", ["revealing", "showInactive", "maximize"]],
    ["win32", ["showInactive"]],
    ["darwin", ["showInactive"]],
  ]) {
    const win = wakeableWindow();
    wakeHidden(win, { restack: () => {}, platform, revealing: () => win.calls.push("revealing") });
    assert.deepEqual(win.calls, expected, platform);
  }
});

test("a launch cancelled after the page mounted, before the bound, wakes nothing", (t) => {
  // The boot failing (main.js: the splash put away, the failure
  // reported) or the splash dismissed while the wake is pending.
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const ipc = new EventEmitter();
  const contents = sender();
  const { ready } = readyLatch();
  let cancelled = false;
  let wakes = 0;
  frameWake(contents, ipc, { ready, wake: () => (wakes += 1), cancelled: () => cancelled });
  ipc.emit(APP_MOUNTED_CHANNEL, { sender: contents });
  t.mock.timers.tick(FRAME_WAKE_MS - 1);
  cancelled = true;
  t.mock.timers.tick(1);
  assert.equal(wakes, 0);
  assert.equal(lastWake().woken, false);
});
