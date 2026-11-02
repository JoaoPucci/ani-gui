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
  return {
    width: 480,
    height: 300,
    frame: false,
    show: false,
    resizable: false,
    movable: true,
    alwaysOnTop: true,
    // On the taskbar like the app it stands for, so a launch shows up
    // there at once.
    skipTaskbar: false,
    center: true,
    // The ink the page paints, so nothing flashes before it does.
    backgroundColor: "#0b0b0f",
    webPreferences: {
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
    },
  };
}

/** Show, close and suppress the splash. Stub. */
function splashController({ createWindow, logError }) {
  void createWindow;
  void logError;
  return { show() {}, close() {}, suppress() {} };
}

module.exports = { SPLASH_MARK, SPLASH_PAGE, splashController, splashWindowOptions };
