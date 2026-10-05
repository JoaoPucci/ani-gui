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
 * What a later launch finds, and what it does about it.
 *
 * The window is the one the boot has shown (`shown`); it is let go
 * when that window closes (`closed`) and when the boot fails
 * (`failed`) — a failed boot has no window worth bringing forward.
 * `booted` records what the boot's window was opened against — the
 * backend's apiBase and internal secret — once the boot has opened it.
 *
 * `summon()` then answers a launch:
 *
 *   - a window is up: it is brought forward (revealWindow);
 *   - the boot has not opened its window yet: nothing — the booting
 *     instance shows its window itself;
 *   - the boot failed: nothing — the instance is on its way out;
 *   - the app is quitting: nothing — the quit is stopping the backend
 *     a new window would load against;
 *   - otherwise the boot succeeded and its window is gone, which is
 *     macOS after the last window closed (window-all-closed quits
 *     everywhere else): a window is opened against the boot's backend,
 *     one at a time. A reopen that fails is logged and its window
 *     discarded — on screen, frameless and blank, it would have
 *     nothing on it to close it by — and the next launch tries again.
 *
 * Returns what it did: "revealed", "reopened" or "none".
 */
function windowKeeper({
  createWindow = async () => {},
  quitting = () => false,
  logError = () => {},
  discard = () => {},
} = {}) {
  let current = null;
  let backend = null;
  let reopening = false;
  return {
    booted(opened) {
      backend = opened;
    },
    shown(win) {
      current = win;
    },
    closed(win) {
      if (current === win) current = null;
    },
    failed() {
      current = null;
      backend = null;
    },
    summon() {
      if (revealWindow(current)) return "revealed";
      if (!backend || reopening || quitting()) return "none";
      reopening = true;
      (async () => {
        try {
          await createWindow(backend);
        } catch (err) {
          logError("[main] could not reopen the window:", err);
          discard();
        } finally {
          reopening = false;
        }
      })();
      return "reopened";
    },
  };
}

/**
 * Take the single-instance lock. The first instance gets it and calls
 * `summon()` whenever a later launch arrives — what that does is read
 * at that moment, since the lock is taken before any window exists. A
 * later instance gets `false` after `app.quit()` has been asked, and
 * its caller must stop there: no whenReady work, no backend.
 *
 * @param {{requestSingleInstanceLock(): boolean, quit(): void, on(event: string, handler: Function): void}} app
 * @param {{summon: () => unknown}} options
 * @returns {boolean} whether this process holds the lock
 */
function claimSingleInstance(app, { summon }) {
  if (!app.requestSingleInstanceLock()) {
    app.quit();
    return false;
  }
  app.on("second-instance", () => {
    summon();
  });
  return true;
}

module.exports = { claimSingleInstance, revealWindow, windowKeeper };
