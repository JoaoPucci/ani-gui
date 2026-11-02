"use strict";

// The splash: the window on screen while the backend starts and the
// launch's flags are decided, before the app's own window has painted.
// It sits above the app's window until that window shows, then
// closes (see bootApp in lib/startup.cjs).

const path = require("node:path");

/** Set on the splash's BrowserWindow, so a harness can tell it from
 *  the app's window (electron/e2e/launch.ts). */
const SPLASH_MARK = "aniGuiSplash";

/** The page the splash loads. */
const SPLASH_PAGE = path.join(__dirname, "..", "splash", "index.html");

/** The splash window's options. */
function splashWindowOptions() {
  return {};
}

module.exports = { SPLASH_MARK, SPLASH_PAGE, splashWindowOptions };
