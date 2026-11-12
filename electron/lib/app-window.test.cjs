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
const { appDocumentFrom } = require("./app-document.cjs");
const { guardedIpc } = require("./ipc-guard.cjs");

const APP = appDocumentFrom({ isDev: false });
/** The frame a word from the app's own document comes from. */
const APP_FRAME = { url: "app://localhost/", parent: null };

/**
 * The waiter listens through the guarded IPC (lib/ipc-guard.cjs), the
 * way main.js hands it over. `emit` sends as the app's main frame unless
 * the event names another; `listenerCount` reads the ipcMain beneath.
 */
function guardedChannel(refusals = []) {
  const raw = new EventEmitter();
  const ipc = guardedIpc(raw, { app: APP, logError: (line) => refusals.push(line) });
  ipc.emit = (channel, event) => raw.emit(channel, { senderFrame: APP_FRAME, ...event });
  ipc.listenerCount = (channel) => raw.listenerCount(channel);
  return ipc;
}

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
  const o = appWindowOptions({ width: 1920, height: 1048, icon: "i", preload: "p", additionalArguments: [] });
  assert.equal(o.backgroundColor, APP_BACKGROUND);
  assert.equal(splashWindowOptions().backgroundColor, APP_BACKGROUND);
  assert.equal(o.show, false);
  assert.equal(o.frame, false);
  assert.deepEqual([o.width, o.height], [1920, 1048]);
  assert.equal(o.webPreferences.preload, "p");
  assert.equal(o.webPreferences.contextIsolation, true);
  // A file or link dropped on the window does not become a navigation.
  assert.equal(o.webPreferences.navigateOnDragDrop, false);
});

let ids = 0;
const sender = () => ({ id: (ids += 1) });

test("the reveal waits for the renderer's word that its first page has rendered", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const ipc = guardedChannel();
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

test("a word from a document that is not the app's does not count, even from the window's own contents", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const refusals = [];
  const ipc = guardedChannel(refusals);
  const contents = sender();
  const waiter = appReadyWaiter(contents, ipc);
  let ready = null;
  waiter.ready.then((v) => (ready = v));
  ipc.emit(APP_READY_CHANNEL, { sender: contents, senderFrame: { url: "file:///tmp/dropped.html", parent: null } });
  for (let i = 0; i < 3; i++) await Promise.resolve();
  assert.equal(ready, null, "the guard drops it before the waiter sees it");
  assert.ok(refusals.some((l) => l.includes(APP_READY_CHANNEL)), refusals.join("\n"));
  ipc.emit(APP_READY_CHANNEL, { sender: contents });
  for (let i = 0; i < 3; i++) await Promise.resolve();
  assert.equal(ready, true);
  assert.equal(ipc.listenerCount(APP_READY_CHANNEL), 0, "it stops listening");
});

test("a renderer that never says so holds the reveal no longer than the bound", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  // A safety net for a page that never says it rendered, not the time a
  // render is expected to take: a cold Vite start on the maintainer's
  // machine had the first page render past the 4 s this used to be, and
  // the window showed black after the splash.
  assert.equal(APP_READY_WAIT_MS, 15_000);
  const ipc = guardedChannel();
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
  const ipc = guardedChannel();
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
  const ipc = guardedChannel();
  const contents = sender();
  const waiter = appReadyWaiter(contents, ipc);
  t.mock.timers.tick(APP_READY_WAIT_MS);
  assert.equal(await waiter.ready, false);
  ipc.emit(APP_READY_CHANNEL, { sender: contents });
  assert.equal(await waiter.ready, false);
  assert.deepEqual(lastAppReady(), { rendered: false, gaveUp: true, beforeReveal: false });
});

test("the word can come before the wait is looked at, and still counts", async () => {
  const ipc = guardedChannel();
  const contents = sender();
  const waiter = appReadyWaiter(contents, ipc);
  ipc.emit(APP_READY_CHANNEL, { sender: contents });
  assert.equal(await waiter.ready, true);
});

test("the record says whether the word came while the window was still hidden", async () => {
  const ipc = guardedChannel();
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
  const ipc = guardedChannel();
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
  const ipc = guardedChannel();
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
  const ipc = guardedChannel();
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
  const ipc = guardedChannel();
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
  const ipc = guardedChannel();
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
    const ipc = guardedChannel();
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
  const ipc = guardedChannel();
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
