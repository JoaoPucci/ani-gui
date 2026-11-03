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

/** The longest the reveal waits for that word: past it, the window is
 *  revealed anyway — on the dark background, never white. A cold Vite
 *  compile on the maintainer's machine took about 1.6 s. */
const APP_READY_WAIT_MS = 4000;

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
 * false after `timeoutMs`; either way the listener goes. Start it when
 * the window is made, so a word that comes early is not missed. The
 * record notes whether the window was still hidden (`isShown`) when
 * the word came: the reveal waited for it.
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
  const done = (rendered) => {
    ipc.removeListener(APP_READY_CHANNEL, onReady);
    if (timer) clearTimeout(timer);
    state.rendered = rendered;
    state.gaveUp = !rendered;
    settle(rendered);
  };
  // Matched by the sending contents' id: Electron may hand over another
  // wrapper of the same contents.
  function onReady(event) {
    if (!event || !event.sender || event.sender.id !== contents.id) return;
    state.beforeReveal = !isShown();
    done(true);
  }
  ipc.on(APP_READY_CHANNEL, onReady);
  timer = setTimeout(() => done(false), timeoutMs);
  return { ready };
}

/** What the latest launch's wait for the renderer saw, for the
 *  end-to-end suite. */
function lastAppReady() {
  return { ...current };
}

/** The reveal's gate: open once the splash is done (`splashShown`, a
 *  function returning its wait) and the renderer has said its first
 *  page rendered, or the wait for it gave up. */
function revealGate(splashShown, appReady) {
  return Promise.all([splashShown(), appReady.ready]);
}

module.exports = {
  revealGate,
  APP_BACKGROUND,
  APP_READY_CHANNEL,
  APP_READY_WAIT_MS,
  appReadyWaiter,
  appWindowOptions,
  lastAppReady,
};
