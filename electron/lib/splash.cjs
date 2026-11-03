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

/**
 * What the main process runs in the splash once its window shows and
 * the motion preference is known: under reduced motion, rest on the
 * finished still frame (`reduced`); otherwise start the motion
 * (`shown`) — unless the page's own fallback has already rested it.
 * It answers after two animation frames, once the renderer has
 * produced a frame with the splash in it (which the window's "show"
 * event, emitted when the window is mapped, does not mean), with
 * whether the splash is resting — by this signal, the fallback, or the
 * page's own reduced-motion media query. A hidden or covered splash
 * may never answer; the wait on it is bounded (SPLASH_SHOW_WAIT_MS).
 */
function shownSignal(reduced) {
  const start = reduced
    ? "document.body.classList.add('reduced');"
    : "if (!document.body.classList.contains('reduced')) document.body.classList.add('shown');";
  return (
    start +
    " new Promise((r) => requestAnimationFrame(() => requestAnimationFrame(() =>" +
    " r(document.body.classList.contains('reduced') ||" +
    " matchMedia('(prefers-reduced-motion: reduce)').matches))))"
  );
}

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
  // Settles when the splash can no longer reach the screen (no
  // window, closed, signal failed): nothing to wait for any more.
  let markUp = () => {};
  const released = new Promise((resolve) => (markUp = resolve));
  // Settles with whether the splash is resting once it has painted.
  let markPainted = () => {};
  const painted = new Promise((resolve) => (markPainted = resolve));
  // The shown signal waits for both the window's show and the motion
  // preference; it goes once.
  let windowShown = false;
  let preference = null;
  let signalled = false;
  let win = null;
  let suppressed = false;
  // Set when the splash closes by any hand but the boot's: the user,
  // or a quit already under way.
  let dismissedByUser = false;
  // Set once the boot is past the splash: shown its window, or failed.
  let over = false;
  const alive = () => win && !win.isDestroyed();
  const signal = () => {
    if (signalled || !windowShown || preference === null || !alive()) return;
    signalled = true;
    const target = win;
    Promise.resolve(target.webContents.executeJavaScript(shownSignal(preference)))
      .then((resting) => {
        state.shown = true;
        paintedAt = clock();
        log(`[main] splash painted (${since()})`);
        markPainted(resting === true);
      })
      .catch((err) => {
        logError("[main] could not tell the splash it is shown:", err);
        markUp();
      });
  };
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
      // The page holds its first frame until the shown signal, sent
      // once the window is on screen and the motion preference is
      // known, so the opening plays once the window manager has mapped
      // the window and never before the preference is known.
      splash.once("show", () => {
        log(`[main] splash shown (${since()})`);
        windowShown = true;
        signal();
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
    // The desktop's reduced-motion preference, once read: until then
    // the splash holds its first frame, and it is sent with the shown
    // signal.
    motionPreference(reduced) {
      if (preference !== null) return;
      preference = reduced === true;
      signal();
    },
    // Resolves once the splash has painted and been on screen for its
    // minimum (SPLASH_MIN_ON_SCREEN_MS, or the shorter
    // SPLASH_MIN_ON_SCREEN_REDUCED_MS when it rests on its still
    // frame) — or at once when there is nothing to wait for (never
    // asked to open, failed to, put away, gone). The paint itself is
    // waited for at most SPLASH_SHOW_WAIT_MS.
    async whenShown() {
      if (!state.opened || over || !alive()) return;
      let timer;
      const late = new Promise((resolve) => {
        timer = setTimeout(() => resolve("late"), SPLASH_SHOW_WAIT_MS);
      });
      const first = await Promise.race([
        released.then(() => "released"),
        painted.then((resting) => ({ resting })),
        late,
      ]);
      clearTimeout(timer);
      if (typeof first !== "object" || over || !alive()) return;
      const min = first.resting ? SPLASH_MIN_ON_SCREEN_REDUCED_MS : SPLASH_MIN_ON_SCREEN_MS;
      // Counted from the paint: a window ready long after it waits for
      // nothing more.
      // Never more than the minimum: a clock set back since the paint
      // must not stretch the wait.
      const remaining = Math.min(min, min - (clock() - paintedAt));
      if (remaining <= 0) return;
      let minTimer;
      await Promise.race([
        released,
        new Promise((resolve) => {
          minTimer = setTimeout(resolve, remaining);
        }),
      ]);
      clearTimeout(minTimer);
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
      // Put away: it can no longer be on screen, so nothing waits on it.
      markUp();
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

/** The longest the app's window waits for the splash to paint. The
 *  splash is a light static page and paints well within this; the bound
 *  is for one that never does. With the minimum time on screen after a
 *  paint, the whole wait is at most this plus SPLASH_MIN_ON_SCREEN_MS. */
const SPLASH_SHOW_WAIT_MS = 1000;

/** How long the splash stays on screen after it has painted before the
 *  app's window may replace it: the pull-down's opening has read by
 *  about 500 ms (lamp lit, title in, first frame landed at 499 ms). A
 *  cold launch, whose window is ready later than that anyway, pays
 *  nothing for it. */
const SPLASH_MIN_ON_SCREEN_MS = 500;

/** The same under reduced motion, when the splash rests on its still
 *  frame, which reads at once. */
const SPLASH_MIN_ON_SCREEN_REDUCED_MS = 300;

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
  SPLASH_MIN_ON_SCREEN_MS,
  SPLASH_MIN_ON_SCREEN_REDUCED_MS,
  SPLASH_SHOW_WAIT_MS,
  revealMaximized,
  SPLASH_MARK,
  SPLASH_PAGE,
  lastSplashState,
  splashController,
  splashWindowOptions,
};
