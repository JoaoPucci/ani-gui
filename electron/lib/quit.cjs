"use strict";

// The app's before-quit decision, lifted out of main.js so it can be
// exercised without Electron.

/**
 * Run the close prompt; stop the backend tree only when the quit goes
 * ahead. `promptOnClose` returns whether it prevented this quit — the
 * user cancelled, or confirmed and a fresh close is already on its
 * way, whose own before-quit stops the backend.
 */
function handleBeforeQuit({ promptOnClose, stopBackend }) {
  if (promptOnClose()) return;
  stopBackend();
}

module.exports = { handleBeforeQuit };
