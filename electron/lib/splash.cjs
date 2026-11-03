"use strict";

// The splash: the window on screen while the backend starts and the
// launch's flags are decided. It is on screen first and alone: the
// app's window is not mapped until the splash has painted
// (revealMaximized), so the two never compete for the top, whether or
// not the platform honours always-on-top (native Wayland does not).
// It closes as the app's window shows, in that window's reveal
// (main.js), with bootApp's close in lib/startup.cjs as the fallback.
// A splash the user closes first cancels the launch (onDismissed).

const path = require("node:path");

/** Set on the splash's BrowserWindow, so a harness can tell it from
 *  the app's window (electron/e2e/launch.ts). */
const SPLASH_MARK = "aniGuiSplash";

/** What the main process runs in the splash once its window shows:
 *  start the motion, then answer after two animation frames — once the
 *  renderer has produced a frame with the splash in it, which the
 *  window's "show" event (emitted when the window is mapped) does not
 *  mean. A hidden or covered splash may never answer; the wait on it
 *  is bounded (SPLASH_SHOW_WAIT_MS). */
const SHOWN_SIGNAL =
  "document.body.classList.add('shown');" +
  " new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(() => r(true))))";

/** The page the splash loads. */
const SPLASH_PAGE = path.join(__dirname, "..", "splash", "index.html");

/** The splash window's options. */
function splashWindowOptions() {
  return {
    width: 480,
    height: 300,
    frame: false,
    show: false,
    resizable: false,
    // Movable by dragging anywhere on it (splash.css): it sits above
    // other apps while the backend starts.
    movable: true,
    alwaysOnTop: true,
    // On the taskbar like the app it stands for, so a launch shows up
    // there at once.
    skipTaskbar: false,
    center: true,
    // The ink the page paints, so nothing flashes before it does.
    backgroundColor: "#0c0a09",
    webPreferences: {
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  };
}

/**
 * The splash's lifecycle. `createWindow(options)` makes the window
 * (main.js passes `new BrowserWindow`).
 *
 * - `show` opens it, marked, and shows it once it has painted.
 * - `close` closes it: the boot's window has shown, so it is not the
 *   last window and no quit follows.
 * - `show` after `close` or `suppress` opens nothing: the boot is
 *   already past the splash.
 * - `dismissed` says whether the splash was closed by any hand but the
 *   boot's — the user, or a quit — which `onDismissed` also reports:
 *   the launch is cancelled.
 * - `suppress` puts it away for good when the boot fails: hidden, not
 *   closed, and a paint that arrives later does not show it again —
 *   it would come back always on top, over the dialog that says why
 *   the boot failed.
 */
function splashController({
  createWindow,
  logError,
  onDismissed = () => {},
  log = () => {},
  clock = () => Date.now(),
}) {
  const state = fresh();
  // One info line per step of the splash's life, timed from when it
  // was asked for: enough for a run's log to say whether a splash was
  // seen, and for how long.
  let askedAt = null;
  let paintedAt = null;
  const since = () => `+${clock() - askedAt} ms`;
  current = state;
  // Settles once the splash is on screen, or can no longer get there.
  let markUp = () => {};
  const up = new Promise((resolve) => (markUp = resolve));
  let win = null;
  let suppressed = false;
  // Set when the splash closes by any hand but the boot's: the user,
  // or a quit already under way.
  let dismissedByUser = false;
  // Set once the boot is past the splash: shown its window, or failed.
  let over = false;
  const alive = () => win && !win.isDestroyed();
  return {
    show() {
      if (over) return;
      askedAt = clock();
      log(`[main] splash asked for (${since()})`);
      let splash;
      try {
        splash = createWindow(splashWindowOptions());
      } catch (err) {
        logError("[main] could not show the splash:", err);
        log("[main] splash not opened: the window could not be made");
        markUp();
        return;
      }
      if (!splash) {
        log("[main] splash not opened: no window was made");
        markUp();
        return;
      }
      splash[SPLASH_MARK] = true;
      // The page holds its motion until it is told the window is on
      // screen (splash.js falls back to a timer), so the opening plays
      // once the window manager has mapped the window.
      splash.once("show", () => {
        log(`[main] splash shown (${since()})`);
        Promise.resolve(splash.webContents.executeJavaScript(SHOWN_SIGNAL))
          .then(() => {
            state.shown = true;
            paintedAt = clock();
            log(`[main] splash painted (${since()})`);
          })
          .catch((err) => logError("[main] could not tell the splash it is shown:", err))
          .finally(markUp);
      });
      splash.once("ready-to-show", () => {
        if (!suppressed && win === splash && !splash.isDestroyed()) splash.show();
      });
      splash.on("closed", () => {
        // Load-bearing: a splash destroyed while its shown signal is
        // out never answers it (the call neither resolves nor rejects),
        // so this is what releases a wait on a closed splash.
        markUp();
        const byUser = win === splash && !over;
        if (win === splash) win = null;
        if (byUser) {
          log(
          `[main] splash closed before the boot was done (by the user or a quit): the launch is cancelled (${since()})`,
        );
          dismissedByUser = true;
          over = true;
          onDismissed();
        }
      });
      win = splash;
      state.opened = true;
      Promise.resolve(splash.loadFile(SPLASH_PAGE)).catch((err) =>
        logError("[main] the splash failed to load:", err),
      );
    },
    close() {
      over = true;
      if (alive()) {
        state.closed = true;
        const onScreen = paintedAt === null ? "never painted" : `on screen ${clock() - paintedAt} ms after painting`;
        log(`[main] splash closed by the boot (${since()}, ${onScreen})`);
        win.close();
      }
      win = null;
    },
    // Resolves once the splash is on screen — or there is none to wait
    // for (never asked to open, failed to, put away, gone) — and in any
    // case within SPLASH_SHOW_WAIT_MS.
    whenShown() {
      if (!state.opened || over || !alive()) return Promise.resolve();
      let timer;
      const late = new Promise((resolve) => (timer = setTimeout(resolve, SPLASH_SHOW_WAIT_MS)));
      return Promise.race([up, late]).finally(() => clearTimeout(timer));
    },
    // Whether the user (or a quit) closed the splash before the boot
    // did: the launch is cancelled, and the app's window must not be
    // revealed.
    dismissed() {
      return dismissedByUser;
    },
    suppress() {
      over = true;
      suppressed = true;
      state.suppressed = true;
      if (alive()) {
        log(`[main] splash put away: the boot failed (${since()})`);
        win.hide();
      }
    },
  };
}

/** What the most recently made controller's splash went through:
 *  opened, shown on screen, closed after the boot's window showed, or
 *  suppressed by a failed boot. Read by the end-to-end suite, which
 *  cannot catch the splash on screen in a test run's fast start. */
let current = fresh();

function fresh() {
  return { opened: false, shown: false, closed: false, suppressed: false };
}

function lastSplashState() {
  return { ...current };
}

/** The longest the app's window waits for the splash to be on screen.
 *  The splash is a light static page and paints well within this; the
 *  bound is for one that never does. */
const SPLASH_SHOW_WAIT_MS = 1000;

/**
 * Reveal the app's window: `maximize()` the hidden window, which maps
 * it already maximized (the open-maximized pattern main.js describes),
 * attach what must act from the map on (`onMapped`), show it, and then
 * `afterShow` — the splash's close, at the moment the window is up. A
 * window gone meanwhile is left alone, and so is a launch `cancelled`
 * by the user closing the splash first.
 */
function revealMaximized(win, { onMapped, afterShow, cancelled = () => false }) {
  if (win.isDestroyed() || cancelled()) return;
  win.maximize();
  onMapped();
  win.show();
  afterShow();
}

module.exports = {
  SPLASH_SHOW_WAIT_MS,
  revealMaximized,
  SPLASH_MARK,
  SPLASH_PAGE,
  lastSplashState,
  splashController,
  splashWindowOptions,
};
