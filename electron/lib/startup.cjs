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
 * dropped. Every other rejection — a refused connection, a renderer
 * that crashed mid-load (ERR_FAILED) — leaves a blank window with
 * nothing on it, a close button included (see awaitFirstShow), so it
 * propagates and fails the boot.
 *
 * So does a page that arrives as an error page. `loadURL` resolves
 * for anything that arrives, and the app serves its own error pages:
 * a bundle with no index.html is answered by the app:// handler with
 * a 404, which loads like any page and leaves the same bare window
 * around the words "not found". The status comes from the
 * navigation's commit; `-1`, a navigation outside HTTP, is nothing to
 * judge.
 */
async function loadFirstPage(win, url, logError) {
  // Taken once: a window closed mid-load is destroyed, and reading
  // `webContents` off a destroyed window throws.
  const contents = win.webContents;
  let status = null;
  const onNavigate = (_event, _url, httpResponseCode) => {
    status = httpResponseCode;
  };
  contents.once("did-navigate", onNavigate);
  try {
    await win.loadURL(url);
  } catch (err) {
    if (!err || err.code !== "ERR_ABORTED") throw err;
    logError("[main] first page superseded before it finished loading:", err);
    return;
  } finally {
    contents.removeListener("did-navigate", onNavigate);
  }
  if (typeof status === "number" && status >= 400) {
    throw new Error(`first page answered ${status} (${url})`);
  }
}

// How long a packaged launch gives its window to reach ready-to-show:
// two minutes, the handshake's budget, for the handshake's reason
// (lib/backend-handshake.cjs).
//
// The deadline is for a window that will never paint. One that is
// lost for a reason the boot can see does not wait for it: a renderer
// that dies and a first page that fails to load fail the boot when
// they happen. What is left is a renderer alive and stuck, which is
// rare, against a first paint that is merely slow, which is not — it
// waits on a renderer starting and the bundle being read from the
// same disk that made the backend's first start take 24 s, and on a
// machine rendering in software it waits on that too. A deadline that
// calls slow "failed" costs a working install its launch; one that is
// long only delays the report of a window that was never coming.
const FIRST_SHOW_TIMEOUT_MS = 120_000;

/**
 * How long a launch gives its window to reach ready-to-show, or
 * `null` for no deadline.
 *
 * A dev launch has none. Its first page comes from Vite, and a cold
 * Vite re-optimizing its dependencies can take longer over the first
 * paint than any figure fit for the packaged bundle. The developer is
 * at the terminal watching it work and can stop it there; ending
 * `pnpm dev` under them would be the launcher getting in the way. A
 * renderer that dies still fails a dev launch — awaitFirstShow keeps
 * that half whatever the deadline.
 */
function firstShowTimeoutMs({ isDev }) {
  return isDev ? null : FIRST_SHOW_TIMEOUT_MS;
}

/**
 * Resolve when the window reaches `ready-to-show`; reject when its
 * renderer dies first, when the window is closed first, or when it
 * has not got there within `timeoutMs` — unless that is `null` or
 * left out, which sets no deadline (see firstShowTimeoutMs).
 *
 * The window is on screen well before `ready-to-show`: it is created
 * with `show: false`, but main.js maximizes it straight away, and
 * maximizing a hidden window shows it. It is also frameless — the
 * titlebar and its buttons are the renderer's to draw. So a window
 * that never gets its first paint is a blank rectangle with nothing
 * on it to click, the close button included, and the app would
 * otherwise sit there with its backend running behind it.
 *
 * After the first show the guard lets go: a later renderer crash is
 * not a boot failure.
 */
function awaitFirstShow(win, { timeoutMs }) {
  // Taken once, for the same reason as in loadFirstPage.
  const contents = win.webContents;
  return new Promise((resolve, reject) => {
    const settle = (fn, value) => {
      clearTimeout(timer);
      win.removeListener("ready-to-show", onShow);
      win.removeListener("closed", onClosed);
      contents.removeListener("render-process-gone", onGone);
      fn(value);
    };
    const onShow = () => settle(resolve);
    const onClosed = () =>
      settle(reject, new Error("window closed before the first show"));
    const onGone = (_event, details) =>
      settle(
        reject,
        new Error(
          `renderer gone before the first show (${details && details.reason})`,
        ),
      );
    const timer =
      timeoutMs == null
        ? undefined
        : setTimeout(
            () =>
              settle(
                reject,
                new Error(
                  `window did not reach ready-to-show within ${timeoutMs} ms`,
                ),
              ),
            timeoutMs,
          );
    win.once("ready-to-show", onShow);
    win.once("closed", onClosed);
    contents.on("render-process-gone", onGone);
  });
}

/**
 * Load the window's first page, and show the window once it is ready
 * to be shown — the one place it is shown. Rejects when either the
 * load (loadFirstPage) or the guard (awaitFirstShow) fails.
 *
 * The show follows ready-to-show and does not wait for the load. The
 * two finish in either order — an error page paints like any other —
 * so once the open has failed, the show is off: the boot's report
 * hides the failed window, and a first paint arriving afterwards must
 * not put it back on screen.
 *
 * A window that is gone is not a failure. The window is on screen
 * while it loads, and something that closes it then — the user,
 * through the window manager, on a boot taking its time — makes the
 * load reject like a crashed renderer's (ERR_FAILED) while the app is
 * already quitting over its last window. That is a quit, and the open
 * resolves with nothing to report.
 */
async function openFirstPage(win, url, { timeoutMs, logError }) {
  const contents = win.webContents;
  let failed = false;
  const shown = awaitFirstShow(win, { timeoutMs }).then(() => {
    if (!failed) win.show();
  });
  try {
    await Promise.all([loadFirstPage(win, url, logError), shown]);
  } catch (err) {
    failed = true;
    // Either part may go first: close() destroys the contents and
    // then the window, destroy() the other way round.
    if (win.isDestroyed() || contents.isDestroyed()) return;
    throw err;
  }
}

/**
 * Spawn the backend, then open the window against it. A failure ends
 * the process with code 1, in this order:
 *
 *   1. The backend tree is stopped. `app.exit` skips `before-quit`,
 *      which is where a normal quit stops it, and the backend's own
 *      watch only acts once this process is gone — which, with a
 *      dialog to show, may be a minute away.
 *   2. The failure is reported to the user — `reportFailure`, a dialog
 *      where there is someone to read it (lib/boot-failure.cjs). After
 *      the stop, so nothing is left running behind the dialog; awaited,
 *      so the exit does not take the dialog down unread.
 *   3. The app exits with code 1, however the report ended: dismissed,
 *      timed out, failed to show — or cut short by a quit asked for
 *      from outside while it was up. `onQuitAsked` is how the boot
 *      hears of that one: it is handed a function to call when the app
 *      is asked to quit, and has to keep that quit from running its
 *      own course, which would exit with code 0. It is only called
 *      once the boot has failed.
 */
async function bootApp({
  spawnBackend,
  createWindow,
  stopBackend,
  reportFailure = async () => {},
  onQuitAsked = () => {},
  exit,
  logError,
}) {
  try {
    const backend = await spawnBackend();
    await createWindow(backend);
  } catch (err) {
    logError("[main] startup failed:", err);
    stopBackend();
    const quitAsked = new Promise((resolve) => onQuitAsked(resolve));
    const reported = (async () => {
      try {
        await reportFailure(err);
      } catch (reportErr) {
        logError("[main] could not report the failed startup:", reportErr);
      }
    })();
    await Promise.race([reported, quitAsked]);
    exit(1);
  }
}

module.exports = {
  awaitFirstShow,
  bootApp,
  firstShowTimeoutMs,
  loadFirstPage,
  openFirstPage,
};
