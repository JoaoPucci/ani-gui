"use strict";

// The main process's boot sequence, lifted out of main.js so the
// failure paths can be exercised without Electron.

/**
 * Load the window's first page.
 *
 * A rejection here is not a startup failure. Electron rejects
 * `loadURL` with ERR_ABORTED whenever another main-frame navigation
 * starts before the first page finishes loading, and a window whose
 * first load was superseded is still a working window. A load that
 * genuinely fails also leaves the window open — the `did-fail-load`
 * listener has already logged it, and quitting would only turn a
 * visible failure into a window that vanishes.
 */
async function loadFirstPage(win, url, logError) {
  try {
    await win.loadURL(url);
  } catch (err) {
    logError("[main] first page did not finish loading:", err);
  }
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

/** Placeholder until the first-show guard exists. */
async function awaitFirstShow() {}

module.exports = { awaitFirstShow, bootApp, loadFirstPage };
