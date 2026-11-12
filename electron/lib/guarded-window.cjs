"use strict";

// Every window the main process makes is made here: the launch splash
// and the app's window alike. Each gets the navigation guard
// (window-guard.cjs) and the permission handlers (permissions.cjs) as
// it is made, before anything loads in it. tests/arch/window_guard.sh
// keeps windows from being made anywhere else.
//
// Both rule by the app's own document (app-document.cjs). The splash
// holds a file the main process loads, never the app's document, so a
// navigation inside it is refused and it is granted no permission; its
// page needs neither. The windows share the default session, so the
// permission handlers installed for one are the same ones for the
// other.

const { guardWindow } = require("./window-guard.cjs");
const { guardPermissions } = require("./permissions.cjs");

/** A maker of guarded windows: `make(options)` answers a new window. */
function guardedWindowMaker({ BrowserWindow, app, openExternal, logError }) {
  return (options) => {
    const win = new BrowserWindow(options);
    guardWindow(win.webContents, { app, openExternal, logError });
    guardPermissions(win.webContents.session, { app });
    return win;
  };
}

module.exports = { guardedWindowMaker };
