"use strict";

// The main process's boot sequence, lifted out of main.js so the
// failure paths can be exercised without Electron.

/**
 * Load the window's first page.
 *
 * Only a superseded load is survivable. Electron rejects `loadURL`
 * with ERR_ABORTED whenever another main-frame navigation starts
 * before the first page finishes loading, and the window that
 * navigation lands in still works, so that rejection is logged and
 * dropped. Every other rejection — a missing bundle, a refused
 * connection, a renderer that crashed mid-load (ERR_FAILED) — leaves a
 * blank window the user cannot do anything with, not even close (see
 * awaitFirstShow), so it propagates and fails the boot.
 */
async function loadFirstPage(win, url, logError) {
  try {
    await win.loadURL(url);
  } catch (err) {
    if (!err || err.code !== "ERR_ABORTED") throw err;
    logError("[main] first page superseded before it finished loading:", err);
  }
}

/**
 * Resolve when the window reaches `ready-to-show`; reject when its
 * renderer dies first, or when it has not got there within
 * `timeoutMs`.
 *
 * The window is on screen well before `ready-to-show`: it is created
 * with `show: false`, but main.js maximizes it straight away, and
 * maximizing a hidden window shows it. It is also frameless — the
 * titlebar and its buttons are the renderer's to draw. So a window
 * that never gets its first paint is a blank rectangle with nothing
 * to click, the close button included, and the app would otherwise
 * sit there with its backend running and no way out.
 *
 * After the first show the guard lets go: a later renderer crash is
 * not a boot failure.
 */
function awaitFirstShow(win, { timeoutMs }) {
  return new Promise((resolve, reject) => {
    const settle = (fn, value) => {
      clearTimeout(timer);
      win.removeListener("ready-to-show", onShow);
      win.webContents.removeListener("render-process-gone", onGone);
      fn(value);
    };
    const onShow = () => settle(resolve);
    const onGone = (_event, details) =>
      settle(
        reject,
        new Error(
          `renderer gone before the first show (${details && details.reason})`,
        ),
      );
    const timer = setTimeout(
      () =>
        settle(
          reject,
          new Error(`window did not reach ready-to-show within ${timeoutMs} ms`),
        ),
      timeoutMs,
    );
    win.once("ready-to-show", onShow);
    win.webContents.on("render-process-gone", onGone);
  });
}

/**
 * Spawn the backend, then open the window against it. A failure ends
 * the process with code 1, but stops the backend tree first:
 * `app.exit` skips `before-quit`, which is where a normal quit stops
 * it, and the backend runs detached in its own process group, so
 * nothing else would.
 */
async function bootApp({ spawnBackend, createWindow, stopBackend, exit, logError }) {
  try {
    const backend = await spawnBackend();
    await createWindow(backend);
  } catch (err) {
    logError("[main] startup failed:", err);
    stopBackend();
    exit(1);
  }
}

module.exports = { awaitFirstShow, bootApp, loadFirstPage };
