// The main process's boot: spawn the backend, open the window, load
// its first page.
//
// Electron rejects `loadURL` with ERR_ABORTED whenever another
// main-frame navigation starts before the first page finishes
// loading. The boot used to treat that rejection as a startup
// failure and call `app.exit(1)` — closing the window a moment after
// it appeared — and `app.exit` skips `before-quit`, the only place
// the backend tree was stopped, so the detached backend outlived the
// app. That is the e2e cold-launch flake: the suite's about:blank
// bounce landed before a slow first load finished, the app quit
// underneath it, and while the orphaned backend ran, Playwright's
// close of the exited app did not return, so it waited out the test.

const test = require("node:test");
const assert = require("node:assert/strict");

const { bootApp, loadFirstPage } = require("./startup.cjs");

const aborted = () => new Error("ERR_ABORTED (-3) loading 'about:blank'");

test("a first page superseded by another navigation is not a startup failure", async () => {
  const logged = [];
  const win = { loadURL: async () => Promise.reject(aborted()) };
  await loadFirstPage(win, "app://localhost/", (...args) => logged.push(args));
  assert.equal(logged.length, 1);
});

test("a first page that loads is loaded once and logs nothing", async () => {
  const urls = [];
  const logged = [];
  const win = { loadURL: async (url) => urls.push(url) };
  await loadFirstPage(win, "app://localhost/", (...args) => logged.push(args));
  assert.deepEqual(urls, ["app://localhost/"]);
  assert.equal(logged.length, 0);
});

test("a window that cannot be created stops the backend before the app exits", async () => {
  const events = [];
  await bootApp({
    spawnBackend: async () => {
      events.push("spawn");
      return { apiBase: "http://127.0.0.1:1", internalSecret: "s" };
    },
    createWindow: async () => {
      throw new Error("no display");
    },
    stopBackend: () => events.push("stop"),
    exit: (code) => events.push(`exit:${code}`),
    logError: () => {},
  });
  assert.deepEqual(events, ["spawn", "stop", "exit:1"]);
});

test("a backend that never starts still exits with a failure", async () => {
  const events = [];
  await bootApp({
    spawnBackend: async () => {
      throw new Error("backend exited before handshake");
    },
    createWindow: async () => events.push("window"),
    stopBackend: () => events.push("stop"),
    exit: (code) => events.push(`exit:${code}`),
    logError: () => {},
  });
  assert.deepEqual(events, ["stop", "exit:1"]);
});

test("a clean boot opens the window with the backend's handshake and never exits", async () => {
  const events = [];
  const backend = { apiBase: "http://127.0.0.1:1", internalSecret: "s" };
  await bootApp({
    spawnBackend: async () => backend,
    createWindow: async (got) => events.push(got === backend ? "window" : "wrong"),
    stopBackend: () => events.push("stop"),
    exit: (code) => events.push(`exit:${code}`),
    logError: () => {},
  });
  assert.deepEqual(events, ["window"]);
});
