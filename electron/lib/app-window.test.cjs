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
  appReadyWaiter,
  appWindowOptions,
  lastAppReady,
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
  const o = appWindowOptions({ width: 1920, height: 1048, icon: "i", preload: "p", additionalArguments: [] });
  assert.equal(o.backgroundColor, APP_BACKGROUND);
  assert.equal(splashWindowOptions().backgroundColor, APP_BACKGROUND);
  assert.equal(o.show, false);
  assert.equal(o.frame, false);
  assert.deepEqual([o.width, o.height], [1920, 1048]);
  assert.equal(o.webPreferences.preload, "p");
  assert.equal(o.webPreferences.contextIsolation, true);
});

const sender = () => ({});

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
  ipc.emit(APP_READY_CHANNEL, { sender: contents });
  assert.equal(await waiter.ready, true);
  assert.equal(ipc.listenerCount(APP_READY_CHANNEL), 0, "it stops listening");
  assert.deepEqual(lastAppReady(), { rendered: true, gaveUp: false });
});

test("a renderer that never says so holds the reveal no longer than the bound", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  assert.equal(APP_READY_WAIT_MS, 4000);
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
  assert.deepEqual(lastAppReady(), { rendered: false, gaveUp: true });
});

test("the word can come before the wait is looked at, and still counts", async () => {
  const ipc = new EventEmitter();
  const contents = sender();
  const waiter = appReadyWaiter(contents, ipc);
  ipc.emit(APP_READY_CHANNEL, { sender: contents });
  assert.equal(await waiter.ready, true);
});
