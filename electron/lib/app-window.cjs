"use strict";

// The app's own window: how it is made, and the renderer's word that
// its first page has actually rendered, which the reveal waits for.
//
// The window is revealed at ready-to-show, which is the first paint of
// whatever document has loaded. On a slow first render — Vite compiling
// the frontend on a cold dev start, a cold disk — that is the bare shell
// before the app's stylesheet and first route, which Chromium paints
// white. So the window paints the app's own background from the start,
// and the reveal waits for the renderer to say its first page is up.

/** The app's page background (tokens.css --ink-000), which the window
 *  paints before its page does, and the splash too. */
const APP_BACKGROUND = "#0c0a09";

/** The channel the renderer says it has rendered on. */
const APP_READY_CHANNEL = "ani-gui:app-ready";

/** The longest the reveal waits for that word, counted from the
 *  window's creation, when no splash stands in for the window: a
 *  window opened later (macOS reopening one against the running
 *  backend), or a boot whose splash could not be made. Past it, the
 *  window is revealed anyway, on the dark background, never white —
 *  it would otherwise stay hidden for good, with nothing on screen.
 *
 *  While the boot's splash stands in, the wait has no bound
 *  (appReadyBound): a page that is late leaves the splash stuck,
 *  offering Restart and Quit (lib/splash-stuck.cjs), and the window is
 *  revealed only once its page has rendered. The bound used to apply
 *  there too, and revealed a window whose page had not painted: dark
 *  and empty. */
const APP_READY_WAIT_MS = 15_000;

/** The bound the wait for the window's first page takes: none while
 *  `splash` stands in for the window, APP_READY_WAIT_MS otherwise. */
function appReadyBound(splash) {
  return splash.standsIn() ? null : APP_READY_WAIT_MS;
}

/** The app window's options. */
function appWindowOptions({ width, height, icon, preload, additionalArguments }) {
  return {
    width,
    height,
    minWidth: 960,
    minHeight: 600,
    show: false,
    backgroundColor: APP_BACKGROUND,
    icon,
    // Frameless: the renderer draws its titlebar and controls. Resizable
    // stays at its default (true), so the OS resize edges still work.
    frame: false,
    webPreferences: { preload, contextIsolation: true, nodeIntegration: false, additionalArguments },
  };
}

let current = { rendered: false, gaveUp: false, beforeReveal: false };

/**
 * Wait for `contents`' renderer to say, on APP_READY_CHANNEL of `ipc`,
 * that its first page has rendered. `ready` settles true when it does,
 * false after `timeoutMs` (never, when that is null), and false when
 * the renderer goes or the contents are destroyed first — `gone()` then
 * says so. A renderer gone fails the boot (lib/startup.cjs
 * waitBeforeShow); destroyed contents are a window closed by a quit,
 * which reveals nothing (revealMaximized).
 * Either way the listeners go. Start it when the window is made, so a
 * word that comes early is not missed. The record notes whether the
 * window was still hidden (`isShown`) when the word came: the reveal
 * waited for it.
 *
 * The renderer sends the word after two animation frames, which run in
 * a hidden window only because `paintWhenInitiallyHidden` is left at its
 * default (true). Turned off, `ready-to-show` never fires either, and
 * the window is never revealed (packaged: the boot fails at the
 * first-show deadline).
 */
function appReadyWaiter(contents, ipc, { timeoutMs = APP_READY_WAIT_MS, isShown = () => false } = {}) {
  const state = { rendered: false, gaveUp: false, beforeReveal: false };
  current = state;
  let settle;
  const ready = new Promise((resolve) => (settle = resolve));
  let timer = null;
  let settled = false;
  let lost = false;
  const watchesContents = typeof contents.on === "function";
  const done = (rendered, { gaveUp = !rendered } = {}) => {
    if (settled) return;
    settled = true;
    ipc.removeListener(APP_READY_CHANNEL, onReady);
    if (watchesContents) {
      contents.removeListener("render-process-gone", onGone);
      contents.removeListener("destroyed", onGone);
    }
    if (timer) clearTimeout(timer);
    state.rendered = rendered;
    state.gaveUp = gaveUp;
    settle(rendered);
  };
  // Matched by the sending contents' id: Electron may hand over another
  // wrapper of the same contents.
  function onReady(event) {
    if (!event || !event.sender || event.sender.id !== contents.id) return;
    state.beforeReveal = !isShown();
    done(true);
  }
  // With no bound, nothing else would end the wait for a renderer that
  // can no longer say anything.
  function onGone() {
    lost = true;
    done(false, { gaveUp: false });
  }
  ipc.on(APP_READY_CHANNEL, onReady);
  if (watchesContents) {
    contents.on("render-process-gone", onGone);
    contents.on("destroyed", onGone);
  }
  if (timeoutMs != null) timer = setTimeout(() => done(false), timeoutMs);
  return { ready, gone: () => lost };
}

/** What the latest launch's wait for the renderer saw, for the
 *  end-to-end suite. */
function lastAppReady() {
  return { ...current };
}

/** The reveal's gate: open once the splash is done (`whenShown`) and
 *  the renderer has said its first page rendered, or the wait for it
 *  gave up — and, either way, once the splash has been told and has
 *  completed its rail (`pageReady`, lib/splash.cjs), so no reveal closes
 *  the splash on a rail that is not seen complete. */
function revealGate(splash, appReady) {
  return Promise.all([
    splash.whenShown(),
    // A renderer that went is no page to complete the rail for: the
    // boot is about to fail.
    appReady.ready.then((rendered) => (appReady.gone && appReady.gone() ? undefined : splash.pageReady(rendered === true))),
  ]);
}

module.exports = {
  revealGate,
  APP_BACKGROUND,
  APP_READY_CHANNEL,
  APP_READY_WAIT_MS,
  appReadyBound,
  appReadyWaiter,
  appWindowOptions,
  lastAppReady,
};
