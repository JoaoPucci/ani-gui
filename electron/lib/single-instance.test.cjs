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
