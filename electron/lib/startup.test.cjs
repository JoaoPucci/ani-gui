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
  openFirstPage,
} = require("./startup.cjs");

// Electron's loadURL rejections carry the net error name as `code`.
const loadError = (code, errno, url) =>
  Object.assign(new Error(`${code} (${errno}) loading '${url}'`), { code, errno, url });
const aborted = () => loadError("ERR_ABORTED", -3, "about:blank");

// A fake BrowserWindow whose `loadURL` plays `load`, which gets the
// window so it can emit the navigation events a real load does.
const pageWindow = (load) => {
  const win = fakeWindow();
  win.loadURL = (url) => load(win, url);
  return win;
};
/** A load that arrives: the page commits with `status`, then finishes. */
const answers = (status) => async (win, url) => {
  win.webContents.emit("did-navigate", {}, url, status, "");
};
const fails = (err) => async () => {
  throw err;
};

test("a first page superseded by another navigation is not a startup failure", async () => {
  const logged = [];
  const win = pageWindow(fails(aborted()));
  await loadFirstPage(win, "app://localhost/", (...args) => logged.push(args));
  assert.equal(logged.length, 1);
});

test("a first page that loads is loaded once and logs nothing", async () => {
  const urls = [];
  const logged = [];
  const win = pageWindow(async (w, url) => {
    urls.push(url);
    await answers(200)(w, url);
  });
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
    const win = pageWindow(fails(loadError(code, errno, "app://localhost/")));
    await assert.rejects(loadFirstPage(win, "app://localhost/", () => {}), (err) => err.code === code);
  });
}

// `loadURL` resolves for any page that arrives, an error page
// included. A bundle with its index.html missing is answered by the
// app's own protocol handler with a 404, and that 404 "loads": the
// window comes up frameless around the words "not found", with no
// titlebar and so no close button. An error status on the first page
// is the first page failing.
for (const status of [404, 403, 500]) {
  test(`a first page that arrives as an error page is a startup failure (${status})`, async () => {
    const win = pageWindow(answers(status));
    await assert.rejects(
      loadFirstPage(win, "app://localhost/", () => {}),
      new RegExp(`first page answered ${status}`),
    );
  });
}

test("a first page with no HTTP status to judge counts as loaded", async () => {
  // -1 is what a navigation outside HTTP reports.
  await loadFirstPage(pageWindow(answers(-1)), "file:///index.html", () => {});
  await loadFirstPage(pageWindow(async () => {}), "app://localhost/", () => {});
});

test("the first load leaves no listener on the window, however it ends", async () => {
  for (const load of [answers(200), answers(404), fails(aborted()), fails(loadError("ERR_FAILED", -2, "x"))]) {
    const win = pageWindow(load);
    await loadFirstPage(win, "app://localhost/", () => {}).catch(() => {});
    assert.equal(win.webContents.listenerCount("did-navigate"), 0);
  }
});

// A fake BrowserWindow carrying only what the boot reads, with
// Electron's behaviour once a window is gone: `closeFake` destroys it,
// after which reading `webContents` off the window throws ("Object
// has been destroyed"), as does `show()`, while a reference to the
// contents taken earlier keeps its emitter methods and answers
// `isDestroyed()`.
//
// The two go in either order, both measured: `close()` destroys the
// contents and then emits `closed`; `destroy()` emits `closed` while
// the contents still answer that they are alive.
const fakeWindow = () => {
  const win = new EventEmitter();
  const contents = new EventEmitter();
  let windowGone = false;
  let contentsGone = false;
  win.isDestroyed = () => windowGone;
  contents.isDestroyed = () => contentsGone;
  Object.defineProperty(win, "webContents", {
    get() {
      if (windowGone || contentsGone) throw new TypeError("Object has been destroyed");
      return contents;
    },
  });
  win.closeFake = () => {
    contentsGone = true;
    contents.emit("destroyed");
    windowGone = true;
    win.emit("closed");
  };
  win.destroyFake = () => {
    windowGone = true;
    win.emit("closed");
    contentsGone = true;
    contents.emit("destroyed");
  };
  // For assertions after the window is gone.
  win.contents = contents;
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
  assert.ok(Number.isFinite(firstShowTimeoutMs({ isDev: false })));
  assert.equal(firstShowTimeoutMs({ isDev: true }), null);
});

// The packaged deadline is for a window that will never paint, not for
// one that is slow to. The first paint needs a renderer started and
// the bundle read off the same disk that can make the backend's first
// start take 24 s (see lib/backend-handshake.cjs), and on a machine
// rendering in software it needs that too. A renderer that dies and a
// page that fails to load do not wait for the deadline; they fail the
// boot when they happen.

/** Lets the promise callbacks run; setImmediate is not mocked. */
const settled = () => new Promise((resolve) => setImmediate(resolve));

test("a packaged window a minute and a half from its first paint is slow, not failed", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const win = fakeWindow();
  let outcome = "waiting";
  const shown = awaitFirstShow(win, { timeoutMs: firstShowTimeoutMs({ isDev: false }) });
  shown.then(
    () => (outcome = "shown"),
    () => (outcome = "failed"),
  );
  t.mock.timers.tick(90_000);
  await settled();
  assert.equal(outcome, "waiting");
  win.emit("ready-to-show");
  await shown;
});

test("a packaged window that never paints still fails the boot, when the deadline runs out", async (t) => {
  t.mock.timers.enable({ apis: ["setTimeout"] });
  const win = fakeWindow();
  const shown = awaitFirstShow(win, { timeoutMs: firstShowTimeoutMs({ isDev: false }) });
  t.mock.timers.tick(firstShowTimeoutMs({ isDev: false }));
  await assert.rejects(shown, /did not reach ready-to-show within/);
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

test("a guard given no deadline at all waits, the same as one given none", async () => {
  // An omitted deadline must not become a timer of zero.
  const win = fakeWindow();
  const shown = awaitFirstShow(win, {});
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

test("a window closed before its first show ends the guard, deadline and all", async () => {
  const win = fakeWindow();
  const shown = awaitFirstShow(win, { timeoutMs: 30 });
  win.closeFake();
  await assert.rejects(shown, /closed before the first show/);
  // Nothing is left to fire on a window that no longer exists.
  assert.equal(win.listenerCount("ready-to-show"), 0);
  assert.equal(win.contents.listenerCount("render-process-gone"), 0);
  await new Promise((resolve) => setTimeout(resolve, 60));
});

test("a renderer that dies after the first show is not the guard's business", async () => {
  const win = fakeWindow();
  const shown = awaitFirstShow(win, { timeoutMs: 1_000 });
  win.emit("ready-to-show");
  await shown;
  win.webContents.emit("render-process-gone", {}, { reason: "crashed", exitCode: 139 });
  assert.equal(win.webContents.listenerCount("render-process-gone"), 0);
  assert.equal(win.listenerCount("closed"), 0);
});

// openFirstPage: the load and the guard together, and the one call
// that shows the window. A failed first page and a late first paint
// can arrive in either order — an error page still paints — and the
// boot's report hides the failed window before its dialog goes up. A
// show that lands after that puts the failed window back on screen
// under the dialog.

/** A window that counts its `show()` calls. */
const showableWindow = (load) => {
  const win = pageWindow(load);
  win.shows = 0;
  win.show = () => (win.shows += 1);
  return win;
};

test("the window is shown when it is ready, without waiting for the page to finish", async () => {
  let finish;
  const win = showableWindow(async (w, url) => {
    await answers(200)(w, url);
    await new Promise((resolve) => (finish = resolve));
  });
  const opened = openFirstPage(win, "app://localhost/", { timeoutMs: 1_000, logError: () => {} });
  win.emit("ready-to-show");
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(win.shows, 1);
  finish();
  await opened;
  assert.equal(win.shows, 1);
});

test("a window whose first page failed is not shown when it paints afterwards", async () => {
  const win = showableWindow(answers(404));
  await assert.rejects(
    openFirstPage(win, "app://localhost/", { timeoutMs: 1_000, logError: () => {} }),
    /first page answered 404/,
  );
  // The error page's first paint, arriving after the load was judged.
  win.emit("ready-to-show");
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(win.shows, 0);
});

test("a window that failed its first show fails the boot even if the page loaded", async () => {
  const win = showableWindow(answers(200));
  const opened = openFirstPage(win, "app://localhost/", { timeoutMs: 1_000, logError: () => {} });
  win.webContents.emit("render-process-gone", {}, { reason: "crashed", exitCode: 139 });
  await assert.rejects(opened, /crashed/);
  assert.equal(win.shows, 0);
});

// A window closed while it is still loading — the user's doing, by
// the window manager, on a boot that is taking its time. Electron
// rejects the load (ERR_FAILED, as for a crashed renderer), and the
// app is already quitting through window-all-closed. That is a quit,
// not a boot that failed: reporting it would put "could not start" on
// screen in answer to the user closing the window.

/** A load cut short by the window closing under it. */
const closesMidLoad = async (win) => {
  win.closeFake();
  throw loadError("ERR_FAILED", -2, "app://localhost/");
};

test("a window closed mid-load fails the load with the load's own error", async () => {
  // Not with what reading a destroyed window throws while tidying up.
  const win = pageWindow(closesMidLoad);
  await assert.rejects(
    loadFirstPage(win, "app://localhost/", () => {}),
    (err) => err.code === "ERR_FAILED",
  );
  assert.equal(win.contents.listenerCount("did-navigate"), 0);
});

test("a window closed before its first page arrived is not a failed boot", async () => {
  const win = showableWindow(closesMidLoad);
  await openFirstPage(win, "app://localhost/", { timeoutMs: 1_000, logError: () => {} });
  assert.equal(win.shows, 0);
});

test("nor is one destroyed outright, where the window goes before its contents", async () => {
  const win = showableWindow(async (w) => {
    w.destroyFake();
    throw loadError("ERR_FAILED", -2, "app://localhost/");
  });
  await openFirstPage(win, "app://localhost/", { timeoutMs: 1_000, logError: () => {} });
  assert.equal(win.shows, 0);
});

test("a renderer that crashed mid-load is still a failed boot: its window is there", async () => {
  const win = showableWindow(fails(loadError("ERR_FAILED", -2, "app://localhost/")));
  await assert.rejects(
    openFirstPage(win, "app://localhost/", { timeoutMs: 1_000, logError: () => {} }),
    (err) => err.code === "ERR_FAILED",
  );
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

// The dialog can stay up for a minute, and the app can be asked to
// quit in that minute from outside — a signal, the session ending.
// Left to run its course that quit exits with code 0, and a boot that
// failed has reported success to whatever launched it.
test("a quit asked for while the failure is being reported still exits as a failure", async () => {
  const events = [];
  let quitNow = null;
  const booted = bootApp({
    spawnBackend: async () => {
      throw new Error("backend exited before handshake");
    },
    createWindow: async () => {},
    stopBackend: () => events.push("stop"),
    // A dialog nobody dismisses.
    reportFailure: () => new Promise(() => {}),
    onQuitAsked: (ended) => {
      quitNow = ended;
    },
    exit: (code) => events.push(`exit:${code}`),
    logError: () => {},
  });
  await new Promise((resolve) => setImmediate(resolve));
  assert.equal(typeof quitNow, "function", "the failed boot listens for a quit");
  assert.deepEqual(events, ["stop"]);
  quitNow();
  await booted;
  assert.deepEqual(events, ["stop", "exit:1"]);
});

// A quit asked for during the handshake stops the backend, and the
// backend's exit then fails the handshake. That boot did not fail; it
// was quit. It ends as the quit does — no failure logged, no dialog,
// and the quit's own exit code rather than 1.
test("a boot failed by a quit already under way ends as that quit", async () => {
  const events = [];
  await bootApp({
    spawnBackend: async () => {
      throw new Error("backend exited before handshake (code=null, signal=SIGTERM)");
    },
    createWindow: async () => events.push("window"),
    stopBackend: () => events.push("stop"),
    reportFailure: async () => events.push("report"),
    onQuitAsked: () => events.push("listen"),
    quitting: () => true,
    exit: (code) => events.push(`exit:${code}`),
    logError: () => events.push("log"),
  });
  assert.deepEqual(events, []);
});

test("a boot that succeeds never asks to hear about a quit", async () => {
  let asked = 0;
  await bootApp({
    spawnBackend: async () => ({ apiBase: "http://127.0.0.1:1", internalSecret: "s" }),
    createWindow: async () => {},
    stopBackend: () => {},
    onQuitAsked: () => (asked += 1),
    exit: () => {},
    logError: () => {},
  });
  assert.equal(asked, 0);
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
