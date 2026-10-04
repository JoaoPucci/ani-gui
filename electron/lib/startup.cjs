"use strict";

// The main process's boot sequence, lifted out of main.js so the
// failure paths can be exercised without Electron. Behaviour is
// main.js's as it stood: any failure, the first page's load
// included, ends the process.

/** Load the window's first page. */
async function loadFirstPage(win, url, _logError) {
  await win.loadURL(url);
}

/** Spawn the backend, then open the window against it. */
async function bootApp({ spawnBackend, createWindow, exit, logError }) {
  try {
    const backend = await spawnBackend();
    await createWindow(backend);
  } catch (err) {
    logError("[main] startup failed:", err);
    exit(1);
  }
}

module.exports = { bootApp, loadFirstPage };
