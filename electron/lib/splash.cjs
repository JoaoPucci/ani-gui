"use strict";

// The splash: the window on screen while the backend starts and the
// launch's flags are decided. It is on screen first and alone: the
// app's window is not mapped until the splash has painted
// (revealMaximized), so the two never compete for the top, whether or
// not the platform honours always-on-top (native Wayland does not).
// It closes once the app's window shows (see bootApp in
// lib/startup.cjs).

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
 * - `suppress` puts it away for good when the boot fails: hidden, not
 *   closed, and a paint that arrives later does not show it again —
 *   it would come back always on top, over the dialog that says why
 *   the boot failed.
 */
function splashController({ createWindow, logError }) {
  const state = fresh();
  current = state;
  // Settles once the splash is on screen, or can no longer get there.
  let markUp = () => {};
  const up = new Promise((resolve) => (markUp = resolve));
  let win = null;
  let suppressed = false;
  // Set once the boot is past the splash: shown its window, or failed.
  let over = false;
  const alive = () => win && !win.isDestroyed();
  return {
    show() {
      if (over) return;
      let splash;
      try {
        splash = createWindow(splashWindowOptions());
      } catch (err) {
        logError("[main] could not show the splash:", err);
        markUp();
        return;
      }
      if (!splash) {
        markUp();
        return;
      }
      splash[SPLASH_MARK] = true;
      // The page holds its motion until it is told the window is on
      // screen (splash.js falls back to a timer), so the opening plays
      // once the window manager has mapped the window.
      splash.once("show", () => {
        Promise.resolve(splash.webContents.executeJavaScript(SHOWN_SIGNAL))
          .then(() => {
            state.shown = true;
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
        if (win === splash) win = null;
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
        win.close();
      }
      win = null;
    },
    // Resolves once the splash is on screen — or there is none to wait
    // for (never asked to open, failed to, put away, gone) — and in any
    // case within SPLASH_SHOW_WAIT_MS.
    dismissed() {
      return false;
    },
    whenShown() {
      if (!state.opened || over || !alive()) return Promise.resolve();
      let timer;
      const late = new Promise((resolve) => (timer = setTimeout(resolve, SPLASH_SHOW_WAIT_MS)));
      return Promise.race([up, late]).finally(() => clearTimeout(timer));
    },
    suppress() {
      over = true;
      suppressed = true;
      state.suppressed = true;
      if (alive()) win.hide();
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
 * window gone meanwhile is left alone.
 */
function revealMaximized(win, { onMapped, afterShow }) {
  if (win.isDestroyed()) return;
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
