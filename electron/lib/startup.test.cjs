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

const { EventEmitter } = require("node:events");

const {
  awaitFirstShow,
  bootApp,
  firstShowTimeoutMs,
  loadFirstPage,
} = require("./startup.cjs");

// Electron's loadURL rejections carry the net error name as `code`.
const loadError = (code, errno, url) =>
  Object.assign(new Error(`${code} (${errno}) loading '${url}'`), { code, errno, url });
const aborted = () => loadError("ERR_ABORTED", -3, "about:blank");

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

for (const [code, errno] of [
  ["ERR_FILE_NOT_FOUND", -6],
  ["ERR_CONNECTION_REFUSED", -102],
  // A renderer that crashes during the first load rejects with this.
  ["ERR_FAILED", -2],
]) {
  test(`a first page that fails for any other reason is a startup failure (${code})`, async () => {
    const win = { loadURL: async () => Promise.reject(loadError(code, errno, "app://localhost/")) };
    await assert.rejects(loadFirstPage(win, "app://localhost/", () => {}), (err) => err.code === code);
  });
}

// A fake BrowserWindow carrying only what the first-show guard reads.
const fakeWindow = () => {
  const win = new EventEmitter();
  win.webContents = new EventEmitter();
  return win;
};

test("a window that reaches ready-to-show passes the first-show guard", async () => {
  const win = fakeWindow();
  const shown = awaitFirstShow(win, { timeoutMs: 1_000 });
  win.emit("ready-to-show");
  await shown;
});

test("a renderer that dies before the first show is a startup failure", async () => {
  const win = fakeWindow();
  const shown = awaitFirstShow(win, { timeoutMs: 1_000 });
  win.webContents.emit("render-process-gone", {}, { reason: "crashed", exitCode: 139 });
  await assert.rejects(shown, /crashed/);
});

test("a window that never reaches ready-to-show is a startup failure", async () => {
  const win = fakeWindow();
  await assert.rejects(awaitFirstShow(win, { timeoutMs: 20 }), /ready-to-show/);
});

// A dev launch loads its first page from Vite, and a cold Vite that is
// re-optimizing its dependencies can take longer over the first paint
// than any deadline fit for the packaged bundle. The developer is at
// the terminal and can see it working; ending `pnpm dev` under them is
// the launcher getting in the way.
test("a packaged launch has a first-show deadline and a dev launch has none", () => {
  assert.equal(firstShowTimeoutMs({ isDev: false }), 15_000);
  assert.equal(firstShowTimeoutMs({ isDev: true }), null);
});

test("a window with no deadline waits as long as the first paint takes", async () => {
  const win = fakeWindow();
  const shown = awaitFirstShow(win, { timeoutMs: null });
  const state = await Promise.race([
    shown.then(
      () => "shown",
      () => "failed",
    ),
    new Promise((resolve) => setTimeout(() => resolve("waiting"), 100)),
  ]);
  assert.equal(state, "waiting");
  win.emit("ready-to-show");
  await shown;
});

test("a window with no deadline still fails when its renderer dies", async () => {
  const win = fakeWindow();
  const shown = awaitFirstShow(win, { timeoutMs: null });
  win.webContents.emit("render-process-gone", {}, { reason: "crashed", exitCode: 139 });
  await assert.rejects(shown, /crashed/);
});

test("a renderer that dies after the first show is not the guard's business", async () => {
  const win = fakeWindow();
  const shown = awaitFirstShow(win, { timeoutMs: 1_000 });
  win.emit("ready-to-show");
  await shown;
  win.webContents.emit("render-process-gone", {}, { reason: "crashed", exitCode: 139 });
  assert.equal(win.webContents.listenerCount("render-process-gone"), 0);
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

test("a failed boot stops the backend, says why, and only then exits", async () => {
  const events = [];
  const failure = new Error("no display");
  await bootApp({
    spawnBackend: async () => ({ apiBase: "http://127.0.0.1:1", internalSecret: "s" }),
    createWindow: async () => {
      throw failure;
    },
    stopBackend: () => events.push("stop"),
    // Slow on purpose: the exit has to wait for the report, which is
    // a dialog somebody may be reading.
    reportFailure: async (err) => {
      await new Promise((resolve) => setTimeout(resolve, 20));
      events.push(err === failure ? "report" : "report of something else");
    },
    exit: (code) => events.push(`exit:${code}`),
    logError: () => {},
  });
  assert.deepEqual(events, ["stop", "report", "exit:1"]);
});

test("a report that fails does not keep a failed boot from exiting", async () => {
  const events = [];
  await bootApp({
    spawnBackend: async () => {
      throw new Error("backend exited before handshake");
    },
    createWindow: async () => {},
    stopBackend: () => events.push("stop"),
    reportFailure: async () => {
      throw new Error("no display to report on");
    },
    exit: (code) => events.push(`exit:${code}`),
    logError: () => {},
  });
  assert.deepEqual(events, ["stop", "exit:1"]);
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
