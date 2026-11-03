"use strict";

// The app's own window: how it is made, and the renderer's word that
// its first page has actually rendered, which the reveal waits for.

/** The app's page background (tokens.css --ink-000), painted by the
 *  window before its page does. Stub. */
const APP_BACKGROUND = "#ffffff";

/** The channel the renderer says it has rendered on. */
const APP_READY_CHANNEL = "ani-gui:app-ready";

/** The longest the reveal waits for that word. Stub. */
const APP_READY_WAIT_MS = 0;

/** The app window's options. Stub. */
function appWindowOptions({ width, height, icon, preload, additionalArguments }) {
  return {
    width,
    height,
    minWidth: 960,
    minHeight: 600,
    show: false,
    icon,
    frame: false,
    webPreferences: { preload, contextIsolation: true, nodeIntegration: false, additionalArguments },
  };
}

/** Wait for `contents`' renderer to say it has rendered. Stub. */
function appReadyWaiter(contents, ipc, options = {}) {
  void contents;
  void ipc;
  void options;
  return { ready: Promise.resolve(false) };
}

/** What the latest launch's wait for the renderer saw. Stub. */
function lastAppReady() {
  return {};
}

module.exports = {
  APP_BACKGROUND,
  APP_READY_CHANNEL,
  APP_READY_WAIT_MS,
  appReadyWaiter,
  appWindowOptions,
  lastAppReady,
};
