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

/**
 * The options for the prompt a quit with downloads running shows. As
 * main.js had it: English, whatever language the app is in.
 */
function closePromptOptions({ count }) {
  const plural = count === 1 ? "" : "s";
  return {
    type: "question",
    buttons: ["Cancel", "Quit anyway"],
    defaultId: 0,
    cancelId: 0,
    title: "Active downloads",
    message: `${count} download${plural} in progress.`,
    detail: "They will be cancelled if you quit. Continue?",
  };
}

module.exports = { closePromptOptions, handleBeforeQuit };
