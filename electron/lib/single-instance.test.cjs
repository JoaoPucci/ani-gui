"use strict";

const test = require("node:test");
const assert = require("node:assert");

const { claimSingleInstance, revealWindow, windowKeeper } = require("./single-instance.cjs");

/** A stand-in for Electron's `app`: records the calls the module makes. */
function fakeApp({ lock }) {
  const calls = [];
  const handlers = {};
  return {
    calls,
    handlers,
    requestSingleInstanceLock() {
      calls.push("requestSingleInstanceLock");
      return lock;
    },
    quit() {
      calls.push("quit");
    },
    on(event, handler) {
      calls.push(`on:${event}`);
      handlers[event] = handler;
    },
  };
}

/** A stand-in for a BrowserWindow in a given state. */
function fakeWindow({ destroyed = false, minimized = false, visible = true } = {}) {
  const calls = [];
  return {
    calls,
    isDestroyed: () => destroyed,
    isMinimized: () => minimized,
    isVisible: () => visible,
    restore() {
      calls.push("restore");
    },
    show() {
      calls.push("show");
    },
    focus() {
      calls.push("focus");
    },
  };
}

test("the first instance takes the lock and listens for later launches", () => {
  const app = fakeApp({ lock: true });
  const claimed = claimSingleInstance(app, { summon: () => {} });
  assert.strictEqual(claimed, true);
  assert.deepStrictEqual(app.calls, ["requestSingleInstanceLock", "on:second-instance"]);
});

test("a later instance quits at once and registers nothing", () => {
  // It must not go on to boot: main.js stops at this answer, before
  // whenReady work and before the backend is spawned.
  const app = fakeApp({ lock: false });
  const claimed = claimSingleInstance(app, { summon: () => {} });
  assert.strictEqual(claimed, false);
  assert.deepStrictEqual(app.calls, ["requestSingleInstanceLock", "quit"]);
});

test("a later launch is answered by summon", () => {
  const app = fakeApp({ lock: true });
  let summoned = 0;
  claimSingleInstance(app, { summon: () => summoned++ });
  app.handlers["second-instance"]();
  assert.strictEqual(summoned, 1);
});

test("a later launch brings the running window forward", () => {
  const keeper = windowKeeper();
  const win = fakeWindow();
  keeper.shown(win);
  assert.strictEqual(keeper.summon(), "revealed");
  assert.deepStrictEqual(win.calls, ["focus"]);
});

test("the window is the one shown when the later launch arrives", () => {
  // The lock is taken before any window exists; the window a launch
  // reveals is the one current at the moment of the launch.
  const keeper = windowKeeper();
  const app = fakeApp({ lock: true });
  claimSingleInstance(app, { summon: () => keeper.summon() });
  const win = fakeWindow({ minimized: true });
  keeper.shown(win);
  app.handlers["second-instance"]();
  assert.deepStrictEqual(win.calls, ["restore", "focus"]);
});

test("a launch while the first is still booting finds no window", () => {
  assert.strictEqual(windowKeeper().summon(), "none");
});

test("a closed window is let go; another window's close is not its", () => {
  const keeper = windowKeeper();
  const win = fakeWindow();
  keeper.shown(win);
  keeper.closed(fakeWindow());
  assert.strictEqual(keeper.summon(), "revealed");
  keeper.closed(win);
  win.calls.length = 0;
  assert.strictEqual(keeper.summon(), "none");
  assert.deepStrictEqual(win.calls, []);
});

test("a failed boot has no window to bring forward", () => {
  const keeper = windowKeeper();
  const win = fakeWindow({ visible: false });
  keeper.shown(win);
  keeper.failed();
  assert.strictEqual(keeper.summon(), "none");
  assert.deepStrictEqual(win.calls, []);
});

test("a minimized window is restored before it is focused", () => {
  const win = fakeWindow({ minimized: true });
  assert.strictEqual(revealWindow(win), true);
  assert.deepStrictEqual(win.calls, ["restore", "focus"]);
});

test("a hidden window is shown before it is focused", () => {
  const win = fakeWindow({ visible: false });
  assert.strictEqual(revealWindow(win), true);
  assert.deepStrictEqual(win.calls, ["show", "focus"]);
});

test("no window yet — a launch while the first is still booting — is a no-op", () => {
  // The booting instance shows its window itself once it is ready.
  assert.strictEqual(revealWindow(null), false);
  assert.strictEqual(revealWindow(undefined), false);
});

test("a destroyed window is left alone", () => {
  const win = fakeWindow({ destroyed: true });
  assert.strictEqual(revealWindow(win), false);
  assert.deepStrictEqual(win.calls, []);
});

// After the boot: a launch that finds no window opens one. On macOS
// closing the last window leaves the app and its backend running, and
// the lock then sends every later launch here — so a keeper that only
// reveals would leave the user with no window from any launch.

const BACKEND = { apiBase: "http://127.0.0.1:4321", internalSecret: "s3cret" };

/** A keeper whose createWindow is recorded and settled by the test. */
function reopeningKeeper({ quitting = () => false } = {}) {
  const opened = [];
  const errors = [];
  let settle;
  const keeper = windowKeeper({
    createWindow: (backend) => {
      opened.push(backend);
      return new Promise((resolve, reject) => {
        settle = { resolve, reject };
      });
    },
    quitting,
    logError: (...args) => errors.push(args),
  });
  return { keeper, opened, errors, settle: () => settle };
}

test("after the boot, a launch with no window opens one against the boot's backend", () => {
  const { keeper, opened } = reopeningKeeper();
  const win = fakeWindow();
  keeper.shown(win);
  keeper.booted(BACKEND);
  keeper.closed(win);
  assert.strictEqual(keeper.summon(), "reopened");
  assert.deepStrictEqual(opened, [BACKEND]);
});

test("a window being reopened is not reopened again by the next launch", async () => {
  const { keeper, opened, settle } = reopeningKeeper();
  keeper.booted(BACKEND);
  assert.strictEqual(keeper.summon(), "reopened");
  assert.strictEqual(keeper.summon(), "none");
  assert.strictEqual(opened.length, 1);
  const win = fakeWindow();
  keeper.shown(win);
  settle().resolve();
  await Promise.resolve();
  assert.strictEqual(keeper.summon(), "revealed");
  assert.strictEqual(opened.length, 1);
});

test("a reopen that fails is logged, and the next launch tries again", async () => {
  const { keeper, opened, errors, settle } = reopeningKeeper();
  keeper.booted(BACKEND);
  assert.strictEqual(keeper.summon(), "reopened");
  settle().reject(new Error("renderer gone"));
  await new Promise((resolve) => setImmediate(resolve));
  assert.strictEqual(errors.length, 1);
  assert.strictEqual(keeper.summon(), "reopened");
  assert.strictEqual(opened.length, 2);
});

test("a window that is up is brought forward, not reopened", () => {
  const { keeper, opened } = reopeningKeeper();
  const win = fakeWindow();
  keeper.shown(win);
  keeper.booted(BACKEND);
  assert.strictEqual(keeper.summon(), "revealed");
  assert.deepStrictEqual(opened, []);
});

test("a launch during the boot opens nothing", () => {
  // The boot's window may be on screen already — shown before it is
  // ready — but the boot has not opened it: that is still the boot's.
  const { keeper, opened } = reopeningKeeper();
  assert.strictEqual(keeper.summon(), "none");
  keeper.shown(fakeWindow({ visible: false }));
  keeper.failed();
  assert.strictEqual(keeper.summon(), "none");
  assert.deepStrictEqual(opened, []);
});

test("a launch after a failed boot opens nothing", () => {
  const { keeper, opened } = reopeningKeeper();
  keeper.booted(BACKEND);
  keeper.failed();
  assert.strictEqual(keeper.summon(), "none");
  assert.deepStrictEqual(opened, []);
});

test("a launch while the app is quitting opens nothing", () => {
  // The quit is stopping the backend the window would load against.
  const { keeper, opened } = reopeningKeeper({ quitting: () => true });
  keeper.booted(BACKEND);
  assert.strictEqual(keeper.summon(), "none");
  assert.deepStrictEqual(opened, []);
});
