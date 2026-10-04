"use strict";

// The app's before-quit decision, lifted out of main.js so it can be
// exercised without Electron.

const { messagesFor } = require("./main-messages.cjs");

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
 * The options for the prompt a quit with downloads running shows, in
 * `locale` (see lib/main-messages.cjs). Cancel comes first and is
 * both the default and what Escape picks: the prompt exists to stop
 * an accidental quit, so the accident must land on the safe button.
 */
function closePromptOptions({ locale, count }) {
  const messages = messagesFor(locale);
  return {
    type: "question",
    buttons: [messages.cancel, messages.quitAnyway],
    defaultId: 0,
    cancelId: 0,
    title: messages.downloadsTitle,
    message: messages.downloadsInProgress(count),
    detail: messages.downloadsQuitDetail,
  };
}

module.exports = { closePromptOptions, handleBeforeQuit };
