"use strict";

// One running instance per profile.
//
// Each instance spawns its own backend, and two backends on one
// profile share the history file, the cache database and the
// numbering — while the history's lock belongs to the process and
// cannot see across processes. So a second launch hands over to the
// instance already running and quits, before it spawns anything.
//
// Electron keys the lock on the userData directory, so it follows the
// profile: main.js must call this after app.setName() (which moves
// userData to `ani-gui-dev` under the dev profile) and before anything
// reads userData. A dev instance and an installed one therefore never
// block each other, and neither do runs that point XDG_CONFIG_HOME
// (Linux) or APPDATA (Windows) somewhere of their own, as the e2e
// suites do.

/**
 * Bring a window forward: restored if minimized, shown if hidden, then
 * focused. Returns whether there was a window to reveal — there is
 * none while the first instance is still booting, and that instance
 * shows its window itself once the page is ready.
 */
function revealWindow(win) {
  if (!win || win.isDestroyed()) return false;
  if (win.isMinimized()) win.restore();
  if (!win.isVisible()) win.show();
  win.focus();
  return true;
}

/**
 * Take the single-instance lock. The first instance gets it and
 * reveals `getWindow()` whenever a later launch arrives; the window is
 * read at that moment, since the lock is taken before any window
 * exists. A later instance gets `false` after `app.quit()` has been
 * asked, and its caller must stop there: no whenReady work, no backend.
 *
 * @param {{requestSingleInstanceLock(): boolean, quit(): void, on(event: string, handler: Function): void}} app
 * @param {{getWindow: () => any}} options
 * @returns {boolean} whether this process holds the lock
 */
function claimSingleInstance(app, { getWindow }) {
  if (!app.requestSingleInstanceLock()) {
    app.quit();
    return false;
  }
  app.on("second-instance", () => {
    revealWindow(getWindow());
  });
  return true;
}

module.exports = { claimSingleInstance, revealWindow };
