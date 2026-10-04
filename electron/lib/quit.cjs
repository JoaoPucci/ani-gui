"use strict";

// The app's before-quit decision, lifted out of main.js so it can be
// exercised without Electron. Behaviour is main.js's as it stood.

/**
 * Run the close prompt, then stop the backend tree.
 * `promptOnClose` returns whether it cancelled this quit.
 */
function handleBeforeQuit({ promptOnClose, stopBackend }) {
  promptOnClose();
  stopBackend();
}

module.exports = { handleBeforeQuit };
