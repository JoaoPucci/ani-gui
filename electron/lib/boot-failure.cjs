"use strict";

// What a failed boot tells the user.
//
// A boot that fails logs its reason and exits with code 1. A packaged
// app started from the desktop has no terminal to log to — the user
// double-clicks and nothing happens — so it shows an error dialog
// first. Lifted out of main.js so the decision can be exercised
// without Electron.

const { messagesFor } = require("./main-messages.cjs");

// How long the dialog stays up when nobody dismisses it. Long enough
// to read two sentences and a line of detail, or to come back to after
// looking away; short enough that a run nobody is watching ends well
// inside any job's patience. The backend is already stopped by then,
// so all that waits is the dialog.
const BOOT_FAILURE_DIALOG_MS = 60_000;

/**
 * The dialog a failed boot shows, or `null` when it shows none.
 *
 * None where nobody is there to read one:
 *
 *   - an unattended run — `ANI_GUI_UNATTENDED=1`, which the e2e suites
 *     set. Their cold-launch retry depends on a failed boot ending the
 *     process at once, and a dialog waits for a person. An environment
 *     variable rather than a guess from the display or the command
 *     line: a harness knows it is one, and nothing else does.
 *   - a dev launch, whose terminal already carries the reason, and
 *     which a dialog would hold open until someone clicked it away.
 *
 * Otherwise an error box in the app's language with the failure's own
 * message as its detail. That message is the developer's text, not
 * translated: it is what a bug report needs quoted.
 *
 * `timeoutMs` bounds it (see showBounded): an unattended run that did
 * not say so — a packager's smoke test under Xvfb — must still exit.
 */
function bootFailureDialog({ env, isDev, locale, error }) {
  if (isDev || (env && env.ANI_GUI_UNATTENDED === "1")) return null;
  const messages = messagesFor(locale);
  const reason = error && error.message ? error.message : String(error);
  return {
    options: {
      type: "error",
      title: "ani-gui",
      message: messages.bootFailed,
      detail: messages.bootFailedDetail(reason),
      buttons: [messages.close],
      defaultId: 0,
      noLink: true,
    },
    timeoutMs: BOOT_FAILURE_DIALOG_MS,
  };
}

/**
 * Show `dialog` through `show` and resolve when it is dismissed, when
 * its `timeoutMs` has passed, or when it could not be shown at all —
 * whichever comes first. Resolves at once, showing nothing, for no
 * dialog. Never rejects: the caller is on its way to exiting, and the
 * dialog must not be what stops it.
 *
 * `show` has to be non-blocking (Electron's `dialog.showMessageBox`,
 * not its `Sync` twin or `showErrorBox`), or the timeout could never
 * fire.
 */
function showBounded(dialog, show) {
  if (!dialog) return Promise.resolve();
  return new Promise((resolve) => {
    const timer = setTimeout(resolve, dialog.timeoutMs);
    const done = () => {
      clearTimeout(timer);
      resolve();
    };
    try {
      Promise.resolve(show(dialog.options)).then(done, done);
    } catch {
      done();
    }
  });
}

module.exports = { bootFailureDialog, showBounded };
