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
    frame: false,
    webPreferences: { preload, contextIsolation: true, nodeIntegration: false, additionalArguments },
  };
}

let current = { rendered: false, gaveUp: false };

/**
 * Wait for `contents`' renderer to say, on APP_READY_CHANNEL of `ipc`,
 * that its first page has rendered. `ready` settles true when it does,
 * false after `timeoutMs`; either way the listener goes. Start it when
 * the window is made, so a word that comes early is not missed.
 */
function appReadyWaiter(contents, ipc, { timeoutMs = APP_READY_WAIT_MS } = {}) {
  const state = { rendered: false, gaveUp: false };
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
  function onReady(event) {
    if (event && event.sender === contents) done(true);
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

module.exports = {
  APP_BACKGROUND,
  APP_READY_CHANNEL,
  APP_READY_WAIT_MS,
  appReadyWaiter,
  appWindowOptions,
  lastAppReady,
};
